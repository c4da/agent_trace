# Changelog

All notable changes to **AgentTrace Gateway & AI Guard** (`agent_trace_gw`) are documented in this file.

## [1.0.0] - 2026-10-25 — First Official Enterprise Release

### 👑 Admin Per-Model Governance Plane & Shielded Client Layer (`src/admin_governance.rs`)
- **Admin Governance Plane (`GET/POST /api/v1/admin/governance`, `POST /api/v1/admin/governance/model`, `POST /api/v1/admin/governance/evaluate`)**:
  - Allows Administrators to configure **Global Restrictions** (`GlobalAdminRestrictions`) and independent **Per-Model Restriction Profiles** (`ModelRestrictionProfile`) for every model family (`claude-*`, `gpt-*`, `qwen3.8:27b`, `deepseek-r1:1.5b`).
  - Per-model granular controls: `trust_tier`, `enabled`, `fallback_model` (transparent rerouting when a model is disabled), `enforcement_mode` (`HardBlock`, `RedactAndForward`, `AuditOnly`), `allow_shell_execution`, `allow_file_writes`, `confine_to_playground`, `allowed_path_prefixes`, `blocked_path_patterns`, `max_prompt_chars`, and `admin_system_prompt_prefix`.
  - **Client Override Shield (`ignore_client_mode_overrides: true`)**: Prevents clients or prompt injections from passing `X-Guard-Mode: audit` to weaken a `HardBlock` model profile.
- **Shielded Client Layer (`GET /api/v1/client/shield-status` & `POST /v1/messages`)**:
  - Developers simply launch `claude` in their terminal (`ANTHROPIC_BASE_URL=http://127.0.0.1:8080 claude`) completely shielded from policy internals, regexes, and vault paths.

### 🛡️ Active AI Guard, Pre-Image CAS File/Deletion Undo & 35 Native MCP Tools
- **Synchronous Pre-Execution Hard AI Guard (`src/ai_guard.rs` / `POST /api/v1/guard/intercept` / MCP `intercept_ai_guard`)**:
  - Physically blocks destructive shell commands (`rm -rf /`, `Format-Volume`, system directory deletion, credential dumps via `mimikatz`/`SAM`/`/etc/shadow`, download cradles `eval $(curl ...)`, `-EncodedCommand`, and reverse shells) before execution (`HTTP 403 Forbidden` in `~1.5 ms`).
  - Integrated **DLP Credential & Secret Scanner** catching Anthropic (`sk-ant-...`), OpenAI (`sk-proj-...`), AWS IAM (`AKIA...`), GCP (`AIzaSy...`), GitHub PAT (`ghp_...`), multi-line PEM private keys, and `.env` credential leaks with support for both `HardBlock` and in-flight `RedactAndForward`.
- **Pre-Image Content-Addressable Storage File & Deletion Undo (`src/cas_guard.rs` / `POST /api/v1/cas/protect` / MCP `protect_or_revert_cas_file`)**:
  - Snapshots protected files into memory with SHA-256 digests (`GET /api/v1/cas/status`, `POST /api/v1/cas/protect`, `POST /api/v1/cas/enforce`) and auto-reverts unauthorized out-of-band file modifications **or rogue file deletions** in `< 2 ms`.
- **35-Tool Native Rust MCP Server (`src/bin/agent_trace_mcp.rs` & `src/agent_interface/schemas.rs`)**:
  - Added 5 new MCP tools: `manage_admin_model_governance`, `protect_or_revert_cas_file`, `intercept_ai_guard`, `encrypt_vault_secret`, and `export_siem_elk`.
- **Claude Code Terminal Hub (`src/claude_bridge.rs` / `POST /v1/messages`)**:
  - Spec-compliant Anthropic Messages API proxy (`/v1/messages` & `/v1/messages/count_tokens`).
  - Connects native `claude` CLI (`ANTHROPIC_BASE_URL=http://127.0.0.1:8080 claude` or `.\scripts\claude_guarded.ps1`) with zero loss of Claude terminal UX (diffs, tool calls, streaming).
- **Distinct Multi-Agent Fingerprinting (`src/agent_identity.rs`)**:
  - Uniquely identifies every agent instance, role, subagent, host, and session (`<engine>:<role>@<host>#<instance>`, e.g., `claude-code:terminal@VECTOR#pid4812`, `Antigravity:image-generator@PC-1#9c5286`).
- **ChaCha20-Poly1305 AEAD Secret Vault, Constant-Time Auth & Inter-Gateway mTLS (`src/security_hardening.rs`)**:
  - Encrypts secrets at rest (`ENCv1:<nonce>:<ciphertext>`) via `ring::aead::CHACHA20_POLY1305`.
  - Constant-time Bearer token & API-key verification.
  - Inter-gateway mTLS client certificate SHA-256 thumbprint pinning (`x-client-cert-sha256`) and HMAC-SHA256 signed anti-replay headers (`x-gateway-hmac`, 60s window).
- **3rd-Party SIEM / ELK Stack Exporter (`src/siem_exporter.rs` / `GET /api/v1/export/elk`)**:
  - Exports all security alerts and ledger events in Elastic Common Schema (**ECS 8.11**) and Elasticsearch `/_bulk` NDJSON format.
- **Empirical Sub-5ms Benchmarks & 127 Verification Tests (`src/benchmarks.rs`, `src/adversarial_tests.rs`, `src/unhappy_scenarios_tests.rs`)**:
  - Automated commit-synchronized benchmark suite (`scripts/update_benchmarks_and_badges.py` & `.githooks/pre-commit`) verifying memory RSS (`~13 MB`), `120` unit/adversarial tests, `7` E2E tests, and `7` live cluster/security scenarios (`scripts/run_live_unhappy_and_security_scenarios.py`).
