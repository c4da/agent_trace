# AgentTrace Cluster & Local Agent Guidelines

Welcome to **AgentTrace Gateway**, the high-performance, cryptographic Rust reverse proxy and multi-node telemetry mesh for distributed AI agent operations.

---

## 1. Local Agent Ecosystem & Access Endpoints

### A. Local Gemma 4 (Ollama)
- **Local Host Endpoint**: `http://127.0.0.1:11434` / `http://localhost:11434`
- **Model**: `gemma4:latest`
- **OpenAI Compatible Endpoint**: `http://127.0.0.1:11434/v1`
- **Native Generate API**: `http://127.0.0.1:11434/api/generate`
- **Tags & Status**: `http://127.0.0.1:11434/api/tags`, `http://127.0.0.1:11434/api/ps`

### B. Local Hermes Agent
- **CLI Executable**: `C:\Users\milos\AppData\Local\hermes\bin\hermes.cmd`
- **Config Path**: `C:\Users\milos\AppData\Local\hermes\config.yaml`
- **Inference Routing**: Configured to route via local gateway (`http://localhost:8080/v1`) or Ollama (`http://localhost:11434/v1`)

### C. AgentTrace Gateway (PC-1 Lead Node)
- **Crate Location**: `C:\cada\projects\agent_trace_gw`
- **Executable**: `C:\cada\projects\agent_trace_gw\target\debug\agent_trace_gw.exe`
- **Gateway Address**: `http://0.0.0.0:8080`
- **Live UI Dashboard**: `http://localhost:8080/dashboard`
- **Proxy Endpoint**: `http://localhost:8080/v1/chat/completions`
- **Models Endpoint**: `http://localhost:8080/v1/models`
- **Cryptographic Ledger**: `http://localhost:8080/api/v1/ledger/verify`
- **Telemetry Ingest**: `http://localhost:8080/api/v1/events`
- **Cluster Summary**: `http://localhost:8080/api/v1/summary`
- **Real-Time Stream**: `http://localhost:8080/api/v1/stream` (SSE)

---

## 2. Remote Cluster Node Access (2-PC Topology)

### PC-2 (Worker Node)
- **IP Address**: `192.168.1.102`
- **Port**: `8642`
- **Platform**: `hermes-agent` v0.21.5
- **Auth Token**: stored in `.secrets/agenttrace.env` as `WORKER_HERMES_KEY` (never commit it)
- **Health Check**: `http://192.168.1.102:8642/health`
- **Models List**: `http://192.168.1.102:8642/v1/models`
- **Gateway Dynamic Routing Headers**:
  - `X-Upstream-Url`: `http://192.168.1.102:8642` (must be listed in `UPSTREAM_ALLOWLIST`)
  - `Authorization`: `Bearer $WORKER_HERMES_KEY`
  - `X-Node-Id`: `PC-2`

### Secrets
- All credentials live in `.secrets/agenttrace.env` (gitignored). Template: `secrets.example.env`.
- Environment variables with the same name override the file.
- The gateway's Ed25519 signing identity lives in `.secrets/node_key.pk8`.

---

## 3. Operational Rules & Token Discipline

1. **Zero External API Cost**:
   - Secondary boilerplate, unit tests (`cargo test`), telemetry parsing, and documentation must be delegated to local **Gemma 4** and **Hermes**.
2. **Queue Waiting Protocol**:
   - The local LLM may be busy with local agent tasks. Check queue status or retry with backoff before assuming timeouts.
3. **Cryptographic Provenance**:
   - Every file edit, diff, and command execution should be logged to the gateway to maintain an unbroken SHA-256 hash chain and Ed25519 node signatures.
