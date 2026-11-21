use axum::routing::post;
use axum::{Json, Router};
use axum::extract::State;
use chrono::Duration;
use serde_json::json;
use std::sync::MutexGuard;

use crate::auth::{encode_token, normalize_login, verify_password_blocking};
use crate::dto::{LoginRequest, LoginResponse};
use crate::error::{AppError, AppResult};
use crate::models::Role;
use crate::rows::{UserRow, USER_SELECT};
use crate::state::{AppState, AuditRecord, LoginAttemptMap};

use super::ClientIp;

const MAX_FAILED_LOGINS: u32 = 10;
/// Per-IP cap across all logins: stops one IP spraying a few passwords on every account.
/// Generous on purpose: a whole class may log in from one school NAT address.
const MAX_FAILED_PER_IP: u32 = 50;
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
    let login_norm = normalize_login(&req.login);
    let ip_s = ip.0.as_deref().unwrap_or("-");
    let key = format!("{ip_s}|{login_norm}");
    let ip_key = format!("{ip_s}|*");
    let now = state.clock.now();
    // audit only a bounded prefix: people sometimes type the password into the login field
    let login_logged: String = req.login.chars().take(64).collect();

    let blocked_minutes = {
        let mut attempts = lock_attempts(&state);
        if attempts.len() > MAX_TRACKED_KEYS {
            attempts.retain(|_, (_, start)| now - *start < LOGIN_WINDOW);
        }
        let blocked = |k: &str, max: u32| match attempts.get(k) {
            Some((count, start)) if *count >= max && now - *start < LOGIN_WINDOW => {
                let remaining = LOGIN_WINDOW - (now - *start);
                Some(((remaining.num_seconds() + 59) / 60).max(1))
            }
            _ => None,
        };
        blocked(&key, MAX_FAILED_LOGINS).or_else(|| blocked(&ip_key, MAX_FAILED_PER_IP))
    };
    if let Some(mins) = blocked_minutes {
        state
            .audit(
                &state.pool,
                AuditRecord::new("login_blocked")
                    .details(json!({"login": login_logged}))
                    .ip(ip.0.as_deref()),
            )
            .await?;
        return Err(AppError::too_many_requests(
            "too_many_attempts",
            format!("Слишком много попыток входа. Попробуйте через {mins} мин."),
        ));
    }

    let row = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(format!("{USER_SELECT} WHERE u.login = $1")))
        .bind(&login_norm)
        .fetch_optional(&state.pool)
        .await?;

    let ok = verify_password_blocking(
        req.password.clone(),
        row.as_ref().map(|r| r.u.password_hash.clone()),
    )
    .await;
    let active = row.as_ref().map(|r| r.u.is_active).unwrap_or(false);

    if !ok || !active {
        {
            let mut attempts = lock_attempts(&state);
            for k in [key.clone(), ip_key] {
                let entry = attempts.entry(k).or_insert((0, now));
                if now - entry.1 >= LOGIN_WINDOW {
                    *entry = (0, now);
                }
                entry.0 += 1;
            }
        }
        state
            .audit(
                &state.pool,
                AuditRecord::new("login_failure")
                    .details(json!({"login": login_logged}))
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
