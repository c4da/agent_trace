# ------------------------------------------------------------------------------
# Multi-stage Dockerfile for AgentTrace Gateway
# ------------------------------------------------------------------------------

# Stage 1: Build binary
FROM rust:1.80-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/agent_trace_gw

COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
COPY docs/ ./docs/

RUN cargo build --release

# Stage 2: Minimal runtime image
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /usr/src/agent_trace_gw/target/release/agent_trace_gw /usr/local/bin/agent_trace_gw

# Volume for persisted workgroups, ledger, and SQLite database
VOLUME ["/app/.agent_trace"]

EXPOSE 8080

ENV RUST_LOG=info
ENV AGENT_TRACE_BIND=0.0.0.0:8080

ENTRYPOINT ["agent_trace_gw"]
