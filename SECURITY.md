# Security Policy

AgentTrace Gateway is a local developer tool: a scrubbing OpenAI-compatible proxy with a signed hash-chained ledger. It is not a hardened multi-tenant service.

## Supported Versions

Only the latest release on `main` receives fixes. Versions before 2.0.0 had serious issues (unauthenticated file read/write endpoints, secrets forwarded upstream and stored raw) and should not be used.

## Security model

- Binds `127.0.0.1` by default.
- No CORS layer; cross-origin browser access is denied.
- While loopback-bound, a `Host` header other than `localhost`, `127.0.0.1` or `[::1]` gets `421` (DNS-rebinding defense).
- If `AGENTTRACE_TOKEN` is set, every route except the static pages and `/login` requires it (`x-agenttrace-token`, `x-api-key`, `Authorization: Bearer`, or the SameSite=Strict HttpOnly `agenttrace_token` cookie set by `GET /login?token=`). `AGENTTRACE_REQUIRE_AUTH=true` fails closed without a token.
- `--insecure-lan` / `AGENTTRACE_ALLOW_INSECURE_LAN` binds `0.0.0.0` and refuses to start without a token. There is no TLS.
- Outbound proxy requests are limited to `UPSTREAM_ALLOWLIST` origins.
- DLP redacts secrets from every message before forwarding and before logging. It is pattern-based and cannot catch everything.
- The ledger is chained with SHA-256 and signed with an Ed25519 node key (`NODE_KEY_PATH`); it detects tampering but does not prevent deletion of the whole database.

## Reporting a Vulnerability

Please do not open a public issue. Use a confidential GitHub Security Advisory on the repository, or email `security@agenttrace.dev`, with a description, a minimal reproduction, and the impact.
