use prila_server::auth::{hash_password, normalize_login};
use prila_server::config::Config;
use prila_server::AppState;
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    load_dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = Config::from_env().unwrap_or_else(|e| {
        eprintln!("configuration error: {e}");
        std::process::exit(1);
    });
    std::fs::create_dir_all(&config.photo_dir).ok();

    let pool = PgPoolOptions::new()
        .max_connections(16)
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    ensure_admin(&pool, &config).await?;

    let state = AppState::new(pool, config.clone());
    prila_server::jobs::spawn_all(state.clone());

    let app = prila_server::build_app(state);
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "server listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}

fn load_dotenv() {
    // minimal .env loader so we don't need the dotenvy crate
    let Ok(content) = std::fs::read_to_string(".env") else {
        return;
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if std::env::var(k.trim()).is_err() {
                std::env::set_var(k.trim(), v.trim().trim_matches('"').trim_matches('\''));
            }
        }
    }
}

async fn ensure_admin(
    pool: &sqlx::PgPool,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    // only an *active* admin counts: if every admin was deactivated, nobody could log in
    let exists: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT id FROM users WHERE role = 'admin' AND is_active LIMIT 1")
            .fetch_optional(pool)
            .await?;
    if exists.is_some() {
        return Ok(());
    }
    let login = normalize_login(&config.admin_login);
    let hash = hash_password(&config.admin_password)?;
    // the configured login may already exist (e.g. the deactivated admin):
    // restore it as an active admin with the password from ADMIN_PASSWORD
    let restored = sqlx::query(
        "UPDATE users SET role = 'admin', is_active = TRUE, password_hash = $2,
                password_changed_at = now()
         WHERE lower(login) = $1",
    )
    .bind(&login)
    .bind(&hash)
    .execute(pool)
    .await?;
    if restored.rows_affected() > 0 {
        tracing::warn!(login = %login, "no active admin: restored admin account from ADMIN_LOGIN/ADMIN_PASSWORD");
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO users (role, login, password_hash, full_name)
         VALUES ('admin', $1, $2, 'Администратор')",
    )
    .bind(&login)
    .bind(&hash)
    .execute(pool)
    .await?;
    tracing::info!(login = %login, "created initial admin user");
    Ok(())
}
