# ---- admin frontend build ----
FROM node:22-alpine AS admin-build
WORKDIR /build
# sync-types resolves ../shared/api-types relative to the admin dir -> /shared/api-types
COPY shared/api-types /shared/api-types
COPY admin/package.json admin/package-lock.json ./
RUN npm ci
COPY admin/ ./
RUN npm run build

# ---- mobile app, web build (served at /app) ----
FROM node:22-alpine AS web-build
WORKDIR /build
COPY mobile/package.json mobile/package-lock.json ./
RUN npm ci
COPY mobile/ ./
RUN npx expo export -p web --output-dir dist

# ---- rust build ----
FROM rust:1-slim AS server-build
WORKDIR /app
COPY server/Cargo.toml server/Cargo.lock ./
COPY server/migrations ./migrations
COPY server/src ./src
RUN cargo build --release --locked

# ---- runtime ----
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /app app \
    && mkdir -p /data/photos /data/logs && chown -R app:app /data
WORKDIR /app
COPY --from=server-build /app/target/release/prila-server /app/prila-server
COPY --from=admin-build /build/dist /app/static
COPY --from=web-build /build/dist /app/static/app
ENV STATIC_DIR=/app/static \
    PHOTO_DIR=/data/photos \
    LOG_DIR=/data/logs \
    BIND_ADDR=0.0.0.0:8080
VOLUME ["/data/photos", "/data/logs"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s CMD curl -fsS http://localhost:8080/api/health || exit 1
# start as root only to fix ownership of volumes created by older (root) images,
# then run the server as the unprivileged `app` user (setpriv is in util-linux)
CMD ["sh", "-c", "chown -R app:app /data/photos /data/logs && exec setpriv --reuid=app --regid=app --init-groups /app/prila-server"]
