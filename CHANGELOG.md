# Changelog

All notable changes to **AgentTrace Gateway & AI Guard** (`agent_trace_gw`) are documented in this file.

## [1.0.0] - 2026-10-24 — First Official Enterprise Release

### 🛡️ Active AI Guard & Corporate Hardening
- **Synchronous Pre-Execution Hard AI Guard (`src/ai_guard.rs` / `POST /api/v1/guard/intercept`)**:
  - Physically blocks destructive shell commands (`rm -rf /`, `Format-Volume`, system directory deletion, credential dumps via `mimikatz`/`SAM`/`/etc/shadow`, and reverse shells) before execution (`HTTP 403 Forbidden`).
  - Integrated **DLP Credential & Secret Scanner** catching Anthropic (`sk-ant-...`), OpenAI (`sk-proj-...`), AWS IAM (`AKIA...`), GCP (`AIzaSy...`), GitHub PAT (`ghp_...`), PEM private keys, and `.env` credential leaks with support for both `HardBlock` and in-flight `RedactAndForward`.
  - Pre-Image Content-Addressable Storage (`PreImageCasGuard`) auto-reverts unauthorized out-of-band file modifications in `< 1 ms`.
- **Corporate Desktop Claude Code Terminal Hub (`src/claude_bridge.rs` / `POST /v1/messages`)**:
  - Spec-compliant Anthropic Messages API proxy (`/v1/messages` & `/v1/messages/count_tokens`).
  - Connects native `claude` CLI (`ANTHROPIC_BASE_URL=http://127.0.0.1:8080 claude` or `.\scripts\claude_guarded.ps1`) with zero loss of Claude terminal UX (diffs, tool calls, streaming).
- **Distinct Multi-Agent Fingerprinting (`src/agent_identity.rs`)**:
  - Uniquely identifies every agent instance, role, subagent, host, and session (`<engine>:<role>@<host>#<instance>`, e.g., `claude-code:terminal@VECTOR#pid4812`, `Antigravity:image-generator@PC-1#9c5286`).
- **ChaCha20-Poly1305 AEAD Secret Vault, Constant-Time Auth & Inter-Gateway mTLS (`src/security_hardening.rs`)**:
  - Encrypts secrets at rest (`ENCv1:<nonce>:<ciphertext>`) via `ring::aead::CHACHA20_POLY1305`.
  - Constant-time Bearer token & API-key verification (`ring::constant_time::verify_slices_are_equal`).
  - Inter-gateway mTLS client certificate SHA-256 thumbprint pinning (`x-client-cert-sha256`) and HMAC-SHA256 signed anti-replay headers (`x-gateway-hmac`, 60s window).
- **Enterprise 3rd-Party SIEM / ELK Stack Exporter (`src/siem_exporter.rs` / `GET /api/v1/export/elk`)**:
  - Exports all security alerts and ledger events in Elastic Common Schema (**ECS 8.11**) and Elasticsearch `/_bulk` NDJSON format.
- **Empirical Sub-5ms Benchmarks & Adversarial Test Suite (`src/benchmarks.rs`, `src/adversarial_tests.rs`)**:
  - Automated commit-synchronized benchmark suite (`scripts/update_benchmarks_and_badges.py` & `.githooks/pre-commit`) verifying memory RSS (`< 20 MB`) and sub-millisecond latencies.
  - Full `YYYY-MM-DD HH:MM:SS` date+time formatting across all Operator Console (`/console`) and Dashboard (`/dashboard`) views.
