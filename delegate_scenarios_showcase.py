import json
import urllib.request
import sys
import os

if sys.platform == "win32":
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

def query_ollama(model: str, system_prompt: str, user_prompt: str, max_tokens: int = 4000) -> str:
    url = "http://127.0.0.1:11434/api/chat"
    payload = {
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ],
        "stream": False,
        "options": {
            "temperature": 0.2,
            "num_predict": max_tokens
        }
    }
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=180) as resp:
            res = json.loads(resp.read().decode("utf-8"))
            msg = res.get("message", {})
            return msg.get("content", "")
    except Exception as e:
        return f"Error querying {model}: {e}"

def unload_model(model: str):
    try:
        url = "http://127.0.0.1:11434/api/generate"
        payload = {"model": model, "keep_alive": 0}
        req = urllib.request.Request(url, data=json.dumps(payload).encode("utf-8"), headers={"Content-Type": "application/json"})
        urllib.request.urlopen(req, timeout=10)
    except Exception:
        pass

prompt = """Write a compelling, professional, and practical Markdown document titled:
# Why AgentTrace: Real-World Scenarios, Core Value Proposition & UI Demonstrations

The document must clearly articulate:
1. The Core Vision:
   - Why was AgentTrace built?
   - What problems are we solving?
   - Why does a developer or enterprise team NEED it when deploying autonomous AI coding agents (Copilot CLI, Hermes Agent, Antigravity, Claude Code)?
2. The 5 Core Selling Scenarios:
   - For EACH scenario, detail:
     a) The Real-World Danger / Pain Point (e.g. autonomous agent running unattended, reading .env, making unreviewed code writes, burning expensive API tokens).
     b) Why standard tools fail (git status doesn't show in-flight edits, standard loggers can be altered, proxy without policy allows leaks).
     c) The AgentTrace Solution (cryptographic ledger, policy engine, zero-token router).
     d) The UI Demonstration (how the developer can click or press a key in the Terminal Console or Visual Dashboard to see the exact solution live).
   - The 5 Scenarios:
     1. Secret & Sensitive Path Intrusion (.env, id_rsa, .aws credentials)
     2. In-Flight Code Forensics & Unified Diff Viewer
     3. Zero-Token Model Interception & Cloud Cost Leakage Prevention
     4. 1-Keystroke Cryptographic Proof of Non-Tampering (11,600+ block hash chain)
     5. Instant Runtime Agent Freezing & Sandbox Lockdown
3. Educational Rust Concepts (for a developer learning Rust):
   - How policy evaluation works without locks (`Arc<RwLock<PolicyEngine>>`).
   - How event hashing and Ed25519 signatures guarantee ledger immutability.
   - Clean, notebook-friendly code structure.

Keep the language practical, sharp, and engineering-focused (no corporate marketing fluff).
"""

def main():
    model = "qwen3.5:9b"
    print(f"Querying {model} to write Why AgentTrace Scenarios & Value Proposition...")
    content = query_ollama(
        model,
        "You are a Lead Software Architect and Security Engineer explaining why developer security tools succeed and teaching Rust systems design.",
        prompt,
        max_tokens=4000
    )
    unload_model(model)
    
    if not content:
        print("Failed to get content from Qwen.")
        sys.exit(1)
        
    out_path = os.path.join("docs", "WHY_AGENTTRACE_SCENARIOS_AND_VALUE_PROPOSITION.md")
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(content)
        
    print(f"Successfully generated {out_path} ({len(content)} bytes).")

if __name__ == "__main__":
    main()
