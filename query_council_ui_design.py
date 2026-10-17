import json
import urllib.request
import sys

if sys.platform == "win32":
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

def query_ollama(model: str, system_prompt: str, user_prompt: str) -> str:
    url = "http://127.0.0.1:11434/api/chat"
    payload = {
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ],
        "stream": False,
        "options": {
            "temperature": 0.3,
            "num_predict": 1200
        }
    }
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            res = json.loads(resp.read().decode("utf-8"))
            return res.get("message", {}).get("content", "")
    except Exception as e:
        return f"Error querying {model}: {e}"

qwen_prompt = """You are a Principal DevOps & Systems Engineer designing the operational console for AgentTrace Gateway.
The user feedback on our current web UI: 'The current one has fancy colors and boxes, but it's not made for work. We want a new UI that looks like a terminal, practical, high density, built for real everyday workflows.'

Provide your architectural design for a Terminal-style Operational Console (Web TUI):
1. Key User Scenarios & Problems Solved:
   - What questions do developers/operators actually ask when autonomous agents (Copilot CLI, Hermes, Claude Code, Antigravity) run on their machines? (e.g., Did it touch .env? What bash commands ran? Why did routing pick model X? Show me the git diff.)
2. Practical Layout & Panes:
   - High information density (monochrome/amber/green phosphor accents, dense monospace tables, zero wasted padding).
   - Top status bar (gateway health, ledger blocks, active watchers).
   - Main split view: Live Event Stream / Tail + Side Inspector (inspect payload, diff, signatures).
3. Command Bar / Keyboard Ergonomics:
   - Vim/htop-like shortcuts (j/k navigation, / for filter, :verify, :tail, :diff).
"""

deepseek_prompt = """You are a Principal Security Auditor & Cryptographic Engineer designing a Terminal-style security console for AgentTrace Gateway.
The goal is to design an ultra-practical, high-density terminal interface (Web TUI) that solves real developer & security operator scenarios.

Provide your security & audit perspective:
1. Critical Investigative Scenarios:
   - Detecting malicious/rogue agent actions (file exfiltration, network calls, destructive commands).
   - Verifying cryptographic ledger integrity (tamper detection, broken block localization).
   - Policy sandboxing & live threat scoring (monitoring blast radius).
2. Essential Terminal Features & Interactive Widgets:
   - Unified Diff Viewer pane for file modifications.
   - Live Filter syntax (e.g. `type:FileWrite target:*.env`, `verdict:Blocked`).
   - One-keystroke ledger verification (`V` or `:verify`).
"""

def main():
    print("=== Querying Qwen (DevOps / Systems Engineer Perspective) ===")
    qwen_res = query_ollama(
        "qwen3.5:9b",
        "You are an expert Systems Architect specializing in Terminal UIs (TUI), operator ergonomics, and developer tools.",
        qwen_prompt
    )
    print(qwen_res)
    print("\n" + "="*60 + "\n")

    print("=== Querying DeepSeek-R1 (Security Auditor & Cryptography Perspective) ===")
    deepseek_res = query_ollama(
        "deepseek-r1:1.5b",
        "You are an expert Security Engineer and Cryptographic Auditor specializing in security operations centers (SOC) and terminal forensics.",
        deepseek_prompt
    )
    print(deepseek_res)

    with open("docs/UI_COUNCIL_REDESIGN_PROPOSALS.md", "w", encoding="utf-8") as f:
        f.write("# AgentTrace Terminal Console UI: Multi-Model Architectural Council Proposals\n\n")
        f.write("## 1. Qwen 3.5 (Systems & Operational Ergonomics Perspective)\n\n")
        f.write(qwen_res + "\n\n---\n\n")
        f.write("## 2. DeepSeek-R1 (Security Forensics & Cryptographic Audit Perspective)\n\n")
        f.write(deepseek_res + "\n")

    print("\nSaved design council proposals to docs/UI_COUNCIL_REDESIGN_PROPOSALS.md")

if __name__ == "__main__":
    main()
