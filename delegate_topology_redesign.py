import urllib.request
import json
import sys

prompt = """
You are a senior distributed systems architect and UI engineer.
The current AgentTrace Gateway dashboard displays topology as two side-by-side cards (PC-1 and PC-2) with a tiny network bridge, where the Gateway is wrongly buried inside PC-1 as a sub-item, and PC-2 merely mentions 'Worker Telemetry -> Central Gateway (:8080)'. The operator correctly pointed out that the Gateway is NOT visually shown as the central interconnect, making the architecture misleading.

Redesign the Distributed Cluster Topology section for `dashboard.html`.

Architecture facts:
1. PC-1 (Lead Node · LEAD-NODE · 127.0.0.1):
   - PC-1 Antigravity Lead Agent (Cloud Gemini Pro RPC, System Architect & Orchestrator)
   - PC-1 Local Engines (Ollama :11434 with qwen3.8:27b, gemma4:latest, qwen3:14b)
2. AgentTrace Gateway & Cryptographic Ledger (:8080) - CENTRAL HUB:
   - Sits prominently in the center connecting PC-1 and PC-2!
   - Acts as Central Reverse Proxy & Smart Router (/v1/chat/completions)
   - Runs Autonomous Agent Sandbox Governance & Whitelisting
   - Maintains Ed25519 Tamper-Proof Cryptographic Hash Chain Ledger
   - Has live status, metrics, and quick action buttons
3. PC-2 (Worker Node · WORKER-NODE · 127.0.0.1):
   - Dedicated NVIDIA GPU CUDA compute node
   - Runs Hermes Agent v0.21.5 on port :8642
   - Primary model: deepseek-r1:1.5b (CUDA accelerated, fast CoT reasoning, syntax checks, test assertions at zero token cost)
   - Worker Telemetry Forwarder streaming signed blocks back to Gateway :8080

Provide:
1. A complete Mermaid flowchart diagram string (`graph LR` or `flowchart LR`) showing the exact data flows between PC-1, the Central Gateway, and PC-2.
2. The HTML and CSS for the 3-column / Hub-and-Spoke layout where:
   - Left is PC-1 (Lead Node)
   - Center is AgentTrace Gateway (:8080) (Central Hub & Interconnect)
   - Right is PC-2 (Worker Node with NVIDIA CUDA & deepseek-r1:1.5b)
   with animated interconnect pulses, model tags, and capabilities badges.
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

print("Querying DeepSeek R1 (100% GPU CUDA) to redesign cluster topology layout...")
req = urllib.request.Request(
    "http://127.0.0.1:11434/api/generate",
    data=json.dumps(payload).encode("utf-8"),
    headers={"Content-Type": "application/json"}
)

chunks = []
try:
    with urllib.request.urlopen(req, timeout=120) as resp:
        for line in resp:
            if not line:
                continue
            data = json.loads(line.decode("utf-8"))
            tok = data.get("response", "")
            chunks.append(tok)
            if data.get("done", False):
                break
    result = "".join(chunks)
    with open("topology_redesign_output.md", "w", encoding="utf-8") as f:
        f.write(result)
    print(f"DeepSeek R1 response saved ({len(result)} bytes).")
except Exception as e:
    print(f"Error: {e}", file=sys.stderr)
    sys.exit(1)
