//! เทสระดับ service ของหน้าตั้งค่า master 4 ตัว: prename, nationality, patient_group, lab
//! (โรคกับปีงบประมาณมีไฟล์ของตัวเอง) แต่ละ master ตรวจชุดเดียวกัน: CRUD+ลำดับ, ชื่อซ้ำไม่สนตัวพิมพ์,
//! ค่าว่าง/ยาวเกิน, ห้ามลบข้อมูลที่ยังถูกใช้, สิทธิ์ user.manage, ไม่พบ uuid และหน้าเมนูที่ seed

mod common;

use common::*;
use sqlx::PgPool;
use uuid::Uuid;

// ---------------------------------------------------------------------------------------------
// prename
// ---------------------------------------------------------------------------------------------
mod prename {
    use super::*;
    use my_his_backend::auth::CurrentUser;
    use my_his_backend::prename::service::{self, CreatePrenameInput, UpdatePrenameInput};
    use my_his_backend::prename::PrenameError;

    fn input<'a>(name: &'a str, abbreviation: Option<&'a str>) -> CreatePrenameInput<'a> {
        CreatePrenameInput {
            name,
            abbreviation,
            sort_order: 0,
        }
    }

    fn no_change() -> UpdatePrenameInput<'static> {
        UpdatePrenameInput {
            name: None,
            abbreviation: None,
            sort_order: None,
        }
    }

    async fn find(
        pool: &PgPool,
        actor: &CurrentUser,
        name: &str,
    ) -> my_his_backend::prename::PrenameRow {
        service::list(pool, actor)
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("{name:?} not listed"))
    }

    #[sqlx::test]
    async fn crud_ordering_and_optional_abbreviation(pool: PgPool) {
        let actor = admin(&pool).await;
        service::create(
            &pool,
            &actor,
            CreatePrenameInput {
                sort_order: 2,
                ..input("A", Some("a"))
            },
        )
        .await
        .unwrap();
        service::create(
            &pool,
            &actor,
            CreatePrenameInput {
                sort_order: 1,
                ..input("C", None)
            },
        )
        .await
        .unwrap();
        service::create(
            &pool,
            &actor,
            CreatePrenameInput {
                sort_order: 1,
                ..input("B", Some("  "))
            },
        )
        .await
        .unwrap();

        // เรียง sort_order แล้วชื่อ; abbreviation ว่าง/ช่องว่างล้วนเก็บเป็น NULL
        let listed = service::list(&pool, &actor).await.unwrap();
        let names: Vec<&str> = listed.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["B", "C", "A"]);
        assert_eq!(listed[0].abbreviation, None);
        assert_eq!(listed[2].abbreviation.as_deref(), Some("a"));

        // ตั้งค่า/ล้างค่า abbreviation: ส่งค่า = ตั้ง, ส่ง "" = ล้างเป็น NULL, ไม่ส่ง = ไม่แตะ
        let b = find(&pool, &actor, "B").await.public_id;
        service::update(
            &pool,
            &actor,
            b,
            UpdatePrenameInput {
                abbreviation: Some(" Bb "),
                ..no_change()
            },
        )
        .await
        .unwrap();
        assert_eq!(
            find(&pool, &actor, "B").await.abbreviation.as_deref(),
            Some("Bb")
        );
        service::update(
            &pool,
            &actor,
            b,
            UpdatePrenameInput {
                sort_order: Some(7),
                ..no_change()
            },
        )
        .await
        .unwrap();
        assert_eq!(
            find(&pool, &actor, "B").await.abbreviation.as_deref(),
            Some("Bb")
        );
        service::update(
            &pool,
            &actor,
            b,
            UpdatePrenameInput {
                abbreviation: Some(""),
                ..no_change()
            },
        )
        .await
        .unwrap();
        let cleared = find(&pool, &actor, "B").await;
        assert_eq!(cleared.abbreviation, None);
        assert_eq!(cleared.sort_order, 7);

        service::delete(&pool, &actor, b).await.unwrap();
        assert_eq!(service::list(&pool, &actor).await.unwrap().len(), 2);
        let (deleted, by): (bool, Option<i32>) = sqlx::query_as(
            "select deleted_at is not null, deleted_by from prename where uuid = $1",
        )
        .bind(b)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(deleted);
        assert_eq!(by, Some(actor.user.id.0));
    }

    #[sqlx::test]
    async fn duplicate_names_conflict_case_insensitively(pool: PgPool) {
        let actor = admin(&pool).await;
        service::create(&pool, &actor, input("Mister", None))
            .await
            .unwrap();
        for dup in ["Mister", "mister", "MISTER", "  Mister "] {
            let result = service::create(&pool, &actor, input(dup, None)).await;
            assert!(
                matches!(&result, Err(PrenameError::Conflict(m)) if m == "prename name already taken"),
                "{dup:?} -> {result:?}"
            );
        }
        service::create(&pool, &actor, input("Miss", None))
            .await
            .unwrap();
        let miss = find(&pool, &actor, "Miss").await.public_id;
        let clash = service::update(
            &pool,
            &actor,
            miss,
            UpdatePrenameInput {
                name: Some("mister"),
                ..no_change()
            },
        )
        .await;
        assert!(matches!(clash, Err(PrenameError::Conflict(_))));
        // เปลี่ยนแค่ตัวพิมพ์ของชื่อตัวเองได้
        service::update(
            &pool,
            &actor,
            miss,
            UpdatePrenameInput {
                name: Some("MISS"),
                ..no_change()
            },
        )
        .await
        .unwrap();
        // ลบแล้วสร้างชื่อเดิมได้
        service::delete(&pool, &actor, miss).await.unwrap();
        service::create(&pool, &actor, input("Miss", None))
            .await
            .unwrap();
    }

    #[sqlx::test]
    async fn blank_and_overlong_text_is_rejected(pool: PgPool) {
        let actor = admin(&pool).await;
        for name in ["", "   "] {
            assert!(matches!(
                service::create(&pool, &actor, input(name, None)).await,
                Err(PrenameError::Validation(_))
            ));
        }
        service::create(&pool, &actor, input(&"ก".repeat(100), None))
            .await
            .unwrap();
        assert!(matches!(
            service::create(&pool, &actor, input(&"ข".repeat(101), None)).await,
            Err(PrenameError::Validation(_))
        ));
        assert!(matches!(
            service::create(&pool, &actor, input("X", Some(&"a".repeat(101)))).await,
            Err(PrenameError::Validation(_))
        ));
        let id = find(&pool, &actor, &"ก".repeat(100)).await.public_id;
        assert!(matches!(
            service::update(
                &pool,
                &actor,
                id,
                UpdatePrenameInput {
                    name: Some(" "),
                    ..no_change()
                }
            )
            .await,
            Err(PrenameError::Validation(_))
        ));
    }

    #[sqlx::test]
    async fn used_by_patient_cannot_be_deleted(pool: PgPool) {
        let actor = admin(&pool).await;
        service::create(&pool, &actor, input("Mr", None))
            .await
            .unwrap();
        let row = find(&pool, &actor, "Mr").await;
        let patient = insert_patient(&pool, actor.user.id.0, row.id.0, None).await;

        let blocked = service::delete(&pool, &actor, row.public_id).await;
        assert!(
            matches!(&blocked, Err(PrenameError::Conflict(m)) if m == "prename is still used"),
            "{blocked:?}"
        );
        soft_delete_row(&pool, "patient", patient, actor.user.id.0).await;
        service::delete(&pool, &actor, row.public_id).await.unwrap();
    }

    /// คำนำหน้าของผู้ใช้งาน (users.prename_id — ผู้ใช้กรอกตอนลงทะเบียน) ก็กันลบเหมือนของคนไข้
    /// ผู้ใช้ไม่มีการลบจริง จึงลบคำนำหน้าได้หลังเปลี่ยนคำนำหน้าของผู้ใช้คนนั้นออกแล้วเท่านั้น
    #[sqlx::test]
    async fn used_by_user_cannot_be_deleted(pool: PgPool) {
        let actor = admin(&pool).await;
        service::create(&pool, &actor, input("Mrs", None))
            .await
            .unwrap();
        let row = find(&pool, &actor, "Mrs").await;
        sqlx::query("update users set prename_id = $1 where id = $2")
            .bind(row.id.0)
            .bind(actor.user.id.0)
            .execute(&pool)
            .await
            .unwrap();

        let blocked = service::delete(&pool, &actor, row.public_id).await;
        assert!(
            matches!(&blocked, Err(PrenameError::Conflict(m)) if m == "prename is still used"),
            "{blocked:?}"
        );
        sqlx::query("update users set prename_id = null where id = $1")
            .bind(actor.user.id.0)
            .execute(&pool)
            .await
            .unwrap();
        service::delete(&pool, &actor, row.public_id).await.unwrap();
    }

    #[sqlx::test]
    async fn permission_and_not_found(pool: PgPool) {
        let admin_actor = admin(&pool).await;
        service::create(&pool, &admin_actor, input("Mr", None))
            .await
            .unwrap();
        let id = find(&pool, &admin_actor, "Mr").await.public_id;

        let nurse = nurse(&pool).await;
        assert!(matches!(
            service::list(&pool, &nurse).await,
            Err(PrenameError::Forbidden)
        ));
        assert!(matches!(
            service::create(&pool, &nurse, input("X", None)).await,
            Err(PrenameError::Forbidden)
        ));
        assert!(matches!(
            service::update(&pool, &nurse, id, no_change()).await,
            Err(PrenameError::Forbidden)
        ));
        assert!(matches!(
            service::delete(&pool, &nurse, id).await,
            Err(PrenameError::Forbidden)
        ));
        assert_eq!(service::list(&pool, &admin_actor).await.unwrap().len(), 1);

        let missing = Uuid::new_v4();
        assert!(matches!(
            service::update(&pool, &admin_actor, missing, no_change()).await,
            Err(PrenameError::NotFound)
        ));
        assert!(matches!(
            service::delete(&pool, &admin_actor, missing).await,
            Err(PrenameError::NotFound)
        ));
    }

    #[sqlx::test]
    async fn menu_page_is_seeded(pool: PgPool) {
        let (label, group, permission) = seeded_page(&pool, "/settings/prenames").await;
        assert_eq!(label, "ตั้งค่าคำนำหน้า");
        assert_eq!(group.as_deref(), Some("การตั้งค่า"));
        assert_eq!(permission, "user.manage");
    }
}

// ---------------------------------------------------------------------------------------------
// nationality
// ---------------------------------------------------------------------------------------------
mod nationality {
    use super::*;
    use my_his_backend::auth::CurrentUser;
    use my_his_backend::nationality::service::{
        self, CreateNationalityInput, UpdateNationalityInput,
    };
    use my_his_backend::nationality::NationalityError;

    fn input<'a>(national_name: &'a str, nation: &'a str) -> CreateNationalityInput<'a> {
        CreateNationalityInput {
            national_name,
            nation,
            sort_order: 0,
            is_active: true,
        }
    }

    fn no_change() -> UpdateNationalityInput<'static> {
        UpdateNationalityInput {
            national_name: None,
            nation: None,
            sort_order: None,
            is_active: None,
        }
    }

    async fn find(
        pool: &PgPool,
        actor: &CurrentUser,
        name: &str,
    ) -> my_his_backend::nationality::NationalityRow {
        service::list(pool, actor)
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.national_name == name)
            .unwrap_or_else(|| panic!("{name:?} not listed"))
    }

    #[sqlx::test]
    async fn crud_and_ordering(pool: PgPool) {
        let actor = admin(&pool).await;
        service::create(
            &pool,
            &actor,
            CreateNationalityInput {
                sort_order: 2,
                ..input("A", "Aland")
            },
        )
        .await
        .unwrap();
        service::create(
            &pool,
            &actor,
            CreateNationalityInput {
                sort_order: 1,
                ..input("C", "Cuba")
            },
        )
        .await
        .unwrap();
        service::create(
            &pool,
            &actor,
            CreateNationalityInput {
                sort_order: 1,
                ..input("B", "Benin")
            },
        )
        .await
        .unwrap();

        let listed = service::list(&pool, &actor).await.unwrap();
        let names: Vec<&str> = listed.iter().map(|r| r.national_name.as_str()).collect();
        assert_eq!(names, ["B", "C", "A"]);
        assert!(listed.iter().all(|r| r.is_active));

        let b = find(&pool, &actor, "B").await.public_id;
        service::update(
            &pool,
            &actor,
            b,
            UpdateNationalityInput {
                nation: Some("  Benin City "),
                is_active: Some(false),
                sort_order: Some(9),
                ..no_change()
            },
        )
        .await
        .unwrap();
        let updated = find(&pool, &actor, "B").await;
        assert_eq!(updated.nation, "Benin City");
        assert!(!updated.is_active);
        assert_eq!(updated.sort_order, 9);

        service::delete(&pool, &actor, b).await.unwrap();
        assert_eq!(service::list(&pool, &actor).await.unwrap().len(), 2);
    }

    #[sqlx::test]
    async fn duplicate_names_conflict_case_insensitively(pool: PgPool) {
        let actor = admin(&pool).await;
        service::create(&pool, &actor, input("Thai", "Thailand"))
            .await
            .unwrap();
        for dup in ["Thai", "thai", "THAI", " Thai "] {
            let result = service::create(&pool, &actor, input(dup, "X")).await;
            assert!(
                matches!(&result, Err(NationalityError::Conflict(m)) if m == "nationality name already taken"),
                "{dup:?} -> {result:?}"
            );
        }
        // ประเทศซ้ำได้ (หลายสัญชาติ/เชื้อชาติมาจากประเทศเดียวกัน)
        service::create(&pool, &actor, input("Lao", "Thailand"))
            .await
            .unwrap();
        let lao = find(&pool, &actor, "Lao").await.public_id;
        assert!(matches!(
            service::update(
                &pool,
                &actor,
                lao,
                UpdateNationalityInput {
                    national_name: Some("thai"),
                    ..no_change()
                }
            )
            .await,
            Err(NationalityError::Conflict(_))
        ));
        service::delete(&pool, &actor, lao).await.unwrap();
        service::create(&pool, &actor, input("Lao", "Laos"))
            .await
            .unwrap();
    }

    #[sqlx::test]
    async fn blank_and_overlong_text_is_rejected(pool: PgPool) {
        let actor = admin(&pool).await;
        for (a, b) in [("", "X"), ("  ", "X"), ("X", ""), ("X", " \t ")] {
            assert!(
                matches!(
                    service::create(&pool, &actor, input(a, b)).await,
                    Err(NationalityError::Validation(_))
                ),
                "{a:?}/{b:?}"
            );
        }
        service::create(&pool, &actor, input(&"ก".repeat(100), &"ข".repeat(100)))
            .await
            .unwrap();
        assert!(matches!(
            service::create(&pool, &actor, input(&"ก".repeat(101), "X")).await,
            Err(NationalityError::Validation(_))
        ));
        assert!(matches!(
            service::create(&pool, &actor, input("Y", &"ก".repeat(101))).await,
            Err(NationalityError::Validation(_))
        ));
    }

    #[sqlx::test]
    async fn used_by_patient_cannot_be_deleted(pool: PgPool) {
        let actor = admin(&pool).await;
        service::create(&pool, &actor, input("Thai", "Thailand"))
            .await
            .unwrap();
        let row = find(&pool, &actor, "Thai").await;
        let prename = insert_prename(&pool, actor.user.id.0, "Mr").await;
        let patient = insert_patient(&pool, actor.user.id.0, prename, Some(row.id.0)).await;

        let blocked = service::delete(&pool, &actor, row.public_id).await;
        assert!(
            matches!(&blocked, Err(NationalityError::Conflict(m)) if m == "nationality is still used"),
            "{blocked:?}"
        );
        soft_delete_row(&pool, "patient", patient, actor.user.id.0).await;
        service::delete(&pool, &actor, row.public_id).await.unwrap();
    }

    #[sqlx::test]
    async fn permission_and_not_found(pool: PgPool) {
        let admin_actor = admin(&pool).await;
        service::create(&pool, &admin_actor, input("Thai", "Thailand"))
            .await
            .unwrap();
        let id = find(&pool, &admin_actor, "Thai").await.public_id;

        let nurse = nurse(&pool).await;
        assert!(matches!(
            service::list(&pool, &nurse).await,
            Err(NationalityError::Forbidden)
        ));
        assert!(matches!(
            service::create(&pool, &nurse, input("X", "X")).await,
            Err(NationalityError::Forbidden)
        ));
        assert!(matches!(
            service::update(&pool, &nurse, id, no_change()).await,
            Err(NationalityError::Forbidden)
        ));
        assert!(matches!(
            service::delete(&pool, &nurse, id).await,
            Err(NationalityError::Forbidden)
        ));
        assert_eq!(service::list(&pool, &admin_actor).await.unwrap().len(), 1);

        let missing = Uuid::new_v4();
        assert!(matches!(
            service::update(&pool, &admin_actor, missing, no_change()).await,
            Err(NationalityError::NotFound)
        ));
        assert!(matches!(
            service::delete(&pool, &admin_actor, missing).await,
            Err(NationalityError::NotFound)
        ));
    }

    #[sqlx::test]
    async fn menu_page_is_seeded(pool: PgPool) {
        let (label, group, permission) = seeded_page(&pool, "/settings/nationalities").await;
        assert_eq!(label, "ตั้งค่าสัญชาติ");
        assert_eq!(group.as_deref(), Some("การตั้งค่า"));
        assert_eq!(permission, "user.manage");
    }
}

// ---------------------------------------------------------------------------------------------
// patient_group
// ---------------------------------------------------------------------------------------------
mod patient_group {
    use super::*;
    use my_his_backend::auth::CurrentUser;
    use my_his_backend::patient_group::service::{
        self, CreatePatientGroupInput, UpdatePatientGroupInput,
    };
    use my_his_backend::patient_group::PatientGroupError;

    fn input(name: &str) -> CreatePatientGroupInput<'_> {
        CreatePatientGroupInput {
            name,
            sort_order: 0,
            is_active: true,
        }
    }

    fn no_change() -> UpdatePatientGroupInput<'static> {
        UpdatePatientGroupInput {
            name: None,
            sort_order: None,
            is_active: None,
        }
    }

    async fn find(
        pool: &PgPool,
        actor: &CurrentUser,
        name: &str,
    ) -> my_his_backend::patient_group::PatientGroupRow {
        service::list(pool, actor)
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("{name:?} not listed"))
    }

    #[sqlx::test]
    async fn crud_and_ordering(pool: PgPool) {
        let actor = admin(&pool).await;
        service::create(
            &pool,
            &actor,
            CreatePatientGroupInput {
                sort_order: 2,
                ..input("A")
            },
        )
        .await
        .unwrap();
        service::create(
            &pool,
            &actor,
            CreatePatientGroupInput {
                sort_order: 1,
                ..input("C")
            },
        )
        .await
        .unwrap();
        service::create(
            &pool,
            &actor,
            CreatePatientGroupInput {
                sort_order: 1,
                ..input("B")
            },
        )
        .await
        .unwrap();

        let listed = service::list(&pool, &actor).await.unwrap();
        let names: Vec<&str> = listed.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["B", "C", "A"]);

        let b = find(&pool, &actor, "B").await.public_id;
        service::update(
            &pool,
            &actor,
            b,
            UpdatePatientGroupInput {
                name: Some("  จามจุรี1 "),
                is_active: Some(false),
                ..no_change()
            },
        )
        .await
        .unwrap();
        let updated = find(&pool, &actor, "จามจุรี1").await;
        assert!(!updated.is_active);

        service::delete(&pool, &actor, b).await.unwrap();
        assert_eq!(service::list(&pool, &actor).await.unwrap().len(), 2);
    }

    #[sqlx::test]
    async fn duplicate_names_conflict_case_insensitively(pool: PgPool) {
        let actor = admin(&pool).await;
        service::create(&pool, &actor, input("Ward1"))
            .await
            .unwrap();
        for dup in ["Ward1", "ward1", "WARD1", " Ward1 "] {
            let result = service::create(&pool, &actor, input(dup)).await;
            assert!(
                matches!(&result, Err(PatientGroupError::Conflict(m)) if m == "patient group name already taken"),
                "{dup:?} -> {result:?}"
            );
        }
        service::create(&pool, &actor, input("Ward2"))
            .await
            .unwrap();
        let ward2 = find(&pool, &actor, "Ward2").await.public_id;
        assert!(matches!(
            service::update(
                &pool,
                &actor,
                ward2,
                UpdatePatientGroupInput {
                    name: Some("ward1"),
                    ..no_change()
                }
            )
            .await,
            Err(PatientGroupError::Conflict(_))
        ));
        service::delete(&pool, &actor, ward2).await.unwrap();
        service::create(&pool, &actor, input("Ward2"))
            .await
            .unwrap();
    }

    #[sqlx::test]
    async fn blank_and_overlong_text_is_rejected(pool: PgPool) {
        let actor = admin(&pool).await;
        for name in ["", "  "] {
            assert!(matches!(
                service::create(&pool, &actor, input(name)).await,
                Err(PatientGroupError::Validation(_))
            ));
        }
        service::create(&pool, &actor, input(&"ก".repeat(100)))
            .await
            .unwrap();
        assert!(matches!(
            service::create(&pool, &actor, input(&"ก".repeat(101))).await,
            Err(PatientGroupError::Validation(_))
        ));
    }

    #[sqlx::test]
    async fn used_by_visit_group_or_lab_visit_group_cannot_be_deleted(pool: PgPool) {
        let actor = admin(&pool).await;
        let user = actor.user.id.0;
        service::create(&pool, &actor, input("Ward1"))
            .await
            .unwrap();
        let row = find(&pool, &actor, "Ward1").await;

        // อ้างโดย visit_group
        let visit_group: i32 = sqlx::query_scalar(
            "insert into visit_group (patient_group_id, created_by, updated_by) values ($1, $2, $2) returning id",
        )
        .bind(row.id.0)
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
        let blocked = service::delete(&pool, &actor, row.public_id).await;
        assert!(
            matches!(&blocked, Err(PatientGroupError::Conflict(m)) if m == "patient group is still used"),
            "{blocked:?}"
        );

        // ปลด visit_group แล้ว แต่ยังถูกอ้างโดย lab_visit_group
        soft_delete_row(&pool, "visit_group", visit_group, user).await;
        let budget_year: i32 = sqlx::query_scalar(
            "insert into budget_year (year, start_date, end_date, created_by, updated_by)
             values ('2570', '2026-10-01', '2027-09-30', $1, $1) returning id",
        )
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
        let lab_visit_group: i32 = sqlx::query_scalar(
            "insert into lab_visit_group (budget_year_id, patient_group_id, cd4_1st_date, cd4_2nd_date, vl_date, created_by, updated_by)
             values ($1, $2, '2026-10-04', '2027-04-04', '2027-04-04', $3, $3) returning id",
        )
        .bind(budget_year)
        .bind(row.id.0)
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(matches!(
            service::delete(&pool, &actor, row.public_id).await,
            Err(PatientGroupError::Conflict(_))
        ));

        soft_delete_row(&pool, "lab_visit_group", lab_visit_group, user).await;
        service::delete(&pool, &actor, row.public_id).await.unwrap();
    }

    #[sqlx::test]
    async fn permission_and_not_found(pool: PgPool) {
        let admin_actor = admin(&pool).await;
        service::create(&pool, &admin_actor, input("Ward1"))
            .await
            .unwrap();
        let id = find(&pool, &admin_actor, "Ward1").await.public_id;

        let nurse = nurse(&pool).await;
        assert!(matches!(
            service::list(&pool, &nurse).await,
            Err(PatientGroupError::Forbidden)
        ));
        assert!(matches!(
            service::create(&pool, &nurse, input("X")).await,
            Err(PatientGroupError::Forbidden)
        ));
        assert!(matches!(
            service::update(&pool, &nurse, id, no_change()).await,
            Err(PatientGroupError::Forbidden)
        ));
        assert!(matches!(
            service::delete(&pool, &nurse, id).await,
            Err(PatientGroupError::Forbidden)
        ));
        assert_eq!(service::list(&pool, &admin_actor).await.unwrap().len(), 1);

        let missing = Uuid::new_v4();
        assert!(matches!(
            service::update(&pool, &admin_actor, missing, no_change()).await,
            Err(PatientGroupError::NotFound)
        ));
        assert!(matches!(
            service::delete(&pool, &admin_actor, missing).await,
            Err(PatientGroupError::NotFound)
        ));
    }

    #[sqlx::test]
    async fn menu_page_is_seeded(pool: PgPool) {
        let (label, group, permission) = seeded_page(&pool, "/settings/patient-groups").await;
        assert_eq!(label, "ตั้งค่ากลุ่มคนไข้");
        assert_eq!(group.as_deref(), Some("การตั้งค่า"));
        assert_eq!(permission, "user.manage");
    }
}

// ---------------------------------------------------------------------------------------------
// lab
// ---------------------------------------------------------------------------------------------
mod lab {
    use super::*;
    use my_his_backend::auth::CurrentUser;
    use my_his_backend::lab::service::{self, CreateLabInput, UpdateLabInput};
    use my_his_backend::lab::LabError;

    fn input<'a>(name: &'a str, abbreviation: Option<&'a str>) -> CreateLabInput<'a> {
        CreateLabInput {
            name,
            abbreviation,
            is_active: true,
            sort_order: 0,
        }
    }

    fn no_change() -> UpdateLabInput<'static> {
        UpdateLabInput {
            name: None,
            abbreviation: None,
            is_active: None,
            sort_order: None,
        }
    }

    async fn find(pool: &PgPool, actor: &CurrentUser, name: &str) -> my_his_backend::lab::LabRow {
        service::list(pool, actor)
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("{name:?} not listed"))
    }

    #[sqlx::test]
    async fn crud_ordering_and_optional_abbreviation(pool: PgPool) {
        let actor = admin(&pool).await;
        service::create(
            &pool,
            &actor,
            CreateLabInput {
                sort_order: 2,
                ..input("A", Some("a"))
            },
        )
        .await
        .unwrap();
        service::create(
            &pool,
            &actor,
            CreateLabInput {
                sort_order: 1,
                ..input("C", None)
            },
        )
        .await
        .unwrap();
        service::create(
            &pool,
            &actor,
            CreateLabInput {
                sort_order: 1,
                ..input("B", Some(" "))
            },
        )
        .await
        .unwrap();

        let listed = service::list(&pool, &actor).await.unwrap();
        let names: Vec<&str> = listed.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["B", "C", "A"]);
        assert_eq!(listed[0].abbreviation, None);
        assert_eq!(listed[2].abbreviation.as_deref(), Some("a"));
        assert!(listed.iter().all(|r| r.is_active));

        let b = find(&pool, &actor, "B").await.public_id;
        service::update(
            &pool,
            &actor,
            b,
            UpdateLabInput {
                abbreviation: Some(" Bb "),
                is_active: Some(false),
                ..no_change()
            },
        )
        .await
        .unwrap();
        let updated = find(&pool, &actor, "B").await;
        assert_eq!(updated.abbreviation.as_deref(), Some("Bb"));
        assert!(!updated.is_active);
        service::update(
            &pool,
            &actor,
            b,
            UpdateLabInput {
                abbreviation: Some(""),
                ..no_change()
            },
        )
        .await
        .unwrap();
        assert_eq!(find(&pool, &actor, "B").await.abbreviation, None);

        // ยังไม่มีตารางใดอ้างอิง lab — ลบได้เสมอ
        service::delete(&pool, &actor, b).await.unwrap();
        assert_eq!(service::list(&pool, &actor).await.unwrap().len(), 2);
    }

    #[sqlx::test]
    async fn duplicate_names_conflict_case_insensitively(pool: PgPool) {
        let actor = admin(&pool).await;
        service::create(&pool, &actor, input("CD4 count", Some("CD4")))
            .await
            .unwrap();
        for dup in ["CD4 count", "cd4 COUNT", "  CD4 count "] {
            let result = service::create(&pool, &actor, input(dup, None)).await;
            assert!(
                matches!(&result, Err(LabError::Conflict(m)) if m == "lab name already taken"),
                "{dup:?} -> {result:?}"
            );
        }
        // คำย่อซ้ำได้
        service::create(&pool, &actor, input("Viral load", Some("CD4")))
            .await
            .unwrap();
        let vl = find(&pool, &actor, "Viral load").await.public_id;
        assert!(matches!(
            service::update(
                &pool,
                &actor,
                vl,
                UpdateLabInput {
                    name: Some("cd4 count"),
                    ..no_change()
                }
            )
            .await,
            Err(LabError::Conflict(_))
        ));
        service::delete(&pool, &actor, vl).await.unwrap();
        service::create(&pool, &actor, input("Viral load", None))
            .await
            .unwrap();
    }

    #[sqlx::test]
    async fn blank_and_overlong_text_is_rejected(pool: PgPool) {
        let actor = admin(&pool).await;
        for name in ["", "  "] {
            assert!(matches!(
                service::create(&pool, &actor, input(name, None)).await,
                Err(LabError::Validation(_))
            ));
        }
        service::create(&pool, &actor, input(&"ก".repeat(100), None))
            .await
            .unwrap();
        assert!(matches!(
            service::create(&pool, &actor, input(&"ก".repeat(101), None)).await,
            Err(LabError::Validation(_))
        ));
        assert!(matches!(
            service::create(&pool, &actor, input("X", Some(&"a".repeat(101)))).await,
            Err(LabError::Validation(_))
        ));
    }

    #[sqlx::test]
    async fn permission_and_not_found(pool: PgPool) {
        let admin_actor = admin(&pool).await;
        service::create(&pool, &admin_actor, input("CD4", None))
            .await
            .unwrap();
        let id = find(&pool, &admin_actor, "CD4").await.public_id;

        let nurse = nurse(&pool).await;
        assert!(matches!(
            service::list(&pool, &nurse).await,
            Err(LabError::Forbidden)
        ));
        assert!(matches!(
            service::create(&pool, &nurse, input("X", None)).await,
            Err(LabError::Forbidden)
        ));
        assert!(matches!(
            service::update(&pool, &nurse, id, no_change()).await,
            Err(LabError::Forbidden)
        ));
        assert!(matches!(
            service::delete(&pool, &nurse, id).await,
            Err(LabError::Forbidden)
        ));
        assert_eq!(service::list(&pool, &admin_actor).await.unwrap().len(), 1);

        let missing = Uuid::new_v4();
        assert!(matches!(
            service::update(&pool, &admin_actor, missing, no_change()).await,
            Err(LabError::NotFound)
        ));
        assert!(matches!(
            service::delete(&pool, &admin_actor, missing).await,
            Err(LabError::NotFound)
        ));
    }

    #[sqlx::test]
    async fn menu_page_is_seeded(pool: PgPool) {
        let (label, group, permission) = seeded_page(&pool, "/settings/labs").await;
        assert_eq!(label, "ตั้งค่าแลป");
        assert_eq!(group.as_deref(), Some("การตั้งค่า"));
        assert_eq!(permission, "user.manage");
    }
}
