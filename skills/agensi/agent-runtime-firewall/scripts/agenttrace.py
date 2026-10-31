#!/usr/bin/env python3
"""Minimal stdlib client for the AgentTrace gateway.

Use it when the agent-trace MCP server is not configured in your agent.
It calls the same tools over HTTP: POST {AGENTTRACE_URL}/api/v1/agent/action

Examples:
  python agenttrace.py ping
  python agenttrace.py intercept_ai_guard --args '{"action": "Execute", "command": "rm -rf build"}'
  python agenttrace.py dispatch_cluster_prompt --args '{"prompt": "Review for bugs:"}' --attach src/lib.rs

Exit codes: 0 ok, 1 tool error, 2 gateway unreachable, 3 guard hard-blocked.
Environment: AGENTTRACE_URL (default http://127.0.0.1:8080), AGENTTRACE_TOKEN, AGENTTRACE_AGENT_ID.
"""
import argparse
import json
import os
import sys
import urllib.error
import urllib.request

DEFAULT_URL = "http://127.0.0.1:8080"
EXIT_OK, EXIT_TOOL_ERROR, EXIT_UNREACHABLE, EXIT_BLOCKED = 0, 1, 2, 3


def base_url():
    return (os.environ.get("AGENTTRACE_URL") or DEFAULT_URL).rstrip("/")


def build_request(path, payload=None):
    """Builds a request with the optional auth token header."""
    data = json.dumps(payload).encode("utf-8") if payload is not None else None
    req = urllib.request.Request(base_url() + path, data=data)
    req.add_header("Content-Type", "application/json")
    token = os.environ.get("AGENTTRACE_TOKEN", "").strip()
    if token:
        req.add_header("X-AgentTrace-Token", token)
    return req


def call_tool(tool, params, timeout=300):
    """Executes one AgentTrace tool and returns the response dict."""
    payload = {
        "agent_id": os.environ.get("AGENTTRACE_AGENT_ID", "skill-client"),
        "tool": tool,
        "parameters": params,
    }
    with urllib.request.urlopen(build_request("/api/v1/agent/action", payload), timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def attach_files(params, paths):
    """Appends file contents to params['prompt'] as fenced blocks (remote models cannot read your disk)."""
    blocks = []
    for path in paths:
        with open(path, "r", encoding="utf-8", errors="replace") as fh:
            blocks.append(f"\n\n--- FILE: {path} ---\n```\n{fh.read()}\n```")
    params["prompt"] = params.get("prompt", "") + "".join(blocks)
    return params


def exit_code_for(response):
    if not response.get("success", False):
        return EXIT_TOOL_ERROR
    result = response.get("result") or {}
    if isinstance(result, dict) and result.get("hard_blocked") is True:
        return EXIT_BLOCKED
    return EXIT_OK


def ping():
    try:
        with urllib.request.urlopen(build_request("/api/v1/ledger/verify"), timeout=5) as resp:
            print(json.dumps({"reachable": True, "url": base_url(), "status": resp.status}))
            return EXIT_OK
    except (urllib.error.URLError, OSError) as exc:
        print(json.dumps({"reachable": False, "url": base_url(), "error": str(exc)}))
        return EXIT_UNREACHABLE


def parse_args(argv):
    parser = argparse.ArgumentParser(description="Call an AgentTrace gateway tool over HTTP.")
    parser.add_argument("tool", help="Tool name, or 'ping' to check the gateway is reachable.")
    parser.add_argument("--args", default="{}", help="Tool parameters as a JSON object.")
    parser.add_argument("--attach", action="append", default=[], help="File to append to the prompt (repeatable).")
    return parser.parse_args(argv)


def main(argv=None):
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    args = parse_args(argv)
    if args.tool == "ping":
        return ping()
    params = json.loads(args.args)
    if args.attach:
        params = attach_files(params, args.attach)
    try:
        response = call_tool(args.tool, params)
    except urllib.error.HTTPError as exc:
        print(json.dumps({"success": False, "http_status": exc.code, "error": exc.read().decode("utf-8", "replace")}))
        return EXIT_TOOL_ERROR
    except (urllib.error.URLError, OSError) as exc:
        print(json.dumps({"success": False, "error": f"Gateway unreachable at {base_url()}: {exc}"}))
        return EXIT_UNREACHABLE
    print(json.dumps(response, indent=2, ensure_ascii=False))
    return exit_code_for(response)


if __name__ == "__main__":
    sys.exit(main())
