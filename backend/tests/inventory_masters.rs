//! เทสระดับ service ของ master คลัง 3 หน้า: item_category, item_unit, item (+ หน่วยแปลงของรายการ)
//! ตรวจชุดเดียวกับ masters.rs (CRUD+ลำดับ, ชื่อซ้ำไม่สนตัวพิมพ์, ค่าว่าง/ยาวเกิน, ห้ามลบข้อมูลที่ยังถูกใช้, สิทธิ์ราย
//! action, ไม่พบ uuid, หน้าเมนูที่ seed) + กฎเฉพาะของคลัง (docs/inventory-design.md): ติดตามวันหมดอายุต้องติดตามล็อต,
//! ธงของรายการคัดลอกจากหมวดตอนเพิ่มเท่านั้น, หน่วยแปลง, หมวด/หน่วยที่ปิดใช้งานเลือกใหม่ไม่ได้แต่คงไว้ได้

mod common;

use common::*;
use my_his_backend::auth::CurrentUser;
use my_his_backend::domain::Resource;
use my_his_backend::item::service::{self as item_service, ConversionInput, ItemInput, ItemView};
use my_his_backend::item::ItemError;
use my_his_backend::item_category::service::{
    self as category_service, CreateItemCategoryInput, UpdateItemCategoryInput,
};
use my_his_backend::item_category::{ItemCategoryError, ItemCategoryRow};
use my_his_backend::item_unit::service::{
    self as unit_service, CreateItemUnitInput, UpdateItemUnitInput,
};
use my_his_backend::item_unit::{ItemUnitError, ItemUnitRow};
use sqlx::PgPool;
use uuid::Uuid;

fn category(name: &str) -> CreateItemCategoryInput<'_> {
    CreateItemCategoryInput {
        name,
        default_track_lot: false,
        default_track_expiry: false,
        default_patient_linkable: false,
        sort_order: 0,
        is_active: true,
    }
}

fn category_no_change() -> UpdateItemCategoryInput<'static> {
    UpdateItemCategoryInput {
        name: None,
        default_track_lot: None,
        default_track_expiry: None,
        default_patient_linkable: None,
        sort_order: None,
        is_active: None,
    }
}

fn unit(name: &str) -> CreateItemUnitInput<'_> {
    CreateItemUnitInput {
        name,
        sort_order: 0,
        is_active: true,
    }
}

fn unit_no_change() -> UpdateItemUnitInput<'static> {
    UpdateItemUnitInput {
        name: None,
        sort_order: None,
        is_active: None,
    }
}

/// รายการที่ไม่ส่งธง (= ใช้ค่าตั้งต้นของหมวด/ค่าเดิม) ไม่มีจุดสั่งซื้อและหน่วยแปลง
fn item(name: &str, category_id: Uuid, base_unit_id: Uuid) -> ItemInput<'_> {
    ItemInput {
        name,
        category_id,
        base_unit_id,
        track_lot: None,
        track_expiry: None,
        patient_linkable: None,
        reorder_point: None,
        is_active: true,
        conversions: Vec::new(),
    }
}

fn conversion(unit_id: Uuid, factor: i32) -> ConversionInput {
    ConversionInput { unit_id, factor }
}

async fn find_category(pool: &PgPool, actor: &CurrentUser, name: &str) -> ItemCategoryRow {
    category_service::list(pool, actor)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("{name:?} not listed"))
}

async fn find_unit(pool: &PgPool, actor: &CurrentUser, name: &str) -> ItemUnitRow {
    unit_service::list(pool, actor)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("{name:?} not listed"))
}

async fn find_item(pool: &PgPool, actor: &CurrentUser, name: &str) -> ItemView {
    item_service::list(pool, actor)
        .await
        .unwrap()
        .into_iter()
        .find(|v| v.item.name == name)
        .unwrap_or_else(|| panic!("{name:?} not listed"))
}

/// หมวด "ยา" (ติดตามล็อต + วันหมดอายุ + ผูกผู้ป่วยได้) และหน่วย เม็ด/แผง/กล่อง — uuid ของแต่ละแถว
struct Fixture {
    drug: Uuid,
    tablet: Uuid,
    strip: Uuid,
    boxed: Uuid,
}

async fn fixture(pool: &PgPool, actor: &CurrentUser) -> Fixture {
    category_service::create(
        pool,
        actor,
        CreateItemCategoryInput {
            default_track_lot: true,
            default_track_expiry: true,
            default_patient_linkable: true,
            ..category("ยา")
        },
    )
    .await
    .unwrap();
    for name in ["เม็ด", "แผง", "กล่อง"] {
        unit_service::create(pool, actor, unit(name)).await.unwrap();
    }
    Fixture {
        drug: find_category(pool, actor, "ยา").await.public_id,
        tablet: find_unit(pool, actor, "เม็ด").await.public_id,
        strip: find_unit(pool, actor, "แผง").await.public_id,
        boxed: find_unit(pool, actor, "กล่อง").await.public_id,
    }
}

fn item_validation(result: Result<(), ItemError>) -> String {
    match result {
        Err(ItemError::Validation(message)) => message,
        other => panic!("expected a validation error, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// item_category
// ---------------------------------------------------------------------------------------------
mod item_category {
    use super::*;

    #[sqlx::test]
    async fn crud_ordering_and_default_flags(pool: PgPool) {
        let actor = admin(&pool).await;
        for (name, sort_order) in [("A", 2), ("C", 1), ("B", 1)] {
            category_service::create(
                &pool,
                &actor,
                CreateItemCategoryInput {
                    sort_order,
                    ..category(name)
                },
            )
            .await
            .unwrap();
        }
        category_service::create(
            &pool,
            &actor,
            CreateItemCategoryInput {
                default_track_lot: true,
                default_track_expiry: true,
                default_patient_linkable: true,
                sort_order: 3,
                ..category("ยา")
            },
        )
        .await
        .unwrap();

        let listed = category_service::list(&pool, &actor).await.unwrap();
        let names: Vec<&str> = listed.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["B", "C", "A", "ยา"]);
        let drug = find_category(&pool, &actor, "ยา").await;
        assert!(
            drug.default_track_lot && drug.default_track_expiry && drug.default_patient_linkable
        );
        let b = find_category(&pool, &actor, "B").await;
        assert!(!b.default_track_lot && !b.default_track_expiry && !b.default_patient_linkable);

        category_service::update(
            &pool,
            &actor,
            b.public_id,
            UpdateItemCategoryInput {
                name: Some("  เวชภัณฑ์ "),
                default_track_lot: Some(true),
                is_active: Some(false),
                ..category_no_change()
            },
        )
        .await
        .unwrap();
        let updated = find_category(&pool, &actor, "เวชภัณฑ์").await;
        assert!(updated.default_track_lot && !updated.default_track_expiry);
        assert!(!updated.is_active);

        category_service::delete(&pool, &actor, b.public_id)
            .await
            .unwrap();
        assert_eq!(
            category_service::list(&pool, &actor).await.unwrap().len(),
            3
        );
    }

    #[sqlx::test]
    async fn duplicate_names_conflict_case_insensitively(pool: PgPool) {
        let actor = admin(&pool).await;
        category_service::create(&pool, &actor, category("Lab"))
            .await
            .unwrap();
        for dup in ["Lab", "lab", "LAB", " Lab "] {
            let result = category_service::create(&pool, &actor, category(dup)).await;
            assert!(
                matches!(&result, Err(ItemCategoryError::Conflict(m)) if m == "item category name already taken"),
                "{dup:?} -> {result:?}"
            );
        }
        category_service::create(&pool, &actor, category("Drug"))
            .await
            .unwrap();
        let drug = find_category(&pool, &actor, "Drug").await.public_id;
        assert!(matches!(
            category_service::update(
                &pool,
                &actor,
                drug,
                UpdateItemCategoryInput {
                    name: Some("lab"),
                    ..category_no_change()
                }
            )
            .await,
            Err(ItemCategoryError::Conflict(_))
        ));
        category_service::delete(&pool, &actor, drug).await.unwrap();
        category_service::create(&pool, &actor, category("Drug"))
            .await
            .unwrap();
    }

    #[sqlx::test]
    async fn blank_and_overlong_name_is_rejected(pool: PgPool) {
        let actor = admin(&pool).await;
        for name in ["", "  "] {
            assert!(matches!(
                category_service::create(&pool, &actor, category(name)).await,
                Err(ItemCategoryError::Validation(_))
            ));
        }
        category_service::create(&pool, &actor, category(&"ก".repeat(100)))
            .await
            .unwrap();
        assert!(matches!(
            category_service::create(&pool, &actor, category(&"ก".repeat(101))).await,
            Err(ItemCategoryError::Validation(_))
        ));
    }

    #[sqlx::test]
    async fn expiry_tracking_requires_lot_tracking(pool: PgPool) {
        let actor = admin(&pool).await;
        let rejected = category_service::create(
            &pool,
            &actor,
            CreateItemCategoryInput {
                default_track_expiry: true,
                ..category("X")
            },
        )
        .await;
        assert!(
            matches!(&rejected, Err(ItemCategoryError::Validation(m)) if m == "expiry tracking requires lot tracking"),
            "{rejected:?}"
        );

        category_service::create(
            &pool,
            &actor,
            CreateItemCategoryInput {
                default_track_lot: true,
                default_track_expiry: true,
                ..category("ยา")
            },
        )
        .await
        .unwrap();
        let drug = find_category(&pool, &actor, "ยา").await.public_id;

        // ส่งมาธงเดียว ต้องตรวจคู่กับค่าเดิมของอีกธง
        let lot_off = category_service::update(
            &pool,
            &actor,
            drug,
            UpdateItemCategoryInput {
                default_track_lot: Some(false),
                ..category_no_change()
            },
        )
        .await;
        assert!(matches!(lot_off, Err(ItemCategoryError::Validation(_))));

        category_service::update(
            &pool,
            &actor,
            drug,
            UpdateItemCategoryInput {
                default_track_lot: Some(false),
                default_track_expiry: Some(false),
                ..category_no_change()
            },
        )
        .await
        .unwrap();
        let expiry_on = category_service::update(
            &pool,
            &actor,
            drug,
            UpdateItemCategoryInput {
                default_track_expiry: Some(true),
                ..category_no_change()
            },
        )
        .await;
        assert!(matches!(expiry_on, Err(ItemCategoryError::Validation(_))));

        let row = find_category(&pool, &actor, "ยา").await;
        assert!(!row.default_track_lot && !row.default_track_expiry);
    }

    #[sqlx::test]
    async fn used_by_item_cannot_be_deleted(pool: PgPool) {
        let actor = admin(&pool).await;
        let f = fixture(&pool, &actor).await;
        item_service::create(&pool, &actor, item("พาราเซตามอล", f.drug, f.tablet))
            .await
            .unwrap();

        let blocked = category_service::delete(&pool, &actor, f.drug).await;
        assert!(
            matches!(&blocked, Err(ItemCategoryError::Conflict(m)) if m == "item category is still used"),
            "{blocked:?}"
        );

        let paracetamol = find_item(&pool, &actor, "พาราเซตามอล").await.item.public_id;
        item_service::delete(&pool, &actor, paracetamol)
            .await
            .unwrap();
        category_service::delete(&pool, &actor, f.drug)
            .await
            .unwrap();
    }

    #[sqlx::test]
    async fn permission_and_not_found(pool: PgPool) {
        let admin_actor = admin(&pool).await;
        category_service::create(&pool, &admin_actor, category("ยา"))
            .await
            .unwrap();
        let id = find_category(&pool, &admin_actor, "ยา").await.public_id;

        // มีสิทธิ์ครบทุกหน้าอื่น (รวมหน่วยนับ/รายการ) ก็ยังจัดการหมวดไม่ได้
        let other = actor_with(&pool, all_except(Resource::ItemCategory)).await;
        assert!(matches!(
            category_service::list(&pool, &other).await,
            Err(ItemCategoryError::Forbidden)
        ));
        assert!(matches!(
            category_service::create(&pool, &other, category("X")).await,
            Err(ItemCategoryError::Forbidden)
        ));
        assert!(matches!(
            category_service::update(&pool, &other, id, category_no_change()).await,
            Err(ItemCategoryError::Forbidden)
        ));
        assert!(matches!(
            category_service::delete(&pool, &other, id).await,
            Err(ItemCategoryError::Forbidden)
        ));

        // ดูอย่างเดียว: ดูได้ แต่เพิ่มไม่ได้
        let viewer = actor_with(&pool, vec![perm("item_category.view")]).await;
        assert_eq!(
            category_service::list(&pool, &viewer).await.unwrap().len(),
            1
        );
        assert!(matches!(
            category_service::create(&pool, &viewer, category("X")).await,
            Err(ItemCategoryError::Forbidden)
        ));

        let missing = Uuid::new_v4();
        assert!(matches!(
            category_service::update(&pool, &admin_actor, missing, category_no_change()).await,
            Err(ItemCategoryError::NotFound)
        ));
        assert!(matches!(
            category_service::delete(&pool, &admin_actor, missing).await,
            Err(ItemCategoryError::NotFound)
        ));
    }

    #[sqlx::test]
    async fn menu_page_is_seeded(pool: PgPool) {
        let (label, group, permission) = seeded_page(&pool, "/settings/item-categories").await;
        assert_eq!(label, "ตั้งค่าหมวดคลัง");
        assert_eq!(group.as_deref(), Some("การตั้งค่า"));
        assert_eq!(permission, "item_category.view");
    }
}

// ---------------------------------------------------------------------------------------------
// item_unit
// ---------------------------------------------------------------------------------------------
mod item_unit {
    use super::*;

    #[sqlx::test]
    async fn crud_and_ordering(pool: PgPool) {
        let actor = admin(&pool).await;
        for (name, sort_order) in [("กล่อง", 3), ("เม็ด", 1), ("แผง", 2)] {
            unit_service::create(
                &pool,
                &actor,
                CreateItemUnitInput {
                    sort_order,
                    ..unit(name)
                },
            )
            .await
            .unwrap();
        }
        let listed = unit_service::list(&pool, &actor).await.unwrap();
        let names: Vec<&str> = listed.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["เม็ด", "แผง", "กล่อง"]);

        let strip = find_unit(&pool, &actor, "แผง").await.public_id;
        unit_service::update(
            &pool,
            &actor,
            strip,
            UpdateItemUnitInput {
                name: Some(" แผง (10 เม็ด) "),
                is_active: Some(false),
                ..unit_no_change()
            },
        )
        .await
        .unwrap();
        assert!(!find_unit(&pool, &actor, "แผง (10 เม็ด)").await.is_active);

        unit_service::delete(&pool, &actor, strip).await.unwrap();
        assert_eq!(unit_service::list(&pool, &actor).await.unwrap().len(), 2);
    }

    #[sqlx::test]
    async fn duplicate_names_conflict_case_insensitively(pool: PgPool) {
        let actor = admin(&pool).await;
        unit_service::create(&pool, &actor, unit("Box"))
            .await
            .unwrap();
        for dup in ["Box", "box", "BOX", " Box "] {
            let result = unit_service::create(&pool, &actor, unit(dup)).await;
            assert!(
                matches!(&result, Err(ItemUnitError::Conflict(m)) if m == "item unit name already taken"),
                "{dup:?} -> {result:?}"
            );
        }
        unit_service::create(&pool, &actor, unit("Tab"))
            .await
            .unwrap();
        let tab = find_unit(&pool, &actor, "Tab").await.public_id;
        assert!(matches!(
            unit_service::update(
                &pool,
                &actor,
                tab,
                UpdateItemUnitInput {
                    name: Some("box"),
                    ..unit_no_change()
                }
            )
            .await,
            Err(ItemUnitError::Conflict(_))
        ));
        unit_service::delete(&pool, &actor, tab).await.unwrap();
        unit_service::create(&pool, &actor, unit("Tab"))
            .await
            .unwrap();
    }

    #[sqlx::test]
    async fn blank_and_overlong_name_is_rejected(pool: PgPool) {
        let actor = admin(&pool).await;
        for name in ["", "  "] {
            assert!(matches!(
                unit_service::create(&pool, &actor, unit(name)).await,
                Err(ItemUnitError::Validation(_))
            ));
        }
        unit_service::create(&pool, &actor, unit(&"ก".repeat(100)))
            .await
            .unwrap();
        assert!(matches!(
            unit_service::create(&pool, &actor, unit(&"ก".repeat(101))).await,
            Err(ItemUnitError::Validation(_))
        ));
    }

    #[sqlx::test]
    async fn used_as_base_unit_or_conversion_cannot_be_deleted(pool: PgPool) {
        let actor = admin(&pool).await;
        let f = fixture(&pool, &actor).await;
        item_service::create(
            &pool,
            &actor,
            ItemInput {
                conversions: vec![conversion(f.boxed, 100)],
                ..item("พาราเซตามอล", f.drug, f.tablet)
            },
        )
        .await
        .unwrap();
        let paracetamol = find_item(&pool, &actor, "พาราเซตามอล").await.item.public_id;

        for used in [f.tablet, f.boxed] {
            let blocked = unit_service::delete(&pool, &actor, used).await;
            assert!(
                matches!(&blocked, Err(ItemUnitError::Conflict(m)) if m == "item unit is still used"),
                "{blocked:?}"
            );
        }
        // หน่วยที่ไม่มีรายการใช้ลบได้
        unit_service::delete(&pool, &actor, f.strip).await.unwrap();

        // เอาหน่วยแปลงออก → กล่องลบได้, เม็ดยังเป็นหน่วยฐานอยู่
        item_service::replace(
            &pool,
            &actor,
            paracetamol,
            item("พาราเซตามอล", f.drug, f.tablet),
        )
        .await
        .unwrap();
        unit_service::delete(&pool, &actor, f.boxed).await.unwrap();
        assert!(matches!(
            unit_service::delete(&pool, &actor, f.tablet).await,
            Err(ItemUnitError::Conflict(_))
        ));

        item_service::delete(&pool, &actor, paracetamol)
            .await
            .unwrap();
        unit_service::delete(&pool, &actor, f.tablet).await.unwrap();
    }

    #[sqlx::test]
    async fn permission_and_not_found(pool: PgPool) {
        let admin_actor = admin(&pool).await;
        unit_service::create(&pool, &admin_actor, unit("เม็ด"))
            .await
            .unwrap();
        let id = find_unit(&pool, &admin_actor, "เม็ด").await.public_id;

        let other = actor_with(&pool, all_except(Resource::ItemUnit)).await;
        assert!(matches!(
            unit_service::list(&pool, &other).await,
            Err(ItemUnitError::Forbidden)
        ));
        assert!(matches!(
            unit_service::create(&pool, &other, unit("X")).await,
            Err(ItemUnitError::Forbidden)
        ));
        assert!(matches!(
            unit_service::update(&pool, &other, id, unit_no_change()).await,
            Err(ItemUnitError::Forbidden)
        ));
        assert!(matches!(
            unit_service::delete(&pool, &other, id).await,
            Err(ItemUnitError::Forbidden)
        ));
        assert_eq!(
            unit_service::list(&pool, &admin_actor).await.unwrap().len(),
            1
        );

        let missing = Uuid::new_v4();
        assert!(matches!(
            unit_service::update(&pool, &admin_actor, missing, unit_no_change()).await,
            Err(ItemUnitError::NotFound)
        ));
        assert!(matches!(
            unit_service::delete(&pool, &admin_actor, missing).await,
            Err(ItemUnitError::NotFound)
        ));
    }

    #[sqlx::test]
    async fn menu_page_is_seeded(pool: PgPool) {
        let (label, group, permission) = seeded_page(&pool, "/settings/item-units").await;
        assert_eq!(label, "ตั้งค่าหน่วยนับ");
        assert_eq!(group.as_deref(), Some("การตั้งค่า"));
        assert_eq!(permission, "item_unit.view");
    }
}

// ---------------------------------------------------------------------------------------------
// item (+ หน่วยแปลง)
// ---------------------------------------------------------------------------------------------
mod item {
    use super::*;

    fn flags(view: &ItemView) -> (bool, bool, bool) {
        (
            view.item.track_lot,
            view.item.track_expiry,
            view.item.patient_linkable,
        )
    }

    #[sqlx::test]
    async fn create_copies_flags_from_category_unless_given(pool: PgPool) {
        let actor = admin(&pool).await;
        let f = fixture(&pool, &actor).await;
        category_service::create(&pool, &actor, category("เวชภัณฑ์"))
            .await
            .unwrap();
        let supplies = find_category(&pool, &actor, "เวชภัณฑ์").await.public_id;

        // ไม่ส่งธง = ค่าตั้งต้นของหมวด
        item_service::create(&pool, &actor, item("พาราเซตามอล", f.drug, f.tablet))
            .await
            .unwrap();
        item_service::create(&pool, &actor, item("สำลี", supplies, f.tablet))
            .await
            .unwrap();
        assert_eq!(
            flags(&find_item(&pool, &actor, "พาราเซตามอล").await),
            (true, true, true)
        );
        assert_eq!(
            flags(&find_item(&pool, &actor, "สำลี").await),
            (false, false, false)
        );

        // ส่งมาบางธง = ใช้ค่าที่ส่ง ที่เหลือจากหมวด
        item_service::create(
            &pool,
            &actor,
            ItemInput {
                track_expiry: Some(false),
                patient_linkable: Some(false),
                ..item("ถุงมือ", f.drug, f.tablet)
            },
        )
        .await
        .unwrap();
        assert_eq!(
            flags(&find_item(&pool, &actor, "ถุงมือ").await),
            (true, false, false)
        );

        // ติ๊กวันหมดอายุในหมวดที่ไม่ติดตามล็อต (ล็อตมาจากหมวด = false) → ไม่ผ่าน
        let message = item_validation(
            item_service::create(
                &pool,
                &actor,
                ItemInput {
                    track_expiry: Some(true),
                    ..item("หลอดเก็บเลือด", supplies, f.tablet)
                },
            )
            .await,
        );
        assert_eq!(message, "expiry tracking requires lot tracking");
    }

    #[sqlx::test]
    async fn replace_keeps_flags_and_does_not_recopy_from_new_category(pool: PgPool) {
        let actor = admin(&pool).await;
        let f = fixture(&pool, &actor).await;
        category_service::create(&pool, &actor, category("เวชภัณฑ์"))
            .await
            .unwrap();
        let supplies = find_category(&pool, &actor, "เวชภัณฑ์").await.public_id;
        item_service::create(&pool, &actor, item("พาราเซตามอล", f.drug, f.tablet))
            .await
            .unwrap();
        let id = find_item(&pool, &actor, "พาราเซตามอล").await.item.public_id;

        // ย้ายไปหมวดที่ธงตั้งต้นเป็น false ทั้งหมด โดยไม่ส่งธง → ธงเดิมคงอยู่
        item_service::replace(&pool, &actor, id, item("พาราเซตามอล", supplies, f.tablet))
            .await
            .unwrap();
        let moved = find_item(&pool, &actor, "พาราเซตามอล").await;
        assert_eq!(moved.item.category_name, "เวชภัณฑ์");
        assert_eq!(flags(&moved), (true, true, true));

        // ปิดล็อตอย่างเดียว (วันหมดอายุเดิม = true) → ไม่ผ่าน
        let message = item_validation(
            item_service::replace(
                &pool,
                &actor,
                id,
                ItemInput {
                    track_lot: Some(false),
                    ..item("พาราเซตามอล", supplies, f.tablet)
                },
            )
            .await,
        );
        assert_eq!(message, "expiry tracking requires lot tracking");

        item_service::replace(
            &pool,
            &actor,
            id,
            ItemInput {
                track_lot: Some(false),
                track_expiry: Some(false),
                patient_linkable: Some(false),
                reorder_point: Some(200),
                ..item("พาราเซตามอล", supplies, f.tablet)
            },
        )
        .await
        .unwrap();
        let replaced = find_item(&pool, &actor, "พาราเซตามอล").await;
        assert_eq!(flags(&replaced), (false, false, false));
        assert_eq!(replaced.item.reorder_point, Some(200));
    }

    #[sqlx::test]
    async fn conversions_are_validated(pool: PgPool) {
        let actor = admin(&pool).await;
        let f = fixture(&pool, &actor).await;
        let with = |conversions: Vec<ConversionInput>| ItemInput {
            conversions,
            ..item("พาราเซตามอล", f.drug, f.tablet)
        };

        let cases = [
            (
                vec![conversion(f.tablet, 10)],
                "conversion unit must differ from base unit",
            ),
            (
                vec![conversion(f.boxed, 100), conversion(f.boxed, 50)],
                "duplicate conversion unit",
            ),
            (
                vec![conversion(f.strip, 1)],
                "conversion factor must be greater than 1",
            ),
            (
                vec![conversion(f.strip, 0)],
                "conversion factor must be greater than 1",
            ),
            (
                vec![conversion(f.strip, -10)],
                "conversion factor must be greater than 1",
            ),
            (vec![conversion(Uuid::new_v4(), 10)], "unknown item unit"),
            (
                (0..11).map(|_| conversion(f.strip, 10)).collect(),
                "too many conversions",
            ),
        ];
        for (conversions, expected) in cases {
            let message =
                item_validation(item_service::create(&pool, &actor, with(conversions)).await);
            assert_eq!(message, expected);
        }
        assert!(item_service::list(&pool, &actor).await.unwrap().is_empty());

        // หน่วยแปลงเทียบหน่วยฐานเสมอ เรียงตาม factor
        item_service::create(
            &pool,
            &actor,
            with(vec![conversion(f.boxed, 100), conversion(f.strip, 10)]),
        )
        .await
        .unwrap();
        let view = find_item(&pool, &actor, "พาราเซตามอล").await;
        let listed: Vec<(&str, i32)> = view
            .conversions
            .iter()
            .map(|c| (c.unit_name.as_str(), c.factor))
            .collect();
        assert_eq!(listed, [("แผง", 10), ("กล่อง", 100)]);
        assert_eq!(view.item.base_unit_name, "เม็ด");
    }

    #[sqlx::test]
    async fn conversions_are_synced_as_a_set(pool: PgPool) {
        let actor = admin(&pool).await;
        let f = fixture(&pool, &actor).await;
        item_service::create(
            &pool,
            &actor,
            ItemInput {
                conversions: vec![conversion(f.strip, 10), conversion(f.boxed, 100)],
                ..item("พาราเซตามอล", f.drug, f.tablet)
            },
        )
        .await
        .unwrap();
        let id = find_item(&pool, &actor, "พาราเซตามอล").await.item.public_id;

        let rows = |pool: PgPool| async move {
            sqlx::query_as::<_, (String, i32, bool)>(
                "select u.name::text, cv.factor, cv.deleted_at is not null
                 from item_unit_conversion cv join item_unit u on u.id = cv.unit_id
                 order by cv.id",
            )
            .fetch_all(&pool)
            .await
            .unwrap()
        };
        let owned = |expected: &[(&str, i32, bool)]| -> Vec<(String, i32, bool)> {
            expected
                .iter()
                .map(|(name, factor, deleted)| (name.to_string(), *factor, *deleted))
                .collect()
        };

        // เอาแผงออก, กล่องเปลี่ยน factor → แผง soft delete, กล่องแก้แถวเดิม (ไม่สร้างแถวใหม่)
        item_service::replace(
            &pool,
            &actor,
            id,
            ItemInput {
                conversions: vec![conversion(f.boxed, 50)],
                ..item("พาราเซตามอล", f.drug, f.tablet)
            },
        )
        .await
        .unwrap();
        assert_eq!(
            rows(pool.clone()).await,
            owned(&[("แผง", 10, true), ("กล่อง", 50, false)])
        );

        // เพิ่มแผงกลับมา → แถวใหม่ (partial unique นับเฉพาะแถวที่ยังไม่ถูกลบ)
        item_service::replace(
            &pool,
            &actor,
            id,
            ItemInput {
                conversions: vec![conversion(f.strip, 12), conversion(f.boxed, 50)],
                ..item("พาราเซตามอล", f.drug, f.tablet)
            },
        )
        .await
        .unwrap();
        assert_eq!(
            rows(pool.clone()).await,
            owned(&[("แผง", 10, true), ("กล่อง", 50, false), ("แผง", 12, false)])
        );
        let listed: Vec<(String, i32)> = find_item(&pool, &actor, "พาราเซตามอล")
            .await
            .conversions
            .into_iter()
            .map(|c| (c.unit_name, c.factor))
            .collect();
        assert_eq!(listed, [("แผง".to_string(), 12), ("กล่อง".to_string(), 50)]);
    }

    #[sqlx::test]
    async fn inactive_category_and_units_cannot_be_newly_chosen_but_can_be_kept(pool: PgPool) {
        let actor = admin(&pool).await;
        let f = fixture(&pool, &actor).await;
        category_service::create(
            &pool,
            &actor,
            CreateItemCategoryInput {
                is_active: false,
                ..category("เลิกใช้")
            },
        )
        .await
        .unwrap();
        let retired = find_category(&pool, &actor, "เลิกใช้").await.public_id;
        unit_service::create(
            &pool,
            &actor,
            CreateItemUnitInput {
                is_active: false,
                ..unit("ขวด")
            },
        )
        .await
        .unwrap();
        let bottle = find_unit(&pool, &actor, "ขวด").await.public_id;

        assert_eq!(
            item_validation(
                item_service::create(&pool, &actor, item("A", retired, f.tablet)).await
            ),
            "item category is inactive"
        );
        assert_eq!(
            item_validation(item_service::create(&pool, &actor, item("A", f.drug, bottle)).await),
            "item unit is inactive"
        );
        assert_eq!(
            item_validation(
                item_service::create(
                    &pool,
                    &actor,
                    ItemInput {
                        conversions: vec![conversion(bottle, 10)],
                        ..item("A", f.drug, f.tablet)
                    },
                )
                .await
            ),
            "item unit is inactive"
        );
        assert_eq!(
            item_validation(
                item_service::create(&pool, &actor, item("A", Uuid::new_v4(), f.tablet)).await
            ),
            "unknown item category"
        );
        assert_eq!(
            item_validation(
                item_service::create(&pool, &actor, item("A", f.drug, Uuid::new_v4())).await
            ),
            "unknown item unit"
        );

        // ผูกอยู่แล้วแล้วค่อยปิดใช้งาน → แก้ไขรายการโดยคงค่าเดิมได้
        let full = || ItemInput {
            conversions: vec![conversion(f.boxed, 100)],
            ..item("พาราเซตามอล", f.drug, f.tablet)
        };
        item_service::create(&pool, &actor, full()).await.unwrap();
        let id = find_item(&pool, &actor, "พาราเซตามอล").await.item.public_id;
        category_service::update(
            &pool,
            &actor,
            f.drug,
            UpdateItemCategoryInput {
                is_active: Some(false),
                ..category_no_change()
            },
        )
        .await
        .unwrap();
        for unit_id in [f.tablet, f.boxed] {
            unit_service::update(
                &pool,
                &actor,
                unit_id,
                UpdateItemUnitInput {
                    is_active: Some(false),
                    ..unit_no_change()
                },
            )
            .await
            .unwrap();
        }
        item_service::replace(
            &pool,
            &actor,
            id,
            ItemInput {
                reorder_point: Some(50),
                ..full()
            },
        )
        .await
        .unwrap();
        assert_eq!(
            find_item(&pool, &actor, "พาราเซตามอล")
                .await
                .item
                .reorder_point,
            Some(50)
        );

        // แต่เพิ่มหน่วย inactive ที่ไม่เคยผูกไม่ได้
        assert_eq!(
            item_validation(
                item_service::replace(
                    &pool,
                    &actor,
                    id,
                    ItemInput {
                        conversions: vec![conversion(f.boxed, 100), conversion(bottle, 10)],
                        ..full()
                    },
                )
                .await
            ),
            "item unit is inactive"
        );
    }

    #[sqlx::test]
    async fn name_and_reorder_point_rules(pool: PgPool) {
        let actor = admin(&pool).await;
        let f = fixture(&pool, &actor).await;
        let base = |name| item(name, f.drug, f.tablet);

        let overlong = "ก".repeat(101);
        for name in ["", "  ", overlong.as_str()] {
            assert!(matches!(
                item_service::create(&pool, &actor, base(name)).await,
                Err(ItemError::Validation(_))
            ));
        }
        assert_eq!(
            item_validation(
                item_service::create(
                    &pool,
                    &actor,
                    ItemInput {
                        reorder_point: Some(-1),
                        ..base("A")
                    },
                )
                .await
            ),
            "reorder point must not be negative"
        );
        // 0 = เตือนเมื่อของหมด
        item_service::create(
            &pool,
            &actor,
            ItemInput {
                reorder_point: Some(0),
                ..base("Paracetamol")
            },
        )
        .await
        .unwrap();
        assert_eq!(
            find_item(&pool, &actor, "Paracetamol")
                .await
                .item
                .reorder_point,
            Some(0)
        );

        // ชื่อซ้ำไม่ได้ทั้งระบบ (ไม่สนตัวพิมพ์ ไม่ใช่แค่ในหมวด)
        category_service::create(&pool, &actor, category("เวชภัณฑ์"))
            .await
            .unwrap();
        let supplies = find_category(&pool, &actor, "เวชภัณฑ์").await.public_id;
        for dup in ["Paracetamol", "paracetamol", " PARACETAMOL "] {
            let result = item_service::create(&pool, &actor, item(dup, supplies, f.tablet)).await;
            assert!(
                matches!(&result, Err(ItemError::Conflict(m)) if m == "item name already taken"),
                "{dup:?} -> {result:?}"
            );
        }
        let id = find_item(&pool, &actor, "Paracetamol").await.item.public_id;
        item_service::delete(&pool, &actor, id).await.unwrap();
        item_service::create(&pool, &actor, base("Paracetamol"))
            .await
            .unwrap();
    }

    #[sqlx::test]
    async fn delete_soft_deletes_conversions(pool: PgPool) {
        let actor = admin(&pool).await;
        let f = fixture(&pool, &actor).await;
        item_service::create(
            &pool,
            &actor,
            ItemInput {
                conversions: vec![conversion(f.strip, 10), conversion(f.boxed, 100)],
                ..item("พาราเซตามอล", f.drug, f.tablet)
            },
        )
        .await
        .unwrap();
        let id = find_item(&pool, &actor, "พาราเซตามอล").await.item.public_id;
        item_service::delete(&pool, &actor, id).await.unwrap();

        assert!(item_service::list(&pool, &actor).await.unwrap().is_empty());
        let (total, live): (i64, i64) = sqlx::query_as(
            "select count(*), count(*) filter (where deleted_at is null) from item_unit_conversion",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!((total, live), (2, 0));
        assert!(matches!(
            item_service::delete(&pool, &actor, id).await,
            Err(ItemError::NotFound)
        ));
    }

    #[sqlx::test]
    async fn options_list_only_active_categories_and_units(pool: PgPool) {
        let actor = admin(&pool).await;
        let f = fixture(&pool, &actor).await;
        category_service::create(
            &pool,
            &actor,
            CreateItemCategoryInput {
                is_active: false,
                ..category("เลิกใช้")
            },
        )
        .await
        .unwrap();
        category_service::create(&pool, &actor, category("ลบแล้ว"))
            .await
            .unwrap();
        let deleted = find_category(&pool, &actor, "ลบแล้ว").await.public_id;
        category_service::delete(&pool, &actor, deleted)
            .await
            .unwrap();
        unit_service::update(
            &pool,
            &actor,
            f.strip,
            UpdateItemUnitInput {
                is_active: Some(false),
                ..unit_no_change()
            },
        )
        .await
        .unwrap();

        let options = item_service::options(&pool, &actor).await.unwrap();
        let categories: Vec<(&str, bool, bool, bool)> = options
            .categories
            .iter()
            .map(|c| {
                (
                    c.name.as_str(),
                    c.default_track_lot,
                    c.default_track_expiry,
                    c.default_patient_linkable,
                )
            })
            .collect();
        assert_eq!(categories, [("ยา", true, true, true)]);
        let mut units: Vec<&str> = options.units.iter().map(|u| u.name.as_str()).collect();
        units.sort();
        let mut expected = ["เม็ด", "กล่อง"];
        expected.sort();
        assert_eq!(units, expected);
    }

    #[sqlx::test]
    async fn permission_and_not_found(pool: PgPool) {
        let admin_actor = admin(&pool).await;
        let f = fixture(&pool, &admin_actor).await;
        item_service::create(&pool, &admin_actor, item("พาราเซตามอล", f.drug, f.tablet))
            .await
            .unwrap();
        let id = find_item(&pool, &admin_actor, "พาราเซตามอล")
            .await
            .item
            .public_id;

        // สิทธิ์หน้าหมวด/หน่วย (และหน้าอื่นทั้งหมด) ไม่ให้จัดการรายการ
        let other = actor_with(&pool, all_except(Resource::Item)).await;
        assert!(matches!(
            item_service::list(&pool, &other).await,
            Err(ItemError::Forbidden)
        ));
        assert!(matches!(
            item_service::options(&pool, &other).await,
            Err(ItemError::Forbidden)
        ));
        assert!(matches!(
            item_service::create(&pool, &other, item("X", f.drug, f.tablet)).await,
            Err(ItemError::Forbidden)
        ));
        assert!(matches!(
            item_service::replace(&pool, &other, id, item("X", f.drug, f.tablet)).await,
            Err(ItemError::Forbidden)
        ));
        assert!(matches!(
            item_service::delete(&pool, &other, id).await,
            Err(ItemError::Forbidden)
        ));

        // ดูรายการอย่างเดียว: เห็นรายการและตัวเลือกของฟอร์ม (ไม่ต้องมีสิทธิ์หน้าหมวด/หน่วย) แต่เพิ่มไม่ได้
        let viewer = actor_with(&pool, vec![perm("item.view")]).await;
        assert_eq!(item_service::list(&pool, &viewer).await.unwrap().len(), 1);
        assert_eq!(
            item_service::options(&pool, &viewer)
                .await
                .unwrap()
                .categories
                .len(),
            1
        );
        assert!(matches!(
            item_service::create(&pool, &viewer, item("X", f.drug, f.tablet)).await,
            Err(ItemError::Forbidden)
        ));

        let missing = Uuid::new_v4();
        assert!(matches!(
            item_service::replace(&pool, &admin_actor, missing, item("X", f.drug, f.tablet)).await,
            Err(ItemError::NotFound)
        ));
        assert!(matches!(
            item_service::delete(&pool, &admin_actor, missing).await,
            Err(ItemError::NotFound)
        ));
    }

    #[sqlx::test]
    async fn menu_page_is_seeded(pool: PgPool) {
        let (label, group, permission) = seeded_page(&pool, "/settings/items").await;
        assert_eq!(label, "ตั้งค่ารายการคลัง");
        assert_eq!(group.as_deref(), Some("การตั้งค่า"));
        assert_eq!(permission, "item.view");
    }
}
