# Chapter 10: Progressive Enterprise Security — Admin Per-Model Governance, Shielded Client Layer, ChaCha20-Poly1305 AEAD Vaults, Hard AI Guard & Elastic SIEM (ECS 8.11)

---

## 10.1 The "Progressive Security" Philosophy: Zero-Friction by Default, Zero-Trust on Demand

One of the hardest architectural trade-offs in systems engineering is balancing **developer ergonomics** against **production security compliance**:
- If a tool forces a solo developer on a laptop to generate X.509 certificates, configure an external Vault, and pass Bearer tokens just to inspect local prompts on `localhost:8080`, nobody will use it.
- Conversely, if a tool lacks per-model Admin governance, encrypted secrets at rest, mutual authentication (mTLS), pre-execution DLP blocking, and SIEM/ELK integration, security teams will reject it in production.

**AgentTrace (`v1.0.0`) solves this via Progressive Opt-In Security**:
Every enterprise security layer is compiled directly into the single `~13 MB` Rust binary, but **all restrictive locks are optional and disabled by default**:

| Security Capability | Default Out-of-the-Box Behavior (Zero Config) | How to Opt-In for Production | Implementation Module |
| :--- | :--- | :--- | :--- |
| **Admin Per-Model Governance** | **Pre-configured profiles for `claude-*`, `gpt-*`, `qwen3.8:27b`, `deepseek-r1:1.5b`** | Customize via `POST /api/v1/admin/governance/model` + `AGENTTRACE_ADMIN_KEY` | [`src/admin_governance.rs`](../../src/admin_governance.rs) |
| **Secrets Storage (`.secrets/agenttrace.env`)** | **Plaintext (`KEY=value`) by default** — edit with any text editor | Encrypt individual values with `POST /api/v1/vault/encrypt` (`ENCv1:<nonce>:<cipher>`) | [`src/secrets.rs`](../../src/secrets.rs) · [`src/security_hardening.rs`](../../src/security_hardening.rs) |
| **API / Bearer Token Auth** | **Disabled (Permissive)** — local tools connect without headers | Set `AGENTTRACE_REQUIRE_AUTH=true` + `AGENTTRACE_TOKEN=<secret>` | [`src/secrets.rs`](../../src/secrets.rs) |
| **Inter-Gateway TLS & mTLS** | **Disabled** — plain HTTP on trusted home LAN | Set `AGENTTRACE_MTLS_REQUIRED=true` + cert thumbprints & HMAC secret | [`src/security_hardening.rs`](../../src/security_hardening.rs) |
| **3rd-Party SIEM / ELK Export** | **Disabled** — stores locally in embedded SQLite WAL | Set `AGENTTRACE_ELK_URL=http://...:9200` or `AGENTTRACE_ELK_SPOOL=logs/ecs.ndjson` | [`src/siem_exporter.rs`](../../src/siem_exporter.rs) |
| **AI Guard Mode (`/v1/messages`)** | **Configurable (`HardBlock`, `RedactAndForward`, `AuditOnly`)** | Set `AGENTTRACE_GUARD_MODE=hard` or `redact` or `audit` | [`src/ai_guard.rs`](../../src/ai_guard.rs) · [`src/claude_bridge.rs`](../../src/claude_bridge.rs) |

---

## 10.2 Admin Per-Model Governance Plane vs. Shielded Client Layer (`src/admin_governance.rs`)

Different AI models have fundamentally different risk profiles:
- **External Cloud Models (`claude-*`, `gpt-*`)**: Outbound prompts leave the local network, requiring strict Data Loss Prevention (DLP).
- **Local Sovereign Reasoning Models (`qwen3.8:27b` on `PC-1`)**: Prompts stay 100% on local GPU hardware, safe to read/write internal source files (`src/`, `tests/`) and run `cargo test`.
- **Small Fast Worker Models (`deepseek-r1:1.5b` on `PC-2`)**: Ultra-fast CUDA execution (`1.5B` parameters), but higher hallucination risk for shell commands—so they should be barred from executing shell commands (`allow_shell_execution: false`) and confined strictly to `playground/` (`confine_to_playground: true`).

In [`src/admin_governance.rs`](../../src/admin_governance.rs), `AdminGovernanceEngine` separates the system into two planes:
1. **The Admin Plane (`/api/v1/admin/governance`)**:
   - Administrators define `GlobalAdminRestrictions` and per-model `ModelRestrictionProfile` entries.
   - `ignore_client_mode_overrides: true` guarantees a client or prompt injection can never pass `X-Guard-Mode: audit` to weaken a `HardBlock` model profile.
   - If an Administrator disables a model (`enabled: false`) and assigns `fallback_model: Some("qwen3.8:27b")`, client requests are **transparently rerouted** to `qwen3.8:27b`.
2. **The Shielded Client Layer (`/v1/messages` & `/api/v1/client/shield-status`)**:
   - The developer simply runs `ANTHROPIC_BASE_URL=http://127.0.0.1:8080 claude` in their terminal—**shielded from everything**.
   - Secrets are scrubbed in-flight, and any action blocked by a model's profile returns a clean, non-leaky terminal message (`🛡️ [AgentTrace Shielded Terminal]`) while full forensics are committed to the Admin's SHA-256 ledger.

---

## 10.3 Optional Secrets-at-Rest Encryption (`ChaCha20-Poly1305` AEAD Vault)

By default, `.secrets/agenttrace.env` stores keys in standard plaintext (`WORKER_HERMES_KEY=my-secret-key-12345`) so developers can inspect and edit their configuration effortlessly.

When deploying on shared workstations or production servers, operators can optionally encrypt any sensitive value using **Authenticated Encryption with Associated Data (AEAD)** via `ring::aead::CHACHA20_POLY1305` ([`src/security_hardening.rs`](../../src/security_hardening.rs)):

```text
ENCv1:<12-byte-CSPRNG-nonce-hex>:<ciphertext-and-16-byte-Poly1305-MAC-tag-hex>
```

### Why `ChaCha20-Poly1305`?
1. **Constant-Time Software Speed (`10 µs` roundtrip)**: Runs in pure constant-time ARX (Add-Rotate-XOR) operations, immune to cache-timing attacks.
2. **Cryptographic Tamper Rejection (AEAD)**: Binds the Additional Authenticated Data (`AAD = "agenttrace-secret-v1"`) and appends a 128-bit authentication tag. Flipping even 1 bit of ciphertext causes `open_in_place` to reject the secret before decryption.
3. **Hybrid Plaintext + Encrypted Coexistence**: Inside [`ClusterSecurityConfig::load()`](../../src/secrets.rs), each line of `.secrets/agenttrace.env` is inspected independently (`ENCv1:` decrypted in-memory; plaintext read with zero overhead).

---

## 10.4 Constant-Time Bearer Authentication (`constant_time_eq`)

When `AGENTTRACE_REQUIRE_AUTH=true` is enabled, the Gateway checks `Authorization: Bearer <token>`, `x-agenttrace-token`, or `x-api-key` using a bitwise XOR accumulator in [`src/security_hardening.rs`](../../src/security_hardening.rs):

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

## 10.5 Transport Layer Security (TLS) & Inter-Gateway Mutual TLS (mTLS)

In a multi-node AgentTrace cluster (`PC-1` Lead Node communicating with `PC-2` Worker Node across VLANs), [`InterGatewayMtlsConfig`](../../src/security_hardening.rs) provides two layers of optional peer verification:
1. **X.509 Client Certificate Thumbprint Pinning (`x-client-cert-sha256`)** via `AGENTTRACE_MTLS_THUMBPRINTS`.
2. **HMAC-SHA256 Request Signing & 60-Second Anti-Replay Window**:
   $$\text{HMAC}_{\text{SHA256}}\Big(K_{\text{cluster}},\; \text{node\_id} \;\|\; \text{timestamp} \;\|\; \text{SHA256}(\text{body})\Big)$$

---

## 10.6 Hard Pre-Execution AI Guard, Pre-Image CAS Undo & SIEM Export

1. **Synchronous Pre-Execution Hard AI Guard ([`src/ai_guard.rs`](../../src/ai_guard.rs))**:
   - Scans prompts and shell commands in `~1.2 ms` for leaked credentials, prompt injections, and destructive commands (`HardBlock`, `RedactAndForward`, `AuditOnly`).
2. **Pre-Image Content-Addressable Storage (CAS) File & Deletion Undo ([`src/cas_guard.rs`](../../src/cas_guard.rs))**:
   - Snapshots files in memory (`POST /api/v1/cas/protect`) and restores corrupted **or deleted** files to disk in `< 2 ms` (`POST /api/v1/cas/enforce`).
3. **3rd-Party SIEM / ELK Stack Exporter ([`src/siem_exporter.rs`](../../src/siem_exporter.rs))**:
   - Translates every `TraceEvent` into **Elastic Common Schema (ECS 8.11.0)** JSON documents (`GET /api/v1/export/elk?limit=100&format=bulk`).
