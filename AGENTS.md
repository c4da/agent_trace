# AgentTrace Cluster & Local Agent Guidelines

Welcome to **AgentTrace Gateway**, the high-performance, cryptographic Rust reverse proxy and multi-node telemetry mesh for distributed AI agent operations.

---

## 1. Cluster Model Topology & Delegation Matrix

### A. Antigravity (Lead Architect & Orchestrator)
- **Role**: System architecture, multi-file planning, system invariant enforcement, user communication.
- **Model**: Frontier / Cloud Gemini reasoning engine.

### B. Nodes & Models — discover them, don't assume them
Models and nodes change, so none are listed here. Before delegating, look up what is online right now:
- **MCP**: call `inspect_cluster` (nodes, health, models) or `get_delegation_config` (routing roles).
- **HTTP**: `GET http://localhost:8080/v1/models` — every model, tagged with the `node_id` serving it (`LOCAL` = this PC's Ollama).

How discovery works: every 15 s the gateway reads the local Ollama list and each worker's `/v1/models`. Workers that stop answering drop out after 60 s. Pick models by size and role from that live list, and use `auto` if unsure.

### C. Known machines
- **PC-1 (lead, 192.168.1.101)**: runs the gateway (`http://localhost:8080`) and Ollama (`http://127.0.0.1:11434`).
- **PC-2 (worker, 192.168.1.102)**: Hermes agent on `http://192.168.1.102:8642`. **Started manually**, so it is often offline: check that it appears in `/v1/models` before dispatching to it, and fall back to local models if it is missing.
- Workers are registered in `.secrets/agenttrace.env`: `WORKER_UPSTREAMS` (`NAME=url,...`), `UPSTREAM_ALLOWLIST`, and `WORKER_HERMES_KEY` (bearer token sent to workers).

---

## 2. AgentTrace Gateway Endpoints & MCP Interface
- **Native Rust MCP Server**: `.\target\debug\agent_trace_mcp.exe` (Stdio JSON-RPC 2.0 bridge)
- **Terminal Console UI**: `http://localhost:8080/console` (High-density Web TUI)
- **Visual Dashboard UI**: `http://localhost:8080/dashboard`
- **Proxy Endpoint**: `http://localhost:8080/v1/chat/completions` (smart auto-routes by model name!)
- **Aggregated Models Registry**: `http://localhost:8080/v1/models`
- **Ledger Verification**: `http://localhost:8080/api/v1/ledger/verify`
- **Telemetry Ingest**: `http://localhost:8080/api/v1/events`
- **MCP HTTP Endpoint**: `http://localhost:8080/api/v1/mcp`
- **Savings (tokens served locally)**: `http://localhost:8080/api/v1/savings`
- **Real-Time Stream**: `http://localhost:8080/api/v1/stream` (SSE)

---

## 3. The "Agent Tool & Project Book" Workflow Directive
> *"Here's a tool — let your agent read it, he will thank you later."*

1. **Agent Tool, Not Human Tool**: Humans type prompts directly in their editor, terminal, or IDE. Agents interact natively with AgentTrace via the standalone Rust MCP binary (`agent_trace_mcp.exe`).
2. **Autonomous Project Chronicle**: Whenever the agent refactors code, solves architectural problems, or audits security invariants, the agent MUST call `record_project_decision` to log the rationale, tags, and lessons learned. The decision is stored in the signed ledger and shown in the console event stream.
3. **Zero External API Cost**: Secondary boilerplate, unit tests (`cargo test`), telemetry parsing, and documentation must be delegated to cluster models via `dispatch_cluster_prompt`, choosing from the live model list (section 1B).
4. **Security Model**: The gateway binds loopback by default; when `AGENTTRACE_TOKEN` is set, send it as `Authorization: Bearer` or `x-agenttrace-token`. Secrets in prompts are redacted before forwarding or logging.
5. **Cryptographic Provenance**: Events reported to the gateway are logged to maintain an unbroken SHA-256 hash chain and Ed25519 node signatures.

---

## 4. UI Design Philosophy & Implementation Standard ("Work-First" Blueprint)
Whenever designing, building, or refactoring any UI (Web TUI or Dashboard), strictly follow these 6 principles:
1. **Anti-SaaS-Fluff & High Information Density**: Reject oversized marketing cards and wasted whitespace. Prioritize compact monospace grids, terminal/CRT aesthetics (Ghostty, k9s, htop), and 30–50 visible records at a glance.
2. **Scenario-First Architecture (Scenarios Sell The App)**: Build the layout and filters directly around the top 3–5 concrete questions, fears, and real-world incidents the user faces.
3. **Live Interactive Demonstrations**: Include a `[⚡ Live Scenario Demos]` toolbar allowing users to trigger and test-drive real-world scenarios with instant visual proofs (diffs, alerts, explainer cards).
4. **Manual-First Ergonomics (Human-in-the-Loop Forensics)**: Users want to inspect, test, and verify things manually before trusting automation. Always provide: compound grep filtering, stream pause/freeze (`P`), step-by-step navigation (`j`/`k`), manual event flagging/pinning (`F`), and raw JSON/CSV export.
5. **Keyboard-First Navigation**: Every primary action must have a single-key binding (`j`/`k`, `/`, `:`, `V`, `P`, `D`, `B`, `F`, `?`).
6. **Dual-Mode & Zero Bloat**: Preserve visual overviews (`/dashboard`) alongside the operator console (`/console`) with a 1-click toggle, using fast, zero-dependency code.

