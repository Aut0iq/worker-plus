use chrono_tz::Tz;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub qr_secret: String,
    pub timezone: Tz,
    pub photo_dir: PathBuf,
    pub log_dir: PathBuf,
    pub static_dir: Option<PathBuf>,
    pub bind_addr: String,
    pub admin_login: String,
    pub admin_password: String,
    pub expo_push_enabled: bool,
    pub expo_push_url: String,
    pub trust_proxy: bool,
    pub cors_origins: String,
    pub max_session_hours: i64,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let get = |k: &str| -> Result<String, String> {
            std::env::var(k).map_err(|_| format!("missing env var {k}"))
        };
        let opt = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        let timezone: Tz = opt("APP_TIMEZONE", "Asia/Vladivostok")
            .parse()
            .map_err(|_| "invalid APP_TIMEZONE".to_string())?;
        let static_dir = std::env::var("STATIC_DIR").ok().map(PathBuf::from);
        let jwt_secret = get("JWT_SECRET")?;
        let qr_secret = get("QR_SECRET")?;
        check_secret("JWT_SECRET", &jwt_secret)?;
        check_secret("QR_SECRET", &qr_secret)?;
        Ok(Config {
            database_url: get("DATABASE_URL")?,
            jwt_secret,
            qr_secret,
            timezone,
            photo_dir: PathBuf::from(opt("PHOTO_DIR", "./data/photos")),
            log_dir: PathBuf::from(opt("LOG_DIR", "./data/logs")),
            static_dir,
            bind_addr: opt("BIND_ADDR", "0.0.0.0:8080"),
            admin_login: opt("ADMIN_LOGIN", "admin"),
            admin_password: get("ADMIN_PASSWORD")?,
            expo_push_enabled: opt("EXPO_PUSH_ENABLED", "false")
                .parse()
                .unwrap_or(false),
            expo_push_url: opt("EXPO_PUSH_URL", "https://exp.host/--/api/v2/push/send"),
            trust_proxy: opt("TRUST_PROXY", "false").parse().unwrap_or(false),
            cors_origins: opt("CORS_ORIGINS", "*"),
            max_session_hours: opt("MAX_SESSION_HOURS", "4")
                .parse()
                .map_err(|_| "invalid MAX_SESSION_HOURS".to_string())?,
        })
    }
}

/// Anyone who knows JWT_SECRET/QR_SECRET can forge logins and QR codes, so the
/// placeholder values from .env.example and very short secrets are refused.
fn check_secret(name: &str, value: &str) -> Result<(), String> {
    let v = value.trim();
    if v.to_lowercase().contains("change-me") {
        return Err(format!("{name} still has the placeholder value from .env.example"));
    }
    if v.chars().count() < 16 {
        return Err(format!(
            "{name} is too short (need 16+ chars, 32+ recommended; e.g. `openssl rand -hex 32`)"
        ));
    }
    if v.chars().count() < 32 {
        eprintln!("warning: {name} is shorter than 32 characters");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check_secret;

    #[test]
    fn secrets_are_checked() {
        assert!(check_secret("X", "change-me-long-random-string").is_err());
        assert!(check_secret("X", "short").is_err());
        assert!(check_secret("X", "0123456789abcdef0123456789abcdef").is_ok());
    }
}
