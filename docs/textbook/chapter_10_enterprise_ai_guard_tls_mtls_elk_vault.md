# Chapter 10: Progressive Enterprise Security — Optional TLS/mTLS, ChaCha20-Poly1305 AEAD Vaults, Hard AI Guard & Elastic SIEM (ECS 8.11)

---

## 10.1 The "Progressive Security" Philosophy: Zero-Friction by Default, Zero-Trust on Demand

One of the hardest architectural trade-offs in systems engineering is balancing **developer ergonomics** against **enterprise security compliance**:
- If a tool forces a solo developer on a laptop to generate X.509 certificates, configure an external Vault, and pass Bearer tokens just to inspect local prompts on `localhost:8080`, nobody will use it.
- Conversely, if a tool lacks encrypted secrets at rest, mutual authentication (mTLS), pre-execution DLP blocking, and SIEM/ELK integration, corporate security teams will reject it in production.

**AgentTrace (`v1.0.0`) solves this via Progressive Opt-In Security**:
Every enterprise security layer is compiled directly into the single `12.45 MB` Rust binary, but **all restrictive locks are optional and disabled by default**:

| Security Capability | Default Out-of-the-Box Behavior (Zero Config) | How to Opt-In for Corporate / Production | Implementation Module |
| :--- | :--- | :--- | :--- |
| **Secrets Storage (`.secrets/agenttrace.env`)** | **Plaintext (`KEY=value`) by default** — edit with any text editor | Encrypt individual values with `POST /api/v1/vault/encrypt` (`ENCv1:<nonce>:<cipher>`) | [`src/secrets.rs`](../../src/secrets.rs) · [`src/security_hardening.rs`](../../src/security_hardening.rs) |
| **API / Bearer Token Auth** | **Disabled (Permissive)** — local tools connect without headers | Set `AGENTTRACE_REQUIRE_AUTH=true` + `AGENTTRACE_TOKEN=<secret>` | [`src/secrets.rs`](../../src/secrets.rs) |
| **Inter-Gateway TLS & mTLS** | **Disabled** — plain HTTP on trusted home LAN | Set `AGENTTRACE_MTLS_REQUIRED=true` + cert thumbprints & HMAC secret | [`src/security_hardening.rs`](../../src/security_hardening.rs) |
| **3rd-Party SIEM / ELK Export** | **Disabled** — stores locally in embedded SQLite WAL | Set `AGENTTRACE_ELK_URL=http://...:9200` or `AGENTTRACE_ELK_SPOOL=logs/ecs.ndjson` | [`src/siem_exporter.rs`](../../src/siem_exporter.rs) |
| **AI Guard Mode (`/v1/messages`)** | **Configurable (`HardBlock`, `RedactAndForward`, `AuditOnly`)** | Set `AGENTTRACE_GUARD_MODE=hard` or `redact` or `audit` | [`src/ai_guard.rs`](../../src/ai_guard.rs) · [`src/claude_bridge.rs`](../../src/claude_bridge.rs) |

---

## 10.2 Optional Secrets-at-Rest Encryption (`ChaCha20-Poly1305` AEAD Vault)

By default, `.secrets/agenttrace.env` stores keys in standard plaintext (`WORKER_HERMES_KEY=my-secret-key-12345`) so developers can inspect and edit their configuration effortlessly.

When deploying on shared corporate workstations or production servers, operators can optionally encrypt any sensitive value using **Authenticated Encryption with Associated Data (AEAD)** via `ring::aead::CHACHA20_POLY1305` ([`src/security_hardening.rs`](../../src/security_hardening.rs)):

```text
ENCv1:<12-byte-CSPRNG-nonce-hex>:<ciphertext-and-16-byte-Poly1305-MAC-tag-hex>
```

### Why `ChaCha20-Poly1305`?
1. **Constant-Time Software Speed (`10 µs` roundtrip)**: Unlike AES-GCM on CPUs without hardware AES-NI instructions, Daniel J. Bernstein's `ChaCha20` stream cipher paired with the `Poly1305` universal authenticator runs in pure constant-time ARX (Add-Rotate-XOR) operations, immune to cache-timing attacks.
2. **Cryptographic Tamper Rejection (AEAD)**: Every encrypted secret binds the Additional Authenticated Data (`AAD = "agenttrace-secret-v1"`) and appends a 128-bit authentication tag. If an attacker flips even a single bit of the ciphertext in `.secrets/agenttrace.env`, `open_in_place` rejects the secret before decryption.
3. **Hybrid Plaintext + Encrypted Coexistence**: Inside [`ClusterSecurityConfig::load()`](../../src/secrets.rs), each line of `.secrets/agenttrace.env` is inspected independently:
   - If the value starts with `ENCv1:`, it is decrypted in-memory using `EncryptedSecretVault`.
   - Otherwise, it is read directly as plaintext with zero overhead.

---

## 10.3 Constant-Time Bearer Authentication (`constant_time_eq`)

When `AGENTTRACE_REQUIRE_AUTH=true` is enabled, the Gateway checks `Authorization: Bearer <token>`, `x-agenttrace-token`, or `x-api-key`.

Standard string comparison (`a == b`) short-circuits on the first mismatched byte, allowing an attacker to measure nanosecond response-time differences to guess a token byte-by-byte. AgentTrace prevents this via a bitwise XOR accumulator in [`src/security_hardening.rs`](../../src/security_hardening.rs):

```rust
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.is_empty() || b_bytes.is_empty() || a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (&x, &y) in a_bytes.iter().zip(b_bytes.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
```

---

## 10.4 Transport Layer Security (TLS) & Inter-Gateway Mutual TLS (mTLS)

In a multi-node AgentTrace cluster (e.g., `PC-1` Lead Node communicating with `PC-2` Worker Node or across corporate VLANs), [`InterGatewayMtlsConfig`](../../src/security_hardening.rs) provides two complementary layers of optional peer verification:

1. **X.509 Client Certificate Thumbprint Pinning (`x-client-cert-sha256`)**:
   - When `AGENTTRACE_MTLS_THUMBPRINTS` is configured with a comma-separated list of trusted SHA-256 certificate fingerprints, the Gateway verifies that the peer's client certificate matches an explicitly pinned identity.
2. **HMAC-SHA256 Request Signing & 60-Second Anti-Replay Window**:
   - Every inter-gateway request carries `x-gateway-node-id`, `x-gateway-timestamp` (Unix epoch seconds), and `x-gateway-hmac`:
     $$\text{HMAC}_{\text{SHA256}}\Big(K_{\text{cluster}},\; \text{node\_id} \;\|\; \text{timestamp} \;\|\; \text{SHA256}(\text{body})\Big)$$
   - If a packet is captured and replayed more than `60 seconds` later (`|now - req_ts| > 60`), or if a single byte of the JSON payload is modified in transit, `verify_peer_gateway` immediately rejects the request.

---

## 10.5 Hard Pre-Execution AI Guard & Corporate Claude Code Terminal Hub

Unlike passive telemetry collectors that log events *after* execution, [`src/ai_guard.rs`](../../src/ai_guard.rs) and [`src/claude_bridge.rs`](../../src/claude_bridge.rs) place a synchronous **`1.07 ms` Pre-Execution Gate** in front of both OpenAI-compatible (`/v1/chat/completions`) and Anthropic-native (`POST /v1/messages`) traffic:

1. **Data Loss Prevention (DLP) Secret Scanner**:
   - Detects and scrubs Anthropic (`sk-ant-*`), OpenAI (`sk-proj-*`), AWS (`AKIA*`), GCP (`AIzaSy*`), GitHub (`ghp_*`, `github_pat_*`), Slack (`xoxb-*`), full multi-line PEM private keys, and `.env` credential assignments.
2. **Three Configurable Enforcement Modes**:
   - **`HardBlock`**: Stops the request locally (`0 bytes` sent to cloud) and returns a structured security explanation inside the developer's `claude` terminal (or HTTP `403 Forbidden`).
   - **`RedactAndForward`**: Replaces secrets in-flight with `[REDACTED:...]` across both string prompts and structured content-block arrays, allowing the developer's Claude session to continue uninterrupted.
   - **`AuditOnly`**: Logs the finding to the signed SQLite ledger and SIEM without modifying or blocking the request.
3. **Distinct Multi-Agent Fingerprinting ([`src/agent_identity.rs`](../../src/agent_identity.rs))**:
   - Assigns every agent session and subagent a canonical identity `<engine>:<role>@<host>#<instance_short>` (e.g., `claude-code:terminal@VECTOR#pid4812`, `Antigravity:image-generator@PC-1#9c5286`) so multiple concurrent agents on the same machine never collide in the audit trail.

---

## 10.6 Enterprise 3rd-Party Logging: ELK Stack & Elastic Common Schema (ECS 8.11)

For organizations using **Elasticsearch / Kibana (ELK)**, **Logstash**, **Filebeat**, or **Splunk**, [`src/siem_exporter.rs`](../../src/siem_exporter.rs) translates every `TraceEvent` into **Elastic Common Schema (ECS 8.11.0)** JSON documents:

- **On-Demand Bulk Export (`GET /api/v1/export/elk?limit=100&format=bulk`)**:
  - Outputs Elasticsearch `/_bulk` NDJSON (`{"index":{"_index":"agenttrace-ecs-events"}}` followed by the ECS document line) ready for direct `curl` or Logstash HTTP polling.
- **Optional Live Background Shipping & Local Spooling**:
  - Setting `AGENTTRACE_ELK_URL` ships blocked/audited events asynchronously via a non-blocking Tokio task (`ship_event_to_siem_async`), while `AGENTTRACE_ELK_SPOOL` appends NDJSON lines to a local file for Filebeat tailing.
