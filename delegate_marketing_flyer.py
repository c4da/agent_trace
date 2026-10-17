import urllib.request
import json
import sys
import os
import time

sys.stdout.reconfigure(encoding='utf-8')

GATEWAY_URL = "http://localhost:8080/v1/chat/completions"
MODEL = "gemma4:latest"

prompt = """You are an elite developer-marketing copywriter and systems architect for high-performance AI infrastructure.

Create a complete, stunning, ready-to-print Marketing Flyer / Product One-Pager in GitHub-flavored Markdown for:

# AgentTrace Gateway
**Subtitle: The Zero-Token Autonomous Agent Governance & Cryptographic Ledger Swarm**

### Target Audience:
AI Infrastructure Engineers, Enterprise CISOs, Agentic System Builders, and Autonomous Cluster Developers.

### Include the Following Detailed Sections:

1. **HERO SECTION**:
   - Punchy, high-impact headline & sub-headline.
   - 2-sentence executive summary explaining how AgentTrace Gateway stops cloud token waste while enforcing cryptographic accountability.

2. **THE 3 FATAL DILEMMAS OF MODERN AGENT SWARMS**:
   - The Token Hemorrhage (uncontrolled API billing on routine tasks).
   - The Rogue Execution Threat (arbitrary file modification, credential theft, zero blast radius containment).
   - The Black Box Audit Deficit (no cryptographic non-repudiation of agent actions).

3. **THE SOLUTION: 4 ARCHITECTURAL PILLARS**:
   - **Pillar 1: Zero-Token Local Cluster Dispatch**: Autonomous intent routing to PC-1 (Qwen 27B / Gemma 4) and PC-2 CUDA (DeepSeek R1), slashing API costs by 90%+.
   - **Pillar 2: Ed25519 Cryptographic Telemetry Ledger**: SHA-256 tamper-evident hash chaining with instant Merkle root verification (`/api/v1/ledger/verify`).
   - **Pillar 3: Active Sandbox Governance & Blast Radius Scoring**: Real-time permission engine protecting sensitive files (`.env`, credentials) and scoring containment.
   - **Pillar 4: Universal Drop-in OpenAI & MCP Reverse Proxy**: Zero client refactoring—instant integration with Antigravity, Cursor, Claude Code, and custom SDKs.

4. **CLUSTER TOPOLOGY SNAPSHOT**:
   - Clean ASCII diagram showing:
     [Clients: IDE / Antigravity / MCP] 
              │
              ▼
     [AgentTrace Gateway :8080] (Governance + Ledger + Router)
         ├───▶ [PC-1 Lead Node :11434] (Heavy Reasoning & Math)
         └───▶ [PC-2 Worker Node :8642] (NVIDIA CUDA Fast Verification)

5. **BY THE NUMBERS (BENCHMARKS)**:
   - Provide a clean markdown table showing: Metric, AgentTrace Gateway, Traditional Cloud Setup.
   - (Include: Token Cost, Audit Proof, Blast Radius Control, Proxy Latency).

6. **60-SECOND QUICKSTART**:
   - Exact CLI commands to launch the lead node and connect a worker.

7. **CALL TO ACTION**:
   - Inspiring closing message and links to Dashboard (`http://localhost:8080/dashboard`).

Write the complete markdown flyer now. Make it authoritative, captivating, technically precise, and formatted beautifully.
"""

payload = {
    "model": MODEL,
    "messages": [
        {
            "role": "system",
            "content": "You are a master technical marketing strategist. Write an authoritative, inspiring, and polished developer flyer."
        },
        {
            "role": "user",
            "content": prompt
        }
    ],
    "max_tokens": 1600,
    "temperature": 0.7
}

print(f"[*] Delegating Marketing Flyer creation to {MODEL} via AgentTrace Gateway ({GATEWAY_URL})...")
t0 = time.time()

req = urllib.request.Request(
    GATEWAY_URL,
    data=json.dumps(payload).encode("utf-8"),
    headers={"Content-Type": "application/json"}
)

try:
    with urllib.request.urlopen(req, timeout=240) as resp:
        res = json.loads(resp.read().decode("utf-8"))
        content = res["choices"][0]["message"]["content"]
        elapsed = time.time() - t0
        
        print(f"[+] Successfully received marketing flyer from {MODEL} in {elapsed:.2f}s!")
        
        os.makedirs("docs", exist_ok=True)
        flyer_path = os.path.join("docs", "MARKETING_FLYER.md")
        with open(flyer_path, "w", encoding="utf-8") as f:
            f.write(content)
        print(f"[+] Saved flyer markdown to: {flyer_path}")
        
        print("\n--- FLYER HEADLINE PREVIEW ---")
        lines = [l for l in content.splitlines() if l.strip()]
        for l in lines[:10]:
            print(l)

except Exception as e:
    print(f"[-] Error delegating to gateway: {e}")
    sys.exit(1)
