# AgentTrace Plugin

Antigravity plugin for **AgentTrace Gateway**: an OpenAI-compatible scrubbing proxy with a signed hash-chained ledger and a 17-tool MCP server.

## What This Plugin Bundles

1. **Native Rust MCP Server (`agent-trace`)**: spawns `agent_trace_mcp.exe` over stdio JSON-RPC 2.0. Tools cover ledger verification, telemetry search, alerts, cluster inspection, route overrides, cluster prompt delegation, and project decision logging.
2. **3 Agent Skills**:
   - `agent-trace-audit`: ledger verification, telemetry queries and alert triage.
   - `agent-trace-cluster`: inspect the cluster and delegate work to local models.
   - `agent-trace-project-book`: record decisions into the ledger with `record_project_decision`.
3. **Governance Rules (`rules/AGENTS.md`)**: UI ergonomics, code modularity, and local-model delegation guidance.

## Activation

Start the gateway (`cargo run --bin agent_trace_gw`), install the plugin, and the MCP tools become available to the agent. If the gateway has `AGENTTRACE_TOKEN` set, add it to the `env` block of `mcp_config.json` as `AGENTTRACE_TOKEN`.
