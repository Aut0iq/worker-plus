use axum::routing::post;
use axum::{Json, Router};
use axum::extract::State;
use serde_json::json;

use crate::auth::{encode_token, verify_password};
use crate::dto::{LoginRequest, LoginResponse};
use crate::error::{AppError, AppResult};
use crate::models::Role;
use crate::rows::{UserRow, USER_SELECT};
use crate::state::{AppState, AuditRecord};

use super::ClientIp;

pub fn router() -> Router<AppState> {
    Router::new().route("/auth/login", post(login))
}

async fn login(
    State(state): State<AppState>,
    ip: ClientIp,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<LoginResponse>> {
    let row = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(format!("{USER_SELECT} WHERE u.login = $1")))
        .bind(&req.login)
        .fetch_optional(&state.pool)
        .await?;

    let ok = match &row {
        Some(r) => verify_password(&req.password, &r.u.password_hash),
        None => false,
    };
    let active = row.as_ref().map(|r| r.u.is_active).unwrap_or(false);

    if !ok || !active {
        state
            .audit(
                &state.pool,
                AuditRecord::new("login_failure")
                    .details(json!({"login": req.login}))
                    .ip(ip.0.as_deref()),
            )
            .await?;
        return Err(AppError::unauthorized("Неверный логин или пароль"));
    }

    let UserRow { u: user, group_name } = row.expect("checked above");
    let token = encode_token(user.id, &state.config.jwt_secret, state.clock.now())?;

    state
        .audit(
            &state.pool,
            AuditRecord::new("login_success")
                .actor(user.id, user.role.as_str())
                .entity("user", user.id)
                .ip(ip.0.as_deref()),
        )
        .await?;
    if user.role == Role::Komendant {
        state
            .komendant_log(&state.pool, user.id, "login", None, None, None)
            .await?;
    }

    Ok(Json(LoginResponse {
        token,
        user: crate::dto::UserDto::of(user, group_name),
    }))
}
