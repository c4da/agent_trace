#!/usr/bin/env python3
"""
AgentTrace Gateway: End-to-End Verification Test Suite
Tests:
1. Zero-Walk Over-the-Air (OTA) Binary Manifest & Checksum Integrity
2. Dynamic PowerShell Bootstrap Script Generation
3. Standalone Native Rust MCP Server (agent_trace_mcp.exe) stdio JSON-RPC
4. Autonomous MCP Workgroup & Rule Provisioning
5. Remote Worker Node Deploy Trigger Endpoint
6. Cryptographic Ledger Chain Integrity & Ed25519 Signatures

Dependencies: Pure Python Standard Library (unittest, urllib, json, subprocess, hashlib).
"""

import unittest
import urllib.request
import urllib.error
import json
import subprocess
import os
import sys

GATEWAY_BASE = os.environ.get("AGENTTRACE_GATEWAY_URL", "http://127.0.0.1:8080")
MCP_EXE = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "target", "debug", "agent_trace_mcp.exe")

class TestZeroWalkAndHardening(unittest.TestCase):

    def test_01_ota_manifest_and_binary_checksum(self):
        """Verifies that the Lead Node correctly serves the compiled binary and cryptographic manifest."""
        manifest_url = f"{GATEWAY_BASE}/api/v1/dist/manifest"
        req = urllib.request.Request(manifest_url)
        with urllib.request.urlopen(req, timeout=5) as resp:
            self.assertEqual(resp.status, 200)
            data = json.loads(resp.read().decode("utf-8"))

        self.assertEqual(data["binary_name"], "agent_trace_gw.exe")
        self.assertEqual(len(data["sha256"]), 64)
        self.assertGreater(data["size_bytes"], 10_000_000)
        self.assertTrue(data["download_url"].endswith("/api/v1/dist/agent_trace_gw.exe"))

        # Verify X-Content-SHA256 header on binary stream
        bin_url = f"{GATEWAY_BASE}/api/v1/dist/agent_trace_gw.exe"
        head_req = urllib.request.Request(bin_url, method="HEAD")
        with urllib.request.urlopen(head_req, timeout=5) as head_resp:
            self.assertEqual(head_resp.status, 200)
            header_sha = head_resp.headers.get("x-content-sha256")
            self.assertIsNotNone(header_sha)
            self.assertEqual(header_sha.lower(), data["sha256"].lower())

    def test_02_dynamic_bootstrap_ps1_generation(self):
        """Verifies dynamic PowerShell bootstrap script generation with pre-filled Lead Node URL."""
        script_url = f"{GATEWAY_BASE}/bootstrap.ps1"
        req = urllib.request.Request(script_url)
        with urllib.request.urlopen(req, timeout=5) as resp:
            self.assertEqual(resp.status, 200)
            content_type = resp.headers.get("content-type", "")
            self.assertTrue("text/plain" in content_type)
            script = resp.read().decode("utf-8")

        self.assertIn("AgentTrace Zero-Walk Worker Bootstrap Script", script)
        self.assertIn("agent_trace_gw.exe", script)
        self.assertIn("Get-FileHash", script)
        self.assertIn(GATEWAY_BASE.rstrip("/"), script)

    def test_03_mcp_standalone_tools_list(self):
        """Verifies that agent_trace_mcp.exe exposes the new autonomous governance and deploy tools."""
        self.assertTrue(os.path.isfile(MCP_EXE), f"agent_trace_mcp.exe not found at {MCP_EXE}")

        rpc_req = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}) + "\n"
        proc = subprocess.Popen(
            [MCP_EXE],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        stdout, _ = proc.communicate(input=rpc_req, timeout=5)
        self.assertEqual(proc.returncode, 0)

        res = json.loads(stdout.strip())
        self.assertIn("result", res)
        tools = res["result"].get("tools", [])
        tool_names = [t["name"] for t in tools]

        self.assertIn("upsert_workgroup", tool_names)
        self.assertIn("attach_workgroup_rule", tool_names)
        self.assertIn("deploy_cluster_worker", tool_names)
        self.assertIn("check_worker_connection", tool_names)
        self.assertIn("get_cluster_bootstrap_command", tool_names)
        self.assertIn("record_project_decision", tool_names)
        self.assertIn("get_workgroup_rules", tool_names)

    def test_04_mcp_autonomous_workgroup_and_rule_provisioning(self):
        """Tests end-to-end workgroup creation and rule attachment over MCP stdio JSON-RPC."""
        # 1. Create workgroup via upsert_workgroup tool
        upsert_rpc = json.dumps({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "upsert_workgroup",
                "arguments": {
                    "id": "wg-test-autonomous-cad",
                    "name": "Autonomous CAD & Invariant Testing",
                    "problem_domain": "cad_geometry",
                    "description": "Formally tests geometric invariants and B-rep boundaries.",
                    "members": [
                        { "agent_id": "qwen3.8:27b", "role": "LeadArchitect" },
                        { "agent_id": "deepseek-r1:1.5b", "role": "SanityVerifier" }
                    ]
                }
            }
        }) + "\n"

        proc = subprocess.Popen([MCP_EXE], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        stdout, _ = proc.communicate(input=upsert_rpc, timeout=5)
        res = json.loads(stdout.strip())
        self.assertFalse(res.get("result", {}).get("isError", False))

        # 2. Attach rule via attach_workgroup_rule tool
        attach_rpc = json.dumps({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "attach_workgroup_rule",
                "arguments": {
                    "workgroup_id": "wg-test-autonomous-cad",
                    "title": "Euler-Poincare Invariant Rule",
                    "inline_markdown": "- Invariant: V - E + F = 2 for closed 2-manifold polyhedra.\n- Zero volume divergence inversion.",
                    "enforce_strict": True
                }
            }
        }) + "\n"

        proc2 = subprocess.Popen([MCP_EXE], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        stdout2, _ = proc2.communicate(input=attach_rpc, timeout=5)
        res2 = json.loads(stdout2.strip())
        self.assertFalse(res2.get("result", {}).get("isError", False))

        # 3. Verify rule was stored in workgroup
        rules_req = urllib.request.Request(f"{GATEWAY_BASE}/api/v1/workgroups")
        with urllib.request.urlopen(rules_req, timeout=5) as resp:
            wg_data = json.loads(resp.read().decode("utf-8"))
        groups = wg_data.get("workgroups", [])
        target_group = next((g for g in groups if g["id"] == "wg-test-autonomous-cad"), None)
        self.assertIsNotNone(target_group)
        self.assertTrue(len(target_group["markdown_rules"]) >= 1)
        self.assertEqual(target_group["markdown_rules"][0]["title"], "Euler-Poincare Invariant Rule")

    def test_05_deploy_worker_endpoint(self):
        """Verifies that the OTA deploy endpoint accepts valid requests and returns structured trigger instructions."""
        deploy_url = f"{GATEWAY_BASE}/api/v1/cluster/deploy-worker"
        payload = json.dumps({
            "target_ip": "192.168.1.102",
            "port": 8642,
            "method": "hermes_or_ssh"
        }).encode("utf-8")

        req = urllib.request.Request(deploy_url, data=payload, headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=8) as resp:
            self.assertEqual(resp.status, 200)
            data = json.loads(resp.read().decode("utf-8"))
            self.assertEqual(data["status"], "triggered")
            self.assertEqual(data["target_ip"], "192.168.1.102")
            self.assertIn("manual_bootstrap_command", data)

    def test_06_mcp_cluster_worker_inspection_and_bootstrap(self):
        """Verifies that an AI agent can inspect cluster worker reachability and obtain the bootstrap command via MCP."""
        # 1. Call get_cluster_bootstrap_command over MCP
        boot_rpc = json.dumps({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "get_cluster_bootstrap_command",
                "arguments": { "os_family": "windows" }
            }
        }) + "\n"

        proc = subprocess.Popen([MCP_EXE], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        stdout, _ = proc.communicate(input=boot_rpc, timeout=5)
        res = json.loads(stdout.strip())
        self.assertFalse(res.get("result", {}).get("isError", False))
        content = json.loads(res["result"]["content"][0]["text"])
        self.assertIn("powershell_command", content)
        self.assertIn("bootstrap.ps1", content["powershell_command"])

        # 2. Call check_worker_connection over MCP
        check_rpc = json.dumps({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "check_worker_connection",
                "arguments": { "worker_ip": "192.168.1.102" }
            }
        }) + "\n"

        proc2 = subprocess.Popen([MCP_EXE], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        stdout2, _ = proc2.communicate(input=check_rpc, timeout=10)
        res2 = json.loads(stdout2.strip())
        self.assertFalse(res2.get("result", {}).get("isError", False))
        content2 = json.loads(res2["result"]["content"][0]["text"])
        self.assertIn("reports", content2)
        self.assertTrue(len(content2["reports"]) >= 1)

    def test_07_cryptographic_ledger_intact(self):
        """Verifies that all actions were cryptographically committed to the SHA-256 / Ed25519 chain without breaks."""
        verify_url = f"{GATEWAY_BASE}/api/v1/ledger/verify"
        req = urllib.request.Request(verify_url)
        with urllib.request.urlopen(req, timeout=5) as resp:
            self.assertEqual(resp.status, 200)
            data = json.loads(resp.read().decode("utf-8"))
            self.assertTrue(data.get("is_valid", False))
            self.assertGreater(data.get("total_blocks", 0), 0)
            self.assertIsNone(data.get("broken_block_id"))
            self.assertEqual(len(data.get("latest_block_hash", "")), 64)

if __name__ == "__main__":
    unittest.main(verbosity=2)
