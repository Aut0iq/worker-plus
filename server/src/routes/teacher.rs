use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;
use ts_rs::TS;
use uuid::Uuid;

use crate::auth::TeacherUser;
use crate::domain;
use crate::dto::{
    AssignmentCreateRequest, AssignmentDto, GroupDto, Paginated, SessionDto, UserDto,
};
use crate::error::{AppError, AppResult};
use crate::models::Assignment;
use crate::rows::{
    AssignmentRow, GroupRow, SessionRow, UserRow, ASSIGNMENT_SELECT, GROUP_SELECT,
    SESSION_SELECT, USER_SELECT,
};
use crate::state::{AppState, AuditRecord};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/groups", get(list_groups))
        .route("/groups/{id}/students", get(group_students))
        .route("/students/debtors", get(debtors))
        .route("/students/{id}", get(student_profile))
        .route("/students/{id}/assignments", post(assign_hours))
}

async fn list_groups(
    State(state): State<AppState>,
    _auth: TeacherUser,
) -> AppResult<Json<Vec<GroupDto>>> {
    let groups = sqlx::query_as::<_, GroupRow>(sqlx::AssertSqlSafe(format!(
        "{GROUP_SELECT} ORDER BY g.name"
    )))
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(groups.into_iter().map(Into::into).collect()))
}

async fn group_students(
    State(state): State<AppState>,
    _auth: TeacherUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<UserDto>>> {
    let rows = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(format!(
        "{USER_SELECT} WHERE u.group_id = $1 AND u.role = 'student' AND u.is_active ORDER BY u.full_name"
    )))
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

#[derive(serde::Deserialize)]
struct DebtorsQuery {
    group_id: Option<Uuid>,
    page: Option<i64>,
    per_page: Option<i64>,
}

/// Students who still owe hours, largest debt first (teacher's "должники").
async fn debtors(
    State(state): State<AppState>,
    _auth: TeacherUser,
    Query(q): Query<DebtorsQuery>,
) -> AppResult<Json<Paginated<UserDto>>> {
    let p = crate::dto::PageQuery {
        page: q.page,
        per_page: q.per_page,
    };
    let cond = "u.role = 'student' AND u.is_active AND u.debt_minutes > 0
                AND ($1::uuid IS NULL OR u.group_id = $1)";
    let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM users u WHERE {cond}"
    )))
    .bind(q.group_id)
    .fetch_one(&state.pool)
    .await?;
    let rows = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(format!(
        "{USER_SELECT} WHERE {cond} ORDER BY u.debt_minutes DESC, u.full_name LIMIT $2 OFFSET $3"
    )))
    .bind(q.group_id)
    .bind(p.per_page())
    .bind(p.offset())
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(Paginated {
        items: rows.into_iter().map(Into::into).collect(),
        total,
        page: p.page(),
        per_page: p.per_page(),
    }))
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../shared/api-types/")]
pub struct StudentProfileDto {
    pub user: UserDto,
    pub recent_sessions: Vec<SessionDto>,
    pub assignments: Vec<AssignmentDto>,
}

async fn student_profile(
    State(state): State<AppState>,
    _auth: TeacherUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<StudentProfileDto>> {
    let row = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(format!(
        "{USER_SELECT} WHERE u.id = $1 AND u.role = 'student'"
    )))
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::not_found("Студент не найден"))?;

    let sessions = sqlx::query_as::<_, SessionRow>(sqlx::AssertSqlSafe(format!(
        "{SESSION_SELECT} WHERE s.student_id = $1 ORDER BY s.started_at DESC LIMIT 20"
    )))
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    let assignments = sqlx::query_as::<_, AssignmentRow>(sqlx::AssertSqlSafe(format!(
        "{ASSIGNMENT_SELECT} WHERE a.student_id = $1 ORDER BY a.created_at DESC LIMIT 50"
    )))
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(StudentProfileDto {
        user: row.into(),
        recent_sessions: sessions.into_iter().map(Into::into).collect(),
        assignments: assignments.into_iter().map(Into::into).collect(),
    }))
}

async fn assign_hours(
    State(state): State<AppState>,
    auth: TeacherUser,
    Path(id): Path<Uuid>,
    Json(req): Json<AssignmentCreateRequest>,
) -> AppResult<Json<AssignmentDto>> {
    if req.minutes <= 0 || req.minutes > domain::MAX_ASSIGN_MINUTES {
        return Err(AppError::bad_request(
            "invalid_minutes",
            format!(
                "Можно назначить от 1 минуты до {} за раз",
                domain::fmt_minutes(domain::MAX_ASSIGN_MINUTES)
            ),
        ));
    }
    // the reason is optional (teachers assign hours only); stored as '' when absent
    let reason = req.reason.as_deref().map(str::trim).unwrap_or("");

    let mut tx = state.pool.begin().await?;
    let exists: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE id = $1 AND role = 'student' FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    if exists.is_none() {
        return Err(AppError::not_found("Студент не найден"));
    }

    let assignment = sqlx::query_as::<_, Assignment>(
        "INSERT INTO assignments (student_id, assigned_by, minutes, reason)
         VALUES ($1,$2,$3,$4) RETURNING *",
    )
    .bind(id)
    .bind(auth.user.id)
    .bind(req.minutes)
    .bind(reason)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("UPDATE users SET debt_minutes = debt_minutes + $1 WHERE id = $2")
        .bind(req.minutes)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    state
        .audit(
            &mut *tx,
            AuditRecord::new("assignment_create")
                .actor(auth.user.id, auth.user.role.as_str())
                .entity("assignment", assignment.id)
                .details(json!({"student_id": id, "minutes": req.minutes, "reason": reason})),
        )
        .await?;
    let mut body = format!("Вам назначено {}", domain::fmt_minutes(req.minutes));
    if !reason.is_empty() {
        body = format!("{body}: {reason}");
    }
    state
        .notify(
            &mut tx,
            &[id],
            "assignment",
            "Назначены часы",
            &body,
            json!({"assignment_id": assignment.id}),
        )
        .await?;
    tx.commit().await?;

    state.push_later(
        vec![id],
        "Назначены часы".into(),
        body,
        json!({"assignment_id": assignment.id}),
    );

    let row = sqlx::query_as::<_, AssignmentRow>(sqlx::AssertSqlSafe(format!(
        "{ASSIGNMENT_SELECT} WHERE a.id = $1"
    )))
    .bind(assignment.id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row.into()))
}
