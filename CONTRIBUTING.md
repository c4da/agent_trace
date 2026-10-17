# 🤝 Contributing to AgentTrace Gateway

Thank you for your interest in contributing to **AgentTrace Gateway**! We welcome bug reports, documentation improvements, new Problem Workgroup presets, and core performance optimizations.

---

## 1. Core Architectural Guidelines & Invariants

To maintain long-term code quality, all contributions must adhere to our engineering standards:

### A. Anti-Monolith & Feature-per-File Standard (Strict Rule)
- **Hard Module Size Ceiling**: No single `.rs` file may exceed **800 lines**. Proactively decompose large files into single-responsibility submodules.
- **Top-Level `mod.rs` as Coordinator**: Top-level `mod.rs` files act strictly as API gateways (re-exporting public types, declaring submodules, routing dispatch), never hoarding raw implementation logic.
- **Concise Functions**: Functions should remain short ($\le 3$ arguments where practical; method names add context).

### B. Pure Math & Logic Separation
- Core algorithms and state machines must remain decoupled from UI or GPU asset frameworks. Keep dependencies lightweight and portable (`serde`, `tokio`, `axum`, `rusqlite`, `ring`).

### C. Bulletproof Invariant-Driven Tests
- Every bug fix or feature must include deterministic unit tests (`cargo test`).
- Security rules must include tests proving path traversal defense and SSRF origin validation.

---

## 2. Local Development & Verification

### Prerequisites
- **Rust 1.78+**: [Install Rust](https://rustup.rs/)
- **Cargo**: Standard Rust build tool
- **Python 3.10+**: (Optional) For running SDK client and simulation scripts

### Build & Test Commands
```bash
# Clone the repository
git clone https://github.com/c4da/agent_trace.git
cd agent_trace

# Run all unit and integration tests (87+ tests)
cargo test

# Build debug binary
cargo build

# Run gateway locally (:8080)
cargo run --release
```

Once running, access the high-density operator UI:
- **Terminal Console**: `http://localhost:8080/console`
- **Visual Dashboard**: `http://localhost:8080/dashboard`
- **Workgroups & Rules**: `http://localhost:8080/workgroups`
- **Contribution Graph**: `http://localhost:8080/graph`

---

## 3. Pull Request Submission Checklist

Before submitting a PR, verify:

- [ ] `cargo test` passes cleanly with **zero failures**.
- [ ] `cargo check` compiles with **zero warnings**.
- [ ] No file exceeds **800 lines** (verified via `Get-Content` or `wc -l`).
- [ ] **Strict Anonymity**: No personal machine paths (`C:\Users\...`, `$HOME/...`), internal private tokens, or hardcoded IP credentials.
- [ ] All new documentation follows the **Line 0 How-To** standard and feature-per-file modularity.
- [ ] Ledger verification succeeds (`curl -s http://localhost:8080/api/v1/ledger/verify`).
