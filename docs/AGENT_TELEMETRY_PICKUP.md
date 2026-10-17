# Agent Telemetry Pickup & Handoff Guide

When a secondary or autonomous AI agent (Hermes, Qwen, DeepSeek, Claude Code, Copilot CLI, or a custom Python/MCP watchdog) connects to an active **AgentTrace Gateway** session, it should not have to scrape HTML or make 10 separate API requests to figure out what happened.

AgentTrace provides **4 zero-friction ways** for another agent to pick up live telemetry, inspect active security alerts, verify the SHA-256 ledger, and check or enforce **Safe Zone vs. Playground** containment rules.

---

## 1. One-Call Telemetry Handoff Digest (`GET /api/v1/agent/handoff`)

Any agent or script can fetch a complete, structured JSON snapshot of the gateway in a single HTTP `GET` request:

```bash
curl -s "http://localhost:8080/api/v1/agent/handoff?limit=25"
```

### What the Handoff Payload Contains
| Key | Description |
| :--- | :--- |
| `gateway_status` | Total logged traces, blocked policy violations, pending human approvals, active alerts count, and total tokens/cost. |
| `ledger_integrity` | Live cryptographic SHA-256 chain verification status (`valid`, `checked_records`). |
| `containment_zones` | Full Safe Zone (`vault_no_access`, `guarded_read_only`) and Danger Zone (`agent_playground`) state, plus `confine_to_playgrounds` and `confined_agents`. |
| `active_alerts` | Unacknowledged high-risk alerts (e.g., Vault access attempts, path traversal, shell escapes). |
| `recent_file_diffs` | The most recent file modifications (`file_edit`, `file_write`) with unified diffs and line change counts. |
| `recent_events` | Compact digest of the last `N` intercepted tool calls, shell commands, and LLM completions. |
| `pickup_quickstart` | Self-documenting endpoint map so an inspecting LLM immediately knows what actions it can take next. |

---

## 2. Zero-Dependency Python SDK & CLI (`sdk/agent_trace_client.py`)

 Located at [`sdk/agent_trace_client.py`](../sdk/agent_trace_client.py), this module uses **100% pure Python standard library** (`urllib.request`, `json`, `argparse`) — no `pip install` required.

### A. CLI Briefing (Ideal for Subagents & Shell-Equipped Agents)
```bash
# Compact markdown briefing for an incoming agent's context window
python sdk/agent_trace_client.py --briefing

# Full JSON handoff digest
python sdk/agent_trace_client.py --handoff --limit 30

# List or reset active security alerts
python sdk/agent_trace_client.py --alerts
python sdk/agent_trace_client.py --reset-alerts

# Inspect Safe Zones & Playgrounds or confine a specific agent
python sdk/agent_trace_client.py --zones
python sdk/agent_trace_client.py --confine untrusted-agent-01
python sdk/agent_trace_client.py --unconfine untrusted-agent-01

# Verify cryptographic SHA-256 hash chain
python sdk/agent_trace_client.py --verify
```

### B. Python SDK Import (For Custom Watchdogs & Orchestrators)
```python
from sdk.agent_trace_client import AgentTraceClient

client = AgentTraceClient("http://localhost:8080")

# 1. Pick up full session state in one call
handoff = client.get_handoff(limit=25)
print("Ledger Valid:", handoff["ledger_integrity"]["valid"])
print("Active Alerts:", len(handoff["active_alerts"]))

# 2. Generate a compact LLM-ready markdown briefing
print(client.format_briefing(limit=15))

# 3. Query filtered events (e.g., only policy blocks)
blocked = client.query_events(verdict="BLOCKED_POLICY", limit=10)

# 4. Enforce Playground confinement on a suspect agent
if len(handoff["active_alerts"]) > 0:
    client.confine_agent(enabled=True, agent_id="rogue-subagent")
```

---

## 3. Native MCP Server & JSON Action API (17 Built-In Agent Tools)

AgentTrace natively speaks both **Model Context Protocol (MCP JSON-RPC 2.0)** and a direct **REST Action API**.

### A. Connect any MCP Client (`Claude Code`, `Hermes`, `Cursor`, etc.)
Point your MCP client to:
- **MCP HTTP Endpoint**: `http://localhost:8080/api/v1/mcp`
- **Tool Discovery (`GET`)**: `http://localhost:8080/api/v1/agent/tools`
- **Direct Tool Execution (`POST`)**: `http://localhost:8080/api/v1/agent/action`

Example direct tool call via `POST /api/v1/agent/action`:
```bash
curl -s -X POST http://localhost:8080/api/v1/agent/action \
  -H "Content-Type: application/json" \
  -d '{"tool": "get_telemetry_handoff", "arguments": {"limit": 15}}'
```

### B. Complete Catalog of 17 Agent Tools
| Category | Tool Name | Purpose |
| :--- | :--- | :--- |
| **Telemetry Handoff** | `get_telemetry_handoff` | **Recommended first call.** Returns status, ledger verification, zones, alerts, recent diffs, and events in one shot. |
| **Observability** | `get_system_status` | High-level gateway metrics, token usage, cost, and alert counters. |
| **Observability** | `query_events` | Filter telemetry by `agent_id`, `event_type`, `verdict`, `target_contains`, or `search`. |
| **Observability** | `get_event_detail` | Inspect full payload, unified diff, cryptographic hashes, and Ed25519 signatures for an event ID. |
| **Security Alerts** | `get_security_alerts` | List all active (unacknowledged) security alerts triggered by policy/zone violations. |
| **Security Alerts** | `acknowledge_alert` | Dismiss a specific alert by `event_id` or pass `"all"` to reset all active alerts after triage. |
| **Safe Zones & Playground** | `list_containment_zones` | List all `vault_no_access`, `guarded_read_only`, and `agent_playground` zones and confinement locks. |
| **Safe Zones & Playground** | `upsert_containment_zone` | Dynamically add or update a Safe Zone or Agent Playground path rule. |
| **Safe Zones & Playground** | `confine_agent_to_playground` | Lock a specific `agent_id` (or all agents) strictly inside `agent_playground` directories. |
| **Policy & Firewall** | `get_policy` | Read active blocked commands, protected paths, and allowed roots. |
| **Policy & Firewall** | `add_blocked_command` | Dynamically block a dangerous shell command substring. |
| **Policy & Firewall** | `add_protected_path` | Add a path substring to the legacy protected paths list. |
| **Human-in-the-Loop** | `list_pending_approvals` | List intercepted high-risk operations awaiting approval. |
| **Human-in-the-Loop** | `resolve_approval` | Approve or reject a pending operation by `approval_id`. |
| **Forensics & Audit** | `verify_ledger` | Verify the cryptographic SHA-256 hash chain across all persisted records. |
| **Forensics & Audit** | `submit_trace_event` | Log a custom audit/trace event (e.g., watchdog findings) into the immutable ledger. |
| **Cluster Inference** | `delegate_cluster_task` | Dispatch a prompt to a local/cluster LLM node through the gateway proxy. |

---

## 4. Copy-Paste System Prompt for an Incoming Agent

Drop this block into the prompt or `AGENTS.md` of any secondary agent that needs to pick up telemetry from AgentTrace:

```markdown
### AgentTrace Telemetry Pickup Protocol
You have live access to the AgentTrace Security & Provenance Gateway at `http://localhost:8080`.
1. **Immediate Situational Awareness**:
   - Run `python sdk/agent_trace_client.py --briefing` OR call `GET http://localhost:8080/api/v1/agent/handoff?limit=25`.
2. **Inspect & Triage Alerts**:
   - Check `active_alerts` in the handoff digest. Inspect any `BLOCKED_POLICY` events via `get_event_detail`.
   - Once triaged, clear resolved alerts with `python sdk/agent_trace_client.py --reset-alerts` or `POST /api/v1/agent/action` (`{"tool":"acknowledge_alert","arguments":{"event_id":"all"}}`).
3. **Enforce Containment (`Safe Zone` vs `Agent Playground`)**:
   - If an agent is behaving suspiciously or running untrusted code, confine it to the Playground (`/workspace/playground`) via:
     `python sdk/agent_trace_client.py --confine <AGENT_ID>`
```
