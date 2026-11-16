use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::{AuthUser, KomendantUser};
use crate::domain;
use crate::dto::{PageQuery, Paginated, QrOpenRequest, QrOpenResponse, RejectSessionRequest, SessionDto};
use crate::error::{AppError, AppResult};
use crate::models::{Difficulty, Role, SessionStatus, User, WorkSession};
use crate::qr;
use crate::rows::{SessionRow, SESSION_SELECT};
use crate::state::{AppState, AuditRecord};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/komendant/qr/open", post(qr_open))
        .route("/komendant/qr", get(qr_refresh))
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
    issue_qr(&state, &auth, q.difficulty.unwrap_or(Difficulty::Light), false)
        .await
        .map(Json)
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
    let session = sqlx::query_as::<_, WorkSession>(
        "SELECT * FROM work_sessions WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::not_found("Сессия не найдена"))?;

    if session.komendant_id != auth.user.id {
        return Err(AppError::forbidden("Чужая сессия"));
    }
    if session.status != SessionStatus::PendingConfirmation {
        return Err(AppError::conflict(
            "bad_status",
            "Сессия не ожидает подтверждения",
        ));
    }
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
         applied_to_debt = $2, applied_to_balance = $3, photo_review = 'pending'
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
                .actor(auth.user.id, "komendant")
                .entity("session", session.id)
                .details(json!({"to_debt": alloc.to_debt, "to_balance": alloc.to_balance})),
        )
        .await?;
    state
        .komendant_log(
            &mut *tx,
            auth.user.id,
            "session_confirmed",
            Some(session.student_id),
            Some(session.id),
            Some(json!({"to_debt": alloc.to_debt, "to_balance": alloc.to_balance})),
        )
        .await?;
    state
        .notify(
            &mut tx,
            &[session.student_id],
            "session_credited",
            "Часы зачтены",
            &format!("Комендант подтвердил сессию: зачтено {} мин", credited),
            json!({"session_id": session.id}),
        )
        .await?;
    tx.commit().await?;

    state.push_later(
        vec![session.student_id],
        "Часы зачтены".into(),
        format!("Зачтено {} мин", credited),
        json!({"session_id": session.id}),
    );

    let row = sqlx::query_as::<_, SessionRow>(sqlx::AssertSqlSafe(format!("{SESSION_SELECT} WHERE s.id = $1")))
        .bind(session.id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.into()))
}

async fn reject_session(
    State(state): State<AppState>,
    auth: KomendantUser,
    Path(id): Path<Uuid>,
    Json(req): Json<RejectSessionRequest>,
) -> AppResult<Json<SessionDto>> {
    let mut tx = state.pool.begin().await?;
    let session = sqlx::query_as::<_, WorkSession>(
        "SELECT * FROM work_sessions WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::not_found("Сессия не найдена"))?;

    if session.komendant_id != auth.user.id {
        return Err(AppError::forbidden("Чужая сессия"));
    }
    if session.status != SessionStatus::PendingConfirmation {
        return Err(AppError::conflict(
            "bad_status",
            "Сессия не ожидает подтверждения",
        ));
    }
    sqlx::query(
        "UPDATE work_sessions SET status = 'komendant_rejected', rejected_reason = $1 WHERE id = $2",
    )
    .bind(&req.reason)
    .bind(session.id)
    .execute(&mut *tx)
    .await?;

    state
        .audit(
            &mut *tx,
            AuditRecord::new("session_reject")
                .actor(auth.user.id, "komendant")
                .entity("session", session.id)
                .details(json!({"reason": req.reason})),
        )
        .await?;
    state
        .komendant_log(
            &mut *tx,
            auth.user.id,
            "session_rejected",
            Some(session.student_id),
            Some(session.id),
            Some(json!({"reason": req.reason})),
        )
        .await?;
    state
        .notify(
            &mut tx,
            &[session.student_id],
            "session_rejected",
            "Сессия отклонена",
            &format!("Комендант отклонил сессию: {}", req.reason),
            json!({"session_id": session.id}),
        )
        .await?;
    tx.commit().await?;

    state.push_later(
        vec![session.student_id],
        "Сессия отклонена".into(),
        format!("Комендант отклонил сессию: {}", req.reason),
        json!({"session_id": session.id}),
    );

    let row = sqlx::query_as::<_, SessionRow>(sqlx::AssertSqlSafe(format!("{SESSION_SELECT} WHERE s.id = $1")))
        .bind(session.id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.into()))
}

const PHOTO_CONTENT_TYPES: [(&str, &str); 4] = [
    ("jpg", "image/jpeg"),
    ("png", "image/png"),
    ("heic", "image/heic"),
    ("webp", "image/webp"),
];

async fn serve_photo(state: &AppState, session: &WorkSession) -> AppResult<Response> {
    let rel = session
        .photo_path
        .as_ref()
        .ok_or_else(|| AppError::not_found("Фото отсутствует"))?;
    // photo_path is generated server-side (uuid.ext); guard anyway
    if rel.contains('/') || rel.contains("..") {
        return Err(AppError::internal("bad photo path"));
    }
    let ext = rel.rsplit('.').next().unwrap_or("");
    let mime = PHOTO_CONTENT_TYPES
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, m)| *m)
        .unwrap_or("application/octet-stream");
    let bytes = tokio::fs::read(state.config.photo_dir.join(rel))
        .await
        .map_err(|_| AppError::not_found("Фото не найдено на диске"))?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, mime.parse().unwrap());
    headers.insert(
        header::CACHE_CONTROL,
        "private, no-store".parse().unwrap(),
    );
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
        .ok_or_else(|| AppError::not_found("Сессия не найдена"))?;

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
