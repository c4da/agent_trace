# GitHub Copilot CLI Opinion on Next Focus Areas

/ Search (glob)
  │ "tests/**/*.rs"
  └ 5 files found

/ Search (glob)
  │ "src/**/*.rs"
  └ 17 files found

/ Search (glob)
  │ "*"
  └ 335 files found

● Check repository worktree status (shell)                                                       19s
  │ git status --short
  └ 2 lines…

● Read ARCHITECTURE_AND_MODULARITY_REVIEW.md
  │ docs\ARCHITECTURE_AND_MODULARITY_REVIEW.md
  └ L1:220 (91 lines read)

● Read README.md
  └ L1:180 (153 lines read)

● Read Cargo.toml
  └ L1:180 (29 lines read)

● Read QWEN_OPINION.md
  │ docs\QWEN_OPINION.md
  └ L1:180 (3 lines read)

● Read registry.rs
  │ src\registry.rs
  └ L1:220 (220 lines read)

● Read proxy.rs
  │ src\proxy.rs
  └ L1:220 (219 lines read)

● Read copilot_watcher.rs
  │ src\copilot_watcher.rs
  └ L1:180 (180 lines read)

● Read ledger.rs
  │ src\ledger.rs
  └ L1:220 (219 lines read)

● Read proxy.rs
  │ src\proxy.rs
  └ L220:390 (171 lines read)

● Read db.rs
  │ src\db.rs
  └ L1:240 (239 lines read)

● Read delegation_engine.rs
  │ src\delegation_engine.rs
  └ L1:240 (240 lines read)

● Read secrets.rs
  │ src\secrets.rs
  └ L1:200 (197 lines read)

● Read handlers.rs
  │ src\handlers.rs
  └ L1:210 (210 lines read)

● Read main.rs
  │ src\main.rs
  └ L1:190 (188 lines read)

● Read delegation_engine.rs
  │ src\delegation_engine.rs
  └ L240:520 (281 lines read)

/ Search (grep)
  │ "find_best_candidate|is_healthy|latency_ms" (src\registry.rs)
  └ 145 lines found

I’d prioritize the next work in this order. The highest-value shift is from adding features to making the routing, audit, and security guarantees hold under failure and adversarial input.

1. **Make the request path uniformly zero-trust.**  
   **Problem/opportunity:** The proxy flow accepts caller-supplied routing headers and checks the upstream allowlist, but doesn’t visibly apply the configured gateway authorization check. Policy evaluation is present on telemetry ingestion, not on inference dispatch. The allowlist’s prefix matching also risks accepting a URL that merely begins with an allowed origin. Separately, inference events currently include prompts and completions in ledger details.  
   **Rust design:** Add shared Axum middleware for authentication on proxy and control-plane routes; authorize model, node, and override decisions before dispatch. Parse destinations with `url::Url` and compare exact scheme, host, and port against configured origins—never string prefixes. Make prompt/completion capture an explicit opt-in; redact secrets and enforce event-size limits before persistence.  
   **Impact:** Preserves zero-token routing while preventing accidental cloud egress or unauthorized control changes. Reduces SSRF and sensitive-data exposure. Smaller telemetry records also mean less SQLite I/O and lower memory use.

2. **Put ledger writes behind a bounded, acknowledged writer.**  
   **Problem/opportunity:** SQLite is wrapped in a synchronous `Mutex<Connection>` and written from async request paths. Proxy telemetry is persisted in spawned tasks whose insert errors are discarded, so failures can silently lose audit records; concurrent inserts also serialize on the connection.  
   **Rust design:** Send events through a bounded `tokio::sync::mpsc` channel to a dedicated writer task (or `spawn_blocking` worker). That writer alone assigns the previous hash, signs events, and commits ordered batches transactionally. Return an acknowledgment where the caller requires durable audit; use explicit backpressure and observable failure handling rather than dropping errors.  
   **Impact:** Maintains a single authoritative hash-chain order, makes audit loss detectable, and keeps database work off Tokio request workers. Batching improves ingestion throughput without requiring external services or tokens.

3. **Route on measured capacity, not just model capability.**  
   **Problem/opportunity:** The registry tracks health and latency, but candidate selection primarily ranks by capability and parameter tier. That can send work to a suitable but overloaded or slower node while a faster option is available.  
   **Rust design:** Add per-node/model rolling latency and failure metrics, queue/load signals, timeouts, and circuit-breaker state. Rank healthy candidates by intent fit plus observed service time and availability; retain deterministic fallbacks and expose the reason for each choice in trace metadata. Benchmark these decisions with replayable request fixtures.  
   **Impact:** Directs lightweight verification and routine code tasks to efficient local capacity, reserving larger models for tasks that benefit from them. This strengthens zero-token discipline while improving tail latency and resilience to a node becoming slow or unreachable.

4. **Verify cluster-node identity and freshness at the boundary.**  
   **Problem/opportunity:** Ed25519 signing establishes a useful ledger foundation, but node identity needs an explicit lifecycle and authenticated admission to prevent forged or replayed worker heartbeats/events from influencing the registry or audit trail.  
   **Rust design:** Bind each node ID to a registered public key; verify signed, timestamped heartbeats with a nonce or monotonic sequence before updating registry state. Define key rotation and revocation, reject stale/replayed messages, and make verification failures auditable. Keep the verification logic behind a small Rust trait so it can be exercised with deterministic test identities.  
   **Impact:** Makes cluster membership and health data trustworthy, limits spoofing and replay risks, and gives routing decisions a stronger security basis. Verification is local cryptographic work, with negligible impact compared with inference latency.




Changes    +0 -0
AI Credits 0.87 (1m 9s)
Tokens     ↑ 221.3k (179.1k cached, 42.2k written) • ↓ 3.3k (1.8k reasoning)
Resume     copilot --resume=e2312459-b784-4e5a-bb89-23ee28785059
