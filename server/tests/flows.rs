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
    // komendant cannot confirm other komendant's sessions: covered implicitly
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
    // our parser expects commas; use comma CSV
    let csv = csv.replace(';', ",");
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
