//! เทสระดับ service ของนัดหมายแล็ป (lab_visit_group + lab_visit) บน DB สะอาด (`#[sqlx::test]` รัน
//! migration ทั้งหมดให้) — เรียก service ตรง ๆ โดยสร้าง `CurrentUser` เอง ใช้ข้อมูลสมมติเท่านั้น
//! (คอลัมน์ข้อมูลระบุตัวคนไข้ใส่ bytea ปลอม — โมดูลนี้ไม่อ่านคอลัมน์เหล่านั้นเลย)

mod common;

use chrono::NaiveDate;
use common::*;
use my_his_backend::auth::CurrentUser;
use my_his_backend::domain::Resource;
use my_his_backend::lab_visit::service::{self, GroupDetail, GroupInput, VisitInput};
use my_his_backend::lab_visit::LabVisitError;
use sqlx::PgPool;
use uuid::Uuid;

/// action, target_type, target_id, metadata (เป็นข้อความ)
type AuditRow = (String, Option<String>, Option<String>, Option<String>);

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

struct Fixture {
    /// ผู้ใช้ที่มีทุกสิทธิ์ของหน้านัดหมายแล็ป (lab_visit.*) และ patient.view — หน้าเว็บของนัดหมาย join ชื่อ/HN จึงต้องมี
    /// patient.view อยู่แล้ว และ `add_visits` ต้องการสิทธิ์นี้ด้วย (คำตอบขึ้นกับสถานะในทะเบียนผู้ป่วย)
    actor: CurrentUser,
    user_id: i32,
    prename_id: i32,
    /// ปีงบ 2569 (ใช้งานอยู่)
    year_2569: Uuid,
    /// ปีงบ 2570 (ใช้งานอยู่)
    year_2570: Uuid,
    /// ปีงบ 2568 (ปิดใช้งาน)
    year_inactive: Uuid,
    /// กลุ่ม "จามจุรี 1" (ใช้งานอยู่)
    group_a: Uuid,
    /// กลุ่ม "จามจุรี 2" (ใช้งานอยู่)
    group_b: Uuid,
    /// กลุ่มที่ปิดใช้งาน
    group_inactive: Uuid,
}

async fn insert_budget_year(pool: &PgPool, user_id: i32, year: &str, active: bool) -> Uuid {
    sqlx::query_scalar(
        "insert into budget_year (year, start_date, end_date, is_active, created_by, updated_by)
         values ($1, '2025-10-01', '2026-09-30', $2, $3, $3) returning uuid",
    )
    .bind(year)
    .bind(active)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_patient_group(pool: &PgPool, user_id: i32, name: &str, active: bool) -> Uuid {
    sqlx::query_scalar(
        "insert into patient_group (name, is_active, created_by, updated_by)
         values ($1, $2, $3, $3) returning uuid",
    )
    .bind(name)
    .bind(active)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn setup(pool: &PgPool) -> Fixture {
    let mut permissions = common::all_of(Resource::LabVisit);
    permissions.push(common::perm("patient.view"));
    let actor = actor_with(pool, permissions).await;
    let user_id = actor.user.id.0;
    let prename_id = insert_prename(pool, user_id, "นาย").await;
    Fixture {
        year_2569: insert_budget_year(pool, user_id, "2569", true).await,
        year_2570: insert_budget_year(pool, user_id, "2570", true).await,
        year_inactive: insert_budget_year(pool, user_id, "2568", false).await,
        group_a: insert_patient_group(pool, user_id, "จามจุรี 1", true).await,
        group_b: insert_patient_group(pool, user_id, "จามจุรี 2", true).await,
        group_inactive: insert_patient_group(pool, user_id, "กลุ่มเก่า", false).await,
        actor,
        user_id,
        prename_id,
    }
}

/// คนไข้ตัวอย่าง (คอลัมน์ที่เข้ารหัสใส่ bytea ปลอม, blind index ไม่ซ้ำกัน) — คืน (id ภายใน, uuid)
async fn new_patient(pool: &PgPool, f: &Fixture, active: bool) -> (i32, Uuid) {
    let unique = Uuid::new_v4().as_bytes().to_vec();
    sqlx::query_as(
        "insert into patient (hn, hn_idx, nap_no, nap_no_idx, prename_id, first_name, last_name,
                              sex, is_thai, is_active, created_by, updated_by)
         values ($1, $1, $1, $1, $2, $1, $1, 1, true, $3, $4, $4) returning id, uuid",
    )
    .bind(unique)
    .bind(f.prename_id)
    .bind(active)
    .bind(f.user_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn group_input(f: &Fixture) -> GroupInput {
    GroupInput {
        budget_year_id: f.year_2569,
        patient_group_id: f.group_a,
        cd4_first_date: date(2025, 12, 9),
        cd4_second_date: date(2026, 6, 9),
        vl_date: date(2025, 12, 9),
        sort_order: None,
    }
}

fn empty_visit() -> VisitInput<'static> {
    VisitInput {
        take_med_year_range: None,
        take_med_month_range: None,
        vl_result: None,
        drug_recipe: None,
        sort_order: None,
    }
}

async fn create(pool: &PgPool, f: &Fixture, input: GroupInput) -> Result<Uuid, LabVisitError> {
    service::create_group(pool, &f.actor, input).await
}

async fn detail(pool: &PgPool, f: &Fixture, group: Uuid) -> GroupDetail {
    service::get_group(pool, &f.actor, group).await.unwrap()
}

/// uuid ของคนไข้ในนัดหมายตามลำดับที่ API คืน
async fn visit_patients(pool: &PgPool, f: &Fixture, group: Uuid) -> Vec<Uuid> {
    detail(pool, f, group)
        .await
        .visits
        .iter()
        .map(|v| v.patient_public_id)
        .collect()
}

fn is_validation(result: &Result<impl std::fmt::Debug, LabVisitError>, expected: &str) -> bool {
    matches!(result, Err(LabVisitError::Validation(m)) if m == expected)
}

fn is_conflict(result: &Result<impl std::fmt::Debug, LabVisitError>, expected: &str) -> bool {
    matches!(result, Err(LabVisitError::Conflict(m)) if m == expected)
}

#[sqlx::test]
async fn read_and_write_permissions_are_checked_separately(pool: PgPool) {
    let f = setup(&pool).await;
    let group = create(&pool, &f, group_input(&f)).await.unwrap();
    let (_, patient) = new_patient(&pool, &f, true).await;
    service::add_visits(&pool, &f.actor, group, &[patient])
        .await
        .unwrap();
    let visit = detail(&pool, &f, group).await.visits[0].public_id;

    // อ่านอย่างเดียว: อ่านได้ทุกอย่าง เขียนไม่ได้สักอย่าง
    let reader = actor_with(&pool, vec![common::perm("lab_visit.view")]).await;
    assert!(service::list_groups(&pool, &reader).await.is_ok());
    assert!(service::options(&pool, &reader).await.is_ok());
    assert!(service::get_group(&pool, &reader, group).await.is_ok());
    let writes = [
        service::create_group(&pool, &reader, group_input(&f))
            .await
            .map(|_| ()),
        service::update_group(&pool, &reader, group, group_input(&f)).await,
        service::delete_group(&pool, &reader, group).await,
        service::add_visits(&pool, &reader, group, &[patient]).await,
        service::update_visit(&pool, &reader, visit, empty_visit()).await,
        service::delete_visit(&pool, &reader, visit).await,
    ];
    for result in writes {
        assert!(
            matches!(result, Err(LabVisitError::Forbidden)),
            "{result:?}"
        );
    }

    // add_visits ต้องมี patient.view ด้วย: ผู้ที่มี lab_visit.view + lab_visit.update แต่ไม่มี patient.view ถูกปฏิเสธเหมือนกันทุกกรณี
    // (ไม่มีทางแยกว่า uuid สุ่ม / ผู้ป่วย inactive / ผู้ป่วยที่อยู่ในนัดหมายแล้ว / ผู้ป่วยที่เพิ่มได้ ผ่านข้อความตอบ — ไม่งั้นเป็น oracle ของ
    // patient.is_active ที่อ่านได้เฉพาะ patient.view)
    let (_, inactive) = new_patient(&pool, &f, false).await;
    let (_, free) = new_patient(&pool, &f, true).await;
    let no_patient_view = actor_with(
        &pool,
        vec![
            common::perm("lab_visit.view"),
            common::perm("lab_visit.update"),
        ],
    )
    .await;
    for patient_ids in [
        vec![Uuid::new_v4()], // ไม่มีในระบบ
        vec![inactive],       // inactive
        vec![patient],        // อยู่ในนัดหมายนี้แล้ว
        vec![free],           // เพิ่มได้ปกติถ้ามีสิทธิ์
    ] {
        let result = service::add_visits(&pool, &no_patient_view, group, &patient_ids).await;
        assert!(
            matches!(result, Err(LabVisitError::Forbidden)),
            "{patient_ids:?} -> {result:?}"
        );
    }
    // และต้องไม่เพิ่มอะไรลงนัดหมาย
    assert_eq!(detail(&pool, &f, group).await.visits.len(), 1);
    // ผู้ที่มี patient.view ด้วยได้คำตอบตามสถานะจริงเหมือนเดิม
    assert!(matches!(
        service::add_visits(&pool, &f.actor, group, &[inactive]).await,
        Err(LabVisitError::Validation(_))
    ));
    service::add_visits(&pool, &f.actor, group, &[free])
        .await
        .unwrap();

    // แก้ไขอย่างเดียว: แก้ได้ แต่อ่านไม่ได้ (แก้ไขไม่ implies ดู)
    let writer = actor_with(&pool, vec![common::perm("lab_visit.update")]).await;
    assert!(matches!(
        service::list_groups(&pool, &writer).await,
        Err(LabVisitError::Forbidden)
    ));
    assert!(matches!(
        service::options(&pool, &writer).await,
        Err(LabVisitError::Forbidden)
    ));
    assert!(matches!(
        service::get_group(&pool, &writer, group).await,
        Err(LabVisitError::Forbidden)
    ));
    service::update_visit(&pool, &writer, visit, empty_visit())
        .await
        .unwrap();

    // ไม่มีสิทธิ์ของหน้านัดหมายแล็ปเลย (แม้มีสิทธิ์เพจอื่นครบ รวมดูผู้ป่วย)
    for outsider in [
        actor_with(&pool, common::all_except(Resource::LabVisit)).await,
        nurse(&pool).await,
    ] {
        assert!(matches!(
            service::list_groups(&pool, &outsider).await,
            Err(LabVisitError::Forbidden)
        ));
        assert!(matches!(
            service::create_group(&pool, &outsider, group_input(&f)).await,
            Err(LabVisitError::Forbidden)
        ));
    }
}

#[sqlx::test]
async fn create_then_list_and_get_group(pool: PgPool) {
    let f = setup(&pool).await;
    let first = create(&pool, &f, group_input(&f)).await.unwrap();
    let second = create(
        &pool,
        &f,
        GroupInput {
            budget_year_id: f.year_2570,
            ..group_input(&f)
        },
    )
    .await
    .unwrap();

    let listed = service::list_groups(&pool, &f.actor).await.unwrap();
    assert_eq!(listed.len(), 2);
    let g = &listed[0];
    assert_eq!(g.public_id, first);
    assert_eq!(
        (g.budget_year.as_str(), g.patient_group_name.as_str()),
        ("2569", "จามจุรี 1")
    );
    assert_eq!(
        (g.budget_year_public_id, g.patient_group_public_id),
        (f.year_2569, f.group_a)
    );
    assert_eq!(
        (g.cd4_first_date, g.cd4_second_date, g.vl_date),
        (date(2025, 12, 9), date(2026, 6, 9), date(2025, 12, 9))
    );
    assert_eq!(g.patient_count, 0);
    // sort_order ไม่ส่ง = ต่อท้ายลำดับสุดท้าย
    assert_eq!((listed[0].sort_order, listed[1].sort_order), (1, 2));
    assert_eq!(listed[1].public_id, second);

    let d = detail(&pool, &f, first).await;
    assert_eq!(d.group.public_id, first);
    assert!(d.visits.is_empty());

    assert!(matches!(
        service::get_group(&pool, &f.actor, Uuid::new_v4()).await,
        Err(LabVisitError::GroupNotFound)
    ));
}

#[sqlx::test]
async fn invalid_group_values_are_rejected(pool: PgPool) {
    let f = setup(&pool).await;
    let deleted_year = insert_budget_year(&pool, f.user_id, "2560", true).await;
    sqlx::query("update budget_year set deleted_at = now(), deleted_by = $2 where uuid = $1")
        .bind(deleted_year)
        .bind(f.user_id)
        .execute(&pool)
        .await
        .unwrap();

    let base = group_input(&f);
    let cases = [
        (
            "cd4 second date must be after first date",
            GroupInput {
                cd4_second_date: base.cd4_first_date,
                ..base
            },
        ),
        (
            "cd4 second date must be after first date",
            GroupInput {
                cd4_second_date: date(2025, 12, 8),
                ..base
            },
        ),
        (
            "unknown budget year",
            GroupInput {
                budget_year_id: Uuid::new_v4(),
                ..base
            },
        ),
        (
            "unknown budget year",
            GroupInput {
                budget_year_id: deleted_year,
                ..base
            },
        ),
        (
            "budget year is inactive",
            GroupInput {
                budget_year_id: f.year_inactive,
                ..base
            },
        ),
        (
            "unknown patient group",
            GroupInput {
                patient_group_id: Uuid::new_v4(),
                ..base
            },
        ),
        (
            "patient group is inactive",
            GroupInput {
                patient_group_id: f.group_inactive,
                ..base
            },
        ),
    ];
    for (expected, input) in cases {
        let result = create(&pool, &f, input).await;
        assert!(is_validation(&result, expected), "{expected}: {result:?}");
    }
    assert!(service::list_groups(&pool, &f.actor)
        .await
        .unwrap()
        .is_empty());
}

#[sqlx::test]
async fn same_year_and_group_conflicts_until_deleted(pool: PgPool) {
    let f = setup(&pool).await;
    let first = create(&pool, &f, group_input(&f)).await.unwrap();

    let duplicate = create(&pool, &f, group_input(&f)).await;
    assert!(
        is_conflict(&duplicate, "lab visit group already exists"),
        "{duplicate:?}"
    );

    // ปีเดียวกันแต่คนละกลุ่ม / กลุ่มเดียวกันแต่คนละปี ได้
    create(
        &pool,
        &f,
        GroupInput {
            patient_group_id: f.group_b,
            ..group_input(&f)
        },
    )
    .await
    .unwrap();
    let next_year = create(
        &pool,
        &f,
        GroupInput {
            budget_year_id: f.year_2570,
            ..group_input(&f)
        },
    )
    .await
    .unwrap();

    // แก้ให้ไปชนนัดหมายอื่น → conflict
    let clash = service::update_group(&pool, &f.actor, next_year, group_input(&f)).await;
    assert!(
        is_conflict(&clash, "lab visit group already exists"),
        "{clash:?}"
    );

    // ลบแล้วสร้างปี+กลุ่มเดิมใหม่ได้
    service::delete_group(&pool, &f.actor, first).await.unwrap();
    create(&pool, &f, group_input(&f)).await.unwrap();
}

#[sqlx::test]
async fn update_replaces_values_and_may_keep_master_deactivated_later(pool: PgPool) {
    let f = setup(&pool).await;
    let group = create(&pool, &f, group_input(&f)).await.unwrap();
    let sort_before = detail(&pool, &f, group).await.group.sort_order;

    // ปีงบ/กลุ่มที่ใช้อยู่ถูกปิดใช้งานภายหลัง — แก้วันนัดโดยคงค่าเดิมได้, sort_order ไม่ส่ง = คงเดิม
    sqlx::query("update budget_year set is_active = false where uuid = $1")
        .bind(f.year_2569)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("update patient_group set is_active = false where uuid = $1")
        .bind(f.group_a)
        .execute(&pool)
        .await
        .unwrap();
    service::update_group(
        &pool,
        &f.actor,
        group,
        GroupInput {
            cd4_first_date: date(2026, 1, 15),
            cd4_second_date: date(2026, 7, 15),
            vl_date: date(2026, 7, 15),
            ..group_input(&f)
        },
    )
    .await
    .unwrap();
    let g = detail(&pool, &f, group).await.group;
    assert_eq!(
        (g.cd4_first_date, g.cd4_second_date, g.vl_date),
        (date(2026, 1, 15), date(2026, 7, 15), date(2026, 7, 15))
    );
    assert_eq!(g.sort_order, sort_before);

    // แต่ย้ายไปปีงบที่ปิดใช้งานตัวอื่นไม่ได้
    let moved = service::update_group(
        &pool,
        &f.actor,
        group,
        GroupInput {
            budget_year_id: f.year_inactive,
            ..group_input(&f)
        },
    )
    .await;
    assert!(
        is_validation(&moved, "budget year is inactive"),
        "{moved:?}"
    );

    // ส่ง sort_order = ตั้งค่าใหม่
    service::update_group(
        &pool,
        &f.actor,
        group,
        GroupInput {
            sort_order: Some(42),
            ..group_input(&f)
        },
    )
    .await
    .unwrap();
    assert_eq!(detail(&pool, &f, group).await.group.sort_order, 42);

    assert!(matches!(
        service::update_group(&pool, &f.actor, Uuid::new_v4(), group_input(&f)).await,
        Err(LabVisitError::GroupNotFound)
    ));
}

#[sqlx::test]
async fn options_list_masters_that_are_not_deleted(pool: PgPool) {
    let f = setup(&pool).await;
    let gone = insert_patient_group(&pool, f.user_id, "ลบแล้ว", true).await;
    sqlx::query("update patient_group set deleted_at = now(), deleted_by = $2 where uuid = $1")
        .bind(gone)
        .bind(f.user_id)
        .execute(&pool)
        .await
        .unwrap();

    let options = service::options(&pool, &f.actor).await.unwrap();
    let mut years: Vec<(String, bool)> = options
        .budget_years
        .iter()
        .map(|y| (y.year.clone(), y.is_active))
        .collect();
    years.sort();
    assert_eq!(
        years,
        [
            ("2568".to_string(), false),
            ("2569".to_string(), true),
            ("2570".to_string(), true)
        ]
    );
    let mut groups: Vec<(Uuid, bool)> = options
        .patient_groups
        .iter()
        .map(|g| (g.public_id, g.is_active))
        .collect();
    groups.sort();
    let mut expected = vec![
        (f.group_a, true),
        (f.group_b, true),
        (f.group_inactive, false),
    ];
    expected.sort();
    assert_eq!(groups, expected);
}

#[sqlx::test]
async fn add_visits_appends_in_request_order_and_dedupes(pool: PgPool) {
    let f = setup(&pool).await;
    let group = create(&pool, &f, group_input(&f)).await.unwrap();
    let (_, p1) = new_patient(&pool, &f, true).await;
    let (_, p2) = new_patient(&pool, &f, true).await;
    let (_, p3) = new_patient(&pool, &f, true).await;

    service::add_visits(&pool, &f.actor, group, &[p2, p1, p2])
        .await
        .unwrap();
    service::add_visits(&pool, &f.actor, group, &[p3])
        .await
        .unwrap();

    let d = detail(&pool, &f, group).await;
    let order: Vec<(Uuid, i32)> = d
        .visits
        .iter()
        .map(|v| (v.patient_public_id, v.sort_order))
        .collect();
    assert_eq!(order, [(p2, 1), (p1, 2), (p3, 3)]);
    assert!(d.visits.iter().all(|v| v.vl_result.is_none()
        && v.drug_recipe.is_none()
        && v.take_med_year_range.is_none()
        && v.take_med_month_range.is_none()));
    assert_eq!(d.group.patient_count, 3);
    assert_eq!(
        service::list_groups(&pool, &f.actor).await.unwrap()[0].patient_count,
        3
    );
}

#[sqlx::test]
async fn add_visits_rejects_bad_patients_without_partial_insert(pool: PgPool) {
    let f = setup(&pool).await;
    let group = create(&pool, &f, group_input(&f)).await.unwrap();
    let (_, p1) = new_patient(&pool, &f, true).await;
    let (_, inactive) = new_patient(&pool, &f, false).await;
    let (deleted_id, deleted) = new_patient(&pool, &f, true).await;
    soft_delete_row(&pool, "patient", deleted_id, f.user_id).await;

    let too_many: Vec<Uuid> = (0..=service::MAX_PATIENTS_PER_REQUEST)
        .map(|_| Uuid::new_v4())
        .collect();
    let cases: [(&str, Vec<Uuid>); 5] = [
        ("patient ids are required", vec![]),
        ("too many patients", too_many),
        ("unknown patient", vec![p1, Uuid::new_v4()]),
        ("unknown patient", vec![p1, deleted]),
        ("patient is inactive", vec![p1, inactive]),
    ];
    for (expected, ids) in cases {
        let result = service::add_visits(&pool, &f.actor, group, &ids).await;
        assert!(is_validation(&result, expected), "{expected}: {result:?}");
    }
    // ไม่มีใครถูกเพิ่ม (ทั้งชุดถูกปฏิเสธ)
    assert!(visit_patients(&pool, &f, group).await.is_empty());

    service::add_visits(&pool, &f.actor, group, &[p1])
        .await
        .unwrap();
    let (_, p2) = new_patient(&pool, &f, true).await;
    let again = service::add_visits(&pool, &f.actor, group, &[p2, p1]).await;
    assert!(is_conflict(&again, "patient already in group"), "{again:?}");
    assert_eq!(visit_patients(&pool, &f, group).await, [p1]);

    assert!(matches!(
        service::add_visits(&pool, &f.actor, Uuid::new_v4(), &[p2]).await,
        Err(LabVisitError::GroupNotFound)
    ));
}

#[sqlx::test]
async fn removed_patient_can_be_added_back_and_may_join_other_groups(pool: PgPool) {
    let f = setup(&pool).await;
    let group = create(&pool, &f, group_input(&f)).await.unwrap();
    let other = create(
        &pool,
        &f,
        GroupInput {
            budget_year_id: f.year_2570,
            ..group_input(&f)
        },
    )
    .await
    .unwrap();
    let (_, p1) = new_patient(&pool, &f, true).await;

    service::add_visits(&pool, &f.actor, group, &[p1])
        .await
        .unwrap();
    // คนเดียวกันอยู่ได้หลายนัดหมาย (คนละปี)
    service::add_visits(&pool, &f.actor, other, &[p1])
        .await
        .unwrap();

    let visit = detail(&pool, &f, group).await.visits[0].public_id;
    service::delete_visit(&pool, &f.actor, visit).await.unwrap();
    assert!(visit_patients(&pool, &f, group).await.is_empty());
    assert!(matches!(
        service::delete_visit(&pool, &f.actor, visit).await,
        Err(LabVisitError::VisitNotFound)
    ));

    service::add_visits(&pool, &f.actor, group, &[p1])
        .await
        .unwrap();
    assert_eq!(visit_patients(&pool, &f, group).await, [p1]);
}

#[sqlx::test]
async fn patients_deleted_from_registry_are_hidden_and_not_counted(pool: PgPool) {
    let f = setup(&pool).await;
    let group = create(&pool, &f, group_input(&f)).await.unwrap();
    let (_, p1) = new_patient(&pool, &f, true).await;
    let (p2_id, p2) = new_patient(&pool, &f, true).await;
    service::add_visits(&pool, &f.actor, group, &[p1, p2])
        .await
        .unwrap();
    let p2_visit = detail(&pool, &f, group).await.visits[1].public_id;

    soft_delete_row(&pool, "patient", p2_id, f.user_id).await;

    let d = detail(&pool, &f, group).await;
    assert_eq!(
        d.visits
            .iter()
            .map(|v| v.patient_public_id)
            .collect::<Vec<_>>(),
        [p1]
    );
    assert_eq!(d.group.patient_count, 1);
    assert_eq!(
        service::list_groups(&pool, &f.actor).await.unwrap()[0].patient_count,
        1
    );
    assert!(matches!(
        service::update_visit(&pool, &f.actor, p2_visit, empty_visit()).await,
        Err(LabVisitError::VisitNotFound)
    ));
    assert!(!visit_patients(&pool, &f, group).await.contains(&p2));
}

#[sqlx::test]
async fn update_visit_validates_trims_and_replaces(pool: PgPool) {
    let f = setup(&pool).await;
    let group = create(&pool, &f, group_input(&f)).await.unwrap();
    let (_, p1) = new_patient(&pool, &f, true).await;
    service::add_visits(&pool, &f.actor, group, &[p1])
        .await
        .unwrap();
    let visit = detail(&pool, &f, group).await.visits[0].public_id;

    service::update_visit(
        &pool,
        &f.actor,
        visit,
        VisitInput {
            take_med_year_range: Some(21),
            take_med_month_range: Some(6),
            vl_result: Some(" <20 "),
            drug_recipe: Some("  TDF+3TC+DTG "),
            sort_order: Some(7),
        },
    )
    .await
    .unwrap();
    let v = &detail(&pool, &f, group).await.visits[0];
    assert_eq!(
        (v.take_med_year_range, v.take_med_month_range),
        (Some(21), Some(6))
    );
    assert_eq!(
        (v.vl_result.as_deref(), v.drug_recipe.as_deref()),
        (Some("<20"), Some("TDF+3TC+DTG"))
    );
    assert_eq!(v.sort_order, 7);

    // แทนที่ทั้งชุด: None/ช่องว่าง = ล้างค่า, sort_order None = คงเดิม; ขอบเขต 0 กับค่าสูงสุดผ่าน
    service::update_visit(
        &pool,
        &f.actor,
        visit,
        VisitInput {
            take_med_year_range: Some(0),
            take_med_month_range: Some(11),
            vl_result: Some("   "),
            ..empty_visit()
        },
    )
    .await
    .unwrap();
    let v = &detail(&pool, &f, group).await.visits[0];
    assert_eq!(
        (v.take_med_year_range, v.take_med_month_range),
        (Some(0), Some(11))
    );
    assert_eq!(
        (v.vl_result.as_deref(), v.drug_recipe.as_deref()),
        (None, None)
    );
    assert_eq!(v.sort_order, 7);

    let long_vl = "ก".repeat(101);
    let long_recipe = "ก".repeat(201);
    let cases = [
        (
            "take med years is out of range",
            VisitInput {
                take_med_year_range: Some(101),
                ..empty_visit()
            },
        ),
        (
            "take med years is out of range",
            VisitInput {
                take_med_year_range: Some(-1),
                ..empty_visit()
            },
        ),
        (
            "take med months is out of range",
            VisitInput {
                take_med_month_range: Some(12),
                ..empty_visit()
            },
        ),
        (
            "vl result must be at most 100 characters",
            VisitInput {
                vl_result: Some(&long_vl),
                ..empty_visit()
            },
        ),
        (
            "drug recipe must be at most 200 characters",
            VisitInput {
                drug_recipe: Some(&long_recipe),
                ..empty_visit()
            },
        ),
    ];
    for (expected, input) in cases {
        let result = service::update_visit(&pool, &f.actor, visit, input).await;
        assert!(is_validation(&result, expected), "{expected}: {result:?}");
    }
    // 100 ตัวอักษรไทย (300 byte) ยังผ่าน
    service::update_visit(
        &pool,
        &f.actor,
        visit,
        VisitInput {
            vl_result: Some(&long_vl[..long_vl.len() - "ก".len()]),
            ..empty_visit()
        },
    )
    .await
    .unwrap();

    assert!(matches!(
        service::update_visit(&pool, &f.actor, Uuid::new_v4(), empty_visit()).await,
        Err(LabVisitError::VisitNotFound)
    ));
}

#[sqlx::test]
async fn deleting_group_soft_deletes_its_visits(pool: PgPool) {
    let f = setup(&pool).await;
    let group = create(&pool, &f, group_input(&f)).await.unwrap();
    let (_, p1) = new_patient(&pool, &f, true).await;
    let (_, p2) = new_patient(&pool, &f, true).await;
    service::add_visits(&pool, &f.actor, group, &[p1, p2])
        .await
        .unwrap();
    let visit = detail(&pool, &f, group).await.visits[0].public_id;

    service::delete_group(&pool, &f.actor, group).await.unwrap();

    assert!(service::list_groups(&pool, &f.actor)
        .await
        .unwrap()
        .is_empty());
    assert!(matches!(
        service::get_group(&pool, &f.actor, group).await,
        Err(LabVisitError::GroupNotFound)
    ));
    assert!(matches!(
        service::delete_group(&pool, &f.actor, group).await,
        Err(LabVisitError::GroupNotFound)
    ));
    assert!(matches!(
        service::update_visit(&pool, &f.actor, visit, empty_visit()).await,
        Err(LabVisitError::VisitNotFound)
    ));

    // แถวยังอยู่ (soft delete) แต่ถูกทำเครื่องหมายลบครบทุกแถว
    let (total, alive): (i64, i64) = sqlx::query_as(
        "select count(*), count(*) filter (where v.deleted_at is null)
         from lab_visit v join lab_visit_group g on g.id = v.lab_visit_group_id
         where g.uuid = $1",
    )
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((total, alive), (2, 0));
}

#[sqlx::test]
async fn every_write_is_audited_without_patient_values(pool: PgPool) {
    let f = setup(&pool).await;
    let group = create(&pool, &f, group_input(&f)).await.unwrap();
    service::update_group(&pool, &f.actor, group, group_input(&f))
        .await
        .unwrap();
    let (_, p1) = new_patient(&pool, &f, true).await;
    let (_, p2) = new_patient(&pool, &f, true).await;
    service::add_visits(&pool, &f.actor, group, &[p1, p2])
        .await
        .unwrap();
    let visits = detail(&pool, &f, group).await.visits;
    service::update_visit(
        &pool,
        &f.actor,
        visits[0].public_id,
        VisitInput {
            vl_result: Some("<20"),
            ..empty_visit()
        },
    )
    .await
    .unwrap();
    service::delete_visit(&pool, &f.actor, visits[1].public_id)
        .await
        .unwrap();
    service::delete_group(&pool, &f.actor, group).await.unwrap();

    let rows: Vec<AuditRow> = sqlx::query_as(
        "select action, target_type, target_id, metadata::text
         from audit_log where actor_user_id = $1 order by id",
    )
    .bind(f.user_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    let group_id = group.to_string();
    let first_visit = visits[0].public_id.to_string();
    let second_visit = visits[1].public_id.to_string();
    let expected = [
        (
            "lab_visit_group_created",
            "lab_visit_group",
            group_id.as_str(),
        ),
        (
            "lab_visit_group_updated",
            "lab_visit_group",
            group_id.as_str(),
        ),
        ("lab_visit_created", "lab_visit", first_visit.as_str()),
        ("lab_visit_created", "lab_visit", second_visit.as_str()),
        ("lab_visit_updated", "lab_visit", first_visit.as_str()),
        ("lab_visit_deleted", "lab_visit", second_visit.as_str()),
        (
            "lab_visit_group_deleted",
            "lab_visit_group",
            group_id.as_str(),
        ),
    ];
    assert_eq!(rows.len(), expected.len(), "{rows:?}");
    for (row, (action, target_type, target_id)) in rows.iter().zip(expected) {
        assert_eq!(row.0, action);
        assert_eq!(row.1.as_deref(), Some(target_type));
        assert_eq!(row.2.as_deref(), Some(target_id));
        // รายละเอียดการเปลี่ยนแปลง: หัวนัดหมายมีค่า (ปีงบ/กลุ่ม/วันนัด — ไม่ใช่ข้อมูลคนไข้), คนไข้ในนัดหมายมีแค่ชื่อช่อง
        // ที่เปลี่ยน — ผล VL (ข้อมูลสุขภาพ) ต้องไม่หลุดเข้า audit ไม่ว่าแถวไหน
        let expected_metadata = match action {
            "lab_visit_updated" => Some(r#"{"changed": ["vl_result"]}"#),
            "lab_visit_created" | "lab_visit_deleted" | "lab_visit_group_deleted" => None,
            _ => row.3.as_deref(),
        };
        assert_eq!(row.3.as_deref(), expected_metadata, "{action}");
        assert!(!row.3.as_deref().unwrap_or("").contains("<20"), "{action}");
    }
    let created = rows[0].3.as_deref().unwrap();
    assert!(created.contains(r#""budget_year": "2569""#), "{created}");
}
