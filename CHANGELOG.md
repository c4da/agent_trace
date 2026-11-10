# Changelog

## [2.0.0] - 2026-11-06 — slim core

The project was cut from roughly 34k lines to a focused core: an OpenAI-compatible scrubbing proxy, a signed hash-chained ledger, a 17-tool MCP server, and two UI pages (`/console`, `/dashboard`).

### Security fixes
- Removed unauthenticated endpoints that could read and write arbitrary files on the host.
- Added auth middleware covering every route except the static pages and `/login`; `AGENTTRACE_TOKEN` is accepted via `x-agenttrace-token`, `x-api-key`, `Authorization: Bearer`, or the `agenttrace_token` cookie set by `GET /login?token=`.
- Removed the permissive CORS layer; cross-origin browser access is now denied.
- Added a Host header check (421) while bound to loopback, defending against DNS rebinding.
- `--insecure-lan` / `AGENTTRACE_ALLOW_INSECURE_LAN` now refuses to start without `AGENTTRACE_TOKEN`.
- DLP now actually redacts the forwarded request and every message. Previously secrets were forwarded upstream and stored raw in the ledger.
- Passwords embedded in URLs (`scheme://user:pass@host`) are now redacted.

### Removed
- Command firewall (`ai_guard`), policy engine, safe zones/playgrounds, CAS rollback guard
- Idle worker, skill forge, admin governance, persistence layer, SIEM exporter
- Claude bridge (`/v1/messages`), OTA distribution, prompt hub, problem workgroups, projects, artifacts, tasks
- Studio client and mobile UIs, cluster health watchdog, Copilot and Antigravity watchers, agent identity, OS hardening, benchmarks, contributions
- Worker mode, UDP beacon, install/provision scripts, Python helper scripts and the benchmark CI workflow
- MCP tools: reduced from 36 to 17

### Changed
- `Cargo.lock` is now committed.
- Added `GET /api/v1/savings` and a "tokens served locally" figure in the console and dashboard.
- Dockerfile and docker-compose.yml rewritten; CI now runs `cargo test` and `cargo clippy`.

## [1.1.0] - 2026-10-25

Auto-skill forge, pluggable persistence and benchmarks (all since removed in 2.0.0).

## [1.0.0] - 2026-10-25

First release.
