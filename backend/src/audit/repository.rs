//! อ่าน `audit_log` สำหรับหน้าประวัติ (ห้ามรู้เรื่อง HTTP) ใช้ query!/query_as! ตรวจตอน compile
//!
//! **ไม่อ่าน `target_id`** ของเหตุการณ์เข้า/ออก — login ไม่สำเร็จด้วยชื่อที่ไม่มีในระบบเก็บชื่อที่พิมพ์มาไว้ในคอลัมน์นั้น
//! ซึ่งอาจเป็นรหัสผ่านที่พิมพ์ผิดช่อง จึงไม่ให้ออกจาก backend เลย ผู้ใช้ของแถวมาจาก `actor_user_id` เท่านั้น
//!
//! ประวัติการใช้งาน (ทุก action ยกเว้นเข้า/ออก) ใช้ `target_id` แค่เป็น uuid สำหรับ join หาชื่อรายการ ไม่ส่งออกไปตรง ๆ
//! **แถวผู้ป่วยไม่มีชื่อรายการเสมอ** — ไม่ join ตาราง patient (ข้อมูลระบุตัวอยู่ในนั้น) และบังคับเป็น null ซ้ำอีกชั้น
//! รายชื่อ action เข้า/ออกเขียนซ้ำในสอง query ข้างล่าง ต้องตรงกัน
//! (เทส `activities_exclude_auth_events_and_filter_by_bangkok_day` คุมไว้)

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

/// เหตุการณ์เข้า/ออกระบบหนึ่งแถว — user* เป็น None เมื่อไม่รู้ว่าใคร (login ด้วยชื่อที่ไม่มีในระบบ)
#[derive(Debug)]
pub struct AuthEventRow {
    pub action: String,
    pub occurred_at: DateTime<Utc>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub user_public_id: Option<Uuid>,
    pub username: Option<String>,
    pub display_name: Option<String>,
}

/// เหตุการณ์ login สำเร็จ/ไม่สำเร็จ, logout, logout ทุกอุปกรณ์ ในช่วง [from, until) เรียงใหม่→เก่า ไม่เกิน `limit` แถว
/// `host(ip)` ได้ IP เปล่า ๆ (`ip::text` จะติด `/32`)
pub async fn list_auth_events(
    pool: &PgPool,
    from: DateTime<Utc>,
    until: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<AuthEventRow>, RepositoryError> {
    Ok(sqlx::query_as!(
        AuthEventRow,
        r#"select a.action, a.created_at as occurred_at, host(a.ip) as "ip?",
                  a.metadata ->> 'userAgent' as "user_agent?",
                  u.uuid as "user_public_id?", u.username as "username?", u.display_name as "display_name?"
           from audit_log a
           left join users u on u.id = a.actor_user_id
           where a.action in ('login_succeeded', 'login_failed', 'logout', 'logout_all')
             and a.created_at >= $1 and a.created_at < $2
           order by a.created_at desc, a.id desc
           limit $3"#,
        from,
        until,
        limit
    )
    .fetch_all(pool)
    .await?)
}

/// การใช้งานหนึ่งแถว (เพิ่ม/แก้ไข/ลบ/ส่งออก ฯลฯ) — `target_label` = ชื่อรายการที่ไม่ใช่ข้อมูลผู้ป่วย
#[derive(Debug)]
pub struct ActivityRow {
    pub action: String,
    pub occurred_at: DateTime<Utc>,
    pub target_type: Option<String>,
    pub target_label: Option<String>,
    /// เฉพาะ `lab_visit_group_exported`: cd4 | roster และ excel | pdf
    pub export_document: Option<String>,
    pub export_format: Option<String>,
    /// รายละเอียดการเปลี่ยนแปลง `{before?, after?, changed?}` (null = ไม่มี เช่นลบ/แถวเก่า) — ผู้ป่วยมีแค่ `changed`
    pub details: Option<Value>,
    pub user_public_id: Option<Uuid>,
    pub username: Option<String>,
    pub display_name: Option<String>,
}

/// ทุก action ยกเว้นเข้า/ออกระบบ ในช่วง [from, until) เรียงใหม่→เก่า ไม่เกิน `limit` แถว
///
/// ชื่อรายการ: ตารางที่ soft delete (ผู้ใช้, master, นัดหมาย) อ่านจาก join ด้วย uuid ของแถว (ได้ชื่อปัจจุบัน
/// และใช้ได้กับแถวเก่าก่อนมีหน้านี้) ส่วน role/เพจ ลบจริงจึงใช้ชื่อที่เก็บไว้ใน `metadata.label` ตอนบันทึก
/// `pg_input_is_valid` กัน `target_id` ที่ไม่ใช่ uuid (เช่น username ของ admin คนแรก) ไม่ให้ cast พัง
pub async fn list_activities(
    pool: &PgPool,
    from: DateTime<Utc>,
    until: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<ActivityRow>, RepositoryError> {
    Ok(sqlx::query_as!(
        ActivityRow,
        r#"with ev as (
               select a.id, a.created_at, a.action, a.target_type, a.actor_user_id, a.metadata,
                      case when pg_input_is_valid(a.target_id, 'uuid') then a.target_id::uuid end as target_uuid
               from audit_log a
               where a.action not in ('login_succeeded', 'login_failed', 'logout', 'logout_all')
                 and a.created_at >= $1 and a.created_at < $2
               order by a.created_at desc, a.id desc
               limit $3
           )
           select ev.action, ev.created_at as occurred_at, ev.target_type as "target_type?",
                  case ev.target_type
                      when 'user' then tu.display_name || ' (' || tu.username || ')'
                      when 'role' then ev.metadata ->> 'label'
                      when 'page' then ev.metadata ->> 'label'
                      when 'budget_year' then byr.year
                      when 'disease' then dis.name
                      when 'prename' then pre.name
                      when 'nationality' then nat.national_name
                      when 'patient_group' then pgr.name
                      when 'lab' then lb.name
                      when 'lab_visit_group' then 'ปีงบ ' || gby.year || ' · ' || gpg.name
                      when 'lab_visit' then 'ปีงบ ' || gby.year || ' · ' || gpg.name
                      -- หน้า SQL: สรุปจาก metadata (ไม่มีตัว SQL เก็บไว้) เช่น "12 แถว · patient, lab_visit"
                      when 'sql' then
                          case when ev.metadata ->> 'status' = 'error'
                               then 'ล้มเหลว' || coalesce(' (' || (ev.metadata ->> 'errorCode') || ')', '')
                               -- คำสั่งที่หน้า SQL ไม่รัน เช่น "ไม่อนุญาต (DELETE)"
                               when ev.metadata ->> 'status' = 'blocked'
                               then 'ไม่อนุญาต (' || coalesce(ev.metadata ->> 'keyword',
                                    case ev.metadata ->> 'reason' when 'multiple_statements' then 'หลายคำสั่ง'
                                         else 'แก้ข้อมูล' end) || ')'
                               else (ev.metadata ->> 'rows') || ' แถว'
                                    || coalesce(' · ' || (select string_agg(t, ', ')
                                                         from jsonb_array_elements_text(ev.metadata -> 'tables') t), '')
                          end
                      -- patient และชนิดอื่น ๆ: ไม่มีชื่อรายการ
                      else null
                  end as "target_label?",
                  case when ev.action = 'lab_visit_group_exported' then ev.metadata ->> 'document' end
                      as "export_document?",
                  case when ev.action = 'lab_visit_group_exported' then ev.metadata ->> 'format' end
                      as "export_format?",
                  -- ผู้ป่วย/คนไข้ในนัดหมาย: อ่านได้แค่ชื่อช่องที่เปลี่ยน (กันซ้ำอีกชั้นแม้มีค่าหลุดลงไป)
                  case
                      when ev.target_type in ('patient', 'lab_visit') then
                          case when ev.metadata ? 'changed'
                               then jsonb_build_object('changed', ev.metadata -> 'changed') end
                      else (select jsonb_object_agg(d.key, d.value) from jsonb_each(ev.metadata) d
                            where d.key in ('before', 'after', 'changed'))
                  end as "details?",
                  u.uuid as "user_public_id?", u.username as "username?", u.display_name as "display_name?"
           from ev
           left join users u on u.id = ev.actor_user_id
           left join users tu on ev.target_type = 'user' and tu.uuid = ev.target_uuid
           left join budget_year byr on ev.target_type = 'budget_year' and byr.uuid = ev.target_uuid
           left join disease dis on ev.target_type = 'disease' and dis.uuid = ev.target_uuid
           left join prename pre on ev.target_type = 'prename' and pre.uuid = ev.target_uuid
           left join nationality nat on ev.target_type = 'nationality' and nat.uuid = ev.target_uuid
           left join patient_group pgr on ev.target_type = 'patient_group' and pgr.uuid = ev.target_uuid
           left join lab lb on ev.target_type = 'lab' and lb.uuid = ev.target_uuid
           left join lab_visit lv on ev.target_type = 'lab_visit' and lv.uuid = ev.target_uuid
           left join lab_visit_group lvg on lvg.id = lv.lab_visit_group_id
               or (ev.target_type = 'lab_visit_group' and lvg.uuid = ev.target_uuid)
           left join budget_year gby on gby.id = lvg.budget_year_id
           left join patient_group gpg on gpg.id = lvg.patient_group_id
           order by ev.created_at desc, ev.id desc"#,
        from,
        until,
        limit
    )
    .fetch_all(pool)
    .await?)
}
