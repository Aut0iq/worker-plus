pub mod auth;
pub mod clock;
pub mod config;
pub mod domain;
pub mod dto;
pub mod error;
pub mod import;
pub mod jobs;
pub mod models;
pub mod qr;
pub mod rows;
pub mod routes;
pub mod state;
pub mod summaries;

use axum::extract::DefaultBodyLimit;
use axum::http::HeaderValue;
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

pub use state::AppState;

pub fn build_app(state: AppState) -> Router {
    let api = Router::new()
        .merge(routes::auth::router())
        .merge(routes::me::router())
        .merge(routes::student::router())
        .merge(routes::komendant::router())
        .merge(routes::teacher::router())
        .merge(routes::rewards::router())
        .merge(routes::admin::router())
        .route("/health", axum::routing::get(routes::health))
        .route("/config", axum::routing::get(routes::public_config))
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024));

    let app = Router::new().nest("/api", api);

    let app = match &state.config.static_dir {
        Some(dir) if dir.join("index.html").exists() => app.fallback_service(
            ServeDir::new(dir).not_found_service(ServeFile::new(dir.join("index.html"))),
        ),
        _ => app,
    };

    let cors = if state.config.cors_origins.trim() == "*" {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        let origins: Vec<HeaderValue> = state
            .config
            .cors_origins
            .split(',')
            .filter_map(|o| o.trim().parse().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(Any)
            .allow_headers(Any)
    };

    app.layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
