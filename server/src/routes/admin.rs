use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::{hash_password, random_password, AdminUser};
use crate::domain;
use crate::dto::*;
use crate::error::{AppError, AppResult};
use crate::import;
use crate::models::*;
use crate::rows::*;
use crate::state::{AppState, AuditRecord};
use crate::summaries;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/users", get(list_users).post(create_user))
        .route("/admin/users/import", post(import_users))
        .route("/admin/users/{id}", get(get_user).patch(update_user).delete(delete_user))
        .route("/admin/users/{id}/reset-password", post(reset_password))
        .route("/admin/users/{id}/adjust", post(adjust_user))
        .route("/admin/groups", get(list_groups).post(create_group))
        .route("/admin/groups/{id}", patch(update_group).delete(delete_group))
        .route("/admin/sessions", get(list_sessions))
        .route("/admin/sessions/{id}/photo", get(session_photo))
        .route("/admin/sessions/{id}/photo-review", post(photo_review))
        .route("/admin/rewards", post(create_reward))
        .route(
            "/admin/rewards/{id}",
            patch(update_reward).delete(delete_reward),
        )
        .route("/admin/rewards/{id}/image", post(upload_reward_image))
        .route("/admin/orders", get(list_orders))
        .route("/admin/orders/{id}/issue", post(issue_order))
        .route("/admin/orders/{id}/cancel", post(cancel_order))
        .route("/admin/summaries", get(list_summaries))
        .route("/admin/summaries/generate", post(generate_summary))
        .route("/admin/summaries/{id}", get(get_summary))
        .route("/admin/summaries/{id}/xlsx", get(summary_xlsx))
        .route("/admin/audit-log", get(audit_log))
        .route("/admin/komendant-log", get(komendant_log))
}

// ---------- users ----------

#[derive(Deserialize)]
struct UsersQuery {
    role: Option<Role>,
    group_id: Option<Uuid>,
    q: Option<String>,
    is_active: Option<bool>,
    page: Option<i64>,
    per_page: Option<i64>,
}

async fn list_users(
    State(state): State<AppState>,
    _auth: AdminUser,
    Query(q): Query<UsersQuery>,
) -> AppResult<Json<Paginated<UserDto>>> {
    let p = PageQuery {
        page: q.page,
        per_page: q.per_page,
    };
    let mut cond = String::from("TRUE");
    let mut n = 0i32;
    if q.role.is_some() {
        n += 1;
        cond += &format!(" AND u.role = ${n}");
    }
    if q.group_id.is_some() {
        n += 1;
        cond += &format!(" AND u.group_id = ${n}");
    }
    if q.q.is_some() {
        n += 1;
        cond += &format!(" AND (u.full_name ILIKE ${n} OR u.login ILIKE ${n})");
    }
    if q.is_active.is_some() {
        n += 1;
        cond += &format!(" AND u.is_active = ${n}");
    }
    macro_rules! bind_f {
        ($qb:expr) => {{
            let mut qb = $qb;
            if let Some(v) = q.role {
                qb = qb.bind(v);
            }
            if let Some(v) = q.group_id {
                qb = qb.bind(v);
            }
            if let Some(v) = &q.q {
                qb = qb.bind(format!("%{v}%"));
            }
            if let Some(v) = q.is_active {
                qb = qb.bind(v);
            }
            qb
        }};
    }
    let total: i64 = bind_f!(sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM users u WHERE {cond}"
    ))))
    .fetch_one(&state.pool)
    .await?;

    let items = bind_f!(sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(format!(
        "{USER_SELECT} WHERE {cond} ORDER BY u.full_name LIMIT ${} OFFSET ${}",
        n + 1,
        n + 2
    ))))
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

async fn get_user(
    State(state): State<AppState>,
    _auth: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserDto>> {
    let row = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(format!("{USER_SELECT} WHERE u.id = $1")))
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("Пользователь не найден"))?;
    Ok(Json(row.into()))
}

async fn create_user(
    State(state): State<AppState>,
    auth: AdminUser,
    Json(req): Json<UserCreateRequest>,
) -> AppResult<Json<UserCreateResponse>> {
    if req.login.trim().is_empty() || req.full_name.trim().is_empty() {
        return Err(AppError::bad_request("invalid_input", "Логин и ФИО обязательны"));
    }
    if req.role == Role::Student && req.group_id.is_none() {
        // allowed, but group stays empty
    }
    let generated = req.password.is_none().then(random_password);
    let password = req
        .password
        .clone()
        .or_else(|| generated.clone())
        .unwrap();
    if password.len() < 6 {
        return Err(AppError::bad_request(
            "weak_password",
            "Пароль должен содержать не менее 6 символов",
        ));
    }
    let hash = hash_password(&password)?;

    let mut tx = state.pool.begin().await?;
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (role, login, password_hash, full_name, group_id)
         VALUES ($1,$2,$3,$4,$5) RETURNING *",
    )
    .bind(req.role)
    .bind(req.login.trim())
    .bind(&hash)
    .bind(req.full_name.trim())
    .bind(req.group_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(d) if d.is_unique_violation() => {
            AppError::conflict("login_taken", "Логин уже занят")
        }
        _ => AppError::from(e),
    })?;
    state
        .audit(
            &mut *tx,
            AuditRecord::new("user_create")
                .actor(auth.user.id, "admin")
                .entity("user", user.id)
                .details(json!({"login": user.login, "role": user.role})),
        )
        .await?;
    tx.commit().await?;

    let row = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(format!("{USER_SELECT} WHERE u.id = $1")))
        .bind(user.id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(UserCreateResponse {
        user: row.into(),
        generated_password: generated,
    }))
}

async fn update_user(
    State(state): State<AppState>,
    auth: AdminUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UserUpdateRequest>,
) -> AppResult<Json<UserDto>> {
    let mut tx = state.pool.begin().await?;
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::not_found("Пользователь не найден"))?;

    if let Some(role) = req.role {
        sqlx::query("UPDATE users SET role = $1 WHERE id = $2")
            .bind(role)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(login) = &req.login {
        if login.trim().is_empty() {
            return Err(AppError::bad_request("invalid_input", "Пустой логин"));
        }
        sqlx::query("UPDATE users SET login = $1 WHERE id = $2")
            .bind(login.trim())
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| match &e {
                sqlx::Error::Database(d) if d.is_unique_violation() => {
                    AppError::conflict("login_taken", "Логин уже занят")
                }
                _ => AppError::from(e),
            })?;
    }
    if let Some(name) = &req.full_name {
        sqlx::query("UPDATE users SET full_name = $1 WHERE id = $2")
            .bind(name.trim())
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(group) = req.group_id {
        sqlx::query("UPDATE users SET group_id = $1 WHERE id = $2")
            .bind(group)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(active) = req.is_active {
        if id == auth.user.id && !active {
            return Err(AppError::bad_request(
                "self_deactivate",
                "Нельзя деактивировать себя",
            ));
        }
        sqlx::query("UPDATE users SET is_active = $1 WHERE id = $2")
            .bind(active)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    state
        .audit(
            &mut *tx,
            AuditRecord::new("user_update")
                .actor(auth.user.id, "admin")
                .entity("user", user.id)
                .details(json!({"changes": serde_json::to_value(&req).unwrap_or_default()})),
        )
        .await?;
    tx.commit().await?;

    let row = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(format!("{USER_SELECT} WHERE u.id = $1")))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.into()))
}

async fn delete_user(
    State(state): State<AppState>,
    auth: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    if id == auth.user.id {
        return Err(AppError::bad_request(
            "self_deactivate",
            "Нельзя деактивировать себя",
        ));
    }
    let mut tx = state.pool.begin().await?;
    let res = sqlx::query("UPDATE users SET is_active = FALSE WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::not_found("Пользователь не найден"));
    }
    state
        .audit(
            &mut *tx,
            AuditRecord::new("user_deactivate")
                .actor(auth.user.id, "admin")
                .entity("user", id),
        )
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"ok": true})))
}

async fn reset_password(
    State(state): State<AppState>,
    auth: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ResetPasswordResponse>> {
    let password = random_password();
    let hash = hash_password(&password)?;
    let mut tx = state.pool.begin().await?;
    let res = sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(&hash)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::not_found("Пользователь не найден"));
    }
    state
        .audit(
            &mut *tx,
            AuditRecord::new("password_reset")
                .actor(auth.user.id, "admin")
                .entity("user", id),
        )
        .await?;
    tx.commit().await?;
    Ok(Json(ResetPasswordResponse { password }))
}

async fn adjust_user(
    State(state): State<AppState>,
    auth: AdminUser,
    Path(id): Path<Uuid>,
    Json(req): Json<AdjustRequest>,
) -> AppResult<Json<UserDto>> {
    if req.reason.trim().is_empty() {
        return Err(AppError::bad_request("invalid_reason", "Укажите причину"));
    }
    let mut tx = state.pool.begin().await?;
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::not_found("Пользователь не найден"))?;
    let new_debt = user.debt_minutes + req.debt_delta;
    let new_balance = user.balance_minutes + req.balance_delta;
    if new_debt < 0 || new_balance < 0 {
        return Err(AppError::bad_request(
            "negative_amount",
            "Долг и баланс не могут быть отрицательными",
        ));
    }
    sqlx::query("UPDATE users SET debt_minutes = $1, balance_minutes = $2 WHERE id = $3")
        .bind(new_debt)
        .bind(new_balance)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    state
        .audit(
            &mut *tx,
            AuditRecord::new("user_adjust")
                .actor(auth.user.id, "admin")
                .entity("user", id)
                .details(json!({
                    "debt_delta": req.debt_delta,
                    "balance_delta": req.balance_delta,
                    "reason": req.reason,
                })),
        )
        .await?;
    tx.commit().await?;

    let row = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(format!("{USER_SELECT} WHERE u.id = $1")))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.into()))
}

async fn import_users(
    State(state): State<AppState>,
    auth: AdminUser,
    mut multipart: Multipart,
) -> AppResult<Json<ImportResponse>> {
    let mut file: Option<(String, Vec<u8>)> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") || field.file_name().is_some() {
            let name = field.file_name().unwrap_or("file.csv").to_string();
            let data = field.bytes().await.map_err(|_| {
                AppError::bad_request("bad_multipart", "Некорректные данные формы")
            })?;
            file = Some((name, data.to_vec()));
        }
    }
    let (name, data) =
        file.ok_or_else(|| AppError::bad_request("missing_file", "Файл не передан"))?;
    let results = import::import_users(&state, auth.user.id, &name, &data).await?;
    Ok(Json(ImportResponse { results }))
}

// ---------- groups ----------

async fn list_groups(
    State(state): State<AppState>,
    _auth: AdminUser,
) -> AppResult<Json<Vec<GroupDto>>> {
    let groups = sqlx::query_as::<_, Group>("SELECT * FROM groups ORDER BY name")
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(groups.into_iter().map(Into::into).collect()))
}

#[derive(Deserialize)]
struct GroupUpsert {
    name: String,
}

async fn create_group(
    State(state): State<AppState>,
    auth: AdminUser,
    Json(req): Json<GroupUpsert>,
) -> AppResult<Json<GroupDto>> {
    if req.name.trim().is_empty() {
        return Err(AppError::bad_request("invalid_input", "Пустое название группы"));
    }
    let mut tx = state.pool.begin().await?;
    let g = sqlx::query_as::<_, Group>(
        "INSERT INTO groups (name) VALUES ($1) RETURNING *",
    )
    .bind(req.name.trim())
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(d) if d.is_unique_violation() => {
            AppError::conflict("name_taken", "Группа с таким названием уже существует")
        }
        _ => AppError::from(e),
    })?;
    state
        .audit(
            &mut *tx,
            AuditRecord::new("group_create")
                .actor(auth.user.id, "admin")
                .entity("group", g.id)
                .details(json!({"name": g.name})),
        )
        .await?;
    tx.commit().await?;
    Ok(Json(g.into()))
}

async fn update_group(
    State(state): State<AppState>,
    auth: AdminUser,
    Path(id): Path<Uuid>,
    Json(req): Json<GroupUpsert>,
) -> AppResult<Json<GroupDto>> {
    let mut tx = state.pool.begin().await?;
    let g = sqlx::query_as::<_, Group>(
        "UPDATE groups SET name = $1 WHERE id = $2 RETURNING *",
    )
    .bind(req.name.trim())
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(d) if d.is_unique_violation() => {
            AppError::conflict("name_taken", "Группа с таким названием уже существует")
        }
        _ => AppError::from(e),
    })?
    .ok_or_else(|| AppError::not_found("Группа не найдена"))?;
    state
        .audit(
            &mut *tx,
            AuditRecord::new("group_update")
                .actor(auth.user.id, "admin")
                .entity("group", g.id)
                .details(json!({"name": g.name})),
        )
        .await?;
    tx.commit().await?;
    Ok(Json(g.into()))
}

async fn delete_group(
    State(state): State<AppState>,
    auth: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let mut tx = state.pool.begin().await?;
    let students: i64 =
        sqlx::query_scalar("SELECT count(*) FROM users WHERE group_id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    if students > 0 {
        return Err(AppError::conflict(
            "group_in_use",
            "В группе есть студенты",
        ));
    }
    let res = sqlx::query("DELETE FROM groups WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::not_found("Группа не найдена"));
    }
    state
        .audit(
            &mut *tx,
            AuditRecord::new("group_delete")
                .actor(auth.user.id, "admin")
                .entity("group", id),
        )
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"ok": true})))
}

// ---------- sessions ----------

#[derive(Deserialize)]
struct AdminSessionsQuery {
    status: Option<SessionStatus>,
    photo_review: Option<PhotoReview>,
    student_id: Option<Uuid>,
    komendant_id: Option<Uuid>,
    from: Option<chrono::DateTime<chrono::Utc>>,
    to: Option<chrono::DateTime<chrono::Utc>>,
    page: Option<i64>,
    per_page: Option<i64>,
}

async fn list_sessions(
    State(state): State<AppState>,
    _auth: AdminUser,
    Query(q): Query<AdminSessionsQuery>,
) -> AppResult<Json<Paginated<SessionDto>>> {
    let p = PageQuery {
        page: q.page,
        per_page: q.per_page,
    };
    let mut cond = String::from("TRUE");
    let mut n = 0i32;
    if q.status.is_some() {
        n += 1;
        cond += &format!(" AND s.status = ${n}");
    }
    if q.photo_review.is_some() {
        n += 1;
        cond += &format!(" AND s.photo_review = ${n}");
    }
    if q.student_id.is_some() {
        n += 1;
        cond += &format!(" AND s.student_id = ${n}");
    }
    if q.komendant_id.is_some() {
        n += 1;
        cond += &format!(" AND s.komendant_id = ${n}");
    }
    if q.from.is_some() {
        n += 1;
        cond += &format!(" AND s.started_at >= ${n}");
    }
    if q.to.is_some() {
        n += 1;
        cond += &format!(" AND s.started_at < ${n}");
    }
    macro_rules! bind_filters {
        ($qb:expr) => {{
            let mut qb = $qb;
            if let Some(v) = q.status {
                qb = qb.bind(v);
            }
            if let Some(v) = q.photo_review {
                qb = qb.bind(v);
            }
            if let Some(v) = q.student_id {
                qb = qb.bind(v);
            }
            if let Some(v) = q.komendant_id {
                qb = qb.bind(v);
            }
            if let Some(v) = q.from {
                qb = qb.bind(v);
            }
            if let Some(v) = q.to {
                qb = qb.bind(v);
            }
            qb
        }};
    }
    let total: i64 = bind_filters!(sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM work_sessions s WHERE {cond}"
    ))))
    .fetch_one(&state.pool)
    .await?;
    let items = bind_filters!(sqlx::query_as::<_, SessionRow>(sqlx::AssertSqlSafe(format!(
        "{SESSION_SELECT} WHERE {cond} ORDER BY s.started_at DESC LIMIT ${} OFFSET ${}",
        n + 1,
        n + 2
    ))))
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

async fn session_photo(
    State(state): State<AppState>,
    _auth: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    let session = sqlx::query_as::<_, WorkSession>("SELECT * FROM work_sessions WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("Сессия не найдена"))?;
    let rel = session
        .photo_path
        .as_ref()
        .ok_or_else(|| AppError::not_found("Фото отсутствует"))?;
    if rel.contains('/') || rel.contains("..") {
        return Err(AppError::internal("bad photo path"));
    }
    let ext = rel.rsplit('.').next().unwrap_or("");
    let mime = match ext {
        "png" => "image/png",
        "heic" => "image/heic",
        "webp" => "image/webp",
        _ => "image/jpeg",
    };
    let bytes = tokio::fs::read(state.config.photo_dir.join(rel))
        .await
        .map_err(|_| AppError::not_found("Фото не найдено на диске"))?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, mime.parse().unwrap());
    headers.insert(header::CACHE_CONTROL, "private, no-store".parse().unwrap());
    Ok((StatusCode::OK, headers, bytes).into_response())
}

async fn photo_review(
    State(state): State<AppState>,
    auth: AdminUser,
    Path(id): Path<Uuid>,
    Json(req): Json<PhotoReviewRequest>,
) -> AppResult<Json<SessionDto>> {
    let mut tx = state.pool.begin().await?;
    let session = sqlx::query_as::<_, WorkSession>(
        "SELECT * FROM work_sessions WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::not_found("Сессия не найдена"))?;

    if session.status != SessionStatus::Credited || session.photo_review != Some(PhotoReview::Pending)
    {
        return Err(AppError::conflict(
            "bad_status",
            "Фото этой сессии не ожидает проверки",
        ));
    }

    if req.approved {
        sqlx::query(
            "UPDATE work_sessions SET photo_review = 'approved', photo_review_comment = $1,
             photo_reviewed_by = $2, photo_reviewed_at = $3 WHERE id = $4",
        )
        .bind(&req.comment)
        .bind(auth.user.id)
        .bind(state.clock.now())
        .bind(id)
        .execute(&mut *tx)
        .await?;
    } else {
        // revert credited minutes in the same transaction
        let student = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1 FOR UPDATE")
            .bind(session.student_id)
            .fetch_one(&mut *tx)
            .await?;
        let applied_debt = session.applied_to_debt.unwrap_or(0);
        let applied_balance = session.applied_to_balance.unwrap_or(0);
        let rev = domain::revert(applied_debt, applied_balance, student.balance_minutes);
        sqlx::query(
            "UPDATE users SET balance_minutes = balance_minutes - $1, debt_minutes = debt_minutes + $2
             WHERE id = $3",
        )
        .bind(rev.from_balance)
        .bind(rev.to_debt)
        .bind(student.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE work_sessions SET photo_review = 'rejected', photo_review_comment = $1,
             photo_reviewed_by = $2, photo_reviewed_at = $3,
             reverted_from_balance = $4, reverted_to_debt = $5
             WHERE id = $6",
        )
        .bind(&req.comment)
        .bind(auth.user.id)
        .bind(state.clock.now())
        .bind(rev.from_balance)
        .bind(rev.to_debt)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        state
            .notify(
                &mut tx,
                &[session.student_id, session.komendant_id],
                "photo_rejected",
                "Фото отклонено",
                "Администратор отклонил фото сессии, начисление отменено",
                json!({"session_id": id}),
            )
            .await?;
    }

    state
        .audit(
            &mut *tx,
            AuditRecord::new("photo_review")
                .actor(auth.user.id, "admin")
                .entity("session", id)
                .details(json!({"approved": req.approved, "comment": req.comment})),
        )
        .await?;
    tx.commit().await?;

    if !req.approved {
        state.push_later(
            vec![session.student_id, session.komendant_id],
            "Фото отклонено".into(),
            "Начисление по сессии отменено".into(),
            json!({"session_id": id}),
        );
    }

    let row = sqlx::query_as::<_, SessionRow>(sqlx::AssertSqlSafe(format!("{SESSION_SELECT} WHERE s.id = $1")))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.into()))
}

// ---------- rewards ----------

async fn create_reward(
    State(state): State<AppState>,
    auth: AdminUser,
    Json(req): Json<RewardUpsertRequest>,
) -> AppResult<Json<RewardDto>> {
    if req.title.trim().is_empty() || req.price_minutes < 0 {
        return Err(AppError::bad_request("invalid_input", "Некорректные данные награды"));
    }
    let mut tx = state.pool.begin().await?;
    let r = sqlx::query_as::<_, Reward>(
        "INSERT INTO rewards (title, description, price_minutes, stock, is_active)
         VALUES ($1,$2,$3,$4,$5) RETURNING *",
    )
    .bind(req.title.trim())
    .bind(req.description.clone().unwrap_or_default())
    .bind(req.price_minutes)
    .bind(req.stock)
    .bind(req.is_active.unwrap_or(true))
    .fetch_one(&mut *tx)
    .await?;
    state
        .audit(
            &mut *tx,
            AuditRecord::new("reward_create")
                .actor(auth.user.id, "admin")
                .entity("reward", r.id)
                .details(json!({"title": r.title, "price_minutes": r.price_minutes})),
        )
        .await?;
    tx.commit().await?;
    Ok(Json(super::rewards::reward_dto(r)))
}

async fn update_reward(
    State(state): State<AppState>,
    auth: AdminUser,
    Path(id): Path<Uuid>,
    Json(req): Json<RewardUpsertRequest>,
) -> AppResult<Json<RewardDto>> {
    let mut tx = state.pool.begin().await?;
    let r = sqlx::query_as::<_, Reward>(
        "UPDATE rewards SET title = $1, description = $2, price_minutes = $3,
         stock = $4, is_active = $5 WHERE id = $6 RETURNING *",
    )
    .bind(req.title.trim())
    .bind(req.description.clone().unwrap_or_default())
    .bind(req.price_minutes)
    .bind(req.stock)
    .bind(req.is_active.unwrap_or(true))
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::not_found("Награда не найдена"))?;
    state
        .audit(
            &mut *tx,
            AuditRecord::new("reward_update")
                .actor(auth.user.id, "admin")
                .entity("reward", r.id)
                .details(json!({"title": r.title})),
        )
        .await?;
    tx.commit().await?;
    Ok(Json(super::rewards::reward_dto(r)))
}

async fn delete_reward(
    State(state): State<AppState>,
    auth: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let mut tx = state.pool.begin().await?;
    let res = sqlx::query("DELETE FROM rewards WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::not_found("Награда не найдена"));
    }
    state
        .audit(
            &mut *tx,
            AuditRecord::new("reward_delete")
                .actor(auth.user.id, "admin")
                .entity("reward", id),
        )
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"ok": true})))
}

async fn upload_reward_image(
    State(state): State<AppState>,
    auth: AdminUser,
    Path(id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<Json<RewardDto>> {
    let mut img: Option<(String, Vec<u8>)> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.file_name().is_some() || field.name() == Some("image") {
            let mime = field.content_type().unwrap_or("").to_string();
            let data = field.bytes().await.map_err(|_| {
                AppError::bad_request("bad_multipart", "Некорректные данные формы")
            })?;
            img = Some((mime, data.to_vec()));
        }
    }
    let (_mime, bytes) =
        img.ok_or_else(|| AppError::bad_request("missing_file", "Изображение не передано"))?;
    let ext = crate::domain::sniff_image_ext(&bytes).ok_or_else(|| {
        AppError::bad_request("bad_photo_type", "Изображение должно быть jpeg, png, heic или webp")
    })?;
    if bytes.len() > 10 * 1024 * 1024 {
        return Err(AppError::bad_request("bad_photo_size", "Файл больше 10 МБ"));
    }

    let rel = format!("{}.{}", Uuid::new_v4(), ext);
    let dir = state.config.photo_dir.join("rewards");
    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::write(dir.join(&rel), &bytes).await?;

    let mut tx = state.pool.begin().await?;
    let r = sqlx::query_as::<_, Reward>(
        "UPDATE rewards SET image_path = $1 WHERE id = $2 RETURNING *",
    )
    .bind(&rel)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::not_found("Награда не найдена"))?;
    state
        .audit(
            &mut *tx,
            AuditRecord::new("reward_image")
                .actor(auth.user.id, "admin")
                .entity("reward", r.id),
        )
        .await?;
    tx.commit().await?;
    Ok(Json(super::rewards::reward_dto(r)))
}

// ---------- orders ----------

#[derive(Deserialize)]
struct OrdersQuery {
    status: Option<OrderStatus>,
    user_id: Option<Uuid>,
    page: Option<i64>,
    per_page: Option<i64>,
}

async fn list_orders(
    State(state): State<AppState>,
    _auth: AdminUser,
    Query(q): Query<OrdersQuery>,
) -> AppResult<Json<Paginated<OrderDto>>> {
    let p = PageQuery {
        page: q.page,
        per_page: q.per_page,
    };
    let mut cond = String::from("TRUE");
    let mut n = 0i32;
    if q.status.is_some() {
        n += 1;
        cond += &format!(" AND o.status = ${n}");
    }
    if q.user_id.is_some() {
        n += 1;
        cond += &format!(" AND o.user_id = ${n}");
    }
    macro_rules! bind_f {
        ($qb:expr) => {{
            let mut qb = $qb;
            if let Some(v) = q.status {
                qb = qb.bind(v);
            }
            if let Some(v) = q.user_id {
                qb = qb.bind(v);
            }
            qb
        }};
    }
    let total: i64 = bind_f!(sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM orders o WHERE {cond}"
    ))))
    .fetch_one(&state.pool)
    .await?;
    let items = bind_f!(sqlx::query_as::<_, OrderRow>(sqlx::AssertSqlSafe(format!(
        "{ORDER_SELECT} WHERE {cond} ORDER BY o.created_at DESC LIMIT ${} OFFSET ${}",
        n + 1,
        n + 2
    ))))
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

async fn issue_order(
    State(state): State<AppState>,
    auth: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<OrderDto>> {
    let mut tx = state.pool.begin().await?;
    let order = sqlx::query_as::<_, Order>("SELECT * FROM orders WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::not_found("Заказ не найден"))?;
    if order.status != OrderStatus::Pending {
        return Err(AppError::conflict("bad_status", "Заказ уже обработан"));
    }
    sqlx::query(
        "UPDATE orders SET status = 'issued', issued_by = $1, issued_at = $2 WHERE id = $3",
    )
    .bind(auth.user.id)
    .bind(state.clock.now())
    .bind(id)
    .execute(&mut *tx)
    .await?;
    state
        .audit(
            &mut *tx,
            AuditRecord::new("order_issue")
                .actor(auth.user.id, "admin")
                .entity("order", id),
        )
        .await?;
    state
        .notify(
            &mut tx,
            &[order.user_id],
            "order_issued",
            "Заказ выдан",
            "Ваш заказ выдан",
            json!({"order_id": id}),
        )
        .await?;
    tx.commit().await?;
    state.push_later(
        vec![order.user_id],
        "Заказ выдан".into(),
        "Ваш заказ выдан".into(),
        json!({"order_id": id}),
    );
    let row = sqlx::query_as::<_, OrderRow>(sqlx::AssertSqlSafe(format!("{ORDER_SELECT} WHERE o.id = $1")))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.into()))
}

async fn cancel_order(
    State(state): State<AppState>,
    auth: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<OrderDto>> {
    let mut tx = state.pool.begin().await?;
    let order = sqlx::query_as::<_, Order>("SELECT * FROM orders WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::not_found("Заказ не найден"))?;
    if order.status != OrderStatus::Pending {
        return Err(AppError::conflict("bad_status", "Заказ уже обработан"));
    }
    // refund balance and restore stock
    sqlx::query("UPDATE users SET balance_minutes = balance_minutes + $1 WHERE id = $2")
        .bind(order.price_minutes)
        .bind(order.user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE rewards SET stock = stock + 1 WHERE id = $1 AND stock IS NOT NULL")
        .bind(order.reward_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE orders SET status = 'cancelled' WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    state
        .audit(
            &mut *tx,
            AuditRecord::new("order_cancel")
                .actor(auth.user.id, "admin")
                .entity("order", id),
        )
        .await?;
    state
        .notify(
            &mut tx,
            &[order.user_id],
            "order_cancelled",
            "Заказ отменён",
            "Ваш заказ отменён, минуты возвращены на баланс",
            json!({"order_id": id}),
        )
        .await?;
    tx.commit().await?;
    state.push_later(
        vec![order.user_id],
        "Заказ отменён".into(),
        "Ваш заказ отменён, минуты возвращены".into(),
        json!({"order_id": id}),
    );
    let row = sqlx::query_as::<_, OrderRow>(sqlx::AssertSqlSafe(format!("{ORDER_SELECT} WHERE o.id = $1")))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.into()))
}

// ---------- summaries ----------

async fn list_summaries(
    State(state): State<AppState>,
    _auth: AdminUser,
) -> AppResult<Json<Vec<SummaryDto>>> {
    let rows = sqlx::query_as::<_, MonthlySummary>(
        "SELECT * FROM monthly_summaries ORDER BY year DESC, month DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|s| SummaryDto {
                id: s.id,
                year: s.year,
                month: s.month,
                generated_at: s.generated_at,
                rows: None,
            })
            .collect(),
    ))
}

async fn get_summary(
    State(state): State<AppState>,
    _auth: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<SummaryDto>> {
    let s = sqlx::query_as::<_, MonthlySummary>("SELECT * FROM monthly_summaries WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("Сводка не найдена"))?;
    let rows = sqlx::query_as::<_, SummaryRow>(
        "SELECT * FROM summary_rows WHERE summary_id = $1 ORDER BY group_name NULLS LAST, full_name",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(SummaryDto {
        id: s.id,
        year: s.year,
        month: s.month,
        generated_at: s.generated_at,
        rows: Some(
            rows.into_iter()
                .map(|r| SummaryRowDto {
                    id: r.id,
                    student_id: r.student_id,
                    full_name: r.full_name,
                    group_name: r.group_name,
                    sessions_count: r.sessions_count,
                    rounded_minutes: r.rounded_minutes,
                    credited_minutes: r.credited_minutes,
                    assigned_minutes: r.assigned_minutes,
                    debt_minutes: r.debt_minutes,
                    balance_minutes: r.balance_minutes,
                })
                .collect(),
        ),
    }))
}

async fn generate_summary(
    State(state): State<AppState>,
    auth: AdminUser,
    Json(req): Json<GenerateSummaryRequest>,
) -> AppResult<Json<SummaryDto>> {
    if !(1..=12).contains(&req.month) {
        return Err(AppError::bad_request("invalid_month", "Месяц должен быть 1-12"));
    }
    let id = summaries::generate(&state, req.year, req.month, Some(auth.user.id)).await?;
    let s = sqlx::query_as::<_, MonthlySummary>("SELECT * FROM monthly_summaries WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(SummaryDto {
        id: s.id,
        year: s.year,
        month: s.month,
        generated_at: s.generated_at,
        rows: None,
    }))
}

async fn summary_xlsx(
    State(state): State<AppState>,
    _auth: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    let s = sqlx::query_as::<_, MonthlySummary>("SELECT * FROM monthly_summaries WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("Сводка не найдена"))?;
    let rows = sqlx::query_as::<_, SummaryRow>(
        "SELECT * FROM summary_rows WHERE summary_id = $1 ORDER BY group_name NULLS LAST, full_name",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let bytes = summaries::to_xlsx(s.year, s.month, &rows)
        .map_err(|e| AppError::internal(format!("xlsx: {e}")))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            .parse()
            .unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=summary-{}-{:02}.xlsx", s.year, s.month)
            .parse()
            .unwrap(),
    );
    Ok((StatusCode::OK, headers, bytes).into_response())
}

// ---------- logs ----------

#[derive(Deserialize)]
struct AuditQuery {
    actor_id: Option<Uuid>,
    action: Option<String>,
    from: Option<chrono::DateTime<chrono::Utc>>,
    to: Option<chrono::DateTime<chrono::Utc>>,
    page: Option<i64>,
    per_page: Option<i64>,
}

async fn audit_log(
    State(state): State<AppState>,
    _auth: AdminUser,
    Query(q): Query<AuditQuery>,
) -> AppResult<Json<Paginated<AuditEntryDto>>> {
    let p = PageQuery {
        page: q.page,
        per_page: q.per_page,
    };
    let mut cond = String::from("TRUE");
    let mut n = 0i32;
    if q.actor_id.is_some() {
        n += 1;
        cond += &format!(" AND a.actor_id = ${n}");
    }
    if q.action.is_some() {
        n += 1;
        cond += &format!(" AND a.action = ${n}");
    }
    if q.from.is_some() {
        n += 1;
        cond += &format!(" AND a.ts >= ${n}");
    }
    if q.to.is_some() {
        n += 1;
        cond += &format!(" AND a.ts < ${n}");
    }
    macro_rules! bind_f {
        ($qb:expr) => {{
            let mut qb = $qb;
            if let Some(v) = q.actor_id {
                qb = qb.bind(v);
            }
            if let Some(v) = &q.action {
                qb = qb.bind(v);
            }
            if let Some(v) = q.from {
                qb = qb.bind(v);
            }
            if let Some(v) = q.to {
                qb = qb.bind(v);
            }
            qb
        }};
    }
    let total: i64 = bind_f!(sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM audit_log a WHERE {cond}"
    ))))
    .fetch_one(&state.pool)
    .await?;

    #[derive(sqlx::FromRow)]
    struct AuditRow {
        #[sqlx(flatten)]
        a: AuditEntry,
        actor_login: Option<String>,
    }

    let items = bind_f!(sqlx::query_as::<_, AuditRow>(sqlx::AssertSqlSafe(format!(
        "SELECT a.*, u.login AS actor_login FROM audit_log a
         LEFT JOIN users u ON u.id = a.actor_id
         WHERE {cond} ORDER BY a.ts DESC LIMIT ${} OFFSET ${}",
        n + 1,
        n + 2
    ))))
    .bind(p.per_page())
    .bind(p.offset())
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(Paginated {
        items: items
            .into_iter()
            .map(|r| AuditEntryDto {
                id: r.a.id,
                ts: r.a.ts,
                actor_id: r.a.actor_id,
                actor_login: r.actor_login,
                actor_role: r.a.actor_role,
                action: r.a.action,
                entity_type: r.a.entity_type,
                entity_id: r.a.entity_id,
                details: r.a.details,
                ip: r.a.ip,
            })
            .collect(),
        total,
        page: p.page(),
        per_page: p.per_page(),
    }))
}

#[derive(Deserialize)]
struct KomLogQuery {
    komendant_id: Option<Uuid>,
    from: Option<chrono::DateTime<chrono::Utc>>,
    to: Option<chrono::DateTime<chrono::Utc>>,
    page: Option<i64>,
    per_page: Option<i64>,
}

async fn komendant_log(
    State(state): State<AppState>,
    _auth: AdminUser,
    Query(q): Query<KomLogQuery>,
) -> AppResult<Json<Paginated<KomendantLogEntryDto>>> {
    let p = PageQuery {
        page: q.page,
        per_page: q.per_page,
    };
    let mut cond = String::from("TRUE");
    let mut n = 0i32;
    if q.komendant_id.is_some() {
        n += 1;
        cond += &format!(" AND l.komendant_id = ${n}");
    }
    if q.from.is_some() {
        n += 1;
        cond += &format!(" AND l.ts >= ${n}");
    }
    if q.to.is_some() {
        n += 1;
        cond += &format!(" AND l.ts < ${n}");
    }
    macro_rules! bind_f {
        ($qb:expr) => {{
            let mut qb = $qb;
            if let Some(v) = q.komendant_id {
                qb = qb.bind(v);
            }
            if let Some(v) = q.from {
                qb = qb.bind(v);
            }
            if let Some(v) = q.to {
                qb = qb.bind(v);
            }
            qb
        }};
    }
    let total: i64 = bind_f!(sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM komendant_log l WHERE {cond}"
    ))))
    .fetch_one(&state.pool)
    .await?;

    #[derive(sqlx::FromRow)]
    struct KomRow {
        #[sqlx(flatten)]
        l: KomendantLogEntry,
        komendant_name: Option<String>,
        student_name: Option<String>,
    }

    let items = bind_f!(sqlx::query_as::<_, KomRow>(sqlx::AssertSqlSafe(format!(
        "SELECT l.*, u.full_name AS komendant_name, s.full_name AS student_name
         FROM komendant_log l
         LEFT JOIN users u ON u.id = l.komendant_id
         LEFT JOIN users s ON s.id = l.student_id
         WHERE {cond} ORDER BY l.ts DESC LIMIT ${} OFFSET ${}",
        n + 1,
        n + 2
    ))))
    .bind(p.per_page())
    .bind(p.offset())
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(Paginated {
        items: items
            .into_iter()
            .map(|r| KomendantLogEntryDto {
                id: r.l.id,
                ts: r.l.ts,
                komendant_id: r.l.komendant_id,
                komendant_name: r.komendant_name,
                action: r.l.action,
                student_id: r.l.student_id,
                student_name: r.student_name,
                session_id: r.l.session_id,
                details: r.l.details,
            })
            .collect(),
        total,
        page: p.page(),
        per_page: p.per_page(),
    }))
}
