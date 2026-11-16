pub mod admin;
pub mod auth;
pub mod komendant;
pub mod me;
pub mod rewards;
pub mod student;
pub mod teacher;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::Json;
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;

use crate::state::AppState;

pub async fn health() -> Json<serde_json::Value> {
    Json(json!({"status": "ok"}))
}

/// Public client-facing settings: display timezone and session limit.
pub async fn public_config(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<serde_json::Value> {
    Json(json!({
        "timezone": state.config.timezone.name(),
        "max_session_hours": state.config.max_session_hours,
    }))
}

/// Client IP extracted from ConnectInfo (if present).
pub struct ClientIp(pub Option<String>);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let ip = parts
            .extensions
            .get::<axum::extract::ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip().to_string());
        Ok(ClientIp(ip))
    }
}
