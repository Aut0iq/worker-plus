use axum::routing::post;
use axum::{Json, Router};
use axum::extract::State;
use chrono::Duration;
use serde_json::json;
use std::sync::MutexGuard;

use crate::auth::{encode_token, verify_password};
use crate::dto::{LoginRequest, LoginResponse};
use crate::error::{AppError, AppResult};
use crate::models::Role;
use crate::rows::{UserRow, USER_SELECT};
use crate::state::{AppState, AuditRecord, LoginAttemptMap};

use super::ClientIp;

const MAX_FAILED_LOGINS: u32 = 10;
const LOGIN_WINDOW: Duration = Duration::minutes(15);
const MAX_TRACKED_KEYS: usize = 10_000;

pub fn router() -> Router<AppState> {
    Router::new().route("/auth/login", post(login))
}

fn lock_attempts(state: &AppState) -> MutexGuard<'_, LoginAttemptMap> {
    state.login_attempts.lock().unwrap_or_else(|e| e.into_inner())
}

async fn login(
    State(state): State<AppState>,
    ip: ClientIp,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<LoginResponse>> {
    let key = format!(
        "{}|{}",
        ip.0.as_deref().unwrap_or("-"),
        req.login.trim().to_lowercase()
    );
    let now = state.clock.now();

    let blocked_minutes = {
        let mut attempts = lock_attempts(&state);
        if attempts.len() > MAX_TRACKED_KEYS {
            attempts.retain(|_, (_, start)| now - *start < LOGIN_WINDOW);
        }
        match attempts.get(&key) {
            Some((count, start)) if *count >= MAX_FAILED_LOGINS && now - *start < LOGIN_WINDOW => {
                let remaining = LOGIN_WINDOW - (now - *start);
                Some(((remaining.num_seconds() + 59) / 60).max(1))
            }
            _ => None,
        }
    };
    if let Some(mins) = blocked_minutes {
        state
            .audit(
                &state.pool,
                AuditRecord::new("login_blocked")
                    .details(json!({"login": req.login}))
                    .ip(ip.0.as_deref()),
            )
            .await?;
        return Err(AppError::too_many_requests(
            "too_many_attempts",
            format!("Слишком много попыток входа. Попробуйте через {mins} мин."),
        ));
    }

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
        {
            let mut attempts = lock_attempts(&state);
            let entry = attempts.entry(key.clone()).or_insert((0, now));
            if now - entry.1 >= LOGIN_WINDOW {
                *entry = (0, now);
            }
            entry.0 += 1;
        }
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

    lock_attempts(&state).remove(&key);

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
