# AGENTS.md

## Build / test / run

```bash
cd server
cargo build                 # build
cargo clippy -- -D warnings # lint (must stay clean)
cargo test                  # unit + integration tests; also re-exports TS DTOs to shared/api-types/
cargo run                   # run server (reads .env in server/)
```

Integration tests use `#[sqlx::test]` — each test gets a fresh database, so the
`DATABASE_URL` user needs CREATEDB. With dev-db.sh (trust auth, user `prila`
is the cluster superuser) this works out of the box:

```bash
export DATABASE_URL=postgres://prila@127.0.0.1:5433/prila
```

## Dev Postgres (no root, no docker)

```bash
scripts/dev-db.sh install   # one-time: fetch zonky embedded postgres 16 -> .devdb/
scripts/dev-db.sh start     # 127.0.0.1:5433, db=prila, user=prila, trust auth
scripts/dev-db.sh stop|status
```

## Docker

```bash
docker compose up --build   # postgres:16 + server (multi-stage Dockerfile)
docker compose config       # validate
```

## Environment notes (how this workspace was built)

This machine's WSL has **no outbound network** (DNS + TCP dead) and no gcc,
curl, pip or docker access. Everything was installed from files downloaded
through the Windows host:

- **Rust**: standalone dist tarball `rust-1.99.0-x86_64-unknown-linux-gnu.tar.xz`
  (+ clippy, rustfmt components) -> `~/toolchains/rust`. On PATH via `~/prila-env.sh`.
- **C toolchain** (linker for cargo): Debian debs extracted into `~/sysroot`
  (`dpkg -x`), wrapper scripts `~/bin/{ccwrap,arwrap,ranlibwrap}` (symlinked as
  `~/bin/{cc,gcc,ar,ranlib}`) call the extracted gcc-14/binutils with `--sysroot`.
  On PATH via `~/prila-env.sh`.
- **Cargo registry**: crates were fetched on Windows (rustup cargo) and the
  registry cache copied to `~/.cargo/registry`; `CARGO_NET_OFFLINE=true` is set in
  `~/prila-env.sh`. To add deps: edit Cargo.toml, run `cargo fetch` with the
  Windows cargo (`/mnt/c/Users/Flayzer/.cargo/bin/cargo.exe`), then re-copy new
  `.crate` files + index entries from `C:\Users\Flayzer\.cargo\registry` to
  `~/.cargo/registry`.
- **Postgres 16**: zonky embedded binaries jar unpacked into `~/.devdb/`
  (`PG_HOME`; data dir must live on the Linux fs — /mnt/c breaks initdb
  permissions), `initdb` + `pg_ctl` per `scripts/dev-db.sh`. The bundle has no
  `psql`; a Debian `postgresql-client-17` is extracted to `~/pgclient` —
  `LD_LIBRARY_PATH=~/pgclient/usr/lib/x86_64-linux-gnu
  ~/pgclient/usr/lib/postgresql/17/bin/psql -h 127.0.0.1 -p 5433 -U prila prila`.
- `CARGO_TARGET_DIR=/home/aut0iq/prila-target` (build on the Linux fs — /mnt/c is slow).

```bash
source ~/prila-env.sh   # rust + cc wrapper + offline cargo + target dir
```

## Conventions

- Server errors: `{ "error": { "code", "message" } }`, Russian `message`, stable `code`.
- All money-like fields are integer **minutes**; all timestamps server-side UTC.
- State-changing handlers must write `audit_log` via `state.audit(&mut *tx, ...)`
  inside the same transaction; komendant actions additionally via
  `state.komendant_log(...)`.
- Money mutations run inside one DB transaction with `SELECT ... FOR UPDATE`.

## Project status / next steps

- Done: Rust server (`server/`, all endpoints, tests), admin web panel (`admin/`).
- TODO: Expo (React Native, TypeScript) mobile app in `mobile/` for roles
  student / komendant / teacher. Student: profile (debt/balance), scan QR to
  start, finish = photo + QR scan, live timer, history, shop, orders,
  notifications. Komendant: choose difficulty (light x1 / hard x2), rotating QR
  (`POST /api/komendant/qr/open`, refresh `GET /api/komendant/qr` every ~10s),
  confirm/reject pending sessions with photo. Teacher: groups -> students ->
  assign hours. Push via Expo tokens (`POST /api/me/push-token`). Types: copy
  `shared/api-types/*.ts` the same way as `admin/scripts/sync-types.mjs`.
- Fresh machine (normal network): install Rust + PostgreSQL 16 (or
  `docker compose up -d db`), copy `.env.example` to `server/.env` and fill it,
  then `cd server && cargo run`; `cd admin && npm ci && npm run dev`
  (http://localhost:5173, proxies `/api` to :8080).
