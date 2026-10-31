---
name: agent-decision-ledger
description: Cryptographically commit major architectural decisions, bug root-causes, refactoring rationales, and verified invariant rules into a tamper-evident SHA-256 ledger and live Studio Project Book. Use whenever completing a milestone, refactoring an architecture, solving an evasive bug, or auditing historical agent sessions.
license: MIT OR Apache-2.0
compatibility: Requires a running AgentTrace gateway (https://github.com/c4da/agent_trace) via the agent-trace MCP server or Python 3 for scripts/agenttrace.py.
metadata:
  author: c4da
  version: "1.0.0"
---

# Agent Decision Ledger & Living Project Book

AI agents often suffer from "session amnesia": each new session starts from scratch and re-solves
the exact same problems or undoes prior architectural decisions.

This skill equips coding agents to record architectural decisions into AgentTrace's
tamper-evident SHA-256 + Ed25519 ledger, giving future sessions a cryptographically verifiable
memory bank and human operators a live living project chronicle.

If AgentTrace is not installed yet, follow [references/setup.md](references/setup.md).

## When to Record a Decision

Record an entry whenever you:
1. **Complete a major architectural milestone** (e.g. new crate, new engine subsystem, protocol bridge).
2. **Resolve an evasive bug or regression** (documenting root-cause and permanent fix).
3. **Refactor code for architectural invariants** (e.g. SRP separation, mathematical invariants like Euler-Poincaré $\chi = 2$).
4. **Enforce or verify project governance rules** (e.g. zero external tokens, single responsibility principle).

Do **not** record trivial edits (fixing typos, formatting, simple variable renames).

## 1. Commit a Decision via MCP

Call `record_project_decision`:

```json
{
  "title": "Migrate Polyhedral Mesh Boolean Kernel to Pure B-Rep Half-Spaces",
  "rationale": "Previous mesh boolean relied on unstructured triangle clipping which produced non-manifold T-junctions. Decomposed algorithm into convex half-space clipping in strux_math to guarantee 2-manifold closed solids.",
  "tags": ["#architecture", "#geometry", "#b-rep", "#invariants"],
  "lessons_learned": "Linear vertex deduplication becomes an O(N^2) bottleneck for complex facets; required spatial hashing with tolerance epsilon.",
  "rules_verified": ["rule-srp-coding", "rule-pure-math-invariant"],
  "diff": "crates/strux_math/src/cad/convex_intersection.rs (+484 lines)"
}
```

HTTP fallback:

```bash
python scripts/agenttrace.py record_project_decision --args '{
  "title": "Migrate Polyhedral Mesh Boolean Kernel to Pure B-Rep Half-Spaces",
  "rationale": "Decomposed algorithm into convex half-space clipping to guarantee 2-manifold closed solids.",
  "tags": ["#architecture", "#geometry"],
  "lessons_learned": "Linear deduplication is O(N^2); spatial hashing required."
}'
```

## 2. Verify Ledger Integrity

Before or after major milestone deliveries, verify that the cryptographic hash chain
and Ed25519 signature provenance remain unbroken:

Call `verify_ledger` (or HTTP fallback: `python scripts/agenttrace.py verify_ledger`).

Result structure:
- `is_valid`: `true` when every block's SHA-256 prev_hash matches and signatures verify.
- `total_blocks`: total committed event count.
- `broken_block_id`: `null` if healthy; block ID if tampering occurred.

## 3. Query Past Telemetry & Project Memory

To inspect what previous agent sessions or human operators did:

Call `query_telemetry`:
```json
{
  "target_contains": "convex_intersection",
  "limit": 20
}
```

This returns chronological actions (file writes, LLM inferences, decisions) touching that target.

## 4. Human Visual Inspection

Direct the human user to their local browser:
- **Studio Living Project Book**: `http://localhost:8080/client`
- **Real-Time Forensic Console**: `http://localhost:8080/console`
- **System Dashboard**: `http://localhost:8080/dashboard`
