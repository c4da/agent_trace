# AgentTrace Cluster & Local Agent Guidelines

Welcome to **AgentTrace Gateway**, the high-performance, cryptographic Rust reverse proxy and multi-node telemetry mesh for distributed AI agent operations.

---

## 1. Cluster Model Topology & Delegation Matrix

### A. Antigravity (Lead Architect & Orchestrator)
- **Role**: System architecture, multi-file planning, system invariant enforcement, user communication.
- **Model**: Frontier / Cloud Gemini reasoning engine.

### B. PC-1 Lead Node (VECTOR · 192.168.1.101)
- **Endpoints**:
  - AgentTrace Gateway: `http://localhost:8080` / `http://192.168.1.101:8080`
  - Ollama Engine: `http://127.0.0.1:11434` / `http://127.0.0.1:11434/v1`
  - Hermes CLI: `C:\Users\milos\AppData\Local\hermes\bin\hermes.cmd`
- **Models & Delegation Roles**:
  - `qwen3.8:27b` (Heavy Local Reasoning Partner):
    - Complex architectural refactoring, structural code analysis, deep math/physics problem solving.
  - `qwen3:14b` (Fast Local Code Partner):
    - Rust & Python unit tests (`cargo test`), boilerplate generation, docstrings, telemetry parsers, log analyzers.

### C. PC-2 Worker Node (NODE-02 · 192.168.1.102)
- **Endpoints**:
  - Hermes Agent / Ollama: `http://192.168.1.102:8642` / `http://192.168.1.102:8642/v1`
  - Auth Token: stored in `.secrets/agenttrace.env` as `WORKER_HERMES_KEY`
- **Models & Delegation Roles**:
  - `deepseek-r1:1.5b` (Ultra-Fast GPU Reasoning & Sanity Verification):
    - NVIDIA CUDA acceleration, rapid chain-of-thought verification, syntax checks, regex validation, fast assertion audits at zero token cost.

---

## 2. AgentTrace Gateway Endpoints
- **Dashboard UI**: `http://localhost:8080/dashboard`
- **Proxy Endpoint**: `http://localhost:8080/v1/chat/completions` (smart auto-routes by model name!)
- **Aggregated Models Registry**: `http://localhost:8080/v1/models`
- **Ledger Verification**: `http://localhost:8080/api/v1/ledger/verify`
- **Telemetry Ingest**: `http://localhost:8080/api/v1/events`
- **Cluster Summary**: `http://localhost:8080/api/v1/summary`
- **Real-Time Stream**: `http://localhost:8080/api/v1/stream` (SSE)

---

## 3. Operational Protocols & Zero-Token Discipline
1. **Zero External API Cost**:
   - Secondary boilerplate, unit tests (`cargo test`), telemetry parsing, and documentation must be delegated to PC-1 (`qwen3.8:27b`, `qwen3:14b`) and PC-2 (`deepseek-r1:1.5b`).
2. **Queue Waiting Protocol**:
   - The local LLM may be busy with local agent tasks. Check queue status or retry with backoff before assuming timeouts.
3. **Cryptographic Provenance**:
   - Every file edit, diff, and command execution is logged to the gateway to maintain an unbroken SHA-256 hash chain and Ed25519 node signatures.
