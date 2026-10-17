import json
import urllib.request
import subprocess
import os
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent

PROMPT = """You are an expert AI systems architect analyzing the AgentTrace Gateway project.

Context & Current Architecture:
1. Core Functionality:
   - Reverse proxy & dynamic intent router (OpenAI-compatible /v1/chat/completions).
   - Cluster topology: PC-1 Lead Node (Antigravity, Copilot CLI, Local Ollama Qwen 27B/14B), PC-2 Worker Node (Hermes deepseek-r1:1.5b CUDA), Central Gateway (:8080).
   - Dynamic Model Registry matching model capabilities (ComplexReasoning, FastCodeGeneration, SanityVerification, etc.).
   - AI Agent Sandbox Governance: Path whitelists, network boundaries, strict blacklist blocking (.env, .secrets, credentials).
   - Cryptographic Ledger: Canonical V2 SHA-256 hash chains, Ed25519 digital signatures, SQLite WAL mode, tamper detection.
   - Telemetry Ingestion: Native Antigravity IDE watcher, Native GitHub Copilot CLI session watcher (~/.copilot/session-state/*/events.jsonl), LAN UDP beacon.
   - Comprehensive In-Memory HTTP Integration Test Suite (tests/test_http_endpoints.rs with Tower and Mock HTTP bodies, 66 tests passing).
   - Dashboard UI: Real-time SSE streaming, SVG animated architecture diagram, Mermaid flowchart, governance modal.

2. Quality Fixes (PR #1) recently merged:
   - Refactored 13-parameter hash function into zero-copy, lifetime-bounded RawEventFields struct.
   - Resolved all Clippy linter warnings.
   - Added full HTTP integration tests.

Question:
As an autonomous AI agent in this ecosystem, what should the team focus on next?
Provide your prioritized technical opinion with 3 to 5 concrete architectural/engineering proposals, detailing:
- The Problem / Opportunity
- Proposed Design & Implementation in Rust
- Impact on Zero-Token Discipline, Security, and Cluster Performance
"""

def query_qwen():
    print("[1/2] Querying Local Qwen 3.8 (27B) via Gateway / Ollama...")
    url = "http://127.0.0.1:11434/api/chat"
    payload = {
        "model": "qwen3.8:27b",
        "messages": [
            {"role": "system", "content": "You are Qwen 3.8 (27B), the Lead Local Reasoning Partner in the AgentTrace cluster."},
            {"role": "user", "content": PROMPT}
        ],
        "stream": False,
        "options": {
            "temperature": 0.3,
            "num_predict": 1500
        }
    }
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=180) as resp:
            result = json.loads(resp.read().decode("utf-8"))
            content = result.get("message", {}).get("content", "")
            print(" -> Qwen response received successfully.")
            return content
    except Exception as e:
        print(f" -> Error querying Qwen: {e}")
        return f"Error querying Qwen: {e}"

def query_copilot():
    print("[2/2] Querying GitHub Copilot CLI (copilot.exe)...")
    copilot_prompt = PROMPT.strip()
    cmd = [
        "copilot.exe",
        "-p", copilot_prompt,
        "--allow-all-tools",
        "--no-ask-user"
    ]
    try:
        proc = subprocess.run(
            cmd,
            cwd=PROJECT_ROOT,
            capture_output=True,
            text=True,
            timeout=180,
            encoding="utf-8",
            errors="replace"
        )
        output = proc.stdout
        if proc.stderr:
            output += "\n" + proc.stderr
        print(" -> Copilot CLI response received successfully.")
        return output
    except Exception as e:
        print(f" -> Error querying Copilot CLI: {e}")
        return f"Error querying Copilot CLI: {e}"

if __name__ == "__main__":
    qwen_text = query_qwen()
    with (PROJECT_ROOT / "docs" / "QWEN_OPINION.md").open("w", encoding="utf-8") as f:
        f.write("# Qwen 3.8 (27B) Opinion on Next Focus Areas\n\n" + qwen_text)
    print("Saved docs/QWEN_OPINION.md")

    copilot_text = query_copilot()
    with (PROJECT_ROOT / "docs" / "COPILOT_OPINION.md").open("w", encoding="utf-8") as f:
        f.write("# GitHub Copilot CLI Opinion on Next Focus Areas\n\n" + copilot_text)
    print("Saved docs/COPILOT_OPINION.md")

    print("\n[DONE] Both opinions successfully gathered and recorded.")
