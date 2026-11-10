---
name: agent-trace-cluster
description: Inspect the local AI cluster (PC-1 Lead Node, PC-2 Worker Node), explain or override model routing, and delegate prompts to local models through AgentTrace.
---

# AgentTrace Cluster Skill

Use this skill when inspecting cluster nodes, checking which model a prompt would route to, or offloading boilerplate work to local models.

## Endpoints

- **Lead Node (PC-1)**: AgentTrace Gateway `http://localhost:8080`; Ollama `http://127.0.0.1:11434`.
- **Worker Node (PC-2)**: Hermes `http://192.168.1.102:8642`, started manually, so it may be offline. It is registered through `WORKER_UPSTREAMS` and `UPSTREAM_ALLOWLIST` in `.secrets/agenttrace.env`.
- **Models are discovered, not fixed**: call `inspect_cluster` or `GET /v1/models` (each model is tagged with its `node_id`) before choosing one. Use `auto` if unsure.

## MCP tools

- `inspect_cluster`: list known nodes and models.
- `explain_route` / `decide_route`: show which upstream and model a prompt would use, and why.
- `set_route_override`, `list_route_overrides`, `remove_route_override`: manage routing overrides.
- `get_delegation_config` / `set_delegation_config`: read or change delegation settings.
- `dispatch_cluster_prompt`: send a prompt to a local or worker model (secrets are redacted before forwarding).
- `verify_ledger`: confirm the hash chain and signatures are intact.

If the gateway has `AGENTTRACE_TOKEN` set, the MCP bridge needs the same value in its environment.
