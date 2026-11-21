use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::{AuthUser, KomendantUser};
use crate::domain;
use crate::dto::{
    FinishQrResponse, PageQuery, Paginated, QrOpenRequest, QrOpenResponse, RejectSessionRequest, SessionDto,
};
use crate::error::{AppError, AppResult};
use crate::models::{Difficulty, Role, SessionStatus, User, WorkSession};
use crate::qr;
use crate::rows::{SessionRow, SESSION_SELECT};
use crate::state::{AppState, AuditRecord};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/komendant/qr/open", post(qr_open))
        .route("/komendant/qr", get(qr_refresh))
        .route("/komendant/qr/finish/open", post(finish_qr_open))
        .route("/komendant/qr/finish", get(finish_qr_refresh))
        .route("/komendant/sessions", get(list_sessions))
        .route("/komendant/sessions/{id}/confirm", post(confirm_session))
        .route("/komendant/sessions/{id}/reject", post(reject_session))
        .route("/komendant/sessions/{id}/photo", get(session_photo))
}

#[derive(Deserialize)]
struct QrQuery {
    difficulty: Option<Difficulty>,
}

async fn issue_qr(
    state: &AppState,
    auth: &KomendantUser,
    difficulty: Difficulty,
    log: bool,
) -> AppResult<QrOpenResponse> {
    let issued = qr::issue(
        &state.config.qr_secret,
        auth.user.id,
        difficulty,
        state.clock.now(),
    );
    if log {
        let mut tx = state.pool.begin().await?;
        state
            .audit(
                &mut *tx,
                AuditRecord::new("qr_open")
                    .actor(auth.user.id, "komendant")
                    .details(json!({"difficulty": difficulty})),
            )
            .await?;
        state
            .komendant_log(
                &mut *tx,
                auth.user.id,
                "qr_open",
                None,
                None,
                Some(json!({"difficulty": difficulty})),
            )
            .await?;
        tx.commit().await?;
    }
    Ok(QrOpenResponse {
        qr: issued.qr,
        expires_at: issued.expires_at,
        difficulty: issued.difficulty,
    })
}

async fn qr_open(
    State(state): State<AppState>,
    auth: KomendantUser,
    Json(req): Json<QrOpenRequest>,
) -> AppResult<Json<QrOpenResponse>> {
    issue_qr(&state, &auth, req.difficulty, true).await.map(Json)
}

async fn qr_refresh(
    State(state): State<AppState>,
    auth: KomendantUser,
    Query(q): Query<QrQuery>,
) -> AppResult<Json<QrOpenResponse>> {
    let difficulty = q.difficulty.ok_or_else(|| {
        AppError::bad_request("missing_difficulty", "Не указана сложность (light или hard)")
    })?;
    issue_qr(&state, &auth, difficulty, false).await.map(Json)
}

fn finish_qr(state: &AppState, auth: &KomendantUser) -> FinishQrResponse {
    let issued = qr::issue_finish(&state.config.qr_secret, auth.user.id, state.clock.now());
    FinishQrResponse {
        qr: issued.qr,
        expires_at: issued.expires_at,
    }
}

/// Opening the finish QR is logged like the start QR; refreshing it is not.
async fn finish_qr_open(
    State(state): State<AppState>,
    auth: KomendantUser,
) -> AppResult<Json<FinishQrResponse>> {
    let mut tx = state.pool.begin().await?;
    state
        .audit(
            &mut *tx,
            AuditRecord::new("finish_qr_open").actor(auth.user.id, "komendant"),
        )
        .await?;
    state
        .komendant_log(&mut *tx, auth.user.id, "finish_qr_open", None, None, None)
        .await?;
    tx.commit().await?;
    Ok(Json(finish_qr(&state, &auth)))
}

async fn finish_qr_refresh(State(state): State<AppState>, auth: KomendantUser) -> Json<FinishQrResponse> {
    Json(finish_qr(&state, &auth))
}

#[derive(Deserialize)]
struct SessionQuery {
    status: Option<SessionStatus>,
    page: Option<i64>,
    per_page: Option<i64>,
}

impl SessionQuery {
    fn paging(&self) -> PageQuery {
        PageQuery {
            page: self.page,
            per_page: self.per_page,
        }
    }
}

async fn list_sessions(
    State(state): State<AppState>,
    auth: KomendantUser,
    Query(q): Query<SessionQuery>,
) -> AppResult<Json<Paginated<SessionDto>>> {
    let (total, items) = if let Some(st) = q.status {
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM work_sessions WHERE komendant_id = $1 AND status = $2",
        )
        .bind(auth.user.id)
        .bind(st)
        .fetch_one(&state.pool)
        .await?;
        let items = sqlx::query_as::<_, SessionRow>(sqlx::AssertSqlSafe(format!(
            "{SESSION_SELECT} WHERE s.komendant_id = $1 AND s.status = $2
             ORDER BY s.started_at DESC LIMIT $3 OFFSET $4"
        )))
        .bind(auth.user.id)
        .bind(st)
        .bind(q.paging().per_page())
        .bind(q.paging().offset())
        .fetch_all(&state.pool)
        .await?;
        (total, items)
    } else {
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM work_sessions WHERE komendant_id = $1",
        )
        .bind(auth.user.id)
        .fetch_one(&state.pool)
        .await?;
        let items = sqlx::query_as::<_, SessionRow>(sqlx::AssertSqlSafe(format!(
            "{SESSION_SELECT} WHERE s.komendant_id = $1
             ORDER BY s.started_at DESC LIMIT $2 OFFSET $3"
        )))
        .bind(auth.user.id)
        .bind(q.paging().per_page())
        .bind(q.paging().offset())
        .fetch_all(&state.pool)
        .await?;
        (total, items)
    };
    Ok(Json(Paginated {
        items: items.into_iter().map(Into::into).collect(),
        total,
        page: q.paging().page(),
        per_page: q.paging().per_page(),
    }))
}

async fn confirm_session(
    State(state): State<AppState>,
    auth: KomendantUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<SessionDto>> {
    let mut tx = state.pool.begin().await?;
    let session = lock_pending_session(&mut tx, id).await?;
    if session.komendant_id != auth.user.id {
        return Err(AppError::forbidden("Чужой ОПТ"));
    }
    let pushes = credit_session(&state, &mut tx, &session, auth.user.id, "komendant").await?;
    tx.commit().await?;
    for (users, title, body) in pushes {
        state.push_later(users, title, body, json!({"session_id": session.id}));
    }
    session_dto(&state, session.id).await.map(Json)
}

pub(crate) async fn session_dto(state: &AppState, id: Uuid) -> AppResult<SessionDto> {
    let row = sqlx::query_as::<_, SessionRow>(sqlx::AssertSqlSafe(format!(
        "{SESSION_SELECT} WHERE s.id = $1"
    )))
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    Ok(row.into())
}

/// Lock a session that must be in `pending_confirmation`.
pub(crate) async fn lock_pending_session(
    tx: &mut sqlx::PgConnection,
    id: Uuid,
) -> AppResult<WorkSession> {
    let session = sqlx::query_as::<_, WorkSession>(
        "SELECT * FROM work_sessions WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::not_found("ОПТ не найден"))?;
    if session.status != SessionStatus::PendingConfirmation {
        return Err(AppError::conflict(
            "bad_status",
            "ОПТ не ожидает подтверждения",
        ));
    }
    Ok(session)
}

/// Push messages to send after commit: (recipients, title, body).
pub(crate) type Pushes = Vec<(Vec<Uuid>, String, String)>;

/// Credit a pending session (komendant confirm, or admin resolving a stuck one):
/// minutes cover debt first, the rest goes to balance; the photo goes to the
/// admin review queue. Notifies the student and the teachers who assigned
/// the hours being worked off. Runs inside the caller's transaction.
pub(crate) async fn credit_session(
    state: &AppState,
    tx: &mut sqlx::PgConnection,
    session: &WorkSession,
    actor_id: Uuid,
    actor_role: &str,
) -> AppResult<Pushes> {
    let credited = session.credited_minutes.unwrap_or(0);

    // lock the student row while adjusting debt/balance
    let student = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1 FOR UPDATE")
        .bind(session.student_id)
        .fetch_one(&mut *tx)
        .await?;

    let alloc = domain::allocate(credited, student.debt_minutes);
    sqlx::query(
        "UPDATE users SET debt_minutes = debt_minutes - $1, balance_minutes = balance_minutes + $2
         WHERE id = $3",
    )
    .bind(alloc.to_debt)
    .bind(alloc.to_balance)
    .bind(student.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE work_sessions SET status = 'credited', confirmed_at = $1,
         applied_to_debt = $2, applied_to_balance = $3,
         photo_review = CASE WHEN photo_path IS NULL THEN NULL ELSE 'pending' END
         WHERE id = $4",
    )
    .bind(state.clock.now())
    .bind(alloc.to_debt)
    .bind(alloc.to_balance)
    .bind(session.id)
    .execute(&mut *tx)
    .await?;

    state
        .audit(
            &mut *tx,
            AuditRecord::new("session_confirm")
                .actor(actor_id, actor_role)
                .entity("session", session.id)
                .details(json!({"to_debt": alloc.to_debt, "to_balance": alloc.to_balance})),
        )
        .await?;
    state
        .komendant_log(
            &mut *tx,
            session.komendant_id,
            "session_confirmed",
            Some(session.student_id),
            Some(session.id),
            Some(json!({"to_debt": alloc.to_debt, "to_balance": alloc.to_balance, "by": actor_role})),
        )
        .await?;

    let credited_s = domain::fmt_minutes(credited);
    let mut pushes: Pushes = Vec::new();
    let body = format!("Общественно полезный труд подтверждён: зачтено {credited_s}");
    state
        .notify(
            tx,
            &[session.student_id],
            "session_credited",
            "Часы зачтены",
            &body,
            json!({"session_id": session.id}),
        )
        .await?;
    pushes.push((vec![session.student_id], "Часы зачтены".into(), body));

    // teachers who assigned hours to this student learn that debt was worked off
    if alloc.to_debt > 0 {
        let teachers: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT a.assigned_by FROM assignments a
             JOIN users t ON t.id = a.assigned_by
             WHERE a.student_id = $1 AND t.is_active AND t.role = 'teacher'",
        )
        .bind(session.student_id)
        .fetch_all(&mut *tx)
        .await?;
        if !teachers.is_empty() {
            let left = student.debt_minutes - alloc.to_debt;
            let title = if left == 0 {
                "Задание выполнено"
            } else {
                "Студент выполнил часть задания"
            };
            let body = format!(
                "{}: выполнено {}, осталось {}",
                student.full_name,
                domain::fmt_minutes(alloc.to_debt),
                domain::fmt_minutes(left)
            );
            state
                .notify(
                    tx,
                    &teachers,
                    "debt_worked_off",
                    title,
                    &body,
                    json!({"session_id": session.id, "student_id": session.student_id}),
                )
                .await?;
            pushes.push((teachers, title.into(), body));
        }
    }
    Ok(pushes)
}

async fn reject_session(
    State(state): State<AppState>,
    auth: KomendantUser,
    Path(id): Path<Uuid>,
    Json(req): Json<RejectSessionRequest>,
) -> AppResult<Json<SessionDto>> {
    let reason = req.reason.trim();
    if reason.is_empty() {
        return Err(AppError::bad_request("invalid_reason", "Укажите причину"));
    }
    let mut tx = state.pool.begin().await?;
    let session = lock_pending_session(&mut tx, id).await?;
    if session.komendant_id != auth.user.id {
        return Err(AppError::forbidden("Чужой ОПТ"));
    }
    let push = reject_pending(&state, &mut tx, &session, reason, auth.user.id, "komendant").await?;
    tx.commit().await?;
    state.push_later(push.0, push.1, push.2, json!({"session_id": session.id}));
    session_dto(&state, session.id).await.map(Json)
}

/// Reject a pending session (komendant, or admin resolving a stuck one). Inside the caller's tx.
pub(crate) async fn reject_pending(
    state: &AppState,
    tx: &mut sqlx::PgConnection,
    session: &WorkSession,
    reason: &str,
    actor_id: Uuid,
    actor_role: &str,
) -> AppResult<(Vec<Uuid>, String, String)> {
    sqlx::query(
        "UPDATE work_sessions SET status = 'komendant_rejected', rejected_reason = $1 WHERE id = $2",
    )
    .bind(reason)
    .bind(session.id)
    .execute(&mut *tx)
    .await?;
    state
        .audit(
            &mut *tx,
            AuditRecord::new("session_reject")
                .actor(actor_id, actor_role)
                .entity("session", session.id)
                .details(json!({"reason": reason})),
        )
        .await?;
    state
        .komendant_log(
            &mut *tx,
            session.komendant_id,
            "session_rejected",
            Some(session.student_id),
            Some(session.id),
            Some(json!({"reason": reason, "by": actor_role})),
        )
        .await?;
    let body = format!("Общественно полезный труд отклонён: {reason}");
    state
        .notify(
            tx,
            &[session.student_id],
            "session_rejected",
            "ОПТ отклонён",
            &body,
            json!({"session_id": session.id}),
        )
        .await?;
    Ok((vec![session.student_id], "ОПТ отклонён".into(), body))
}

/// Serve a session photo from disk. Shared by the komendant/student and admin routes.
pub(crate) async fn serve_photo(state: &AppState, session: &WorkSession) -> AppResult<Response> {
    let rel = session
        .photo_path
        .as_ref()
        .ok_or_else(|| AppError::not_found("Фото отсутствует"))?;
    // photo_path is generated server-side (uuid.ext); guard anyway
    if rel.contains('/') || rel.contains('\\') || rel.contains("..") {
        return Err(AppError::internal("bad photo path"));
    }
    let mime = domain::image_mime(rel.rsplit('.').next().unwrap_or(""));
    let bytes = tokio::fs::read(state.config.photo_dir.join(rel))
        .await
        .map_err(|_| AppError::not_found("Фото не найдено на диске"))?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    Ok((StatusCode::OK, headers, bytes).into_response())
}

async fn session_photo(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    // komendant can view own sessions' photos; admin can view any
    let session = sqlx::query_as::<_, WorkSession>("SELECT * FROM work_sessions WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("ОПТ не найден"))?;

    let allowed = match auth.user.role {
        Role::Komendant => session.komendant_id == auth.user.id,
        Role::Admin => true,
        Role::Student => session.student_id == auth.user.id,
        _ => false,
    };
    if !allowed {
        return Err(AppError::forbidden("Нет доступа к фото"));
    }
    if auth.user.role == Role::Komendant {
        state
            .komendant_log(
                &state.pool,
                auth.user.id,
                "photo_view",
                Some(session.student_id),
                Some(session.id),
                None,
            )
            .await?;
    }
    serve_photo(&state, &session).await
}
