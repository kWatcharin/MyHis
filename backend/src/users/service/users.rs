//! บัญชีผู้ใช้: รายชื่อ, สร้าง (ได้ลิงก์ลงทะเบียน), แก้ข้อมูล/ชุด role, เปิด-ปิดใช้งาน, รีเซ็ตรหัสผ่าน (ได้ลิงก์ตั้งรหัสใหม่)
//! (docs/auth-design.md ข้อ 5, 8, 10)
//!
//! **ไม่มีรหัสผ่านชั่วคราวแล้ว:** สร้างผู้ใช้/รีเซ็ตรหัส ตั้ง password_hash เป็น hash ของค่าสุ่มที่ทิ้งทันที (login ไม่มีทาง
//! ผ่าน) แล้วออกลิงก์ใช้ครั้งเดียวอายุ 3 วัน — ผู้ใช้ตั้งรหัสเองผ่านลิงก์ (โมดูล registration) แอดมินไม่เคยเห็นรหัส
//!
//! **กันแอดมินล็อกตัวเองออก:** ปิดบัญชีตัวเอง, รีเซ็ตรหัสของตัวเอง และแทน role ของตัวเองด้วยชุดที่ไม่มี
//! `user.manage` ถูกปฏิเสธ — ผู้กระทำมี `user.manage` เสมอ (require_user_manage) และหลังทุกคำสั่งในไฟล์นี้
//! ยังเป็นผู้ใช้ active ที่มี `user.manage` อยู่ ระบบจึงไม่มีทางเหลือแอดมิน 0 คนผ่านหน้าจัดการผู้ใช้

use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::{map_unique_violation, require_user_manage, UsersError};
use crate::auth::CurrentUser;
use crate::domain::{PrenameId, RoleId, UserId};
use crate::users::repository::{self, NewRegistrationLink, UserDetailsWrite};
use crate::{password, token};

/// อายุของลิงก์ลงทะเบียน/ตั้งรหัสผ่านใหม่ (เจ้าของระบบเลือก 3 วัน)
const REGISTRATION_LINK_TTL_DAYS: i64 = 3;

/// username หลัง trim + ตัวพิมพ์เล็ก: ยาว 3–32 ตัว เฉพาะ a-z 0-9 . _ - (พิมพ์บนมือถือได้ง่าย ไม่มีช่องว่าง)
const USERNAME_MIN_CHARS: usize = 3;
const USERNAME_MAX_CHARS: usize = 32;
const DISPLAY_NAME_MAX_CHARS: usize = 100;
/// ชื่อ/นามสกุล/ตำแหน่ง ต่อช่อง
const PROFILE_TEXT_MAX_CHARS: usize = 100;

/// ลิงก์ที่เพิ่งออก — token ดิบอยู่ในนี้ (ส่งกลับใน response ครั้งเดียว) DB เก็บแค่ hash
pub struct IssuedLink {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

impl std::fmt::Debug for IssuedLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuedLink")
            .field("token", &"[redacted]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[derive(Debug)]
pub struct CreatedUser {
    pub public_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub role: repository::RoleRef,
    pub link: IssuedLink,
}

/// แทนที่ข้อมูลผู้ใช้ทั้งชุดจากฟอร์มแก้ไขของแอดมิน (`PUT /users/{id}`) — None = ล้างค่า
pub struct ReplaceUserInput<'a> {
    pub display_name: &'a str,
    pub prename_public_id: Option<Uuid>,
    pub first_name: Option<&'a str>,
    pub last_name: Option<&'a str>,
    pub position: Option<&'a str>,
    pub role_public_ids: &'a [Uuid],
}

fn new_link(now: DateTime<Utc>) -> (IssuedLink, String) {
    let token = token::generate_token();
    let token_hash = token::hash_token(&token);
    let link = IssuedLink {
        token,
        expires_at: now + Duration::days(REGISTRATION_LINK_TTL_DAYS),
    };
    (link, token_hash)
}

/// hash ของบัญชีที่ยังไม่มีรหัสผ่าน — argon2 ของค่าสุ่ม 256 บิตที่ทิ้งทันที ไม่มีใครรู้ค่าเดิม login จึงไม่มีทางผ่าน
/// (ตอบ "invalid credentials" เหมือนรหัสผิดทุกประการ ไม่ต้องแก้ตรรกะ login)
async fn unusable_password_hash() -> Result<String, UsersError> {
    let secret = token::generate_token();
    tokio::task::spawn_blocking(move || password::hash(&secret))
        .await
        .map_err(|_| password::PasswordError::Hash)?
        .map_err(UsersError::from)
}

/// username ที่เก็บจริง = trim + ตัวพิมพ์เล็ก (login normalize แบบเดียวกัน — auth::service::login)
fn normalize_username(raw: &str) -> Result<String, UsersError> {
    let username = raw.trim().to_lowercase();
    let length = username.chars().count();
    let allowed = username
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if !(USERNAME_MIN_CHARS..=USERNAME_MAX_CHARS).contains(&length) || !allowed {
        return Err(UsersError::Validation("invalid username".to_string()));
    }
    Ok(username)
}

fn normalize_display_name(raw: &str) -> Result<String, UsersError> {
    let display_name = raw.trim();
    if display_name.is_empty() {
        return Err(UsersError::Validation(
            "display name is required".to_string(),
        ));
    }
    if display_name.chars().count() > DISPLAY_NAME_MAX_CHARS {
        return Err(UsersError::Validation(format!(
            "display name must be at most {DISPLAY_NAME_MAX_CHARS} characters"
        )));
    }
    Ok(display_name.to_string())
}

/// ช่องข้อความไม่บังคับ (ชื่อ/สกุล/ตำแหน่ง): trim แล้วว่าง = None
fn normalize_optional_text(raw: Option<&str>, field: &str) -> Result<Option<String>, UsersError> {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.chars().count() > PROFILE_TEXT_MAX_CHARS {
        return Err(UsersError::Validation(format!(
            "{field} must be at most {PROFILE_TEXT_MAX_CHARS} characters"
        )));
    }
    Ok(Some(value.to_string()))
}

async fn resolve_user_id(pool: &PgPool, public_id: Uuid) -> Result<UserId, UsersError> {
    repository::find_user_id_by_public_id(pool, public_id)
        .await?
        .ok_or(UsersError::UserNotFound)
}

async fn resolve_prename(
    pool: &PgPool,
    public_id: Option<Uuid>,
) -> Result<Option<PrenameId>, UsersError> {
    let Some(public_id) = public_id else {
        return Ok(None);
    };
    repository::find_prename_id(pool, public_id)
        .await?
        .map(Some)
        .ok_or_else(|| UsersError::Validation("unknown prename".to_string()))
}

/// ตัด uuid ซ้ำ (คงลำดับเดิม) + บังคับอย่างน้อย 1 role — ตรวจก่อนหาผู้ใช้/role
fn dedupe_role_ids(role_public_ids: &[Uuid]) -> Result<Vec<Uuid>, UsersError> {
    let mut unique_role_ids: Vec<Uuid> = Vec::with_capacity(role_public_ids.len());
    for role_public_id in role_public_ids {
        if !unique_role_ids.contains(role_public_id) {
            unique_role_ids.push(*role_public_id);
        }
    }
    if unique_role_ids.is_empty() {
        return Err(UsersError::Validation(
            "a user must have at least one role".to_string(),
        ));
    }
    Ok(unique_role_ids)
}

/// หา role ทั้งชุดด้วย query เดียว (ไม่ครบ = RoleNotFound) + กฎกันตัวเอง: แก้ role ของตัวเองต้องเหลือ user.manage
async fn resolve_roles(
    pool: &PgPool,
    actor: &CurrentUser,
    target: UserId,
    unique_role_ids: &[Uuid],
) -> Result<Vec<RoleId>, UsersError> {
    let roles = repository::find_roles_for_assignment(pool, unique_role_ids).await?;
    if roles.len() != unique_role_ids.len() {
        return Err(UsersError::RoleNotFound);
    }
    if target == actor.user.id && !roles.iter().any(|role| role.grants_user_manage) {
        return Err(UsersError::Validation(
            "cannot remove your own user management permission".to_string(),
        ));
    }
    Ok(roles.iter().map(|role| role.id).collect())
}

pub async fn list_users(
    pool: &PgPool,
    actor: &CurrentUser,
) -> Result<Vec<repository::UserWithRoles>, UsersError> {
    require_user_manage(actor)?;
    Ok(repository::list_users(pool).await?)
}

/// สร้างบัญชีที่ยังไม่ลงทะเบียน + ลิงก์ลงทะเบียน — ผู้ใช้กรอกข้อมูลตัวเองและตั้งรหัสผ่านผ่านลิงก์ (`now` = เวลาออกลิงก์)
pub async fn create_user(
    pool: &PgPool,
    actor: &CurrentUser,
    username: &str,
    display_name: &str,
    role_public_id: Uuid,
    now: DateTime<Utc>,
) -> Result<CreatedUser, UsersError> {
    require_user_manage(actor)?;
    let username = normalize_username(username)?;
    let display_name = normalize_display_name(display_name)?;

    let role = repository::find_role_by_public_id(pool, role_public_id)
        .await?
        .ok_or(UsersError::RoleNotFound)?;

    let password_hash = unusable_password_hash().await?;
    let (link, token_hash) = new_link(now);

    let (_, public_id) = repository::insert_user_with_role(
        pool,
        actor.user.id,
        &username,
        &display_name,
        &password_hash,
        role.id,
        &NewRegistrationLink {
            token_hash: &token_hash,
            expires_at: link.expires_at,
        },
    )
    .await
    .map_err(|e| map_unique_violation(e, "username already taken"))?;

    Ok(CreatedUser {
        public_id,
        username,
        display_name,
        role: repository::RoleRef {
            public_id: role.public_id,
            name: role.name,
        },
        link,
    })
}

/// แก้ชื่อที่แสดง และ/หรือ เปิด-ปิดใช้งาน (None = ไม่แตะ, ไม่ส่งอะไรเลย = ไม่ทำอะไร)
pub async fn update_user(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    display_name: Option<&str>,
    is_active: Option<bool>,
) -> Result<(), UsersError> {
    require_user_manage(actor)?;
    let display_name = display_name.map(normalize_display_name).transpose()?;
    let id = resolve_user_id(pool, public_id).await?;

    if is_active == Some(false) && id == actor.user.id {
        return Err(UsersError::Validation(
            "cannot deactivate your own account".to_string(),
        ));
    }
    if display_name.is_none() && is_active.is_none() {
        return Ok(());
    }

    repository::update_user(
        pool,
        actor.user.id,
        id,
        public_id,
        display_name.as_deref(),
        is_active,
    )
    .await?;
    Ok(())
}

/// แทนที่ชื่อที่แสดง + คำนำหน้า/ชื่อ/สกุล/ตำแหน่ง + ชุด role ในคำสั่งเดียว (ฟอร์มแก้ไขผู้ใช้)
pub async fn replace_user(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    input: ReplaceUserInput<'_>,
) -> Result<(), UsersError> {
    require_user_manage(actor)?;
    let display_name = normalize_display_name(input.display_name)?;
    let first_name = normalize_optional_text(input.first_name, "first name")?;
    let last_name = normalize_optional_text(input.last_name, "last name")?;
    let position = normalize_optional_text(input.position, "position")?;
    let unique_role_ids = dedupe_role_ids(input.role_public_ids)?;

    let id = resolve_user_id(pool, public_id).await?;
    let prename_id = resolve_prename(pool, input.prename_public_id).await?;
    let role_ids = resolve_roles(pool, actor, id, &unique_role_ids).await?;

    repository::replace_user(
        pool,
        actor.user.id,
        id,
        public_id,
        &UserDetailsWrite {
            display_name: &display_name,
            prename_id,
            first_name: first_name.as_deref(),
            last_name: last_name.as_deref(),
            position: position.as_deref(),
        },
        &role_ids,
    )
    .await?;
    Ok(())
}

/// รีเซ็ตรหัสผ่าน: รหัสเดิมใช้ไม่ได้ทันที + ปลดล็อก + เพิกถอน session ทั้งหมด + ลิงก์ตั้งรหัสใหม่ (แทนลิงก์เดิม)
/// ใช้ออกลิงก์ใหม่ให้คนที่ยังไม่ลงทะเบียน (ลิงก์หาย/หมดอายุ) ด้วย — รีเซ็ตของตัวเองไม่ได้ (จะเตะ session ปัจจุบันหลุด)
/// ให้ใช้ POST /auth/password แทน
pub async fn reset_password(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    now: DateTime<Utc>,
) -> Result<IssuedLink, UsersError> {
    require_user_manage(actor)?;
    let id = resolve_user_id(pool, public_id).await?;
    if id == actor.user.id {
        return Err(UsersError::Validation(
            "cannot reset your own password".to_string(),
        ));
    }

    let password_hash = unusable_password_hash().await?;
    let (link, token_hash) = new_link(now);
    repository::reset_password(
        pool,
        actor.user.id,
        id,
        public_id,
        &password_hash,
        &NewRegistrationLink {
            token_hash: &token_hash,
            expires_at: link.expires_at,
        },
    )
    .await?;
    Ok(link)
}

/// แทนชุด role ทั้งหมดของผู้ใช้ (full replace) — uuid ซ้ำในคำขอถูกตัดทิ้ง หา role ทั้งชุดด้วย query เดียว
pub async fn replace_user_roles(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    role_public_ids: &[Uuid],
) -> Result<(), UsersError> {
    require_user_manage(actor)?;
    let unique_role_ids = dedupe_role_ids(role_public_ids)?;
    let id = resolve_user_id(pool, public_id).await?;
    let role_ids = resolve_roles(pool, actor, id, &unique_role_ids).await?;
    repository::replace_user_roles(pool, actor.user.id, id, public_id, &role_ids).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_is_trimmed_and_lowercased() {
        assert_eq!(
            normalize_username("  Nurse.Som_1-a ").unwrap(),
            "nurse.som_1-a"
        );
    }

    #[test]
    fn username_rejects_bad_length_or_characters() {
        let too_long = "a".repeat(33);
        for bad in [
            "ab",
            "   ",
            too_long.as_str(),
            "สมศรี",
            "nurse som",
            "nurse@home",
        ] {
            assert!(
                matches!(normalize_username(bad), Err(UsersError::Validation(_))),
                "{bad:?}"
            );
        }
        assert!(normalize_username(&"a".repeat(32)).is_ok());
    }

    #[test]
    fn display_name_is_trimmed_and_bounded() {
        assert_eq!(normalize_display_name("  สมศรี ใจดี ").unwrap(), "สมศรี ใจดี");
        assert!(normalize_display_name("   ").is_err());
        assert!(normalize_display_name(&"ก".repeat(100)).is_ok());
        assert!(normalize_display_name(&"ก".repeat(101)).is_err());
    }

    #[test]
    fn optional_profile_text_trims_blank_to_none_and_bounds_length() {
        assert_eq!(
            normalize_optional_text(Some("  พยาบาลวิชาชีพ "), "position").unwrap(),
            Some("พยาบาลวิชาชีพ".to_string())
        );
        assert_eq!(
            normalize_optional_text(Some("   "), "position").unwrap(),
            None
        );
        assert_eq!(normalize_optional_text(None, "position").unwrap(), None);
        let err = normalize_optional_text(Some(&"ก".repeat(101)), "position").unwrap_err();
        assert!(
            matches!(err, UsersError::Validation(m) if m == "position must be at most 100 characters")
        );
    }

    #[test]
    fn new_link_expires_after_ttl_and_stores_only_hash() {
        let now = Utc::now();
        let (link, token_hash) = new_link(now);
        assert_eq!(link.expires_at, now + Duration::days(3));
        assert_eq!(token_hash, token::hash_token(&link.token));
        assert_ne!(token_hash, link.token);
        assert!(!format!("{link:?}").contains(&link.token));
    }

    #[test]
    fn duplicate_role_ids_are_removed_in_order() {
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        assert_eq!(dedupe_role_ids(&[a, b, a]).unwrap(), vec![a, b]);
        assert!(dedupe_role_ids(&[]).is_err());
    }
}
