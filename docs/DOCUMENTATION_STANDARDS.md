# 📚 Documentation & Technical Writing Standards

This document establishes the mandatory standards for all documentation, user guides, README files, and scenario walkthroughs across the AgentTrace ecosystem.

---

## 1. Line 0 How-To Index & Fast-Path Quickstart
- **Actionable Above the Fold**: The root `README.md` must present a comprehensive How-To table and 30-second copy-paste quick recipes directly on Line 0 (immediately below header badges).
- **No Marketing Fluff First**: Users need to install, configure, and run scenarios within 60 seconds without wading through marketing prose. High-level architecture and conceptual overviews must follow after the quick-start table.

---

## 2. Modular Feature-per-File Architecture
- **Single Responsibility for Docs**: Never write monolithic multi-thousand-line documentation files bundling disparate topics.
- **Dedicated How-To Files**: Every distinct operational capability must have its own focused file in `docs/`:
  - Installation & Setup: `docs/HOW_TO_INSTALL_AND_SETUP.md`
  - Scenarios: `docs/HOW_TO_RUN_SCENARIOS.md`
  - Safe Zones & Sandboxing: `docs/HOW_TO_SAFE_ZONES_AND_PLAYGROUNDS.md`
  - Workgroups & Mesh: `docs/HOW_TO_PROBLEM_WORKGROUPS_AND_USE_CASES.md`
  - Central Hub & Graph: `docs/HOW_TO_PROMPT_HUB_AND_GRAPH.md`
  - UI Design Principles: `docs/UI_DESIGN_PHILOSOPHY_AND_PROMPT.md`
  - Code Refactoring & SRP: `docs/CODE_REFACTOR_AND_ENGINEERING_RULES.md`
  - Standards & Governance: `docs/DOCUMENTATION_STANDARDS.md`

---

## 3. Zero Personal Paths & Environment Neutrality
- **Strict Anonymity & Portability**: Documentation must never contain personal system paths (e.g., `C:\cada\...`, `C:\Users\milos\...`), internal private tokens, or hardcoded personal credentials.
- **Relative Linking**: Use repository-relative paths (`docs/...`, `src/...`, `tests/...`) for all internal markdown hyperlinks.

---

## 4. Scenario-Driven Documentation ("Prove It")
Every guide and scenario walkthrough must strictly follow the **Three-Part Verification Pattern**:
1. **The Problem / Risk**: What dangerous action or operational friction does the user face?
2. **The Command / UI Action**: Clear CLI one-liner (`curl`, `python sdk/agent_trace_client.py`, or GUI keybinding).
3. **The Concrete Verification Proof**: Exact JSON output, terminal log, or cryptographic ledger hash proving the interception.

---

## 5. Metric & Test Badge Integrity
- **Honest Test Reporting**: Badges must reflect actual passing test counts (e.g., `87 Passed`).
- When tests or endpoints are added, badges in `README.md` must be updated synchronously.

---

## 6. Workgroup Governance Coverage
All documented problem workgroups must clearly explain:
- **Top-Down Tree Hierarchy**: Top Hub $\rightarrow$ Lead Architect $\rightarrow$ Primary Builder / Sanity Verifier.
- **Attached Governance Rules**: Which `.md` rule file is bound to each workgroup.
- **Trigger Keywords**: How prompts automatically match and route to the correct team of models.
