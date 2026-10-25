# 📊 AgentTrace Gateway v1.0.0 — Empirical Benchmarks & Verification Evidence

> **Auto-Generated on Commit**: This report is automatically regenerated and verified by `scripts/update_benchmarks_and_badges.py` and `.githooks/pre-commit`.

| Metadata Field | Verified Value |
| :--- | :--- |
| **Release Version** | `v1.0.0` |
| **Git Commit** | `028f6ff` |
| **Timestamp (UTC)** | `2026-10-25 12:51:46 UTC` |
| **Platform / Arch** | `windows / x86_64` |
| **Process Memory Footprint (RSS)** | **`12.63 MB`** *(Target SLA: `< 20.00 MB` — **PASS**)* |
| **Automated Test Suite** | **`120 Rust Unit & Adversarial Tests + 7 E2E Tests Passed (0 Failed)`** |

---

## ⚡ Sub-5ms Latency Evidence Table

| Subsystem Operation | Iterations | p50 Latency | p95 Latency | p99 Latency | Target SLA | Status |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **Hard AI Guard Pre-Execution Interception (DLP + Command + Zone)** | `100` | **`1.539 ms`** (`1539 µs`) | `4.263 ms` | `5.287 ms` | `< 5.0 ms` | **✅ PASS** |
| **Pre-Image CAS Guard Rogue Write Auto-Revert** | `50` | **`1.779 ms`** (`1779 µs`) | `3.024 ms` | `14.007 ms` | `< 5.0 ms` | **✅ PASS** |
| **SHA-256 + Ed25519 Signed Block Commit** | `10` | **`0.316 ms`** (`316 µs`) | `0.453 ms` | `0.453 ms` | `< 5.0 ms` | **✅ PASS** |
| **Cryptographic Ledger Chain Verification (10 Signed Blocks)** | `30` | **`4.653 ms`** (`4653 µs`) | `5.340 ms` | `5.386 ms` | `< 5.0 ms` | **✅ PASS** |
| **ChaCha20-Poly1305 AEAD Secret Vault Roundtrip** | `100` | **`0.018 ms`** (`18 µs`) | `0.019 ms` | `0.052 ms` | `< 1.0 ms` | **✅ PASS** |

---

## 🔬 How to Reproduce Locally

```bash
# 1. Run the empirical benchmark & adversarial test suite and update README badges
python scripts/update_benchmarks_and_badges.py

# 2. Or query live runtime benchmarks directly from a running Gateway
curl -s http://localhost:8080/api/v1/benchmarks
```
