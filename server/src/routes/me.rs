use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::auth::{hash_password, verify_password, AuthUser};
use crate::dto::{
    ChangePasswordRequest, NotificationDto, PageQuery, Paginated, PushTokenRequest, UserDto,
};
use crate::error::{AppError, AppResult};
use crate::models::Notification;
use crate::rows::{UserRow, USER_SELECT};
use crate::state::{AppState, AuditRecord};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me", get(me))
        .route("/me/password", post(change_password))
        .route("/me/push-token", post(add_push_token))
        .route("/me/push-token", delete(remove_push_token))
        .route("/notifications", get(notifications))
        .route("/notifications/read-all", post(notifications_read_all))
        .route("/notifications/{id}/read", post(notification_read))
}

async fn me(State(state): State<AppState>, auth: AuthUser) -> AppResult<Json<UserDto>> {
    let row = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(format!("{USER_SELECT} WHERE u.id = $1")))
        .bind(auth.user.id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.into()))
}

async fn change_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<ChangePasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if req.new_password.len() < 6 {
        return Err(AppError::bad_request(
            "weak_password",
            "Новый пароль должен содержать не менее 6 символов",
        ));
    }
    if !verify_password(&req.old_password, &auth.user.password_hash) {
        return Err(AppError::bad_request(
            "wrong_password",
            "Неверный текущий пароль",
        ));
    }
    let hash = hash_password(&req.new_password)?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(&hash)
        .bind(auth.user.id)
        .execute(&mut *tx)
        .await?;
    state
        .audit(
            &mut *tx,
            AuditRecord::new("password_change")
                .actor(auth.user.id, auth.user.role.as_str())
                .entity("user", auth.user.id),
        )
        .await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn add_push_token(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<PushTokenRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if req.token.trim().is_empty() {
        return Err(AppError::bad_request("invalid_token", "Пустой push-токен"));
    }
    sqlx::query(
        "INSERT INTO push_tokens (user_id, token) VALUES ($1, $2)
         ON CONFLICT (token) DO UPDATE SET user_id = EXCLUDED.user_id",
    )
    .bind(auth.user.id)
    .bind(req.token.trim())
    .execute(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn remove_push_token(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<PushTokenRequest>,
) -> AppResult<Json<serde_json::Value>> {
    sqlx::query("DELETE FROM push_tokens WHERE user_id = $1 AND token = $2")
        .bind(auth.user.id)
        .bind(req.token.trim())
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn notifications(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(p): Query<PageQuery>,
) -> AppResult<Json<Paginated<NotificationDto>>> {
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notifications WHERE user_id = $1",
    )
    .bind(auth.user.id)
    .fetch_one(&state.pool)
    .await?;
    let items = sqlx::query_as::<_, Notification>(
        "SELECT * FROM notifications WHERE user_id = $1
         ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(auth.user.id)
    .bind(p.per_page())
    .bind(p.offset())
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(Paginated {
        items: items
            .into_iter()
            .map(|n| NotificationDto {
                id: n.id,
                kind: n.kind,
                title: n.title,
                body: n.body,
                data: n.data,
                read_at: n.read_at,
                created_at: n.created_at,
            })
            .collect(),
        total,
        page: p.page(),
        per_page: p.per_page(),
    }))
}

async fn notification_read(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    sqlx::query(
        "UPDATE notifications SET read_at = now() WHERE id = $1 AND user_id = $2 AND read_at IS NULL",
    )
    .bind(id)
    .bind(auth.user.id)
    .execute(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn notifications_read_all(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    sqlx::query("UPDATE notifications SET read_at = now() WHERE user_id = $1 AND read_at IS NULL")
        .bind(auth.user.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({"ok": true})))
}
