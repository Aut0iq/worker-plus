use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

use prila_server::auth::hash_password;
use prila_server::clock::MockClock;
use prila_server::config::Config;
use prila_server::AppState;

pub struct TestApp {
    pub app: Router,
    pub state: AppState,
    pub clock: Arc<MockClock>,
    #[allow(dead_code)]
    pub tmp: TempDir,
}

pub const JWT_SECRET: &str = "test-jwt-secret";
pub const QR_SECRET: &str = "test-qr-secret";
pub const PASSWORD: &str = "password1";

pub fn test_config(tmp: &TempDir) -> Config {
    Config {
        database_url: String::new(),
        jwt_secret: JWT_SECRET.into(),
        qr_secret: QR_SECRET.into(),
        timezone: "Asia/Vladivostok".parse().unwrap(),
        photo_dir: tmp.path().join("photos"),
        log_dir: tmp.path().join("logs"),
        static_dir: None,
        bind_addr: "127.0.0.1:0".into(),
        admin_login: "admin".into(),
        admin_password: "adminpass".into(),
        // deliberately unreachable: push must never break requests
        expo_push_enabled: true,
        expo_push_url: "http://127.0.0.1:1/unreachable".into(),
        cors_origins: "*".into(),
        max_session_hours: 4,
    }
}

pub async fn make_app(pool: PgPool, now: DateTime<Utc>) -> TestApp {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("prila_server=error")
        .try_init();
    let tmp = TempDir::new().unwrap();
    let clock = MockClock::new(now);
    let clock_dyn: Arc<dyn prila_server::clock::Clock> = clock.clone();
    let state = AppState::new_test(
        pool,
        test_config(&tmp),
        clock_dyn,
        Some(&tmp.path().join("logs")),
    );
    let app = prila_server::build_app(state.clone());
    TestApp {
        app,
        state,
        clock,
        tmp,
    }
}

pub async fn create_group(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO groups (name) VALUES ($1) RETURNING id")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

pub async fn create_user(
    pool: &PgPool,
    role: &str,
    login: &str,
    name: &str,
    group: Option<Uuid>,
) -> Uuid {
    let hash = hash_password(PASSWORD).unwrap();
    sqlx::query_scalar(
        "INSERT INTO users (role, login, password_hash, full_name, group_id)
         VALUES ($1,$2,$3,$4,$5) RETURNING id",
    )
    .bind(role)
    .bind(login)
    .bind(hash)
    .bind(name)
    .bind(group)
    .fetch_one(pool)
    .await
    .unwrap()
}

pub async fn set_debt_balance(pool: &PgPool, user: Uuid, debt: i64, balance: i64) {
    sqlx::query("UPDATE users SET debt_minutes = $1, balance_minutes = $2 WHERE id = $3")
        .bind(debt)
        .bind(balance)
        .bind(user)
        .execute(pool)
        .await
        .unwrap();
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({"_raw": String::from_utf8_lossy(&bytes)}));
    (status, v)
}

pub async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut b = Request::builder().method(method).uri(path);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let req = match body {
        Some(v) => b
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&v).unwrap()))
            .unwrap(),
        None => b.body(Body::empty()).unwrap(),
    };
    send(app, req).await
}

pub async fn call_multipart(
    app: &Router,
    path: &str,
    token: &str,
    parts: Vec<(&str, Option<&str>, &str, Vec<u8>)>, // (name, filename, content_type, data)
) -> (StatusCode, Value) {
    let boundary = "TESTBOUNDARY";
    let mut body: Vec<u8> = Vec::new();
    for (name, filename, ctype, data) in parts {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"").as_bytes());
        if let Some(f) = filename {
            body.extend_from_slice(format!("; filename=\"{f}\"").as_bytes());
        }
        body.extend_from_slice(b"\r\n");
        if !ctype.is_empty() {
            body.extend_from_slice(format!("Content-Type: {ctype}\r\n").as_bytes());
        }
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(&data);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    send(app, req).await
}

pub async fn login(app: &Router, login: &str) -> String {
    let (status, v) = call(
        app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({"login": login, "password": PASSWORD})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "login failed: {v}");
    v["token"].as_str().unwrap().to_string()
}

/// komendant opens a QR of the given difficulty via the API
pub async fn open_qr(app: &Router, komendant_token: &str, difficulty: &str) -> String {
    let (status, v) = call(
        app,
        "POST",
        "/api/komendant/qr/open",
        Some(komendant_token),
        Some(json!({"difficulty": difficulty})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "qr open failed: {v}");
    v["qr"].as_str().unwrap().to_string()
}

pub async fn start_session(app: &Router, student_token: &str, qr: &str) -> Value {
    let (status, v) = call(
        app,
        "POST",
        "/api/student/sessions/start",
        Some(student_token),
        Some(json!({"qr": qr})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "session start failed: {v}");
    v
}

pub async fn finish_session(
    app: &Router,
    student_token: &str,
    qr: &str,
) -> (StatusCode, Value) {
    call_multipart(
        app,
        "/api/student/sessions/finish",
        student_token,
        vec![
            ("qr", None, "", qr.as_bytes().to_vec()),
            ("photo", Some("p.jpg"), "image/jpeg", b"\xff\xd8\xff\xe0fakejpeg".to_vec()),
        ],
    )
    .await
}
