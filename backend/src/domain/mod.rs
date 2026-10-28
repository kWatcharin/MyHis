//! ชนิดข้อมูลกลาง: UserId, RoleId, PageId, Permission, User (docs/rust-guidelines.md ข้อ 4)
//! newtype แทนชนิดพื้นฐานสำหรับ id, enum แทน string ที่ส่งไปมา
//!
//! `XId` คือ pk แบบ integer ที่ใช้ภายใน backend (join/FK) เท่านั้น — ไม่ออกนอกระบบ ส่วน id ที่
//! API/frontend เห็นคือ `uuid` ของแถวนั้น (ชนิด `Uuid` ธรรมดา เรียก `public_id` ในโค้ด)
//! ชนิดในไฟล์นี้ไม่ผูกกับรูปแบบ JSON ของ API โดยตรง — auth::dto เป็นคนแปลงเป็น wire format
//! (แยก DTO ออกจากชนิดโดเมน เหมือนที่แยกจาก struct ของตาราง)

use uuid::Uuid;

pub mod changes;
pub mod item_tracking;
pub mod patient_crypto;
pub mod username;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct UserId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct RoleId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct PageId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct BudgetYearId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct DiseaseId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct PrenameId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct NationalityId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct PatientGroupId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct LabId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct PatientId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct LabVisitGroupId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct LabVisitId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct ItemCategoryId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct ItemUnitId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct ItemId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct StockLotId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct StockReceiptId(pub i32);

/// เพจ/ข้อมูลที่ตั้งสิทธิ์แยกได้ — ส่วนหน้าของคีย์ permission เช่น `patient` ใน `patient.view`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resource {
    /// ทะเบียนผู้ป่วย (ข้อมูลระบุตัวคนไข้) — หน้า "ผู้ป่วย"
    Patient,
    /// นัดหมายแล็ป — หน้าที่แสดงชื่อ/HN คนไข้ต้องมี `patient.view` ด้วย
    LabVisit,
    User,
    Role,
    Page,
    BudgetYear,
    Disease,
    Prename,
    Nationality,
    PatientGroup,
    Lab,
    /// ตารางเวร (ยังไม่มีหน้า)
    Shift,
    /// audit log (ยังไม่มีหน้า)
    Audit,
    /// หน้า SQL (`/data/script`) — รัน SQL อ่านอย่างเดียวด้วย role ฐานข้อมูลแยก (sql_console/) ถอดรหัสข้อมูลผู้ป่วย
    /// ในผลลัพธ์ได้เมื่อมี `patient.view` ด้วย
    Sql,
    /// master คลัง: หมวด, หน่วยนับ, รายการ (+ หน่วยแปลงของรายการ ใช้สิทธิ์ของ `item`)
    ItemCategory,
    ItemUnit,
    Item,
    /// ใบรับเข้าคลัง (รวมยอดยกมา) — `delete` = ยกเลิกใบรับ (ลง movement กลับรายการ ไม่ลบแถว)
    StockReceipt,
}

/// การกระทำต่อเพจ: ดู / เพิ่ม / แก้ไข / ลบ / ส่งออกเอกสาร — ตรวจแยกกันทุกตัว (แก้ไขไม่ implies ดู)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    View,
    Create,
    Update,
    Delete,
    /// ส่งออก PDF/Excel — เอกสารสร้างฝั่งหน้าเว็บ backend ไม่มี endpoint ให้ตรวจ สิทธิ์นี้คุมที่หน้าเว็บ
    Export,
}

use Action::{Create, Delete, Export, Update, View};

/// permission ทั้งหมดที่มีจริง (resource, action, คีย์ที่เก็บใน DB) — ปิดตาย ผูกกับ authorization check จริงในโค้ด
/// สร้างผ่าน UI ไม่ได้ คีย์ต้องตรงกับ migration `…_page_action_permissions.sql` เป๊ะ (docs/auth-design.md ข้อ 10)
const PERMISSIONS: &[(Resource, Action, &str)] = &[
    (Resource::Patient, View, "patient.view"),
    (Resource::Patient, Create, "patient.create"),
    (Resource::Patient, Update, "patient.update"),
    (Resource::Patient, Delete, "patient.delete"),
    (Resource::LabVisit, View, "lab_visit.view"),
    (Resource::LabVisit, Create, "lab_visit.create"),
    (Resource::LabVisit, Update, "lab_visit.update"),
    (Resource::LabVisit, Delete, "lab_visit.delete"),
    (Resource::LabVisit, Export, "lab_visit.export"),
    (Resource::User, View, "user.view"),
    (Resource::User, Create, "user.create"),
    (Resource::User, Update, "user.update"),
    (Resource::Role, View, "role.view"),
    (Resource::Role, Create, "role.create"),
    (Resource::Role, Update, "role.update"),
    (Resource::Role, Delete, "role.delete"),
    (Resource::Page, View, "page.view"),
    (Resource::Page, Create, "page.create"),
    (Resource::Page, Update, "page.update"),
    (Resource::Page, Delete, "page.delete"),
    (Resource::BudgetYear, View, "budget_year.view"),
    (Resource::BudgetYear, Create, "budget_year.create"),
    (Resource::BudgetYear, Update, "budget_year.update"),
    (Resource::BudgetYear, Delete, "budget_year.delete"),
    (Resource::Disease, View, "disease.view"),
    (Resource::Disease, Create, "disease.create"),
    (Resource::Disease, Update, "disease.update"),
    (Resource::Disease, Delete, "disease.delete"),
    (Resource::Prename, View, "prename.view"),
    (Resource::Prename, Create, "prename.create"),
    (Resource::Prename, Update, "prename.update"),
    (Resource::Prename, Delete, "prename.delete"),
    (Resource::Nationality, View, "nationality.view"),
    (Resource::Nationality, Create, "nationality.create"),
    (Resource::Nationality, Update, "nationality.update"),
    (Resource::Nationality, Delete, "nationality.delete"),
    (Resource::PatientGroup, View, "patient_group.view"),
    (Resource::PatientGroup, Create, "patient_group.create"),
    (Resource::PatientGroup, Update, "patient_group.update"),
    (Resource::PatientGroup, Delete, "patient_group.delete"),
    (Resource::Lab, View, "lab.view"),
    (Resource::Lab, Create, "lab.create"),
    (Resource::Lab, Update, "lab.update"),
    (Resource::Lab, Delete, "lab.delete"),
    (Resource::Shift, View, "shift.view"),
    (Resource::Shift, Update, "shift.update"),
    (Resource::Audit, View, "audit.view"),
    // เพิ่มหลัง migration page_action_permissions — admin ได้จาก migration sql_console_admin_permission (role อื่นติ๊กเอง)
    (Resource::Sql, View, "sql.view"),
    // master คลัง — admin ได้จาก migration inventory_masters (role อื่นติ๊กเอง)
    (Resource::ItemCategory, View, "item_category.view"),
    (Resource::ItemCategory, Create, "item_category.create"),
    (Resource::ItemCategory, Update, "item_category.update"),
    (Resource::ItemCategory, Delete, "item_category.delete"),
    (Resource::ItemUnit, View, "item_unit.view"),
    (Resource::ItemUnit, Create, "item_unit.create"),
    (Resource::ItemUnit, Update, "item_unit.update"),
    (Resource::ItemUnit, Delete, "item_unit.delete"),
    (Resource::Item, View, "item.view"),
    (Resource::Item, Create, "item.create"),
    (Resource::Item, Update, "item.update"),
    (Resource::Item, Delete, "item.delete"),
    // ธุรกรรมคลัง — admin ได้จาก migration stock_receipts (role อื่นติ๊กเอง)
    (Resource::StockReceipt, View, "stock_receipt.view"),
    (Resource::StockReceipt, Create, "stock_receipt.create"),
    (Resource::StockReceipt, Update, "stock_receipt.update"),
    (Resource::StockReceipt, Delete, "stock_receipt.delete"),
];

/// permission หนึ่งตัว = แถวหนึ่งใน `PERMISSIONS` (เก็บเป็นลำดับแถว) — สร้างได้จากตารางเท่านั้น (`of`, `parse`,
/// `all`) จึงไม่มีคู่ resource/action ที่ไม่มีจริง เช่น user.delete
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Permission(u8);

impl std::fmt::Debug for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Permission {
    fn row(self) -> &'static (Resource, Action, &'static str) {
        // ลำดับแถวมาจาก PERMISSIONS เอง (ตัวสร้างทุกตัวอยู่ในไฟล์นี้) จึงอยู่ในช่วงเสมอ
        &PERMISSIONS[usize::from(self.0)]
    }

    fn from_index(index: usize) -> Option<Self> {
        u8::try_from(index).ok().map(Permission)
    }

    /// permission ของ resource + action — None ถ้าคู่นี้ไม่มีจริง (เช่น ผู้ใช้งานไม่มี "ลบ")
    pub fn of(resource: Resource, action: Action) -> Option<Self> {
        PERMISSIONS
            .iter()
            .position(|(r, a, _)| *r == resource && *a == action)
            .and_then(Self::from_index)
    }

    /// ทุก permission ตามลำดับในตาราง
    pub fn all() -> impl Iterator<Item = Self> {
        (0..PERMISSIONS.len()).filter_map(Self::from_index)
    }

    pub fn resource(self) -> Resource {
        self.row().0
    }

    pub fn action(self) -> Action {
        self.row().1
    }

    /// ค่าที่เก็บใน role_permissions.permission / page_permissions.permission — ต้องตรงกับ migration เป๊ะ
    pub fn as_str(self) -> &'static str {
        self.row().2
    }

    /// parse ค่าที่มาจาก DB — ถ้าไม่รู้จักถือว่าข้อมูลเสีย ให้ error ไม่ใช่ข้าม
    /// (docs/auth-design.md ข้อ 4)
    pub fn parse(value: &str) -> Option<Self> {
        PERMISSIONS
            .iter()
            .position(|(_, _, key)| *key == value)
            .and_then(Self::from_index)
    }
}

/// ผู้ใช้ระดับโดเมน — ไม่มี password_hash ไม่ผูกกับรูปแบบ JSON โดยตรง
#[derive(Debug, Clone)]
pub struct User {
    pub id: UserId,
    pub public_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub must_change_password: bool,
}
