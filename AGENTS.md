# AgentTrace Repository Guidance

AgentTrace Gateway is a Rust reverse proxy and telemetry service for AI agent operations.

## Development

- Keep changes focused and follow existing Rust and Python conventions.
- Run `cargo fmt --check` and the relevant tests before submitting changes.
- Never commit `.secrets/`, local environment files, database files, or private keys.
- Configure node addresses and credentials locally; do not add machine-specific endpoints or tokens to tracked files.
- Use `WORKER_NODE_URL` and `UPSTREAM_ALLOWLIST` to configure a remote worker.

---

## 4. UI Design Philosophy & Implementation Standard ("Work-First" Blueprint)
Whenever designing, building, or refactoring any UI (Web TUI, Desktop GUI, or Dashboard), strictly follow these 6 principles:
1. **Anti-SaaS-Fluff & High Information Density**: Reject oversized marketing cards and wasted whitespace. Prioritize compact monospace grids, terminal/CRT aesthetics (Ghostty, k9s, htop), and 30–50 visible records at a glance.
2. **Scenario-First Architecture (Scenarios Sell The App)**: Build the layout and filters directly around the top 3–5 concrete questions, fears, and real-world incidents the user faces.
3. **Live Interactive Demonstrations**: Include a `[⚡ Live Scenario Demos]` toolbar allowing users to trigger and test-drive real-world scenarios with instant visual proofs (diffs, alerts, explainer cards).
4. **Manual-First Ergonomics (Human-in-the-Loop Forensics)**: Users want to inspect, test, and verify things manually before trusting automation. Always provide: compound grep filtering, stream pause/freeze (`P`), step-by-step navigation (`j`/`k`), an interactive "What If?" policy sandbox form (`M`), manual event flagging/pinning (`F`), and raw JSON/CSV export.
5. **Keyboard-First Navigation**: Every primary action must have a single-key binding (`j`/`k`, `/`, `:`, `V`, `P`, `D`, `B`, `F`, `M`, `?`).
6. **Dual-Mode & Zero Bloat**: Preserve visual overviews (`/dashboard`) alongside the operator console (`/console`) with a 1-click toggle, using fast, zero-dependency code.
