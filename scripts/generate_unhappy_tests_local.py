import urllib.request
import json
import sys

def main():
    model = "gemma4:latest"
    print(f"[*] Querying local model {model} for advanced unhappy-path security test suites...")
    
    prompt = """You are a senior Rust systems engineer and offensive security auditor.
In AgentTrace Gateway (`src/adversarial_tests.rs`), we test resistance against malicious agent behavior.

Here are the imports and types available:
```rust
use crate::{
    ai_guard::{evaluate_hard_guard, inspect_shell_command_hard, scan_and_redact_secrets, scan_prompt_injection, GuardEnforcementMode, GuardInterceptRequest, ThreatCategory},
    cas_guard::{CasVerdict, PreImageCasGuard},
    db::Database,
    ledger::NodeIdentity,
    policy::{AccessAction, PolicyEngine},
    schema::{EventType, SecurityVerdict, TraceEvent},
    secrets::ClusterSecurityConfig,
    security_hardening::{constant_time_eq, EncryptedSecretVault, InterGatewayMtlsConfig},
    agent_identity::AgentIdentity,
};
use axum::http::{HeaderMap, HeaderValue};
use std::collections::HashSet;
use std::path::PathBuf;
use tempfile::NamedTempFile;
```

Write 4 distinct, production-grade unhappy/adversarial unit tests in Rust:
1. `test_adversarial_dlp_obfuscated_and_embedded_secrets`: Tests secrets embedded in markdown backticks, JSON string escapes (`\"sk-ant-...\"`), mixed tabs/newlines, and multiple secrets in one prompt.
2. `test_adversarial_command_obfuscation_and_whitespace`: Tests destructive command evasion attempts (tab-separated `rm\\t-rf\\t/`, uppercase `RM -RF /`, PowerShell variations like `del  /f /s /q c:\\windows`, and secret exfiltration via curl + .env with tricky spaces).
3. `test_adversarial_ledger_signature_tampering_and_swapped_keys`: Inserts a valid signed event into SQLite, then asserts that verifying with a different newly-generated `NodeIdentity` or tampering with the event payload hash fails chain verification or signature verification.
4. `test_adversarial_agent_identity_header_fuzzing`: Tests `AgentIdentity::from_headers_and_session` with headers containing colons, @, newlines, empty strings, and long 500-char session strings to ensure clean deterministic sanitization without panicking.

Output ONLY valid Rust code inside a ```rust block. Every test must compile and pass assertions against the real behavior.
"""

    payload = {
        "model": model,
        "messages": [
            {"role": "system", "content": "You write clean, strictly compiling Rust unit tests. Output only Rust code."},
            {"role": "user", "content": prompt}
        ],
        "stream": False,
    }

    req = urllib.request.Request(
        "http://127.0.0.1:11434/api/chat",
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"}
    )

    try:
        resp = urllib.request.urlopen(req, timeout=120)
        res_data = json.loads(resp.read().decode("utf-8"))
        content = res_data["message"]["content"]
        print("\n--- MODEL GENERATED TESTS ---\n")
        print(content)
        
        # Save output to scratch
        with open("scratch_generated_tests.rs", "w", encoding="utf-8") as f:
            f.write(content)
            
    finally:
        # Crucial VRAM hygiene per user rules!
        print("\n[*] Unloading VRAM for model", model)
        unload_req = urllib.request.Request(
            "http://127.0.0.1:11434/api/generate",
            data=json.dumps({"model": model, "keep_alive": 0}).encode("utf-8"),
            headers={"Content-Type": "application/json"}
        )
        try:
            urllib.request.urlopen(unload_req, timeout=10)
            print("[+] VRAM unloaded successfully.")
        except Exception as e:
            print(f"[!] Warning: VRAM unload failed: {e}")

if __name__ == "__main__":
    main()
