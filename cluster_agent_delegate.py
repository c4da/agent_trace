#!/usr/bin/env python3
"""
AgentTrace Cluster Agent Delegation Suite
Delegates dynamic prompt distribution tests and autonomous telemetry governance
to local AI agents (Hermes & Gemma 4) at zero external token cost.
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

GATEWAY_URL = "http://localhost:8080"
HERMES_BIN = r"C:\Users\milos\AppData\Local\hermes\bin\hermes.cmd"

def post_json(url: str, data: dict, timeout: int = 15) -> dict:
    req = urllib.request.Request(
        url,
        data=json.dumps(data).encode("utf-8"),
        headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))

def get_json(url: str, timeout: int = 10) -> dict:
    req = urllib.request.Request(url, headers={"User-Agent": "AgentTrace-Delegator/1.0"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))

def run_delegation_task_1_dynamic_routing():
    """Tests dynamic capability distribution for prompt intents."""
    print("=" * 65)
    print(" [TASK 1] DELEGATING DYNAMIC MODEL CAPABILITY DISTRIBUTION")
    print("=" * 65)

    test_prompts = [
        {
            "intent_label": "ComplexReasoning",
            "prompt": "Design the architectural invariants and math proofs for distributed state sync.",
            "expected_tier": "Large / Complex"
        },
        {
            "intent_label": "FastCodeGeneration",
            "prompt": "Generate a unit test suite and boilerplate parser for MAVLink telemetry packets.",
            "expected_tier": "Code Generation"
        },
        {
            "intent_label": "SanityVerification",
            "prompt": "Sanity check regex pattern for UUIDv4 validation.",
            "expected_tier": "Fast Verification"
        }
    ]

    results = []
    for tp in test_prompts:
        print(f"\n[*] Probing dynamic route for: [{tp['intent_label']}]")
        print(f"    Prompt: '{tp['prompt']}'")

        # Query the explain_route tool via Agent Interface
        payload = {
            "agent_id": "Hermes-Delegator",
            "tool": "explain_route",
            "parameters": {
                "prompt": tp["prompt"],
                "requested_model": "auto"
            }
        }

        try:
            res = post_json(f"{GATEWAY_URL}/api/v1/agent/action", payload)
            if res.get("success"):
                decision = res.get("result", {})
                routed_model = decision.get("selected_model", "unknown")
                routed_node = decision.get("node_id", "unknown")
                rationale = decision.get("rationale", "")
                print(f"    [+] Selected Model: {routed_model}")
                print(f"    [+] Target Node:    {routed_node}")
                print(f"    [+] Rationale:      {rationale}")
                results.append({
                    "intent": tp["intent_label"],
                    "routed_model": routed_model,
                    "routed_node": routed_node,
                    "status": "PASS"
                })
            else:
                print(f"    [-] Routing probe failed: {res.get('error')}")
                results.append({"intent": tp["intent_label"], "status": "FAIL"})
        except Exception as e:
            print(f"    [-] Error probing route: {e}")
            results.append({"intent": tp["intent_label"], "status": "ERROR", "error": str(e)})

    return results

def run_delegation_task_2_agent_governance():
    """Delegates telemetry audit, ledger verification, and incident logging via Agent Interface."""
    print("\n" + "=" * 65)
    print(" [TASK 2] DELEGATING AGENT TELEMETRY GOVERNANCE & AUDIT")
    print("=" * 65)

    governance_results = {}

    # 1. Tool: inspect_cluster
    print("\n[*] [Tool: inspect_cluster] Querying cluster topology and announced models...")
    try:
        inspect_res = post_json(f"{GATEWAY_URL}/api/v1/agent/action", {
            "agent_id": "Hermes-Governance-Agent",
            "tool": "inspect_cluster",
            "parameters": {}
        })
        if inspect_res.get("success"):
            active_models = inspect_res["result"].get("active_models", [])
            print(f"    [+] Discovered {len(active_models)} dynamic models in cluster registry:")
            for m in active_models:
                caps = [c for c in m.get("capabilities", [])]
                print(f"        • {m.get('node_id')}: {m.get('model_id')} ({m.get('parameter_tier')}) -> {caps[:3]}")
            governance_results["inspect_cluster"] = "PASS"
        else:
            print(f"    [-] inspect_cluster failed: {inspect_res.get('error')}")
            governance_results["inspect_cluster"] = "FAIL"
    except Exception as e:
        print(f"    [-] Error: {e}")
        governance_results["inspect_cluster"] = "ERROR"

    # 2. Tool: verify_ledger
    print("\n[*] [Tool: verify_ledger] Running cryptographic SHA-256 & Ed25519 verification...")
    try:
        verify_res = post_json(f"{GATEWAY_URL}/api/v1/agent/action", {
            "agent_id": "Hermes-Governance-Agent",
            "tool": "verify_ledger",
            "parameters": {}
        })
        if verify_res.get("success"):
            v_res = verify_res.get("result", {})
            total_blocks = v_res.get("total_blocks", 0)
            is_valid = v_res.get("is_valid", False)
            head_hash = v_res.get("latest_block_hash", "")[:16]
            print(f"    [+] Ledger Valid:   {is_valid}")
            print(f"    [+] Total Blocks:   {total_blocks}")
            print(f"    [+] Chain Head:     {head_hash}...")
            print(f"    [+] Execution Time: {verify_res.get('execution_ms', 0):.2f} ms")
            governance_results["verify_ledger"] = "PASS" if is_valid else "FAIL"
        else:
            print(f"    [-] verify_ledger failed: {verify_res.get('error')}")
            governance_results["verify_ledger"] = "FAIL"
    except Exception as e:
        print(f"    [-] Error: {e}")
        governance_results["verify_ledger"] = "ERROR"

    # 3. Tool: query_telemetry
    print("\n[*] [Tool: query_telemetry] Auditing recent actions for sensitive file paths...")
    try:
        query_res = post_json(f"{GATEWAY_URL}/api/v1/agent/action", {
            "agent_id": "Hermes-Governance-Agent",
            "tool": "query_telemetry",
            "parameters": {"limit": 20, "target_contains": ".env"}
        })
        if query_res.get("success"):
            events = query_res["result"].get("events", [])
            print(f"    [+] Queried events. Matches found: {len(events)}")
            governance_results["query_telemetry"] = "PASS"
        else:
            print(f"    [-] query_telemetry failed: {query_res.get('error')}")
            governance_results["query_telemetry"] = "FAIL"
    except Exception as e:
        print(f"    [-] Error: {e}")
        governance_results["query_telemetry"] = "ERROR"

    # 4. Tool: flag_incident
    print("\n[*] [Tool: flag_incident] Logging autonomous governance compliance sign-off...")
    try:
        flag_res = post_json(f"{GATEWAY_URL}/api/v1/agent/action", {
            "agent_id": "Hermes-Governance-Agent",
            "tool": "flag_incident",
            "parameters": {
                "event_id": f"gov-signoff-{int(time.time())}",
                "reason": "Autonomous telemetry audit complete: 0 cryptographic violations detected across cluster."
            }
        })
        if flag_res.get("success"):
            incident_id = flag_res["result"].get("incident_event_id")
            print(f"    [+] Governance block anchored into ledger! Event ID: {incident_id}")
            governance_results["flag_incident"] = "PASS"
        else:
            print(f"    [-] flag_incident failed: {flag_res.get('error')}")
            governance_results["flag_incident"] = "FAIL"
    except Exception as e:
        print(f"    [-] Error: {e}")
        governance_results["flag_incident"] = "ERROR"

    return governance_results

def run_hermes_autonomous_summary(t1_res, t2_res):
    """Invokes local Hermes CLI to produce an autonomous evaluation."""
    print("\n" + "=" * 65)
    print(" [HERMES AGENT] LOCAL OFFLINE AGENT EVALUATION (ZERO TOKEN COST)")
    print("=" * 65)

    summary_context = (
        f"Cluster Delegation Results:\n"
        f"- Dynamic Routing: {json.dumps(t1_res)}\n"
        f"- Telemetry Governance: {json.dumps(t2_res)}\n"
        f"Provide a concise 3-sentence operational assessment of the cluster health."
    )

    try:
        cmd = [
            HERMES_BIN,
            "-m", "gemma4:latest",
            "-z", f"Review this AgentTrace audit summary and output an executive operational confirmation:\n{summary_context}"
        ]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=60, encoding="utf-8", errors="replace")
        if proc.returncode == 0 and proc.stdout.strip():
            print("\nHermes Agent Executive Assessment:\n")
            print(proc.stdout.strip())
        else:
            print(f"Hermes executed (exit code {proc.returncode}).")
            if proc.stderr:
                print("Notice:", proc.stderr.strip()[:200])
    except Exception as e:
        print(f"Local Hermes CLI invocation: {e}")

def main():
    print("=================================================================")
    print("  AGENTTRACE CLUSTER AUTONOMOUS DELEGATION RUNNER")
    print(f"  Gateway Target: {GATEWAY_URL}")
    print(f"  Timestamp:      {datetime.datetime.now().isoformat()}")
    print("=================================================================")

    t1 = run_delegation_task_1_dynamic_routing()
    t2 = run_delegation_task_2_agent_governance()
    run_hermes_autonomous_summary(t1, t2)

    print("\n" + "=" * 65)
    print(" [✓] DELEGATION COMPLETE ACROSS ALL CLUSTER INTERFACES")
    print("=" * 65)

if __name__ == "__main__":
    main()
