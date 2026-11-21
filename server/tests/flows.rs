mod common;

use axum::http::StatusCode;
use chrono::{DateTime, TimeZone, Utc};
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use common::*;

async fn setup(app: &common::TestApp, pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
    let group = create_group(pool, "ИВТ-21").await;
    let admin = create_user(pool, "admin", "admin1", "Админ", None).await;
    let _teacher = create_user(pool, "teacher", "teach1", "Учитель", None).await;
    let kom = create_user(pool, "komendant", "kom1", "Комендант", None).await;
    let student = create_user(pool, "student", "stud1", "Студент Иван", Some(group)).await;
    let _ = app;
    (admin, _teacher, kom, student)
}

#[sqlx::test]
async fn full_flow_start_finish_confirm_partial_debt(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    let (_admin, _teacher, kom, student) = setup(&app, &pool).await;
    set_debt_balance(&pool, student, 90, 0).await;

    let tok_student = login(&app.app, "stud1").await;
    let tok_kom = login(&app.app, "kom1").await;
    let tok_teacher = login(&app.app, "teach1").await;

    // teacher assigns extra debt: 90 -> 150
    let (s, v) = call(
        &app.app,
        "POST",
        &format!("/api/students/{student}/assignments"),
        Some(&tok_teacher),
        Some(json!({"minutes": 60, "reason": "нарушение"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let debt: i64 = sqlx::query_scalar("SELECT debt_minutes FROM users WHERE id = $1")
        .bind(student)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(debt, 150);

    // komendant opens hard QR
    let qr = open_qr(&app.app, &tok_kom, "hard").await;
    let session = start_session(&app.app, &tok_student, &qr).await;
    assert_eq!(session["status"], "active");
    assert_eq!(session["coefficient"], 2);

    // 1h43m31s later -> rounded 104, credited 208
    app.clock.advance_seconds(3600 + 43 * 60 + 31);
    let qr2 = open_qr(&app.app, &tok_kom, "light").await; // difficulty ignored at finish
    let (s, v) = finish_session(&app.app, &tok_student, &qr2).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["status"], "pending_confirmation");
    assert_eq!(v["raw_seconds"], 6211);
    assert_eq!(v["rounded_minutes"], 104);
    assert_eq!(v["credited_minutes"], 208);

    // duplicate open session impossible while pending
    let qr3 = open_qr(&app.app, &tok_kom, "light").await;
    let (s, _) = call(
        &app.app,
        "POST",
        "/api/student/sessions/start",
        Some(&tok_student),
        Some(json!({"qr": qr3})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);

    let sid = v["id"].as_str().unwrap();
    let (s, v) = call(
        &app.app,
        "POST",
        &format!("/api/komendant/sessions/{sid}/confirm"),
        Some(&tok_kom),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["status"], "credited");
    assert_eq!(v["applied_to_debt"], 150);
    assert_eq!(v["applied_to_balance"], 58);
    assert_eq!(v["photo_review"], "pending");

    let (debt, balance): (i64, i64) =
        sqlx::query_as("SELECT debt_minutes, balance_minutes FROM users WHERE id = $1")
            .bind(student)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((debt, balance), (0, 58));
    let _ = kom;
}

#[sqlx::test]
async fn finish_after_max_hours_expires_and_notifies(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    let (admin, _t, kom, student) = setup(&app, &pool).await;
    let tok_student = login(&app.app, "stud1").await;
    let tok_kom = login(&app.app, "kom1").await;

    let qr = open_qr(&app.app, &tok_kom, "light").await;
    start_session(&app.app, &tok_student, &qr).await;

    app.clock.advance_seconds(4 * 3600 + 5);
    let qr2 = open_qr(&app.app, &tok_kom, "light").await;
    let (s, v) = finish_session(&app.app, &tok_student, &qr2).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["status"], "expired");
    assert!(v["credited_minutes"].is_null());

    for uid in [student, kom, admin] {
        let n: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM notifications WHERE user_id = $1 AND kind = 'session_expired'",
        )
        .bind(uid)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 1, "no expired notification for {uid}");
    }
}

#[sqlx::test]
async fn background_expiry_job(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    let (_a, _t, kom, student) = setup(&app, &pool).await;
    let tok_student = login(&app.app, "stud1").await;
    let tok_kom = login(&app.app, "kom1").await;

    let qr = open_qr(&app.app, &tok_kom, "light").await;
    let s = start_session(&app.app, &tok_student, &qr).await;
    let sid: Uuid = s["id"].as_str().unwrap().parse().unwrap();

    app.clock.advance_seconds(5 * 3600);
    let n = prila_server::jobs::expire_overdue_sessions(&app.state)
        .await
        .unwrap();
    assert_eq!(n, 1);
    // idempotent
    let n2 = prila_server::jobs::expire_overdue_sessions(&app.state)
        .await
        .unwrap();
    assert_eq!(n2, 0);

    let (status, expired_at): (String, Option<DateTime<Utc>>) =
        sqlx::query_as("SELECT status::text, expired_at FROM work_sessions WHERE id = $1")
            .bind(sid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "expired");
    assert!(expired_at.is_some());

    let notif: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notifications WHERE user_id = $1 AND kind = 'session_expired'",
    )
    .bind(student)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(notif, 1);
    let _ = kom;
}

#[sqlx::test]
async fn komendant_reject_session(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    let (_a, _t, _kom, student) = setup(&app, &pool).await;
    let tok_student = login(&app.app, "stud1").await;
    let tok_kom = login(&app.app, "kom1").await;

    let qr = open_qr(&app.app, &tok_kom, "light").await;
    let _s = start_session(&app.app, &tok_student, &qr).await;
    app.clock.advance_seconds(3600);
    let qr2 = open_qr(&app.app, &tok_kom, "light").await;
    let (_s, v) = finish_session(&app.app, &tok_student, &qr2).await;
    let sid = v["id"].as_str().unwrap();

    let (st, v) = call(
        &app.app,
        "POST",
        &format!("/api/komendant/sessions/{sid}/reject"),
        Some(&tok_kom),
        Some(json!({"reason": "фото нечитаемо"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["status"], "komendant_rejected");
    let _ = student;
}

#[sqlx::test]
async fn photo_review_reject_reverts(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    let (_a, _t, kom, student) = setup(&app, &pool).await;
    set_debt_balance(&pool, student, 50, 0).await;
    let tok_student = login(&app.app, "stud1").await;
    let tok_kom = login(&app.app, "kom1").await;
    let tok_admin = login(&app.app, "admin1").await;

    let qr = open_qr(&app.app, &tok_kom, "light").await;
    start_session(&app.app, &tok_student, &qr).await;
    app.clock.advance_seconds(2 * 3600); // 120 min -> credited 120
    let qr2 = open_qr(&app.app, &tok_kom, "light").await;
    let (_s, v) = finish_session(&app.app, &tok_student, &qr2).await;
    let sid = v["id"].as_str().unwrap();
    call(
        &app.app,
        "POST",
        &format!("/api/komendant/sessions/{sid}/confirm"),
        Some(&tok_kom),
        None,
    )
    .await;
    // credited=120: debt 50, balance +70. Now spend balance down to 20.
    set_debt_balance(&pool, student, 0, 20).await;

    let (st, v) = call(
        &app.app,
        "POST",
        &format!("/api/admin/sessions/{sid}/photo-review"),
        Some(&tok_admin),
        Some(json!({"approved": false, "comment": "не то фото"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["photo_review"], "rejected");

    // from_balance = min(20, 70) = 20; to_debt = 50 + (70-20) = 100
    let (debt, balance): (i64, i64) =
        sqlx::query_as("SELECT debt_minutes, balance_minutes FROM users WHERE id = $1")
            .bind(student)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((debt, balance), (100, 0));
    let _ = kom;
}

#[sqlx::test]
async fn rewards_buy_flow(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    let (_a, _t, _k, student) = setup(&app, &pool).await;
    let tok_student = login(&app.app, "stud1").await;
    let tok_admin = login(&app.app, "admin1").await;

    let (st, v) = call(
        &app.app,
        "POST",
        "/api/admin/rewards",
        Some(&tok_admin),
        Some(json!({"title": "Мерч", "price_minutes": 100, "stock": 2})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let rid = v["id"].as_str().unwrap();

    set_debt_balance(&pool, student, 0, 50).await;
    let (st, v) = call(
        &app.app,
        "POST",
        &format!("/api/rewards/{rid}/buy"),
        Some(&tok_student),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"]["code"], "insufficient_balance");

    set_debt_balance(&pool, student, 0, 150).await;
    let (st, v) = call(
        &app.app,
        "POST",
        &format!("/api/rewards/{rid}/buy"),
        Some(&tok_student),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["status"], "pending");
    let oid = v["id"].as_str().unwrap();

    let (stock, bal): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT stock FROM rewards WHERE id = $1),
                (SELECT balance_minutes FROM users WHERE id = $2)",
    )
    .bind(Uuid::parse_str(rid).unwrap())
    .bind(student)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((stock, bal), (1, 50));

    // issue then verify status; second issue conflicts
    let (st, v) = call(
        &app.app,
        "POST",
        &format!("/api/admin/orders/{oid}/issue"),
        Some(&tok_admin),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let (st, _) = call(
        &app.app,
        "POST",
        &format!("/api/admin/orders/{oid}/issue"),
        Some(&tok_admin),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);

    // buy again then cancel -> refund + stock restored
    set_debt_balance(&pool, student, 0, 150).await;
    let (_st, v) = call(
        &app.app,
        "POST",
        &format!("/api/rewards/{rid}/buy"),
        Some(&tok_student),
        None,
    )
    .await;
    let oid2 = v["id"].as_str().unwrap();
    let (st, _v) = call(
        &app.app,
        "POST",
        &format!("/api/admin/orders/{oid2}/cancel"),
        Some(&tok_admin),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (stock, bal): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT stock FROM rewards WHERE id = $1),
                (SELECT balance_minutes FROM users WHERE id = $2)",
    )
    .bind(Uuid::parse_str(rid).unwrap())
    .bind(student)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((stock, bal), (1, 150));
}

#[sqlx::test]
async fn audit_and_komendant_logs_written(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    let (_a, _t, kom, _s) = setup(&app, &pool).await;
    let tok_student = login(&app.app, "stud1").await;
    let tok_kom = login(&app.app, "kom1").await;

    let qr = open_qr(&app.app, &tok_kom, "light").await;
    start_session(&app.app, &tok_student, &qr).await;
    app.clock.advance_seconds(3600);
    let qr2 = open_qr(&app.app, &tok_kom, "light").await;
    let (_s, v) = finish_session(&app.app, &tok_student, &qr2).await;
    call(
        &app.app,
        "POST",
        &format!("/api/komendant/sessions/{}/confirm", v["id"].as_str().unwrap()),
        Some(&tok_kom),
        None,
    )
    .await;

    let audit_actions: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT action FROM audit_log ORDER BY action")
            .fetch_all(&pool)
            .await
            .unwrap();
    for a in [
        "login_success",
        "qr_open",
        "session_start",
        "session_finish",
        "session_confirm",
    ] {
        assert!(audit_actions.iter().any(|x| x == a), "missing audit {a}");
    }

    let kom_actions: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT action FROM komendant_log ORDER BY action")
            .fetch_all(&pool)
            .await
            .unwrap();
    for a in [
        "login",
        "qr_open",
        "session_started",
        "session_finished",
        "session_confirmed",
    ] {
        assert!(kom_actions.iter().any(|x| x == a), "missing komendant log {a}");
    }

    // the admin view joins the student name
    let tok_admin = login(&app.app, "admin1").await;
    let (st, v) = call(
        &app.app,
        "GET",
        "/api/admin/komendant-log?per_page=50",
        Some(&tok_admin),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let started = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["action"] == "session_started")
        .expect("session_started entry");
    assert_eq!(started["student_name"], "Студент Иван");
    let _ = kom;
}

#[sqlx::test]
async fn role_restrictions(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    setup(&app, &pool).await;
    let tok_student = login(&app.app, "stud1").await;

    for (method, path) in [
        ("GET", "/api/komendant/sessions"),
        ("POST", "/api/komendant/qr/open"),
        ("GET", "/api/admin/users"),
        ("GET", "/api/admin/sessions"),
        ("POST", "/api/admin/summaries/generate"),
    ] {
        let (st, v) = call(&app.app, method, path, Some(&tok_student), None).await;
        assert_eq!(st, StatusCode::FORBIDDEN, "{method} {path}: {v}");
    }
    // unauthenticated
    let (st, _) = call(&app.app, "GET", "/api/me", None, None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    // komendant cannot use admin endpoints
    let tok_kom = login(&app.app, "kom1").await;
    let (st, _) = call(&app.app, "GET", "/api/admin/users", Some(&tok_kom), None).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    // komendant cannot confirm/reject another komendant's session, nor a non-pending one
    let kom2 = create_user(&pool, "komendant", "kom2", "Комендант 2", None).await;
    let _ = kom2;
    let tok_kom2 = login(&app.app, "kom2").await;
    let qr = open_qr(&app.app, &tok_kom, "light").await;
    let active = start_session(&app.app, &tok_student, &qr).await;
    let sid = active["id"].as_str().unwrap();
    let (st, _) = call(
        &app.app,
        "POST",
        &format!("/api/komendant/sessions/{sid}/confirm"),
        Some(&tok_kom),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "active session is not pending");
    app.clock.advance_seconds(600);
    let (st, _) = finish_photo_only(&app.app, &tok_student).await;
    assert_eq!(st, StatusCode::OK);
    for path in ["confirm", "reject"] {
        let (st, _) = call(
            &app.app,
            "POST",
            &format!("/api/komendant/sessions/{sid}/{path}"),
            Some(&tok_kom2),
            Some(json!({"reason": "x"})),
        )
        .await;
        assert_eq!(st, StatusCode::FORBIDDEN, "{path} by other komendant");
    }
}

#[sqlx::test]
async fn qr_komendant_mismatch_at_finish(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    let group = create_group(&pool, "G").await;
    let _k1 = create_user(&pool, "komendant", "k1", "К1", None).await;
    let _k2 = create_user(&pool, "komendant", "k2", "К2", None).await;
    let _s = create_user(&pool, "student", "s1", "S", Some(group)).await;
    let tok_student = login(&app.app, "s1").await;
    let tok_k1 = login(&app.app, "k1").await;
    let tok_k2 = login(&app.app, "k2").await;

    let qr1 = open_qr(&app.app, &tok_k1, "light").await;
    start_session(&app.app, &tok_student, &qr1).await;
    app.clock.advance_seconds(1800);
    let qr2 = open_qr(&app.app, &tok_k2, "light").await;
    let (st, v) = finish_session(&app.app, &tok_student, &qr2).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"]["code"], "qr_komendant_mismatch");
}

#[sqlx::test]
async fn monthly_summary_and_xlsx(pool: PgPool) {
    // "now" = Dec 1 2026, 00:10 Vladivostok (UTC+10) = Nov 30 14:10 UTC
    // -> 1st of month after 00:05 -> summary generated for November 2026
    let app = make_app(
        pool.clone(),
        Utc.with_ymd_and_hms(2026, 11, 30, 14, 10, 0).unwrap(),
    )
    .await;
    let (_a, _t, kom, student) = setup(&app, &pool).await;
    let _ = kom;
    set_debt_balance(&pool, student, 30, 70).await;
    let tok_admin = login(&app.app, "admin1").await;

    // credited session ending Oct 15 local (UTC+10 -> 02:00 UTC)
    sqlx::query(
        "INSERT INTO work_sessions
         (student_id, komendant_id, difficulty, coefficient, started_at, ended_at, status,
          rounded_minutes, credited_minutes, applied_to_debt, applied_to_balance, photo_review)
         SELECT $1, id, 'light', 1, '2026-11-15 00:00+00', '2026-11-15 02:00+00', 'credited',
                120, 120, 100, 20, 'pending'
         FROM users WHERE role = 'komendant' LIMIT 1",
    )
    .bind(student)
    .execute(&pool)
    .await
    .unwrap();
    // a photo-rejected credited session: contributes 0
    sqlx::query(
        "INSERT INTO work_sessions
         (student_id, komendant_id, difficulty, coefficient, started_at, ended_at, status,
          rounded_minutes, credited_minutes, applied_to_debt, applied_to_balance, photo_review)
         SELECT $1, id, 'light', 1, '2026-11-16 00:00+00', '2026-11-16 02:00+00', 'credited',
                60, 60, 0, 60, 'rejected'
         FROM users WHERE role = 'komendant' LIMIT 1",
    )
    .bind(student)
    .execute(&pool)
    .await
    .unwrap();
    // assignment in October + assignment in September (should not count)
    sqlx::query(
        "INSERT INTO assignments (student_id, assigned_by, minutes, reason, created_at)
         SELECT $1, id, 45, 'nov', '2026-11-10 00:00+00' FROM users WHERE role = 'admin' LIMIT 1",
    )
    .bind(student)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO assignments (student_id, assigned_by, minutes, reason, created_at)
         SELECT $1, id, 999, 'oct', '2026-10-10 00:00+00' FROM users WHERE role = 'admin' LIMIT 1",
    )
    .bind(student)
    .execute(&pool)
    .await
    .unwrap();

    let sid = prila_server::jobs::maybe_generate_monthly(&app.state)
        .await
        .unwrap()
        .expect("summary generated");
    // second call is a no-op
    assert!(prila_server::jobs::maybe_generate_monthly(&app.state)
        .await
        .unwrap()
        .is_none());

    let row: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT sessions_count, rounded_minutes, credited_minutes, assigned_minutes,
                debt_minutes, balance_minutes
         FROM summary_rows WHERE summary_id = $1 AND student_id = $2",
    )
    .bind(sid)
    .bind(student)
    .fetch_one(&pool)
    .await
    .unwrap();
    // sessions: 2 credited in Oct; rounded: 120 + 0(rejected); credited: 120; assigned: 45; snapshot 30/70
    assert_eq!(row, (2, 120, 120, 45, 30, 70));

    // xlsx endpoint returns a real xlsx
    let resp = app
        .app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/api/admin/summaries/{sid}/xlsx"))
                .header("authorization", format!("Bearer {tok_admin}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(ct.contains("spreadsheetml"), "{ct}");
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert!(bytes.len() > 1000 && &bytes[..2] == b"PK", "not a valid xlsx");
}

#[sqlx::test]
async fn login_failure_audited_and_import_csv(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    setup(&app, &pool).await;
    let tok_admin = login(&app.app, "admin1").await;

    let (st, _v) = call(
        &app.app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({"login": "stud1", "password": "wrong"})),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'login_failure'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n, 1);

    let csv = "ФИО;Логин;Пароль;Роль;Группа\n\
               Пётр Петров;petr;;студент;А-1\n\
               Ольга Олегова;olga;secret1;преподаватель;\n";
    // Excel-style CSV: UTF-8 BOM + ';' delimiter
    let csv = format!("\u{feff}{csv}");
    let (st, v) = call_multipart(
        &app.app,
        "/api/admin/users/import",
        &tok_admin,
        vec![("file", Some("users.csv"), "text/csv", csv.into_bytes())],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let results = v["results"].as_array().unwrap();
    assert!(results.iter().all(|r| r["ok"] == true));
    let petr = &results[0];
    assert!(petr["generated_password"].as_str().unwrap().len() >= 8);
    let grp: i64 = sqlx::query_scalar("SELECT count(*) FROM groups WHERE name = 'А-1'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(grp, 1);
    let role_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM users WHERE login = 'olga' AND role = 'teacher'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(role_count, 1);
}

#[sqlx::test]
async fn monthly_summary_catchup(pool: PgPool) {
    // "now" = Nov 15 2026 local (UTC+10 -> Nov 14 15:00 UTC): past the 1st,
    // server was down on the 1st -> October summary must be generated on sight.
    let app = make_app(
        pool.clone(),
        Utc.with_ymd_and_hms(2026, 11, 14, 15, 0, 0).unwrap(),
    )
    .await;
    setup(&app, &pool).await;

    let sid = prila_server::jobs::maybe_generate_monthly(&app.state)
        .await
        .unwrap()
        .expect("catch-up summary generated");
    let (y, m): (i32, i32) =
        sqlx::query_as("SELECT year, month FROM monthly_summaries WHERE id = $1")
            .bind(sid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((y, m), (2026, 10));
    assert!(prila_server::jobs::maybe_generate_monthly(&app.state)
        .await
        .unwrap()
        .is_none());

    // on the 1st before 00:05 local nothing is generated
    app.clock
        .set(Utc.with_ymd_and_hms(2026, 11, 30, 14, 3, 0).unwrap()); // Dec 1 00:03 local
    assert!(prila_server::jobs::maybe_generate_monthly(&app.state)
        .await
        .unwrap()
        .is_none());
}

#[sqlx::test]
async fn login_rate_limit_blocks_and_window_expires(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    setup(&app, &pool).await;

    for _ in 0..10 {
        let (st, _v) = call(
            &app.app,
            "POST",
            "/api/auth/login",
            None,
            Some(json!({"login": "stud1", "password": "wrong"})),
        )
        .await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
    }

    // 11th attempt is blocked even with the correct password
    let (st, v) = call(
        &app.app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({"login": "stud1", "password": PASSWORD})),
    )
    .await;
    assert_eq!(st, StatusCode::TOO_MANY_REQUESTS, "{v}");
    assert_eq!(v["error"]["code"], "too_many_attempts");

    let n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'login_blocked'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n, 1);

    // after the 15-minute window the same login works again
    app.clock.advance_seconds(16 * 60);
    let (st, v) = call(
        &app.app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({"login": "stud1", "password": PASSWORD})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert!(v["token"].is_string());
}

#[sqlx::test]
async fn login_success_resets_attempt_counter(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    setup(&app, &pool).await;

    let wrong = || {
        call(
            &app.app,
            "POST",
            "/api/auth/login",
            None,
            Some(json!({"login": "stud1", "password": "wrong"})),
        )
    };
    for _ in 0..9 {
        assert_eq!(wrong().await.0, StatusCode::UNAUTHORIZED);
    }
    // a successful login clears the counter
    login(&app.app, "stud1").await;
    for _ in 0..9 {
        assert_eq!(wrong().await.0, StatusCode::UNAUTHORIZED);
    }
    // still under the limit in the new window: correct password succeeds
    let (st, _v) = call(
        &app.app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({"login": "stud1", "password": PASSWORD})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
}

async fn login_with_xff(app: &axum::Router, xff: &str) -> StatusCode {
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("content-type", "application/json")
        .header("x-forwarded-for", xff)
        .body(axum::body::Body::from(
            serde_json::to_vec(&json!({"login": "stud1", "password": "wrong"})).unwrap(),
        ))
        .unwrap();
    app.clone().oneshot(req).await.unwrap().status()
}

async fn last_login_failure_ip(pool: &PgPool) -> Option<String> {
    sqlx::query_scalar(
        "SELECT ip FROM audit_log WHERE action = 'login_failure' ORDER BY ts DESC LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test]
async fn xff_ignored_without_trust_proxy(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    create_user(&pool, "student", "stud1", "Студент", None).await;

    let st = login_with_xff(&app.app, "203.0.113.7").await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    // no ConnectInfo in tests: ignoring XFF leaves ip NULL
    assert_eq!(last_login_failure_ip(&pool).await, None);
}

#[sqlx::test]
async fn xff_last_entry_used_with_trust_proxy(pool: PgPool) {
    let app = make_app_with(pool.clone(), Utc::now(), |c| c.trust_proxy = true).await;
    create_user(&pool, "student", "stud1", "Студент", None).await;

    let st = login_with_xff(&app.app, "1.1.1.1, 203.0.113.7").await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    assert_eq!(last_login_failure_ip(&pool).await.as_deref(), Some("203.0.113.7"));
}

// ---------- regression tests for NEEDTOFIX audit 2026-11-17 ----------

async fn debt_balance(pool: &PgPool, user: Uuid) -> (i64, i64) {
    sqlx::query_as("SELECT debt_minutes, balance_minutes FROM users WHERE id = $1")
        .bind(user)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// student finishes and the komendant confirms; returns the session id
async fn credited_session(app: &common::TestApp, tok_student: &str, tok_kom: &str, secs: i64) -> String {
    let qr = open_qr(&app.app, tok_kom, "light").await;
    let v = start_session(&app.app, tok_student, &qr).await;
    let sid = v["id"].as_str().unwrap().to_string();
    app.clock.advance_seconds(secs);
    let (st, v) = finish_photo_only(&app.app, tok_student).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let (st, v) = call(
        &app.app,
        "POST",
        &format!("/api/komendant/sessions/{sid}/confirm"),
        Some(tok_kom),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    sid
}

#[sqlx::test]
async fn work_photos_deleted_after_three_or_seven_days(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    setup(&app, &pool).await;
    let tok_student = login(&app.app, "stud1").await;
    let tok_kom = login(&app.app, "kom1").await;
    let tok_admin = login(&app.app, "admin1").await;
    let photo = |id: Uuid| {
        let pool = pool.clone();
        async move {
            sqlx::query_as::<_, (Option<String>, Option<String>)>(
                "SELECT photo_path, photo_review FROM work_sessions WHERE id = $1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };

    let id = |s: String| Uuid::parse_str(&s).unwrap();

    // credited + photo approved by the admin -> 3 days
    let approved = credited_session(&app, &tok_student, &tok_kom, 1800).await;
    let (st, v) = call(
        &app.app,
        "POST",
        &format!("/api/admin/sessions/{approved}/photo-review"),
        Some(&tok_admin),
        Some(json!({"approved": true})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    // credited, photo never reviewed -> 7 days
    let unreviewed = credited_session(&app, &tok_student, &tok_kom, 1800).await;
    // rejected by the komendant -> 3 days
    let rejected = pending_session(&app, &tok_student, &tok_kom).await;
    let (st, v) = call(
        &app.app,
        "POST",
        &format!("/api/komendant/sessions/{rejected}/reject"),
        Some(&tok_kom),
        Some(json!({"reason": "не то"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    // still waiting for the komendant -> kept
    let waiting = pending_session(&app, &tok_student, &tok_kom).await;

    let (approved, unreviewed, rejected, waiting) = (id(approved), id(unreviewed), id(rejected), id(waiting));
    let file = |p: Option<String>| app.state.config.photo_dir.join(p.unwrap());
    let approved_file = file(photo(approved).await.0);
    let unreviewed_file = file(photo(unreviewed).await.0);
    let rejected_file = file(photo(rejected).await.0);
    let waiting_file = file(photo(waiting).await.0);

    app.clock.advance_seconds(2 * 86400);
    assert_eq!(prila_server::jobs::cleanup_photos(&app.state).await.unwrap(), 0);

    app.clock.advance_seconds(2 * 86400); // 4 days
    assert_eq!(prila_server::jobs::cleanup_photos(&app.state).await.unwrap(), 2);
    assert!(!approved_file.exists() && !rejected_file.exists());
    assert_eq!(photo(approved).await, (None, Some("approved".into())), "review result kept");
    assert!(unreviewed_file.exists());

    app.clock.advance_seconds(4 * 86400); // 8 days
    assert_eq!(prila_server::jobs::cleanup_photos(&app.state).await.unwrap(), 1);
    assert!(!unreviewed_file.exists());
    assert_eq!(photo(unreviewed).await, (None, None), "no photo, nothing left to review");
    assert!(waiting_file.exists(), "komendant has not decided yet");
    assert!(photo(waiting).await.0.is_some());
}

/// student works 10 minutes and finishes; the session waits for the komendant
async fn pending_session(app: &common::TestApp, tok_student: &str, tok_kom: &str) -> String {
    let qr = open_qr(&app.app, tok_kom, "light").await;
    let sid = start_session(&app.app, tok_student, &qr).await["id"].as_str().unwrap().to_string();
    app.clock.advance_seconds(600);
    let (st, v) = finish_photo_only(&app.app, tok_student).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    sid
}

async fn finish_photo_only(app: &axum::Router, tok: &str) -> (StatusCode, serde_json::Value) {
    call_multipart(
        app,
        "/api/student/sessions/finish",
        tok,
        vec![("photo", Some("p.jpg"), "image/jpeg", b"\xff\xd8\xff\xe0fakejpeg".to_vec())],
    )
    .await
}

#[sqlx::test]
async fn finish_without_qr_and_multipart_errors(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    setup(&app, &pool).await;
    let tok_student = login(&app.app, "stud1").await;
    let tok_kom = login(&app.app, "kom1").await;
    let qr = open_qr(&app.app, &tok_kom, "light").await;
    start_session(&app.app, &tok_student, &qr).await;
    app.clock.advance_seconds(1800);

    // no photo -> missing_photo (not a misleading QR error)
    let (st, v) = call_multipart(
        &app.app,
        "/api/student/sessions/finish",
        &tok_student,
        vec![("note", None, "", b"x".to_vec())],
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"]["code"], "missing_photo");

    // truncated multipart body -> bad_multipart
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/student/sessions/finish")
        .header("authorization", format!("Bearer {tok_student}"))
        .header("content-type", "multipart/form-data; boundary=B")
        .body(axum::body::Body::from(
            b"--B\r\nContent-Disposition: form-data; name=\"photo\"; filename=\"p.jpg\"\r\n\r\n\xff\xd8"
                .to_vec(),
        ))
        .unwrap();
    let resp = app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // photo only, no QR -> accepted
    let (st, v) = finish_photo_only(&app.app, &tok_student).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["status"], "pending_confirmation");
}

#[sqlx::test]
async fn parallel_double_start_gives_conflict_not_500(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    setup(&app, &pool).await;
    let tok_student = login(&app.app, "stud1").await;
    let tok_kom = login(&app.app, "kom1").await;
    let qr = open_qr(&app.app, &tok_kom, "light").await;
    let body = Some(json!({"qr": qr}));
    let (a, b) = tokio::join!(
        call(&app.app, "POST", "/api/student/sessions/start", Some(&tok_student), body.clone()),
        call(&app.app, "POST", "/api/student/sessions/start", Some(&tok_student), body.clone()),
    );
    let mut codes = [a.0, b.0];
    codes.sort();
    assert_eq!(codes, [StatusCode::OK, StatusCode::CONFLICT], "{} / {}", a.1, b.1);
}

#[sqlx::test]
async fn photo_review_approve_keeps_credit_and_reject_needs_comment(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    let (_a, _t, _k, student) = setup(&app, &pool).await;
    let tok_student = login(&app.app, "stud1").await;
    let tok_kom = login(&app.app, "kom1").await;
    let tok_admin = login(&app.app, "admin1").await;
    let sid = credited_session(&app, &tok_student, &tok_kom, 3600).await;
    assert_eq!(debt_balance(&pool, student).await, (0, 60));

    // admin sees it in the review queue
    let (_, v) = call(&app.app, "GET", "/api/admin/sessions?photo_review=pending", Some(&tok_admin), None).await;
    assert_eq!(v["total"], 1);

    let (st, v) = call(
        &app.app,
        "POST",
        &format!("/api/admin/sessions/{sid}/photo-review"),
        Some(&tok_admin),
        Some(json!({"approved": false, "comment": "  "})),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"]["code"], "comment_required");

    let (st, v) = call(
        &app.app,
        "POST",
        &format!("/api/admin/sessions/{sid}/photo-review"),
        Some(&tok_admin),
        Some(json!({"approved": true})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["photo_review"], "approved");
    assert_eq!(debt_balance(&pool, student).await, (0, 60));
}

#[sqlx::test]
async fn admin_adjust_bounds_and_negative(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    let (_a, _t, _k, student) = setup(&app, &pool).await;
    let tok_admin = login(&app.app, "admin1").await;
    set_debt_balance(&pool, student, 30, 10).await;
    let path = format!("/api/admin/users/{student}/adjust");

    let (st, v) = call(&app.app, "POST", &path, Some(&tok_admin),
        Some(json!({"debt_delta": 20, "balance_delta": -10, "reason": "правка"}))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(debt_balance(&pool, student).await, (50, 0));

    let (st, v) = call(&app.app, "POST", &path, Some(&tok_admin),
        Some(json!({"debt_delta": -51, "balance_delta": 0, "reason": "x"}))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"]["code"], "negative_amount");

    let (st, v) = call(&app.app, "POST", &path, Some(&tok_admin),
        Some(json!({"debt_delta": i64::MAX, "balance_delta": 0, "reason": "x"}))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"]["code"], "invalid_amount");
    assert_eq!(debt_balance(&pool, student).await, (50, 0));
}

#[sqlx::test]
async fn teacher_assign_hours_only_debtors_and_notified(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    let (_a, teacher, _k, student) = setup(&app, &pool).await;
    let tok_teacher = login(&app.app, "teach1").await;
    let tok_student = login(&app.app, "stud1").await;
    let tok_kom = login(&app.app, "kom1").await;
    let path = format!("/api/students/{student}/assignments");

    // no reason needed
    let (st, v) = call(&app.app, "POST", &path, Some(&tok_teacher), Some(json!({"minutes": 120}))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    // too much / overflow
    for minutes in [0, 100 * 60 + 1, i64::MAX] {
        let (st, v) = call(&app.app, "POST", &path, Some(&tok_teacher), Some(json!({"minutes": minutes}))).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{minutes}: {v}");
    }
    let (_, v) = call(&app.app, "GET", "/api/students/debtors", Some(&tok_teacher), None).await;
    assert_eq!(v["total"], 1, "{v}");
    assert_eq!(v["items"][0]["debt_minutes"], 120);

    credited_session(&app, &tok_student, &tok_kom, 3600).await;
    let body: String = sqlx::query_scalar(
        "SELECT body FROM notifications WHERE user_id = $1 AND kind = 'debt_worked_off'",
    )
    .bind(teacher)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(body.contains("1ч 0м"), "{body}");
}

#[sqlx::test]
async fn delete_reward_with_orders_is_conflict(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    let (_a, _t, _k, student) = setup(&app, &pool).await;
    let tok_admin = login(&app.app, "admin1").await;
    let tok_student = login(&app.app, "stud1").await;
    let (_, v) = call(&app.app, "POST", "/api/admin/rewards", Some(&tok_admin),
        Some(json!({"title": "Кружка", "price_minutes": 10}))).await;
    let rid = v["id"].as_str().unwrap().to_string();
    set_debt_balance(&pool, student, 0, 10).await;
    let (st, _) = call(&app.app, "POST", &format!("/api/rewards/{rid}/buy"), Some(&tok_student), None).await;
    assert_eq!(st, StatusCode::OK);
    let (st, v) = call(&app.app, "DELETE", &format!("/api/admin/rewards/{rid}"), Some(&tok_admin), None).await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["code"], "reward_has_orders");
    // invalid update is a 400, not a DB CHECK 500
    let (st, _) = call(&app.app, "PATCH", &format!("/api/admin/rewards/{rid}"), Some(&tok_admin),
        Some(json!({"title": "", "price_minutes": -1}))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn admin_self_role_and_last_admin_guard(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    let (admin, _t, _k, _s) = setup(&app, &pool).await;
    let tok_admin = login(&app.app, "admin1").await;
    let (st, v) = call(&app.app, "PATCH", &format!("/api/admin/users/{admin}"), Some(&tok_admin),
        Some(json!({"role": "student"}))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"]["code"], "self_role_change");
}

#[sqlx::test]
async fn password_reset_and_change_revoke_old_tokens(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    let (_a, _t, _k, student) = setup(&app, &pool).await;
    let tok_admin = login(&app.app, "admin1").await;
    let old = login(&app.app, "stud1").await;
    app.clock.advance_seconds(5);

    let (st, v) = call(&app.app, "POST", &format!("/api/admin/users/{student}/reset-password"),
        Some(&tok_admin), None).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let new_pwd = v["password"].as_str().unwrap().to_string();
    let (st, _) = call(&app.app, "GET", "/api/me", Some(&old), None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED, "token from before the reset is revoked");

    let (st, v) = call(&app.app, "POST", "/api/auth/login", None,
        Some(json!({"login": "stud1", "password": new_pwd}))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let tok = v["token"].as_str().unwrap().to_string();
    app.clock.advance_seconds(5);

    // own password change returns a fresh token; the previous one dies
    let (st, v) = call(&app.app, "POST", "/api/me/password", Some(&tok),
        Some(json!({"old_password": new_pwd, "new_password": "пароль"}))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let fresh = v["token"].as_str().unwrap().to_string();
    let (st, _) = call(&app.app, "GET", "/api/me", Some(&tok), None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, _) = call(&app.app, "GET", "/api/me", Some(&fresh), None).await;
    assert_eq!(st, StatusCode::OK);
}

#[sqlx::test]
async fn logins_are_case_insensitive(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    setup(&app, &pool).await;
    let tok_admin = login(&app.app, "admin1").await;
    let (st, v) = call(&app.app, "POST", "/api/admin/users", Some(&tok_admin),
        Some(json!({"role": "student", "login": " Ivan ", "password": "secret1", "full_name": "Иван"}))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["user"]["login"], "ivan");

    let (st, _) = call(&app.app, "POST", "/api/auth/login", None,
        Some(json!({"login": "IVAN", "password": "secret1"}))).await;
    assert_eq!(st, StatusCode::OK);

    let (st, v) = call(&app.app, "POST", "/api/admin/users", Some(&tok_admin),
        Some(json!({"role": "student", "login": "iVaN", "full_name": "Дубль"}))).await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");

    let csv = "Иван Второй,IVAN,,студент,\n";
    let (st, v) = call_multipart(&app.app, "/api/admin/users/import", &tok_admin,
        vec![("file", Some("u.csv"), "text/csv", csv.as_bytes().to_vec())]).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["results"][0]["ok"], false, "{v}");
}

#[sqlx::test]
async fn admin_resolves_stuck_pending_session(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    let (_a, _t, kom, student) = setup(&app, &pool).await;
    let tok_student = login(&app.app, "stud1").await;
    let tok_kom = login(&app.app, "kom1").await;
    let tok_admin = login(&app.app, "admin1").await;
    let qr = open_qr(&app.app, &tok_kom, "light").await;
    let v = start_session(&app.app, &tok_student, &qr).await;
    let sid = v["id"].as_str().unwrap().to_string();
    app.clock.advance_seconds(1800);
    finish_photo_only(&app.app, &tok_student).await;
    // komendant leaves: the session would block the student forever
    sqlx::query("UPDATE users SET is_active = FALSE WHERE id = $1").bind(kom).execute(&pool).await.unwrap();

    let path = format!("/api/admin/sessions/{sid}/resolve");
    let (st, _) = call(&app.app, "POST", &path, Some(&tok_admin), Some(json!({"confirm": false}))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "reject needs a reason");
    let (st, v) = call(&app.app, "POST", &path, Some(&tok_admin), Some(json!({"confirm": true}))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["status"], "credited");
    assert_eq!(v["photo_review"], "pending");
    assert_eq!(debt_balance(&pool, student).await, (0, 30));
}

#[sqlx::test]
async fn groups_report_student_count(pool: PgPool) {
    let app = make_app(pool.clone(), Utc::now()).await;
    setup(&app, &pool).await;
    let tok_admin = login(&app.app, "admin1").await;
    let (st, v) = call(&app.app, "GET", "/api/groups", Some(&tok_admin), None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v[0]["student_count"], 1, "{v}");
}

#[sqlx::test]
async fn static_admin_cache_headers(pool: PgPool) {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("assets")).unwrap();
    std::fs::write(tmp.path().join("index.html"), "<html></html>").unwrap();
    std::fs::write(tmp.path().join("assets/app-abc123.js"), "x").unwrap();
    std::fs::create_dir_all(tmp.path().join("app/_expo/static/js")).unwrap();
    std::fs::write(tmp.path().join("app/index.html"), "<html>web-app</html>").unwrap();
    std::fs::write(tmp.path().join("app/_expo/static/js/entry-1.js"), "x").unwrap();
    let dir = tmp.path().to_path_buf();
    let app = make_app_with(pool, Utc::now(), move |c| c.static_dir = Some(dir)).await;
    let immutable = "public, max-age=31536000, immutable";
    for (path, cache, body) in [
        ("/", "no-cache", "<html></html>"),
        ("/users", "no-cache", "<html></html>"),
        ("/assets/app-abc123.js", immutable, "x"),
        ("/app", "no-cache", "<html>web-app</html>"),
        ("/app/login", "no-cache", "<html>web-app</html>"),
        ("/app/_expo/static/js/entry-1.js", immutable, "x"),
    ] {
        let req = axum::http::Request::builder().uri(path).body(axum::body::Body::empty()).unwrap();
        let resp = app.app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "{path}");
        assert_eq!(resp.headers()["cache-control"], cache, "{path}");
        assert_eq!(resp.headers()["x-content-type-options"], "nosniff");
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        assert_eq!(bytes, body.as_bytes(), "{path}");
    }
}
