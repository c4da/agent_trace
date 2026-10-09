import urllib.request
import json
import sys
import os
import time

sys.stdout.reconfigure(encoding='utf-8')

GATEWAY_URL = "http://localhost:8080/v1/chat/completions"
MODEL = "gemma4:latest"

prompt = """You are a Principal Rust Architect and Senior Code Reviewer.
Perform an architectural, modularity, and test coverage review of the **AgentTrace Gateway** codebase.

### System Architecture Overview:
- `src/lib.rs`: Exports core modules.
- `src/main.rs`: Entry point, sets up Axum router, SQLite WAL database, background services, watchers.
- `src/handlers.rs`: Axum HTTP & SSE handlers, static assets, event ingestion, policy and delegation routes.
- `src/proxy.rs`: OpenAI-compatible reverse proxy (`/v1/chat/completions`), dynamic routing, upstream allowlist enforcement.
- `src/delegation_engine.rs`: Dynamic routing override rules (Session, Agent, Tag, Model, PromptPattern), intent arbitration.
- `src/registry.rs`: Cluster model registry, node heartbeats, capability discovery, offline failover.
- `src/router.rs`: Prompt intent classification (Math, Fast Code, Verification, Cloud fallback).
- `src/policy.rs`: Granular sandbox access control (Read/Write/Network/Command whitelists, .env blacklists, blast radius score).
- `src/ledger.rs`: Node identity with Ed25519 signing/verification, SHA-256 hash chaining.
- `src/db.rs`: SQLite WAL persistence for events, nodes, policies, analytics aggregation.
- `src/agent_interface.rs`: MCP (Model Context Protocol) JSON-RPC handler, AI agent tool dispatch, overrides API.
- `src/analytics.rs`: Event aggregations, threat scoring, read/write ratio calculations.
- `src/secrets.rs`: Cluster security config, bearer tokens, upstream URL allowlists.
- `src/schema.rs`: Shared telemetry data structures (TraceEvent, NodeHeartbeat, SecurityVerdict, PolicyDecision).
- `src/services.rs`: Model introspector, discovery beacon, worker telemetry forwarder.

### Current Test Status:
- 36 unit tests across `analytics`, `antigravity_watcher`, `agent_interface`, `delegation_engine`, `policy`, `registry`, `router`, `services`, `db`.
- 4 integration tests in `tests/test_ledger.rs`.
- Zero unit tests in `src/secrets.rs`, `src/schema.rs`, `src/ledger.rs` (unit-level), `src/proxy.rs`, `src/handlers.rs`.
- Sparse coverage in `agent_interface.rs` and `policy.rs`.

### Please provide a structured review report:
1. **Architectural Evaluation & Modularity**:
   - Assess separation of concerns between Router, DelegationEngine, and Proxy.
   - Assess state management (AppState, AgentInterfaceState, Arc<RwLock<...>>).
   - Evaluate error handling and domain types.
2. **Identified Architectural Risks & Modularity Bottlenecks**:
   - Where are the tightly coupled spots?
   - Any concurrency or lock contention considerations?
3. **Targeted Unit Test Expansion Plan**:
   - Specific scenarios needed for `secrets.rs` (SSRF upstream allowlists, auth verification).
   - Specific scenarios needed for `schema.rs` (JSON serialization/deserialization, default fallbacks).
   - Specific scenarios needed for `ledger.rs` (keypair generation, sign & verify roundtrips, invalid signatures).
   - Specific scenarios needed for `policy.rs` (wildcard matching, directory boundaries, case sensitivity, command execution safety).
   - Specific scenarios needed for `delegation_engine.rs` (multiple rule ordering, edge cases in prompt patterns).
   - Specific scenarios needed for `agent_interface.rs` (MCP JSON-RPC request/response contracts, invalid methods).
4. **Concrete Rust Unit Test Implementations**:
   - Provide concrete, syntactically valid Rust test code snippets ready to be added to the codebase.
"""

payload = {
    "model": MODEL,
    "messages": [
        {
            "role": "system",
            "content": "You are a world-class Rust software architect and systems engineer. Provide detailed, rigorous, production-grade architectural review and actionable Rust test suites."
        },
        {
            "role": "user",
            "content": prompt
        }
    ],
    "max_tokens": 2000,
    "temperature": 0.4
}

print(f"[*] Delegating Architectural & Modularity Review to {MODEL} via AgentTrace Gateway ({GATEWAY_URL})...")
t0 = time.time()

req = urllib.request.Request(
    GATEWAY_URL,
    data=json.dumps(payload).encode("utf-8"),
    headers={"Content-Type": "application/json"}
)

try:
    with urllib.request.urlopen(req, timeout=240) as resp:
        res = json.loads(resp.read().decode("utf-8"))
        content = res["choices"][0]["message"]["content"]
        elapsed = time.time() - t0
        
        print(f"[+] Review completed by {MODEL} in {elapsed:.2f}s!")
        
        os.makedirs("docs", exist_ok=True)
        review_path = os.path.join("docs", "ARCHITECTURE_AND_MODULARITY_REVIEW.md")
        with open(review_path, "w", encoding="utf-8") as f:
            f.write(content)
        print(f"[+] Saved review to: {review_path}")
        
        print("\n--- REVIEW SUMMARY PREVIEW ---")
        lines = [l for l in content.splitlines() if l.strip()]
        for l in lines[:15]:
            print(l)

except Exception as e:
    print(f"[-] Error during delegation: {e}")
    sys.exit(1)
