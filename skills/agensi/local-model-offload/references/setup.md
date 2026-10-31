# AgentTrace setup

This skill drives the open-source AgentTrace gateway (MIT OR Apache-2.0):
https://github.com/c4da/agent_trace

## 1. Build and start the gateway

Requires a Rust toolchain (1.78+).

```bash
git clone https://github.com/c4da/agent_trace.git
cd agent_trace
cargo build --release
./target/release/agent_trace_gw        # Windows: .\target\release\agent_trace_gw.exe
```

The gateway listens on port `8080` (override with `PORT`). Check it is up:

```bash
python scripts/agenttrace.py ping
```

Optional: local models for offloading are reached through Ollama at `http://127.0.0.1:11434`
(override with `UPSTREAM_URL`).

> [!WARNING]
> The gateway binds to all interfaces and authentication is **off** by default.
> On any shared network, enable it before use:
> `AGENTTRACE_REQUIRE_AUTH=true` and `AGENTTRACE_TOKEN=<long random string>`
> (set the same `AGENTTRACE_TOKEN` for the MCP server and `scripts/agenttrace.py`),
> or block port 8080 in your firewall.

## 2. Connect your agent (MCP, recommended)

The MCP server is a stdio bridge to the running gateway:
`target/release/agent_trace_mcp` (`.exe` on Windows). Pass `AGENTTRACE_URL` if the gateway
is not on `http://localhost:8080`, and `AGENTTRACE_TOKEN` if auth is enabled.

| Agent | Command / config |
| :--- | :--- |
| Claude Code | `claude mcp add agent-trace -- /path/to/agent_trace_mcp` |
| GitHub Copilot CLI | `copilot mcp add agent-trace -- /path/to/agent_trace_mcp` |
| Cursor / Windsurf / Claude Desktop | add to `mcpServers` in the client's MCP JSON: `{"agent-trace": {"command": "/path/to/agent_trace_mcp"}}` |

Clients may prefix tool names (for example `agent-trace-intercept_ai_guard` or
`mcp__agent-trace__intercept_ai_guard`). The skill refers to the bare tool name.

## 3. No MCP? Use the HTTP fallback

`scripts/agenttrace.py` is a zero-dependency Python 3 client for the same tools:

```bash
python scripts/agenttrace.py <tool_name> --args '<json parameters>' [--attach FILE ...]
```

Exit codes: `0` ok, `1` tool error, `2` gateway unreachable, `3` guard hard-blocked.
