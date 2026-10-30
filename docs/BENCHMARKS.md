# 📊 AgentTrace Gateway v1.1.0 — Empirical Benchmarks & Verification Evidence

> **Auto-Generated on Commit**: This report is automatically regenerated and verified by `scripts/update_benchmarks_and_badges.py` and `.githooks/pre-commit`.

| Metadata Field | Verified Value |
| :--- | :--- |
| **Benchmark Version Tag** | **`v1.1.0`** |
| **Release Version** | `v1.1.0` |
| **Git Commit** | `201831a` |
| **Timestamp (UTC)** | `2026-10-30 01:36:35 UTC` |
| **Platform / Arch** | `windows / x86_64` |
| **Process Memory Footprint (RSS)** | **`16.14 MB`** *(Target SLA: `< 20.00 MB` — **PASS**)* |
| **Automated Test Suite** | **`133 Rust Unit & Adversarial Tests + 7 E2E Tests Passed (0 Failed)`** |

---

## ⚡ Sub-5ms Latency Evidence Table (`v1.1.0`)

| Version Tag | Subsystem Operation | Iterations | p50 Latency | p95 Latency | p99 Latency | Target SLA | Status |
| :---: | :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| `v1.1.0` | **Hard AI Guard Pre-Execution Interception (DLP + Command + Zone)** | `100` | **`1.332 ms`** (`1332 µs`) | `6.057 ms` | `6.590 ms` | `< 5.0 ms` | **✅ PASS** |
| `v1.1.0` | **Pre-Image CAS Guard Rogue Write Auto-Revert** | `50` | **`1.091 ms`** (`1091 µs`) | `2.088 ms` | `2.320 ms` | `< 5.0 ms` | **✅ PASS** |
| `v1.1.0` | **SHA-256 + Ed25519 Signed Block Commit** | `10` | **`0.171 ms`** (`171 µs`) | `0.250 ms` | `0.250 ms` | `< 5.0 ms` | **✅ PASS** |
| `v1.1.0` | **Cryptographic Ledger Chain Verification (10 Signed Blocks)** | `30` | **`2.542 ms`** (`2542 µs`) | `2.710 ms` | `2.752 ms` | `< 5.0 ms` | **✅ PASS** |
| `v1.1.0` | **ChaCha20-Poly1305 AEAD Secret Vault Roundtrip** | `100` | **`0.010 ms`** (`10 µs`) | `0.012 ms` | `0.044 ms` | `< 1.0 ms` | **✅ PASS** |
| `v1.1.0` | **Auto-Skill Forge Trigger Match & In-Flight Injection** | `100` | **`0.009 ms`** (`9 µs`) | `0.009 ms` | `0.033 ms` | `< 1.0 ms` | **✅ PASS** |
| `v1.1.0` | **Unified Persistence SQLite + SHA-256 Integrity Roundtrip** | `30` | **`0.021 ms`** (`21 µs`) | `0.064 ms` | `0.143 ms` | `< 5.0 ms` | **✅ PASS** |
| `v1.1.0` | **OS-Kernel Warden Self-Guard & Write-Boundary Check (0 KB RAM)** | `100` | **`0.002 ms`** (`2 µs`) | `0.002 ms` | `0.009 ms` | `< 1.0 ms` | **✅ PASS** |

---

## 🧠 Auto-Skill Forge & De Facto Learning Token-Savings Benchmark (`v1.1.0`)

| Version Tag | Metric / Scenario | Cold Multi-Turn Rediscovery (No Skill) | With Auto-Skill Forge (Trigger-Gated) | Verified Savings / Outcome |
| :---: | :--- | :---: | :---: | :---: |
| `v1.1.0` | **Recurring Build / Guardrail Recovery Task** | `~15,300 tokens` *(4–6 turns + failed tool outputs)* | **`~120 tokens`** *(1-turn compact skill injection)* | **`99.20%` Token Reduction (`-15,180 tokens/task`)** |
| `v1.1.0` | **Unrelated Prompt Context Overhead** | `0 tokens` | **`0 tokens`** *(keyword/glob trigger-gated)* | **Zero Context Bloat (`0` wasted tokens)** |
| `v1.1.0` | **Cumulative Cluster Tokens Saved (Active Skills)** | Baseline | **`1,916,657 tokens saved`** | **Direct API Cost & Latency Reduction** |

---

## 🔬 How to Reproduce Locally

```bash
# 1. Run the empirical benchmark & adversarial test suite and update README badges
python scripts/update_benchmarks_and_badges.py

# 2. Or query live runtime benchmarks & token-savings ROI directly from a running Gateway
curl -s http://localhost:8080/api/v1/benchmarks
curl -s http://localhost:8080/api/v1/skills/roi-benchmark
```
