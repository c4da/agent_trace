import urllib.request
import json
import sys

prompt = """
Write the HTML and JavaScript for the AgentTrace Gateway Governance Setup modal.
The modal ID is 'governance-modal'.

It has 3 tabs:
1. tab-policies:
   - table showing policies fetched from GET /api/v1/policy
   - form with inputs: agent_id, enforcing (checkbox), read_whitelist, write_whitelist, network_whitelist, blocked_patterns, and a 'Save Policy' button calling POST /api/v1/policy
   - delete button calling DELETE /api/v1/policy/:agent_id
2. tab-simulator:
   - inputs: agent_id, action (Read/Write/Execute/NetworkPull), target
   - button calling POST /api/v1/policy/evaluate
   - div showing verdict (Allowed/Blocked/Flagged) and reason
3. tab-overrides:
   - table of overrides from GET /api/v1/delegate/overrides
   - form to add override: criteria_type, criteria_value, target_model, target_node, reason
   - button calling POST /api/v1/delegate/override
   - delete button calling DELETE /api/v1/delegate/override/:rule_id
   - prompt routing test calling POST /api/v1/delegate/decide

Output clean HTML and JS functions:
openGovernanceModal(), closeGovernanceModal(), switchGovTab(tabId), loadGovPolicies(), saveGovPolicy(), deleteGovPolicy(agentId), evaluateGovPolicy(), loadGovOverrides(), saveGovOverride(), deleteGovOverride(ruleId), simulateGovRoute().
"""

payload = {
    "model": "deepseek-r1:1.5b",
    "prompt": prompt,
    "stream": True,
    "options": {
        "temperature": 0.2,
        "num_predict": 2048
    }
}

print("Streaming governance UI generation from DeepSeek R1 1.5B (CUDA / Zero External Tokens)...")
req = urllib.request.Request(
    "http://127.0.0.1:11434/api/generate",
    data=json.dumps(payload).encode("utf-8"),
    headers={"Content-Type": "application/json"}
)

full_resp = []
try:
    with urllib.request.urlopen(req, timeout=120) as resp:
        for line in resp:
            if not line:
                continue
            chunk = json.loads(line.decode("utf-8"))
            token = chunk.get("response", "")
            full_resp.append(token)
            if chunk.get("done", False):
                break
    
    output = "".join(full_resp)
    with open("governance_ui_generated.html", "w", encoding="utf-8") as f:
        f.write(output)
    print(f"Generated {len(output)} bytes, saved to governance_ui_generated.html")
except Exception as e:
    print(f"Error: {e}", file=sys.stderr)
    sys.exit(1)
