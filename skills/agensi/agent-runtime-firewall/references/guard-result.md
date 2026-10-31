# `intercept_ai_guard` result reference

The tool returns a JSON object (inside the MCP text content, or under `result` in the HTTP
fallback):

| Field | Type | Meaning |
| :--- | :--- | :--- |
| `verdict` | string | `Allowed`, `Flagged` or `Blocked`. |
| `enforcement_mode` | string | `HardBlock` (default), `RedactAndForward` or `AuditOnly`. |
| `hard_blocked` | bool | `true` means the action must not run. |
| `risk_score` | 0–100 | 0 is clean, 100 is a critical breach attempt. |
| `sanitized_prompt` | string or null | Scrubbed text when `prompt_text` was supplied. |
| `findings` | array | Individual detections (secret type, destructive pattern, zone violation, injection). |
| `reasons` | array of strings | Human-readable explanations. Show these to the user. |
| `evaluation_latency_us` | int | Guard evaluation time in microseconds. |

## What the guard looks for

- Leaked credentials in text: Anthropic, OpenAI, AWS, GCP and GitHub keys, PEM private keys,
  `.env`-style assignments.
- Destructive or exfiltrating shell commands.
- Prompt-injection directives in text.
- Paths outside configured safe zones / playgrounds, and secret vault paths.

## Modes

- `HardBlock`: block on detection. Use for commands and paths.
- `RedactAndForward`: return a scrubbed copy of the text. Use for outbound text.
- `AuditOnly`: record only. Use when the user explicitly wants observation without blocking.

The guard can miss things and can flag false positives. Treat a clean result as "no known
risk found", not as proof that an action is safe.
