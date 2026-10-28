//! สิทธิ์แยกตามเพจ (migration `20261013100000_page_action_permissions.sql` + ตรวจราย action ใน service):
//! การแปลงสิทธิ์เดิมของทุก role และ page_permissions, คีย์ใน DB ต้อง parse ได้ทุกตัว, ดู/เพิ่ม/แก้ไข/ลบ ตรวจแยกกัน,
//! หน้าผู้ใช้งานอ่านรายการ role/คำนำหน้าด้วย `user.view` ได้ และหน้านัดหมายแล็ปต้องมี `patient.view` ด้วยถึงอ่านทะเบียนได้

mod common;

use axum::http::{Method, StatusCode};
use common::{
    actor_with, insert_user_with_password, login, perm, run_migration_files, send, test_app,
};
use my_his_backend::disease::service::{self as disease, CreateDiseaseInput, UpdateDiseaseInput};
use my_his_backend::disease::DiseaseError;
use my_his_backend::domain::{Action, Permission, Resource};
use my_his_backend::prename::service::{self as prename, CreatePrenameInput};
use my_his_backend::prename::PrenameError;
use my_his_backend::users::{service as users, UsersError};
use sqlx::PgPool;

const PAGE_ACTIONS_MIGRATION: i64 = 20261013100000;

/// permission ของ role เรียงใน Rust (ไม่ขึ้นกับ collation ของ DB)
async fn role_permissions(pool: &PgPool, role: &str) -> Vec<String> {
    let mut keys: Vec<String> = sqlx::query_scalar(
        "select rp.permission from role_permissions rp join roles r on r.id = rp.role_id where r.name = $1",
    )
    .bind(role)
    .fetch_all(pool)
    .await
    .unwrap();
    keys.sort();
    keys
}

async fn insert_role(pool: &PgPool, name: &str, permissions: &[&str]) {
    sqlx::query(
        "with r as (insert into roles (name) values ($1) returning id)
         insert into role_permissions (role_id, permission) select r.id, unnest($2::text[]) from r",
    )
    .bind(name)
    .bind(permissions)
    .execute(pool)
    .await
    .unwrap();
}

#[test]
fn permission_table_round_trips_and_has_no_impossible_pairs() {
    let all: Vec<Permission> = Permission::all().collect();
    // 47 ตัวจาก migration page_action_permissions + sql.view (หน้า SQL) + 12 ตัวของ master คลัง (inventory_masters)
    // + 4 ตัวของใบรับเข้า (stock_receipts)
    assert_eq!(all.len(), 64);
    for permission in &all {
        assert_eq!(Permission::parse(permission.as_str()), Some(*permission));
        assert_eq!(
            Permission::of(permission.resource(), permission.action()),
            Some(*permission)
        );
    }
    // ผู้ใช้งานไม่มี "ลบ" และส่งออกมีแค่นัดหมายแล็ป
    assert_eq!(Permission::of(Resource::User, Action::Delete), None);
    assert_eq!(Permission::of(Resource::Patient, Action::Export), None);
    assert!(Permission::of(Resource::LabVisit, Action::Export).is_some());
    // คีย์เก่าไม่รู้จักแล้ว
    for old in [
        "user.manage",
        "patient.read",
        "patient.write",
        "shift.read",
        "shift.write",
        "audit.read",
    ] {
        assert_eq!(Permission::parse(old), None, "{old}");
    }
}

/// role ที่ admin สร้างเองและเพจที่ admin ผูกสิทธิ์เอง ถูกแปลงด้วยกฎเดียวกับ role ระบบ — ความสามารถเดิมคงอยู่ครบ
#[sqlx::test(migrations = false)]
async fn migration_converts_every_role_and_page_permission(pool: PgPool) {
    run_migration_files(&pool, |version| version < PAGE_ACTIONS_MIGRATION).await;
    insert_role(&pool, "ผู้ช่วย", &["patient.read"]).await;
    insert_role(&pool, "เขียนอย่างเดียว", &["patient.write"]).await;
    insert_role(&pool, "ผู้ตรวจ", &["user.manage", "shift.read", "audit.read"]).await;
    sqlx::query(
        "with p as (insert into pages (label, route, sort_order) values ('รายงาน', '/reports', 99) returning id)
         insert into page_permissions (page_id, permission)
         select p.id, unnest(array['user.manage', 'patient.write']) from p",
    )
    .execute(&pool)
    .await
    .unwrap();
    run_migration_files(&pool, |version| version >= PAGE_ACTIONS_MIGRATION).await;

    assert_eq!(
        role_permissions(&pool, "ผู้ช่วย").await,
        ["lab_visit.export", "lab_visit.view", "patient.view"]
    );
    assert_eq!(
        role_permissions(&pool, "เขียนอย่างเดียว").await,
        [
            "lab_visit.create",
            "lab_visit.delete",
            "lab_visit.update",
            "patient.create",
            "patient.delete",
            "patient.update"
        ]
    );
    let auditor = role_permissions(&pool, "ผู้ตรวจ").await;
    // user.view/create/update + role/page/master 6 หน้า × 4 + shift.view + audit.view
    assert_eq!(auditor.len(), 3 + 8 * 4 + 2, "{auditor:?}");
    // ไม่ได้สิทธิ์ผู้ป่วย/นัดหมาย (patient_group.* เป็นหน้า master ที่ได้จาก user.manage ตามปกติ)
    assert!(!auditor
        .iter()
        .any(|key| key.starts_with("patient.") || key.starts_with("lab_visit.")));
    assert!(auditor.iter().any(|key| key == "role.update"));
    assert!(auditor.iter().any(|key| key == "audit.view"));

    // role ระบบ: admin ได้ครบทุกตัว (47 ตัวจาก migration นี้ + sql.view จาก migration sql_console_admin_permission
    // + master คลัง 12 ตัวจาก migration inventory_masters + ใบรับเข้า 4 ตัวจาก stock_receipts), nurse (seed: shift +
    // patient) ได้ shift.view/update + ผู้ป่วย + นัดหมายแล็ป (migration ที่ตามมาแตะเฉพาะ admin)
    assert_eq!(role_permissions(&pool, "admin").await.len(), 64);
    assert!(role_permissions(&pool, "admin")
        .await
        .iter()
        .any(|key| key == "sql.view"));
    assert!(!role_permissions(&pool, "nurse")
        .await
        .iter()
        .any(|key| key == "sql.view"));
    assert_eq!(role_permissions(&pool, "nurse").await.len(), 2 + 4 + 5);

    let mut report_page: Vec<String> = sqlx::query_scalar(
        "select pp.permission from page_permissions pp join pages p on p.id = pp.page_id where p.route = '/reports'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    report_page.sort();
    // เพจที่ admin สร้างเอง: user.manage → role.view (คนที่จัดการสิทธิ์ได้เห็นต่อ), patient.write → patient.update
    assert_eq!(report_page, ["patient.update", "role.view"]);
}

/// backend parse คีย์ทุก request (auth) และทุกครั้งที่คำนวณเมนู — คีย์ที่ไม่รู้จักแม้ตัวเดียวทำให้ทุก request พัง
#[sqlx::test]
async fn every_stored_permission_key_parses(pool: PgPool) {
    let keys: Vec<String> = sqlx::query_scalar(
        "select permission from role_permissions union select permission from page_permissions",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(!keys.is_empty());
    for key in keys {
        assert!(Permission::parse(&key).is_some(), "{key}");
    }
}

fn disease_input(name: &str) -> CreateDiseaseInput<'_> {
    CreateDiseaseInput {
        name,
        abbreviation: name,
        sort_order: 0,
        is_active: true,
    }
}

fn no_change() -> UpdateDiseaseInput<'static> {
    UpdateDiseaseInput {
        name: None,
        abbreviation: None,
        sort_order: None,
        is_active: None,
    }
}

/// ดู / เพิ่ม / แก้ไข / ลบ ตรวจแยกกัน — ไม่มีตัวไหน implies ตัวอื่น (ตัวอย่างจากหน้าตั้งค่าโรค)
#[sqlx::test]
async fn page_actions_are_checked_separately(pool: PgPool) {
    let admin = common::admin(&pool).await;
    disease::create(&pool, &admin, disease_input("DM"))
        .await
        .unwrap();
    let id = disease::list(&pool, &admin).await.unwrap()[0].public_id;

    let viewer = actor_with(&pool, vec![perm("disease.view")]).await;
    assert!(disease::list(&pool, &viewer).await.is_ok());
    assert!(matches!(
        disease::create(&pool, &viewer, disease_input("HT")).await,
        Err(DiseaseError::Forbidden)
    ));
    assert!(matches!(
        disease::update(&pool, &viewer, id, no_change()).await,
        Err(DiseaseError::Forbidden)
    ));
    assert!(matches!(
        disease::delete(&pool, &viewer, id).await,
        Err(DiseaseError::Forbidden)
    ));

    let creator = actor_with(&pool, vec![perm("disease.create")]).await;
    assert!(matches!(
        disease::list(&pool, &creator).await,
        Err(DiseaseError::Forbidden)
    ));
    disease::create(&pool, &creator, disease_input("HT"))
        .await
        .unwrap();
    assert!(matches!(
        disease::delete(&pool, &creator, id).await,
        Err(DiseaseError::Forbidden)
    ));

    let updater = actor_with(&pool, vec![perm("disease.update")]).await;
    disease::update(&pool, &updater, id, no_change())
        .await
        .unwrap();

    let deleter = actor_with(&pool, vec![perm("disease.delete")]).await;
    disease::delete(&pool, &deleter, id).await.unwrap();

    // สิทธิ์ของหน้า master อื่นไม่ให้เข้าหน้าโรค
    let other_master = actor_with(&pool, common::all_of(Resource::Prename)).await;
    assert!(matches!(
        disease::list(&pool, &other_master).await,
        Err(DiseaseError::Forbidden)
    ));
}

/// หน้าผู้ใช้งานต้องใช้รายการ role และคำนำหน้าในฟอร์ม — `user.view` อ่านได้ แต่แก้สองหน้านั้นไม่ได้
#[sqlx::test]
async fn user_view_reads_roles_and_prenames_but_cannot_change_them(pool: PgPool) {
    let user_viewer = actor_with(&pool, vec![perm("user.view")]).await;
    assert!(!users::list_roles(&pool, &user_viewer)
        .await
        .unwrap()
        .is_empty());
    assert!(prename::list(&pool, &user_viewer).await.is_ok());
    assert!(matches!(
        users::create_role(&pool, &user_viewer, "ใหม่", None, &[]).await,
        Err(UsersError::Forbidden)
    ));
    assert!(matches!(
        prename::create(
            &pool,
            &user_viewer,
            CreatePrenameInput {
                name: "ดร.",
                abbreviation: None,
                sort_order: 0
            }
        )
        .await,
        Err(PrenameError::Forbidden)
    ));

    // ไม่มีทั้งสิทธิ์หน้าผู้ใช้งานและหน้านั้นเอง = อ่านไม่ได้
    let nurse = common::nurse(&pool).await;
    assert!(matches!(
        users::list_roles(&pool, &nurse).await,
        Err(UsersError::Forbidden)
    ));
    assert!(matches!(
        prename::list(&pool, &nurse).await,
        Err(PrenameError::Forbidden)
    ));
}

/// สิทธิ์ดูนัดหมายแล็ปอย่างเดียวอ่านนัดหมายได้ แต่อ่านทะเบียนผู้ป่วย (ชื่อ/HN) ไม่ได้ — หน้าเว็บจึงต้องการ patient.view ด้วย
#[sqlx::test]
async fn lab_visit_view_alone_does_not_open_the_patient_registry(pool: PgPool) {
    insert_role(&pool, "ดูนัดหมาย", &["lab_visit.view"]).await;
    let app = test_app(pool.clone()).await;
    insert_user_with_password(&pool, "lab.viewer", "lab-viewer-password-1", "ดูนัดหมาย").await;
    let (cookie, _) = login(&app, "lab.viewer", "lab-viewer-password-1").await;

    let groups = send(
        &app,
        Method::GET,
        "/api/lab-visit-groups",
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(groups.status, StatusCode::OK);
    let patients = send(&app, Method::GET, "/api/patients", Some(&cookie), None).await;
    assert_eq!(patients.status, StatusCode::FORBIDDEN);

    // เมนู: เห็นนัดหมายแล็ป แต่ไม่เห็นหน้าผู้ป่วย
    let menu = send(&app, Method::GET, "/api/menu", Some(&cookie), None).await;
    let routes: Vec<&str> = menu
        .body
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["route"].as_str())
        .collect();
    assert!(routes.contains(&"/lab/appointments"), "{routes:?}");
    assert!(!routes.contains(&"/patients"), "{routes:?}");
}
