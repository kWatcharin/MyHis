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

    #[error("account temporarily locked")]
    AccountLocked { until: DateTime<Utc> },

    #[error("not authenticated")]
    Unauthenticated,

    #[error("forbidden")]
    Forbidden,

    #[error("not found")]
    NotFound,

    #[error("{0}")]
    Conflict(String),

    #[error("{0}")]
    Validation(String),

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
            AuthError::AccountLocked { until } => AppError::AccountLocked { until },
            err @ AuthError::PasswordUnchanged => AppError::Validation(err.to_string()),
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

impl AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            AppError::AccountLocked { .. } => StatusCode::TOO_MANY_REQUESTS,
            AppError::Unauthenticated => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::Database(_) | AppError::DataIntegrity(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn is_internal(&self) -> bool {
        matches!(self, AppError::Database(_) | AppError::DataIntegrity(_))
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
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

        let mut response = (status, Json(ErrorBody { error: message })).into_response();

        if let AppError::AccountLocked { until } = &self {
            let seconds = (*until - Utc::now()).num_seconds().max(0);
            if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
                response.headers_mut().insert("Retry-After", value);
            }
        }

        response
    }
}
