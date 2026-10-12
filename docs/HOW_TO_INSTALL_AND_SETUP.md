# ⚡ How to Install & Set Up AgentTrace Gateway

This guide walks you through installing, launching, and verifying **AgentTrace Gateway** in under 60 seconds—from a single standalone workstation to a multi-node LAN cluster.

---

## 1. Prerequisites

- **Rust Toolchain**: `rustc` / `cargo` (`1.75+`) — install via [rustup.rs](https://rustup.rs/)
- **Python 3.8+** *(optional)*: Only needed if you use the zero-dependency CLI SDK (`sdk/agent_trace_client.py`) or single-command provisioner (`/install.py`)
- **Ollama** *(optional)*: Install [Ollama](https://ollama.com) if you want local zero-token inference routing (`qwen3.5:9b`, `deepseek-r1:1.5b`, `qwen3.8:27b`)

---

## 2. Option A: 1-Minute Quickstart (Build from Source)

Clone the repository and run the single-binary Rust server:

```bash
git clone https://github.com/c4da/agent_trace.git
cd agent_trace
cargo run --release
```

As soon as it starts, your operator endpoints are live on `http://localhost:8080`:

| Endpoint | Purpose |
| :--- | :--- |
| **`http://localhost:8080/dashboard`** | **Work-First Operator Dashboard** (Scenario Toolbar, Prompt Hub `H`, Safe Zones `Z`, Manual Forensics) |
| **`http://localhost:8080/console`** | **High-Density Terminal Console (Web TUI)** (Ghostty/k9s style, 30–50 rows visible, keyboard-driven) |
| **`http://localhost:8080/graph`** | **Multi-Agent Problem Contribution Graph** (Bipartite network, 4 switchable metrics, 2 topic modes) |
| **`http://localhost:8080/v1/chat/completions`** | **Drop-in OpenAI-Compatible Proxy** (Auto-routes by model name across all cluster nodes) |
| **`http://localhost:8080/api/v1/hub/dispatch`** | **Central Prompt Hub** (`auto`, `pipeline`, and `swarm` multi-model execution) |

Verify that the gateway is healthy and its cryptographic ledger is intact:

```bash
curl -s http://localhost:8080/api/v1/ledger/verify
```

---

## 3. Option B: Automated Single-Command Provisioner (Windows / Linux / macOS)

If you already have a Lead Node running on your LAN (e.g. `192.168.1.101:8080`) and want to provision a new workstation or worker node with one command, AgentTrace serves dynamic bootstrap scripts directly over HTTP:

### Cross-Platform Python Bootstrap
```bash
curl -sSL http://localhost:8080/install.py | python3 - --role lead
```

### Windows PowerShell Bootstrap
```powershell
irm http://localhost:8080/install.ps1 | iex
```

### Linux / macOS Bash Bootstrap
```bash
curl -sSL http://localhost:8080/install.sh | bash -s -- --role worker --lead-ip 192.168.1.101
```

---

## 4. Option C: Configuring Environment Variables (`.secrets/agenttrace.env`)

AgentTrace works out-of-the-box with zero configuration, storing its cryptographic ledger in `.agent_trace/audit_ledger.jsonl` and its Ed25519 signing key in `.agent_trace/node_identity.key`.

To customize ports, remote worker nodes, or API keys, create `.secrets/agenttrace.env`:

```ini
# Gateway bind address (default: 0.0.0.0:8080)
AGENTTRACE_BIND=0.0.0.0:8080

# Optional: Remote Worker Node (PC-2) endpoint and bearer token
WORKER_NODE_URL=http://192.168.1.102:8642
WORKER_HERMES_KEY=your-worker-secret-token

# Optional: Strict policy mode (block shell execution outside playgrounds)
STRICT_CONTAINMENT=true
```

---

## 5. Connecting Your First AI Agent or IDE

Point any OpenAI-compatible client (Cursor, Continue, Aider, OpenAI SDK, or custom scripts) to `http://localhost:8080/v1`:

```bash
export OPENAI_API_BASE="http://localhost:8080/v1"
export OPENAI_API_KEY="agenttrace-local"
```

Or use the built-in zero-dependency Python CLI right away:

```bash
# Get a live cluster & security briefing
python sdk/agent_trace_client.py --briefing

# Send a prompt through the Central Prompt Hub (auto-selects best model)
python sdk/agent_trace_client.py --ask "Write a Rust function to validate SHA-256 hex strings"
```

---

## 🔗 Related Setup Guides
- [Worker Node Setup Guide (`docs/WORKER_NODE_SETUP.md`)](./WORKER_NODE_SETUP.md) — How to configure a secondary GPU/CPU node (Ollama / Hermes) to join the cluster.
- [Cluster Deployment Guide (`docs/CLUSTER_DEPLOYMENT_GUIDE.md`)](./CLUSTER_DEPLOYMENT_GUIDE.md) — Multi-node network topology, firewall rules, and LAN auto-discovery.
