#!/usr/bin/env python3
"""Zero-dependency Python SDK and CLI for another AI agent to pick up AgentTrace telemetry."""

import argparse
import json
import os
import sys
import urllib.error
import urllib.request


class AgentTraceClient:
    """Pure standard-library client for AgentTrace Gateway telemetry pickup."""

    def __init__(self, base_url: str = "", token: str = "", timeout: float = 90.0) -> None:
        self.base_url = (base_url or os.environ.get("AGENTTRACE_URL", "http://localhost:8080")).rstrip("/")
        self.token = token or os.environ.get("AGENTTRACE_TOKEN", "")
        self.timeout = timeout

    def _request(self, method: str, path: str, payload: dict | None = None) -> dict | list:
        url = f"{self.base_url}{path}"
        headers = {"Content-Type": "application/json"}
        if self.token:
            headers["Authorization"] = f"Bearer {self.token}"
            headers["x-agenttrace-token"] = self.token
        data = json.dumps(payload).encode("utf-8") if payload is not None else None
        req = urllib.request.Request(url, data=data, headers=headers, method=method)
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                return json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as exc:
            body = exc.read().decode("utf-8", errors="replace")
            return {"error": f"HTTP {exc.code}", "details": body}
        except Exception as exc:
            return {"error": str(exc)}

    def get_handoff_snapshot(self, recent_limit: int = 15) -> dict:
        """Fetches the single-call telemetry pickup digest for an incoming AI agent."""
        res = self.call_tool("get_telemetry_handoff", {"recent_limit": recent_limit})
        return res.get("result", res) if isinstance(res, dict) else {"data": res}

    def call_tool(self, tool_name: str, parameters: dict | None = None) -> dict:
        """Invokes any registered AgentTrace tool via POST /api/v1/agent/action."""
        payload = {
            "agent_id": os.environ.get("AGENT_ID", "telemetry-pickup-agent"),
            "tool": tool_name,
            "parameters": parameters or {},
        }
        res = self._request("POST", "/api/v1/agent/action", payload)
        return res if isinstance(res, dict) else {"result": res}

    def get_active_alerts(self, include_acked: bool = False) -> dict:
        """Returns unresolved security violations (or full alert history)."""
        return self.call_tool("get_security_alerts", {"include_acked": include_acked})

    def acknowledge_alert(self, event_id: str = "", reset_all: bool = False) -> dict:
        """Non-destructively acknowledges an alert without mutating the SHA-256 ledger."""
        return self.call_tool("acknowledge_alert", {"event_id": event_id, "reset_all": reset_all})

    def verify_ledger(self) -> dict:
        """Verifies the SHA-256 hash chain and Ed25519 signatures across all blocks."""
        return self.call_tool("verify_ledger", {})

def format_markdown_briefing(snapshot: dict) -> str:
    """Formats a compact Markdown situational briefing for an LLM context window."""
    summary = snapshot.get("cluster_summary") or {}
    ledger = snapshot.get("ledger_integrity") or {}
    alerts = snapshot.get("active_security_alerts") or []
    diffs = snapshot.get("recent_code_diffs") or []

    lines = [
        "# AgentTrace Telemetry Handoff Briefing",
        f"- **Generated At**: `{snapshot.get('generated_at', 'N/A')}`",
        f"- **Cluster Status**: `{summary.get('total_nodes', 0)}` nodes | `{summary.get('total_events', 0)}` total events | `{summary.get('security_alerts', 0)}` active alerts",
        f"- **Cryptographic Ledger**: Valid=`{ledger.get('is_valid', False)}` | Blocks=`{ledger.get('total_blocks', 0)}` | Head=`{str(ledger.get('latest_block_hash', ''))[:16]}...`",
        "",
        f"## 1. Active Unresolved Security Alerts ({len(alerts)})",
    ]
    if not alerts:
        lines.append("- None (0 unresolved violations).")
    for a in alerts[:8]:
        reasons = ", ".join(a.get("security_reasons") or [])
        lines.append(
            f"- `[{a.get('security_verdict')}]` **{a.get('node_id')}** -> `{a.get('target')}` ({a.get('event_type')}) — {reasons} (`id: {a.get('event_id')}`)"
        )

    lines.append("")
    lines.append(f"## 2. Recent In-Flight Code Diffs ({len(diffs)})")
    if not diffs:
        lines.append("- No recent file write diffs recorded.")
    for d in diffs[:5]:
        lines.append(f"- **{d.get('node_id')}** modified `{d.get('target')}` (`{d.get('security_verdict')}`)")

    return "\n".join(lines)


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    parser = argparse.ArgumentParser(description="AgentTrace Telemetry Pickup CLI")
    parser.add_argument("--url", default="", help="Gateway base URL (default: http://localhost:8080)")
    parser.add_argument("--handoff", action="store_true", help="Fetch full JSON telemetry handoff snapshot")
    parser.add_argument("--briefing", action="store_true", help="Print compact Markdown briefing for an LLM prompt")
    parser.add_argument("--alerts", action="store_true", help="List active unresolved security alerts")
    parser.add_argument("--reset-alerts", action="store_true", help="Acknowledge/reset all active security alerts")
    parser.add_argument("--verify", action="store_true", help="Verify SHA-256 + Ed25519 ledger integrity")
    args = parser.parse_args()

    client = AgentTraceClient(base_url=args.url, timeout=120.0)

    if args.briefing:
        snap = client.get_handoff_snapshot()
        print(format_markdown_briefing(snap))
        return 0
    if args.alerts:
        print(json.dumps(client.get_active_alerts(), indent=2))
        return 0
    if args.reset_alerts:
        print(json.dumps(client.acknowledge_alert(reset_all=True), indent=2))
        return 0
    if args.verify:
        print(json.dumps(client.verify_ledger(), indent=2))
        return 0

    # Default action: print full telemetry handoff snapshot
    print(json.dumps(client.get_handoff_snapshot(), indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())

