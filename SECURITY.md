# 🛡️ Security Policy & Vulnerability Disclosure

AgentTrace Gateway is designed to provide cryptographic auditability, spatial sandboxing, and policy enforcement for autonomous AI coding agents. We take security vulnerabilities seriously.

---

## 1. Supported Versions

Only the latest release on the `main` branch is actively supported with security updates.

| Version | Supported | Security Maintenance |
| :--- | :---: | :--- |
| `0.1.x` (main) | ✅ Yes | Active cryptographic ledger & policy patches |
| `< 0.1.0` | ❌ No | Deprecated |

---

## 2. Threat Model & Security Boundaries

AgentTrace Gateway enforces multiple distinct security barriers:

1. **Spatial Safe Zones & VaultNoAccess**:
   - Sensitive directories (`.secrets`, `.ssh`, `.env`, `node_identity.key`) are classified as `VaultNoAccess`.
   - Lexical path normalization (`resolve_canonical_target`) collapses directory traversal (`..`) attempts before evaluating access permissions.
   - Untrusted agents are confined to `./playground`, where source trees (`src/`) are mounted read-only and command execution is strictly whitelisted.
2. **Cryptographic Ledger Integrity**:
   - Every telemetry event is hashed via SHA-256 and signed with Ed25519.
   - Any tampering or modification of past records invalidates the hash chain and triggers immediate security alerts.
3. **SSRF Origin Tuple Defense**:
   - Outbound proxy requests enforce RFC 3986 canonical origin tuple comparison `(scheme, host, port)` against network whitelists.
   - Naive prefix matching is strictly forbidden to prevent subdomain collision attacks (e.g., `http://127.0.0.1.attacker.io`).

---

## 3. Reporting a Vulnerability

If you discover a security vulnerability, **please do not open a public issue.**

Instead, report it responsibly via:
- **Email**: Send detailed reproduction steps and logs to `security@agenttrace.dev` (or open a confidential GitHub Security Advisory).
- **Encrypted Communication**: Include PGP/GPG encrypted details if attaching proof-of-concept exploits.

### What to Include in Your Report:
1. Description of the vulnerability (e.g., path traversal bypass, ledger collision, SSRF origin escape).
2. Minimal reproducible example or curl script.
3. Impact assessment (e.g., unauthorized file access, ledger tampering).
4. Proposed patch or mitigation if available.

---

## 4. Response Timeline & SLA

- **Initial Acknowledgment**: Within **24 hours**.
- **Triage & Reproduction**: Within **48 hours**.
- **Fix & Advisory Release**: Critical vulnerabilities patched within **7 calendar days**.
