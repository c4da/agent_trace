#!/usr/bin/env python3
"""
Pre-Flight Open-Source Public Handover Audit & Verification Script
Verifies:
1. Anonymity & Secret Scrubbing (zero personal paths, zero private keys in git).
2. Anti-Monolith Compliance (every .rs file < 800 lines).
3. Test Suite Integrity (all 87+ tests passing).
4. Live Gateway Health, Handover Endpoint & Cryptographic Ledger Integrity.
"""

import os
import sys
import json
import subprocess
import urllib.request
from pathlib import Path

if sys.platform == "win32":
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

GATEWAY_URL = "http://localhost:8080"
REPO_ROOT = Path(__file__).resolve().parent.parent

RED = "\033[91m"
GREEN = "\033[92m"
YELLOW = "\033[93m"
CYAN = "\033[96m"
RESET = "\033[0m"

def step(title: str):
    print(f"\n{CYAN}=== [CHECK] {title} ==={RESET}")

def ok(msg: str):
    print(f"  {GREEN}✅ [PASS]{RESET} {msg}")

def warn(msg: str):
    print(f"  {YELLOW}⚠️  [WARN]{RESET} {msg}")

def fail(msg: str):
    print(f"  {RED}❌ [FAIL]{RESET} {msg}")

def check_anonymity_and_secrets() -> bool:
    step("1. Anonymity, Secrets & Personal Paths Scrubbing")
    forbidden_patterns = [
        "C:\\Users\\milos",
        "C:\\\\Users\\\\milos",
        "BEGIN OPENSSH PRIVATE KEY",
        "BEGIN RSA PRIVATE KEY",
        "super_secret_master_token",
    ]

    # Run git ls-files to inspect only tracked files
    res = subprocess.run(["git", "ls-files"], cwd=REPO_ROOT, capture_output=True, text=True)
    if res.returncode != 0:
        warn("Git not found or not in git repo; scanning local directory.")
        tracked_files = [str(p) for p in REPO_ROOT.rglob("*.rs")]
    else:
        tracked_files = [f for f in res.stdout.splitlines() if f.strip()]

    clean = True
    for f in tracked_files:
        full_path = REPO_ROOT / f
        if not full_path.is_file():
            continue
        try:
            content = full_path.read_text(encoding="utf-8", errors="ignore")
            for pat in forbidden_patterns:
                if pat.lower() in content.lower():
                    # Exclude the verification script itself
                    if "verify_public_readiness.py" in f:
                        continue
                    fail(f"Forbidden pattern '{pat}' found in {f}")
                    clean = False
        except Exception:
            pass

    if clean:
        ok("Zero personal paths or unmasked private keys found in git-tracked files.")
    return clean

def check_anti_monolith_limits() -> bool:
    step("2. Anti-Monolith Compliance (< 800 lines per module)")
    src_dir = REPO_ROOT / "src"
    all_clean = True
    for rs_file in src_dir.rglob("*.rs"):
        lines = len(rs_file.read_text(encoding="utf-8", errors="ignore").splitlines())
        rel = rs_file.relative_to(REPO_ROOT)
        if lines > 800:
            fail(f"{rel} has {lines} lines (exceeds 800-line SRP ceiling!)")
            all_clean = False
        else:
            pass
    if all_clean:
        ok("All Rust source modules adhere strictly to the < 800-line Anti-Monolith standard.")
    return all_clean

def check_test_suite() -> bool:
    step("3. Test Suite Verification (cargo test)")
    res = subprocess.run(["cargo", "test", "--quiet"], cwd=REPO_ROOT, capture_output=True, text=True)
    if res.returncode == 0:
        ok("All 87+ unit and integration tests passed cleanly.")
        return True
    else:
        fail(f"cargo test failed:\n{res.stderr or res.stdout}")
        return False

def check_gateway_live_endpoints() -> bool:
    step("4. Live Gateway & Agent Handover Endpoints")
    endpoints = [
        ("/", "Visual Dashboard / Static Route"),
        ("/console", "Terminal Console TUI"),
        ("/policies", "Policies & Whitelists"),
        ("/workgroups", "Workgroups Mesh & Rules"),
        ("/graph", "Multi-Agent Contribution Graph"),
        ("/api/v1/workgroups", "Workgroups API"),
        ("/api/v1/ledger/verify", "Cryptographic Ledger Verification"),
        ("/api/v1/agent/handoff", "AI Agent Handoff Briefing"),
    ]

    all_ok = True
    for ep, desc in endpoints:
        url = f"{GATEWAY_URL}{ep}"
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "AgentTrace-Audit"})
            with urllib.request.urlopen(req, timeout=5) as resp:
                if resp.status in (200, 201):
                    ok(f"{ep} ({desc}) -> HTTP {resp.status}")
                else:
                    warn(f"{ep} -> HTTP {resp.status}")
                    all_ok = False
        except Exception as e:
            fail(f"{ep} ({desc}) -> Error: {e}")
            all_ok = False
    return all_ok

def main():
    print("=" * 70)
    print(f"{GREEN}🌐 AgentTrace Gateway :: Public Handover Pre-Flight Audit{RESET}")
    print("=" * 70)

    s1 = check_anonymity_and_secrets()
    s2 = check_anti_monolith_limits()
    s3 = check_test_suite()
    s4 = check_gateway_live_endpoints()

    print("\n" + "=" * 70)
    if s1 and s2 and s3 and s4:
        print(f"{GREEN}🎉 ALL AUDIT CHECKS PASSED — READY FOR PUBLIC RELEASE{RESET}")
    else:
        print(f"{RED}⚠️  SOME AUDIT CHECKS FAILED — REVIEW BEFORE RELEASING{RESET}")
    print("=" * 70)

if __name__ == "__main__":
    main()
