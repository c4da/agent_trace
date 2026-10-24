# From Zero to Systems Engineer: Learning Practical Rust with AgentTrace

**A Beginner-Friendly Guide to Modern Rust, Concurrency, and Distributed Systems**

---

## 🦀 Welcome to Rust!

If you are just starting to learn Rust, welcome! Rust is one of the most admired and powerful programming languages in the world because it provides **C/C++ speed and low-level hardware control** with **guaranteed memory safety and zero data races**—all without a garbage collector.

However, learning Rust from abstract tutorials can feel daunting. This textbook takes a radically practical, project-based approach: **you will learn Rust from the ground up by dissecting and understanding the real code running in the AgentTrace Gateway**.

By the end of this book, you won't just know syntax—you will understand how to build asynchronous web servers, stream real-time data, manage concurrent state, sign data with cryptography, and talk to databases like a seasoned systems engineer.

---

## 📑 Table of Contents & Roadmap

### Module 1: The Foundations
- [Chapter 1: The Rust Philosophy & First Steps](chapter_01_getting_started.md)
  - Why Rust? The memory safety revolution without a garbage collector
  - Installing Rust (`rustup`, `cargo`, `rustc`)
  - Variables, Mutability (`mut`), and Primitive Types (`i32`, `u64`, `f64`, `bool`, `&str`, `String`)
  - Functions, Expressions vs Statements, and Basic Control Flow (`if`/`else`, `loop`, `while`, `for`)
- [Chapter 2: The Heart of Rust: Ownership & Borrowing](chapter_02_ownership_borrowing.md)
  - The Stack vs. The Heap: What actually happens in memory
  - The Three Golden Rules of Ownership
  - Move semantics vs. `Clone` vs. `Copy`
  - References and Borrowing: Shared (`&T`) vs. Mutable (`&mut T`)
  - The Borrow Checker: Why it's your best friend, not your enemy

### Module 2: Data Modeling & Robustness
- [Chapter 3: Structs, Enums & Pattern Matching](chapter_03_structs_enums_matching.md)
  - Custom Data Types: Structs (`TraceEvent`, `NodeHeartbeat`)
  - Enums with Data: Why Rust enums are superior to C-style enums (`EventType`, `SecurityVerdict`)
  - Pattern Matching: `match` and `if let` constructs
  - The Billion-Dollar Mistake Solved: `Option<T>` (`Some` vs `None`) instead of `null`
- [Chapter 4: Traits, Generics & Serde](chapter_04_traits_and_generics.md)
  - Generics: Writing flexible, reusable code with zero runtime cost
  - Traits: Rust's version of interfaces (`Display`, `Clone`, `Default`)
  - Attribute Derives: What `#[derive(Clone, Debug, Serialize, Deserialize)]` actually does under the hood
  - Parsing JSON like a pro with `serde` and `serde_json`
- [Chapter 5: Error Handling: Never Crashing in Production](chapter_05_error_handling_idioms.md)
  - Unrecoverable errors (`panic!`) vs Recoverable errors (`Result<T, E>`)
  - `Ok(T)` and `Err(E)` patterns
  - The Question Mark Operator (`?`): Elegant error propagation
  - Handling `Box<dyn std::error::Error>` and custom error types

### Module 3: Concurrency & Web Services
- [Chapter 6: Asynchronous Programming with Tokio](chapter_06_async_tokio_concurrency.md)
  - Synchronous vs Asynchronous execution: Why threads aren't enough
  - The Tokio async runtime and `#[tokio::main]`
  - Futures and `async` / `await` syntax
  - Shared mutable state in async: `Arc<T>`, `Mutex<T>`, and `RwLock<T>`
  - Event distribution across tasks: `tokio::sync::broadcast` and channels
  - The Actor Pattern & Bounded MPSC Channels (`ledger_writer.rs`): Eliminating SQLite lock contention
- [Chapter 7: Building High-Performance Web Services with Axum](chapter_07_building_web_apis_axum.md)
  - What is Axum? Modern, ergonomic async web framework
  - Routers, Handlers, and HTTP methods (`GET`, `POST`, `DELETE`)
  - Axum Extractors: `State`, `Json`, `Query`, `Path`, and `HeaderMap`
  - Streaming responses: Server-Sent Events (SSE) with `async-stream`
  - Advanced Axum (`ota_dist.rs`): Binary streaming, custom cryptographic headers (`X-Content-SHA256`), and dynamic PowerShell bootstrap templating

### Module 4: Real-World Systems & Architecture
- [Chapter 8: Systems Programming, Cryptography & SQLite](chapter_08_systems_crypto_sqlite.md)
  - Working with files, paths (`PathBuf`), and standard I/O
  - Cryptography with `ring`: Generating Ed25519 keypairs, signing digests, and public key verification
  - Length-prefixed SHA-256 hashing and `hex` encoding
  - Embedded databases with `rusqlite`: Connection pooling, prepared statements, and transactions
  - In-Memory Content-Addressable Storage (CAS) with Thread Safety (`cas_guard.rs`): Sub-5ms atomic rollbacks
- [Chapter 9: Testing, Idiomatic Modularity & Building Your First App](chapter_09_testing_and_architecture.md)
  - Library vs Binary targets (`lib.rs` vs `main.rs`)
  - Writing unit tests with `#[test]` and `#[cfg(test)]`
  - Integration testing with `tempfile`
  - Hands-on tutorial: Writing a standalone mini-telemetry signer from scratch
  - Advanced Verification: Headless Invariant Verification (tamper simulation), Stdio JSON-RPC subprocess testing, and E2E test harnesses
- [Chapter 10: Practical Rust Security Engineering — Optional AEAD Vaults (`ChaCha20-Poly1305`), Constant-Time Auth, mTLS HMACs & SIEM Streaming](chapter_10_enterprise_security_aead_mtls_siem.md)
  - Designing Optional, Zero-Cost Security in Rust (Plaintext by Default, Encrypted on Demand)
  - Authenticated Encryption with Associated Data (`ring::aead::CHACHA20_POLY1305` `ENCv1:` envelopes)
  - Writing a Constant-Time Comparator (`constant_time_eq`) to Prevent Timing Side-Channels
  - Inter-Gateway mTLS Thumbprint Pinning & `ring::hmac::HMAC_SHA256` Anti-Replay Windows
  - Zero-Dependency Elastic Common Schema (ECS 8.11) `/_bulk` NDJSON Formatting for ELK (`siem_exporter.rs`)
