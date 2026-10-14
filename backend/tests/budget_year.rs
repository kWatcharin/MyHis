//! เทสระดับ service ของ budget_year บน DB สะอาด (`#[sqlx::test]` รัน migration ทั้งหมดให้) —
//! เรียก service ตรง ๆ โดยสร้าง `CurrentUser` เอง (ไม่ผ่าน HTTP/cookie)

use chrono::NaiveDate;
use my_his_backend::auth::CurrentUser;
use my_his_backend::budget_year::service::{self, CreateBudgetYearInput, UpdateBudgetYearInput};
use my_his_backend::budget_year::BudgetYearError;
use my_his_backend::domain::{Permission, User, UserId};
use sqlx::PgPool;
use uuid::Uuid;

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

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

fn input(year: &str) -> CreateBudgetYearInput<'_> {
    CreateBudgetYearInput {
        year,
        start_date: date(2026, 10, 1),
        end_date: date(2027, 9, 30),
        sort_order: 0,
        is_active: true,
    }
}

fn no_change() -> UpdateBudgetYearInput<'static> {
    UpdateBudgetYearInput {
        year: None,
        start_date: None,
        end_date: None,
        sort_order: None,
        is_active: None,
    }
}

#[sqlx::test]
async fn create_list_update_delete(pool: PgPool) {
    let actor = admin(&pool).await;

    service::create(&pool, &actor, input("2570")).await.unwrap();
    service::create(
        &pool,
        &actor,
        CreateBudgetYearInput {
            sort_order: 5,
            ..input("2571")
        },
    )
    .await
    .unwrap();

    // เรียง sort_order ก่อน แล้ว year มาก→น้อย
    let listed = service::list(&pool, &actor).await.unwrap();
    let years: Vec<&str> = listed.iter().map(|r| r.year.as_str()).collect();
    assert_eq!(years, ["2570", "2571"]);
    assert_eq!(listed[0].start_date, date(2026, 10, 1));
    assert!(listed[0].is_active);

    let id = listed[0].public_id;
    service::update(
        &pool,
        &actor,
        id,
        UpdateBudgetYearInput {
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
    assert!(!updated.is_active);
    assert_eq!(updated.sort_order, 9);
    assert_eq!(updated.year, "2570");

    service::delete(&pool, &actor, id).await.unwrap();
    let after = service::list(&pool, &actor).await.unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].year, "2571");

    // soft delete: แถวยังอยู่ใน DB พร้อม deleted_by
    let (deleted_at_set, deleted_by): (bool, Option<i32>) = sqlx::query_as(
        "select deleted_at is not null, deleted_by from budget_year where uuid = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(deleted_at_set);
    assert_eq!(deleted_by, Some(actor.user.id.0));
}

#[sqlx::test]
async fn duplicate_year_conflicts_until_soft_deleted(pool: PgPool) {
    let actor = admin(&pool).await;
    service::create(&pool, &actor, input("2570")).await.unwrap();

    let duplicate = service::create(&pool, &actor, input("2570")).await;
    assert!(matches!(duplicate, Err(BudgetYearError::Conflict(m)) if m == "year already taken"));

    // แก้ปีของอีกแถวไปชนก็ต้อง conflict
    service::create(&pool, &actor, input("2571")).await.unwrap();
    let other = service::list(&pool, &actor)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.year == "2571")
        .unwrap();
    let clash = service::update(
        &pool,
        &actor,
        other.public_id,
        UpdateBudgetYearInput {
            year: Some("2570"),
            ..no_change()
        },
    )
    .await;
    assert!(matches!(clash, Err(BudgetYearError::Conflict(_))));

    // ลบแล้วสร้างปีเดิมซ้ำได้
    let first = service::list(&pool, &actor)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.year == "2570")
        .unwrap();
    service::delete(&pool, &actor, first.public_id)
        .await
        .unwrap();
    service::create(&pool, &actor, input("2570")).await.unwrap();
}

#[sqlx::test]
async fn invalid_year_and_date_range_are_rejected(pool: PgPool) {
    let actor = admin(&pool).await;

    for bad in ["257", "25700", "25a0", "๒๕๗๐"] {
        let result = service::create(&pool, &actor, input(bad)).await;
        assert!(
            matches!(result, Err(BudgetYearError::Validation(_))),
            "year {bad:?} should be rejected"
        );
    }

    let backwards = service::create(
        &pool,
        &actor,
        CreateBudgetYearInput {
            start_date: date(2027, 1, 1),
            end_date: date(2026, 12, 31),
            ..input("2570")
        },
    )
    .await;
    assert!(matches!(backwards, Err(BudgetYearError::Validation(_))));

    // วันเริ่ม = วันสิ้นสุด ใช้ได้
    service::create(
        &pool,
        &actor,
        CreateBudgetYearInput {
            start_date: date(2026, 10, 1),
            end_date: date(2026, 10, 1),
            ..input("2570")
        },
    )
    .await
    .unwrap();

    // ส่งมาฝั่งเดียวต้องเช็คกับค่าเดิมของอีกฝั่ง: เดิม 2026-10-01..2026-10-01 เลื่อนวันสิ้นสุดไปก่อนวันเริ่ม
    let id = service::list(&pool, &actor).await.unwrap()[0].public_id;
    let result = service::update(
        &pool,
        &actor,
        id,
        UpdateBudgetYearInput {
            end_date: Some(date(2026, 9, 1)),
            ..no_change()
        },
    )
    .await;
    assert!(matches!(result, Err(BudgetYearError::Validation(_))));
}

#[sqlx::test]
async fn year_used_by_lab_visit_group_cannot_be_deleted(pool: PgPool) {
    let actor = admin(&pool).await;
    service::create(&pool, &actor, input("2570")).await.unwrap();
    let year = service::list(&pool, &actor).await.unwrap().remove(0);

    let patient_group_id: i32 = sqlx::query_scalar(
        "insert into patient_group (name, created_by, updated_by) values ('g', $1, $1) returning id",
    )
    .bind(actor.user.id.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    let lab_visit_group_id: i32 = sqlx::query_scalar(
        "insert into lab_visit_group (budget_year_id, patient_group_id, cd4_1st_date, cd4_2nd_date, vl_date, created_by, updated_by)
         values ($1, $2, '2026-10-04', '2027-04-04', '2027-04-04', $3, $3) returning id",
    )
    .bind(year.id.0)
    .bind(patient_group_id)
    .bind(actor.user.id.0)
    .fetch_one(&pool)
    .await
    .unwrap();

    let blocked = service::delete(&pool, &actor, year.public_id).await;
    assert!(
        matches!(blocked, Err(BudgetYearError::Conflict(m)) if m == "budget year is still used")
    );

    // นัดหมายถูกลบ (soft) แล้ว ปีนั้นลบได้
    sqlx::query("update lab_visit_group set deleted_at = now(), deleted_by = $2 where id = $1")
        .bind(lab_visit_group_id)
        .bind(actor.user.id.0)
        .execute(&pool)
        .await
        .unwrap();
    service::delete(&pool, &actor, year.public_id)
        .await
        .unwrap();
}

#[sqlx::test]
async fn requires_page_permission(pool: PgPool) {
    let admin_actor = admin(&pool).await;
    service::create(&pool, &admin_actor, input("2570"))
        .await
        .unwrap();
    let id = service::list(&pool, &admin_actor).await.unwrap()[0].public_id;

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
        Err(BudgetYearError::Forbidden)
    ));
    assert!(matches!(
        service::create(&pool, &nurse, input("2571")).await,
        Err(BudgetYearError::Forbidden)
    ));
    assert!(matches!(
        service::update(&pool, &nurse, id, no_change()).await,
        Err(BudgetYearError::Forbidden)
    ));
    assert!(matches!(
        service::delete(&pool, &nurse, id).await,
        Err(BudgetYearError::Forbidden)
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
        Err(BudgetYearError::NotFound)
    ));
    assert!(matches!(
        service::delete(&pool, &actor, missing).await,
        Err(BudgetYearError::NotFound)
    ));
}

#[sqlx::test]
async fn menu_page_is_seeded_under_settings_group(pool: PgPool) {
    let (label, group, permission): (String, Option<String>, String) = sqlx::query_as(
        "select p.label, m.label, pp.permission
         from pages p
         join page_permissions pp on pp.page_id = p.id
         left join pages m on m.id = p.ref_id
         where p.route = '/settings/budget-years'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(label, "ตั้งค่าปีงบประมาณ");
    assert_eq!(group.as_deref(), Some("การตั้งค่า"));
    assert_eq!(permission, "budget_year.view");
}
