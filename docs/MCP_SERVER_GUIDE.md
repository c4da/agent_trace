# 🔌 AgentTrace Native Rust MCP Server Guide

> **"Here's a tool — let your agent read it, he will thank you later."**

`agent_trace_mcp` is a standalone, blazing-fast native Rust executable that implements the **Model Context Protocol (MCP)** specification over standard input/output (`stdio` JSON-RPC 2.0).

It connects any frontier AI agent (**Antigravity**, **Claude Desktop**, **Cursor**, **Windsurf**, or **GitHub Copilot CLI**) directly to the running AgentTrace Gateway (`agent_trace_gw.exe`).

---

## 🎯 Architectural Philosophy: Why Standalone Rust MCP?

1. **Zero SQLite Lock Contention**: The 24/7 background Gateway daemon (`agent_trace_gw.exe`) coordinates database commits, signature verification, and WebSocket streaming. The MCP binary acts as an asynchronous bridge over local HTTP (`localhost:8080`), eliminating file lock contention on Windows.
2. **Sub-Millisecond Execution**: Instant process startup with zero Python interpreter overhead or virtual environment (`venv`) activation.
3. **Automatic Studio Chronicle Population**: When your agent calls `record_project_decision` or dispatches tasks via the MCP tools, the Studio UI (`http://localhost:8080/client`) updates live via WebSockets. The human developer gains an autonomous **Project Chronicle & Living Memory Bank** without typing a prompt in the web browser.

---

## 🛠️ How to Configure in AI Coding Clients

### 1. Google Antigravity Plugin (Recommended — Zero Setup)
The easiest and most powerful way to use AgentTrace in Antigravity is via the bundled **AgentTrace Plugin** (`plugins/agent-trace`):
- **Includes**: Bundled MCP server configuration, 3 specialized agent skills (`agent-trace-audit`, `agent-trace-cluster`, `agent-trace-project-book`), and autonomous governance rules (`rules/AGENTS.md`).
- **Install**:
  ```powershell
  # Copy into global Antigravity plugins directory:
  Copy-Item -Path "plugins\agent-trace" -Destination "$env:USERPROFILE\.gemini\config\plugins\agent-trace" -Recurse -Force
  ```
- **Manage**: Visit `[agent-trace](customization://agent-trace)` to toggle or view bundled tools.

#### Standalone `mcp_config.json` Configuration:
If you prefer configuring only the raw MCP server in `~/.gemini/config/mcp_config.json` or your project `.agents/`:
```json
{
  "mcpServers": {
    "agent-trace": {
      "command": "C:\\cada\\projects\\agent_trace_gw\\target\\debug\\agent_trace_mcp.exe",
      "args": [],
      "env": {
        "AGENTTRACE_URL": "http://localhost:8080"
      }
    }
  }
}
```

### 2. Claude Desktop (`claude_desktop_config.json`)
```json
{
  "mcpServers": {
    "agent-trace": {
      "command": "C:\\cada\\projects\\agent_trace_gw\\target\\debug\\agent_trace_mcp.exe"
    }
  }
}
```

### 3. Cursor & Windsurf (`mcp.json`)
```json
{
  "mcpServers": {
    "agent-trace": {
      "command": "C:\\cada\\projects\\agent_trace_gw\\target\\debug\\agent_trace_mcp.exe"
    }
  }
}
```

---

## 🧰 Available MCP Tools (29 Native Tools)

### Cluster Expansion & Zero-Walk Node Setup
| Tool Name | Parameters | Purpose |
| :--- | :--- | :--- |
| **`check_worker_connection`** | `worker_ip` (optional) | Scans LAN ports (`:8080` Gateway, `:8642` Hermes CUDA, `:11434` Ollama) across worker nodes. Reports millisecond latency, service status (`ONLINE`, `GATEWAY_MISSING`, `OFFLINE`), and tailored action recommendations. |
| **`get_cluster_bootstrap_command`** | `os_family` (`windows`/`linux`), `lead_url` (optional) | Dynamically returns the exact 1-click bootstrap script (`irm http://.../bootstrap.ps1 \| iex` or `curl ...`) pre-populated with the Lead Node's active LAN IP. |
| **`deploy_cluster_worker`** | `target_ip`, `port` (default 8642), `os_family` | Triggers zero-walk remote deployment to expand the cluster to an edge machine. |
| **`inspect_cluster`** | *none* | Returns comprehensive cluster topology, node health, active inference engines, and VRAM statuses. |

### Architectural Governance & Problem Workgroups
| Tool Name | Parameters | Purpose |
| :--- | :--- | :--- |
| **`record_project_decision`** | `title`, `rationale`, `tags`, `lessons_learned`, `rules_verified`, `diff` | Cryptographically commits an architectural milestone or decision into the SHA-256 ledger and populates the Studio Project Book. |
| **`upsert_workgroup`** | `id`, `name`, `problem_domain`, `description`, `members` | Autonomously defines or updates a Problem Workgroup hierarchy with assigned agent roles. |
| **`attach_workgroup_rule`** | `workgroup_id`, `rule_path` or `inline_markdown`, `title`, `enforce_strict` | Dynamically grounds a workgroup in mandatory architectural rules (`rule-work-first-ui`, `rule-srp-coding`). |
| **`get_workgroup_rules`** | `workgroup_id` (optional) | Retrieves active Problem Workgroups and their attached Markdown governance rules. |

### Token Economics & Local Delegation
| Tool Name | Parameters | Purpose |
| :--- | :--- | :--- |
| **`dispatch_cluster_prompt`** | `prompt`, `workgroup_id`, `mode` (`auto`/`pipeline`/`swarm`), `requested_model` | Offloads heavy reasoning or boilerplate generation to local cluster nodes (`PC-1` / `PC-2`) via the Prompt Hub at zero external token cost. |
| **`delegate_cluster_task`** | `task`, `model_tier`, `timeout_seconds` | Dispatches routine coding tasks directly to the optimal local LLM endpoint. |

### Provenance, Containment & Forensics
| Tool Name | Parameters | Purpose |
| :--- | :--- | :--- |
| **`verify_ledger`** | *none* | Cryptographically verifies the entire SHA-256 hash chain and Ed25519 node signatures across all blocks. |
| **`get_telemetry_handoff`** | `recent_limit` | Single-call telemetry pickup digest for an incoming agent: cluster health, active security violations, and recent code diffs. |
| **`query_telemetry`** | `target_contains`, `limit` | Searches and filters recorded actions (file reads, writes, commands, LLM calls). |
| **`list_containment_zones`** | *none* | Lists all active Safe Zones (`VaultNoAccess`, `GuardedReadOnly`) and Playgrounds. |
| **`confine_agent_to_playground`** | `agent_id`, `playground_path` | Instantly restricts an untrusted agent to `./playground`, mounting production code Read-Only. |
| **`get_security_alerts`** | `status`, `limit` | Queries active containment alerts and security violations. |
| **`acknowledge_alert`** | `event_id` | Clears or acknowledges handled security alerts. |

---

## ⚡ Verification Command

Test the MCP stdio handshake directly from your terminal:

```powershell
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | .\target\debug\agent_trace_mcp.exe
```
