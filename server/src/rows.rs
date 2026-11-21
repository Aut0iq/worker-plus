use sqlx::FromRow;

use crate::dto::{AssignmentDto, GroupDto, OrderDto, SessionDto, UserDto};
use crate::models::{Assignment, Group, Order, User, WorkSession};

pub const SESSION_SELECT: &str = "
    SELECT s.*, st.full_name AS student_name, g.name AS group_name,
           k.full_name AS komendant_name
    FROM work_sessions s
    JOIN users st ON st.id = s.student_id
    LEFT JOIN groups g ON g.id = st.group_id
    JOIN users k ON k.id = s.komendant_id";

#[derive(FromRow)]
pub struct SessionRow {
    #[sqlx(flatten)]
    pub s: WorkSession,
    pub student_name: String,
    pub group_name: Option<String>,
    pub komendant_name: String,
}

impl From<SessionRow> for SessionDto {
    fn from(r: SessionRow) -> Self {
        let s = r.s;
        SessionDto {
            id: s.id,
            student_id: s.student_id,
            student_name: Some(r.student_name),
            group_name: r.group_name,
            komendant_id: s.komendant_id,
            komendant_name: Some(r.komendant_name),
            difficulty: s.difficulty,
            coefficient: s.coefficient,
            started_at: s.started_at,
            ended_at: s.ended_at,
            status: s.status,
            raw_seconds: s.raw_seconds,
            rounded_minutes: s.rounded_minutes,
            credited_minutes: s.credited_minutes,
            applied_to_debt: s.applied_to_debt,
            applied_to_balance: s.applied_to_balance,
            photo_review: s.photo_review,
            photo_review_comment: s.photo_review_comment,
            confirmed_at: s.confirmed_at,
            rejected_reason: s.rejected_reason,
            has_photo: s.photo_path.is_some(),
            created_at: s.created_at,
        }
    }
}

#[derive(FromRow)]
pub struct UserRow {
    #[sqlx(flatten)]
    pub u: User,
    pub group_name: Option<String>,
}

pub const USER_SELECT: &str = "
    SELECT u.*, g.name AS group_name
    FROM users u LEFT JOIN groups g ON g.id = u.group_id";

impl From<UserRow> for UserDto {
    fn from(r: UserRow) -> Self {
        UserDto::of(r.u, r.group_name)
    }
}

pub const ASSIGNMENT_SELECT: &str = "
    SELECT a.*, st.full_name AS student_name, t.full_name AS assigned_by_name
    FROM assignments a
    JOIN users st ON st.id = a.student_id
    JOIN users t ON t.id = a.assigned_by";

#[derive(FromRow)]
pub struct AssignmentRow {
    #[sqlx(flatten)]
    pub a: Assignment,
    pub student_name: String,
    pub assigned_by_name: String,
}

impl From<AssignmentRow> for AssignmentDto {
    fn from(r: AssignmentRow) -> Self {
        AssignmentDto {
            id: r.a.id,
            student_id: r.a.student_id,
            student_name: Some(r.student_name),
            assigned_by: r.a.assigned_by,
            assigned_by_name: Some(r.assigned_by_name),
            minutes: r.a.minutes,
            reason: r.a.reason,
            created_at: r.a.created_at,
        }
    }
}

pub const ORDER_SELECT: &str = "
    SELECT o.*, u.full_name AS user_name, r.title AS reward_title
    FROM orders o
    JOIN users u ON u.id = o.user_id
    JOIN rewards r ON r.id = o.reward_id";

#[derive(FromRow)]
pub struct OrderRow {
    #[sqlx(flatten)]
    pub o: Order,
    pub user_name: String,
    pub reward_title: String,
}

impl From<OrderRow> for OrderDto {
    fn from(r: OrderRow) -> Self {
        OrderDto {
            id: r.o.id,
            user_id: r.o.user_id,
            user_name: Some(r.user_name),
            reward_id: r.o.reward_id,
            reward_title: Some(r.reward_title),
            price_minutes: r.o.price_minutes,
            status: r.o.status,
            issued_by: r.o.issued_by,
            issued_at: r.o.issued_at,
            created_at: r.o.created_at,
        }
    }
}

pub const GROUP_SELECT: &str = "
    SELECT g.*,
           (SELECT count(*) FROM users u
             WHERE u.group_id = g.id AND u.role = 'student' AND u.is_active) AS student_count
    FROM groups g";

#[derive(FromRow)]
pub struct GroupRow {
    #[sqlx(flatten)]
    pub g: Group,
    pub student_count: i64,
}

impl From<GroupRow> for GroupDto {
    fn from(r: GroupRow) -> Self {
        GroupDto::of(r.g, r.student_count)
    }
}
