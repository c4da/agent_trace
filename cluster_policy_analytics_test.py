#!/usr/bin/env python3
"""
AgentTrace Telemetry Analytics & Agent Sandbox Whitelist Verification Suite
Delegates end-to-end testing of:
1. Deep Telemetry Analytics & Hotspot Detection
2. Agent Data Access Whitelist & Sandbox Policy Enforcement
3. Autonomous Local Agent Audit Synthesis via Hermes CLI
"""

import datetime
import json
import subprocess
import sys
import time
import urllib.request
import urllib.error

if sys.platform == "win32":
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

import os

GATEWAY_URL = "http://localhost:8080"
HERMES_BIN = os.path.expandvars(r"%LOCALAPPDATA%\hermes\bin\hermes.cmd")

def post_json(url: str, data: dict, timeout: int = 15) -> tuple:
    """Returns (status_code, response_dict)"""
    body = json.dumps(data).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=body,
        headers={"Content-Type": "application/json", "User-Agent": "AgentTrace-Tester/1.0"}
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.getcode(), json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        err_body = e.read().decode("utf-8")
        try:
            return e.code, json.loads(err_body)
        except Exception:
            return e.code, {"error": err_body}

def get_json(url: str, timeout: int = 10) -> dict:
    req = urllib.request.Request(url, headers={"User-Agent": "AgentTrace-Tester/1.0"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))

def delete_json(url: str, timeout: int = 10) -> dict:
    req = urllib.request.Request(url, method="DELETE", headers={"User-Agent": "AgentTrace-Tester/1.0"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))

def test_telemetry_analytics():
    print("=" * 65)
    print(" [SECTION 1] DEEP TELEMETRY ANALYTICS & HOTSPOT VERIFICATION")
    print("=" * 65)

    print("\n[*] Querying GET /api/v1/telemetry/analytics...")
    res = get_json(f"{GATEWAY_URL}/api/v1/telemetry/analytics")
    
    total_events = res.get("total_events", 0)
    top_reads = res.get("top_read_targets", [])
    top_writes = res.get("top_write_targets", [])
    agent_profiles = res.get("agent_profiles", [])
    time_series = res.get("time_series", [])
    security_anomalies = res.get("security_anomalies", [])
    ledger_valid = res.get("ledger_valid", False)

    print(f"    [+] Total Ledger Events:    {total_events}")
    print(f"    [+] Ledger Valid:           {ledger_valid}")
    print(f"    [+] Top Read Hotspots:      {len(top_reads)}")
    for r in top_reads[:5]:
        print(f"        • [{r.get('count')} reads] {r.get('target')} (Risk: {r.get('risk_flagged')})")

    print(f"\n    [+] Top Write Hotspots:     {len(top_writes)}")
    for w in top_writes[:5]:
        print(f"        • [{w.get('count')} writes] {w.get('target')} (Risk: {w.get('risk_flagged')})")

    print(f"\n    [+] Agent Behavioral Profiles: {len(agent_profiles)}")
    for p in agent_profiles:
        print(f"        • Node: {p.get('agent_id')} | Events: {p.get('total_events')} | R/W: {p.get('read_to_write_ratio')}:1 | Threat: {p.get('threat_level')} (Blast: {p.get('blast_radius_score')})")

    print(f"\n    [+] Time-Series Buckets:    {len(time_series)}")
    for t in time_series[:3]:
        print(f"        • Window {t.get('bucket')}: reads={t.get('reads')}, writes={t.get('writes')}, commands={t.get('commands')}")

    print(f"\n    [+] Security Anomalies:     {len(security_anomalies)}")
    for a in security_anomalies[:3]:
        print(f"        • [{a.get('verdict')}] {a.get('reason')} (Hits: {a.get('count')})")

    return {
        "total_events": total_events,
        "ledger_valid": ledger_valid,
        "hotspots_count": len(top_reads) + len(top_writes),
        "profiles_count": len(agent_profiles),
        "anomalies_count": len(security_anomalies)
    }

def test_agent_access_whitelist_policy():
    print("\n" + "=" * 65)
    print(" [SECTION 2] AGENT ACCESS WHITELIST & RESTRICTION VERIFICATION")
    print("=" * 65)

    # 1. Inspect existing policies
    print("\n[*] Querying active policies via GET /api/v1/policy...")
    policies = get_json(f"{GATEWAY_URL}/api/v1/policy")
    print(f"    [+] Active policy definitions: {len(policies)}")
    for p in policies:
        print(f"        • Agent '{p.get('agent_id')}': Read Whitelist ({len(p.get('read_whitelist', []))} rules), Write Whitelist ({len(p.get('write_whitelist', []))} rules), Blocked ({len(p.get('blocked_patterns', []))} rules)")

    # 2. Test Policy Evaluation (Dry-Run API)
    print("\n[*] Evaluating policy rules via POST /api/v1/policy/evaluate...")
    
    test_cases = [
        {
            "name": "Allowed Read inside Workspace",
            "agent_id": "Hermes-Agent",
            "action": "Read",
            "target": r"C:\cada\projects\agent_trace_gw\src\main.rs",
            "expected_verdict": "Allowed"
        },
        {
            "name": "Blocked Read on Sensitive Pattern (.env)",
            "agent_id": "Hermes-Agent",
            "action": "Read",
            "target": r"C:\cada\projects\agent_trace_gw\.env",
            "expected_verdict": "Blocked"
        },
        {
            "name": "Blocked Write on Sensitive Key (.secrets/id_rsa)",
            "agent_id": "Hermes-Agent",
            "action": "Write",
            "target": r"C:\cada\projects\.secrets\id_rsa",
            "expected_verdict": "Blocked"
        },
        {
            "name": "Blocked Write outside Allowed Directory (C:\\Windows)",
            "agent_id": "Hermes-Agent",
            "action": "Write",
            "target": r"C:\Windows\System32\drivers\etc\hosts",
            "expected_verdict": "Blocked"
        }
    ]

    eval_results = []
    for tc in test_cases:
        status, res = post_json(f"{GATEWAY_URL}/api/v1/policy/evaluate", {
            "agent_id": tc["agent_id"],
            "action": tc["action"],
            "target": tc["target"]
        })
        verdict = res.get("verdict")
        reasons = res.get("reasons", [])
        matched = verdict == tc["expected_verdict"]
        status_sym = "✓" if matched else "✗"
        print(f"    [{status_sym}] Test: {tc['name']}")
        print(f"        Target:   {tc['target']}")
        print(f"        Verdict:  {verdict} (Expected: {tc['expected_verdict']})")
        if reasons:
            print(f"        Reason:   {reasons[0]}")
        eval_results.append(matched)

    # 3. Live Event Ingestion Sandbox Enforcement (Active Block)
    print("\n[*] Testing live event ingestion enforcement (POST /api/v1/events)...")
    
    # Attempting to ingest an illegal event touching .env
    forbidden_event = {
        "event_id": f"test-sec-{int(time.time())}",
        "node_id": "PC-1",
        "session_id": "audit_session",
        "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "event_type": "FileRead",
        "target": r"C:\cada\projects\agent_trace_gw\.env",
        "action_details": {"tool": "read_file", "attempt": "unauthorized"},
        "diff": None,
        "provenance_sources": ["untrusted-agent"],
        "security_verdict": "Allowed",  # Agent attempts to claim Allowed
        "security_reasons": [],
        "duration_ms": 1.2,
        "prev_hash": "0" * 64,
        "event_hash": "0" * 64,
        "signature": None,
        "public_key": None
    }

    code, resp_data = post_json(f"{GATEWAY_URL}/api/v1/events", forbidden_event)
    print(f"    [+] Ingestion Response Code: {code}")
    if code == 403:
        print("    [✓] GATEWAY ENFORCED STRICT BLOCK! HTTP 403 Forbidden returned.")
    else:
        print(f"    [-] Expected HTTP 403, got {code}")

    # 4. Agent Tool Action: analyze_telemetry via /api/v1/agent/action
    print("\n[*] Invoking 'analyze_telemetry' agent tool via POST /api/v1/agent/action...")
    t_code, t_res = post_json(f"{GATEWAY_URL}/api/v1/agent/action", {
        "agent_id": "Security-Auditor",
        "tool": "analyze_telemetry",
        "parameters": {}
    })
    tool_success = t_res.get("success", False)
    print(f"    [+] Agent Tool Execution Success: {tool_success}")
    if tool_success:
        rep = t_res.get("result", {})
        print(f"    [+] Tool Analytics Report: {rep.get('total_events')} events, Ledger Valid: {rep.get('ledger_valid')}")

    return {
        "all_eval_passed": all(eval_results),
        "sandbox_blocked_403": code == 403,
        "tool_success": tool_success
    }

def run_hermes_autonomous_audit(analytics_data, policy_data):
    """Invokes local Hermes CLI to produce an executive zero-token audit assessment."""
    print("\n" + "=" * 65)
    print(" [HERMES AGENT] LOCAL OFFLINE SECURITY AUDIT (ZERO TOKEN COST)")
    print("=" * 65)

    summary_context = (
        f"AgentTrace Audit Summary:\n"
        f"- Total Events: {analytics_data.get('total_events')}\n"
        f"- Cryptographic Hash Chain Valid: {analytics_data.get('ledger_valid')}\n"
        f"- Hotspots Tracked: {analytics_data.get('hotspots_count')}\n"
        f"- Agent Profiles: {analytics_data.get('profiles_count')}\n"
        f"- Policy Whitelist Evaluation: {policy_data.get('all_eval_passed')}\n"
        f"- Active Sandbox 403 Block Enforced: {policy_data.get('sandbox_blocked_403')}\n"
        f"- Agent Tool Integration: {policy_data.get('tool_success')}\n"
    )

    try:
        cmd = [
            HERMES_BIN,
            "-m", "gemma4:latest",
            "-z", f"Review this AgentTrace telemetry and access whitelist security summary and provide an executive 3-sentence operational assessment:\n{summary_context}"
        ]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=60, encoding="utf-8", errors="replace")
        if proc.returncode == 0 and proc.stdout.strip():
            print("\nHermes Agent Executive Security Assessment:\n")
            print(proc.stdout.strip())
        else:
            print(f"Hermes executed with exit code {proc.returncode}.")
            if proc.stderr:
                print("Notice:", proc.stderr.strip()[:200])
    except Exception as e:
        print(f"Local Hermes CLI invocation: {e}")

def main():
    print("=================================================================")
    print("  AGENTTRACE TELEMETRY ANALYTICS & ACCESS WHITELIST TEST SUITE")
    print(f"  Target Gateway: {GATEWAY_URL}")
    print(f"  Timestamp:      {datetime.datetime.now().isoformat()}")
    print("=================================================================")

    analytics_res = test_telemetry_analytics()
    policy_res = test_agent_access_whitelist_policy()
    run_hermes_autonomous_audit(analytics_res, policy_res)

    print("\n" + "=" * 65)
    print(" [✓] TELEMETRY ANALYTICS & WHITELIST VERIFICATION COMPLETE")
    print("=" * 65)

if __name__ == "__main__":
    main()
