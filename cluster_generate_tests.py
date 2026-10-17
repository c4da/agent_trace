import urllib.request
import json
import sys

sys.stdout.reconfigure(encoding='utf-8')

prompt = """You are a Rust testing specialist.
Write comprehensive Rust unit tests for `analytics.rs` and `db.rs` in `agent_trace_gw`.

`analytics.rs` has:
```rust
impl AgentProfile {
    pub fn compute_threat(_reads: u64, writes: u64, commands: u64, violations: u64) -> (f64, String);
    pub fn compute_ratio(reads: u64, writes: u64) -> f64;
}
```
Formula:
`score = ((writes * 10 + commands * 5 + violations * 25).min(100)) as f64`
`level = if violations > 0 || score >= 75.0 { "CRITICAL" } else if score >= 50.0 { "HIGH" } else if score >= 20.0 { "MEDIUM" } else { "LOW" }`
`compute_ratio(reads, writes)`: if writes == 0 { reads as f64 } else { (reads/writes * 100).round() / 100.0 }

Please output the exact Rust `#[cfg(test)] mod tests { ... }` block for `analytics.rs` testing all edge cases:
1. compute_threat for LOW (<20 score, 0 violations)
2. compute_threat for MEDIUM (>=20, <50 score, 0 violations)
3. compute_threat for HIGH (>=50, <75 score, 0 violations)
4. compute_threat for CRITICAL (score >= 75 or violations > 0)
5. compute_threat score cap at 100
6. compute_ratio with zero writes
7. compute_ratio with normal numbers and rounding
"""

payload = {
    "model": "gemma4:latest",
    "messages": [
        {"role": "system", "content": "You are a senior Rust systems programmer writing clean, idiomatic unit tests."},
        {"role": "user", "content": prompt}
    ],
    "max_tokens": 1000,
    "temperature": 0.1
}

req = urllib.request.Request(
    "http://localhost:8080/v1/chat/completions",
    data=json.dumps(payload).encode("utf-8"),
    headers={"Content-Type": "application/json"}
)

print("[*] Delegating test generation to local cluster agent (gemma4:latest)...")
try:
    with urllib.request.urlopen(req, timeout=90) as resp:
        res = json.loads(resp.read().decode("utf-8"))
        content = res["choices"][0]["message"].get("content") or res["choices"][0]["message"].get("reasoning")
        print("\n=== GENERATED UNIT TESTS ===")
        print(content)
except Exception as e:
    print(f"[-] Error: {e}")
