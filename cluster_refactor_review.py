import urllib.request
import json
import sys

# Ensure UTF-8 output
sys.stdout.reconfigure(encoding='utf-8')

prompt = """You are an expert Rust systems architect and code reviewer.
Review the architecture and modularity of the agent_trace_gw repository based on the following module structure:

1. `src/lib.rs` exports:
   pub mod agent_interface;
   pub mod analytics;
   pub mod antigravity_watcher;
   pub mod db;
   pub mod delegation_engine;
   pub mod ledger;
   pub mod policy;
   pub mod proxy;
   pub mod registry;
   pub mod router;
   pub mod schema;
   pub mod secrets;

2. Currently, `src/main.rs` also contains:
   mod agent_interface;
   mod analytics;
   mod antigravity_watcher;
   ... (re-declaring all 12 modules instead of importing `use agent_trace_gw::*;`)
   Along with 620 lines mixing HTTP route handlers, background service loops (UDP beacon, model introspector, worker forwarder), and static asset distribution.

3. Current unit test distribution:
   - `policy.rs`: 3 tests
   - `router.rs`: 5 tests
   - `registry.rs`: 4 tests
   - `delegation_engine.rs`: 2 tests
   - `tests/test_ledger.rs`: 4 tests
   - `analytics.rs`: 0 tests
   - `db.rs`: 0 tests
   - `agent_interface.rs`: 0 tests

Please analyze:
1. What are the key modularity and architectural flaws in having `main.rs` re-declare modules instead of using the library crate?
2. How should `main.rs` be refactored into modular components (e.g., handlers module, background services module)?
3. What unit tests are critical to add for `analytics.rs` and `db.rs` to ensure rock-solid production reliability?
4. What are the top 3 high-impact features or extensions we can build next?

Provide a concise, highly structured, and actionable architectural review.
"""

payload = {
    "model": "gemma4:latest",
    "messages": [
        {"role": "system", "content": "You are a senior Rust systems architect."},
        {"role": "user", "content": prompt}
    ],
    "max_tokens": 1500,
    "temperature": 0.2
}

req = urllib.request.Request(
    "http://localhost:8080/v1/chat/completions",
    data=json.dumps(payload).encode("utf-8"),
    headers={"Content-Type": "application/json"}
)

print("[*] Delegating architectural review to local cluster model (gemma4:latest)...")
try:
    with urllib.request.urlopen(req, timeout=90) as resp:
        res = json.loads(resp.read().decode("utf-8"))
        content = res["choices"][0]["message"].get("content") or res["choices"][0]["message"].get("reasoning")
        print("\n=== CLUSTER AGENT ARCHITECTURAL REVIEW ===")
        print(content)
except Exception as e:
    print(f"[-] Error calling cluster agent: {e}")
