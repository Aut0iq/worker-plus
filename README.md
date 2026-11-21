# Work+ — учёт общественно полезного труда студентов

> Термины интерфейса: **ОПТ / общественно полезный труд** = сессия (`work_sessions`,
> в старых документах — «отработка»); **задание** = долг (`debt_minutes`, «должники»
> → вкладка «Задания»). В коде и API имена прежние.

> Полная документация для передачи проекта (ТЗ, решения, статус, задачи): [docs/HANDOFF.md](docs/HANDOFF.md).
> Текущие задачи и решения заказчика — [NEEDTOFIX.md](NEEDTOFIX.md).

Учёт часов отработки студентов общежития: REST API на Rust (axum + sqlx/Postgres,
`server/`), веб-панель администратора React (`admin/`) и мобильное приложение Expo
для студентов, комендантов и преподавателей (`mobile/`, см. [mobile/README.md](mobile/README.md)).
Приложение собирается в Android APK (`ru.workerplus.app`) и в веб-версию, которую
раздаёт сам сервер по пути `/app` (ПК и iPhone через Safari; без камеры).

## Роли

- **student** — студент: видит задание/баланс, начинает ОПТ по QR начала коменданта,
  завершает по QR завершения (любого коменданта) + фото работы, покупает награды
  за минуты баланса.
- **teacher** — преподаватель: назначает часы (целые часы, ≤ 100 за раз) студентам
  по группам, видит должников; получает уведомление, когда студент отработал часы.
- **komendant** — комендант: показывает QR начала (меняется каждые 30 с) и QR
  завершения (каждые 3 мин), подтверждает/отклоняет ОПТ.
- **admin** — администратор: веб-панель, пользователи, группы, награды, заказы,
  проверка фото, решение по сессиям вместо коменданта, месячные сводки, логи.
  Может всё, что преподаватель, и **только он** списывает часы (корректировка
  долга/баланса — в админке «Корр.» и в приложении «Списать часы»).

## Быстрый старт (docker-compose)

```bash
cp .env.example .env   # заполнить DB_PASSWORD, JWT_SECRET, QR_SECRET, ADMIN_PASSWORD
docker compose up --build
```

Сервер слушает `http://localhost:8080` (порт — `PORT` в `.env`): API — `/api`,
админ-панель — `/`, веб-версия приложения — `/app`. Postgres — `postgres:16`.
Образ собирает Rust-сервер, админку и веб-версию приложения (`server/Dockerfile`).

## Android APK

Собирается только в GitHub Actions: пуш тега `v1.2.3` → подписанный
`worker-plus-1.2.3.apk` в GitHub Release (`.github/workflows/release-android.yml`).
Секреты репозитория: `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD` (подпись),
`EAS_PROJECT_ID` (проект expo.dev) и `GOOGLE_SERVICES_JSON` (Firebase) — для push.
Ключ FCM V1 (сервисный аккаунт Firebase) загружен в Expo (`eas credentials`).

### HTTPS на VPS

Указать `DOMAIN=prila.example.com` в `.env` (A-запись домена → IP VPS, порты 80/443
открыты) и запустить `docker compose --profile https up -d` — Caddy сам получит
сертификат Let's Encrypt и проксирует на сервер (`Caddyfile`).

### Бэкапы

`scripts/backup.sh [каталог]` — `pg_dump` базы и архив фото (по умолчанию `./backups`,
хранит `KEEP_DAYS=14` дней). Cron на VPS:
`0 3 * * * cd /opt/worker-plus && scripts/backup.sh /var/backups/prila >> /var/log/prila-backup.log 2>&1`.
Восстановление:
```bash
docker compose exec -T db pg_restore -U prila -d prila --clean --if-exists < db-XXXX.dump
docker compose exec -T server tar -C /data -xzf - < photos-XXXX.tar.gz
```

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
| `TRUST_PROXY` | `false` | брать IP клиента из последнего `X-Forwarded-For` (только за Caddy, вместе с `PORT=127.0.0.1:8080`) |
| `DOMAIN` | — | домен для профиля `https` (Caddy) |

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
окно 30 с, валиден текущее и предыдущее окно. QR завершения: `prila1f.<komendant>.<window>.<hmac16>`,
окно 180 с (текущее + предыдущее, 3–6 мин); QR начала и завершения не взаимозаменяемы
(`wrong_qr_kind`).

## API (сводно)

Ошибки: `{ "error": { "code", "message" } }` (message по-русски). Списки:
`{ items, total, page, per_page }`. Авторизация: `Authorization: Bearer <jwt>`.

### Auth / профиль / уведомления
- `POST /api/auth/login` `{login,password}` → `{token,user}`; логин без учёта регистра;
  лимит: 10 неудачных попыток за 15 мин по ip+login и 50 по ip → 429 `too_many_attempts`
- `GET /api/me` · `POST /api/me/password` `{old_password,new_password}` → `{token}`
  (смена/сброс пароля отзывает старые токены)
- `POST /api/me/push-token` / `DELETE /api/me/push-token` `{token}`
- `GET /api/notifications` · `GET /api/notifications/unread-count` ·
  `POST /api/notifications/{id}/read` · `POST /api/notifications/read-all`
- `GET /api/health`

### Студент
- `POST /api/student/sessions/start` `{qr}`
- `POST /api/student/sessions/finish` (multipart: `qr` — QR завершения любого активного
  коменданта, обязателен (`missing_qr`); `photo` jpeg/png/heic/webp ≤10МБ). ОПТ остаётся
  за комендантом, который его начинал
- `GET /api/student/sessions` · `GET /api/student/sessions/active` (с `server_now`)
- `GET /api/student/assignments` · `GET /api/student/orders` · `GET /api/student/orders/{id}`
- `GET /api/rewards` · `POST /api/rewards/{id}/buy` · `GET /api/rewards/{id}/image`

### Комендант
- `POST /api/komendant/qr/open` `{difficulty: light|hard}` → `{qr,expires_at}` (логируется)
- `GET /api/komendant/qr?difficulty=` — обновление QR (не логируется)
- `POST /api/komendant/qr/finish/open` → `{qr,expires_at}` (логируется) ·
  `GET /api/komendant/qr/finish` — QR завершения
- `GET /api/komendant/sessions?status=`
- `POST /api/komendant/sessions/{id}/confirm` · `POST /api/komendant/sessions/{id}/reject {reason}`
- `GET /api/komendant/sessions/{id}/photo` (также доступна студенту-владельцу и админу)

### Преподаватель (+admin)
- `GET /api/groups` · `GET /api/groups/{id}/students`
- `GET /api/students/debtors` — студенты с долгом
- `GET /api/students/{id}` — профиль + сессии + назначения
- `POST /api/students/{id}/assignments` `{minutes, reason?}` (≤ 100 ч за раз)

### Админ
- Пользователи: `GET/POST /api/admin/users` (фильтры role, group_id, q, is_active),
  `GET/PATCH/DELETE /api/admin/users/{id}` (delete = деактивация),
  `POST /api/admin/users/{id}/reset-password`, `POST /api/admin/users/{id}/adjust`
  `{debt_delta,balance_delta,reason}`, `POST /api/admin/users/import` (multipart CSV/XLSX:
  `ФИО, Логин, Пароль, Роль, Группа` — атомарно, ошибки по строкам)
- Группы: `GET/POST /api/admin/groups`, `PATCH/DELETE /api/admin/groups/{id}`
- Сессии: `GET /api/admin/sessions` (status, photo_review, student_id, komendant_id, from, to),
  `GET /api/admin/sessions/{id}/photo`,
  `POST /api/admin/sessions/{id}/photo-review` `{approved, comment}`,
  `POST /api/admin/sessions/{id}/resolve` `{confirm, reason?}` — решение за коменданта
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
- каждый час: удаление прочитанных уведомлений старше 180 дней и фото работы —
  через 3 дня после решения (проверка фото админом или отклонение комендантом),
  иначе через 7 дней после окончания сессии; фото сессии, ждущей коменданта, не трогается.

Типы DTO для клиентов экспортируются в `shared/api-types/*.ts` через `cargo test` (ts-rs).
