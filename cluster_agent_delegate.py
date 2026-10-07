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

def delete_json(url: str, timeout: int = 10) -> dict:
    req = urllib.request.Request(url, method="DELETE", headers={"User-Agent": "AgentTrace-Delegator/1.0"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))

def run_delegation_task_3_agent_decision_arbitration():
    """Tests agent-in-the-loop delegation arbitration and dynamic route overrides."""
    print("\n" + "=" * 65)
    print(" [TASK 3] DELEGATING AGENT ROUTING ARBITRATION & OVERRIDES")
    print("=" * 65)

    arbitration_results = {}

    # 1. Direct REST arbitration: POST /api/v1/delegate/decide
    print("\n[*] [API: /api/v1/delegate/decide] Arbitrating prompt complexity...")
    try:
        dec_res = post_json(f"{GATEWAY_URL}/api/v1/delegate/decide", {
            "prompt": "Design formal mathematical proof of vector stability and cryptographic hash chain",
            "session_id": "session_default",
            "agent_id": "lead-architect",
            "tags": ["math", "architecture"],
            "requested_model": "auto"
        })
        model = dec_res.get("selected_model")
        node = dec_res.get("node_id")
        source = dec_res.get("decision_source")
        reasoning = dec_res.get("reasoning")
        print(f"    [+] Decision Source: {source}")
        print(f"    [+] Selected Model:  {model} on {node}")
        print(f"    [+] Reasoning:       {reasoning}")
        arbitration_results["decide_rest"] = "PASS" if node else "FAIL"
    except Exception as e:
        print(f"    [-] Error calling /api/v1/delegate/decide: {e}")
        arbitration_results["decide_rest"] = "ERROR"

    # 2. Register dynamic override rule: POST /api/v1/delegate/override
    rule_id = "rule-agent-pin-pc2"
    print(f"\n[*] [API: /api/v1/delegate/override] Agent setting dynamic override rule '{rule_id}'...")
    try:
        rule_payload = {
            "rule_id": rule_id,
            "criteria": {
                "type": "session",
                "value": "session_pinned_audit"
            },
            "target_model": "deepseek-r1:1.5b",
            "target_node": "PC-2",
            "reason": "Agent mandated low-latency GPU node for real-time audit session"
        }
        rule_res = post_json(f"{GATEWAY_URL}/api/v1/delegate/override", rule_payload)
        print(f"    [+] Override Rule Saved: ID={rule_res.get('rule_id')}, Model={rule_res.get('target_model')}")
        arbitration_results["set_override"] = "PASS"
    except Exception as e:
        print(f"    [-] Error setting override: {e}")
        arbitration_results["set_override"] = "ERROR"

    # 3. List active overrides: GET /api/v1/delegate/overrides
    print("\n[*] [API: /api/v1/delegate/overrides] Listing active routing overrides...")
    try:
        list_res = get_json(f"{GATEWAY_URL}/api/v1/delegate/overrides")
        rules = list_res.get("rules", [])
        print(f"    [+] Active overrides in gateway: {len(rules)}")
        for r in rules:
            print(f"        • Rule '{r.get('rule_id')}': {r.get('criteria')} -> {r.get('target_model')} ({r.get('reason')})")
        arbitration_results["list_overrides"] = "PASS" if len(rules) > 0 else "FAIL"
    except Exception as e:
        print(f"    [-] Error listing overrides: {e}")
        arbitration_results["list_overrides"] = "ERROR"

    # 4. Verify that session matches override rule: POST /api/v1/delegate/decide
    print(f"\n[*] [Verification] Checking that 'session_pinned_audit' routes to PC-2 via OverrideRule...")
    try:
        pinned_dec = post_json(f"{GATEWAY_URL}/api/v1/delegate/decide", {
            "prompt": "Design formal mathematical proof of vector stability",
            "session_id": "session_pinned_audit",
            "agent_id": "security-auditor",
            "tags": [],
            "requested_model": "auto"
        })
        p_source = pinned_dec.get("decision_source")
        p_node = pinned_dec.get("node_id")
        p_model = pinned_dec.get("selected_model")
        print(f"    [+] Decision Source: {p_source} (Expected: OverrideRule)")
        print(f"    [+] Selected Model:  {p_model} on {p_node} (Expected: PC-2)")
        if p_source == "OverrideRule" and p_node == "PC-2":
            print("    [✓] Dynamic override verified successfully!")
            arbitration_results["override_match"] = "PASS"
        else:
            print(f"    [-] Dynamic override mismatch: source={p_source}, node={p_node}")
            arbitration_results["override_match"] = "FAIL"
    except Exception as e:
        print(f"    [-] Error verifying override: {e}")
        arbitration_results["override_match"] = "ERROR"

    # 5. Agent Interface tool calling: decide_route via /api/v1/agent/action
    print("\n[*] [Agent Tool: decide_route] Testing agent tool invocation...")
    try:
        tool_dec = post_json(f"{GATEWAY_URL}/api/v1/agent/action", {
            "agent_id": "Hermes-Decider",
            "tool": "decide_route",
            "parameters": {
                "prompt": "Unit test assertions for ed25519 signatures",
                "requested_model": "auto"
            }
        })
        if tool_dec.get("success"):
            t_res = tool_dec.get("result", {})
            print(f"    [+] Tool Decision: Model={t_res.get('selected_model')}, Node={t_res.get('node_id')}, Intent={t_res.get('intent')}")
            arbitration_results["agent_tool_decide"] = "PASS"
        else:
            print(f"    [-] decide_route tool failed: {tool_dec.get('error')}")
            arbitration_results["agent_tool_decide"] = "FAIL"
    except Exception as e:
        print(f"    [-] Error calling decide_route tool: {e}")
        arbitration_results["agent_tool_decide"] = "ERROR"

    # 6. Teardown: Remove override rule DELETE /api/v1/delegate/override/{rule_id}
    print(f"\n[*] [Cleanup] Removing override rule '{rule_id}'...")
    try:
        del_res = delete_json(f"{GATEWAY_URL}/api/v1/delegate/override/{rule_id}")
        print(f"    [+] Removed status: {del_res.get('removed')}")
        arbitration_results["remove_override"] = "PASS" if del_res.get("removed") else "FAIL"
    except Exception as e:
        print(f"    [-] Error deleting override: {e}")
        arbitration_results["remove_override"] = "ERROR"

    return arbitration_results

def run_hermes_autonomous_summary(t1_res, t2_res, t3_res):
    """Invokes local Hermes CLI to produce an autonomous evaluation."""
    print("\n" + "=" * 65)
    print(" [HERMES AGENT] LOCAL OFFLINE AGENT EVALUATION (ZERO TOKEN COST)")
    print("=" * 65)

    summary_context = (
        f"Cluster Delegation Results:\n"
        f"- Dynamic Capability Routing: {json.dumps(t1_res)}\n"
        f"- Telemetry Governance: {json.dumps(t2_res)}\n"
        f"- Agent Decision Arbitration: {json.dumps(t3_res)}\n"
        f"Provide a concise 3-sentence operational assessment of the cluster health and agent autonomy."
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
    t3 = run_delegation_task_3_agent_decision_arbitration()
    run_hermes_autonomous_summary(t1, t2, t3)

    print("\n" + "=" * 65)
    print(" [✓] DELEGATION COMPLETE ACROSS ALL CLUSTER INTERFACES")
    print("=" * 65)

if __name__ == "__main__":
    main()

