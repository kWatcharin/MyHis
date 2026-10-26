//! AppError ตัวกลางตัวเดียวของทั้งแอป + การแปลงเป็น HTTP response (docs/rust-guidelines.md ข้อ 3)
//! กฎ: รายละเอียดทางเทคนิคไปที่ log เท่านั้น response ที่ส่งออกไปบอกเท่าที่ผู้ใช้ต้องรู้

use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;
use tracing::error;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("invalid credentials")]
    InvalidCredentials,

    /// login ไม่ผ่าน — ข้อความเดียวกับ InvalidCredentials แต่ body มี `remainingAttempts` ให้หน้า login แสดง
    #[error("invalid credentials")]
    LoginRejected { remaining_attempts: i32 },

    #[error("account temporarily locked")]
    AccountLocked { until: DateTime<Utc> },

    /// login ไม่สำเร็จจาก IP นี้ครบเพดานของหน้าต่าง (ไม่ขึ้นกับชื่อผู้ใช้) → 429 พร้อม Retry-After
    #[error("too many attempts")]
    TooManyAttempts { until: DateTime<Utc> },

    #[error("not authenticated")]
    Unauthenticated,

    #[error("forbidden")]
    Forbidden,

    /// 403 ที่บอกเหตุผล (เช่น ห้ามให้สิทธิ์เกินตัวเอง) — หน้าเว็บแปลข้อความนี้เป็นไทย
    #[error("{0}")]
    ForbiddenWithReason(String),

    #[error("not found")]
    NotFound,

    #[error("{0}")]
    Conflict(String),

    #[error("{0}")]
    Validation(String),

    /// บริการภายนอกที่ต้องใช้ไม่ตอบ (เช่น Cloudflare Turnstile ตอน login) → 503 ลองใหม่ภายหลัง
    #[error("{0}")]
    Unavailable(String),

    /// หน้า SQL: Postgres ปฏิเสธคำสั่งของผู้ใช้ → 400 พร้อมข้อความของ Postgres (ผู้ใช้พิมพ์ SQL เองจึงต้องเห็น)
    /// ไม่ log ข้อความ (อาจมีค่าจากคำสั่ง) — `QueryFailure` เขียน Debug ให้ redact ไว้แล้ว
    #[error("query failed")]
    QueryFailed(crate::sql_console::QueryFailure),

    /// หน้า SQL: คำสั่งที่ไม่รัน (แก้ข้อมูล / หลายคำสั่ง) → 400 พร้อม `reason` + `keyword` ให้หน้าเว็บเตือนเป็นภาษาไทย
    #[error("statement not allowed")]
    StatementNotAllowed(crate::sql_console::Blocked),

    #[error("database error")]
    Database(#[from] sqlx::Error),

    #[error("data integrity error: {0}")]
    DataIntegrity(String),
}

// re-export ผ่าน crate::auth::{RepositoryError, AuthError} (ดู auth/mod.rs) ไม่ reach เข้า
// auth::repository/auth::service ตรง ๆ — mod.rs ของแต่ละฟีเจอร์เป็นหน้าร้านที่เดียวที่ระบุว่า
// เปิดอะไรออกไป
impl From<crate::auth::RepositoryError> for AppError {
    fn from(err: crate::auth::RepositoryError) -> Self {
        match err {
            crate::auth::RepositoryError::Database(e) => AppError::Database(e),
            crate::auth::RepositoryError::UnknownPermission(p) => {
                AppError::DataIntegrity(format!("unknown permission stored in database: {p}"))
            }
        }
    }
}

impl From<crate::auth::AuthError> for AppError {
    fn from(err: crate::auth::AuthError) -> Self {
        use crate::auth::AuthError;
        match err {
            AuthError::InvalidCredentials => AppError::InvalidCredentials,
            AuthError::LoginRejected { remaining_attempts } => {
                AppError::LoginRejected { remaining_attempts }
            }
            AuthError::AccountLocked { until } => AppError::AccountLocked { until },
            AuthError::TooManyAttempts { until } => AppError::TooManyAttempts { until },
            err @ (AuthError::PasswordUnchanged | AuthError::CaptchaFailed) => {
                AppError::Validation(err.to_string())
            }
            err @ (AuthError::CaptchaUnavailable | AuthError::Busy) => {
                AppError::Unavailable(err.to_string())
            }
            AuthError::Repository(e) => e.into(),
            AuthError::MissingSeedRole(name) => {
                AppError::DataIntegrity(format!("required seed data missing: {name}"))
            }
            AuthError::Password(e) => {
                AppError::DataIntegrity(format!("password hashing failed: {e}"))
            }
        }
    }
}

impl From<crate::users::UsersError> for AppError {
    fn from(err: crate::users::UsersError) -> Self {
        use crate::users::UsersError;
        match err {
            UsersError::Forbidden => AppError::Forbidden,
            UsersError::BeyondOwnPermissions(reason) => {
                AppError::ForbiddenWithReason(reason.to_string())
            }
            UsersError::Conflict(msg) => AppError::Conflict(msg),
            UsersError::Validation(msg) => AppError::Validation(msg),
            UsersError::UserNotFound | UsersError::RoleNotFound | UsersError::PageNotFound => {
                AppError::NotFound
            }
            UsersError::Repository(e) => e.into(),
            UsersError::Password(e) => {
                AppError::DataIntegrity(format!("password hashing failed: {e}"))
            }
        }
    }
}

impl From<crate::users::RepositoryError> for AppError {
    fn from(err: crate::users::RepositoryError) -> Self {
        use crate::users::RepositoryError;
        match err {
            RepositoryError::Database(e) => AppError::Database(e),
            RepositoryError::UnknownPermission(p) => {
                AppError::DataIntegrity(format!("unknown permission stored in database: {p}"))
            }
        }
    }
}

impl From<crate::budget_year::BudgetYearError> for AppError {
    fn from(err: crate::budget_year::BudgetYearError) -> Self {
        use crate::budget_year::BudgetYearError;
        match err {
            BudgetYearError::Forbidden => AppError::Forbidden,
            BudgetYearError::Conflict(msg) => AppError::Conflict(msg),
            BudgetYearError::Validation(msg) => AppError::Validation(msg),
            BudgetYearError::NotFound => AppError::NotFound,
            BudgetYearError::Repository(e) => e.into(),
        }
    }
}

impl From<crate::budget_year::RepositoryError> for AppError {
    fn from(err: crate::budget_year::RepositoryError) -> Self {
        use crate::budget_year::RepositoryError;
        match err {
            RepositoryError::Database(e) => AppError::Database(e),
        }
    }
}

impl From<crate::audit::AuditError> for AppError {
    fn from(err: crate::audit::AuditError) -> Self {
        use crate::audit::AuditError;
        match err {
            AuditError::Forbidden => AppError::Forbidden,
            AuditError::Validation(msg) => AppError::Validation(msg),
            AuditError::Repository(e) => e.into(),
        }
    }
}

impl From<crate::audit::RepositoryError> for AppError {
    fn from(err: crate::audit::RepositoryError) -> Self {
        use crate::audit::RepositoryError;
        match err {
            RepositoryError::Database(e) => AppError::Database(e),
        }
    }
}

impl From<crate::disease::DiseaseError> for AppError {
    fn from(err: crate::disease::DiseaseError) -> Self {
        use crate::disease::DiseaseError;
        match err {
            DiseaseError::Forbidden => AppError::Forbidden,
            DiseaseError::Conflict(msg) => AppError::Conflict(msg),
            DiseaseError::Validation(msg) => AppError::Validation(msg),
            DiseaseError::NotFound => AppError::NotFound,
            DiseaseError::Repository(e) => e.into(),
        }
    }
}

impl From<crate::disease::RepositoryError> for AppError {
    fn from(err: crate::disease::RepositoryError) -> Self {
        use crate::disease::RepositoryError;
        match err {
            RepositoryError::Database(e) => AppError::Database(e),
        }
    }
}

impl From<crate::prename::PrenameError> for AppError {
    fn from(err: crate::prename::PrenameError) -> Self {
        use crate::prename::PrenameError;
        match err {
            PrenameError::Forbidden => AppError::Forbidden,
            PrenameError::Conflict(msg) => AppError::Conflict(msg),
            PrenameError::Validation(msg) => AppError::Validation(msg),
            PrenameError::NotFound => AppError::NotFound,
            PrenameError::Repository(e) => e.into(),
        }
    }
}

impl From<crate::prename::RepositoryError> for AppError {
    fn from(err: crate::prename::RepositoryError) -> Self {
        use crate::prename::RepositoryError;
        match err {
            RepositoryError::Database(e) => AppError::Database(e),
        }
    }
}

impl From<crate::nationality::NationalityError> for AppError {
    fn from(err: crate::nationality::NationalityError) -> Self {
        use crate::nationality::NationalityError;
        match err {
            NationalityError::Forbidden => AppError::Forbidden,
            NationalityError::Conflict(msg) => AppError::Conflict(msg),
            NationalityError::Validation(msg) => AppError::Validation(msg),
            NationalityError::NotFound => AppError::NotFound,
            NationalityError::Repository(e) => e.into(),
        }
    }
}

impl From<crate::nationality::RepositoryError> for AppError {
    fn from(err: crate::nationality::RepositoryError) -> Self {
        use crate::nationality::RepositoryError;
        match err {
            RepositoryError::Database(e) => AppError::Database(e),
        }
    }
}

impl From<crate::patient_group::PatientGroupError> for AppError {
    fn from(err: crate::patient_group::PatientGroupError) -> Self {
        use crate::patient_group::PatientGroupError;
        match err {
            PatientGroupError::Forbidden => AppError::Forbidden,
            PatientGroupError::Conflict(msg) => AppError::Conflict(msg),
            PatientGroupError::Validation(msg) => AppError::Validation(msg),
            PatientGroupError::NotFound => AppError::NotFound,
            PatientGroupError::Repository(e) => e.into(),
        }
    }
}

impl From<crate::patient_group::RepositoryError> for AppError {
    fn from(err: crate::patient_group::RepositoryError) -> Self {
        use crate::patient_group::RepositoryError;
        match err {
            RepositoryError::Database(e) => AppError::Database(e),
        }
    }
}

impl From<crate::lab::LabError> for AppError {
    fn from(err: crate::lab::LabError) -> Self {
        use crate::lab::LabError;
        match err {
            LabError::Forbidden => AppError::Forbidden,
            LabError::Conflict(msg) => AppError::Conflict(msg),
            LabError::Validation(msg) => AppError::Validation(msg),
            LabError::NotFound => AppError::NotFound,
            LabError::Repository(e) => e.into(),
        }
    }
}

impl From<crate::lab::RepositoryError> for AppError {
    fn from(err: crate::lab::RepositoryError) -> Self {
        use crate::lab::RepositoryError;
        match err {
            RepositoryError::Database(e) => AppError::Database(e),
        }
    }
}

impl From<crate::patient::PatientError> for AppError {
    fn from(err: crate::patient::PatientError) -> Self {
        use crate::patient::PatientError;
        match err {
            PatientError::Forbidden => AppError::Forbidden,
            PatientError::Conflict(msg) => AppError::Conflict(msg),
            PatientError::Validation(msg) => AppError::Validation(msg),
            PatientError::NotFound => AppError::NotFound,
            // ไม่แนบรายละเอียดของ CryptoError (ข้อมูลระบุตัวคนไข้/คีย์) — แค่ข้อความกลาง ๆ ใน log
            PatientError::Crypto(_) => {
                AppError::DataIntegrity("patient data could not be decrypted".to_string())
            }
            PatientError::Repository(e) => e.into(),
        }
    }
}

impl From<crate::patient::RepositoryError> for AppError {
    fn from(err: crate::patient::RepositoryError) -> Self {
        use crate::patient::RepositoryError;
        match err {
            RepositoryError::Database(e) => AppError::Database(e),
        }
    }
}

impl From<crate::lab_visit::LabVisitError> for AppError {
    fn from(err: crate::lab_visit::LabVisitError) -> Self {
        use crate::lab_visit::LabVisitError;
        match err {
            LabVisitError::Forbidden => AppError::Forbidden,
            LabVisitError::Conflict(msg) => AppError::Conflict(msg),
            LabVisitError::Validation(msg) => AppError::Validation(msg),
            LabVisitError::GroupNotFound | LabVisitError::VisitNotFound => AppError::NotFound,
            LabVisitError::Repository(e) => e.into(),
        }
    }
}

impl From<crate::lab_visit::RepositoryError> for AppError {
    fn from(err: crate::lab_visit::RepositoryError) -> Self {
        use crate::lab_visit::RepositoryError;
        match err {
            RepositoryError::Database(e) => AppError::Database(e),
        }
    }
}

impl From<crate::registration::RegistrationError> for AppError {
    fn from(err: crate::registration::RegistrationError) -> Self {
        use crate::registration::RegistrationError;
        match err {
            // ข้อความกลางข้อความเดียวไม่ว่าลิงก์ใช้ไม่ได้เพราะอะไร
            err @ RegistrationError::InvalidLink => AppError::Validation(err.to_string()),
            RegistrationError::Validation(msg) => AppError::Validation(msg),
            RegistrationError::Repository(e) => e.into(),
            RegistrationError::Password(e) => {
                AppError::DataIntegrity(format!("password hashing failed: {e}"))
            }
        }
    }
}

impl From<crate::registration::RepositoryError> for AppError {
    fn from(err: crate::registration::RepositoryError) -> Self {
        use crate::registration::RepositoryError;
        match err {
            RepositoryError::Database(e) => AppError::Database(e),
        }
    }
}

impl From<crate::sql_console::SqlConsoleError> for AppError {
    fn from(err: crate::sql_console::SqlConsoleError) -> Self {
        use crate::sql_console::SqlConsoleError;
        match err {
            SqlConsoleError::Forbidden => AppError::Forbidden,
            err @ SqlConsoleError::NotConfigured => AppError::Unavailable(err.to_string()),
            SqlConsoleError::Validation(msg) => AppError::Validation(msg),
            SqlConsoleError::QueryFailed(failure) => AppError::QueryFailed(failure),
            SqlConsoleError::Blocked(blocked) => AppError::StatementNotAllowed(blocked),
            SqlConsoleError::Repository(e) => e.into(),
        }
    }
}

impl From<crate::sql_console::RepositoryError> for AppError {
    fn from(err: crate::sql_console::RepositoryError) -> Self {
        use crate::sql_console::RepositoryError;
        match err {
            RepositoryError::Database(e) => AppError::Database(e),
            err @ RepositoryError::ConnectTimeout => AppError::Unavailable(err.to_string()),
            // ปกติ service แปลงเป็น QueryFailed (400 พร้อมข้อความ) แล้วตั้งแต่ to_failure; ทางนี้กันกรณีที่หลุดมา
            RepositoryError::ResultTooLarge => AppError::Validation("result too large".to_string()),
        }
    }
}

impl AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::InvalidCredentials | AppError::LoginRejected { .. } => {
                StatusCode::UNAUTHORIZED
            }
            AppError::AccountLocked { .. } | AppError::TooManyAttempts { .. } => {
                StatusCode::TOO_MANY_REQUESTS
            }
            AppError::Unauthenticated => StatusCode::UNAUTHORIZED,
            AppError::Forbidden | AppError::ForbiddenWithReason(_) => StatusCode::FORBIDDEN,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Validation(_)
            | AppError::QueryFailed(_)
            | AppError::StatementNotAllowed(_) => StatusCode::BAD_REQUEST,
            AppError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            AppError::Database(_) | AppError::DataIntegrity(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn is_internal(&self) -> bool {
        matches!(self, AppError::Database(_) | AppError::DataIntegrity(_))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    error: String,
    /// เฉพาะ login ไม่ผ่าน: เหลือโอกาสอีกกี่ครั้งก่อนล็อก
    #[serde(skip_serializing_if = "Option::is_none")]
    remaining_attempts: Option<i32>,
    /// เฉพาะหน้า SQL: ข้อความ, SQLSTATE และตำแหน่ง (ตัวอักษรที่เท่าไรในคำสั่งที่พิมพ์) จาก Postgres
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<usize>,
    /// เฉพาะหน้า SQL ที่ไม่รันคำสั่ง: `read_only` / `multiple_statements` และชื่อคำสั่ง (เช่น INSERT)
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keyword: Option<String>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if self.is_internal() {
            error!(error = %self, "unhandled internal error");
        }

        let status = self.status_code();
        let message = if self.is_internal() {
            "internal error".to_string()
        } else {
            self.to_string()
        };

        let remaining_attempts = match &self {
            AppError::LoginRejected { remaining_attempts } => Some(*remaining_attempts),
            _ => None,
        };
        let (detail, code, position) = match &self {
            AppError::QueryFailed(failure) => (
                Some(failure.message.clone()),
                failure.code.clone(),
                failure.position,
            ),
            _ => (None, None, None),
        };
        let (reason, keyword) = match &self {
            AppError::StatementNotAllowed(blocked) => (
                Some(blocked.reason()),
                blocked.keyword().map(str::to_string),
            ),
            _ => (None, None),
        };
        let mut response = (
            status,
            Json(ErrorBody {
                error: message,
                remaining_attempts,
                message: detail,
                code,
                position,
                reason,
                keyword,
            }),
        )
            .into_response();

        if let AppError::AccountLocked { until } | AppError::TooManyAttempts { until } = &self {
            let seconds = (*until - Utc::now()).num_seconds().max(0);
            if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
                response.headers_mut().insert("Retry-After", value);
            }
        }

        response
    }
}
