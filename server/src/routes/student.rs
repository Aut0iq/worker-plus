use axum::extract::{Multipart, Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use uuid::Uuid;

use crate::auth::StudentUser;
use crate::domain;
use crate::dto::{AssignmentDto, OrderDto, PageQuery, Paginated, SessionDto, SessionStartRequest};
use crate::error::{AppError, AppResult};
use crate::models::WorkSession;
use crate::qr;
use crate::rows::{AssignmentRow, OrderRow, SessionRow, ASSIGNMENT_SELECT, ORDER_SELECT, SESSION_SELECT};
use crate::state::{AppState, AuditRecord};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/student/sessions/start", post(start_session))
        .route("/student/sessions/finish", post(finish_session))
        .route("/student/sessions/active", get(active_session))
        .route("/student/sessions", get(list_sessions))
        .route("/student/assignments", get(list_assignments))
        .route("/student/orders", get(list_orders))
        .route("/student/orders/{id}", get(get_order))
}

async fn start_session(
    State(state): State<AppState>,
    auth: StudentUser,
    Json(req): Json<SessionStartRequest>,
) -> AppResult<Json<SessionDto>> {
    let verified = qr::verify(&state.config.qr_secret, &req.qr, state.clock.now())?;

    // komendant must exist and be an active komendant
    let kom_exists: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM users WHERE id = $1 AND role = 'komendant' AND is_active")
            .bind(verified.komendant_id)
            .fetch_optional(&state.pool)
            .await?;
    if kom_exists.is_none() {
        return Err(AppError::bad_request("invalid_qr", "Недействительный QR-код"));
    }

    let mut tx = state.pool.begin().await?;
    let open: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM work_sessions
         WHERE student_id = $1 AND status IN ('active','pending_confirmation')
         FOR UPDATE",
    )
    .bind(auth.user.id)
    .fetch_optional(&mut *tx)
    .await?;
    if open.is_some() {
        return Err(AppError::conflict(
            "session_exists",
            "У вас уже есть незавершённая сессия",
        ));
    }

    let session = sqlx::query_as::<_, WorkSession>(
        "INSERT INTO work_sessions (student_id, komendant_id, difficulty, coefficient, started_at, status)
         VALUES ($1,$2,$3,$4,$5,'active') RETURNING *",
    )
    .bind(auth.user.id)
    .bind(verified.komendant_id)
    .bind(verified.difficulty)
    .bind(verified.difficulty.coefficient() as i32)
    .bind(state.clock.now())
    .fetch_one(&mut *tx)
    .await?;

    state
        .audit(
            &mut *tx,
            AuditRecord::new("session_start")
                .actor(auth.user.id, "student")
                .entity("session", session.id)
                .details(json!({"komendant_id": verified.komendant_id, "difficulty": verified.difficulty})),
        )
        .await?;
    state
        .komendant_log(
            &mut *tx,
            verified.komendant_id,
            "session_started",
            Some(auth.user.id),
            Some(session.id),
            Some(json!({"started_at": session.started_at, "difficulty": verified.difficulty})),
        )
        .await?;
    tx.commit().await?;

    let row = sqlx::query_as::<_, SessionRow>(sqlx::AssertSqlSafe(format!("{SESSION_SELECT} WHERE s.id = $1")))
        .bind(session.id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.into()))
}

const MAX_PHOTO_BYTES: usize = 10 * 1024 * 1024;
async fn finish_session(
    State(state): State<AppState>,
    auth: StudentUser,
    mut multipart: Multipart,
) -> AppResult<Json<SessionDto>> {
    let mut qr: Option<String> = None;
    let mut photo: Option<(String, Vec<u8>)> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("qr") => {
                qr = Some(field.text().await.map_err(|_| {
                    AppError::bad_request("bad_multipart", "Некорректные данные формы")
                })?);
            }
            Some("photo") => {
                let mime = field
                    .content_type()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let data = field.bytes().await.map_err(|_| {
                    AppError::bad_request("bad_multipart", "Некорректные данные формы")
                })?;
                photo = Some((mime, data.to_vec()));
            }
            _ => {}
        }
    }
    let qr = qr.ok_or_else(|| AppError::bad_request("missing_qr", "Не передан QR-код"))?;
    let (_mime, photo_bytes) = photo
        .ok_or_else(|| AppError::bad_request("missing_photo", "Не прикреплено фото"))?;
    let ext = domain::sniff_image_ext(&photo_bytes).ok_or_else(|| {
        AppError::bad_request("bad_photo_type", "Фото должно быть jpeg, png, heic или webp")
    })?;
    if photo_bytes.is_empty() || photo_bytes.len() > MAX_PHOTO_BYTES {
        return Err(AppError::bad_request(
            "bad_photo_size",
            "Фото должно быть не более 10 МБ",
        ));
    }

    let verified = qr::verify(&state.config.qr_secret, &qr, state.clock.now())?;

    let session = sqlx::query_as::<_, WorkSession>(
        "SELECT * FROM work_sessions WHERE student_id = $1 AND status = 'active'",
    )
    .bind(auth.user.id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::conflict("no_active_session", "Нет активной сессии"))?;

    if session.komendant_id != verified.komendant_id {
        return Err(AppError::bad_request(
            "qr_komendant_mismatch",
            "QR-код принадлежит другому коменданту",
        ));
    }

    let now = state.clock.now();
    let raw_seconds = (now - session.started_at).num_seconds().max(0);
    let max_secs = state.config.max_session_hours * 3600;

    if raw_seconds > max_secs {
        return expire_finish(state, auth, session, raw_seconds).await;
    }

    let rounded = domain::rounded_minutes(raw_seconds);
    let credited = domain::credited_minutes(rounded, session.coefficient as i64);
    let photo_rel = format!("{}.{}", Uuid::new_v4(), ext);
    let photo_abs = state.config.photo_dir.join(&photo_rel);
    tokio::fs::create_dir_all(&state.config.photo_dir).await?;
    tokio::fs::write(&photo_abs, &photo_bytes)
        .await
        .map_err(AppError::internal)?;

    let mut tx = state.pool.begin().await?;
    let tx_result: AppResult<()> = async {
        let updated = sqlx::query(
            "UPDATE work_sessions SET ended_at = $1, status = 'pending_confirmation',
             raw_seconds = $2, rounded_minutes = $3, credited_minutes = $4, photo_path = $5
             WHERE id = $6 AND status = 'active'",
        )
        .bind(now)
        .bind(raw_seconds)
        .bind(rounded)
        .bind(credited)
        .bind(&photo_rel)
        .bind(session.id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(AppError::conflict(
                "no_active_session",
                "Нет активной сессии",
            ));
        }
        state
            .audit(
                &mut *tx,
                AuditRecord::new("session_finish")
                    .actor(auth.user.id, "student")
                    .entity("session", session.id)
                    .details(json!({"raw_seconds": raw_seconds, "credited_minutes": credited})),
            )
            .await?;
        state
            .komendant_log(
                &mut *tx,
                session.komendant_id,
                "session_finished",
                Some(auth.user.id),
                Some(session.id),
                Some(json!({"ended_at": now, "credited_minutes": credited})),
            )
            .await?;
        state
            .notify(
                &mut tx,
                &[session.komendant_id],
                "session_finished",
                "Сессия ожидает подтверждения",
                &format!("Студент {} завершил сессию, начислено {} мин", auth.user.full_name, credited),
                json!({"session_id": session.id}),
            )
            .await?;
        Ok(())
    }
    .await;
    match tx_result {
        Ok(()) => {
            if let Err(e) = tx.commit().await {
                let _ = tokio::fs::remove_file(&photo_abs).await;
                return Err(e.into());
            }
        }
        Err(e) => {
            drop(tx);
            let _ = tokio::fs::remove_file(&photo_abs).await;
            return Err(e);
        }
    }

    state.push_later(
        vec![session.komendant_id],
        "Сессия ожидает подтверждения".into(),
        format!("Студент {} завершил сессию", auth.user.full_name),
        json!({"session_id": session.id}),
    );

    let row = sqlx::query_as::<_, SessionRow>(sqlx::AssertSqlSafe(format!("{SESSION_SELECT} WHERE s.id = $1")))
        .bind(session.id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.into()))
}

/// Finish attempted after MAX_SESSION_HOURS: session goes to expired, nothing credited.
async fn expire_finish(
    state: AppState,
    auth: StudentUser,
    session: WorkSession,
    raw_seconds: i64,
) -> AppResult<Json<SessionDto>> {
    let now = state.clock.now();
    let mut tx = state.pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE work_sessions SET ended_at = $1, raw_seconds = $2, status = 'expired', expired_at = $1
         WHERE id = $3 AND status = 'active'",
    )
    .bind(now)
    .bind(raw_seconds)
    .bind(session.id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        drop(tx);
        return Err(AppError::conflict(
            "no_active_session",
            "Нет активной сессии",
        ));
    }
    let admins: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM users WHERE role = 'admin' AND is_active")
            .fetch_all(&mut *tx)
            .await?;
    let mut targets = vec![session.komendant_id, auth.user.id];
    targets.extend(admins);
    let body = format!(
        "Сессия студента {} превысила лимит времени и аннулирована",
        auth.user.full_name
    );
    state
        .notify(&mut tx, &targets, "session_expired", "Сессия аннулирована", &body, json!({"session_id": session.id}))
        .await?;
    state
        .audit(
            &mut *tx,
            AuditRecord::new("session_expire")
                .actor(auth.user.id, "student")
                .entity("session", session.id)
                .details(json!({"reason": "finish_after_max", "raw_seconds": raw_seconds})),
        )
        .await?;
    state
        .komendant_log(
            &mut *tx,
            session.komendant_id,
            "session_expired",
            Some(auth.user.id),
            Some(session.id),
            Some(json!({"reason": "finish_after_max", "raw_seconds": raw_seconds})),
        )
        .await?;
    tx.commit().await?;
    state.push_later(targets, "Сессия аннулирована".into(), body, json!({"session_id": session.id}));

    let row = sqlx::query_as::<_, SessionRow>(sqlx::AssertSqlSafe(format!("{SESSION_SELECT} WHERE s.id = $1")))
        .bind(session.id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.into()))
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct ActiveSessionResponse {
    pub session: Option<SessionDto>,
    #[ts(type = "string")]
    pub server_now: chrono::DateTime<chrono::Utc>,
}

async fn active_session(
    State(state): State<AppState>,
    auth: StudentUser,
) -> AppResult<Json<ActiveSessionResponse>> {
    let row = sqlx::query_as::<_, SessionRow>(sqlx::AssertSqlSafe(format!(
        "{SESSION_SELECT} WHERE s.student_id = $1 AND s.status IN ('active','pending_confirmation')"
    )))
    .bind(auth.user.id)
    .fetch_optional(&state.pool)
    .await?;
    Ok(Json(ActiveSessionResponse {
        session: row.map(Into::into),
        server_now: state.clock.now(),
    }))
}

async fn list_sessions(
    State(state): State<AppState>,
    auth: StudentUser,
    Query(p): Query<PageQuery>,
) -> AppResult<Json<Paginated<SessionDto>>> {
    let total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM work_sessions WHERE student_id = $1")
            .bind(auth.user.id)
            .fetch_one(&state.pool)
            .await?;
    let items = sqlx::query_as::<_, SessionRow>(sqlx::AssertSqlSafe(format!(
        "{SESSION_SELECT} WHERE s.student_id = $1 ORDER BY s.started_at DESC LIMIT $2 OFFSET $3"
    )))
    .bind(auth.user.id)
    .bind(p.per_page())
    .bind(p.offset())
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(Paginated {
        items: items.into_iter().map(Into::into).collect(),
        total,
        page: p.page(),
        per_page: p.per_page(),
    }))
}

async fn list_assignments(
    State(state): State<AppState>,
    auth: StudentUser,
    Query(p): Query<PageQuery>,
) -> AppResult<Json<Paginated<AssignmentDto>>> {
    let total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM assignments WHERE student_id = $1")
            .bind(auth.user.id)
            .fetch_one(&state.pool)
            .await?;
    let items = sqlx::query_as::<_, AssignmentRow>(sqlx::AssertSqlSafe(format!(
        "{ASSIGNMENT_SELECT} WHERE a.student_id = $1 ORDER BY a.created_at DESC LIMIT $2 OFFSET $3"
    )))
    .bind(auth.user.id)
    .bind(p.per_page())
    .bind(p.offset())
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(Paginated {
        items: items.into_iter().map(Into::into).collect(),
        total,
        page: p.page(),
        per_page: p.per_page(),
    }))
}

async fn list_orders(
    State(state): State<AppState>,
    auth: StudentUser,
    Query(p): Query<PageQuery>,
) -> AppResult<Json<Paginated<OrderDto>>> {
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM orders WHERE user_id = $1")
        .bind(auth.user.id)
        .fetch_one(&state.pool)
        .await?;
    let items = sqlx::query_as::<_, OrderRow>(sqlx::AssertSqlSafe(format!(
        "{ORDER_SELECT} WHERE o.user_id = $1 ORDER BY o.created_at DESC LIMIT $2 OFFSET $3"
    )))
    .bind(auth.user.id)
    .bind(p.per_page())
    .bind(p.offset())
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(Paginated {
        items: items.into_iter().map(Into::into).collect(),
        total,
        page: p.page(),
        per_page: p.per_page(),
    }))
}

async fn get_order(
    State(state): State<AppState>,
    auth: StudentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<OrderDto>> {
    let row = sqlx::query_as::<_, OrderRow>(sqlx::AssertSqlSafe(format!(
        "{ORDER_SELECT} WHERE o.id = $1 AND o.user_id = $2"
    )))
    .bind(id)
    .bind(auth.user.id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::not_found("Заказ не найден"))?;
    Ok(Json(row.into()))
}
