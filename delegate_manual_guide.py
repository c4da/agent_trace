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

prompt = """Write a practical engineering and operator guide titled:
# Manual-First Observability: Why Human Investigation Precedes AI Log Analysis in AgentTrace

The user's key insight:
'Sure there is an agent API that can analyze logs, but first a user would want to do things manually.'

Structure the guide into 3 core sections:
1. The 'Manual-First' Philosophy:
   - Why developers and security operators distrust blind AI self-policing (agents auditing agents creates a circular trust problem).
   - The necessity of raw human-in-the-loop manual verification: seeing the exact diff, verifying the hash chain with one's own eyes, testing policy rules interactively.
2. The 5 Core Manual Operator Workflows in AgentTrace:
   - Workflow 1: Manual Live Grep & Compound Querying (filtering by agent, verdict, target without writing code).
   - Workflow 2: Manual In-Flight Diff Inspection (scrubbing line additions and deletions before git commit).
   - Workflow 3: Interactive Policy Testing & Sandboxing (manually testing 'What if an agent touches path X?' via the UI evaluation form before launching the agent).
   - Workflow 4: Manual Flagging & Incident Bookmarking (pinning suspicious events during a live session for post-mortem review).
   - Workflow 5: Manual Cryptographic Audit & JSON/CSV Export (running one-keystroke ledger proof and exporting raw evidence).
3. Educational Rust Implementation Patterns:
   - How `PolicyEngine::evaluate` performs zero-lock pattern matching in Rust using `glob` / regex without locking writer threads.
   - How `tokio::sync::broadcast` enables pause/resume streams without buffering indefinitely on the server.
   - Clean, notebook-friendly, short Rust methods.

Practical, crisp, developer-friendly style. No corporate fluff.
"""

def main():
    model = "qwen3.5:9b"
    print(f"Querying {model} to write Manual-First Operator Guide...")
    content = query_ollama(
        model,
        "You are a Senior Systems Architect and Security Engineer explaining operator ergonomics and Rust systems.",
        prompt,
        max_tokens=4000
    )
    unload_model(model)
    
    if not content:
        print("Failed to get content from Qwen.")
        sys.exit(1)
        
    out_path = os.path.join("docs", "MANUAL_FIRST_OPERATOR_GUIDE.md")
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(content)
        
    print(f"Successfully generated {out_path} ({len(content)} bytes).")

if __name__ == "__main__":
    main()
