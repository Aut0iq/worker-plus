use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;
use ts_rs::TS;
use uuid::Uuid;

use crate::auth::TeacherUser;
use crate::dto::{AssignmentCreateRequest, AssignmentDto, GroupDto, SessionDto, UserDto};
use crate::error::{AppError, AppResult};
use crate::models::{Assignment, Group};
use crate::rows::{
    AssignmentRow, SessionRow, UserRow, ASSIGNMENT_SELECT, SESSION_SELECT, USER_SELECT,
};
use crate::state::{AppState, AuditRecord};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/groups", get(list_groups))
        .route("/groups/{id}/students", get(group_students))
        .route("/students/{id}", get(student_profile))
        .route("/students/{id}/assignments", post(assign_hours))
}

async fn list_groups(
    State(state): State<AppState>,
    _auth: TeacherUser,
) -> AppResult<Json<Vec<GroupDto>>> {
    let groups = sqlx::query_as::<_, Group>("SELECT * FROM groups ORDER BY name")
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
    if req.minutes <= 0 {
        return Err(AppError::bad_request(
            "invalid_minutes",
            "Количество минут должно быть положительным",
        ));
    }
    if req.reason.trim().is_empty() {
        return Err(AppError::bad_request("invalid_reason", "Укажите причину"));
    }

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
    .bind(req.reason.trim())
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
                .details(json!({"student_id": id, "minutes": req.minutes, "reason": req.reason})),
        )
        .await?;
    state
        .notify(
            &mut tx,
            &[id],
            "assignment",
            "Назначены часы",
            &format!("Вам назначено {} мин: {}", req.minutes, req.reason.trim()),
            json!({"assignment_id": assignment.id}),
        )
        .await?;
    tx.commit().await?;

    state.push_later(
        vec![id],
        "Назначены часы".into(),
        format!("Вам назначено {} мин", req.minutes),
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
