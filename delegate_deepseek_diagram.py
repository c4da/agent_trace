import urllib.request
import json
import sys

sys.stdout.reconfigure(encoding='utf-8')

prompt = """You are an expert software visualization architect specializing in Mermaid.js diagrams.
We have an application called AgentTrace Gateway with this exact architecture:

Components:
1. Clients:
   - Antigravity IDE / Lead Architect
   - Hermes Agent
   - External MCP Clients
2. AgentTrace Gateway (:8080):
   - Axum HTTP & SSE Router
   - OpenAI Proxy (/v1/chat/completions)
   - Smart Auto-Router (intent-based)
   - Dynamic Delegation & Override Engine
   - Sandbox Policy Engine (Read/Write/Network whitelists, .env blacklist)
   - Agent Governance & MCP Control Plane
   - Deep Telemetry Analytics
3. Backend Storage:
   - Ed25519 Cryptographic Signer & V2 Hasher
   - SQLite WAL Ledger (cluster_telemetry.db)
4. Cluster Compute Nodes:
   - PC-1 Lead (127.0.0.1) with qwen3.8:27b, gemma4:latest, qwen3:14b
   - PC-2 Worker (127.0.0.1) with deepseek-r1:1.5b, hermes-agent (CUDA)

CRITICAL DIAGRAMMING RULES FOR MERMAID:
- Avoid deeply nested subgraphs! Nested subgraphs cause Mermaid's Dagre layout engine to glitch, stretching boxes and drawing chaotic crisscrossing lines.
- The diagram must render cleanly, beautifully, and symmetrically.
- Use `flowchart LR` (Left to Right) OR a clean un-nested `flowchart TD` (Top to Bottom).
- Show the two main data flows cleanly:
  Flow 1 (Inference): Client -> Proxy -> Router/Delegation -> Compute Nodes (PC-1 / PC-2)
  Flow 2 (Telemetry & Governance): Client -> Policy Engine -> Ed25519 Signer -> SQLite WAL Ledger
- Apply stylish modern CSS classes with clear node shapes and badges.
- Output ONLY the Mermaid diagram code inside a ```mermaid ... ``` code block.
"""

payload = {
    "model": "deepseek-r1:1.5b",
    "messages": [
        {"role": "system", "content": "You are a master Mermaid.js architect. Output only clean, gorgeous, non-crossing Mermaid diagrams."},
        {"role": "user", "content": prompt}
    ],
    "stream": False
}

req = urllib.request.Request(
    "http://127.0.0.1:11434/api/chat",
    data=json.dumps(payload).encode("utf-8"),
    headers={"Content-Type": "application/json"}
)

print("[*] Delegating diagram design to deepseek-r1:1.5b on PC-1...")
try:
    with urllib.request.urlopen(req, timeout=120) as resp:
        data = json.loads(resp.read().decode("utf-8"))
        msg = data.get("message", {})
        thinking = msg.get("thinking", "")
        content = msg.get("content", "")
        print("\n=== DEEPSEEK THINKING ===")
        print(thinking[:500] if thinking else "No CoT")
        print("\n=== DEEPSEEK GENERATED MERMAID DIAGRAM ===")
        print(content)
        
        # Save to file
        with open("deepseek_diagram.md", "w", encoding="utf-8") as f:
            f.write(content)
except Exception as e:
    print(f"[-] Error calling deepseek: {e}")
