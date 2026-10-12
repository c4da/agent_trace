# Manual-First Observability: Why Human Investigation Precedes AI Log Analysis in AgentTrace

## Introduction
The industry is pushing for autonomous agents to audit themselves. In `AgentTrace`, we reject that premise. An agent auditing its own logs creates a circular trust problem: if the agent hallucinates a violation, it reports itself as compliant. If the agent misses a subtle logic error, it remains blind.

This guide outlines the **Manual-First** philosophy. We prioritize raw human verification over probabilistic AI summaries. You must see the diff, verify the hash chain, and test policies interactively before trusting an automated report.

---

## 1. The 'Manual-First' Philosophy

### The Trust Loop Problem
AI log analysis is a black box. When you rely on an agent to tell you "everything is fine," you are outsourcing your security posture to a stochastic model. In high-stakes environments (security, compliance, critical infrastructure), this is unacceptable.

*   **Blind Self-Policing:** An agent auditing its own execution path cannot distinguish between a false positive and a missed threat without external context.
*   **Context Collapse:** AI summaries strip away the "why." A manual operator needs to see *which* line changed, not just that a change occurred.
*   **Deterministic Verification:** Humans verify hashes and diffs deterministically. We need to be able to reproduce the state manually to confirm the system's integrity.

### The Human-in-the-Loop Necessity
In `AgentTrace`, the AI is an assistant, not the authority. The operator must:
1.  **See the exact diff:** Verify line additions/deletions before a git commit.
2.  **Verify the hash chain:** Manually check ledger proofs to ensure no tampering occurred during transmission.
3.  **Test policy rules interactively:** Run "What if" scenarios without triggering actual side effects.

---

## 2. The 5 Core Manual Operator Workflows in AgentTrace

These workflows are designed for the terminal or a lightweight UI, prioritizing speed and direct data access over dashboard abstraction.

### Workflow 1: Manual Live Grep & Compound Querying
**Goal:** Filter logs by agent identity, verdict, and target path without writing complex SQL or Python scripts.

*   **Action:** Use the `agenttrace-cli` to stream logs with compound filters.
*   **Ergonomics:** Supports regex matching on the fly. You can filter for specific agent IDs (`--agent-id=svc-auth`) and verdicts (`--verdict=fail`).
*   **Why Manual?** AI aggregations often group distinct incidents into a single "error rate." You need to see individual events to correlate them with network topology or user actions.

### Workflow 2: Manual In-Flight Diff Inspection
**Goal:** Scrub line additions and deletions before a git commit is finalized.

*   **Action:** Open the `AgentTrace` diff viewer. It highlights changes in real-time as the agent writes to the staging area.
*   **Ergonomics:** Allows you to "scrub" noise (e.g., whitespace changes) before committing. You can approve a commit only after manually verifying the semantic meaning of the change.
*   **Why Manual?** AI diff tools often miss context-sensitive changes (e.g., changing a permission bit vs. adding a line). Human eyes catch intent better than regex.

### Workflow 3: Interactive Policy Testing & Sandboxing
**Goal:** Test "What if an agent touches path X?" without executing the agent.

*   **Action:** Use the UI evaluation form to simulate policy rules against a specific file path or registry key.
*   **Ergonomics:** The system runs a read-only simulation of the `PolicyEngine`. It returns a verdict (Allow/Deny) based on current state, not actual execution.
*   **Why Manual?** Prevents accidental lateral movement during testing. You verify the *rule* before you trust the *agent*.

### Workflow 4: Manual Flagging & Incident Bookmarking
**Goal:** Pin suspicious events during a live session for post-mortem review.

*   **Action:** Click "Pin" on any log entry or diff view. This creates a bookmark in the `AgentTrace` ledger.
*   **Ergonomics:** Bookmarked events are highlighted in red during the final audit report. You can export these pins as a separate incident ticket.
*   **Why Manual?** AI might miss a low-severity anomaly that becomes critical later. Pinning ensures you don't lose context until you have time to investigate.

### Workflow 5: Manual Cryptographic Audit & JSON/CSV Export
**Goal:** Generate one-keystroke ledger proof and export raw evidence for forensics.

*   **Action:** Run `agenttrace audit --export=csv` or `--proof`. This generates a Merkle tree root hash of the current session state.
*   **Ergonomics:** You get a cryptographic signature that proves the logs haven't been altered since generation. Export is raw JSON/CSV, not a pre-processed PDF report.
*   **Why Manual?** AI reports are often formatted for marketing, not forensics. Raw exports allow legal teams to verify integrity independently.

---

## 3. Educational Rust Implementation Patterns

To support these manual workflows without sacrificing performance, `AgentTrace` uses specific Rust patterns. These ensure the UI remains responsive and the audit trail remains secure.

### Pattern A: Zero-Lock Policy Matching
The `PolicyEngine::evaluate` method performs pattern matching on file paths or registry keys without locking writer threads. This prevents blocking the main event loop while scanning large directories.

```rust
use globset::{Glob, GlobMatcher};
use regex::Regex;
use std::sync::Arc;

pub struct PolicyEngine {
    rules: Arc<Vec<(GlobMatcher, Regex, Verdict)>>,
}

impl PolicyEngine {
    /// Evaluates a path against rules without holding a lock on the writer thread.
    pub fn evaluate(&self, path: &str) -> Verdict {
        // Use glob/regex matching in parallel or single-threaded fast path
        for (glob, regex, verdict) in self.rules.iter() {
            if glob.matches(path) && regex.is_match(path) {
                return verdict;
            }
        }
        Verdict::Allow // Default deny or allow based on config
    }
}
```

*   **Why this matters:** By avoiding `Mutex` guards during the evaluation phase, we keep the UI thread free to render diffs and logs. The matching is stateless and fast.

### Pattern B: Backpressure-Resistant Broadcast Streams
We use `tokio::sync::broadcast` to enable pause/resume streams without buffering indefinitely on the server. This allows operators to "pause" a live log stream to inspect a specific event, then resume without losing data.

```rust
use tokio::sync::broadcast;
use std::time::Duration;

pub struct LogStream {
    sender: broadcast::Sender<LogEntry>,
}

impl LogStream {
    pub fn new() -> Self {
        let (tx, rx) = broadcast::channel::<LogEntry>(1024); // Small buffer to prevent memory bloat
        Self { sender: tx }
    }

    /// Sends a log entry. Returns error if channel is full (backpressure).
    pub fn emit(&self, entry: LogEntry) -> Result<(), broadcast::error::SendError<LogEntry>> {
        self.sender.send(entry)
    }
}
```

*   **Why this matters:** In manual workflows, operators often pause a stream to read a specific log line. If the buffer grows indefinitely, memory spikes occur. A small channel forces the sender to handle backpressure, ensuring the system doesn't crash under high load while an operator is investigating.

### Pattern C: Clean Export Methods
For forensic export, we avoid complex serialization libraries that hide data structure. We use `serde_json` directly for JSON and simple CSV iterators for spreadsheets.

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: i64,
    pub agent_id: String,
    pub verdict: Verdict,
    pub path: String,
}

impl AuditEvent {
    /// One-keystroke ledger proof generation.
    pub fn to_proof(&self) -> String {
        // Simplified hash logic for demonstration
        format!("proof:{}{}", self.timestamp, self.path) 
    }
}
```

*   **Why this matters:** The data structure is explicit. No hidden fields or proprietary formats. This ensures that when you export the CSV, every column maps directly to a security control requirement.

---

## Conclusion
`AgentTrace` does not replace human operators; it equips them with better tools. By prioritizing manual verification, diff inspection, and cryptographic proofing, we ensure that AI analysis is a secondary layer of validation, not the primary source of truth. Trust your eyes first, trust the code second.