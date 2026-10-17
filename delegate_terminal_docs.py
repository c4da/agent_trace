import json
import urllib.request
import sys
import os

if sys.platform == "win32":
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

def query_ollama(model: str, system_prompt: str, user_prompt: str, max_tokens: int = 3500) -> str:
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
            content = msg.get("content", "")
            if not content and msg.get("thinking"):
                print("[Note: thinking was present, length:", len(msg.get("thinking")), "]")
            return content
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

prompt = """Write a practical, clear markdown engineering guide and developer tutorial titled:
# Practical Agent Observability: Real-World Scenarios and Rust Architecture for AgentTrace Terminal Console

The user wants two things:
1. Practical real-world scenarios: What questions do developers and security operators actually ask when running autonomous AI coding agents (Copilot CLI, Hermes, Antigravity, Claude Code), and how does the terminal console UI solve them directly?
2. Educational Rust concepts: Teach a developer learning Rust how this works under the hood. Specifically:
   - Compile-time asset embedding with `include_str!` vs runtime file I/O.
   - Axum route handlers returning `Html<&'static str>`.
   - Real-time Server-Sent Events (SSE) using Tokio broadcast channels (`tokio::sync::broadcast`).
   - Clean, short functions adhering to Single Responsibility and <= 3 arguments.

Requirements:
- Practical, short explanations, no fancy fluff.
- Include concrete Rust code snippets showing Axum handlers and SSE stream adapters.
- Detail the 5 core operator scenarios:
  1. Secret & sensitive path intrusion detection (.env, id_rsa).
  2. Unified diff forensics (reviewing agent file edits in real-time).
  3. Zero-token routing & model interception audit.
  4. Cryptographic ledger integrity proof (:verify, SHA-256 hash chains).
  5. Runtime agent sandboxing & policy control.
"""

def main():
    model = "qwen3.5:9b"
    print(f"Querying {model} to author educational Rust guide...")
    content = query_ollama(
        model,
        "You are a Senior Rust Systems Architect and Teacher writing clear, practical, notebook-friendly educational documentation.",
        prompt,
        max_tokens=4000
    )
    unload_model(model)
    
    if not content:
        print("Failed to get content from Qwen.")
        sys.exit(1)
        
    out_path = os.path.join("docs", "TERMINAL_CONSOLE_DESIGN_AND_SCENARIOS.md")
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(content)
        
    print(f"Successfully generated {out_path} ({len(content)} bytes).")

if __name__ == "__main__":
    main()
