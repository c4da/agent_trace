# AgentTrace Operational & Governance Guidelines

When the **AgentTrace** plugin is active, all agent operations, cluster interactions, and code modifications must adhere to the following core principles:

---

## 1. Work-First UI Philosophy (Anti-SaaS-Fluff)
- **High Information Density**: Monospace, compact terminal aesthetics (Ghostty, k9s, htop, lazygit). Keep 30–50 records visible without scrolling.
- **Scenario-Driven Layout**: Design panels directly around operator questions, fears, and real-world incidents.
- **Live Demonstrations**: Always provide interactive scenario triggers (`[⚡ Live Scenario Demos]`) showing immediate visual proof (colorized unified diffs, security alerts, explainer cards).
- **Manual-First Operator Forensics**:
  - Compound live filtering (`type:`, `verdict:`, `target:`).
  - Stream freeze/pause (<kbd>P</kbd>) so data doesn't scroll away.
  - Keyboard navigation (<kbd>j</kbd>/<kbd>k</kbd> step-by-step).
  - Manual Flagging (<kbd>F</kbd>).

---

## 2. Anti-Monolith & Code Modularity (SRP Standard)
- **Strict Single Responsibility Principle**: Never generate monolithic 1,000+ line files bundling multiple interactions or data models.
- **Feature-per-File Architecture**: Every distinct modifier, handler, or tool must reside in its own cohesive file. Top-level `mod.rs` files act strictly as coordinators and clean API gateways.
- **Pure Domain Separation**: Separate pure algorithms/state transitions from heavy I/O, UI frameworks, or network stacks.

---

## 3. Autonomous Local Agent Ecosystem & Token Economy Directive
- **Save External API Tokens**: Delegate boilerplate generation, unit test drafting, docstring creation, and sanity audits to local cluster nodes:
  - Discover what is online first: MCP `inspect_cluster`, or `GET http://localhost:8080/v1/models` (each model is tagged with its `node_id`). Don't assume model names.
  - Workers that are started manually (such as PC-2) may be offline. Fall back to `LOCAL` models, or use `auto`.
- **Cryptographic Provenance**: Every major milestone, architectural decision, and rule verification must be logged to the SHA-256 ledger using `record_project_decision` or through the AgentTrace Gateway.
- **Secrets**: Never paste credentials into prompts; the gateway redacts known patterns, but treat that as a backstop.
