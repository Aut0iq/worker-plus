use argon2::password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::Argon2;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::HeaderMap;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{Role, User};
use crate::state::AppState;

const TOKEN_TTL_DAYS: i64 = 7;

pub fn hash_password(password: &str) -> AppResult<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(AppError::internal)
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
        .unwrap_or(false)
}

/// argon2 is CPU-heavy (tens of ms): run it off the async worker threads so a
/// burst of logins/imports does not stall every other request.
pub async fn hash_password_blocking(password: String) -> AppResult<String> {
    tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(AppError::internal)?
}

/// `verify_password` off the async threads; `None` hash = unknown login (timing-safe dummy check).
pub async fn verify_password_blocking(password: String, hash: Option<String>) -> bool {
    tokio::task::spawn_blocking(move || match hash {
        Some(h) => verify_password(&password, &h),
        None => {
            verify_dummy(&password);
            false
        }
    })
    .await
    .unwrap_or(false)
}

/// Burn the same argon2 time as a real check when the login does not exist,
/// so response timing does not reveal which logins are valid.
pub fn verify_dummy(password: &str) {
    static DUMMY: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    if let Some(h) = DUMMY.get_or_init(|| hash_password("dummy-password-for-timing").ok()) {
        let _ = verify_password(password, h);
    }
}

pub const MIN_PASSWORD_CHARS: usize = 6;

/// Password length in characters (not bytes: "пароль" is 6 chars / 12 bytes).
pub fn password_too_short(password: &str) -> bool {
    password.chars().count() < MIN_PASSWORD_CHARS
}

/// Logins are case-insensitive and stored trimmed + lowercase.
pub fn normalize_login(login: &str) -> String {
    login.trim().to_lowercase()
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    iat: i64,
    exp: i64,
}

pub fn encode_token(user_id: Uuid, secret: &str, now: DateTime<Utc>) -> AppResult<String> {
    let claims = Claims {
        sub: user_id.to_string(),
        iat: now.timestamp(),
        exp: (now + Duration::days(TOKEN_TTL_DAYS)).timestamp(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()).map_err(AppError::internal)?,
    )
    .map_err(AppError::internal)
}

/// Returns (user id, issued-at unix seconds).
fn decode_token(token: &str, secret: &str) -> AppResult<(Uuid, i64)> {
    let key = DecodingKey::from_secret(secret.as_bytes())
        .map_err(|_| AppError::unauthorized("Недействительный или истёкший токен"))?;
    let data = decode::<Claims>(token, &key, &Validation::default())
        .map_err(|_| AppError::unauthorized("Недействительный или истёкший токен"))?;
    let id = Uuid::parse_str(&data.claims.sub)
        .map_err(|_| AppError::unauthorized("Недействительный или истёкший токен"))?;
    Ok((id, data.claims.iat))
}

fn bearer_token(headers: &HeaderMap) -> AppResult<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::unauthorized("Требуется авторизация"))
}

/// Authenticated user, freshly loaded from the DB on every request.
pub struct AuthUser {
    pub user: User,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = bearer_token(&parts.headers)?;
        let (user_id, issued_at) = decode_token(token, &state.config.jwt_secret)?;
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| AppError::unauthorized("Пользователь не найден"))?;
        if !user.is_active {
            return Err(AppError::unauthorized("Учётная запись деактивирована"));
        }
        // tokens issued before the last password change/reset are revoked
        if let Some(changed) = user.password_changed_at {
            if issued_at < changed.timestamp() {
                return Err(AppError::unauthorized(
                    "Пароль был изменён, войдите снова",
                ));
            }
        }
        Ok(AuthUser { user })
    }
}

macro_rules! role_extractor {
    ($name:ident, $role:pat, $msg:expr) => {
        pub struct $name(pub AuthUser);

        impl FromRequestParts<AppState> for $name {
            type Rejection = AppError;

            async fn from_request_parts(
                parts: &mut Parts,
                state: &AppState,
            ) -> Result<Self, Self::Rejection> {
                let auth = AuthUser::from_request_parts(parts, state).await?;
                if !matches!(auth.user.role, $role) {
                    return Err(AppError::forbidden($msg));
                }
                Ok($name(auth))
            }
        }

        impl std::ops::Deref for $name {
            type Target = AuthUser;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

role_extractor!(AdminUser, Role::Admin, "Доступно только администратору");
role_extractor!(StudentUser, Role::Student, "Доступно только студенту");
role_extractor!(KomendantUser, Role::Komendant, "Доступно только коменданту");
role_extractor!(TeacherUser, Role::Teacher | Role::Admin, "Доступно только преподавателю или администратору");

pub fn random_password() -> String {
    use rand::RngExt;
    const ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::rng();
    (0..10)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}
