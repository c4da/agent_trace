---
name: local-model-offload
description: Offload drafting work (unit test scaffolds, docstrings, boilerplate, first-pass code review, log summaries) from a paid cloud coding agent to local models (Ollama, LAN GPU workers) through the AgentTrace gateway, and check the results before using them. Use when the user wants to cut cloud token usage, asks for a local or second-opinion review, or has a large batch of repetitive generation work.
license: MIT OR Apache-2.0
compatibility: Requires a running AgentTrace gateway (https://github.com/c4da/agent_trace) with at least one local model (for example Ollama), via the agent-trace MCP server or Python 3 for scripts/agenttrace.py.
metadata:
  author: c4da
  version: "1.0.0"
---

# Local Model Offload

Send drafting work to local models through AgentTrace's `dispatch_cluster_prompt`, then
check what comes back. Local models are cheap drafters, not final reviewers.

If AgentTrace is not installed yet, follow [references/setup.md](references/setup.md).

## Good and bad tasks to offload

| Offload | Keep in the main agent |
| :--- | :--- |
| Unit test scaffolds for a given function | Final correctness and security judgement |
| Docstrings and comments | Architecture decisions |
| Boilerplate (DTOs, serializers, CLI flags) | Dense math, geometry, concurrency or crypto review |
| First-pass review checklist, style nits | Anything the user will merge without reading |
| Summaries of long logs or test output | Edits that touch many files at once |

## 1. Check what is available

Call `check_cluster_health` (or `inspect_cluster`) once per session. Note which nodes and
models are online. If no model is online, tell the user and do the work yourself.

## 2. Build a self-contained prompt

The gateway and its models **cannot read your workspace**. A prompt that only names a file
gets a generic answer.

- Paste the code itself. With the HTTP fallback use `--attach path/to/file`.
- Send only what is needed: the function under test, the types it uses. Drop unrelated code
  and existing tests. Small local models often have 4k–8k token context windows; above that,
  input is silently truncated. Split large files per function.
- State language and domain explicitly, e.g. "Rust, 3D polyhedral mesh boolean, not 2D
  collision". Small models guess the domain wrong without it.
- Ask for structured output: "Return a numbered list of findings. Each finding: line or
  symbol, problem, suggested fix. Say 'no findings' if none."

## 3. Dispatch

```json
{
  "prompt": "<instructions + pasted code>",
  "mode": "pipeline",
  "workgroup_id": "auto"
}
```

- `mode`: `auto` (single best model), `pipeline` (draft, then a second model verifies),
  `swarm` (models in parallel).
- `workgroup_id`: `auto`, or a configured workgroup such as `wg-coding`.
- `requested_model`: optional, only if the user asks for a specific model.

HTTP fallback:

```bash
python scripts/agenttrace.py dispatch_cluster_prompt \
  --args '{"prompt":"Rust. Write unit tests for parse_header covering empty input and bad UTF-8. Return only code.","mode":"auto"}' \
  --attach src/header.rs
```

The response contains `primary_response`, optional `verification_notes`, and `stages[]` with
`node_id`, `selected_model` and `fallback_used` per stage. Expect tens of seconds per stage.

## 4. Check before you use it

- Read every finding against the actual code. Drop findings that cite lines, symbols or
  behaviour that do not exist.
- Compile and run generated tests. Fix or delete tests that do not build.
- An empty answer, an answer about a different language or domain, or "please provide the
  code" means the prompt failed. Fix the prompt (step 2) instead of trusting the output.
- Never present local output as validation of correctness.

## 5. Report honestly

Tell the user which model produced the draft (`selected_model`), whether a fallback model
was used, which findings you kept after checking and which you discarded.

## Troubleshooting

See [references/model-sizing.md](references/model-sizing.md) for HTTP 500 / out-of-memory
errors, timeouts and fallbacks.
