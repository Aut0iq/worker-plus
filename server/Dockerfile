# ---- admin frontend build ----
FROM node:20-alpine AS admin-build
WORKDIR /build
# sync-types resolves ../shared/api-types relative to the admin dir -> /shared/api-types
COPY shared/api-types /shared/api-types
COPY admin/package.json admin/package-lock.json ./
RUN npm ci
COPY admin/ ./
RUN npm run build

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
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=server-build /app/target/release/prila-server /app/prila-server
COPY --from=admin-build /build/dist /app/static
ENV STATIC_DIR=/app/static \
    PHOTO_DIR=/data/photos \
    LOG_DIR=/data/logs \
    BIND_ADDR=0.0.0.0:8080
VOLUME ["/data/photos", "/data/logs"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s CMD curl -fsS http://localhost:8080/api/health || exit 1
CMD ["/app/prila-server"]
