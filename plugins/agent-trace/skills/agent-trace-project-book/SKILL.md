---
name: agent-trace-project-book
description: Record architectural milestones, design decisions and lessons learned into the AgentTrace signed ledger.
---

# AgentTrace Project Book Skill

Use this skill whenever completing an architectural milestone, refactoring code, solving a complex bug, or documenting an engineering decision so it persists across sessions.

## Recording a Decision via MCP

Call the MCP tool `record_project_decision` with:
- **`title`**: concise summary of what was accomplished.
- **`rationale`**: the technical reason behind the choice.
- **`tags`**: classification array (e.g. `["#refactor", "#security"]`).
- **`lessons_learned`**: takeaways for future runs and human review.
- **`rules_verified`**: guidelines satisfied, if any.
- **`diff`**: optional unified diff or affected file summary.

## Viewing Decisions

Decisions are events in the signed ledger. Browse them in `http://localhost:8080/console` or fetch them with `query_telemetry`. Run `verify_ledger` to confirm the chain is intact.
