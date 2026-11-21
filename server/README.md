# Work+ server (crate `prila-server`)

Rust-сервер (Axum + SQLx/Postgres) для учёта отработок студентов. Раздаёт API (`/api`),
админ-панель (`/`) и веб-версию мобильного приложения (`/app`) из `STATIC_DIR`.
Общие сведения, переменные окружения и полный список API — в корневом `README.md`.

## Запуск

```bash
cp ../.env.example .env   # или см. раздел «Переменные окружения» в ../README.md
# обязательные: DATABASE_URL, JWT_SECRET, QR_SECRET, ADMIN_LOGIN, ADMIN_PASSWORD
cargo run                 # применит миграции и создаст первого админа при старте
```

## Сборка и проверки

```bash
cargo build                 # сборка
cargo clippy --all-targets -- -D warnings
cargo test                  # юнит + интеграционные (#[sqlx::test], нужен DATABASE_URL
                            # пользователя с правом CREATEDB);
                            # заодно ре-экспортирует TS-типы в ../shared/api-types/
```

## Структура

- `src/config.rs` — конфигурация из env
- `src/state.rs` — `AppState` (pool, clock, файлы логов, http), `audit`, `komendant_log`, `notify`, push
- `src/clock.rs` — `Clock` trait (`SystemClock`/`MockClock`) — «сейчас» инъектируется для тестов
- `src/models.rs` — роли и enum-статусы
- `src/domain.rs` — чистая доменная логика: округление, коэффициент, распределение долг/баланс, возврат
- `src/qr.rs` — выдача/проверка QR-токенов (HMAC-SHA256, окно 30 с, текущее и предыдущее)
- `src/auth.rs` — argon2, JWT (7 дней), экстракторы `AuthUser`/`StudentUser`/`TeacherUser`/`KomendantUser`/`AdminUser`
- `src/routes/` — обработчики: auth, me, student, komendant, teacher, rewards, admin
- `src/import.rs` — импорт пользователей CSV/XLSX (одна транзакция на весь файл)
- `src/summaries.rs` — месячные сводки + экспорт `.xlsx` (`fmt_hm` → «1ч 44м»)
- `src/jobs.rs` — фоновые задачи: истечение сессий > `MAX_SESSION_HOURS`, сводка 1-го числа после 00:05,
  ежечасная очистка старых уведомлений и фото работы (3 дня после решения / 7 дней)
- `migrations/` — SQL-миграции (`sqlx::migrate!` при старте)
- `tests/` — интеграционные тесты потоков

## Соглашения

- Ошибки: `{ "error": { "code": "stable_code", "message": "текст по-русски" } }`.
- Списки с пагинацией: `{items, total, page, per_page}`.
- Все денежные поля — целые минуты; время — только серверное (`timestamptz`).
- SQL-строки, собранные через `format!` из констант, помечаются `sqlx::AssertSqlSafe` —
  все данные пользователя только через bind-параметры.
