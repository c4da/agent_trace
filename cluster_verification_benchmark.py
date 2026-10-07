#!/usr/bin/env python3
"""
Cluster Verification & Latency Benchmark Tool
Created via autonomous delegation to PC-2 CUDA Worker Node (Hermes Agent / Gemma 4).
Uses ONLY Python Standard Library (urllib.request, json, time, sys).
"""

import urllib.request
import urllib.error
import json
import time
import sys
from typing import Dict, Any

BASE_URL = "http://localhost:8080"


def http_request(method: str, endpoint: str, data: bytes = None, headers: Dict[str, str] = None, timeout: float = 30.0) -> Any:
    """Handles generic HTTP requests using urllib.request."""
    url = f"{BASE_URL}{endpoint}"
    req = urllib.request.Request(url, headers=headers or {})
    req.method = method.upper()
    if data:
        req.data = data
    return urllib.request.urlopen(req, timeout=timeout)


def get_models() -> Dict[str, Any]:
    """Fetches available models and node assignments from the cluster catalog."""
    print(f"[STATUS] Querying AgentTrace Gateway catalog at {BASE_URL}/v1/models...")
    result = {"status": "FAILURE", "models": [], "pc2_online": False}
    try:
        with http_request('GET', '/v1/models', timeout=10.0) as response:
            content = response.read().decode('utf-8')
            data = json.loads(content)
            result["status"] = "SUCCESS"
            result["models"] = data.get("data", [])
            result["pc2_online"] = data.get("pc2_worker_online", False)
            result["total_models"] = data.get("total_cluster_models", len(result["models"]))
    except Exception as e:
        result["error"] = str(e)
    return result


def verify_ledger() -> Dict[str, Any]:
    """Verifies the cryptographic ledger hash chain and Ed25519 signatures."""
    print(f"[STATUS] Querying Cryptographic Ledger at {BASE_URL}/api/v1/ledger/verify...")
    result = {"status": "FAILURE", "is_valid": False, "total_blocks": 0}
    try:
        with http_request('GET', '/api/v1/ledger/verify', timeout=15.0) as response:
            content = response.read().decode('utf-8')
            data = json.loads(content)
            result["status"] = "SUCCESS"
            result["is_valid"] = data.get("is_valid", False)
            result["total_blocks"] = data.get("total_blocks", 0)
            result["latest_block_hash"] = data.get("latest_block_hash", "")
            result["broken_block_id"] = data.get("broken_block_id")
            result["error_reason"] = data.get("error_reason")
    except Exception as e:
        result["error"] = str(e)
    return result


def test_chat_completion(model: str = "hermes-agent") -> Dict[str, Any]:
    """Tests smart auto-routing and measures round-trip inference latency."""
    print(f"[STATUS] Testing smart auto-routing to '{model}' via {BASE_URL}/v1/chat/completions...")
    result = {"status": "FAILURE", "latency_ms": None, "reply": None}
    chat_payload = {
        "model": model,
        "messages": [{"role": "user", "content": "Cluster latency benchmark ping. Reply in 5 words."}],
        "max_tokens": 20
    }
    payload_bytes = json.dumps(chat_payload).encode('utf-8')
    headers = {"Content-Type": "application/json"}

    start_time = time.time()
    try:
        with http_request('POST', '/v1/chat/completions', data=payload_bytes, headers=headers, timeout=45.0) as response:
            latency_ms = (time.time() - start_time) * 1000.0
            content = response.read().decode('utf-8')
            data = json.loads(content)
            reply = ""
            if "choices" in data and len(data["choices"]) > 0:
                reply = data["choices"][0].get("message", {}).get("content", "")
            result["status"] = "SUCCESS"
            result["latency_ms"] = round(latency_ms, 2)
            result["reply"] = reply.strip()
    except Exception as e:
        result["error"] = str(e)
    return result


def run_benchmark():
    if hasattr(sys.stdout, 'reconfigure'):
        try:
            sys.stdout.reconfigure(encoding='utf-8', errors='replace')
        except Exception:
            pass

    print("=" * 70)
    print(" [*] AGENT TRACE CLUSTER VERIFICATION & BENCHMARK")
    print("=" * 70)

    # 1. Models Catalog
    m_info = get_models()
    print(f" -> Models Status:     {m_info['status']}")
    print(f" -> PC-2 Worker Node:  {'ONLINE' if m_info.get('pc2_online') else 'OFFLINE'}")
    print(f" -> Total Models:      {len(m_info.get('models', []))}")
    for m in m_info.get("models", []):
        mid = m.get("id", "unknown")
        node = m.get("node_id", "unknown")
        role = m.get("role", "")
        print(f"    - [{node:4s}] {mid:20s} - {role}")

    print("-" * 70)

    # 2. Cryptographic Ledger
    l_info = verify_ledger()
    print(f" -> Ledger Status:     {l_info['status']}")
    print(f" -> Cryptographic Valid: {'PASSED (Tamper-Free)' if l_info.get('is_valid') else 'FAILED'}")
    print(f" -> Total Blocks:      {l_info.get('total_blocks')}")
    if l_info.get("latest_block_hash"):
        print(f" -> Latest Block Hash: {l_info.get('latest_block_hash')[:24]}...")
    if not l_info.get("is_valid") and l_info.get("error_reason"):
        print(f" [!] Ledger Error:     {l_info.get('error_reason')}")

    print("-" * 70)

    # 3. Auto-Routing Latency Test to PC-2 Hermes
    c_info = test_chat_completion("hermes-agent")
    print(f" -> Routing Test:      {c_info['status']}")
    if c_info["status"] == "SUCCESS":
        print(f" -> Round-Trip Latency:{c_info['latency_ms']} ms")
        print(f" -> Target Response:   \"{c_info['reply']}\"")
    else:
        print(f" [!] Routing Error:    {c_info.get('error')}")

    print("=" * 70)
    all_ok = (m_info.get("status") == "SUCCESS" and 
              l_info.get("is_valid") is True and 
              c_info.get("status") == "SUCCESS")
    print(f" OVERALL CLUSTER HEALTH: {'[PASS] 100% HEALTHY' if all_ok else '[WARN] ISSUES DETECTED'}")
    print("=" * 70)
    return 0 if all_ok else 1


if __name__ == "__main__":
    sys.exit(run_benchmark())
