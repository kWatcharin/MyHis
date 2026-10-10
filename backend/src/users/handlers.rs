//! ตรวจรูปแบบ input, แปลงผลลัพธ์และ error เป็น HTTP (ห้ามแตะ SQL — เรียก service เท่านั้น)
//! การตรวจสิทธิ์จริงอยู่ที่ service (docs/auth-design.md ข้อ 10) — ที่นี่แค่ extract CurrentUser
//! แล้วส่งต่อ

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use uuid::Uuid;
use validator::Validate;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::state::AppState;
use crate::users::dto::*;
use crate::users::repository::{PageWithPermissions, RoleWithPermissions, UserWithRoles};
use crate::users::service::{self, UpdatePageInput};

fn user_to_summary(u: UserWithRoles) -> UserSummary {
    UserSummary {
        id: u.public_id,
        username: u.username,
        display_name: u.display_name,
        is_active: u.is_active,
        must_change_password: u.must_change_password,
        locked_until: u.locked_until,
        prename_id: u.prename_public_id,
        prename_name: u.prename_name,
        first_name: u.first_name,
        last_name: u.last_name,
        position: u.position,
        registered_at: u.registered_at,
        registration_expires_at: u.registration_expires_at,
        roles: u
            .roles
            .into_iter()
            .map(|r| RoleRef {
                id: r.public_id,
                name: r.name,
            })
            .collect(),
        created_at: u.created_at,
    }
}

fn role_to_response(r: RoleWithPermissions) -> RoleResponse {
    RoleResponse {
        id: r.public_id,
        name: r.name,
        description: r.description,
        is_system: r.is_system,
        permissions: r.permissions.iter().map(|p| p.as_str()).collect(),
    }
}

fn page_to_response(p: PageWithPermissions) -> PageResponse {
    PageResponse {
        id: p.public_id,
        label: p.label,
        icon: p.icon,
        route: p.route,
        sort_order: p.sort_order,
        permissions: p.permissions.iter().map(|perm| perm.as_str()).collect(),
        group_label: p.group_label,
    }
}

fn validation(err: validator::ValidationErrors) -> AppError {
    AppError::Validation(err.to_string())
}

// --- users ---

#[tracing::instrument(skip(state, current_user))]
pub async fn list_users(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<Vec<UserSummary>>, AppError> {
    let users = service::list_users(&state.pool, &current_user).await?;
    Ok(Json(users.into_iter().map(user_to_summary).collect()))
}

fn link_response(link: service::IssuedLink) -> RegistrationLinkResponse {
    RegistrationLinkResponse {
        registration_token: link.token,
        registration_expires_at: link.expires_at,
    }
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn create_user(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<CreateUserResponse>), AppError> {
    payload.validate().map_err(validation)?;

    let now = Utc::now();
    let created = service::create_user(
        &state.pool,
        &current_user,
        &payload.username,
        &payload.display_name,
        payload.role_id,
        now,
    )
    .await?;

    let user = UserSummary {
        id: created.public_id,
        username: created.username,
        display_name: created.display_name,
        is_active: true,
        must_change_password: false,
        locked_until: None,
        prename_id: None,
        prename_name: None,
        first_name: None,
        last_name: None,
        position: None,
        registered_at: None,
        registration_expires_at: Some(created.link.expires_at),
        roles: vec![RoleRef {
            id: created.role.public_id,
            name: created.role.name,
        }],
        created_at: now,
    };

    Ok((
        StatusCode::CREATED,
        Json(CreateUserResponse {
            user,
            link: link_response(created.link),
        }),
    ))
}

#[tracing::instrument(skip_all)]
pub async fn replace_user(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<ReplaceUserRequest>,
) -> Result<StatusCode, AppError> {
    service::replace_user(
        &state.pool,
        &current_user,
        id,
        service::ReplaceUserInput {
            display_name: &payload.display_name,
            prename_public_id: payload.prename_id,
            first_name: payload.first_name.as_deref(),
            last_name: payload.last_name.as_deref(),
            position: payload.position.as_deref(),
            role_public_ids: &payload.role_ids,
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn update_user(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<StatusCode, AppError> {
    payload.validate().map_err(validation)?;
    service::update_user(
        &state.pool,
        &current_user,
        id,
        payload.display_name.as_deref(),
        payload.is_active,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip(state, current_user))]
pub async fn reset_password(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<RegistrationLinkResponse>, AppError> {
    let link = service::reset_password(&state.pool, &current_user, id, Utc::now()).await?;
    Ok(Json(link_response(link)))
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn replace_user_roles(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<ReplaceUserRolesRequest>,
) -> Result<StatusCode, AppError> {
    service::replace_user_roles(&state.pool, &current_user, id, &payload.role_ids).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- roles ---

#[tracing::instrument(skip(state, current_user))]
pub async fn list_roles(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<Vec<RoleResponse>>, AppError> {
    let roles = service::list_roles(&state.pool, &current_user).await?;
    Ok(Json(roles.into_iter().map(role_to_response).collect()))
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn create_role(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(payload): Json<CreateRoleRequest>,
) -> Result<StatusCode, AppError> {
    payload.validate().map_err(validation)?;
    service::create_role(
        &state.pool,
        &current_user,
        &payload.name,
        payload.description.as_deref(),
        &payload.permissions,
    )
    .await?;
    Ok(StatusCode::CREATED)
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn update_role(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRoleRequest>,
) -> Result<StatusCode, AppError> {
    payload.validate().map_err(validation)?;
    service::update_role(
        &state.pool,
        &current_user,
        id,
        payload.name.as_deref(),
        payload.description.as_deref(),
        payload.permissions.as_deref(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip(state, current_user))]
pub async fn delete_role(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    service::delete_role(&state.pool, &current_user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- pages ---

#[tracing::instrument(skip(state, current_user))]
pub async fn list_pages(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<Vec<PageResponse>>, AppError> {
    let pages = service::list_pages(&state.pool, &current_user).await?;
    Ok(Json(pages.into_iter().map(page_to_response).collect()))
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn create_page(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(payload): Json<CreatePageRequest>,
) -> Result<StatusCode, AppError> {
    payload.validate().map_err(validation)?;
    service::create_page(
        &state.pool,
        &current_user,
        service::CreatePageInput {
            label: &payload.label,
            icon: payload.icon.as_deref(),
            route: &payload.route,
            sort_order: payload.sort_order.unwrap_or(0),
            permissions: &payload.permissions,
            group_label: payload.group_label.as_deref(),
        },
    )
    .await?;
    Ok(StatusCode::CREATED)
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn update_page(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdatePageRequest>,
) -> Result<StatusCode, AppError> {
    payload.validate().map_err(validation)?;
    service::update_page(
        &state.pool,
        &current_user,
        id,
        UpdatePageInput {
            label: payload.label.as_deref(),
            // icon: None = ไม่แตะ, Some(v) = ตั้งค่าใหม่ — DTO เป็น Option<String> ชั้นเดียว
            // (serde ทำให้ "ไม่ส่ง field" กับ "ส่ง null" กลายเป็น None เหมือนกัน) เลยยังไม่รองรับ
            // การ "ตั้ง icon กลับเป็น null" ผ่าน endpoint นี้โดยตรง — ถือเป็นข้อจำกัดเล็ก ๆ ที่ยอมรับได้
            icon: payload.icon.as_deref().map(Some),
            route: payload.route.as_deref(),
            sort_order: payload.sort_order,
            permissions: payload.permissions.as_deref(),
            group_label: payload.group_label.as_deref().map(Some),
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip(state, current_user))]
pub async fn delete_page(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    service::delete_page(&state.pool, &current_user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- menu ---

#[tracing::instrument(skip(state, current_user))]
pub async fn menu(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<Vec<MenuItemResponse>>, AppError> {
    let items = service::menu_for(&state.pool, &current_user).await?;
    Ok(Json(
        items
            .into_iter()
            .map(|i| MenuItemResponse {
                id: i.public_id,
                label: i.label,
                icon: i.icon,
                route: i.route,
                group_label: i.group_label,
            })
            .collect(),
    ))
}
