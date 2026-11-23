# AGENTS.md

Full project context (spec, decisions, status): `docs/HANDOFF.md`; current TODO and known
issues: `NEEDTOFIX.md`.

## Build / test / run

```bash
cd server
cargo build                                 # build
cargo clippy --all-targets -- -D warnings   # lint (must stay clean)
cargo test                                  # unit + integration; also re-exports TS DTOs to shared/api-types/
cargo run                                   # run server (reads .env in server/)

cd admin && npm ci && npm run build && npm run lint      # admin (build runs sync-types)
cd mobile && npm ci && npm run typecheck && npm run lint && npm test
cd mobile && npx expo start --lan                         # dev server for Expo Go
```

Integration tests use `#[sqlx::test]` — each test gets a fresh database, so the
`DATABASE_URL` user needs CREATEDB (the `postgres:16` image user is superuser).

After changing DTOs: `cargo test` (regenerates `shared/api-types`), then
`npm run sync-types` in admin/ and mobile/, and commit all three — CI fails if
`shared/api-types` is stale.

## Docker / deploy

```bash
docker compose up -d --build                  # postgres:16 + server (serves admin)
docker compose --profile https up -d          # + Caddy HTTPS (DOMAIN in .env)
scripts/backup.sh /var/backups/prila          # pg_dump + photos
```

## Current dev machine (Windows, B:\worker-plus)

- No local Rust / Postgres: Rust is checked by CI (`.github/workflows/ci.yml`:
  clippy + tests on every push).
- Production: `http://185.28.173.203`, stack in `/opt/worker-plus`. Deploy steps:
  `NEEDTOFIX.md` («Выкатка»); image build with DNS retries: `/root/wp-build.sh`.
- Scheduled task "worker-plus autopush" commits (`new changes`) and pushes every 5 min.
- Old-PC tooling (`scripts/dev-db.sh`, offline cargo in WSL) is only needed on a
  machine without Docker/network.

## Conventions

- Server errors: `{ "error": { "code", "message" } }`, Russian `message`, stable `code`.
- All money-like fields are integer **minutes**; all timestamps server-side UTC.
- State-changing handlers must write `audit_log` via `state.audit(&mut *tx, ...)`
  inside the same transaction; komendant actions additionally via
  `state.komendant_log(...)`.
- Money mutations run inside one DB transaction with `SELECT ... FOR UPDATE`.
- Never edit an applied migration (not even line endings — sqlx checksums the bytes);
  add a new `server/migrations/000N_*.sql`. All text files are LF (`.gitattributes`).
- Commit messages: `new changes`, no assistant attribution.
- UI text (admin, mobile) is Russian; hours display as `1ч 44м`.
