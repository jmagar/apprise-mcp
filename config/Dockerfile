# syntax=docker/dockerfile:1.7

FROM rust:1.90-slim-bookworm AS builder

WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Cache deps first (dummy main to warm the registry)
COPY Cargo.toml Cargo.lock ./
RUN --mount=type=cache,id=apprise-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=apprise-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=apprise-cargo-target,target=/app/target,sharing=locked \
    mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release --locked && rm -rf src

# Build the real binary
COPY src/ src/
RUN --mount=type=cache,id=apprise-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=apprise-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=apprise-cargo-target,target=/app/target,sharing=locked \
    touch src/main.rs && cargo build --release --locked && \
    cp target/release/apprise /usr/local/bin/apprise

# ── Runtime image ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/bin/apprise /usr/local/bin/apprise

RUN groupadd --gid 1000 apprise && \
    useradd --uid 1000 --gid apprise --no-create-home --shell /sbin/nologin apprise && \
    mkdir -p /data && \
    chown apprise:apprise /data

ENV RUST_LOG=info

USER 1000:1000

EXPOSE 40050/tcp

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -sf http://localhost:40050/health || exit 1

LABEL io.modelcontextprotocol.server.name="apprise-mcp"

CMD ["apprise", "serve", "mcp"]
