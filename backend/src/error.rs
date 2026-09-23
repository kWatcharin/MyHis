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
