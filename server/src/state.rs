use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::PgPool;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use uuid::Uuid;

use crate::clock::{Clock, SystemClock};
use crate::config::Config;
use crate::error::{AppError, AppResult};

/// Failed-login counter: key "ip|login" -> (failures, window start).
pub type LoginAttemptMap = HashMap<String, (u32, DateTime<Utc>)>;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub clock: Arc<dyn Clock>,
    pub http: reqwest::Client,
    pub login_attempts: Arc<Mutex<LoginAttemptMap>>,
    audit_file: Arc<Mutex<Option<NonBlocking>>>,
    komendant_file: Arc<Mutex<Option<NonBlocking>>>,
    _guard: Arc<Vec<WorkerGuard>>,
}

impl AppState {
    /// Production state: real clock, rotating log files in `config.log_dir`.
    pub fn new(pool: PgPool, config: Config) -> Self {
        let mut guards = Vec::new();
        let (audit_file, komendant_file) = match std::fs::create_dir_all(&config.log_dir) {
            Ok(()) => {
                let (w1, g1) = tracing_appender::non_blocking(
                    tracing_appender::rolling::daily(&config.log_dir, "audit.log"),
                );
                let (w2, g2) = tracing_appender::non_blocking(
                    tracing_appender::rolling::daily(&config.log_dir, "komendant.log"),
                );
                guards.push(g1);
                guards.push(g2);
                (Some(w1), Some(w2))
            }
            Err(e) => {
                tracing::error!(error = %e, "cannot create log dir, file audit disabled");
                (None, None)
            }
        };
        Self::build(
            pool,
            Arc::new(config),
            Arc::new(SystemClock),
            audit_file,
            komendant_file,
            guards,
        )
    }

    /// State for tests: injectable clock, log files may be disabled.
    pub fn new_test(
        pool: PgPool,
        config: Config,
        clock: Arc<dyn Clock>,
        log_dir: Option<&std::path::Path>,
    ) -> Self {
        let mut guards = Vec::new();
        let mut audit = None;
        let mut kom = None;
        if let Some(dir) = log_dir {
            std::fs::create_dir_all(dir).ok();
            let (w1, g1) = tracing_appender::non_blocking(
                tracing_appender::rolling::daily(dir, "audit.log"),
            );
            let (w2, g2) = tracing_appender::non_blocking(
                tracing_appender::rolling::daily(dir, "komendant.log"),
            );
            guards.push(g1);
            guards.push(g2);
            audit = Some(w1);
            kom = Some(w2);
        }
        Self::build(pool, Arc::new(config), clock, audit, kom, guards)
    }

    fn build(
        pool: PgPool,
        config: Arc<Config>,
        clock: Arc<dyn Clock>,
        audit_file: Option<NonBlocking>,
        komendant_file: Option<NonBlocking>,
        guards: Vec<WorkerGuard>,
    ) -> Self {
        let _ = rustls::crypto::ring::default_provider().install_default();
        AppState {
            pool,
            config,
            clock,
            // a hanging push endpoint must not keep spawned tasks alive forever
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .connect_timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            login_attempts: Arc::new(Mutex::new(HashMap::new())),
            audit_file: Arc::new(Mutex::new(audit_file)),
            komendant_file: Arc::new(Mutex::new(komendant_file)),
            _guard: Arc::new(guards),
        }
    }

    /// Record an audit-log entry: one row in `audit_log` + one JSON line in audit.log.
    /// Runs inside the caller's transaction so it can't be forgotten or half-written.
    pub async fn audit<'e, E>(
        &self,
        executor: E,
        entry: AuditRecord<'_>,
    ) -> AppResult<()>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        let details = entry.details.unwrap_or_else(|| json!({}));
        sqlx::query(
            "INSERT INTO audit_log (actor_id, actor_role, action, entity_type, entity_id, details, ip)
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(entry.actor_id)
        .bind(entry.actor_role)
        .bind(entry.action)
        .bind(entry.entity_type)
        .bind(entry.entity_id.map(|e| e.to_string()))
        .bind(&details)
        .bind(entry.ip)
        .execute(executor)
        .await?;

        let line = json!({
            "ts": self.clock.now().to_rfc3339(),
            "actor_id": entry.actor_id,
            "actor_role": entry.actor_role,
            "action": entry.action,
            "entity_type": entry.entity_type,
            "entity_id": entry.entity_id.map(|e| e.to_string()),
            "details": details,
            "ip": entry.ip,
        });
        write_line(&self.audit_file, &line);
        Ok(())
    }

    /// Record a komendant-log entry: row in `komendant_log` + JSON line in komendant.log.
    pub async fn komendant_log<'e, E>(
        &self,
        executor: E,
        komendant_id: Uuid,
        action: &str,
        student_id: Option<Uuid>,
        session_id: Option<Uuid>,
        details: Option<Value>,
    ) -> AppResult<()>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        let details = details.unwrap_or_else(|| json!({}));
        sqlx::query(
            "INSERT INTO komendant_log (komendant_id, action, student_id, session_id, details)
             VALUES ($1,$2,$3,$4,$5)",
        )
        .bind(komendant_id)
        .bind(action)
        .bind(student_id)
        .bind(session_id)
        .bind(&details)
        .execute(executor)
        .await?;

        let line = json!({
            "ts": self.clock.now().to_rfc3339(),
            "komendant_id": komendant_id,
            "action": action,
            "student_id": student_id,
            "session_id": session_id,
            "details": details,
        });
        write_line(&self.komendant_file, &line);
        Ok(())
    }

    /// Insert notifications for users (inside tx) and schedule push delivery after commit.
    pub async fn notify(
        &self,
        executor: &mut sqlx::PgConnection,
        user_ids: &[Uuid],
        kind: &str,
        title: &str,
        body: &str,
        data: Value,
    ) -> AppResult<()> {
        if user_ids.is_empty() {
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO notifications (user_id, kind, title, body, data)
             SELECT u, $2, $3, $4, $5 FROM unnest($1::uuid[]) AS u",
        )
        .bind(user_ids)
        .bind(kind)
        .bind(title)
        .bind(body)
        .bind(&data)
        .execute(&mut *executor)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    /// Fire-and-forget Expo push to all tokens of the given users. Failures are logged only.
    pub fn push_later(&self, user_ids: Vec<Uuid>, title: String, body: String, data: Value) {
        if !self.config.expo_push_enabled {
            return;
        }
        let pool = self.pool.clone();
        let url = self.config.expo_push_url.clone();
        let http = self.http.clone();
        tokio::spawn(async move {
            if let Err(e) = send_push(&pool, &http, &url, user_ids, title, body, data).await {
                tracing::warn!(error = %e, "expo push failed");
            }
        });
    }
}

fn write_line(file: &Arc<Mutex<Option<NonBlocking>>>, line: &Value) {
    if let Ok(mut guard) = file.lock() {
        if let Some(w) = guard.as_mut() {
            let _ = w.write_all(serde_json::to_string(line).unwrap_or_default().as_bytes());
            let _ = w.write_all(b"\n");
        }
    }
}

pub struct AuditRecord<'a> {
    pub actor_id: Option<Uuid>,
    pub actor_role: Option<&'a str>,
    pub action: &'a str,
    pub entity_type: Option<&'a str>,
    pub entity_id: Option<Uuid>,
    pub details: Option<Value>,
    pub ip: Option<&'a str>,
}

impl<'a> AuditRecord<'a> {
    pub fn new(action: &'a str) -> Self {
        AuditRecord {
            actor_id: None,
            actor_role: None,
            action,
            entity_type: None,
            entity_id: None,
            details: None,
            ip: None,
        }
    }

    pub fn actor(mut self, id: Uuid, role: &'a str) -> Self {
        self.actor_id = Some(id);
        self.actor_role = Some(role);
        self
    }

    pub fn actor_opt(mut self, id: Option<Uuid>, role: &'a str) -> Self {
        self.actor_id = id;
        self.actor_role = id.map(|_| role);
        self
    }

    pub fn entity(mut self, ty: &'a str, id: Uuid) -> Self {
        self.entity_type = Some(ty);
        self.entity_id = Some(id);
        self
    }

    pub fn details(mut self, d: Value) -> Self {
        self.details = Some(d);
        self
    }

    pub fn ip(mut self, ip: Option<&'a str>) -> Self {
        self.ip = ip;
        self
    }
}

#[derive(Serialize)]
struct ExpoMessage<'a> {
    to: &'a str,
    title: &'a str,
    body: &'a str,
    data: &'a Value,
    sound: &'static str,
    priority: &'static str,
    #[serde(rename = "channelId")]
    channel_id: &'static str,
}

async fn send_push(
    pool: &PgPool,
    http: &reqwest::Client,
    url: &str,
    user_ids: Vec<Uuid>,
    title: String,
    body: String,
    data: Value,
) -> Result<(), String> {
    if user_ids.is_empty() {
        return Ok(());
    }
    let tokens: Vec<String> = sqlx::query_scalar(
        "SELECT token FROM push_tokens WHERE user_id = ANY($1)",
    )
    .bind(&user_ids)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    if tokens.is_empty() {
        return Ok(());
    }
    // Expo accepts at most 100 messages per request
    for chunk in tokens.chunks(100) {
        let msgs: Vec<ExpoMessage> = chunk
            .iter()
            .map(|t| ExpoMessage {
                to: t,
                title: &title,
                body: &body,
                data: &data,
                sound: "default",
                priority: "high",
                channel_id: "default",
            })
            .collect();
        let resp = http
            .post(url)
            .json(&msgs)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("push status {status}: {text}"));
        }
        let dead = dead_tokens(chunk, &text);
        if !dead.is_empty() {
            tracing::info!(count = dead.len(), "removing unregistered push tokens");
            sqlx::query("DELETE FROM push_tokens WHERE token = ANY($1)")
                .bind(&dead)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Expo push tickets come back in request order; `DeviceNotRegistered` means the
/// app was uninstalled or the token rotated — such tokens are useless forever.
pub fn dead_tokens(tokens: &[String], response: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(response) else {
        return Vec::new();
    };
    let Some(tickets) = v.get("data").and_then(Value::as_array) else {
        return Vec::new();
    };
    tickets
        .iter()
        .zip(tokens)
        .filter(|(t, _)| {
            t.get("status").and_then(Value::as_str) == Some("error")
                && t.pointer("/details/error").and_then(Value::as_str)
                    == Some("DeviceNotRegistered")
        })
        .map(|(_, tok)| tok.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::dead_tokens;

    #[test]
    fn dead_tokens_from_expo_tickets() {
        let tokens = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let resp = r#"{"data":[
            {"status":"ok","id":"1"},
            {"status":"error","message":"x","details":{"error":"DeviceNotRegistered"}},
            {"status":"error","message":"y","details":{"error":"MessageRateExceeded"}}
        ]}"#;
        assert_eq!(dead_tokens(&tokens, resp), vec!["b".to_string()]);
        assert!(dead_tokens(&tokens, "not json").is_empty());
    }
}
