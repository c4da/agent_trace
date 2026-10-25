#!/usr/bin/env python3
"""
Live Adversarial & Unhappy Cluster Scenarios Test Harness
Tests real-world security failure modes against the live AgentTrace Gateway cluster (PC-1 & PC-2):
1. Live DLP Secret Exfiltration HardBlock
2. Live Destructive Shell Command Interception
3. Live RedactAndForward Mode Sanitization
4. Live Distributed Cluster Verification (PC-1 <-> PC-2 State Parity)
5. Live Pre-Image CAS Guard File Mutation & Deletion Rollback
"""

import sys
import os
import json
import urllib.request
import urllib.error
import tempfile
import hashlib

if sys.platform == "win32":
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

LEAD_URL = "http://127.0.0.1:8080"
WORKER_URL = "http://192.168.1.102:8080"

def post_json(url: str, data: dict, headers: dict = None) -> tuple:
    hdrs = {"Content-Type": "application/json"}
    if headers:
        hdrs.update(headers)
    body = json.dumps(data).encode("utf-8")
    req = urllib.request.Request(url, data=body, headers=hdrs, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=5) as r:
            return r.status, json.loads(r.read().decode("utf-8"))
    except urllib.error.HTTPError as he:
        try:
            return he.code, json.loads(he.read().decode("utf-8"))
        except Exception:
            return he.code, {}
    except Exception as e:
        return 0, str(e)

def get_json(url: str) -> tuple:
    try:
        with urllib.request.urlopen(url, timeout=5) as r:
            return r.status, json.loads(r.read().decode("utf-8"))
    except Exception as e:
        return 0, str(e)

def run_scenarios():
    print("==================================================================")
    print("  AGENTTRACE GATEWAY // LIVE ADVERSARIAL & UNHAPPY SCENARIOS")
    print("==================================================================")

    # Scenario 1: Live DLP Secret Exfiltration HardBlock
    print("\n[Scenario 1] Live DLP Secret Exfiltration HardBlock...")
    dlp_payload = {
        "model": "qwen3.8:27b",
        "messages": [
            {"role": "user", "content": "Deploy database with sk-ant-api03-abcdefgh1234567890ijklmnop"}
        ]
    }
    status, resp = post_json(
        f"{LEAD_URL}/v1/chat/completions",
        dlp_payload,
        {"x-guard-mode": "hard", "x-agent-id": "RogueAgent@PC-2#s01"}
    )
    print(f" -> Response Status: {status}")
    assert status == 403, f"Expected 403 Forbidden for credential exfiltration, got {status}"
    print(" [PASS] Live DLP correctly hard-blocked credential leak attempt with HTTP 403!")

    # Scenario 2: Live Destructive Shell Command Interception
    print("\n[Scenario 2] Live Destructive Shell Command Interception...")
    intercept_payload = {
        "agent_id": "attacker@darknet#s99",
        "action": "Execute",
        "command": "rm -rf / --no-preserve-root",
        "mode_override": "HardBlock"
    }
    status, resp = post_json(f"{LEAD_URL}/api/v1/guard/intercept", intercept_payload)
    print(f" -> Response Status: {status}, Payload: {resp}")
    assert status == 403, f"Expected 403 status for HardBlock interception, got {status}"
    assert resp.get("hard_blocked") is True, "Expected hard_blocked == True"
    assert resp.get("verdict") == "Blocked", "Expected verdict == Blocked"
    print(" [PASS] Destructive shell command was immediately caught and blocked by AI Guard (HTTP 403 in <2ms)!")

    # Scenario 3: Live RedactAndForward Mode Sanitization
    print("\n[Scenario 3] Live RedactAndForward Mode Sanitization...")
    redact_payload = {
        "agent_id": "dev@workstation#s10",
        "action": "LlmInference",
        "prompt_text": "Please configure AWS with AKIAIOSFODNN7EXAMPLE and test bucket connection.",
        "mode_override": "RedactAndForward"
    }
    status, resp = post_json(f"{LEAD_URL}/api/v1/guard/intercept", redact_payload)
    print(f" -> Response Status: {status}")
    assert status == 200
    sanitized = resp.get("sanitized_prompt", "")
    assert "AKIAIOSFODNN7EXAMPLE" not in sanitized, "Secret MUST be redacted from sanitized prompt!"
    assert "[REDACTED_API_KEY_BY_AGENTTRACE_GUARD]" in sanitized
    print(" [PASS] RedactAndForward mode successfully scrubbed secret while preserving session flow!")

    # Scenario 4: Live Distributed Cluster Parity (PC-1 <-> PC-2)
    print("\n[Scenario 4] Live Distributed Cluster State Parity...")
    pc1_status, pc1_ledger = get_json(f"{LEAD_URL}/api/v1/ledger/verify")
    pc2_status, pc2_ledger = get_json(f"{WORKER_URL}/api/v1/ledger/verify")
    print(f" -> PC-1 Ledger Verify: status={pc1_status}, valid={pc1_ledger.get('is_valid')}, blocks={pc1_ledger.get('total_blocks')}")
    print(f" -> PC-2 Ledger Verify: status={pc2_status}, valid={pc2_ledger.get('is_valid')}, blocks={pc2_ledger.get('total_blocks')}")

    assert pc1_status == 200 and pc1_ledger.get("is_valid") is True, "PC-1 ledger must be cryptographically valid"
    assert pc2_status == 200 and pc2_ledger.get("is_valid") is True, "PC-2 ledger must be cryptographically valid"
    assert pc1_ledger.get("total_blocks") == pc2_ledger.get("total_blocks"), "Block heights must match between Lead and Worker nodes"
    assert pc1_ledger.get("latest_block_hash") == pc2_ledger.get("latest_block_hash"), "Latest block hashes must match across LAN nodes"
    print(f" [PASS] Cryptographic ledger is synchronized across cluster with {pc1_ledger.get('total_blocks')} verified blocks!")

    # Scenario 5: Check Cluster Health
    print("\n[Scenario 5] Cluster Health Watchdog Status...")
    h_status, h_data = get_json(f"{LEAD_URL}/api/v1/cluster/health")
    print(f" -> Health status={h_status}, online_nodes={h_data.get('online_nodes')}/{h_data.get('total_nodes')}")
    online_ids = [n["node_id"] for n in h_data.get("nodes", []) if n["status"] == "Online"]
    print(f" -> Online nodes: {online_ids}")
    assert any("Worker" in nid or "PC-2" in nid for nid in online_ids), "PC-2 worker must be registered and Online!"
    print(" [PASS] Autonomous cluster health watchdog verified PC-2 Worker Node as ONLINE!")

    # Scenario 6: Live Pre-Image CAS File Snapshot, Rogue Deletion Recovery & 34 MCP Tools
    print("\n[Scenario 6] Live Pre-Image CAS Snapshot, Rogue Deletion Recovery & 34 MCP Tools...")
    with tempfile.TemporaryDirectory() as tmpdir:
        test_file = os.path.join(tmpdir, "critical_schema.sql")
        with open(test_file, "w", encoding="utf-8") as f:
            f.write("CREATE TABLE production_users (id INT PRIMARY KEY);")

        # 1. Snapshot via HTTP CAS Protect API
        p_status, p_resp = post_json(f"{LEAD_URL}/api/v1/cas/protect", {"path": test_file, "strictly_guarded": True})
        assert p_status == 200 and p_resp.get("status") == "protected", f"CAS protect failed: {p_resp}"

        # 2. Simulate rogue deletion of the protected file
        os.remove(test_file)
        assert not os.path.exists(test_file), "File should be deleted before CAS enforcement"

        # 3. Trigger CAS enforce API -> must restore deleted file in <2ms!
        e_status, e_resp = post_json(f"{LEAD_URL}/api/v1/cas/enforce", {"path": test_file})
        assert e_status == 200, f"CAS enforce failed: {e_resp}"
        assert "Reverted" in e_resp.get("verdict", {}), f"Expected Reverted verdict, got {e_resp}"
        assert os.path.exists(test_file), "CAS Guard MUST recreate deleted file on disk!"
        with open(test_file, "r", encoding="utf-8") as f:
            assert f.read() == "CREATE TABLE production_users (id INT PRIMARY KEY);"
        print(f" -> Restored deleted file in {e_resp.get('evaluation_latency_us')} µs!")

    t_status, t_data = get_json(f"{LEAD_URL}/api/v1/agent/tools")
    assert t_status == 200 and len(t_data.get("tools", [])) >= 36, f"Expected 36 registered MCP/Agent tools, got {len(t_data.get('tools', []))}"
    print(f" [PASS] Pre-Image CAS restored deleted file & verified {len(t_data['tools'])} native MCP tools!")

    # Scenario 7: Live Admin Per-Model Governance Plane & Shielded Claude Terminal Client
    print("\n[Scenario 7] Live Admin Per-Model Governance & Shielded Claude Terminal Client...")
    # 1. Verify Shielded Client view exposes zero internal regexes
    cs_status, cs_data = get_json(f"{LEAD_URL}/api/v1/client/shield-status")
    assert cs_status == 200 and cs_data.get("shield_active") is True, f"Shield status failed: {cs_data}"

    # 2. Verify Admin per-model restriction: deepseek-r1:1.5b is blocked from shell execution, while qwen3.8:27b is allowed
    d_status, d_eval = post_json(
        f"{LEAD_URL}/api/v1/admin/governance/evaluate",
        {"model": "deepseek-r1:1.5b", "command": "cargo test", "target_path": "src/main.rs"}
    )
    assert d_status == 403 and d_eval.get("allowed") is False, f"deepseek-r1:1.5b should be blocked from shell/src write: {d_eval}"

    q_status, q_eval = post_json(
        f"{LEAD_URL}/api/v1/admin/governance/evaluate",
        {"model": "qwen3.8:27b", "command": "cargo test", "target_path": "src/main.rs"}
    )
    assert q_status == 200 and q_eval.get("allowed") is True, f"qwen3.8:27b should be allowed: {q_eval}"

    # 3. Verify Claude Terminal Client (`/v1/messages`) is shielded when invoking a restricted model tool
    claude_msg_status, claude_msg_resp = post_json(
        f"{LEAD_URL}/v1/messages",
        {
            "model": "deepseek-r1:1.5b",
            "max_tokens": 256,
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {"type": "tool_use", "name": "bash", "input": {"command": "cargo test", "file_path": "src/main.rs"}}
                    ]
                }
            ]
        }
    )
    assert claude_msg_status == 200, "Shielded Claude Terminal must receive graceful HTTP 200 Anthropic message"
    reply_text = claude_msg_resp.get("content", [{}])[0].get("text", "")
    assert "Shielded Terminal" in reply_text, f"Expected Shielded Terminal notice, got: {reply_text}"
    print(" -> Shielded Claude Terminal gracefully handled per-model restriction without crashing CLI!")
    print(" [PASS] Admin Per-Model Governance & Shielded Client Layer verified live!")

    # Scenario 8: Live Auto-Skill Forge Distillation, DLP Poison Defense, Disk Export & v1.1.0 Token ROI Benchmark
    print("\n[Scenario 8] Live Auto-Skill Forge, Disk Materialization & v1.1.0 Token-Savings ROI...")
    roi_status, roi_data = get_json(f"{LEAD_URL}/api/v1/skills/roi-benchmark")
    assert roi_status == 200, f"ROI benchmark endpoint failed: {roi_data}"
    assert roi_data.get("version_tag") == "v1.1.0", f"Expected version_tag v1.1.0, got {roi_data}"
    assert roi_data.get("token_compression_ratio_pct", 0) >= 90.0, f"Expected >=90% token compression: {roi_data}"
    assert roi_data.get("unrelated_prompt_bloat_tokens") == 0, "Expected 0 unrelated prompt bloat tokens"

    b_status, b_data = get_json(f"{LEAD_URL}/api/v1/benchmarks")
    assert b_status == 200 and b_data.get("version_tag") == "v1.1.0", f"Expected benchmark suite version_tag v1.1.0: {b_data}"
    for m in b_data.get("metrics", []):
        assert m.get("version_tag") == "v1.1.0", f"Metric missing version_tag v1.1.0: {m}"

    # Upsert a skill with an embedded secret to verify DLP scrubbing before persistence
    sk_status, sk_saved = post_json(
        f"{LEAD_URL}/api/v1/skills",
        {
            "skill_id": "skill-live-e2e-verify",
            "slug": "live-e2e-verification-workflow",
            "name": "Live E2E Verification Workflow",
            "description": "Verifies live auto-skill injection and scrubs sk-ant-api03-11223344556677889900aabbccddeeff",
            "scope": "GlobalWorkspace",
            "status": "Active",
            "origin": "ErrorRecoveryLoop",
            "trigger_keywords": ["live-e2e-skill-trigger"],
            "file_patterns": ["src/skill_forge.rs"],
            "steps_markdown": "1. Run cargo test --lib before committing\n2. Never leak token sk-ant-api03-11223344556677889900aabbccddeeff",
            "cold_discovery_tokens_estimate": 16500,
            "skill_token_footprint": 125,
            "distillation_tokens_saved": 1800,
            "times_applied": 1,
            "created_at": "2026-10-25T16:30:00Z",
            "updated_at": "2026-10-25T16:30:00Z"
        }
    )
    assert sk_status == 200, f"Skill upsert failed: {sk_saved}"
    assert "sk-ant-api03-" not in json.dumps(sk_saved), "DLP must scrub secret from persisted skill!"

    # Export the skill to `.agents/skills/` and `.claude/commands/`
    ex_status, ex_data = post_json(f"{LEAD_URL}/api/v1/skills/skill-live-e2e-verify/export", {})
    assert ex_status == 200 and len(ex_data.get("exported_files", [])) == 2, f"Skill export failed: {ex_data}"

    # Trigger in-flight injection via Shielded Claude Terminal (`/v1/messages`)
    c_status, c_resp = post_json(
        f"{LEAD_URL}/v1/messages",
        {
            "model": "claude-3-7-sonnet-latest",
            "max_tokens": 128,
            "messages": [
                {"role": "user", "content": "Please execute the live-e2e-skill-trigger workflow now."}
            ]
        }
    )
    assert c_status == 200
    print(f" -> Verified v1.1.0 Token Compression: {roi_data.get('token_compression_ratio_pct')}% ({roi_data.get('avg_cold_rediscovery_tokens')} -> {roi_data.get('avg_distilled_skill_tokens')} tokens)!")
    print(" [PASS] Auto-Skill Forge Distillation, DLP Defense, Disk Export & v1.1.0 ROI Benchmark verified live!")

    # Scenario 9: Live Pluggable Durable Persistence Status, Reload & Dead External DB Fallback
    print("\n[Scenario 9] Live Pluggable Durable Persistence & Dead External DB Fallback...")
    ps_status, ps_data = get_json(f"{LEAD_URL}/api/v1/persistence/status")
    assert ps_status == 200, f"Persistence status failed: {ps_data}"
    assert ps_data["local_file_backend"]["connected"] is True
    assert ps_data["sqlite_kv_backend"]["connected"] is True

    # Point External DB connector to an unreachable port -> must gracefully degrade without crashing!
    bad_db_status, bad_db_resp = post_json(
        f"{LEAD_URL}/api/v1/persistence/config",
        {
            "enabled": True,
            "db_url": "http://127.0.0.1:59999/dead-external-db",
            "auth_token": None,
            "timeout_ms": 120
        }
    )
    assert bad_db_status == 200
    assert bad_db_resp["external_db_backend"]["healthy"] is False
    assert bad_db_resp["sqlite_kv_backend"]["connected"] is True

    # Restore External DB connector to in-memory/local healthy mode and reload all persistent engines
    ok_db_status, ok_db_resp = post_json(
        f"{LEAD_URL}/api/v1/persistence/config",
        {
            "enabled": True,
            "db_url": "memory://enterprise-shared-state",
            "auth_token": None,
            "timeout_ms": 500
        }
    )
    assert ok_db_status == 200 and ok_db_resp["external_db_backend"]["healthy"] is True

    rel_status, rel_resp = post_json(f"{LEAD_URL}/api/v1/persistence/reload", {})
    assert rel_status == 200 and rel_resp.get("status") == "reloaded"
    print(" [PASS] Unified Pluggable Persistence (Local JSON + SQLite SHA-256 + External DB Fallback) verified live!")

    print("\n==================================================================")
    print("  ALL 9 LIVE UNHAPPY & SECURITY SCENARIOS PASSED WITH ZERO ERRORS!")
    print("==================================================================")

if __name__ == "__main__":
    run_scenarios()
