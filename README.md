# ⚡ AgentTrace (`v1.1.0`) — Local Rust Runtime Firewall, CAS Rollback Guard & Cryptographic Audit Ledger for AI Coding Agents

> **A 16 MB self-contained Rust runtime that intercepts destructive commands, scrubs leaked secrets before they reach the model, provides instant 1.1 ms rollback for damaged files, and maintains a tamper-evident SHA-256 audit ledger for Claude Code (`claude`), Cursor, Copilot CLI, and Antigravity.**  
> **Zero Python/Docker dependencies. Fails closed by default. Runs locally on `127.0.0.1:8080`.**

[![Release](https://img.shields.io/badge/Release-v1.1.0-blue.svg)](CHANGELOG.md)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.78%2B-orange.svg)](https://www.rust-lang.org/)
[![Footprint](https://img.shields.io/badge/Memory-16.24%20MB%20RSS%20%7C%201.1ms%20Latency-success.svg)](docs/BENCHMARKS.md)
[![Tests](https://img.shields.io/badge/Tests-155%2B%20Tests%20Passing%20(All%20Targets)-brightgreen.svg)](docs/BENCHMARKS.md)
[![AI Guard](https://img.shields.io/badge/AI%20Guard-DLP%20Scrubbing%20%2B%20Instant%20CAS%20Undo-critical.svg)](src/ai_guard.rs)
[![Agensi Skills](https://img.shields.io/badge/Marketplace-Agensi.io%20Skills%20Repo-purple.svg)](https://github.com/agensi-io/agensi-skills)

---

## 🎯 What Problem Does AgentTrace Solve?

Autonomous AI coding agents (Claude Code, Cursor, Copilot CLI, Antigravity) are granted wide shell and filesystem permissions on developer workstations. Without a local runtime firewall:

1. **Destructive Shell Execution**: A hallucinated or confused command (`rm -rf /`, `git push --force`, `chmod -R 777`, `DROP TABLE`, `eval $(curl ...)`) can wipe or break your environment.
2. **Upstream Credential Leaks**: Pasted logs, stack traces, or `.env` files leak raw API keys (`sk-ant-*`, `sk-proj-*`, `AKIA*`, `AIzaSy*`, `ghp_*`, PEM certificates) directly to external model providers.
3. **Catastrophic Refactoring Corruption**: When an agent attempts a multi-file refactor and breaks syntax or deletes critical source files, standard `git checkout` can discard uncommitted work.
4. **Unbounded Path Escapes & Blast Radius**: Without strict boundary enforcement, agents can wander outside the repository, modifying sibling projects, altering .git/hooks, or reading sensitive user directories (~/.ssh, ~/.aws, system folders).

**AgentTrace sits between the agent and your system as a transparent, high-performance runtime guard.** It runs as a lightweight native binary (`~16 MB` RSS, sub-2ms latency) that catches threats before execution and guarantees forensic recovery.

---

## 🛡️ Core Capabilities (Active by Default)

### 1. Synchronous Pre-Flight Firewall (`intercept_ai_guard` / `POST /api/v1/guard/intercept`)
- Synchronously evaluates proposed shell commands, file modifications, and prompt payloads in **`~1.1 ms`**.
- Intercepts destructive patterns: recursive deletes, pipeline downloads (`curl | sh`, `irm | iex`), forced branch pushes, database drops, and suspicious obfuscation.
- Scans outbound prompts and files for sensitive secrets via regex DLP, redacting tokens or rejecting requests in `HardBlock` mode.

### 2. Pre-Image CAS Rollback Guard (`protect_or_revert_cas_file` / `POST /api/v1/cas/protect`)
- Before a multi-file refactor or risky modification, target files are snapshotted into in-memory SHA-256 Content-Addressable Storage (CAS).
- If a sub-agent corrupts, truncates, or **deletes** a file, AgentTrace restores the exact original byte stream to disk in **`~1.1 ms`** without losing uncommitted git progress.

### 3. Cryptographic Audit Ledger (`GET /api/v1/ledger/verify`)
- Every tool touchpoint, command execution, and file modification is signed with an Ed25519 node key and committed to a local SHA-256 hash-chain.
- Any unauthorized post-facto modification to past database records breaks the cryptographic hash link and triggers an immediate verification alert.

### 5. High-Density Operator Console (`GET /console`)
- Built following the **"Work-First"** operator standard (Ghostty / k9s / htop aesthetics).
- Visualizes 30–50 live telemetry records at a glance with keyboard shortcuts (`j`/`k` navigation, `P` screen freeze, `/` grep filter, `M` sandbox evaluator).

### 6. Native MCP Server for Coding Agents
- Native `agent_trace_mcp.exe` communicates over standard `stdio` JSON-RPC 2.0 with Claude Code, Cursor, Copilot CLI, and Antigravity.
- Adheres strictly to the MCP specification with full `inputSchema` parameter schemas.

---

## 📦 Agensi.io Reference Skills (`skills/agensi/`)

In partnership with **[Agensi.io](https://agensi.io)**, AgentTrace provides reference skills installable across 20+ agent environments:

| Skill Directory | Manifest & Spec | Capability & Purpose |
| :--- | :--- | :--- |
| **[`skills/agensi/agent-runtime-firewall/`](skills/agensi/agent-runtime-firewall/)** | [`manifest.json`](skills/agensi/agent-runtime-firewall/manifest.json) | Pre-flight safety checks, destructive command blocking, DLP secret redaction, and Pre-Image CAS rollback. |
| **[`skills/agensi/local-model-offload/`](skills/agensi/local-model-offload/)** | [`manifest.json`](skills/agensi/local-model-offload/manifest.json) | Zero-token local model delegation for unit test scaffolding, boilerplate drafting, and first-pass code reviews. |
| **[`skills/agensi/agent-decision-ledger/`](skills/agensi/agent-decision-ledger/)** | [`manifest.json`](skills/agensi/agent-decision-ledger/manifest.json) | Cryptographic milestone chronicle and living Studio Project Book audit trail for autonomous agent actions. |

> **Architectural Split**: Agensi.io serves as the skill marketplace, distribution channel, and curated package registry. The AgentTrace repository remains the open-source, vendor-neutral runtime and cryptographic trust anchor.

---

## 🔒 Security Architecture & Network Boundaries

AgentTrace strictly adheres to a **Safe-by-Default, Fail-Closed** architecture:

- **Loopback Binding by Default**: AgentTrace binds exclusively to `127.0.0.1:8080`. External machines on your local network cannot access endpoints unless you explicitly pass the `--insecure-lan` flag or set `AGENTTRACE_ALLOW_INSECURE_LAN=true`.
- **Fail-Closed Authentication**: When `AGENTTRACE_REQUIRE_AUTH=true` is enabled, any missing or malformed token fails closed with `401 Unauthorized`. Constant-time comparison prevents timing side-channels.
- **Disabled Remote Deployment**: The `/api/v1/cluster/deploy-worker` endpoint is disabled by default. It requires explicit activation via `--enable-remote-deploy` or `AGENTTRACE_ENABLE_REMOTE_DEPLOY=true` and requires valid authentication.
- **Inter-Gateway HMAC Mutual Auth**: When nodes forward heartbeats or telemetry batches across a cluster, requests carry an `x-gateway-hmac` signature with anti-replay timestamp verification. Inbound handlers strictly reject forged or replayed payloads.

---

## 🚀 Quickstart & Verification

### 1. Build and Run
```bash
git clone https://github.com/c4da/agent_trace.git
cd agent_trace

# Run the complete test suite (155+ unit, adversarial, schema, and HTTP integration tests):
cargo test --all-targets

# Start the gateway on localhost:8080:
cargo run --bin agent_trace_gw
```

### 2. Connect Your Coding Agent via MCP
Add to your Claude Code or Cursor MCP configuration (`claude_desktop_config.json` or `.cursor/mcp.json`):
```json
{
  "mcpServers": {
    "agent-trace": {
      "command": "C:/cada/projects/agent_trace_gw/target/debug/agent_trace_mcp.exe",
      "args": []
    }
  }
}
```

### 3. Connect Claude Code CLI Directly
To route Claude Code through AgentTrace's transparent DLP and CAS guard proxy:
```bash
export ANTHROPIC_BASE_URL=http://127.0.0.1:8080
claude
```

---

## 🧩 Optional Subsystems (Disabled by Default)

To keep the core attack surface minimal and focused, peripheral capabilities are structured as optional modules:

- **LAN Multi-Node Cluster Delegation (`src/delegation_engine.rs`)**: Smart auto-routing between local Ollama (`qwen3.8:27b`), CUDA edge workers (`deepseek-r1:1.5b`), and cloud models.
- **Autonomous Skill Distillation ("Auto-Skill Forge" — `src/skill_forge.rs`)**: Mines recurring debugging and repair patterns from session transcripts into reusable skills.
- **Pluggable 3-Tier Persistence (`src/persistence.rs`)**: Synchronizes local state to external PostgreSQL / MySQL databases (`AGENTTRACE_DB_URL`) with graceful fallback.
- **SIEM / ELK Exporter (`src/siem_exporter.rs`)**: Streams security verdicts in Elastic Common Schema (`ECS 8.11`) format for corporate SOC integration.

---

## 📜 License

Dual-licensed under **MIT** or **Apache-2.0** at your option.
