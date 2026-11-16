# Prila — учёт отработок студентов

Серверная часть (фаза 1): REST API на Rust (axum + sqlx/Postgres) для учёта
часов отработки студентов общежития. Далее: админ-панель React (`admin/`,
фаза 2) и мобильное приложение Expo (`mobile/`, фаза 3).

## Роли

- **student** — студент: видит долг/баланс, начинает/завершает сессии по QR,
  покупает награды за минуты баланса.
- **teacher** — преподаватель: назначает часы отработки студентам по группам.
- **komendant** — комендант: показывает QR для старта/финиша сессий,
  подтверждает/отклоняет сессии.
- **admin** — администратор: веб-панель, пользователи, группы, награды, заказы,
  проверка фото, месячные сводки, логи. Может вызывать teacher-эндпоинты.

## Быстрый старт (docker-compose)

```bash
cp .env.example .env   # заполнить DB_PASSWORD, JWT_SECRET, QR_SECRET, ADMIN_PASSWORD
docker compose up --build
```

API слушает `http://localhost:8080`, Postgres — `postgres:16`.

## Локальная разработка

```bash
scripts/dev-db.sh install   # скачать встроенный Postgres 16 в .devdb/ (без root)
scripts/dev-db.sh start     # поднять на 127.0.0.1:5433, создать БД prila
cd server
cp ../.env.example .env     # и дописать DATABASE_URL=postgres://prila@127.0.0.1:5433/prila
cargo run                   # миграции и первый админ создаются при старте
```

## Переменные окружения

| Переменная | По умолчанию | Описание |
|---|---|---|
| `DATABASE_URL` | — | строка подключения Postgres |
| `JWT_SECRET` | — | секрет подписи JWT (HS256, 7 дней) |
| `QR_SECRET` | — | секрет HMAC для QR-кодов коменданта |
| `APP_TIMEZONE` | `Asia/Vladivostok` | локальная таймзона (месяцы, сводки) |
| `PHOTO_DIR` | `./data/photos` | каталог фото сессий и наград |
| `LOG_DIR` | `./data/logs` | каталог audit.log / komendant.log |
| `STATIC_DIR` | — | dist админ-панели (SPA fallback на index.html) |
| `BIND_ADDR` | `0.0.0.0:8080` | адрес прослушивания |
| `ADMIN_LOGIN` / `ADMIN_PASSWORD` | `admin` / — | первый админ создаётся при старте, если админов нет |
| `EXPO_PUSH_ENABLED` | `false` | слать push через Expo |
| `EXPO_PUSH_URL` | `https://exp.host/--/api/v2/push/send` | endpoint Expo push |
| `CORS_ORIGINS` | `*` | список origin через запятую или `*` |
| `MAX_SESSION_HOURS` | `4` | лимит реального времени сессии (без коэффициента) |

## Модель данных (кратко)

`groups`, `users` (role/login/full_name/group_id/debt_minutes/balance_minutes/
is_active), `push_tokens`, `assignments` (назначенные часы), `work_sessions`
(status: active → pending_confirmation → credited | komendant_rejected | expired;
coefficient 1/2; raw_seconds → rounded_minutes → credited_minutes;
applied_to_debt / applied_to_balance; photo_review: pending → approved/rejected),
`rewards`, `orders`, `notifications`, `audit_log`, `komendant_log`,
`monthly_summaries` + `summary_rows`.

Все суммы — целые минуты. Все метки времени — серверные (`timestamptz`).

Правила: округление `rounded_minutes = (raw_seconds + 30) / 60`;
`credited = rounded * coefficient`; при подтверждении `to_debt = min(debt, credited)`,
остаток — в баланс; при отклонении фото зачёт откатывается (что не удалось
забрать с баланса — возвращается в долг). QR: `prila1.<komendant>.<difficulty>.<window>.<hmac16>`,
окно 30 с, валиден текущее и предыдущее окно.

## API (сводно)

Ошибки: `{ "error": { "code", "message" } }` (message по-русски). Списки:
`{ items, total, page, per_page }`. Авторизация: `Authorization: Bearer <jwt>`.

### Auth / профиль / уведомления
- `POST /api/auth/login` `{login,password}` → `{token,user}`
- `GET /api/me` · `POST /api/me/password` `{old_password,new_password}`
- `POST /api/me/push-token` / `DELETE /api/me/push-token` `{token}`
- `GET /api/notifications` · `POST /api/notifications/{id}/read` · `POST /api/notifications/read-all`
- `GET /api/health`

### Студент
- `POST /api/student/sessions/start` `{qr}`
- `POST /api/student/sessions/finish` (multipart: `qr`, `photo` jpeg/png/heic/webp ≤10МБ)
- `GET /api/student/sessions` · `GET /api/student/sessions/active` (с `server_now`)
- `GET /api/student/assignments` · `GET /api/student/orders` · `GET /api/student/orders/{id}`
- `GET /api/rewards` · `POST /api/rewards/{id}/buy` · `GET /api/rewards/{id}/image`

### Комендант
- `POST /api/komendant/qr/open` `{difficulty: light|hard}` → `{qr,expires_at}` (логируется)
- `GET /api/komendant/qr?difficulty=` — обновление QR (не логируется)
- `GET /api/komendant/sessions?status=`
- `POST /api/komendant/sessions/{id}/confirm` · `POST /api/komendant/sessions/{id}/reject {reason}`
- `GET /api/komendant/sessions/{id}/photo` (также доступна студенту-владельцу и админу)

### Преподаватель (+admin)
- `GET /api/groups` · `GET /api/groups/{id}/students`
- `GET /api/students/{id}` — профиль + сессии + назначения
- `POST /api/students/{id}/assignments` `{minutes, reason}`

### Админ
- Пользователи: `GET/POST /api/admin/users` (фильтры role, group_id, q, is_active),
  `GET/PATCH/DELETE /api/admin/users/{id}` (delete = деактивация),
  `POST /api/admin/users/{id}/reset-password`, `POST /api/admin/users/{id}/adjust`
  `{debt_delta,balance_delta,reason}`, `POST /api/admin/users/import` (multipart CSV/XLSX:
  `ФИО, Логин, Пароль, Роль, Группа` — атомарно, ошибки по строкам)
- Группы: `GET/POST /api/admin/groups`, `PATCH/DELETE /api/admin/groups/{id}`
- Сессии: `GET /api/admin/sessions` (status, photo_review, student_id, komendant_id, from, to),
  `GET /api/admin/sessions/{id}/photo`,
  `POST /api/admin/sessions/{id}/photo-review` `{approved, comment}`
- Награды: `POST /api/admin/rewards`, `PATCH/DELETE /api/admin/rewards/{id}`,
  `POST /api/admin/rewards/{id}/image` (multipart)
- Заказы: `GET /api/admin/orders`, `POST /api/admin/orders/{id}/issue`, `POST /api/admin/orders/{id}/cancel`
- Сводки: `GET /api/admin/summaries`, `GET /api/admin/summaries/{id}`,
  `GET /api/admin/summaries/{id}/xlsx`, `POST /api/admin/summaries/generate {year,month}`
- Логи: `GET /api/admin/audit-log`, `GET /api/admin/komendant-log`
  (фильтры actor/komendant, action, from, to, пагинация)

## Фоновые задачи

- каждые 60 с: `active` сессии старше `MAX_SESSION_HOURS` → `expired` (+уведомления).
- каждые 60 с: 1-го числа после 00:05 локального времени → сводка за прошлый месяц.

Типы DTO для клиентов экспортируются в `shared/api-types/*.ts` через `cargo test` (ts-rs).
