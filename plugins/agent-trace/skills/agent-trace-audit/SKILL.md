---
name: agent-trace-audit
description: Verify the cryptographic SHA-256 ledger, search telemetry events, check containment violations, and audit multi-node agent interactions.
---

# AgentTrace Telemetry & Ledger Audit Skill

Use this skill when verifying the cryptographic integrity of agent actions, querying recorded telemetry, triaging security alerts, or diagnosing agent behavior.

## Key Capabilities

1. **Cryptographic Ledger Verification**:
   - Call the MCP tool `verify_ledger` (or query `GET http://localhost:8080/api/v1/ledger/verify`).
   - Confirms that every event is bound in an unbroken SHA-256 hash chain signed with Ed25519 node keys.
   - Reports `valid: true/false`, block count, and any broken hash link index.

2. **Telemetry Querying & Forensics**:
   - Call MCP tool `query_telemetry` with `target_contains` or `limit`.
   - Filter by action kind: `file_read`, `file_write`, `command_exec`, `llm_prompt`, `mcp_call`.
   - Access the high-density operator console at `http://localhost:8080/console` (<kbd>j</kbd>/<kbd>k</kbd> navigation, <kbd>P</kbd> stream pause, <kbd>F</kbd> manual flag).

3. **Security Alerts & Containment**:
   - Query alerts with `get_security_alerts` (or `http://localhost:8080/api/v1/alerts`).
   - Acknowledge alerts with `acknowledge_alert` (passing `event_id` or `reset_all: true`).
   - Verify that untrusted agents are confined via `confine_agent_to_playground`.
