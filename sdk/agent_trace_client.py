#!/usr/bin/env python3
"""Zero-dependency Python SDK and CLI for another AI agent to pick up AgentTrace telemetry."""

import argparse
import json
import os
import sys
import urllib.error
import urllib.request


class AgentTraceClient:
    """Pure standard-library client for AgentTrace Gateway telemetry pickup and governance."""

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

    def list_zones(self) -> dict:
        """Lists all Safe Zones (VaultNoAccess, GuardedReadOnly) and AgentPlaygrounds."""
        return self.call_tool("list_containment_zones", {})

    def confine_to_playground(self, agent_id: str, confined: bool = True) -> dict:
        """Locks or unlocks an agent (or '*' all agents) to Danger Zone Playgrounds."""
        return self.call_tool("confine_agent_to_playground", {"agent_id": agent_id, "confined": confined})

    def verify_ledger(self) -> dict:
        """Verifies the SHA-256 hash chain and Ed25519 signatures across all blocks."""
        return self.call_tool("verify_ledger", {})

    def list_workgroups(self) -> dict:
        """Lists all Problem Workgroups, assigned agents, and discovered .md rule files."""
        return self._request("GET", "/api/v1/workgroups")

    def attach_markdown_rule(self, workgroup_id: str, title: str, file_path: str = "", inline_markdown: str = "") -> dict:
        """Attaches a Markdown (.md) rule file or inline rule to a Problem Workgroup."""
        return self._request(
            "POST",
            f"/api/v1/workgroups/{workgroup_id}/rules",
            {
                "title": title,
                "file_path": file_path or None,
                "inline_markdown": inline_markdown or None,
                "enforce_strict": True,
            },
        )

    def dispatch_prompt(
        self,
        prompt: str,
        mode: str = "auto",
        workgroup_id: str = "auto",
        agent_id: str = "SDK-Hub-Client",
        unload_heavy_after: bool = True,
    ) -> dict:
        """Dispatches a prompt to the Central Prompt Hub (/api/v1/hub/dispatch).
        Modes:
          - 'auto': Smart intent classification -> best single cluster model
          - 'pipeline': Stage 1 Qwen Builder -> Stage 2 DeepSeek-R1 Sanity Verifier
          - 'swarm': Parallel execution on Qwen + DeepSeek-R1
        """
        return self._request(
            "POST",
            "/api/v1/hub/dispatch",
            {
                "prompt": prompt,
                "mode": mode,
                "workgroup_id": workgroup_id,
                "agent_id": agent_id,
                "unload_heavy_after": unload_heavy_after,
            },
        )


def format_markdown_briefing(snapshot: dict) -> str:
    """Formats a compact Markdown situational briefing for an LLM context window."""
    summary = snapshot.get("cluster_summary") or {}
    ledger = snapshot.get("ledger_integrity") or {}
    alerts = snapshot.get("active_security_alerts") or []
    diffs = snapshot.get("recent_code_diffs") or []
    zones = snapshot.get("containment_zones") or []

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
    lines.append(f"## 2. Containment Zones ({len(zones)})")
    for z in zones:
        lines.append(
            f"- **{z.get('kind')}** `{z.get('name')}`: `{z.get('path_prefix')}` (Scope: `{z.get('applies_to', '*')}`, Exempt: `{z.get('exempt_agents', [])}`)"
        )

    lines.append("")
    lines.append(f"## 3. Recent In-Flight Code Diffs ({len(diffs)})")
    if not diffs:
        lines.append("- No recent file write diffs recorded.")
    for d in diffs[:5]:
        lines.append(f"- **{d.get('node_id')}** modified `{d.get('target')}` (`{d.get('security_verdict')}`)")

    return "\n".join(lines)


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    parser = argparse.ArgumentParser(description="AgentTrace Telemetry Pickup, Workgroups & Prompt Hub CLI")
    parser.add_argument("--url", default="", help="Gateway base URL (default: http://localhost:8080)")
    parser.add_argument("--handoff", action="store_true", help="Fetch full JSON telemetry handoff snapshot")
    parser.add_argument("--briefing", action="store_true", help="Print compact Markdown briefing for an LLM prompt")
    parser.add_argument("--alerts", action="store_true", help="List active unresolved security alerts")
    parser.add_argument("--reset-alerts", action="store_true", help="Acknowledge/reset all active security alerts")
    parser.add_argument("--zones", action="store_true", help="List Safe Zones and Danger Zone Playgrounds")
    parser.add_argument("--confine", metavar="AGENT_ID", help="Confine an agent (or '*') to Danger Zone Playgrounds")
    parser.add_argument("--unconfine", metavar="AGENT_ID", help="Lift Playground confinement for an agent (or '*')")
    parser.add_argument("--verify", action="store_true", help="Verify SHA-256 + Ed25519 ledger integrity")
    parser.add_argument("--workgroups", action="store_true", help="List all Problem Workgroups, assigned agents, and attached .md rules")
    parser.add_argument("--workgroup", default="auto", metavar="WG_ID", help="Target Problem Workgroup ID (e.g. wg-coding, wg-image-gen) when dispatching")
    parser.add_argument("--attach-rule", nargs=2, metavar=("WG_ID", "MD_PATH"), help="Attach a workspace .md rule file to a Problem Workgroup")
    parser.add_argument("--ask", metavar="PROMPT", help="Auto-route a prompt to the best cluster model via Prompt Hub")
    parser.add_argument("--pipeline", metavar="PROMPT", help="Run 2-stage Pipeline (Qwen Draft -> DeepSeek-R1 Verify) via Prompt Hub")
    parser.add_argument("--swarm", metavar="PROMPT", help="Run parallel Swarm (Qwen + DeepSeek-R1) via Prompt Hub")
    args = parser.parse_args()

    client = AgentTraceClient(base_url=args.url, timeout=120.0)

    if args.workgroups:
        print(json.dumps(client.list_workgroups(), indent=2))
        return 0
    if args.attach_rule:
        wg_id, md_path = args.attach_rule
        print(json.dumps(client.attach_markdown_rule(wg_id, title=md_path, file_path=md_path), indent=2))
        return 0
    if args.ask:
        print(json.dumps(client.dispatch_prompt(args.ask, mode="auto", workgroup_id=args.workgroup), indent=2))
        return 0
    if args.pipeline:
        print(json.dumps(client.dispatch_prompt(args.pipeline, mode="pipeline", workgroup_id=args.workgroup), indent=2))
        return 0
    if args.swarm:
        print(json.dumps(client.dispatch_prompt(args.swarm, mode="swarm", workgroup_id=args.workgroup), indent=2))
        return 0
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
    if args.zones:
        print(json.dumps(client.list_zones(), indent=2))
        return 0
    if args.confine:
        print(json.dumps(client.confine_to_playground(args.confine, True), indent=2))
        return 0
    if args.unconfine:
        print(json.dumps(client.confine_to_playground(args.unconfine, False), indent=2))
        return 0
    if args.verify:
        print(json.dumps(client.verify_ledger(), indent=2))
        return 0

    # Default action: print full telemetry handoff snapshot
    print(json.dumps(client.get_handoff_snapshot(), indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())

