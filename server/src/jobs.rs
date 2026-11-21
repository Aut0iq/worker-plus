use chrono::Datelike;
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::WorkSession;
use crate::state::{AppState, AuditRecord};
use crate::summaries;

pub fn spawn_all(state: AppState) {
    spawn_expiry_loop(state.clone());
    spawn_summary_loop(state.clone());
    spawn_cleanup_loop(state);
}

/// Read notifications are kept 180 days, then deleted (the table otherwise only grows).
/// Audit/komendant logs are kept forever: the spec requires them.
const NOTIFICATION_RETENTION_DAYS: i64 = 180;

fn spawn_cleanup_loop(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        loop {
            interval.tick().await;
            if let Err(e) = cleanup_notifications(&state).await {
                tracing::error!(error = ?e, "notification cleanup failed");
            }
            if let Err(e) = cleanup_photos(&state).await {
                tracing::error!(error = ?e, "photo cleanup failed");
            }
        }
    });
}

pub async fn cleanup_notifications(state: &AppState) -> AppResult<u64> {
    let cutoff = state.clock.now() - chrono::Duration::days(NOTIFICATION_RETENTION_DAYS);
    let res = sqlx::query("DELETE FROM notifications WHERE read_at IS NOT NULL AND created_at < $1")
        .bind(cutoff)
        .execute(&state.pool)
        .await?;
    Ok(res.rows_affected())
}

/// Work photos (a swept floor, etc.) are only evidence for the komendant/admin check.
/// Once decided (admin reviewed the photo, or the komendant rejected the session) the
/// photo is deleted after 3 days; otherwise 7 days after the session ended. A session
/// still waiting for the komendant keeps its photo until it is resolved.
const PHOTO_DECIDED_DAYS: i64 = 3;
const PHOTO_RETENTION_DAYS: i64 = 7;

pub async fn cleanup_photos(state: &AppState) -> AppResult<usize> {
    let now = state.clock.now();
    let decided_cutoff = now - chrono::Duration::days(PHOTO_DECIDED_DAYS);
    let cutoff = now - chrono::Duration::days(PHOTO_RETENTION_DAYS);
    // DB first, files second: a failed unlink leaves an orphan file, never a dangling path.
    // An unreviewed photo that is gone has nothing left to review.
    // A komendant rejection has no timestamp of its own: it counts from ended_at.
    let paths: Vec<String> = sqlx::query_scalar(
        "WITH old AS (
             SELECT id, photo_path FROM work_sessions
             WHERE photo_path IS NOT NULL AND status <> 'pending_confirmation'
               AND (ended_at < $2
                    OR (status = 'komendant_rejected' AND ended_at < $1)
                    OR (photo_review IN ('approved', 'rejected') AND photo_reviewed_at < $1))
             FOR UPDATE
         )
         UPDATE work_sessions w SET photo_path = NULL,
             photo_review = CASE WHEN w.photo_review = 'pending' THEN NULL ELSE w.photo_review END
         FROM old WHERE w.id = old.id
         RETURNING old.photo_path",
    )
    .bind(decided_cutoff)
    .bind(cutoff)
    .fetch_all(&state.pool)
    .await?;
    for rel in &paths {
        // photo_path is generated server-side (uuid.ext); guard anyway
        if rel.contains('/') || rel.contains('\\') || rel.contains("..") {
            continue;
        }
        match tokio::fs::remove_file(state.config.photo_dir.join(rel)).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(error = ?e, path = %rel, "photo delete failed"),
        }
    }
    Ok(paths.len())
}

fn spawn_expiry_loop(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) = expire_overdue_sessions(&state).await {
                tracing::error!(error = ?e, "session expiry job failed");
            }
        }
    });
}

fn spawn_summary_loop(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) = maybe_generate_monthly(&state).await {
                tracing::error!(error = ?e, "monthly summary job failed");
            }
        }
    });
}

/// Expire `active` sessions older than MAX_SESSION_HOURS (real time).
/// Idempotent: only `active` rows are touched, so repeated runs are no-ops.
pub async fn expire_overdue_sessions(state: &AppState) -> AppResult<usize> {
    let now = state.clock.now();
    let cutoff = now - chrono::Duration::hours(state.config.max_session_hours);

    let mut tx = state.pool.begin().await?;
    let expired: Vec<WorkSession> = sqlx::query_as::<_, WorkSession>(
        "UPDATE work_sessions SET status = 'expired', expired_at = $1
         WHERE status = 'active' AND started_at < $2
         RETURNING *",
    )
    .bind(now)
    .bind(cutoff)
    .fetch_all(&mut *tx)
    .await?;

    if expired.is_empty() {
        tx.commit().await?;
        return Ok(0);
    }

    let admins: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM users WHERE role = 'admin' AND is_active")
            .fetch_all(&mut *tx)
            .await?;

    let mut push_targets = Vec::new();
    for s in &expired {
        let mut targets = vec![s.student_id, s.komendant_id];
        targets.extend(admins.iter().copied());
        let body = "ОПТ превысил лимит времени и был аннулирован".to_string();
        state
            .notify(
                &mut tx,
                &targets,
                "session_expired",
                "ОПТ аннулирован",
                &body,
                json!({"session_id": s.id}),
            )
            .await?;
        state
            .audit(
                &mut *tx,
                AuditRecord::new("session_expire")
                    .entity("session", s.id)
                    .details(json!({"reason": "timeout", "student_id": s.student_id})),
            )
            .await?;
        state
            .komendant_log(
                &mut *tx,
                s.komendant_id,
                "session_expired",
                Some(s.student_id),
                Some(s.id),
                Some(json!({"reason": "timeout"})),
            )
            .await?;
        push_targets.extend(targets);
    }
    tx.commit().await?;

    push_targets.sort();
    push_targets.dedup();
    state.push_later(
        push_targets,
        "ОПТ аннулирован".into(),
        "ОПТ превысил лимит времени и был аннулирован".into(),
        json!({}),
    );
    Ok(expired.len())
}

/// Called every minute: on the 1st of a month after 00:05 local time, generate
/// the previous month's summary if it does not exist yet. If the server was down
/// on the 1st, catches up on any later day as soon as it sees the gap.
pub async fn maybe_generate_monthly(state: &AppState) -> AppResult<Option<Uuid>> {
    let local = state.clock.now().with_timezone(&state.config.timezone);
    let past_first = local.day() > 1
        || local.time() >= chrono::NaiveTime::from_hms_opt(0, 5, 0).unwrap();
    if !past_first {
        return Ok(None);
    }
    let (py, pm) = prev_month(local.year(), local.month() as i32);

    let exists: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM monthly_summaries WHERE year = $1 AND month = $2")
            .bind(py)
            .bind(pm)
            .fetch_optional(&state.pool)
            .await?;
    if exists.is_some() {
        return Ok(None);
    }
    let id = summaries::generate(state, py, pm, None).await?;
    Ok(Some(id))
}

fn prev_month(year: i32, month: i32) -> (i32, i32) {
    if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    }
}
