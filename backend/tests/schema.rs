//! เทส schema หลัง migration ทั้งหมดรันบน DB สะอาด (`#[sqlx::test]` สร้าง DB ใหม่ให้ทุกเทส):
//! id int + uuid ของตารางเดิม, uuid_v7(), constraint ของตารางคนไข้ และการเก็บ ciphertext จริง
//! ใช้ `sqlx::query` แบบ runtime (ไม่ใช่ `query!`) เพื่อไม่ต้องเพิ่ม metadata ใน `.sqlx/`

use my_his_backend::crypto::Crypto;
use sqlx::PgPool;

const KEY_A: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const KEY_B: &str = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";

async fn insert_user(pool: &PgPool) -> i32 {
    sqlx::query_scalar("insert into users (username, display_name, password_hash) values ('tester', 'Tester', 'x') returning id")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn insert_prename(pool: &PgPool, user_id: i32) -> i32 {
    sqlx::query_scalar(
        "insert into prename (name, created_by, updated_by) values ('นาย', $1, $1) returning id",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_patient(
    pool: &PgPool,
    crypto: &Crypto,
    user_id: i32,
    prename_id: i32,
    hn: &str,
    sex: i32,
) -> Result<i32, sqlx::Error> {
    sqlx::query_scalar(
        "insert into patient (hn, hn_idx, nap_no, nap_no_idx, prename_id, first_name, last_name, sex, is_thai, created_by, updated_by)
         values ($1, $2, $3, $4, $5, $6, $7, $8, true, $9, $9) returning id",
    )
    .bind(crypto.encrypt("patient.hn", hn).unwrap())
    .bind(crypto.blind_index("patient.hn", hn))
    .bind(crypto.encrypt("patient.nap_no", "D4-2026-1").unwrap())
    .bind(crypto.blind_index("patient.nap_no", "D4-2026-1"))
    .bind(prename_id)
    .bind(crypto.encrypt("patient.first_name", "John").unwrap())
    .bind(crypto.encrypt("patient.last_name", "Doe").unwrap())
    .bind(sex)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

#[sqlx::test]
async fn existing_tables_have_int_id_and_unique_uuid(pool: PgPool) {
    for table in ["users", "roles", "pages"] {
        let id_type: String = sqlx::query_scalar(
            "select data_type from information_schema.columns where table_name = $1 and column_name = 'id'",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(id_type, "integer", "{table}.id");

        let uuid_type: String = sqlx::query_scalar(
            "select data_type from information_schema.columns where table_name = $1 and column_name = 'uuid'",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(uuid_type, "uuid", "{table}.uuid");
    }

    // seed role จาก migration เดิมยัง join permission ผ่าน id int ได้
    let admin_permissions: i64 = sqlx::query_scalar(
        "select count(*) from roles r join role_permissions rp on rp.role_id = r.id where r.name = 'admin'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(admin_permissions, 4);

    let pages_with_user_manage: i64 = sqlx::query_scalar(
        "select count(*) from pages p join page_permissions pp on pp.page_id = p.id where pp.permission = 'user.manage'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pages_with_user_manage, 2);
}

#[sqlx::test]
async fn uuid_v7_has_version_7_and_sorts_by_time(pool: PgPool) {
    let first: uuid::Uuid = sqlx::query_scalar("select uuid_v7()")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("select pg_sleep(0.01)")
        .execute(&pool)
        .await
        .unwrap();
    let second: uuid::Uuid = sqlx::query_scalar("select uuid_v7()")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(first.get_version_num(), 7);
    assert_eq!(second.get_version_num(), 7);
    assert!(first < second);
}

#[sqlx::test]
async fn new_rows_get_generated_int_id_and_uuid_v7(pool: PgPool) {
    let user_id = insert_user(&pool).await;
    let prename_id = insert_prename(&pool, user_id).await;
    assert!(prename_id > 0);

    let uuid: uuid::Uuid = sqlx::query_scalar("select uuid from prename where id = $1")
        .bind(prename_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(uuid.get_version_num(), 7);
}

#[sqlx::test]
async fn patient_stores_ciphertext_and_enforces_constraints(pool: PgPool) {
    let crypto = Crypto::from_hex_keys(KEY_A, KEY_B).unwrap();
    let user_id = insert_user(&pool).await;
    let prename_id = insert_prename(&pool, user_id).await;

    let patient_id = insert_patient(&pool, &crypto, user_id, prename_id, "66471", 1)
        .await
        .unwrap();

    // DB เก็บแต่ ciphertext — ไม่มี plaintext ในคอลัมน์ และถอดกลับด้วย key ได้
    let hn_blob: Vec<u8> = sqlx::query_scalar("select hn from patient where id = $1")
        .bind(patient_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!hn_blob.windows(5).any(|w| w == b"66471"));
    assert_eq!(crypto.decrypt("patient.hn", &hn_blob).unwrap(), "66471");

    // ค้นหาด้วย blind index ได้ โดยไม่ต้องถอดรหัส
    let found: Option<i32> =
        sqlx::query_scalar("select id from patient where hn_idx = $1 and deleted_at is null")
            .bind(crypto.blind_index("patient.hn", " 66471 "))
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert_eq!(found, Some(patient_id));

    // hn ซ้ำ (แถวที่ยังไม่ถูกลบ) ต้องชน unique
    let duplicate = insert_patient(&pool, &crypto, user_id, prename_id, "66471", 1).await;
    assert!(duplicate
        .unwrap_err()
        .as_database_error()
        .unwrap()
        .is_unique_violation());

    // sex ต้องเป็น 1 หรือ 2
    let bad_sex = insert_patient(&pool, &crypto, user_id, prename_id, "99999", 3).await;
    assert!(bad_sex
        .unwrap_err()
        .as_database_error()
        .unwrap()
        .is_check_violation());

    // soft delete แล้วใช้ hn เดิมซ้ำได้ (partial unique นับเฉพาะแถวที่ยังไม่ถูกลบ)
    sqlx::query("update patient set deleted_at = now(), deleted_by = $2 where id = $1")
        .bind(patient_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    insert_patient(&pool, &crypto, user_id, prename_id, "66471", 1)
        .await
        .unwrap();
}

#[sqlx::test]
async fn medical_condition_and_lab_visit_group_reject_duplicates(pool: PgPool) {
    let crypto = Crypto::from_hex_keys(KEY_A, KEY_B).unwrap();
    let user_id = insert_user(&pool).await;
    let prename_id = insert_prename(&pool, user_id).await;
    let patient_id = insert_patient(&pool, &crypto, user_id, prename_id, "1", 2)
        .await
        .unwrap();

    let disease_id: i32 = sqlx::query_scalar(
        "insert into disease (name, abbreviation, created_by, updated_by) values ('d', 'd', $1, $1) returning id",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let insert_condition = || async {
        sqlx::query(
            "insert into medical_condition (disease_id, patient_id, created_by, updated_by) values ($1, $2, $3, $3)",
        )
        .bind(disease_id)
        .bind(patient_id)
        .bind(user_id)
        .execute(&pool)
        .await
    };
    insert_condition().await.unwrap();
    assert!(insert_condition()
        .await
        .unwrap_err()
        .as_database_error()
        .unwrap()
        .is_unique_violation());

    let budget_year_id: i32 = sqlx::query_scalar(
        "insert into budget_year (year, start_date, end_date, created_by, updated_by)
         values ('2570', '2026-10-01', '2027-09-30', $1, $1) returning id",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let patient_group_id: i32 = sqlx::query_scalar(
        "insert into patient_group (name, created_by, updated_by) values ('g', $1, $1) returning id",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let insert_group = || async {
        sqlx::query(
            "insert into lab_visit_group (budget_year_id, patient_group_id, cd4_1st_date, cd4_2nd_date, vl_date, created_by, updated_by)
             values ($1, $2, '2026-10-04', '2027-04-04', '2027-04-04', $3, $3)",
        )
        .bind(budget_year_id)
        .bind(patient_group_id)
        .bind(user_id)
        .execute(&pool)
        .await
    };
    insert_group().await.unwrap();
    assert!(insert_group()
        .await
        .unwrap_err()
        .as_database_error()
        .unwrap()
        .is_unique_violation());
}
