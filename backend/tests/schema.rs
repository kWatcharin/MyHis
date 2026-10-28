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
    // ทุก permission ราย action (20261013100000_page_action_permissions.sql) + sql.view (20261019100000)
    // + master คลัง 12 ตัว (20261026100000_inventory_masters.sql)
    assert_eq!(admin_permissions, 60);

    // หน้าตั้งค่าเพจ, role, ปีงบประมาณ, โรค, คำนำหน้า, สัญชาติ, กลุ่มคนไข้, แลป, master คลัง 3 หน้า และผู้ใช้งาน —
    // แต่ละหน้าต้องมีสิทธิ์ "ดู" ของหน้านั้นเอง (เดิมทุกหน้าใช้ user.manage ตัวเดียว)
    let settings_pages: Vec<(String, String)> = sqlx::query_as(
        "select p.route, pp.permission from pages p join page_permissions pp on pp.page_id = p.id
         where p.route like '/settings/%' or p.route = '/users' order by p.route",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        settings_pages,
        [
            ("/settings/budget-years", "budget_year.view"),
            ("/settings/diseases", "disease.view"),
            // เรียงตาม collation ของ DB (ไม่นับขีด): item-categories < items < item-units
            ("/settings/item-categories", "item_category.view"),
            ("/settings/items", "item.view"),
            ("/settings/item-units", "item_unit.view"),
            ("/settings/labs", "lab.view"),
            ("/settings/nationalities", "nationality.view"),
            ("/settings/pages", "page.view"),
            ("/settings/patient-groups", "patient_group.view"),
            ("/settings/prenames", "prename.view"),
            ("/settings/roles", "role.view"),
            ("/users", "user.view"),
        ]
        .map(|(route, permission)| (route.to_string(), permission.to_string()))
    );
}

/// เมนูแบ่ง 2 กลุ่ม: "สิทธิ์" (เพจ, role) กับ "การตั้งค่า" (หน้า master) — route เดิมไม่เปลี่ยน
#[sqlx::test]
async fn menu_groups_permission_pages_separately_from_settings(pool: PgPool) {
    let groups: Vec<(String, Option<String>)> = sqlx::query_as(
        "select p.route, m.label from pages p left join pages m on m.id = p.ref_id
         where p.route like '/settings/%' order by p.route",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    for (route, group) in &groups {
        let expected = if route == "/settings/pages" || route == "/settings/roles" {
            "สิทธิ์"
        } else {
            "การตั้งค่า"
        };
        assert_eq!(group.as_deref(), Some(expected), "{route}");
    }
    // master 6 หน้า + master คลัง 3 หน้า + สิทธิ์ 2 หน้า
    assert_eq!(groups.len(), 11);
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

    // คนไข้คนเดียวอยู่ในนัดหมายเดียวกันซ้ำไม่ได้ (lab_visit_group_patient_uq) — ลบแถวเดิมแล้วเพิ่มกลับได้
    let group_id: i32 = sqlx::query_scalar("select id from lab_visit_group")
        .fetch_one(&pool)
        .await
        .unwrap();
    let insert_visit = || async {
        sqlx::query_scalar::<_, i32>(
            "insert into lab_visit (patient_id, lab_visit_group_id, created_by, updated_by)
             values ($1, $2, $3, $3) returning id",
        )
        .bind(patient_id)
        .bind(group_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await
    };
    let visit_id = insert_visit().await.unwrap();
    assert!(insert_visit()
        .await
        .unwrap_err()
        .as_database_error()
        .unwrap()
        .is_unique_violation());
    sqlx::query("update lab_visit set deleted_at = now(), deleted_by = $2 where id = $1")
        .bind(visit_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    insert_visit().await.unwrap();
}

/// หน้า "นัดหมายแล็ป" อยู่กลุ่มเมนู "Lab" ระหว่างกลุ่ม "ลงทะเบียน" (หน้าผู้ป่วย /patients) กับกลุ่ม "สิทธิ์"
/// และใช้สิทธิ์ผู้ป่วยเดิม
#[sqlx::test]
async fn lab_appointment_page_is_in_lab_group_with_patient_read(pool: PgPool) {
    let (label, group, sort_order, permission): (String, Option<String>, i32, String) =
        sqlx::query_as(
            "select p.label, m.label, p.sort_order, pp.permission
             from pages p
             join page_permissions pp on pp.page_id = p.id
             left join pages m on m.id = p.ref_id
             where p.route = '/lab/appointments'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(label, "นัดหมายแล็ป");
    assert_eq!(group.as_deref(), Some("Lab"));
    assert_eq!(permission, "lab_visit.view");

    let neighbours: Vec<i32> = sqlx::query_scalar(
        "select sort_order from pages where route in ('/patients', '/settings/pages') order by route",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    // /patients (5) < Lab < /settings/pages (10)
    assert!(
        neighbours[0] < sort_order && sort_order < neighbours[1],
        "{neighbours:?}"
    );
}

/// กลุ่มเมนู "ลงทะเบียน" = หน้า "ผู้ป่วย" (/patients, patient.view) + "ผู้ใช้งาน" (/users, user.view)
/// อยู่ก่อนกลุ่ม Lab — nurse ที่ไม่มี user.view เห็นกลุ่มนี้แค่หน้าผู้ป่วย
#[sqlx::test]
async fn registration_group_holds_patients_and_users_pages(pool: PgPool) {
    let pages: Vec<(String, String, Option<String>, String)> = sqlx::query_as(
        "select p.route, p.label, m.label, pp.permission
         from pages p
         join page_permissions pp on pp.page_id = p.id
         left join pages m on m.id = p.ref_id
         where p.route in ('/patients', '/users', '/lab/appointments')
         order by p.sort_order",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let summary: Vec<(&str, &str, Option<&str>, &str)> = pages
        .iter()
        .map(|(route, label, group, permission)| {
            (
                route.as_str(),
                label.as_str(),
                group.as_deref(),
                permission.as_str(),
            )
        })
        .collect();
    // เรียงตาม sort_order: /patients < /users < /lab/appointments
    assert_eq!(
        summary,
        [
            ("/patients", "ผู้ป่วย", Some("ลงทะเบียน"), "patient.view"),
            ("/users", "ผู้ใช้งาน", Some("ลงทะเบียน"), "user.view"),
            (
                "/lab/appointments",
                "นัดหมายแล็ป",
                Some("Lab"),
                "lab_visit.view"
            ),
        ]
    );
}

fn is_check_violation<T>(result: Result<T, sqlx::Error>) -> bool {
    result.err().is_some_and(|e| {
        e.as_database_error()
            .is_some_and(|d| d.is_check_violation())
    })
}

fn is_unique_violation<T>(result: Result<T, sqlx::Error>) -> bool {
    result.err().is_some_and(|e| {
        e.as_database_error()
            .is_some_and(|d| d.is_unique_violation())
    })
}

/// รายการคลังหนึ่งแถว (ธง: ติดตามล็อต, ติดตามวันหมดอายุ) — ผูกผู้ป่วยไม่ได้
async fn insert_item(
    pool: &PgPool,
    user_id: i32,
    category_id: i32,
    unit_id: i32,
    name: &str,
    (track_lot, track_expiry): (bool, bool),
    reorder_point: Option<i32>,
) -> Result<i32, sqlx::Error> {
    sqlx::query_scalar(
        "insert into item (name, category_id, base_unit_id, track_lot, track_expiry, patient_linkable,
                           reorder_point, created_by, updated_by)
         values ($1, $2, $3, $4, $5, false, $6, $7, $7) returning id",
    )
    .bind(name)
    .bind(category_id)
    .bind(unit_id)
    .bind(track_lot)
    .bind(track_expiry)
    .bind(reorder_point)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// master คลังกันที่ DB อีกชั้น (ด่านแรกคือ service): ติดตามวันหมดอายุต้องติดตามล็อต (ทั้งค่าตั้งต้นของหมวดและตัวรายการ),
/// จุดสั่งซื้อไม่ติดลบ, factor > 1, ชื่อรายการซ้ำไม่ได้ (ไม่สนตัวพิมพ์ นับเฉพาะแถวที่ยังไม่ถูกลบ), หน่วยซ้ำในรายการเดียวไม่ได้
#[sqlx::test]
async fn inventory_masters_enforce_tracking_quantities_and_uniqueness(pool: PgPool) {
    let user_id = insert_user(&pool).await;

    let bad_category = sqlx::query(
        "insert into item_category (name, default_track_expiry, created_by, updated_by)
         values ('x', true, $1, $1)",
    )
    .bind(user_id)
    .execute(&pool)
    .await;
    assert!(is_check_violation(bad_category));
    let category_id: i32 = sqlx::query_scalar(
        "insert into item_category (name, default_track_lot, default_track_expiry, created_by, updated_by)
         values ('ยา', true, true, $1, $1) returning id",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut unit_ids = Vec::new();
    for name in ["เม็ด", "กล่อง"] {
        let id: i32 = sqlx::query_scalar(
            "insert into item_unit (name, created_by, updated_by) values ($1, $2, $2) returning id",
        )
        .bind(name)
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        unit_ids.push(id);
    }
    let (tablet, boxed) = (unit_ids[0], unit_ids[1]);

    let insert = |name: &'static str, flags: (bool, bool), reorder_point: Option<i32>| {
        insert_item(
            &pool,
            user_id,
            category_id,
            tablet,
            name,
            flags,
            reorder_point,
        )
    };
    assert!(is_check_violation(insert("a", (false, true), None).await));
    assert!(is_check_violation(
        insert("a", (true, true), Some(-1)).await
    ));
    let item_id = insert("Paracetamol", (true, true), Some(0)).await.unwrap();
    assert!(is_unique_violation(
        insert("PARACETAMOL", (false, false), None).await
    ));

    let insert_conversion = |unit_id: i32, factor: i32| {
        sqlx::query(
            "insert into item_unit_conversion (item_id, unit_id, factor, created_by, updated_by)
             values ($1, $2, $3, $4, $4)",
        )
        .bind(item_id)
        .bind(unit_id)
        .bind(factor)
        .bind(user_id)
        .execute(&pool)
    };
    assert!(is_check_violation(insert_conversion(boxed, 1).await));
    insert_conversion(boxed, 100).await.unwrap();
    assert!(is_unique_violation(insert_conversion(boxed, 50).await));

    // soft delete แล้วใช้ชื่อเดิมซ้ำได้
    sqlx::query("update item set deleted_at = now(), deleted_by = $2 where id = $1")
        .bind(item_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    insert("paracetamol", (false, false), None).await.unwrap();
}
