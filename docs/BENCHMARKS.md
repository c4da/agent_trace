# 📊 AgentTrace Gateway v1.0.0 — Empirical Benchmarks & Verification Evidence

> **Auto-Generated on Commit**: This report is automatically regenerated and verified by `scripts/update_benchmarks_and_badges.py` and `.githooks/pre-commit`.

| Metadata Field | Verified Value |
| :--- | :--- |
| **Release Version** | `v1.0.0` |
| **Git Commit** | `77b79ff` |
| **Timestamp (UTC)** | `2026-10-24 09:30:37 UTC` |
| **Platform / Arch** | `windows / x86_64` |
| **Process Memory Footprint (RSS)** | **`12.96 MB`** *(Target SLA: `< 20.00 MB` — **PASS**)* |
| **Automated Test Suite** | **`103 Rust Unit & Adversarial Tests + 7 E2E Tests Passed (0 Failed)`** |

---

## ⚡ Sub-5ms Latency Evidence Table

| Subsystem Operation | Iterations | p50 Latency | p95 Latency | p99 Latency | Target SLA | Status |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **Hard AI Guard Pre-Execution Interception (DLP + Command + Zone)** | `100` | **`1.331 ms`** (`1331 µs`) | `3.243 ms` | `4.244 ms` | `< 5.0 ms` | **✅ PASS** |
| **Pre-Image CAS Guard Rogue Write Auto-Revert** | `50` | **`1.246 ms`** (`1246 µs`) | `1.883 ms` | `2.762 ms` | `< 5.0 ms` | **✅ PASS** |
| **SHA-256 + Ed25519 Signed Block Commit** | `10` | **`0.281 ms`** (`281 µs`) | `0.408 ms` | `0.408 ms` | `< 5.0 ms` | **✅ PASS** |
| **Cryptographic Ledger Chain Verification (10 Signed Blocks)** | `30` | **`4.092 ms`** (`4092 µs`) | `4.767 ms` | `4.838 ms` | `< 5.0 ms` | **✅ PASS** |
| **ChaCha20-Poly1305 AEAD Secret Vault Roundtrip** | `100` | **`0.017 ms`** (`17 µs`) | `0.018 ms` | `0.041 ms` | `< 1.0 ms` | **✅ PASS** |

---

## 🔬 How to Reproduce Locally

```bash
# 1. Run the empirical benchmark & adversarial test suite and update README badges
python scripts/update_benchmarks_and_badges.py

# 2. Or query live runtime benchmarks directly from a running Gateway
curl -s http://localhost:8080/api/v1/benchmarks
```
