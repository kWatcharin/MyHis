//! เทสระดับ service ของ patient บน DB สะอาด (`#[sqlx::test]` รัน migration ทั้งหมดให้) — เรียก service
//! ตรง ๆ โดยสร้าง `CurrentUser` เอง ใช้ข้อมูลสมมติเท่านั้น (ห้ามใส่ข้อมูลผู้ป่วยจริงในเทส)

mod common;

use chrono::NaiveDate;
use common::*;
use my_his_backend::auth::CurrentUser;
use my_his_backend::crypto::Crypto;
use my_his_backend::domain::Permission;
use my_his_backend::patient::service::{self, PatientInput};
use my_his_backend::patient::{PatientError, PatientView};
use sqlx::PgPool;
use uuid::Uuid;

/// hn, hn_idx, nap_no, nap_no_idx, first_name, last_name (ดิบจาก DB)
type CipherRow = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);
/// action, target_type, target_id, actor_user_id
type AuditRow = (String, Option<String>, Option<String>, Option<i32>);

const KEY_A: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const KEY_B: &str = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";

fn crypto() -> Crypto {
    Crypto::from_hex_keys(KEY_A, KEY_B).unwrap()
}

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn today() -> NaiveDate {
    date(2026, 10, 4)
}

struct Fixture {
    /// ผู้ใช้ที่มี patient.read + patient.write
    actor: CurrentUser,
    prename: Uuid,
    nationality: Uuid,
    /// [d1, d2] ใช้งานอยู่, [2] = d3 ปิดใช้งาน
    diseases: Vec<Uuid>,
}

async fn insert_uuid(pool: &PgPool, sql: &str, user_id: i32, name: &str) -> Uuid {
    sqlx::query_scalar(sql)
        .bind(name)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn setup(pool: &PgPool) -> Fixture {
    let actor = actor_with(
        pool,
        vec![Permission::PatientRead, Permission::PatientWrite],
    )
    .await;
    let user = actor.user.id.0;
    let prename = insert_uuid(
        pool,
        "insert into prename (name, abbreviation, created_by, updated_by)
         values ($1, 'ด.ช.', $2, $2) returning uuid",
        user,
        "เด็กชาย",
    )
    .await;
    let nationality = insert_uuid(
        pool,
        "insert into nationality (national_name, nation, created_by, updated_by)
         values ($1, 'ไทย', $2, $2) returning uuid",
        user,
        "ไทย",
    )
    .await;
    let mut diseases = Vec::new();
    for (name, abbr, active) in [
        ("เบาหวาน", "DM", true),
        ("ความดัน", "HT", true),
        ("หอบหืด", "AS", false),
    ] {
        let uuid: Uuid = sqlx::query_scalar(
            "insert into disease (name, abbreviation, is_active, created_by, updated_by)
             values ($1, $2, $3, $4, $4) returning uuid",
        )
        .bind(name)
        .bind(abbr)
        .bind(active)
        .bind(user)
        .fetch_one(pool)
        .await
        .unwrap();
        diseases.push(uuid);
    }
    Fixture {
        actor,
        prename,
        nationality,
        diseases,
    }
}

fn input<'a>(f: &Fixture, hn: &'a str) -> PatientInput<'a> {
    PatientInput {
        hn,
        nap_no: "NAP-1",
        prename_id: f.prename,
        first_name: "ทดสอบ",
        last_name: "เอ",
        birth_date: None,
        age: Some(30),
        weight: Some(67.01),
        height: Some(171.33),
        sex: 1,
        is_thai: true,
        nationality_id: Some(f.nationality),
        sort_order: None,
        is_active: true,
        disease_ids: &[],
    }
}

async fn list(pool: &PgPool, f: &Fixture, today: NaiveDate) -> Vec<PatientView> {
    service::list(pool, &crypto(), &f.actor, today)
        .await
        .unwrap()
}

async fn create(pool: &PgPool, f: &Fixture, input: PatientInput<'_>) -> Result<(), PatientError> {
    service::create(pool, &crypto(), &f.actor, input, today()).await
}

async fn find(pool: &PgPool, f: &Fixture, hn: &str) -> PatientView {
    list(pool, f, today())
        .await
        .into_iter()
        .find(|p| p.hn == hn)
        .unwrap_or_else(|| panic!("patient {hn:?} not listed"))
}

#[sqlx::test]
async fn create_then_list_round_trips_decrypted_values(pool: PgPool) {
    let f = setup(&pool).await;
    let diseases = [f.diseases[0], f.diseases[1]];
    create(
        &pool,
        &f,
        PatientInput {
            disease_ids: &diseases,
            ..input(&f, "HN-001")
        },
    )
    .await
    .unwrap();
    create(&pool, &f, input(&f, "HN-002")).await.unwrap();

    let listed = list(&pool, &f, today()).await;
    assert_eq!(listed.len(), 2);
    let p = &listed[0];
    assert_eq!(p.hn, "HN-001");
    assert_eq!(p.nap_no, "NAP-1");
    assert_eq!(
        (p.first_name.as_str(), p.last_name.as_str()),
        ("ทดสอบ", "เอ")
    );
    assert_eq!(p.prename_name, "เด็กชาย");
    assert_eq!(p.prename_public_id, f.prename);
    assert_eq!(p.nationality_name.as_deref(), Some("ไทย"));
    assert_eq!(p.nationality_public_id, Some(f.nationality));
    assert_eq!((p.weight, p.height), (Some(67.01), Some(171.33)));
    assert_eq!(
        (p.sex, p.is_thai, p.age, p.is_active),
        (1, true, Some(30), true)
    );

    // sort_order ไม่ส่ง = ต่อท้ายลำดับสุดท้าย
    assert_eq!(listed[0].sort_order, 1);
    assert_eq!(listed[1].sort_order, 2);

    // โรคประจำตัวตามลำดับที่ส่ง พร้อมชื่อ/คำย่อ
    let names: Vec<(&str, &str)> = p
        .conditions
        .iter()
        .map(|c| (c.name.as_str(), c.abbreviation.as_str()))
        .collect();
    assert_eq!(names, [("เบาหวาน", "DM"), ("ความดัน", "HT")]);
    assert!(listed[1].conditions.is_empty());
}

#[sqlx::test]
async fn database_holds_only_ciphertext_and_blind_index(pool: PgPool) {
    let f = setup(&pool).await;
    create(
        &pool,
        &f,
        PatientInput {
            nap_no: "NAP-SECRET-77",
            first_name: "ลับมาก",
            last_name: "สุดยอด",
            ..input(&f, "HN-SECRET-99")
        },
    )
    .await
    .unwrap();

    let row: CipherRow =
        sqlx::query_as("select hn, hn_idx, nap_no, nap_no_idx, first_name, last_name from patient")
            .fetch_one(&pool)
            .await
            .unwrap();

    let secrets: [&[u8]; 4] = [
        b"HN-SECRET-99",
        b"NAP-SECRET-77",
        "ลับมาก".as_bytes(),
        "สุดยอด".as_bytes(),
    ];
    for column in [&row.0, &row.2, &row.4, &row.5] {
        for secret in secrets {
            assert!(
                !column.windows(secret.len()).any(|w| w == secret),
                "plaintext found in ciphertext column"
            );
        }
    }

    let c = crypto();
    assert_eq!(c.decrypt("patient.hn", &row.0).unwrap(), "HN-SECRET-99");
    assert_eq!(
        c.decrypt("patient.nap_no", &row.2).unwrap(),
        "NAP-SECRET-77"
    );
    assert_eq!(c.decrypt("patient.first_name", &row.4).unwrap(), "ลับมาก");
    assert_eq!(c.decrypt("patient.last_name", &row.5).unwrap(), "สุดยอด");
    // blind index ไว้ค้นหาแบบ = และไม่ใช่ plaintext
    assert_eq!(row.1, c.blind_index("patient.hn", "HN-SECRET-99"));
    assert_eq!(row.3, c.blind_index("patient.nap_no", "NAP-SECRET-77"));
    // ciphertext สลับคอลัมน์ไม่ได้ (AAD ผูกกับคอลัมน์)
    assert!(c.decrypt("patient.first_name", &row.0).is_err());
}

#[sqlx::test]
async fn duplicate_hn_conflicts_but_nap_no_may_repeat(pool: PgPool) {
    let f = setup(&pool).await;
    create(&pool, &f, input(&f, "HN-001")).await.unwrap();

    for dup in ["HN-001", "  HN-001 "] {
        let result = create(&pool, &f, input(&f, dup)).await;
        assert!(
            matches!(&result, Err(PatientError::Conflict(m)) if m == "hn already registered"),
            "{dup:?} -> {result:?}"
        );
    }
    // เลขประจำตัวคนไข้ (nap_no) ซ้ำได้ — "NAP-1" ซ้ำกับคนแรกอยู่แล้ว
    create(&pool, &f, input(&f, "HN-002")).await.unwrap();

    // แก้ HN ไปชนคนอื่น → conflict; แก้เป็น HN ของตัวเอง (ไม่เปลี่ยน) ได้
    let second = find(&pool, &f, "HN-002").await.public_id;
    let clash = service::replace(
        &pool,
        &crypto(),
        &f.actor,
        second,
        input(&f, "HN-001"),
        today(),
    )
    .await;
    assert!(matches!(clash, Err(PatientError::Conflict(_))));
    service::replace(
        &pool,
        &crypto(),
        &f.actor,
        second,
        input(&f, "HN-002"),
        today(),
    )
    .await
    .unwrap();

    // ลบแล้ว HN เดิมลงทะเบียนใหม่ได้
    service::delete(&pool, &f.actor, second).await.unwrap();
    create(&pool, &f, input(&f, "HN-002")).await.unwrap();
}

#[sqlx::test]
async fn invalid_values_are_rejected(pool: PgPool) {
    let f = setup(&pool).await;
    let long = |n: usize| "ก".repeat(n);

    let cases: Vec<(&str, PatientInput<'_>)> = vec![
        ("blank hn", input(&f, "   ")),
        (
            "blank nap_no",
            PatientInput {
                nap_no: " ",
                ..input(&f, "H")
            },
        ),
        (
            "blank first name",
            PatientInput {
                first_name: "",
                ..input(&f, "H")
            },
        ),
        (
            "blank last name",
            PatientInput {
                last_name: "  ",
                ..input(&f, "H")
            },
        ),
        (
            "sex 0",
            PatientInput {
                sex: 0,
                ..input(&f, "H")
            },
        ),
        (
            "sex 3",
            PatientInput {
                sex: 3,
                ..input(&f, "H")
            },
        ),
        (
            "weight 0",
            PatientInput {
                weight: Some(0.0),
                ..input(&f, "H")
            },
        ),
        (
            "weight negative",
            PatientInput {
                weight: Some(-1.0),
                ..input(&f, "H")
            },
        ),
        (
            "weight too big",
            PatientInput {
                weight: Some(500.01),
                ..input(&f, "H")
            },
        ),
        (
            "weight nan",
            PatientInput {
                weight: Some(f64::NAN),
                ..input(&f, "H")
            },
        ),
        (
            "height too big",
            PatientInput {
                height: Some(300.01),
                ..input(&f, "H")
            },
        ),
        (
            "birth in future",
            PatientInput {
                birth_date: Some(date(2026, 10, 5)),
                ..input(&f, "H")
            },
        ),
        (
            "birth too old",
            PatientInput {
                birth_date: Some(date(1850, 1, 1)),
                ..input(&f, "H")
            },
        ),
        (
            "age negative",
            PatientInput {
                age: Some(-1),
                ..input(&f, "H")
            },
        ),
        (
            "age 151",
            PatientInput {
                age: Some(151),
                ..input(&f, "H")
            },
        ),
        (
            "unknown prename",
            PatientInput {
                prename_id: Uuid::new_v4(),
                ..input(&f, "H")
            },
        ),
        (
            "unknown nationality",
            PatientInput {
                nationality_id: Some(Uuid::new_v4()),
                ..input(&f, "H")
            },
        ),
    ];
    for (label, case) in cases {
        let result = create(&pool, &f, case).await;
        assert!(
            matches!(result, Err(PatientError::Validation(_))),
            "{label}: {result:?}"
        );
    }

    // ยาวเกินกำหนด (นับตัวอักษร): hn/nap_no 50, ชื่อ/สกุล 100
    let (hn51, hn50, name101, name100) = (long(51), long(50), long(101), long(100));
    assert!(matches!(
        create(&pool, &f, input(&f, &hn51)).await,
        Err(PatientError::Validation(_))
    ));
    assert!(matches!(
        create(
            &pool,
            &f,
            PatientInput {
                first_name: &name101,
                ..input(&f, "H")
            }
        )
        .await,
        Err(PatientError::Validation(_))
    ));
    assert!(matches!(
        create(
            &pool,
            &f,
            PatientInput {
                last_name: &name101,
                ..input(&f, "H")
            }
        )
        .await,
        Err(PatientError::Validation(_))
    ));
    create(
        &pool,
        &f,
        PatientInput {
            first_name: &name100,
            last_name: &name100,
            ..input(&f, &hn50)
        },
    )
    .await
    .unwrap();

    // โรคที่ไม่มีอยู่ / ปิดใช้งาน (ผูกใหม่ไม่ได้)
    let unknown = [Uuid::new_v4()];
    assert!(matches!(
        create(&pool, &f, PatientInput { disease_ids: &unknown, ..input(&f, "H") }).await,
        Err(PatientError::Validation(m)) if m == "unknown disease"
    ));
    let inactive = [f.diseases[2]];
    assert!(matches!(
        create(&pool, &f, PatientInput { disease_ids: &inactive, ..input(&f, "H") }).await,
        Err(PatientError::Validation(m)) if m == "disease is inactive"
    ));

    // ไม่มีผู้ป่วยหลุดเข้า DB จากเคสที่ถูกปฏิเสธ (เหลือแค่ตัวที่ผ่านตัวเดียว)
    assert_eq!(list(&pool, &f, today()).await.len(), 1);
}

#[sqlx::test]
async fn age_is_computed_from_birth_date_not_from_input(pool: PgPool) {
    let f = setup(&pool).await;
    create(
        &pool,
        &f,
        PatientInput {
            birth_date: Some(date(1994, 9, 19)),
            age: Some(5),
            ..input(&f, "HN-001")
        },
    )
    .await
    .unwrap();

    assert_eq!(list(&pool, &f, today()).await[0].age, Some(32)); // age ที่ส่งมา (5) ถูกเมิน
    let stored: Option<i32> = sqlx::query_scalar("select age from patient")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, Some(32));

    // อ่านต่างวันกัน → คำนวณใหม่จากวันเกิด (ไม่ใช้ค่าที่เก็บไว้ซึ่งจะค้างเก่า)
    assert_eq!(list(&pool, &f, date(2027, 9, 18)).await[0].age, Some(32));
    assert_eq!(list(&pool, &f, date(2027, 9, 19)).await[0].age, Some(33));
}

#[sqlx::test]
async fn replace_overwrites_everything_and_clears_nullable_fields(pool: PgPool) {
    let f = setup(&pool).await;
    create(
        &pool,
        &f,
        PatientInput {
            birth_date: Some(date(1990, 1, 1)),
            sort_order: Some(5),
            ..input(&f, "HN-001")
        },
    )
    .await
    .unwrap();
    let id = find(&pool, &f, "HN-001").await.public_id;

    // PUT ด้วยค่าใหม่ทั้งชุด: ฟิลด์ nullable เป็น None = ล้างค่า, sort_order ไม่ส่ง = คงเดิม
    let cleared = PatientInput {
        nap_no: "NAP-2",
        first_name: "ใหม่",
        last_name: "บี",
        birth_date: None,
        age: None,
        weight: None,
        height: None,
        sex: 2,
        is_thai: false,
        nationality_id: None,
        is_active: false,
        ..input(&f, "HN-009")
    };
    service::replace(&pool, &crypto(), &f.actor, id, cleared, today())
        .await
        .unwrap();

    let p = &list(&pool, &f, today()).await[0];
    assert_eq!(p.public_id, id);
    assert_eq!(
        (
            p.hn.as_str(),
            p.nap_no.as_str(),
            p.first_name.as_str(),
            p.last_name.as_str()
        ),
        ("HN-009", "NAP-2", "ใหม่", "บี")
    );
    assert_eq!(
        (p.birth_date, p.age, p.weight, p.height),
        (None, None, None, None)
    );
    assert_eq!((p.sex, p.is_thai, p.is_active), (2, false, false));
    assert_eq!(
        (p.nationality_public_id, p.nationality_name.clone()),
        (None, None)
    );
    assert_eq!(p.sort_order, 5);

    // ส่ง sort_order = ตั้งค่าใหม่
    service::replace(
        &pool,
        &crypto(),
        &f.actor,
        id,
        PatientInput {
            sort_order: Some(9),
            ..input(&f, "HN-009")
        },
        today(),
    )
    .await
    .unwrap();
    assert_eq!(list(&pool, &f, today()).await[0].sort_order, 9);
}

#[sqlx::test]
async fn conditions_follow_the_submitted_set(pool: PgPool) {
    let f = setup(&pool).await;
    let (d1, d2) = (f.diseases[0], f.diseases[1]);
    create(
        &pool,
        &f,
        PatientInput {
            disease_ids: &[d1, d2],
            ..input(&f, "HN-001")
        },
    )
    .await
    .unwrap();
    let id = find(&pool, &f, "HN-001").await.public_id;

    let replace = |ids: Vec<Uuid>| {
        let (pool, f) = (&pool, &f);
        async move {
            service::replace(
                pool,
                &crypto(),
                &f.actor,
                id,
                PatientInput {
                    disease_ids: &ids,
                    ..input(f, "HN-001")
                },
                today(),
            )
            .await
        }
    };
    let abbreviations = || async {
        find(&pool, &f, "HN-001")
            .await
            .conditions
            .into_iter()
            .map(|c| c.abbreviation)
            .collect::<Vec<_>>()
    };
    let condition_rows = |disease: Uuid| {
        let pool = &pool;
        async move {
            let (total, active): (i64, i64) = sqlx::query_as(
                "select count(*), count(*) filter (where mc.deleted_at is null)
                 from medical_condition mc join disease d on d.id = mc.disease_id where d.uuid = $1",
            )
            .bind(disease)
            .fetch_one(pool)
            .await
            .unwrap();
            (total, active)
        }
    };

    assert_eq!(abbreviations().await, ["DM", "HT"]);

    // สลับลำดับ
    replace(vec![d2, d1]).await.unwrap();
    assert_eq!(abbreviations().await, ["HT", "DM"]);

    // ถอดโรค: soft delete แถว (ยังอยู่ใน DB)
    replace(vec![d1]).await.unwrap();
    assert_eq!(abbreviations().await, ["DM"]);
    assert_eq!(condition_rows(d2).await, (1, 0));

    // ผูกกลับ: แถวใหม่ (partial unique นับเฉพาะแถวที่ยังไม่ถูกลบ)
    replace(vec![d1, d2]).await.unwrap();
    assert_eq!(abbreviations().await, ["DM", "HT"]);
    assert_eq!(condition_rows(d2).await, (2, 1));

    // ซ้ำในคำขอ → ผูกครั้งเดียว
    replace(vec![d1, d1, d2, d1]).await.unwrap();
    assert_eq!(abbreviations().await, ["DM", "HT"]);
    assert_eq!(condition_rows(d1).await, (1, 1));

    // ส่งชุดว่าง → ไม่มีโรคประจำตัว
    replace(vec![]).await.unwrap();
    assert!(abbreviations().await.is_empty());

    // โรคที่ผูกอยู่แล้วแล้วถูกปิดใช้งาน → คงไว้ได้; แต่ผูกโรค inactive ใหม่ไม่ได้
    replace(vec![d2]).await.unwrap();
    sqlx::query("update disease set is_active = false where uuid = $1")
        .bind(d2)
        .execute(&pool)
        .await
        .unwrap();
    replace(vec![d2, d1]).await.unwrap();
    assert_eq!(abbreviations().await, ["HT", "DM"]);
    replace(vec![]).await.unwrap();
    assert!(
        matches!(replace(vec![d2]).await, Err(PatientError::Validation(m)) if m == "disease is inactive")
    );
}

#[sqlx::test]
async fn delete_soft_deletes_patient_and_conditions(pool: PgPool) {
    let f = setup(&pool).await;
    create(
        &pool,
        &f,
        PatientInput {
            disease_ids: &[f.diseases[0]],
            ..input(&f, "HN-001")
        },
    )
    .await
    .unwrap();
    let id = find(&pool, &f, "HN-001").await.public_id;

    service::delete(&pool, &f.actor, id).await.unwrap();
    assert!(list(&pool, &f, today()).await.is_empty());

    let (patient_deleted, condition_deleted, by): (bool, bool, Option<i32>) = sqlx::query_as(
        "select p.deleted_at is not null, mc.deleted_at is not null, p.deleted_by
         from patient p join medical_condition mc on mc.patient_id = p.id where p.uuid = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(patient_deleted && condition_deleted);
    assert_eq!(by, Some(f.actor.user.id.0));

    // ลบซ้ำ / แก้ไขผู้ป่วยที่ลบแล้ว → ไม่พบ
    assert!(matches!(
        service::delete(&pool, &f.actor, id).await,
        Err(PatientError::NotFound)
    ));
    assert!(matches!(
        service::replace(&pool, &crypto(), &f.actor, id, input(&f, "HN-001"), today()).await,
        Err(PatientError::NotFound)
    ));
}

#[sqlx::test]
async fn options_list_only_active_master_rows(pool: PgPool) {
    let f = setup(&pool).await;
    sqlx::query("insert into nationality (national_name, nation, is_active, created_by, updated_by) values ('ปิด', 'X', false, $1, $1)")
        .bind(f.actor.user.id.0)
        .execute(&pool)
        .await
        .unwrap();

    let options = service::options(&pool, &f.actor).await.unwrap();
    assert_eq!(options.prenames.len(), 1);
    assert_eq!(options.prenames[0].name, "เด็กชาย");
    assert_eq!(options.prenames[0].abbreviation.as_deref(), Some("ด.ช."));
    assert_eq!(options.nationalities.len(), 1);
    assert_eq!(options.nationalities[0].national_name, "ไทย");
    let mut diseases: Vec<&str> = options
        .diseases
        .iter()
        .map(|d| d.abbreviation.as_str())
        .collect();
    // ลำดับตาม sort_order แล้วชื่อ (ขึ้นกับ collation ของ DB) — เทสนี้ตรวจแค่ว่ามีเฉพาะโรคที่ใช้งานอยู่
    diseases.sort_unstable();
    assert_eq!(diseases, ["DM", "HT"]); // หอบหืด (inactive) ไม่โผล่
}

#[sqlx::test]
async fn read_and_write_permissions_are_checked_separately(pool: PgPool) {
    let f = setup(&pool).await;
    create(&pool, &f, input(&f, "HN-001")).await.unwrap();
    let id = find(&pool, &f, "HN-001").await.public_id;

    let read_only = actor_with(&pool, vec![Permission::PatientRead]).await;
    assert_eq!(
        service::list(&pool, &crypto(), &read_only, today())
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(service::options(&pool, &read_only).await.is_ok());
    assert!(matches!(
        service::create(&pool, &crypto(), &read_only, input(&f, "HN-X"), today()).await,
        Err(PatientError::Forbidden)
    ));
    assert!(matches!(
        service::replace(
            &pool,
            &crypto(),
            &read_only,
            id,
            input(&f, "HN-001"),
            today()
        )
        .await,
        Err(PatientError::Forbidden)
    ));
    assert!(matches!(
        service::delete(&pool, &read_only, id).await,
        Err(PatientError::Forbidden)
    ));

    // write ไม่ implies read
    let write_only = actor_with(&pool, vec![Permission::PatientWrite]).await;
    assert!(matches!(
        service::list(&pool, &crypto(), &write_only, today()).await,
        Err(PatientError::Forbidden)
    ));
    assert!(matches!(
        service::options(&pool, &write_only).await,
        Err(PatientError::Forbidden)
    ));
    service::create(&pool, &crypto(), &write_only, input(&f, "HN-002"), today())
        .await
        .unwrap();

    // สิทธิ์อื่น ๆ (แม้ user.manage หรือ shift.*) ไม่ให้เข้าถึงข้อมูลผู้ป่วย
    for permissions in [
        vec![Permission::UserManage],
        vec![Permission::ShiftRead, Permission::ShiftWrite],
        vec![],
    ] {
        let other = actor_with(&pool, permissions).await;
        assert!(matches!(
            service::list(&pool, &crypto(), &other, today()).await,
            Err(PatientError::Forbidden)
        ));
        assert!(matches!(
            service::delete(&pool, &other, id).await,
            Err(PatientError::Forbidden)
        ));
    }

    // ไม่มีผลข้างเคียงจากคำขอที่ถูกปฏิเสธ
    assert_eq!(list(&pool, &f, today()).await.len(), 2);
}

#[sqlx::test]
async fn unknown_patient_id_is_not_found(pool: PgPool) {
    let f = setup(&pool).await;
    let missing = Uuid::new_v4();
    assert!(matches!(
        service::replace(
            &pool,
            &crypto(),
            &f.actor,
            missing,
            input(&f, "HN-001"),
            today()
        )
        .await,
        Err(PatientError::NotFound)
    ));
    assert!(matches!(
        service::delete(&pool, &f.actor, missing).await,
        Err(PatientError::NotFound)
    ));
}

#[sqlx::test]
async fn audit_log_records_actions_without_any_patient_data(pool: PgPool) {
    let f = setup(&pool).await;
    create(
        &pool,
        &f,
        PatientInput {
            nap_no: "NAP-AUDIT-5",
            first_name: "ลับออดิท",
            last_name: "สกุลออดิท",
            ..input(&f, "HN-AUDIT-5")
        },
    )
    .await
    .unwrap();
    let id = find(&pool, &f, "HN-AUDIT-5").await.public_id;
    service::replace(
        &pool,
        &crypto(),
        &f.actor,
        id,
        input(&f, "HN-AUDIT-6"),
        today(),
    )
    .await
    .unwrap();
    service::delete(&pool, &f.actor, id).await.unwrap();

    let rows: Vec<AuditRow> = sqlx::query_as(
        "select action, target_type, target_id, actor_user_id from audit_log
         where target_type = 'patient' order by id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let actions: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    assert_eq!(
        actions,
        ["patient_created", "patient_updated", "patient_deleted"]
    );
    for (_, target_type, target_id, actor) in &rows {
        assert_eq!(target_type.as_deref(), Some("patient"));
        assert_eq!(target_id.as_deref(), Some(id.to_string().as_str())); // uuid ไม่ใช่ id int หรือ HN
        assert_eq!(*actor, Some(f.actor.user.id.0));
    }

    // ทั้งแถวของ audit_log (ทุกคอลัมน์รวม metadata) ต้องไม่มีข้อมูลผู้ป่วยหลุดเข้าไป
    let dump: Vec<String> =
        sqlx::query_scalar("select audit_log::text from audit_log where target_type = 'patient'")
            .fetch_all(&pool)
            .await
            .unwrap();
    for text in dump {
        for secret in ["HN-AUDIT", "NAP-AUDIT", "ลับออดิท", "สกุลออดิท"]
        {
            assert!(!text.contains(secret), "{secret} leaked into audit_log");
        }
    }
    let metadata_rows: i64 = sqlx::query_scalar(
        "select count(*) from audit_log where target_type = 'patient' and metadata is not null",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(metadata_rows, 0);
}

#[sqlx::test]
async fn migration_seeds_menu_page_and_grants_permissions_to_system_roles(pool: PgPool) {
    let (label, group, permission) = seeded_page(&pool, "/patients").await;
    // seed เดิมเป็นเมนูเดี่ยว "ลงทะเบียนผู้ป่วย" — 20261010100000_users_page.sql ย้ายเข้ากลุ่ม "ลงทะเบียน"
    // คู่กับหน้า "ผู้ใช้งาน" (ดู tests/schema.rs)
    assert_eq!(label, "ผู้ป่วย");
    assert_eq!(group.as_deref(), Some("ลงทะเบียน"));
    assert_eq!(permission, "patient.read");

    for role in ["admin", "nurse"] {
        let granted: Vec<String> = sqlx::query_scalar(
            "select rp.permission from role_permissions rp join roles r on r.id = rp.role_id
             where r.name = $1 and rp.permission like 'patient.%' order by 1",
        )
        .bind(role)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(granted, ["patient.read", "patient.write"], "{role}");
    }
}
