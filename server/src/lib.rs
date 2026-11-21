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

use axum::extract::{DefaultBodyLimit, Request};
use axum::http::{header, HeaderValue};
use axum::middleware::{self, Next};
use axum::response::Response;
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
        Some(dir) if dir.join("index.html").exists() => app
            .fallback_service(
                // `fallback`, not `not_found_service`: SPA routes must answer 200, not 404
                ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html"))),
            )
            .layer(middleware::from_fn(static_cache_headers)),
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

    app.layer(middleware::from_fn(security_headers))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

/// Admin SPA caching: `index.html` (and every SPA route that falls back to it)
/// must be revalidated on each load, otherwise browsers keep a stale bundle after
/// a redeploy; hashed `/assets/*` files never change and can be cached forever.
async fn static_cache_headers(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_owned();
    let mut resp = next.run(req).await;
    if path == "/api" || path.starts_with("/api/") {
        return resp;
    }
    let value = if path.starts_with("/assets/") && resp.status().is_success() {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    resp
}

/// Defence-in-depth headers for every response (API and admin SPA).
/// HSTS is left to the HTTPS proxy (Caddy): over plain HTTP it is meaningless.
async fn security_headers(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    resp
}
