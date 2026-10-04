# syntax=docker/dockerfile:1

# ── Web UI ────────────────────────────────────────────────────────────────────
FROM oven/bun:1 AS web
WORKDIR /app
COPY package.json bun.lock ./
COPY frontend/package.json frontend/
RUN bun install --frozen-lockfile
COPY frontend frontend
# Types are checked in CI; vue-tsc needs Node, which this image does not have
RUN cd frontend && bunx --bun vite build
# → /app/dist

# ── Server ────────────────────────────────────────────────────────────────────
FROM rust:1-bookworm AS server
# cmake and clang: build dependencies of aws-lc-rs (TLS and JWT signing)
RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake clang \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app/server
COPY server .
# SQL is checked against the committed .sqlx cache (see server/.cargo/config.toml),
# so the build needs no database.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/server/target \
    cargo build --release --locked \
    && cp target/release/readd-server /usr/local/bin/readd-server

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim
# ffmpeg assembles voiced books into a single M4B
RUN apt-get update \
    && apt-get install -y --no-install-recommends ffmpeg ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=server /usr/local/bin/readd-server /usr/local/bin/readd-server
COPY --from=web /app/dist /app/dist

ENV DATABASE_URL=/data/readd.db \
    UPLOADS_DIR=/data/uploads \
    DIST_DIR=/app/dist \
    PORT=3000
VOLUME /data
EXPOSE 3000

CMD ["readd-server"]
