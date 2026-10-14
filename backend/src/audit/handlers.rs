//! แปลงผลลัพธ์และ error เป็น HTTP (ห้ามแตะ SQL — เรียก service เท่านั้น) การตรวจสิทธิ์/ช่วงวันอยู่ที่ service

use axum::extract::{Query, State};
use axum::Json;
use uuid::Uuid;

use crate::audit::dto::{
    ActivitiesResponse, ActivityResponse, AuthEventResponse, AuthEventsResponse, DateRangeQuery,
    EventUserResponse,
};
use crate::audit::repository::{ActivityRow, AuthEventRow};
use crate::audit::service;
use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::state::AppState;

fn event_user(
    id: Option<Uuid>,
    username: Option<String>,
    display_name: Option<String>,
) -> Option<EventUserResponse> {
    match (id, username, display_name) {
        (Some(id), Some(username), Some(display_name)) => Some(EventUserResponse {
            id,
            username,
            display_name,
        }),
        _ => None,
    }
}

fn to_response(row: AuthEventRow) -> AuthEventResponse {
    AuthEventResponse {
        occurred_at: row.occurred_at,
        event: row.action,
        user: event_user(row.user_public_id, row.username, row.display_name),
        ip: row.ip,
        user_agent: row.user_agent,
    }
}

#[tracing::instrument(skip(state, current_user))]
pub async fn auth_events(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<DateRangeQuery>,
) -> Result<Json<AuthEventsResponse>, AppError> {
    let result =
        service::list_auth_events(&state.pool, &current_user, query.from, query.to).await?;
    Ok(Json(AuthEventsResponse {
        events: result.events.into_iter().map(to_response).collect(),
        truncated: result.truncated,
    }))
}

fn to_activity_response(row: ActivityRow) -> ActivityResponse {
    ActivityResponse {
        occurred_at: row.occurred_at,
        action: row.action,
        user: event_user(row.user_public_id, row.username, row.display_name),
        target_type: row.target_type,
        target_label: row.target_label,
        export_document: row.export_document,
        export_format: row.export_format,
        details: row.details,
    }
}

#[tracing::instrument(skip(state, current_user))]
pub async fn activities(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<DateRangeQuery>,
) -> Result<Json<ActivitiesResponse>, AppError> {
    let result = service::list_activities(&state.pool, &current_user, query.from, query.to).await?;
    Ok(Json(ActivitiesResponse {
        events: result
            .events
            .into_iter()
            .map(to_activity_response)
            .collect(),
        truncated: result.truncated,
    }))
}
