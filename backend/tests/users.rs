//! เทสโมดูล users (หน้า "ผู้ใช้งาน") บน DB สะอาด (`#[sqlx::test]`):
//! - ระดับ service: สิทธิ์ราย action (`user.*`), กฎข้อมูลขาเข้า, ลิงก์ลงทะเบียน (แทนรหัสผ่านชั่วคราว), กันแอดมินล็อกตัวเองออก,
//!   ผลต่อ session, audit
//! - ระดับ router (ยิงผ่าน `app::router` จริงด้วย cookie จริง): ปิดบัญชี/รีเซ็ตรหัสแล้ว session เดิมใช้ไม่ได้,
//!   ผู้ใช้ใหม่ลงทะเบียนผ่านลิงก์แล้วเข้าระบบได้, ผู้ไม่มีสิทธิ์ถูกปฏิเสธ
//!
//! กรณีขอบของลิงก์ (หมดอายุ/ใช้ซ้ำ/ถูกแทน/บัญชีถูกปิด) อยู่ใน tests/registration.rs
//! รหัสผ่านทั้งหมดเป็นค่าทดสอบ ไม่ใช่ของจริง

mod common;

use axum::http::{Method, StatusCode};
use chrono::{DateTime, Duration, Utc};
use my_his_backend::auth::CurrentUser;
use my_his_backend::token;
use my_his_backend::users::{service, UsersError};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const ADMIN_PASSWORD: &str = "admin-test-password-1";
const NURSE_PASSWORD: &str = "nurse-test-password-1";

async fn role_uuid(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("select uuid from roles where name = $1")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn internal_id(pool: &PgPool, public_id: Uuid) -> i32 {
    sqlx::query_scalar("select id from users where uuid = $1")
        .bind(public_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn is_active(pool: &PgPool, user_id: i32) -> bool {
    sqlx::query_scalar("select is_active from users where id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn insert_session(pool: &PgPool, user_id: i32, token_hash: &str) {
    sqlx::query(
        "insert into sessions (token_hash, user_id, idle_expires_at, absolute_expires_at)
         values ($1, $2, now() + interval '1 hour', now() + interval '1 day')",
    )
    .bind(token_hash)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn session_count(pool: &PgPool, user_id: i32) -> i64 {
    sqlx::query_scalar("select count(*) from sessions where user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn role_names(pool: &PgPool, user_id: i32) -> Vec<String> {
    sqlx::query_scalar(
        "select r.name from user_roles ur join roles r on r.id = ur.role_id
         where ur.user_id = $1 order by r.name",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// action ของ audit ที่ผู้ใช้คนนี้เป็นเป้าหมาย เรียงตามเวลาที่เกิด
async fn audit_actions(pool: &PgPool, target: Uuid) -> Vec<String> {
    sqlx::query_scalar(
        "select action from audit_log where target_type = 'user' and target_id = $1 order by id",
    )
    .bind(target.to_string())
    .fetch_all(pool)
    .await
    .unwrap()
}

/// (หมดอายุ, ใช้แล้วเมื่อ, ถูกแทนเมื่อ, ผู้ออก) ของลิงก์จาก token ดิบ
type LinkState = (
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    Option<i32>,
);

async fn link_state(pool: &PgPool, raw_token: &str) -> LinkState {
    sqlx::query_as(
        "select expires_at, used_at, revoked_at, created_by from registration_links where token_hash = $1",
    )
    .bind(token::hash_token(raw_token))
    .fetch_one(pool)
    .await
    .unwrap()
}

/// ผู้ใช้เป้าหมาย (role nurse) สร้างผ่าน service จริง — คืน (uuid, id ภายใน, token ของลิงก์ลงทะเบียน)
async fn create_nurse(pool: &PgPool, admin: &CurrentUser, username: &str) -> (Uuid, i32, String) {
    let nurse_role = role_uuid(pool, "nurse").await;
    let created = service::create_user(pool, admin, username, "ผู้ใช้ทดสอบ", nurse_role, Utc::now())
        .await
        .unwrap();
    let id = internal_id(pool, created.public_id).await;
    (created.public_id, id, created.link.token)
}

fn is_validation(err: &UsersError, expected: &str) -> bool {
    matches!(err, UsersError::Validation(message) if message == expected)
}

fn details<'a>(display_name: &'a str, role_ids: &'a [Uuid]) -> service::ReplaceUserInput<'a> {
    service::ReplaceUserInput {
        display_name,
        prename_public_id: None,
        first_name: None,
        last_name: None,
        position: None,
        role_public_ids: role_ids,
    }
}

// ============================================================================
// ระดับ service
// ============================================================================

#[sqlx::test]
async fn user_management_requires_user_manage(pool: PgPool) {
    let nurse = common::nurse(&pool).await;
    let target = nurse.user.public_id;
    let nurse_role = role_uuid(&pool, "nurse").await;
    let now = Utc::now();

    assert!(matches!(
        service::list_users(&pool, &nurse).await,
        Err(UsersError::Forbidden)
    ));
    assert!(matches!(
        service::create_user(&pool, &nurse, "new.user", "New", nurse_role, now).await,
        Err(UsersError::Forbidden)
    ));
    assert!(matches!(
        service::update_user(&pool, &nurse, target, Some("x"), None).await,
        Err(UsersError::Forbidden)
    ));
    assert!(matches!(
        service::replace_user(&pool, &nurse, target, details("x", &[nurse_role])).await,
        Err(UsersError::Forbidden)
    ));
    assert!(matches!(
        service::reset_password(&pool, &nurse, target, now).await,
        Err(UsersError::Forbidden)
    ));
    assert!(matches!(
        service::replace_user_roles(&pool, &nurse, target, &[nurse_role]).await,
        Err(UsersError::Forbidden)
    ));
}

#[sqlx::test]
async fn create_user_issues_registration_link_instead_of_password(pool: PgPool) {
    let admin = common::admin(&pool).await;
    let nurse_role = role_uuid(&pool, "nurse").await;
    let now = Utc::now();

    let created = service::create_user(&pool, &admin, "  Nurse.Som ", "  สมศรี ", nurse_role, now)
        .await
        .unwrap();
    assert_eq!(created.username, "nurse.som");
    assert_eq!(created.display_name, "สมศรี");
    assert_eq!(created.role.name, "nurse");
    assert_eq!(created.link.token.len(), 64);
    assert_eq!(created.link.expires_at, now + Duration::days(3));
    // Debug ไม่พิมพ์ token
    assert!(!format!("{created:?}").contains(&created.link.token));

    let (password_hash, active, must_change, registered_at): (
        String,
        bool,
        bool,
        Option<DateTime<Utc>>,
    ) = sqlx::query_as(
        "select password_hash, is_active, must_change_password, registered_at
         from users where uuid = $1",
    )
    .bind(created.public_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(active);
    assert!(!must_change);
    assert!(registered_at.is_none(), "ยังไม่ลงทะเบียนจนกว่าจะตั้งรหัสผ่านผ่านลิงก์");
    // รหัสผ่านเป็น hash ของค่าสุ่มที่ทิ้งไปแล้ว — token ของลิงก์ใช้เป็นรหัสไม่ได้
    assert!(!common::verify_password(&created.link.token, &password_hash).await);

    // DB เก็บแค่ hash ของ token พร้อมผู้ออกลิงก์
    let (expires_at, used_at, revoked_at, created_by) =
        link_state(&pool, &created.link.token).await;
    // Postgres เก็บละเอียดระดับไมโครวินาที (ของ Rust เป็นนาโนวินาที)
    assert!((expires_at - created.link.expires_at).abs() < Duration::milliseconds(1));
    assert!(used_at.is_none() && revoked_at.is_none());
    assert_eq!(created_by, Some(admin.user.id.0));
    let raw_tokens: i64 =
        sqlx::query_scalar("select count(*) from registration_links where token_hash = $1")
            .bind(&created.link.token)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(raw_tokens, 0, "token ดิบต้องไม่อยู่ใน DB");

    let id = internal_id(&pool, created.public_id).await;
    assert_eq!(role_names(&pool, id).await, ["nurse"]);
    let (actor, metadata): (Option<i32>, Option<Value>) = sqlx::query_as(
        "select actor_user_id, metadata from audit_log
         where action = 'user_created' and target_type = 'user' and target_id = $1",
    )
    .bind(created.public_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(actor, Some(admin.user.id.0));
    assert!(metadata.is_none());
}

#[sqlx::test]
async fn create_user_rejects_invalid_input_and_duplicates(pool: PgPool) {
    let admin = common::admin(&pool).await;
    let nurse_role = role_uuid(&pool, "nurse").await;
    let now = Utc::now();

    for bad in ["ab", "   ", "สมศรี", "nurse som", "nurse@home"] {
        let err = service::create_user(&pool, &admin, bad, "Name", nurse_role, now)
            .await
            .unwrap_err();
        assert!(is_validation(&err, "invalid username"), "{bad:?}: {err:?}");
    }
    let err = service::create_user(&pool, &admin, "valid.name", "   ", nurse_role, now)
        .await
        .unwrap_err();
    assert!(is_validation(&err, "display name is required"), "{err:?}");
    let err = service::create_user(
        &pool,
        &admin,
        "valid.name",
        &"ก".repeat(101),
        nurse_role,
        now,
    )
    .await
    .unwrap_err();
    assert!(
        is_validation(&err, "display name must be at most 100 characters"),
        "{err:?}"
    );
    let err = service::create_user(&pool, &admin, "valid.name", "Name", Uuid::new_v4(), now)
        .await
        .unwrap_err();
    assert!(matches!(err, UsersError::RoleNotFound), "{err:?}");

    service::create_user(&pool, &admin, "valid.name", "Name", nurse_role, now)
        .await
        .unwrap();
    // ซ้ำแบบไม่สนตัวพิมพ์/ช่องว่าง (เก็บเป็นตัวเล็กที่ trim แล้ว)
    let err = service::create_user(&pool, &admin, " VALID.NAME ", "Other", nurse_role, now)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, UsersError::Conflict(message) if message == "username already taken"),
        "{err:?}"
    );

    // คำขอที่ถูกปฏิเสธไม่สร้างแถวใด ๆ — มีแค่ผู้กระทำ (แอดมิน) กับผู้ใช้ที่สร้างสำเร็จ 1 คน และลิงก์ 1 อัน
    let users: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let links: i64 = sqlx::query_scalar("select count(*) from registration_links")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((users, links), (2, 1));
}

#[sqlx::test]
async fn update_user_trims_name_and_deactivation_revokes_sessions(pool: PgPool) {
    let admin = common::admin(&pool).await;
    let (target, target_id, _) = create_nurse(&pool, &admin, "target.user").await;
    insert_session(&pool, target_id, "session-hash-a").await;
    insert_session(&pool, target_id, "session-hash-b").await;

    service::update_user(&pool, &admin, target, Some("  ชื่อใหม่ "), None)
        .await
        .unwrap();
    let name: String = sqlx::query_scalar("select display_name from users where id = $1")
        .bind(target_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "ชื่อใหม่");
    // แก้ชื่อไม่เตะใครออกจากระบบ
    assert_eq!(session_count(&pool, target_id).await, 2);

    let err = service::update_user(&pool, &admin, target, Some("   "), None)
        .await
        .unwrap_err();
    assert!(is_validation(&err, "display name is required"), "{err:?}");

    service::update_user(&pool, &admin, target, None, Some(false))
        .await
        .unwrap();
    assert!(!is_active(&pool, target_id).await);
    assert_eq!(session_count(&pool, target_id).await, 0);

    service::update_user(&pool, &admin, target, None, Some(true))
        .await
        .unwrap();
    assert!(is_active(&pool, target_id).await);

    // ไม่ส่งอะไรเลย = ไม่ทำอะไร (ไม่มี audit เพิ่ม)
    service::update_user(&pool, &admin, target, None, None)
        .await
        .unwrap();

    assert_eq!(
        audit_actions(&pool, target).await,
        [
            "user_created",
            "user_updated",
            "user_deactivated",
            "user_activated"
        ]
    );

    let err = service::update_user(&pool, &admin, Uuid::new_v4(), Some("x"), None)
        .await
        .unwrap_err();
    assert!(matches!(err, UsersError::UserNotFound), "{err:?}");
}

#[sqlx::test]
async fn replace_user_updates_details_and_roles_in_one_step(pool: PgPool) {
    let admin = common::admin(&pool).await;
    let (target, target_id, _) = create_nurse(&pool, &admin, "profile.user").await;
    let nurse_role = role_uuid(&pool, "nurse").await;
    let admin_role = role_uuid(&pool, "admin").await;
    let prename_id = common::insert_prename(&pool, admin.user.id.0, "นาง").await;
    let prename_uuid: Uuid = sqlx::query_scalar("select uuid from prename where id = $1")
        .bind(prename_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let roles = [nurse_role, admin_role, nurse_role];
    service::replace_user(
        &pool,
        &admin,
        target,
        service::ReplaceUserInput {
            display_name: "  พี่ศรี ",
            prename_public_id: Some(prename_uuid),
            first_name: Some(" สมศรี "),
            last_name: Some("ใจดี"),
            position: Some("   "),
            role_public_ids: &roles,
        },
    )
    .await
    .unwrap();

    let row: (
        String,
        Option<i32>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "select display_name, prename_id, first_name, last_name, position from users where id = $1",
    )
    .bind(target_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row,
        (
            "พี่ศรี".to_string(),
            Some(prename_id),
            Some("สมศรี".to_string()),
            Some("ใจดี".to_string()),
            None // ช่องว่างล้วน = ล้างค่า
        )
    );
    assert_eq!(role_names(&pool, target_id).await, ["admin", "nurse"]);
    assert_eq!(
        audit_actions(&pool, target).await,
        ["user_created", "user_roles_replaced", "user_updated"]
    );

    // ชุด role เดิม → ไม่มี audit user_roles_replaced เพิ่ม
    service::replace_user(
        &pool,
        &admin,
        target,
        details("พี่ศรี", &[admin_role, nurse_role]),
    )
    .await
    .unwrap();
    assert_eq!(
        audit_actions(&pool, target).await,
        [
            "user_created",
            "user_roles_replaced",
            "user_updated",
            "user_updated"
        ]
    );

    // ค่าผิด → ไม่เขียนอะไรเลย
    let nurse_only = [nurse_role];
    let long_name = "ก".repeat(101);
    let mut bad = details("ชื่อใหม่", &nurse_only);
    bad.first_name = Some(long_name.as_str());
    let err = service::replace_user(&pool, &admin, target, bad)
        .await
        .unwrap_err();
    assert!(
        is_validation(&err, "first name must be at most 100 characters"),
        "{err:?}"
    );
    let mut bad = details("ชื่อใหม่", &nurse_only);
    bad.prename_public_id = Some(Uuid::new_v4());
    let err = service::replace_user(&pool, &admin, target, bad)
        .await
        .unwrap_err();
    assert!(is_validation(&err, "unknown prename"), "{err:?}");
    common::soft_delete_row(&pool, "prename", prename_id, admin.user.id.0).await;
    let mut bad = details("ชื่อใหม่", &nurse_only);
    bad.prename_public_id = Some(prename_uuid);
    let err = service::replace_user(&pool, &admin, target, bad)
        .await
        .unwrap_err();
    assert!(
        is_validation(&err, "unknown prename"),
        "ลบแล้วเลือกไม่ได้: {err:?}"
    );
    let err = service::replace_user(&pool, &admin, target, details("ชื่อใหม่", &[]))
        .await
        .unwrap_err();
    assert!(
        is_validation(&err, "a user must have at least one role"),
        "{err:?}"
    );
    let err = service::replace_user(&pool, &admin, Uuid::new_v4(), details("x", &[nurse_role]))
        .await
        .unwrap_err();
    assert!(matches!(err, UsersError::UserNotFound), "{err:?}");
    let name: String = sqlx::query_scalar("select display_name from users where id = $1")
        .bind(target_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "พี่ศรี");

    // กฎกันตัวเองใช้กับ PUT ด้วย
    let err = service::replace_user(
        &pool,
        &admin,
        admin.user.public_id,
        details("แอดมิน", &[nurse_role]),
    )
    .await
    .unwrap_err();
    assert!(
        is_validation(&err, "cannot remove your own role management permission"),
        "{err:?}"
    );
}

#[sqlx::test]
async fn admin_cannot_deactivate_or_reset_own_account(pool: PgPool) {
    let admin = common::admin(&pool).await;
    let own = admin.user.public_id;

    let err = service::update_user(&pool, &admin, own, None, Some(false))
        .await
        .unwrap_err();
    assert!(
        is_validation(&err, "cannot deactivate your own account"),
        "{err:?}"
    );
    let err = service::reset_password(&pool, &admin, own, Utc::now())
        .await
        .unwrap_err();
    assert!(
        is_validation(&err, "cannot reset your own password"),
        "{err:?}"
    );

    // แก้ชื่อตัวเอง / ยืนยันสถานะเปิดใช้งานของตัวเอง ทำได้ตามปกติ
    service::update_user(&pool, &admin, own, Some("แอดมิน"), Some(true))
        .await
        .unwrap();
    assert!(is_active(&pool, admin.user.id.0).await);
}

#[sqlx::test]
async fn reset_password_replaces_link_unlocks_and_revokes_sessions(pool: PgPool) {
    let admin = common::admin(&pool).await;
    let (target, target_id, first_token) = create_nurse(&pool, &admin, "locked.user").await;
    let hash_before: String = sqlx::query_scalar("select password_hash from users where id = $1")
        .bind(target_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "update users set must_change_password = true, failed_attempts = 5,
                          locked_until = now() + interval '15 minutes'
         where id = $1",
    )
    .bind(target_id)
    .execute(&pool)
    .await
    .unwrap();
    insert_session(&pool, target_id, "session-hash-c").await;

    let now = Utc::now();
    let link = service::reset_password(&pool, &admin, target, now)
        .await
        .unwrap();
    assert_ne!(link.token, first_token);
    assert_eq!(link.expires_at, now + Duration::days(3));

    // ลิงก์เดิมถูกแทน ลิงก์ใหม่ยังใช้ได้
    let (_, _, revoked_at, _) = link_state(&pool, &first_token).await;
    assert!(revoked_at.is_some());
    let (_, used_at, revoked_at, created_by) = link_state(&pool, &link.token).await;
    assert!(used_at.is_none() && revoked_at.is_none());
    assert_eq!(created_by, Some(admin.user.id.0));

    let (password_hash, must_change, failed, locked): (String, bool, i32, Option<DateTime<Utc>>) =
        sqlx::query_as(
            "select password_hash, must_change_password, failed_attempts, locked_until
             from users where id = $1",
        )
        .bind(target_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_ne!(password_hash, hash_before, "รหัสเดิมใช้ไม่ได้ทันที");
    assert!(!must_change, "ไม่มีรหัสชั่วคราวแล้ว");
    assert_eq!(failed, 0);
    assert!(locked.is_none());
    assert_eq!(session_count(&pool, target_id).await, 0);
    assert_eq!(
        audit_actions(&pool, target).await,
        ["user_created", "user_password_reset"]
    );
}

#[sqlx::test]
async fn replace_user_roles_validates_dedupes_and_audits(pool: PgPool) {
    let admin = common::admin(&pool).await;
    let (target, target_id, _) = create_nurse(&pool, &admin, "multi.role").await;
    let nurse_role = role_uuid(&pool, "nurse").await;
    let admin_role = role_uuid(&pool, "admin").await;

    let err = service::replace_user_roles(&pool, &admin, target, &[])
        .await
        .unwrap_err();
    assert!(
        is_validation(&err, "a user must have at least one role"),
        "{err:?}"
    );
    let err = service::replace_user_roles(&pool, &admin, target, &[nurse_role, Uuid::new_v4()])
        .await
        .unwrap_err();
    assert!(matches!(err, UsersError::RoleNotFound), "{err:?}");
    let err = service::replace_user_roles(&pool, &admin, Uuid::new_v4(), &[nurse_role])
        .await
        .unwrap_err();
    assert!(matches!(err, UsersError::UserNotFound), "{err:?}");
    // คำขอที่ล้มไม่แตะ role เดิม
    assert_eq!(role_names(&pool, target_id).await, ["nurse"]);

    // uuid ซ้ำในคำขอถูกตัดทิ้ง (เดิมชน primary key ของ user_roles ได้ 500)
    service::replace_user_roles(&pool, &admin, target, &[nurse_role, admin_role, nurse_role])
        .await
        .unwrap();
    assert_eq!(role_names(&pool, target_id).await, ["admin", "nurse"]);
    assert_eq!(
        audit_actions(&pool, target).await,
        ["user_created", "user_roles_replaced"]
    );
}

#[sqlx::test]
async fn admin_cannot_remove_own_role_management_permission(pool: PgPool) {
    let admin = common::admin(&pool).await;
    let own = admin.user.public_id;
    let nurse_role = role_uuid(&pool, "nurse").await;
    let admin_role = role_uuid(&pool, "admin").await;
    // role ที่แอดมินสร้างเองซึ่งให้ role.view + role.update ก็นับ (กฎดูที่สิทธิ์ ไม่ผูกกับชื่อ role "admin")
    let backup_admin_role: Uuid =
        sqlx::query_scalar("insert into roles (name) values ('ผู้ดูแลสำรอง') returning uuid")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "insert into role_permissions (role_id, permission)
         select id, unnest(array['role.view', 'role.update']) from roles where uuid = $1",
    )
    .bind(backup_admin_role)
    .execute(&pool)
    .await
    .unwrap();

    let err = service::replace_user_roles(&pool, &admin, own, &[nurse_role])
        .await
        .unwrap_err();
    assert!(
        is_validation(&err, "cannot remove your own role management permission"),
        "{err:?}"
    );

    service::replace_user_roles(&pool, &admin, own, &[nurse_role, backup_admin_role])
        .await
        .unwrap();
    service::replace_user_roles(&pool, &admin, own, &[admin_role])
        .await
        .unwrap();

    // ผู้ใช้คนอื่นตั้งเป็นชุดที่จัดการ role ไม่ได้ได้ตามปกติ
    let (other, _, _) = create_nurse(&pool, &admin, "other.user").await;
    service::replace_user_roles(&pool, &admin, other, &[nurse_role])
        .await
        .unwrap();
}

#[sqlx::test]
async fn list_users_returns_profile_lock_and_registration_state(pool: PgPool) {
    let admin = common::admin(&pool).await;
    let (target, target_id, _) = create_nurse(&pool, &admin, "listed.user").await;
    sqlx::query("update users set locked_until = now() + interval '15 minutes' where id = $1")
        .bind(target_id)
        .execute(&pool)
        .await
        .unwrap();

    let users = service::list_users(&pool, &admin).await.unwrap();
    let listed = users.iter().find(|user| user.public_id == target).unwrap();
    assert_eq!(listed.username, "listed.user");
    assert!(listed.is_active);
    assert!(listed.locked_until.is_some());
    assert!(listed.registered_at.is_none());
    assert!(listed.registration_expires_at.is_some());
    assert!(listed.first_name.is_none() && listed.prename_name.is_none());
    let roles: Vec<&str> = listed.roles.iter().map(|role| role.name.as_str()).collect();
    assert_eq!(roles, ["nurse"]);

    // ใช้ลิงก์แล้ว → ไม่มีลิงก์ค้าง (ลิงก์ที่ถูกแทน/ใช้แล้วไม่นับ)
    sqlx::query("update registration_links set used_at = now() where user_id = $1")
        .bind(target_id)
        .execute(&pool)
        .await
        .unwrap();
    let users = service::list_users(&pool, &admin).await.unwrap();
    let listed = users.iter().find(|user| user.public_id == target).unwrap();
    assert!(listed.registration_expires_at.is_none());
}

// ============================================================================
// ระดับ router — session/cookie จริง
// ============================================================================

#[sqlx::test]
async fn session_of_inactive_user_is_rejected(pool: PgPool) {
    let app = common::test_app(pool.clone()).await;
    let (nurse_id, _) =
        common::insert_user_with_password(&pool, "nurse.one", NURSE_PASSWORD, "nurse").await;
    let (cookie, _) = common::login(&app, "nurse.one", NURSE_PASSWORD).await;
    let me = common::send(&app, Method::GET, "/api/auth/me", Some(&cookie), None).await;
    assert_eq!(me.status, StatusCode::OK);

    // ปิดบัญชีตรงใน DB โดยที่ session ยังอยู่ — ตัวตรวจ session ต้องปฏิเสธเองโดยไม่พึ่งการลบ session
    // (docs/auth-design.md ข้อ 7: เจ้าของ session ต้องยัง is_active)
    sqlx::query("update users set is_active = false where id = $1")
        .bind(nurse_id)
        .execute(&pool)
        .await
        .unwrap();
    let me = common::send(&app, Method::GET, "/api/auth/me", Some(&cookie), None).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn deactivating_through_api_logs_the_user_out(pool: PgPool) {
    let app = common::test_app(pool.clone()).await;
    let (_, admin_uuid) =
        common::insert_user_with_password(&pool, "admin.one", ADMIN_PASSWORD, "admin").await;
    let (_, nurse_uuid) =
        common::insert_user_with_password(&pool, "nurse.one", NURSE_PASSWORD, "nurse").await;
    let (admin_cookie, _) = common::login(&app, "admin.one", ADMIN_PASSWORD).await;
    let (nurse_cookie, _) = common::login(&app, "nurse.one", NURSE_PASSWORD).await;

    let response = common::send(
        &app,
        Method::PATCH,
        &format!("/api/users/{nurse_uuid}"),
        Some(&admin_cookie),
        Some(json!({ "isActive": false })),
    )
    .await;
    assert_eq!(response.status, StatusCode::NO_CONTENT);

    let me = common::send(&app, Method::GET, "/api/auth/me", Some(&nurse_cookie), None).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    // login ใหม่ไม่ได้ (ข้อความกลางเดียวกับรหัสผิด)
    let relogin = common::send(
        &app,
        Method::POST,
        "/api/auth/login",
        None,
        Some(json!({ "username": "nurse.one", "password": NURSE_PASSWORD })),
    )
    .await;
    assert_eq!(relogin.status, StatusCode::UNAUTHORIZED);

    // แอดมินปิดบัญชีตัวเองผ่าน API ไม่ได้
    let response = common::send(
        &app,
        Method::PATCH,
        &format!("/api/users/{admin_uuid}"),
        Some(&admin_cookie),
        Some(json!({ "isActive": false })),
    )
    .await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(response.body["error"], "cannot deactivate your own account");
}

#[sqlx::test]
async fn reset_password_through_api_sends_set_password_link(pool: PgPool) {
    let app = common::test_app(pool.clone()).await;
    common::insert_user_with_password(&pool, "admin.one", ADMIN_PASSWORD, "admin").await;
    let (_, nurse_uuid) =
        common::insert_user_with_password(&pool, "nurse.one", NURSE_PASSWORD, "nurse").await;
    let (admin_cookie, _) = common::login(&app, "admin.one", ADMIN_PASSWORD).await;
    let (nurse_cookie, _) = common::login(&app, "nurse.one", NURSE_PASSWORD).await;

    let response = common::send(
        &app,
        Method::POST,
        &format!("/api/users/{nurse_uuid}/reset-password"),
        Some(&admin_cookie),
        None,
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.get("temporaryPassword").is_none());
    assert!(response.body["registrationExpiresAt"].is_string());
    let link_token = response.body["registrationToken"]
        .as_str()
        .unwrap()
        .to_string();

    // session เดิมและรหัสเดิมใช้ไม่ได้ทันที
    let me = common::send(&app, Method::GET, "/api/auth/me", Some(&nurse_cookie), None).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    let old_password = common::send(
        &app,
        Method::POST,
        "/api/auth/login",
        None,
        Some(json!({ "username": "nurse.one", "password": NURSE_PASSWORD })),
    )
    .await;
    assert_eq!(old_password.status, StatusCode::UNAUTHORIZED);

    // ผู้ใช้ลงทะเบียนแล้ว → ลิงก์เป็นโหมดตั้งรหัสใหม่
    let lookup = common::send(
        &app,
        Method::POST,
        "/api/registration/lookup",
        None,
        Some(json!({ "token": link_token })),
    )
    .await;
    assert_eq!(lookup.status, StatusCode::OK);
    assert_eq!(lookup.body["mode"], "reset");
    let done = common::send(
        &app,
        Method::POST,
        "/api/registration/complete",
        None,
        Some(json!({ "token": link_token, "newPassword": "nurse-new-password-2" })),
    )
    .await;
    assert_eq!(done.status, StatusCode::NO_CONTENT);
    let (_, me) = common::login(&app, "nurse.one", "nurse-new-password-2").await;
    assert_eq!(me["mustChangePassword"], json!(false));
}

#[sqlx::test]
async fn new_user_registers_via_link_then_logs_in(pool: PgPool) {
    let app = common::test_app(pool.clone()).await;
    let (admin_id, _) =
        common::insert_user_with_password(&pool, "admin.one", ADMIN_PASSWORD, "admin").await;
    let (admin_cookie, _) = common::login(&app, "admin.one", ADMIN_PASSWORD).await;
    let nurse_role = role_uuid(&pool, "nurse").await;
    let prename_id = common::insert_prename(&pool, admin_id, "นางสาว").await;
    let prename_uuid: Uuid = sqlx::query_scalar("select uuid from prename where id = $1")
        .bind(prename_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let response = common::send(
        &app,
        Method::POST,
        "/api/users",
        Some(&admin_cookie),
        Some(
            json!({ "username": " New.Nurse ", "displayName": "พยาบาลใหม่", "roleId": nurse_role }),
        ),
    )
    .await;
    assert_eq!(response.status, StatusCode::CREATED);
    assert_eq!(response.body["user"]["username"], "new.nurse");
    assert!(response.body["user"]["registeredAt"].is_null());
    assert!(response.body.get("temporaryPassword").is_none());
    let link_token = response.body["registrationToken"]
        .as_str()
        .unwrap()
        .to_string();

    // ยังไม่ลงทะเบียน → login ไม่ได้ (แม้ใช้ token เป็นรหัส)
    let early = common::send(
        &app,
        Method::POST,
        "/api/auth/login",
        None,
        Some(json!({ "username": "new.nurse", "password": link_token })),
    )
    .await;
    assert_eq!(early.status, StatusCode::UNAUTHORIZED);

    let lookup = common::send(
        &app,
        Method::POST,
        "/api/registration/lookup",
        None,
        Some(json!({ "token": link_token })),
    )
    .await;
    assert_eq!(lookup.status, StatusCode::OK);
    assert_eq!(lookup.body["mode"], "register");
    assert_eq!(lookup.body["username"], "new.nurse");
    assert_eq!(lookup.body["displayName"], "พยาบาลใหม่");
    assert!(lookup.body["prenames"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == json!(prename_uuid) && p["name"] == "นางสาว"));

    let done = common::send(
        &app,
        Method::POST,
        "/api/registration/complete",
        None,
        Some(json!({
            "token": link_token,
            "newPassword": "my-own-password-123",
            "prenameId": prename_uuid,
            "firstName": " สมศรี ",
            "lastName": "ใจดี",
            "position": "พยาบาลวิชาชีพ",
        })),
    )
    .await;
    assert_eq!(done.status, StatusCode::NO_CONTENT);

    let (cookie, me) = common::login(&app, "new.nurse", "my-own-password-123").await;
    assert_eq!(me["mustChangePassword"], json!(false));

    // หน้าแอดมินเห็นข้อมูลที่ผู้ใช้กรอกเอง และไม่มีลิงก์ค้างแล้ว
    let list = common::send(&app, Method::GET, "/api/users", Some(&admin_cookie), None).await;
    let user = list
        .body
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "new.nurse")
        .unwrap()
        .clone();
    assert_eq!(user["prenameName"], "นางสาว");
    assert_eq!(user["firstName"], "สมศรี");
    assert_eq!(user["lastName"], "ใจดี");
    assert_eq!(user["position"], "พยาบาลวิชาชีพ");
    assert!(user["registeredAt"].is_string());
    assert!(user["registrationExpiresAt"].is_null());

    // เปลี่ยนรหัสผ่านของตัวเองภายหลัง (POST /auth/password) ยังใช้ได้ตามปกติ
    let change =
        |current: &str, new: &str| json!({ "currentPassword": current, "newPassword": new });
    let same = common::send(
        &app,
        Method::POST,
        "/api/auth/password",
        Some(&cookie),
        Some(change("my-own-password-123", "my-own-password-123")),
    )
    .await;
    assert_eq!(same.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        same.body["error"],
        "new password must be different from current password"
    );
    let wrong = common::send(
        &app,
        Method::POST,
        "/api/auth/password",
        Some(&cookie),
        Some(change("not-the-current-password", "a-brand-new-password")),
    )
    .await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    let changed = common::send(
        &app,
        Method::POST,
        "/api/auth/password",
        Some(&cookie),
        Some(change("my-own-password-123", "a-brand-new-password")),
    )
    .await;
    assert_eq!(changed.status, StatusCode::NO_CONTENT);
    common::login(&app, "new.nurse", "a-brand-new-password").await;
}

#[sqlx::test]
async fn users_api_requires_user_manage_and_hides_internal_ids(pool: PgPool) {
    let app = common::test_app(pool.clone()).await;
    common::insert_user_with_password(&pool, "admin.one", ADMIN_PASSWORD, "admin").await;
    let (_, nurse_uuid) =
        common::insert_user_with_password(&pool, "nurse.one", NURSE_PASSWORD, "nurse").await;
    let nurse_role = role_uuid(&pool, "nurse").await;

    let (nurse_cookie, _) = common::login(&app, "nurse.one", NURSE_PASSWORD).await;
    let list = common::send(&app, Method::GET, "/api/users", Some(&nurse_cookie), None).await;
    assert_eq!(list.status, StatusCode::FORBIDDEN);
    let create = common::send(
        &app,
        Method::POST,
        "/api/users",
        Some(&nurse_cookie),
        Some(json!({ "username": "sneaky", "displayName": "x", "roleId": nurse_role })),
    )
    .await;
    assert_eq!(create.status, StatusCode::FORBIDDEN);
    let replace = common::send(
        &app,
        Method::PUT,
        &format!("/api/users/{nurse_uuid}"),
        Some(&nurse_cookie),
        Some(json!({ "displayName": "x", "roleIds": [nurse_role] })),
    )
    .await;
    assert_eq!(replace.status, StatusCode::FORBIDDEN);

    let (admin_cookie, _) = common::login(&app, "admin.one", ADMIN_PASSWORD).await;
    let list = common::send(&app, Method::GET, "/api/users", Some(&admin_cookie), None).await;
    assert_eq!(list.status, StatusCode::OK);
    let users = list.body.as_array().unwrap();
    assert_eq!(users.len(), 2);
    for user in users {
        assert!(
            Uuid::parse_str(user["id"].as_str().unwrap()).is_ok(),
            "{user}"
        );
        assert!(user.get("passwordHash").is_none(), "{user}");
        assert!(user.get("lockedUntil").is_some(), "{user}");
        // ผู้ใช้ที่สร้างก่อนมีลิงก์ (helper ของเทส/บัญชีเดิม) ถือว่าลงทะเบียนแล้ว
        assert!(user["registeredAt"].is_string(), "{user}");
        for role in user["roles"].as_array().unwrap() {
            assert!(
                Uuid::parse_str(role["id"].as_str().unwrap()).is_ok(),
                "{role}"
            );
        }
    }
}
