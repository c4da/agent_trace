# Multi-stage build for AgentTrace Gateway (TLS via rustls; no system OpenSSL needed).
FROM rust:1-slim-bookworm AS builder
WORKDIR /usr/src/agent_trace_gw
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
RUN cargo build --release --bins

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /usr/src/agent_trace_gw/target/release/agent_trace_gw /usr/local/bin/agent_trace_gw
COPY --from=builder /usr/src/agent_trace_gw/target/release/agent_trace_mcp /usr/local/bin/agent_trace_mcp

# Ledger database and node signing key live here.
VOLUME ["/data"]
ENV DB_PATH=/data/cluster_telemetry.db
ENV NODE_KEY_PATH=/data/node_key.pk8
ENV RUST_LOG=info
EXPOSE 8080

# Binds 0.0.0.0 inside the container; the process refuses to start unless
# AGENTTRACE_TOKEN is set (pass it with `docker run -e AGENTTRACE_TOKEN=...`).
ENTRYPOINT ["agent_trace_gw", "--insecure-lan"]
