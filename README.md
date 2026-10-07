# 🌐 AgentTrace Gateway (`agent_trace_gw`)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.78%2B-orange.svg)](https://www.rust-lang.org/)
[![Ed25519 Ledger](https://img.shields.io/badge/Ledger-Ed25519%20Signed-green.svg)](src/ledger.rs)
[![Zero Auth Friction](https://img.shields.io/badge/Auth-Zero%20Login%20Required-success.svg)](#-licensing--openclaw-business-model)

High-performance, ultra-low-footprint Rust reverse proxy & cryptographic telemetry gateway for distributed AI agent clusters.

- **RAM Footprint:** ~20 MB (vs 500MB+ in Spring Boot), zero GC pauses, no VRAM contention.
- **Cryptographic Provenance:** Authoritative SHA-256 hash-chaining and Ed25519 node identity digital signatures on every tool call and LLM inference.
- **Smart Auto-Routing:** OpenAI-compatible proxy that automatically resolves and routes requests to the optimal cluster node (`PC-1` or `PC-2`) based on the requested model name.
- **Streaming:** Native async Server-Sent Events (SSE) & byte pass-through via Tokio and Axum.
- **Zero-Friction Operations:** Single compiled native binary (`agent_trace_gw.exe`), zero database setup (embedded SQLite WAL mode), zero mandatory external services.

---

## ⚖️ Licensing & OpenClaw Commercial Model

**AgentTrace Gateway** follows the **OpenClaw** operational philosophy:

1. **100% Free & Open Source:** Licensed under the permissive **[MIT License](LICENSE)**. Free for personal, academic, and commercial use with no artificial functional limitations.
2. **Zero Login, Zero Paywalls:** No user accounts, no login portals, no telemetry phone-home requirements, and no local feature gating. All core capabilities—distributed routing, cryptographic ledger, Ed25519 signing, and real-time dashboard—are fully accessible out of the box.
3. **Sustainable Monetization Strategy:**
   - **Managed Fleet Relay (Cloud):** Turnkey multi-region edge mesh for teams orchestrating agents across distributed datacenters and clouds without managing manual VPNs or port forwards.
   - **Enterprise Cryptographic Compliance Packages:** Automated SOC2 Type II, ISO 27001, and EU AI Act audit exports with cryptographically verifiable tamper-proof ledgers.
   - **Custom Agent Tooling & Swarm Integrations:** Bespoke MAVLink/PX4, CAD/FEM solver hooks, and dedicated enterprise SLA support.

---

## 🏗️ Cluster Topology

```
                  ┌─────────────────────────────────────────────────────────┐
                  │                 Lead Node (PC-1: VECTOR)                │
                  │  • Antigravity (Architect & Reasoning Lead)             │
                  │  • Hermes CLI & Ollama (:11434)                         │
                  │    - qwen3.8:27b (Heavy Reasoning Lead)                 │
                  │    - qwen3:14b (Fast Code & Boilerplate Partner)        │
                  │    - gemma4:latest (Offline Executor)                   │
                  └────────────────────────────┬────────────────────────────┘
                                               │
               ┌───────────────────────────────┴───────────────────────────────┐
               ▼                                                               ▼
┌──────────────────────────────────────────────┐        ┌──────────────────────────────────────────────┐
│       AgentTrace Gateway (Rust :8080)        │        │          Worker Node (PC-2: NODE-02)         │
│  • Smart Model Auto-Router                   │◄───────┤  • Hermes Agent Gateway (:8642)              │
│  • Cryptographic Ledger V2 + Ed25519         │        │  • deepseek-r1:1.5b (Fast GPU Reasoning)     │
│  • SSRF-Hardened Upstream Allowlist          │        │  • Gemma 4 on CUDA (Boilerplate & Tests)     │
│  • Embedded Real-Time Dashboard              │        │                                              │
│  • SQLite WAL Ledger (cluster_telemetry.db)  │        │  Run: agent_trace_gw.exe --worker            │
└──────────────────────────────────────────────┘        └──────────────────────────────────────────────┘
```

---

## 🧠 Smart Model Awareness & Auto-Routing

AgentTrace Gateway eliminates manual endpoint switching. Clients connect directly to `http://localhost:8080/v1` using standard OpenAI SDKs:

| Requested Model | Target Host | Node ID | Assigned Cluster Role |
| :--- | :--- | :--- | :--- |
| `qwen3.8:27b` | `http://127.0.0.1:11434` | `PC-1` | Heavy Reasoning & Architecture Lead |
| `qwen3:14b` | `http://127.0.0.1:11434` | `PC-1` | Fast Code Generation & Boilerplate Partner |
| `gemma4:latest` | `http://127.0.0.1:11434` | `PC-1` | Local Offline Generalist |
| `deepseek-r1:1.5b` | `http://192.168.1.102:8642` | `PC-2` | Ultra-Fast GPU Reasoning & Sanity Checks |
| `hermes-agent` | `http://192.168.1.102:8642` | `PC-2` | Autonomous Swarm Worker & Test Runner |

### 1. Unified Cluster Model Catalog
```bash
curl -s http://localhost:8080/v1/models
```
Returns an aggregated list of models across both PC-1 and PC-2, tagged with active node IDs, hosts, and capability roles.

### 2. Auto-Routed Completion Call
```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="not-needed"
)

# Gateway automatically routes this to PC-2 (:8642), attaches worker auth, and signs trace block:
response = client.chat.completions.create(
    model="hermes-agent",
    messages=[{"role": "user", "content": "Execute offline benchmark suite."}]
)
print(response.choices[0].message.content)
```

---

## 🔒 Cryptographic Ledger & Zero-Trust Audit Trail

Every file read, file modification (with full unified diff), shell execution, and LLM inference is cryptographically hashed and linked in a server-authoritative hash-chain:

- **Canonical V2 Hashing:** Length-prefixed SHA-256 digest covering `event_id`, `node_id`, `session_id`, `timestamp`, `event_type`, `target`, `action_details`, `diff`, `provenance_sources`, `security_verdict`, `security_reasons`, and `duration_ms`.
- **Ed25519 Node Signatures:** Every block is signed with the node's private key (`.secrets/node_identity.pk8`).
- **SSRF Prevention:** Upstream allowlist (`UPSTREAM_ALLOWLIST`) strictly restricts outgoing proxy requests to authorized nodes, blocking forged `X-Upstream-Url` headers with `403 Forbidden`.
- **Backwards Compatibility:** Transparently verifies legacy V1 blocks and V2 blocks.

### Verifying the Ledger
```bash
curl -s http://localhost:8080/api/v1/ledger/verify
```
**Response:**
```json
{
  "is_valid": true,
  "total_blocks": 3146,
  "genesis_hash": "0000000000000000000000000000000000000000000000000000000000000000",
  "latest_block_hash": "f52a1e141e45e46fda414f61b030d7bfcfdc4f19fd0b1308e05ff042a8829d79",
  "broken_block_id": null,
  "error_reason": null
}
```

---

## 🚀 Quick Start

### 1. Build & Run on Lead Node (PC-1)
```powershell
cd C:\cada\projects\agent_trace_gw
cargo build --release
.\target\release\agent_trace_gw.exe
```
By default, listens on `0.0.0.0:8080` with dashboard at **`http://localhost:8080/dashboard`**.

### 2. Run as Worker Node on PC-2
```powershell
.\target\release\agent_trace_gw.exe --worker --lead http://192.168.1.101:8080
```
*(See [`docs/WORKER_NODE_SETUP.md`](docs/WORKER_NODE_SETUP.md) for full system service setup on Linux and Windows).*

---

## 🕵️ Verification & Inspection Tools

- **[`cluster_inspector.py`](cluster_inspector.py):** Comprehensive CLI tool for security audits, blast-radius scoring, and node telemetry queries.
- **[`cluster_verification_benchmark.py`](cluster_verification_benchmark.py):** Pure Python standard library benchmark verifying cluster models, ledger integrity, and round-trip routing latency.
- **Autonomous Delegation:** Delegate audit tasks to local agents at zero external token cost:
  ```powershell
  & "C:\Users\milos\AppData\Local\hermes\bin\hermes.cmd" -z "python cluster_inspector.py --audit"
  ```

---

## 📄 License

This project is licensed under the **MIT License** — see the [LICENSE](LICENSE) file for details. 
Completely free, open-source, and unrestricted for personal, academic, and commercial usage.
