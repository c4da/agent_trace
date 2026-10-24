---
name: agent-trace-project-book
description: Record architectural milestones, design decisions, lessons learned, and rule compliance directly into the living Studio Project Book and SHA-256 ledger.
---

# AgentTrace Living Studio Project Book Skill

Use this skill whenever completing an architectural milestone, refactoring code, solving a complex bug, or documenting engineering decisions so they persist across sessions.

## Recording a Decision via MCP

Call the MCP tool `record_project_decision` with:
- **`title`**: Concise summary of what was accomplished (e.g. `Implement Multi-Node Cluster Gateway Federation`).
- **`rationale`**: Technical reason and architectural design behind the choice.
- **`tags`**: Classification array (e.g. `["#cluster", "#federation", "#zero-token", "#mcp"]`).
- **`lessons_learned`**: Educational takeaways for future runs and human inspection.
- **`rules_verified`**: Governance rules satisfied (e.g. `["rule-work-first-ui", "rule-srp-coding"]`).
- **`diff`**: Optional unified diff or affected file summary.

## Viewing the Project Book

Open the AgentTrace Studio UI:
- `http://localhost:8080/client` $\rightarrow$ Navigate to **Studio Project Book** / **Milestone Log**.
- Real-time updates via WebSockets with verifiable SHA-256 block provenance.
