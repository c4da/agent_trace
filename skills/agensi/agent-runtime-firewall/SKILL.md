---
name: agent-runtime-firewall
description: Pre-flight safety checks for coding agents using the local AgentTrace gateway. Use before running destructive or network shell commands (rm -rf, git push --force, curl | sh, chmod -R, DROP TABLE), before reading or writing paths outside the project or near secrets (.env, .ssh, keys), before sending pasted logs/configs to an external model, and before risky multi-file edits that need a quick rollback.
license: MIT OR Apache-2.0
compatibility: Requires a running AgentTrace gateway (https://github.com/c4da/agent_trace) via the agent-trace MCP server or Python 3 for scripts/agenttrace.py.
metadata:
  author: c4da
  version: "1.0.0"
---

# Agent Runtime Firewall

Check risky actions with the AgentTrace guard **before** you execute them, and snapshot
files before risky edits so you can restore them in one call.

If AgentTrace is not installed yet, follow [references/setup.md](references/setup.md).

## 0. Confirm the guard is available

- MCP: the `intercept_ai_guard` and `protect_or_revert_cas_file` tools are listed.
- Otherwise run `python scripts/agenttrace.py ping`.

If neither works, tell the user the firewall is offline. Do **not** run a destructive
command without explicit user confirmation while the guard is unavailable.

## 1. Pre-flight a shell command

Run the check when a command deletes, moves, overwrites, changes permissions, rewrites
git history, pipes downloads into a shell, or sends data off the machine.

Call `intercept_ai_guard`:

```json
{ "action": "Execute", "command": "<exact command you intend to run>" }
```

HTTP fallback:

```bash
python scripts/agenttrace.py intercept_ai_guard --args '{"action":"Execute","command":"git push --force origin main"}'
```

Decide with [references/guard-result.md](references/guard-result.md):

| Result | What you do |
| :--- | :--- |
| `hard_blocked: true` or `verdict: "Blocked"` | Do not run it. Show the user `reasons`, propose a safer alternative. Only the user can override. |
| `verdict: "Flagged"`, `risk_score >= 50` or non-empty `findings` | Explain the findings and ask the user before running. |
| `verdict: "Allowed"`, `risk_score < 50`, no findings | Run the command. |

Never rephrase, split or obfuscate a blocked command to get past the guard.

## 2. Pre-flight a file path

Before reading or writing outside the project root, or anything that looks like
credentials, call `intercept_ai_guard` with `"action": "Read"` or `"action": "Write"` and
`"target_path": "<path>"`. Apply the same decision table.

## 3. Scrub text before it leaves the machine

Before pasting logs, configs, stack traces or env dumps into a web search, issue tracker or
another model, call:

```json
{ "action": "LlmInference", "prompt_text": "<text>", "mode": "RedactAndForward" }
```

Send `sanitized_prompt` instead of the original text. Tell the user if anything was redacted.

## 4. Snapshot files before a risky edit

Use this for multi-file refactors, generated rewrites or config migrations.

1. For each existing file you will change, call `protect_or_revert_cas_file`:
   `{ "operation": "protect", "path": "<absolute path>", "strictly_guarded": true }`
2. Make the edits and run the project's build and tests.
3. **Failure** → for each file call `{ "operation": "enforce", "path": "<path>" }`.
   This restores the snapshot. Delete any new files you created yourself.
4. **Success** → call `protect` again on each file to accept the new content as the baseline,
   or leave the snapshots until the user is satisfied.

Limits:
- `strictly_guarded: true` is required for restore. With `false`, `enforce` accepts the change.
- Snapshots live in gateway memory and are lost when the gateway restarts.
- New files cannot be snapshotted. This complements git; it does not replace commits.
- `{ "operation": "list" }` shows what is currently protected.

## 5. Report

After a guarded task, tell the user briefly which commands were checked, anything blocked
or redacted, and whether any files were restored.
