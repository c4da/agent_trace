# AgentTrace MCP Server Guide (17 tools)

[`agent_trace_mcp`](../src/bin/agent_trace_mcp.rs) is a standalone Rust executable that speaks the Model Context Protocol over stdio (JSON-RPC 2.0). It is a thin bridge to a running AgentTrace Gateway: the gateway owns the database and signing key, the bridge forwards tool calls over local HTTP.

The same tools are also reachable over HTTP: `POST /api/v1/mcp` (JSON-RPC) and `POST /api/v1/agent/action` (`{agent_id, tool, parameters}`), with the list at `GET /api/v1/agent/tools`.

## Configuration

Environment variables for the bridge:

- `AGENTTRACE_URL` (default `http://localhost:8080`)
- `AGENTTRACE_TOKEN` (required if the gateway has a token set)

```json
{
  "mcpServers": {
    "agent-trace": {
      "command": "C:\\path\\to\\agent_trace_gw\\target\\debug\\agent_trace_mcp.exe",
      "args": [],
      "env": {
        "AGENTTRACE_URL": "http://localhost:8080",
        "AGENTTRACE_TOKEN": "only-if-the-gateway-has-one"
      }
    }
  }
}
```

The same block works for Claude Code / Claude Desktop, Cursor, Windsurf and Antigravity (the Antigravity plugin in `plugins/agent-trace` bundles it).

## Tools

| Tool | Purpose |
| --- | --- |
| `record_project_decision` | Record a decision (`title`, `rationale`, `tags`, `lessons_learned`, `rules_verified`, `diff`) as a signed ledger event |
| `dispatch_cluster_prompt` | Send a prompt (`prompt`, optional `mode`, `requested_model`) to a local or worker model; secrets are redacted first |
| `verify_ledger` | Verify the SHA-256 hash chain and Ed25519 signatures |
| `query_telemetry` | Search recorded events (`target_contains`, `limit`) |
| `analyze_telemetry` | Summary statistics over recorded events |
| `inspect_cluster` | Known nodes, upstreams and models |
| `flag_incident` | Add a suspicious event to the alert queue |
| `get_security_alerts` / `acknowledge_alert` | List alerts (`include_acked`); acknowledge by `event_id` or `reset_all` |
| `explain_route` / `decide_route` | Show or execute the routing decision for a prompt |
| `set_route_override` / `list_route_overrides` / `remove_route_override` | Manage routing overrides |
| `get_delegation_config` / `set_delegation_config` | Read or change cloud-to-local delegation settings |
| `get_telemetry_handoff` | One-call digest for an incoming agent: summary, ledger status, alerts, recent events |

## Verify

```sh
curl -s http://localhost:8080/api/v1/agent/tools
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | ./target/debug/agent_trace_mcp
```
