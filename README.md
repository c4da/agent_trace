# AgentTrace Gateway

A small Rust reverse proxy for local AI agent setups. It sits between your agents or editors and your local models (Ollama, or a worker on your LAN), and does three things:

1. **OpenAI-compatible proxy** (`/v1/chat/completions`, `/v1/models`). Every message is scrubbed for secrets (API keys, tokens, PEM keys, passwords in URLs) before the request is forwarded and before anything is logged. Requests are routed to the local Ollama or a configured worker node by model name.
2. **Signed, hash-chained ledger.** Every proxied request and reported event is appended to a SQLite ledger. Each entry is chained with SHA-256 and signed with an Ed25519 node key, so tampering is detectable (`GET /api/v1/ledger/verify`).
3. **MCP server** (`agent_trace_mcp`, 17 tools) so agents can record decisions, delegate prompts to cluster models, query telemetry, and verify the ledger.

Two small web pages ship with it: `/console` (dense terminal-style event view) and `/dashboard` (overview). Both show how many tokens were served by local models instead of a paid API.

This is a local developer tool, not a hardened multi-tenant service. The DLP scrubber is pattern-based and will not catch every secret.

## Quickstart

```sh
cargo run --release --bin agent_trace_gw
# listening on http://127.0.0.1:8080
```

Open `http://127.0.0.1:8080/console`. Ollama is expected at `http://127.0.0.1:11434`.

Point any OpenAI-compatible client at the gateway:

```sh
curl http://127.0.0.1:8080/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model":"auto","messages":[{"role":"user","content":"hello"}]}'
```

Or set the client's base URL to `http://127.0.0.1:8080/v1`. If `AGENTTRACE_TOKEN` is set, use it as the client's API key.

## MCP configuration

Build the bridge (`cargo build --bins`), then add it to your agent's MCP config:

```json
{
  "mcpServers": {
    "agent-trace": {
      "command": "C:\\path\\to\\agent_trace_gw\\target\\debug\\agent_trace_mcp.exe",
      "env": {
        "AGENTTRACE_URL": "http://127.0.0.1:8080",
        "AGENTTRACE_TOKEN": "only-if-the-gateway-has-one"
      }
    }
  }
}
```

Tools: `record_project_decision`, `dispatch_cluster_prompt`, `verify_ledger`, `query_telemetry`, `inspect_cluster`, `flag_incident`, `explain_route`, `decide_route`, `set_route_override`, `list_route_overrides`, `remove_route_override`, `analyze_telemetry`, `get_delegation_config`, `set_delegation_config`, `get_telemetry_handoff`, `get_security_alerts`, `acknowledge_alert`. The same tools are available over HTTP at `POST /api/v1/mcp` and `POST /api/v1/agent/action`. See [docs/MCP_SERVER_GUIDE.md](docs/MCP_SERVER_GUIDE.md).

## Configuration

| Variable | Default | Meaning |
| --- | --- | --- |
| `UPSTREAM_URL` | `http://127.0.0.1:11434` | Default upstream (Ollama) |
| `WORKER_UPSTREAMS` | none | Extra upstreams, `NAME=url,...` e.g. `PC-2=http://192.168.1.102:8642,GPU=http://10.0.0.5:11434`. Each must also be in `UPSTREAM_ALLOWLIST`. Can be set in `.secrets/agenttrace.env`. Workers are polled every 15 s and drop out after 60 s offline |
| `UPSTREAM_ALLOWLIST` | local Ollama | Comma-separated upstream origins the proxy may call (SSRF guard) |
| `WORKER_HERMES_KEY` | none | Bearer token sent to the worker upstream |
| `AGENTTRACE_TOKEN` | none | Shared secret required on all non-static routes |
| `AGENTTRACE_REQUIRE_AUTH` | `false` | `true` fails closed if no token is configured |
| `AGENTTRACE_DLP_MODE` | `redact` | `redact`, `block` (403 on any finding), or `off`. Header `x-guard-mode: block` forces block per request |
| `DB_PATH` | `cluster_telemetry.db` | SQLite ledger |
| `NODE_KEY_PATH` | `.secrets/node_key.pk8` | Ed25519 node signing key |
| `PORT` / `BIND_HOST` | `8080` / `127.0.0.1` | Listen address |
| `AGENTTRACE_ALLOW_INSECURE_LAN` | `false` | Same as `--insecure-lan` |

Variables can also be placed in `.secrets/agenttrace.env` (see `secrets.example.env`).

## Security model

- Binds `127.0.0.1` by default.
- No CORS layer: cross-origin browser access is denied.
- While bound to loopback, requests whose `Host` header is not `localhost`, `127.0.0.1` or `[::1]` get `421` (DNS-rebinding defense).
- If `AGENTTRACE_TOKEN` is set, every route except the static pages (`/`, `/dashboard`, `/console`) and `/login` requires it, via `x-agenttrace-token`, `x-api-key`, `Authorization: Bearer ...`, or the `agenttrace_token` cookie (SameSite=Strict, HttpOnly) that `GET /login?token=...` sets for browser use.
- `AGENTTRACE_REQUIRE_AUTH=true` makes the gateway reject everything when no token is configured.
- `--insecure-lan` (or `AGENTTRACE_ALLOW_INSECURE_LAN`) binds `0.0.0.0`, and the process refuses to start without `AGENTTRACE_TOKEN`. There is no TLS; use it only on a trusted network.
- Upstream URLs must be in the allowlist.
- Secrets are redacted from all messages before forwarding and before logging.

Report vulnerabilities as described in [SECURITY.md](SECURITY.md).

## HTTP routes

`GET /`, `/dashboard`, `/console`, `/login?token=`; `POST /v1/chat/completions`; `GET /v1/models`; `POST /api/v1/dlp/scan`; `POST|GET /api/v1/events`; `GET /api/v1/summary`, `/savings`, `/ledger/verify`, `/stream` (SSE), `/alerts`; `POST /api/v1/alerts/ack`, `/alerts/reset`; `GET /api/v1/agent/handoff`, `/agent/tools`; `POST /api/v1/agent/action`, `/mcp`, `/delegate/decide`, `/delegate/override`; `GET|POST /api/v1/delegate/config`; `GET /api/v1/delegate/overrides`; `DELETE /api/v1/delegate/override/:rule_id`.

## Development

```sh
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

Docker: `AGENTTRACE_TOKEN=... docker compose up --build` (the container binds `0.0.0.0`, so the token is mandatory; the compose file publishes the port on loopback only).

Licensed under MIT OR Apache-2.0.
