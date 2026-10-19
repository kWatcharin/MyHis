//! แปลง HTTP ↔ service ของหน้า SQL — ตรรกะ (สิทธิ์, ถอดรหัส, audit) อยู่ที่ service
//! `#[instrument(skip_all)]` เสมอ: body มีตัว SQL และ response มีข้อมูลผู้ป่วยที่ถอดรหัสแล้ว

use axum::extract::State;
use axum::Json;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::sql_console::dto::{
    ColumnResponse, QueryRequest, QueryResponse, SchemaColumnResponse, SchemaResponse,
    SchemaTableResponse,
};
use crate::sql_console::service;
use crate::state::AppState;

#[tracing::instrument(skip_all)]
pub async fn schema(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<SchemaResponse>, AppError> {
    let console = state.sql_console.as_deref();
    let tables = service::schema(console, &current_user).await?;
    let statement_timeout_ms = console
        .map(|console| u64::try_from(console.statement_timeout().as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default();
    Ok(Json(SchemaResponse {
        tables: tables
            .into_iter()
            .map(|table| SchemaTableResponse {
                name: table.name,
                columns: table
                    .columns
                    .into_iter()
                    .map(|column| SchemaColumnResponse {
                        name: column.name,
                        type_name: column.type_name,
                        encrypted: column.encrypted,
                    })
                    .collect(),
            })
            .collect(),
        max_rows: service::MAX_ROWS,
        statement_timeout_ms,
    }))
}

#[tracing::instrument(skip_all)]
pub async fn query(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(payload): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, AppError> {
    let result = service::run(
        &state.pool,
        state.sql_console.as_deref(),
        &state.config.crypto,
        &current_user,
        &payload.sql,
        payload.decrypt,
    )
    .await?;
    Ok(Json(QueryResponse {
        row_count: result.rows.len(),
        columns: result
            .columns
            .into_iter()
            .map(|column| ColumnResponse {
                name: column.name,
                type_name: column.type_name,
                supported: column.supported,
                encrypted: column.encrypted,
                decrypted: column.decrypted,
            })
            .collect(),
        rows: result.rows,
        truncated: result.truncated,
        duration_ms: result.duration_ms,
        decrypt_errors: result.decrypt_errors,
    }))
}
