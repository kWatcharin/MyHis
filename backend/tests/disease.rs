//! เทสระดับ service ของ disease บน DB สะอาด (`#[sqlx::test]` รัน migration ทั้งหมดให้) —
//! เรียก service ตรง ๆ โดยสร้าง `CurrentUser` เอง (ไม่ผ่าน HTTP/cookie)

use my_his_backend::auth::CurrentUser;
use my_his_backend::disease::service::{self, CreateDiseaseInput, UpdateDiseaseInput};
use my_his_backend::disease::DiseaseError;
use my_his_backend::domain::{Permission, User, UserId};
use sqlx::PgPool;
use uuid::Uuid;

async fn actor_with(pool: &PgPool, permissions: Vec<Permission>) -> CurrentUser {
    let row: (i32, Uuid) = sqlx::query_as(
        "insert into users (username, display_name, password_hash)
         values ($1, 'Tester', 'x') returning id, uuid",
    )
    .bind(format!("tester-{}", Uuid::new_v4()))
    .fetch_one(pool)
    .await
    .unwrap();
    CurrentUser {
        user: User {
            id: UserId(row.0),
            public_id: row.1,
            username: "tester".to_string(),
            display_name: "Tester".to_string(),
            must_change_password: false,
        },
        permissions,
    }
}

async fn admin(pool: &PgPool) -> CurrentUser {
    actor_with(pool, Permission::all().collect()).await
}

fn input<'a>(name: &'a str, abbreviation: &'a str) -> CreateDiseaseInput<'a> {
    CreateDiseaseInput {
        name,
        abbreviation,
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

async fn find(pool: &PgPool, actor: &CurrentUser, name: &str) -> Uuid {
    service::list(pool, actor)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("disease {name:?} not listed"))
        .public_id
}

#[sqlx::test]
async fn create_list_update_delete(pool: PgPool) {
    let actor = admin(&pool).await;

    service::create(&pool, &actor, input("เบาหวาน", "DM"))
        .await
        .unwrap();
    service::create(
        &pool,
        &actor,
        CreateDiseaseInput {
            sort_order: 5,
            ..input("ความดันโลหิตสูง", "HT")
        },
    )
    .await
    .unwrap();
    service::create(&pool, &actor, input("Asthma", "AS"))
        .await
        .unwrap();

    // เรียง sort_order ก่อน แล้วชื่อ — ตัวที่ sort_order 0 มาก่อน (Asthma < เบาหวาน ตามลำดับ collation)
    let listed = service::list(&pool, &actor).await.unwrap();
    assert_eq!(listed.len(), 3);
    assert_eq!(listed.last().unwrap().name, "ความดันโลหิตสูง");
    assert!(listed.iter().all(|r| r.is_active));

    let id = find(&pool, &actor, "เบาหวาน").await;
    service::update(
        &pool,
        &actor,
        id,
        UpdateDiseaseInput {
            abbreviation: Some("  DM2 "),
            is_active: Some(false),
            sort_order: Some(9),
            ..no_change()
        },
    )
    .await
    .unwrap();
    let updated = service::list(&pool, &actor)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.public_id == id)
        .unwrap();
    assert_eq!(updated.abbreviation, "DM2"); // trim แล้ว
    assert_eq!(updated.name, "เบาหวาน");
    assert!(!updated.is_active);
    assert_eq!(updated.sort_order, 9);

    service::delete(&pool, &actor, id).await.unwrap();
    assert_eq!(service::list(&pool, &actor).await.unwrap().len(), 2);

    // soft delete: แถวยังอยู่ใน DB พร้อม deleted_by
    let (deleted_at_set, deleted_by): (bool, Option<i32>) =
        sqlx::query_as("select deleted_at is not null, deleted_by from disease where uuid = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(deleted_at_set);
    assert_eq!(deleted_by, Some(actor.user.id.0));
}

#[sqlx::test]
async fn duplicate_name_conflicts_case_insensitively_until_soft_deleted(pool: PgPool) {
    let actor = admin(&pool).await;
    service::create(&pool, &actor, input("Diabetes", "DM"))
        .await
        .unwrap();

    for duplicate in ["Diabetes", "diabetes", "DIABETES", "  Diabetes  "] {
        let result = service::create(&pool, &actor, input(duplicate, "X")).await;
        assert!(
            matches!(&result, Err(DiseaseError::Conflict(m)) if m == "disease name already taken"),
            "{duplicate:?} should conflict, got {result:?}"
        );
    }

    // คำย่อซ้ำได้
    service::create(&pool, &actor, input("Dengue", "DM"))
        .await
        .unwrap();

    // แก้ชื่อไปชนอีกแถวก็ต้อง conflict
    let dengue = find(&pool, &actor, "Dengue").await;
    let clash = service::update(
        &pool,
        &actor,
        dengue,
        UpdateDiseaseInput {
            name: Some("diabetes"),
            ..no_change()
        },
    )
    .await;
    assert!(matches!(clash, Err(DiseaseError::Conflict(_))));

    // แก้เป็นชื่อตัวเอง (เปลี่ยนแค่ตัวพิมพ์) ได้ ไม่ชนตัวเอง
    service::update(
        &pool,
        &actor,
        dengue,
        UpdateDiseaseInput {
            name: Some("DENGUE"),
            ..no_change()
        },
    )
    .await
    .unwrap();

    // ลบแล้วสร้างชื่อเดิมซ้ำได้
    let diabetes = find(&pool, &actor, "Diabetes").await;
    service::delete(&pool, &actor, diabetes).await.unwrap();
    service::create(&pool, &actor, input("Diabetes", "DM"))
        .await
        .unwrap();
}

#[sqlx::test]
async fn blank_and_overlong_text_is_rejected(pool: PgPool) {
    let actor = admin(&pool).await;

    for (name, abbreviation) in [("", "A"), ("   ", "A"), ("Name", ""), ("Name", "  \t ")] {
        let result = service::create(&pool, &actor, input(name, abbreviation)).await;
        assert!(
            matches!(result, Err(DiseaseError::Validation(_))),
            "{name:?}/{abbreviation:?} should be rejected"
        );
    }

    // ชื่อไทย 100 ตัวอักษร (300 byte) ผ่าน — นับเป็นตัวอักษร ไม่ใช่ byte; 101 ตัวไม่ผ่าน
    let thai_100 = "ก".repeat(100);
    service::create(&pool, &actor, input(&thai_100, "T"))
        .await
        .unwrap();
    let thai_101 = "ข".repeat(101);
    assert!(matches!(
        service::create(&pool, &actor, input(&thai_101, "T")).await,
        Err(DiseaseError::Validation(_))
    ));
    let long_abbreviation = "a".repeat(101);
    assert!(matches!(
        service::create(&pool, &actor, input("Other", &long_abbreviation)).await,
        Err(DiseaseError::Validation(_))
    ));

    // update ก็ตรวจเหมือนกัน
    let id = find(&pool, &actor, &thai_100).await;
    for bad in [
        UpdateDiseaseInput {
            name: Some(" "),
            ..no_change()
        },
        UpdateDiseaseInput {
            abbreviation: Some(""),
            ..no_change()
        },
    ] {
        assert!(matches!(
            service::update(&pool, &actor, id, bad).await,
            Err(DiseaseError::Validation(_))
        ));
    }
}

#[sqlx::test]
async fn disease_used_by_medical_condition_cannot_be_deleted(pool: PgPool) {
    let actor = admin(&pool).await;
    service::create(&pool, &actor, input("Asthma", "AS"))
        .await
        .unwrap();
    let uuid = find(&pool, &actor, "Asthma").await;
    let disease_id: i32 = sqlx::query_scalar("select id from disease where uuid = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .unwrap();

    let prename_id: i32 = sqlx::query_scalar(
        "insert into prename (name, created_by, updated_by) values ('นาย', $1, $1) returning id",
    )
    .bind(actor.user.id.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    let patient_id: i32 = sqlx::query_scalar(
        "insert into patient (hn, hn_idx, nap_no, nap_no_idx, prename_id, first_name, last_name, sex, is_thai, created_by, updated_by)
         values ($2, $2, $2, $2, $1, $2, $2, 1, true, $3, $3) returning id",
    )
    .bind(prename_id)
    .bind(vec![1u8, 2, 3])
    .bind(actor.user.id.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    let condition_id: i32 = sqlx::query_scalar(
        "insert into medical_condition (disease_id, patient_id, created_by, updated_by)
         values ($1, $2, $3, $3) returning id",
    )
    .bind(disease_id)
    .bind(patient_id)
    .bind(actor.user.id.0)
    .fetch_one(&pool)
    .await
    .unwrap();

    let blocked = service::delete(&pool, &actor, uuid).await;
    assert!(
        matches!(&blocked, Err(DiseaseError::Conflict(m)) if m == "disease is still used"),
        "got {blocked:?}"
    );

    // medical_condition ถูกลบ (soft) แล้ว โรคนั้นลบได้
    sqlx::query("update medical_condition set deleted_at = now(), deleted_by = $2 where id = $1")
        .bind(condition_id)
        .bind(actor.user.id.0)
        .execute(&pool)
        .await
        .unwrap();
    service::delete(&pool, &actor, uuid).await.unwrap();
}

#[sqlx::test]
async fn requires_page_permission(pool: PgPool) {
    let admin_actor = admin(&pool).await;
    service::create(&pool, &admin_actor, input("Asthma", "AS"))
        .await
        .unwrap();
    let id = find(&pool, &admin_actor, "Asthma").await;

    let nurse = actor_with(
        &pool,
        vec![
            Permission::parse("shift.view").unwrap(),
            Permission::parse("shift.update").unwrap(),
        ],
    )
    .await;
    assert!(matches!(
        service::list(&pool, &nurse).await,
        Err(DiseaseError::Forbidden)
    ));
    assert!(matches!(
        service::create(&pool, &nurse, input("Flu", "F")).await,
        Err(DiseaseError::Forbidden)
    ));
    assert!(matches!(
        service::update(&pool, &nurse, id, no_change()).await,
        Err(DiseaseError::Forbidden)
    ));
    assert!(matches!(
        service::delete(&pool, &nurse, id).await,
        Err(DiseaseError::Forbidden)
    ));

    // ไม่มีผลข้างเคียง
    assert_eq!(service::list(&pool, &admin_actor).await.unwrap().len(), 1);
}

#[sqlx::test]
async fn unknown_id_is_not_found(pool: PgPool) {
    let actor = admin(&pool).await;
    let missing = Uuid::new_v4();
    assert!(matches!(
        service::update(&pool, &actor, missing, no_change()).await,
        Err(DiseaseError::NotFound)
    ));
    assert!(matches!(
        service::delete(&pool, &actor, missing).await,
        Err(DiseaseError::NotFound)
    ));
}

#[sqlx::test]
async fn menu_page_is_seeded_under_settings_group(pool: PgPool) {
    let (label, group, permission): (String, Option<String>, String) = sqlx::query_as(
        "select p.label, m.label, pp.permission
         from pages p
         join page_permissions pp on pp.page_id = p.id
         left join pages m on m.id = p.ref_id
         where p.route = '/settings/diseases'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(label, "ตั้งค่าโรค");
    assert_eq!(group.as_deref(), Some("การตั้งค่า"));
    assert_eq!(permission, "disease.view");
}
