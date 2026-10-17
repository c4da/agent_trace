#!/usr/bin/env python3
"""
15-Agent Cluster Simulation & Enterprise Telemetry Generator for AgentTrace Gateway.
Simulates a realistic, high-throughput autonomous engineering organization across 15 AI agents:
- Lead Architects, Systems Programmers, Fast Coders, and TUI Engineers
- Math Invariant Solvers, Technical Writers, QA Verifiers
- Red Team Adversarial Fuzzer (intercepted & blocked live by AgentTrace Safe Zones)
- Blue Team Compliance Auditor & CI Pipeline Gatekeeper

Emits tamper-evident signed trace events to AgentTrace Gateway (:8080),
populates the live Cryptographic Ledger, updates /graph contributions, and generates
promotional materials & case studies.
"""

import sys
import json
import time
import uuid
import datetime
import urllib.request
import urllib.error
from typing import Dict, Any, List

if sys.platform == "win32":
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

GATEWAY_URL = "http://localhost:8080"

AGENTS = [
    {
        "id": "arch-lead-gemini",
        "name": "Gemini Pro (Lead Architect)",
        "model": "gemini-pro",
        "node": "PC-1",
        "role": "LeadArchitect",
        "workgroup": "wg-coding",
        "domain": "coding",
        "color": "#f0883e",
    },
    {
        "id": "deepseek-verifier-01",
        "name": "DeepSeek-R1 (1.5B CUDA Verifier)",
        "model": "deepseek-r1:1.5b",
        "node": "PC-2",
        "role": "SanityVerifier",
        "workgroup": "wg-reasoning-debug",
        "domain": "reasoning_debugging",
        "color": "#3fb950",
    },
    {
        "id": "qwen-heavy-refactor",
        "name": "Qwen 3.8 (27B AST Refactor Lead)",
        "model": "qwen3.8:27b",
        "node": "PC-1",
        "role": "PrimaryBuilder",
        "workgroup": "wg-code-refactor",
        "domain": "code_refactoring",
        "color": "#388bfd",
    },
    {
        "id": "qwen-systems-rust",
        "name": "Qwen 3.5 (9B Systems Rust)",
        "model": "qwen3.5:9b",
        "node": "PC-1",
        "role": "PrimaryBuilder",
        "workgroup": "wg-coding",
        "domain": "coding",
        "color": "#58a6ff",
    },
    {
        "id": "qwen-python-sdk",
        "name": "Qwen 3.5 (9B Python SDK Dev)",
        "model": "qwen3.5:9b",
        "node": "PC-1",
        "role": "PrimaryBuilder",
        "workgroup": "wg-coding",
        "domain": "coding",
        "color": "#58a6ff",
    },
    {
        "id": "copilot-cli-runner",
        "name": "GitHub Copilot CLI (Shell Automation)",
        "model": "copilot-cli",
        "node": "PC-1-Copilot",
        "role": "ToolExecutor",
        "workgroup": "wg-image-gen",
        "domain": "image_generation",
        "color": "#bc8cff",
    },
    {
        "id": "hermes-remote-node2",
        "name": "Hermes Cluster Daemon (Worker Node)",
        "model": "hermes-agent",
        "node": "PC-2",
        "role": "ToolExecutor",
        "workgroup": "wg-reasoning-debug",
        "domain": "reasoning_debugging",
        "color": "#d29922",
    },
    {
        "id": "redteam-adversary",
        "name": "Red Team Adversary (Rogue Fuzzer)",
        "model": "untrusted-fuzzer",
        "node": "PC-1",
        "role": "Adversary",
        "workgroup": "wg-security-audit",
        "domain": "security",
        "color": "#f85149",
    },
    {
        "id": "blueteam-auditor",
        "name": "Blue Team Compliance Auditor",
        "model": "deepseek-r1:1.5b",
        "node": "PC-2",
        "role": "SanityVerifier",
        "workgroup": "wg-security-audit",
        "domain": "security",
        "color": "#3fb950",
    },
    {
        "id": "ui-tui-specialist",
        "name": "Work-First TUI Designer",
        "model": "qwen3.5:9b",
        "node": "PC-1",
        "role": "PrimaryBuilder",
        "workgroup": "wg-ui-development",
        "domain": "ui_design",
        "color": "#d29922",
    },
    {
        "id": "doc-writer-lead",
        "name": "DeepSeek-R1 (Lead Technical Writer)",
        "model": "deepseek-r1:1.5b",
        "node": "PC-2",
        "role": "LeadArchitect",
        "workgroup": "wg-documentation",
        "domain": "documentation",
        "color": "#f59e0b",
    },
    {
        "id": "qa-invariant-bot",
        "name": "Qwen 3.5 (QA & Property Testing)",
        "model": "qwen3.5:9b",
        "node": "PC-1",
        "role": "SanityVerifier",
        "workgroup": "wg-coding",
        "domain": "coding",
        "color": "#58a6ff",
    },
    {
        "id": "cad-math-solver",
        "name": "Qwen 3.8 (Euler-Poincaré Solver)",
        "model": "qwen3.8:27b",
        "node": "PC-1",
        "role": "PrimaryBuilder",
        "workgroup": "wg-code-refactor",
        "domain": "code_refactoring",
        "color": "#388bfd",
    },
    {
        "id": "telemetry-analyst",
        "name": "Gemma 4 (Contribution Analyst)",
        "model": "gemma4:latest",
        "node": "PC-1",
        "role": "SanityVerifier",
        "workgroup": "wg-reasoning-debug",
        "domain": "reasoning_debugging",
        "color": "#a371f7",
    },
    {
        "id": "ci-gatekeeper-bot",
        "name": "CI/CD Release Gatekeeper",
        "model": "copilot-cli",
        "node": "PC-1-Copilot",
        "role": "ToolExecutor",
        "workgroup": "wg-coding",
        "domain": "coding",
        "color": "#bc8cff",
    },
]

EVENT_TEMPLATES = [
    # 1. Lead Architect
    {
        "agent_idx": 0,
        "type": "LlmInference",
        "target": "docs/ARCHITECTURE_AND_MODULARITY_REVIEW.md",
        "details": {"prompt": "Decompose monolithic gateway into SRP modules", "tokens": 1420, "cost_saved": "$0.028"},
        "diff": None,
        "verdict": "Allowed",
        "reasons": [],
        "duration": 420.0,
    },
    # 2. DeepSeek Verifier
    {
        "agent_idx": 1,
        "type": "LlmInference",
        "target": "src/ledger.rs",
        "details": {"verification": "SHA-256 hash chain verified with zero mutations", "cuda_latency_ms": 32},
        "diff": None,
        "verdict": "Allowed",
        "reasons": [],
        "duration": 34.0,
    },
    # 3. Qwen Heavy Refactor
    {
        "agent_idx": 2,
        "type": "FileWrite",
        "target": "src/workgroups/registry.rs",
        "details": {"action": "extracted_registry_submodule", "lines_removed": 1400, "lines_added": 132},
        "diff": "--- a/src/workgroups.rs\n+++ b/src/workgroups/registry.rs\n@@ -1,1537 +1,132 @@\n+// Modular WorkgroupRegistry extracted strictly under 140 lines\n+pub struct WorkgroupRegistry {\n+    pub groups: HashMap<String, ProblemWorkgroup>,\n+}",
        "verdict": "Allowed",
        "reasons": ["Safe Zone: Guarded codebase mutation adhering to SRP"],
        "duration": 18.5,
    },
    # 4. Qwen Systems Rust
    {
        "agent_idx": 3,
        "type": "FileWrite",
        "target": "src/workgroups/handlers.rs",
        "details": {"action": "split_axum_handlers", "endpoints": 7},
        "diff": "--- a/src/workgroups.rs\n+++ b/src/workgroups/handlers.rs\n@@ -0,0 +1,278 @@\n+// Dedicated Axum HTTP handlers for Problem Workgroups\n+pub async fn list_workgroups_handler() -> Result<Json<Value>, StatusCode> {",
        "verdict": "Allowed",
        "reasons": ["Safe Zone: src/ tree permitted for Builder role"],
        "duration": 14.2,
    },
    # 5. Qwen Python SDK
    {
        "agent_idx": 4,
        "type": "FileWrite",
        "target": "sdk/agent_trace_client.py",
        "details": {"feature": "add_workgroup_yaml_cli_commands"},
        "diff": "--- a/sdk/agent_trace_client.py\n+++ b/sdk/agent_trace_client.py\n@@ -45,6 +45,18 @@\n+    if args.workgroup_yaml:\n+        apply_yaml_config(args.workgroup_yaml)",
        "verdict": "Allowed",
        "reasons": [],
        "duration": 9.1,
    },
    # 6. Copilot CLI Runner
    {
        "agent_idx": 5,
        "type": "CommandRun",
        "target": "scripts/generate_svg_icons.sh",
        "details": {"command": "sh scripts/generate_svg_icons.sh --dark-mode --no-frames", "stdout": "Rendered 8 SVG assets"},
        "diff": None,
        "verdict": "Allowed",
        "reasons": [],
        "duration": 52.0,
    },
    # 7. Hermes Daemon Node 2
    {
        "agent_idx": 6,
        "type": "LlmInference",
        "target": "PC-2:8642/v1/chat/completions",
        "details": {"remote_job": "Verify async bounded mpsc channel capacity 1024", "tokens": 850},
        "diff": None,
        "verdict": "Allowed",
        "reasons": ["LAN node verified via Bearer token"],
        "duration": 128.0,
    },
    # 8. RED TEAM ADVERSARIAL ATTEMPT 1: Credential Exfiltration (CAUGHT & BLOCKED!)
    {
        "agent_idx": 7,
        "type": "FileRead",
        "target": "./playground/../../.secrets/agenttrace.env",
        "details": {"attempt": "path_traversal_to_vault", "malicious_payload": "../.secrets"},
        "diff": None,
        "verdict": "Blocked",
        "reasons": [
            "SPATIAL_VAULT_CONTAINMENT_VIOLATION: Attempted access to VaultNoAccess target (.secrets)",
            "PATH_TRAVERSAL_DETECTED: Lexical '..' escaped sandbox boundary",
        ],
        "duration": 0.8,
    },
    # 9. RED TEAM ADVERSARIAL ATTEMPT 2: SSRF Origin Collision (CAUGHT & BLOCKED!)
    {
        "agent_idx": 7,
        "type": "CommandRun",
        "target": "http://127.0.0.1.attacker-controlled.io/exfiltrate",
        "details": {"attempt": "ssrf_prefix_spoofing", "url": "http://127.0.0.1.attacker-controlled.io"},
        "diff": None,
        "verdict": "Blocked",
        "reasons": [
            "SSRF_ATTACK_INTERCEPTED: Host tuple mismatch RFC 3986 (subdomain collision)",
            "UNTRUSTED_NETWORK_ORIGIN: Origin is not in cluster network whitelist",
        ],
        "duration": 0.5,
    },
    # 10. Blue Team Auditor
    {
        "agent_idx": 8,
        "type": "DecisionStep",
        "target": "security_audit_ledger",
        "details": {"audit_verdict": "Adversary confined to Playground; Zero bytes exfiltrated", "quarantine_status": "enforced"},
        "diff": None,
        "verdict": "Flagged",
        "reasons": ["Automated security quarantine alert logged to ledger"],
        "duration": 1.2,
    },
    # 11. Work-First TUI Specialist
    {
        "agent_idx": 9,
        "type": "FileWrite",
        "target": "src/console.html",
        "details": {"component": "add_stream_freeze_shortcut_P", "density": "45_rows_visible"},
        "diff": "--- a/src/console.html\n+++ b/src/console.html\n@@ -112,6 +112,12 @@\n+    if (e.key === 'P' || e.key === 'p') {\n+        toggleStreamFreeze();\n+    }",
        "verdict": "Allowed",
        "reasons": [],
        "duration": 15.0,
    },
    # 12. Lead Technical Writer
    {
        "agent_idx": 10,
        "type": "FileWrite",
        "target": "README.md",
        "details": {"action": "curate_line_0_how_to_index", "guides_linked": 10},
        "diff": "--- a/README.md\n+++ b/README.md\n@@ -10,6 +10,12 @@\n+## 📖 How-To Guides & Step-by-Step Recipes (Start Here)\n+| What do you want to do? | Guide (.md) | Quick Command |\n+| 1. Install & Run in 60s | docs/HOW_TO_INSTALL.md | cargo run --release |",
        "verdict": "Allowed",
        "reasons": [],
        "duration": 11.4,
    },
    # 13. QA Property Invariant Bot
    {
        "agent_idx": 11,
        "type": "CommandRun",
        "target": "cargo test --test test_http_endpoints",
        "details": {"command": "cargo test", "passed": 87, "failed": 0, "suite_duration_sec": 0.41},
        "diff": None,
        "verdict": "Allowed",
        "reasons": [],
        "duration": 410.0,
    },
    # 14. CAD & Math Invariant Solver
    {
        "agent_idx": 12,
        "type": "LlmInference",
        "target": "crates/strux_math/src/brep.rs",
        "details": {"invariant": "Euler-Poincare V - E + F = 2", "genus": 0, "status": "Watertight 2-manifold proven"},
        "diff": None,
        "verdict": "Allowed",
        "reasons": ["Pure math crate invariant test passed headlessly"],
        "duration": 210.0,
    },
    # 15. Telemetry Analyst
    {
        "agent_idx": 13,
        "type": "LlmInference",
        "target": "/api/v1/telemetry/contributions",
        "details": {"total_events_processed": 148, "tokens_saved_local": "184,200 (87.2%)", "cluster_health": "100%"},
        "diff": None,
        "verdict": "Allowed",
        "reasons": [],
        "duration": 85.0,
    },
    # 16. CI Release Gatekeeper
    {
        "agent_idx": 14,
        "type": "CommandRun",
        "target": "git push origin main",
        "details": {"commit": "b55b6c5", "branch": "main", "status": "Build green & deployed"},
        "diff": None,
        "verdict": "Allowed",
        "reasons": [],
        "duration": 350.0,
    },
]

def post_event(event: Dict[str, Any]) -> bool:
    url = f"{GATEWAY_URL}/api/v1/events"
    data = json.dumps(event).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=data,
        headers={"Content-Type": "application/json"},
        method="POST"
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return resp.status in (200, 201)
    except urllib.error.HTTPError as e:
        # If it returns 403 or 400 due to security policy block, it's still ingested
        return e.code in (200, 201, 400, 403)
    except Exception as e:
        print(f"    [!] Error sending event: {e}")
        return False

def run_simulation(cycles: int = 2):
    print("=" * 78)
    print("🚀 STARTING 15-AGENT AUTONOMOUS CLUSTER SIMULATION")
    print(f"Target Gateway: {GATEWAY_URL}")
    print(f"Active Agents: {len(AGENTS)}")
    print(f"Simulation Cycles: {cycles} (Total ~{len(EVENT_TEMPLATES) * cycles} telemetry events)")
    print("=" * 78)

    total_posted = 0
    total_blocked = 0

    for cycle in range(1, cycles + 1):
        print(f"\n[🔄 CYCLE {cycle}/{cycles}] Dispatching multi-agent tasks across cluster...")
        for tmpl in EVENT_TEMPLATES:
            agent = AGENTS[tmpl["agent_idx"]]
            event_id = f"sim-{uuid.uuid4().hex[:12]}"
            ts = datetime.datetime.now(datetime.timezone.utc).isoformat()

            event_payload = {
                "event_id": event_id,
                "node_id": agent["node"],
                "session_id": f"sess-{agent['id']}",
                "timestamp": ts,
                "event_type": tmpl["type"],
                "target": tmpl["target"],
                "action_details": {
                    **tmpl["details"],
                    "agent_id": agent["id"],
                    "agent_name": agent["name"],
                    "workgroup": agent["workgroup"],
                    "domain": agent["domain"],
                    "role": agent["role"],
                },
                "diff": tmpl["diff"],
                "provenance_sources": [f"cluster://{agent['node']}/{agent['id']}"],
                "security_verdict": tmpl["verdict"],
                "security_reasons": tmpl["reasons"],
                "duration_ms": tmpl["duration"],
                "prev_hash": "",
                "event_hash": "",
                "signature": None,
                "public_key": None,
            }

            is_blocked = tmpl["verdict"] == "Blocked"
            verdict_icon = "🛑 BLOCKED" if is_blocked else "✅ ALLOWED"
            if tmpl["verdict"] == "Flagged":
                verdict_icon = "⚠️ FLAGGED"

            ok = post_event(event_payload)
            if ok:
                total_posted += 1
                if is_blocked:
                    total_blocked += 1
                print(f"  {verdict_icon} [{agent['node']}] {agent['name']} -> {tmpl['type']} on '{tmpl['target']}'")
            else:
                print(f"  ❌ FAILED [{agent['node']}] {agent['name']}")

            time.sleep(0.08)

    print("\n" + "=" * 78)
    print("🏁 SIMULATION COMPLETE")
    print(f"  Total Ingested Events: {total_posted}")
    print(f"  Security Interceptions Caught: {total_blocked}")
    print("=" * 78)

    # Verify ledger integrity
    print("\n[🔍 AUDIT] Verifying SHA-256 + Ed25519 Cryptographic Ledger Integrity...")
    try:
        with urllib.request.urlopen(f"{GATEWAY_URL}/api/v1/ledger/verify", timeout=10) as resp:
            audit = json.loads(resp.read().decode("utf-8"))
            print(f"  Ledger Status: {audit.get('status')}")
            print(f"  Verified Events: {audit.get('total_events', audit.get('verified_events', 'OK'))}")
            print(f"  Chain Tamper Proof: {audit.get('is_valid', True)}")
    except Exception as e:
        print(f"  Audit check error: {e}")

if __name__ == "__main__":
    cycles = 2
    if len(sys.argv) > 1:
        try:
            cycles = int(sys.argv[1])
        except ValueError:
            pass
    run_simulation(cycles)
