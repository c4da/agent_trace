# 🛡️ Security Policy & Vulnerability Disclosure

AgentTrace Gateway (`v1.0.0`) provides active pre-execution AI runtime firewalling, Admin per-model governance, pre-image file/deletion undo, spatial sandboxing, and cryptographic Ed25519/SHA-256 auditability for autonomous AI coding agents. We take security vulnerabilities seriously.

---

## 1. Supported Versions

Only the latest release on the `main` branch is actively supported with security updates.

| Version | Supported | Security Maintenance |
| :--- | :---: | :--- |
| `1.0.x` (main) | ✅ Yes | Active AI Guard, Admin Governance, CAS Guard & Cryptographic Ledger patches |
| `0.1.x` | ⚠️ Legacy | Upgraded to `1.0.x` |

---

## 2. Threat Model & Multi-Layer Security Boundaries

AgentTrace Gateway enforces six distinct security barriers:

1. **Admin Governance Plane vs. Shielded Client Layer (`src/admin_governance.rs`)**:
   - Administrators assign independent `ModelRestrictionProfile` envelopes to each model family (`claude-*`, `gpt-*`, `qwen3.8:27b`, `deepseek-r1:1.5b`) governing shell execution (`allow_shell_execution`), file writes (`allow_file_writes`), playground confinement (`confine_to_playground`), prompt length ceilings (`max_prompt_chars`), and transparent fallback routing (`fallback_model`).
   - Clients connecting via `ANTHROPIC_BASE_URL=http://127.0.0.1:8080 claude` are shielded from internal security rules and cannot weaken `HardBlock` enforcement via HTTP headers (`ignore_client_mode_overrides: true`).
2. **Synchronous Pre-Execution Hard AI Guard (`src/ai_guard.rs`)**:
   - Inspects prompts, bash commands, and file paths in `~1.5 ms` *before* execution or cloud transmission.
   - Blocks destructive shell commands, remote download cradles, obfuscated PowerShell (`-EncodedCommand`), reverse shells, and prompt injections, while scrubbing or blocking leaked API keys and PEM private keys (`HardBlock` vs. `RedactAndForward`).
3. **Pre-Image Content-Addressable Storage (CAS) File & Deletion Undo (`src/cas_guard.rs`)**:
   - Maintains in-memory SHA-256 pre-images of protected files (`POST /api/v1/cas/protect`) and automatically restores exact bytes to disk in `< 2 ms` if an agent corrupts or deletes a guarded file out-of-band.
4. **Spatial Safe Zones & `VaultNoAccess` (`src/policy.rs`, `src/zones.rs`)**:
   - Sensitive directories (`.secrets`, `.ssh`, `.env`, `node_identity.key`) are locked under `VaultNoAccess`.
   - Lexical path normalization (`resolve_canonical_target`) collapses directory traversal (`..`, URL-encoded `%2e%2e`, mixed slashes) before evaluating access permissions.
5. **Cryptographic Ledger Integrity & AEAD Secret Vault (`src/ledger.rs`, `src/security_hardening.rs`)**:
   - Every telemetry event is chained via SHA-256 and signed with Ed25519.
   - Secrets at rest support `ring::aead::CHACHA20_POLY1305` (`ENCv1:<nonce>:<ciphertext>`) authenticated encryption, constant-time token verification (`constant_time_eq`), and inter-gateway mTLS thumbprint pinning + `HMAC-SHA256` 60-second anti-replay checks.
6. **SSRF Origin Tuple Defense (`src/secrets.rs`)**:
   - Outbound proxy requests enforce RFC 3986 canonical origin tuple comparison `(scheme, host, port)` against network whitelists, blocking loopback/metadata smuggling and subdomain collision attacks (`http://127.0.0.1.attacker.io`).

---

## 3. Reporting a Vulnerability

If you discover a security vulnerability, **please do not open a public issue.**

Instead, report it responsibly via:
- **GitHub Security Advisory**: Open a confidential security advisory on the repository.
- **Email**: Send detailed reproduction steps and logs to `security@agenttrace.dev`.

### What to Include in Your Report:
1. Description of the vulnerability (e.g., path traversal bypass, DLP evasion, ledger collision, SSRF origin escape).
2. Minimal reproducible example or `curl` / Python script.
3. Impact assessment (e.g., unauthorized file access, policy downgrade, ledger tampering).
4. Proposed patch or mitigation if available.

---

## 4. Response Timeline & SLA

- **Initial Acknowledgment**: Within **24 hours**.
- **Triage & Reproduction**: Within **48 hours**.
- **Fix & Advisory Release**: Critical vulnerabilities patched within **7 calendar days**.
