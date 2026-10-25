# 🔌 AgentTrace Native Rust MCP Server Guide (35 Tools)

> **"Here's a tool — let your agent read it, he will thank you later."**

[`agent_trace_mcp`](../src/bin/agent_trace_mcp.rs) is a standalone, zero-dependency native Rust executable that implements the **Model Context Protocol (MCP)** specification over standard input/output (`stdio` JSON-RPC 2.0) as well as over HTTP (`POST /api/v1/mcp` and `POST /api/v1/agent/action`).

It connects any AI coding agent (**Antigravity**, **Claude Code (`claude`)**, **Claude Desktop**, **Cursor**, **Windsurf**, or **GitHub Copilot CLI**) directly to the running AgentTrace Gateway (`agent_trace_gw.exe`).

---

## 🎯 Architectural Philosophy: Why Standalone Rust MCP?

1. **Zero SQLite Lock Contention**: The 24/7 background Gateway daemon (`agent_trace_gw.exe`) coordinates database commits, Ed25519 signature verification, and SSE streaming. The MCP binary acts as an asynchronous bridge over local HTTP (`localhost:8080`), eliminating file lock contention on Windows.
2. **Sub-Millisecond Execution**: Instant process startup with zero Python interpreter overhead or virtual environment (`venv`) activation.
3. **Automatic Studio Chronicle Population**: When your agent calls `record_project_decision` or dispatches tasks via the MCP tools, the Studio UI (`http://localhost:8080/client`) updates live. The human developer gains an autonomous **Project Chronicle & Living Memory Bank** without typing a prompt in the web browser.

---

## 🛠️ How to Configure in AI Coding Clients

### 1. Google Antigravity Plugin (Recommended — Zero Setup)
The easiest way to use AgentTrace in Antigravity is via the bundled **AgentTrace Plugin** (`plugins/agent-trace`):
- **Includes**: Bundled MCP server configuration, 3 specialized agent skills (`agent-trace-audit`, `agent-trace-cluster`, `agent-trace-project-book`), and autonomous governance rules (`rules/AGENTS.md`).
- **Install**:
  ```powershell
  Copy-Item -Path "plugins\agent-trace" -Destination "$env:USERPROFILE\.gemini\config\plugins\agent-trace" -Recurse -Force
  ```

#### Standalone `mcp_config.json` Configuration:
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

### 2. Claude Code CLI & Claude Desktop (`claude_desktop_config.json`)
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

## 🧰 Available MCP Tools (35 Native Tools)

### 1. Active AI Guard, Pre-Image CAS Undo, Admin Governance, Vault & SIEM (5 Flagship Security Tools)
| Tool Name | Parameters | Purpose |
| :--- | :--- | :--- |
| **`protect_or_revert_cas_file`** | `operation` (`protect`/`enforce`/`status`), `path`, `strictly_guarded` | Takes an in-memory SHA-256 pre-image snapshot of a file before a risky multi-file edit (`protect`), inspects/restores corrupted or deleted files in `< 1.5 ms` (`enforce`), or lists all guarded files (`status`). |
| **`manage_admin_model_governance`** | `operation` (`list`/`upsert_model`/`evaluate`), `model`, `command`, `target_path`, `profile` | Admin Governance Plane tool to inspect global & per-model restrictions (`claude-*`, `gpt-*`, `qwen3.8:27b`, `deepseek-r1:1.5b`), upsert a `ModelRestrictionProfile`, or dry-run per-model enforcement. |
| **`intercept_ai_guard`** | `action`, `prompt_text`, `command`, `target_path`, `mode` (`HardBlock`/`RedactAndForward`/`AuditOnly`) | Synchronous `< 1.5 ms` pre-flight check scanning prompts and commands for leaked `.env`/API/PEM secrets, prompt injections, destructive commands, and spatial zone violations. |
| **`encrypt_vault_secret`** | `plaintext` | Encrypts a plaintext API key or secret into a `ChaCha20-Poly1305` AEAD envelope (`ENCv1:<nonce>:<ciphertext>`) for `.secrets/agenttrace.env`. |
| **`export_siem_elk`** | `limit`, `format` (`bulk`/`stream`) | Exports signed telemetry and AI Guard security alerts formatted in Elastic Common Schema (`ECS 8.11.0`) NDJSON for Elasticsearch `/_bulk` or Splunk/Filebeat ingestion. |

### 2. Cluster Expansion, Health Watchdog & Zero-Walk Node Setup
| Tool Name | Parameters | Purpose |
| :--- | :--- | :--- |
| **`check_cluster_health`** | `force_probe` (optional) | Returns real-time health of all registered cluster nodes (`PC-1`, `PC-2`), detecting Windows Modern Standby / Sleep mode and offline services. |
| **`check_worker_connection`** | `worker_ip` (optional) | Scans LAN ports (`:8080` Gateway, `:8642` Hermes CUDA, `:11434` Ollama) across worker nodes and reports millisecond latency. |
| **`get_cluster_bootstrap_command`** | `os_family` (`windows`/`linux`), `lead_url` (optional) | Dynamically returns the 1-liner OTA bootstrap command (`irm http://<LEAD_IP>:8080/bootstrap.ps1 \| iex`) pre-populated with the Lead Node's LAN IP. |
| **`deploy_cluster_worker`** | `target_ip`, `port`, `os_family` | Triggers zero-walk remote deployment to expand the cluster to an edge machine. |
| **`inspect_cluster`** | *none* | Returns comprehensive cluster topology, node health, active inference engines, and model registry capabilities. |

### 3. Architectural Governance & Problem Workgroups
| Tool Name | Parameters | Purpose |
| :--- | :--- | :--- |
| **`record_project_decision`** | `title`, `rationale`, `tags`, `lessons_learned`, `rules_verified`, `diff` | Cryptographically commits an architectural milestone or decision into the SHA-256 ledger and populates the Studio Project Book (`/client`). |
| **`upsert_workgroup`** | `id`, `name`, `problem_domain`, `description`, `members` | Defines or updates a Problem Workgroup hierarchy with assigned agent roles. |
| **`attach_workgroup_rule`** | `workgroup_id`, `rule_path` or `inline_markdown`, `title`, `enforce_strict` | Grounds a workgroup in mandatory Markdown architectural rules (`rule-work-first-ui`, `rule-srp-coding`). |
| **`get_workgroup_rules`** | `workgroup_id` (optional) | Retrieves active Problem Workgroups and their attached `.md` governance rules. |

### 4. Token Economics, Routing & Local Delegation
| Tool Name | Parameters | Purpose |
| :--- | :--- | :--- |
| **`dispatch_cluster_prompt`** | `prompt`, `workgroup_id`, `mode` (`auto`/`pipeline`/`swarm`), `requested_model` | Offloads reasoning, unit test drafting, or boilerplate generation to local cluster nodes (`PC-1` / `PC-2`) at `$0` external token cost. |
| **`explain_route`** / **`decide_route`** | `prompt`, `requested_model`, `agent_id`, `session_id` | Explains or executes dynamic capability routing across local and cloud models. |
| **`set_route_override`** / **`list_route_overrides`** / **`remove_route_override`** | `rule_id`, `pattern`, `target_model`, `target_upstream` | Manages dynamic session, agent, and prompt-pattern routing overrides. |
| **`get_delegation_config`** / **`set_delegation_config`** | `intercept_cloud_models`, `fallback_to_local` | Views or updates autonomous cloud-to-local delegation settings. |

### 5. Provenance, Spatial Containment & Forensics
| Tool Name | Parameters | Purpose |
| :--- | :--- | :--- |
| **`verify_ledger`** | *none* | Cryptographically verifies the entire SHA-256 hash chain and Ed25519 node signatures across all blocks. |
| **`get_telemetry_handoff`** | `recent_limit` | Single-call telemetry pickup digest for an incoming agent: cluster health, active security violations, and recent code diffs. |
| **`query_telemetry`** / **`analyze_telemetry`** | `target_contains`, `limit` | Searches recorded actions and computes blast-radius / read-to-write threat scores. |
| **`flag_incident`** | `target`, `reason`, `severity` | Manually or autonomously flags a suspicious event into the security alert queue. |
| **`get_access_policies`** / **`set_access_policy`** / **`evaluate_access_policy`** | `agent_id`, `action`, `target` | Inspects, updates, or dry-runs agent sandbox access policies. |
| **`list_containment_zones`** / **`upsert_containment_zone`** | `zone` | Lists or updates Safe Zones (`VaultNoAccess`, `GuardedReadOnly`, `AgentPlayground`). |
| **`confine_agent_to_playground`** | `agent_id`, `confine` | Restricts an untrusted agent strictly to `./playground`, mounting production code Read-Only. |
| **`get_security_alerts`** / **`acknowledge_alert`** | `include_acked`, `event_id` | Queries or acknowledges security alerts. |

---

## ⚡ Verification Commands

Test the MCP tool registry over HTTP or `stdio` JSON-RPC 2.0 directly from your terminal:

```powershell
# 1. Inspect all 35 registered tools via HTTP:
curl -s http://localhost:8080/api/v1/agent/tools

# 2. Test the standalone MCP stdio binary:
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | .\target\debug\agent_trace_mcp.exe
```
