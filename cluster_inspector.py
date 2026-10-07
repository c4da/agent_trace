#!/usr/bin/env python3
"""
AgentTrace Cluster Inspector & Autonomous Watchdog
Enables a secondary agent (like Hermes) or an operator to audit all cluster gateways
and nodes from one central vantage point without logging into individual PCs.
"""

import argparse
import datetime
import json
import sys
import time
import urllib.request
import urllib.error
import urllib.parse
from typing import List, Dict, Any, Optional

if sys.platform == "win32":
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

HEARTBEAT_STALE_THRESHOLD_SEC = 120
BLAST_RADIUS_ALERT_THRESHOLD = 40

class ClusterInspector:
    def __init__(self, gateway_urls: Optional[List[str]] = None):
        self.gateway_urls = [u.rstrip('/') for u in (gateway_urls or ['http://localhost:8080'])]

    def _http_get(self, url: str) -> Optional[Any]:
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "AgentTrace-Inspector/1.0"})
            with urllib.request.urlopen(req, timeout=10) as resp:
                if resp.status == 200:
                    return json.loads(resp.read().decode('utf-8'))
        except Exception as e:
            # Silent fallback / logged error
            pass
        return None

    def fetch_cluster_summary(self) -> Dict[str, Any]:
        """Aggregates metrics and active nodes across all configured gateways."""
        aggregated = {
            "gateways_contacted": len(self.gateway_urls),
            "gateways_online": 0,
            "total_nodes": 0,
            "active_nodes": {},
            "total_events": 0,
            "total_reads": 0,
            "total_writes": 0,
            "total_commands": 0,
            "security_alerts": 0,
            "blast_radius_score": 0,
        }

        for gw in self.gateway_urls:
            data = self._http_get(f"{gw}/api/v1/summary")
            if not data:
                continue

            aggregated["gateways_online"] += 1
            aggregated["total_events"] += data.get("total_events", 0)
            aggregated["total_reads"] += data.get("total_reads", 0)
            aggregated["total_writes"] += data.get("total_writes", 0)
            aggregated["total_commands"] += data.get("total_commands", 0)
            aggregated["security_alerts"] += data.get("security_alerts", 0)
            aggregated["blast_radius_score"] = max(aggregated["blast_radius_score"], data.get("blast_radius_score", 0))

            for node in data.get("active_nodes", []):
                nid = node.get("node_id")
                if nid and nid not in aggregated["active_nodes"]:
                    aggregated["active_nodes"][nid] = node

        aggregated["total_nodes"] = len(aggregated["active_nodes"])
        return aggregated

    def fetch_recent_events(self, limit: int = 50, node_filter: Optional[str] = None) -> List[Dict[str, Any]]:
        """Queries events from all gateways, dedupes, and sorts chronologically."""
        all_events = {}
        for gw in self.gateway_urls:
            url = f"{gw}/api/v1/events?limit={limit}"
            events = self._http_get(url)
            if isinstance(events, list):
                for ev in events:
                    eid = ev.get("event_id")
                    if eid:
                        all_events[eid] = ev

        events_list = list(all_events.values())
        if node_filter:
            events_list = [e for e in events_list if e.get("node_id", "").lower() == node_filter.lower()]

        # Sort descending by timestamp
        events_list.sort(key=lambda x: x.get("timestamp", ""), reverse=True)
        return events_list[:limit]

    def run_security_audit(self) -> Dict[str, Any]:
        """
        Runs an autonomous compliance & security scan across all cluster nodes.
        Checks for:
        1. Nodes with stale heartbeats (> 120s)
        2. High blast-radius scores
        3. Unauthorized file touches (.env, credentials)
        4. Non-zero exit code command failures
        """
        summary = self.fetch_cluster_summary()
        events = self.fetch_recent_events(limit=100)

        now = datetime.datetime.now(datetime.timezone.utc)
        stale_nodes = []
        for nid, node in summary["active_nodes"].items():
            last_seen_str = node.get("last_seen", "")
            try:
                # Handle RFC3339
                dt = datetime.datetime.fromisoformat(last_seen_str.replace("Z", "+00:00"))
                age_sec = (now - dt).total_seconds()
                if age_sec > HEARTBEAT_STALE_THRESHOLD_SEC:
                    stale_nodes.append({"node_id": nid, "age_sec": int(age_sec)})
            except Exception:
                pass

        security_violations = []
        failed_commands = []
        for ev in events:
            verdict = ev.get("security_verdict", "Allowed")
            if verdict != "Allowed":
                security_violations.append({
                    "node_id": ev.get("node_id"),
                    "target": ev.get("target"),
                    "verdict": verdict,
                    "reasons": ev.get("security_reasons", [])
                })
            if ev.get("event_type") == "CommandRun":
                details = ev.get("action_details", {})
                code = details.get("exit_code", 0)
                if code != 0:
                    failed_commands.append({
                        "node_id": ev.get("node_id"),
                        "command": ev.get("target"),
                        "exit_code": code
                    })

        blast_radius = summary["blast_radius_score"]
        status = "HEALTHY"
        if security_violations or blast_radius > BLAST_RADIUS_ALERT_THRESHOLD:
            status = "CRITICAL_ALERT"
        elif stale_nodes or failed_commands:
            status = "WARNING"

        return {
            "status": status,
            "blast_radius_score": blast_radius,
            "total_nodes": summary["total_nodes"],
            "gateways_online": summary["gateways_online"],
            "stale_nodes": stale_nodes,
            "security_violations": security_violations,
            "failed_commands": failed_commands,
            "events_analyzed": len(events)
        }

    def print_summary_table(self):
        s = self.fetch_cluster_summary()
        b = "=" * 65
        print(b)
        print(" 🛰️ AGENTTRACE CLUSTER TELEMETRY // FEDERATED SUMMARY")
        print(b)
        print(f" Gateways Queried:   {s['gateways_online']} / {s['gateways_contacted']} Online")
        print(f" Active Cluster PCs: {s['total_nodes']} Nodes Registered")
        print(f" Total Logged Events:{s['total_events']}")
        print(f" Files Touched:      {s['total_reads']} Read | {s['total_writes']} Modified")
        print(f" Commands Executed:  {s['total_commands']}")
        print(f" Security Alerts:    {s['security_alerts']}")
        print(f" Blast Radius Gauge: {s['blast_radius_score']} / 100")
        print("-" * 65)
        print(" CLUSTER NODE REGISTRY:")
        for nid, node in s["active_nodes"].items():
            print(f"  • [{nid}] {node.get('hostname')} ({node.get('ip_address')})")
            print(f"    Role:   {node.get('role')}")
            print(f"    Status: {node.get('status')} (Last seen: {node.get('last_seen', '')[:19]})")
        print(b)

    def print_audit_report(self):
        audit = self.run_security_audit()
        b = "=" * 65
        print(b)
        print(f" 🛡️ AUTONOMOUS AUDIT REPORT // STATUS: {audit['status']}")
        print(b)
        print(f" Analyzed Events:    {audit['events_analyzed']}")
        print(f" Blast Radius Score: {audit['blast_radius_score']} / 100")
        if audit['stale_nodes']:
            print("-" * 65)
            print(" [!] STALE NODES DETECTED (>120s without heartbeat):")
            for sn in audit['stale_nodes']:
                print(f"  * Node {sn['node_id']} (last heartbeat {sn['age_sec']}s ago)")
        if audit['security_violations']:
            print("-" * 65)
            print(" [!] SECURITY VIOLATIONS DETECTED:")
            for v in audit['security_violations']:
                print(f"  * [{v['verdict']}] {v['node_id']} -> {v['target']}")
                print(f"    Reasons: {', '.join(v['reasons'])}")
        if audit['failed_commands']:
            print("-" * 65)
            print(" [!] FAILED COMMANDS:")
            for fc in audit['failed_commands']:
                print(f"  * {fc['node_id']}: `{fc['command']}` exited with code {fc['exit_code']}")
        if not audit['security_violations'] and not audit['stale_nodes']:
            print("-" * 65)
            print(" [✓] No security anomalies or rogue activities detected.")
        print(b)

    def stream_live_events(self):
        """Streams live SSE events from the primary gateway."""
        gw = self.gateway_urls[0]
        url = f"{gw}/api/v1/stream"
        print(f"[*] Connecting to live SSE stream at: {url}")
        print("Press Ctrl+C to stop listening.\n")
        try:
            req = urllib.request.Request(url, headers={"Accept": "text/event-stream"})
            with urllib.request.urlopen(req) as resp:
                for line in resp:
                    decoded = line.decode('utf-8').strip()
                    if decoded.startswith("data:"):
                        payload = decoded[5:].strip()
                        try:
                            ev = json.loads(payload)
                            ts = ev.get("timestamp", "")[11:19]
                            nid = ev.get("node_id", "UNK")
                            typ = ev.get("event_type", "")
                            target = ev.get("target", "")
                            verdict = ev.get("security_verdict", "Allowed")
                            status = "✓" if verdict == "Allowed" else "🚨"
                            print(f"[{ts}] [{nid}] {status} {typ} -> {target}")
                        except Exception:
                            print(f"[RAW] {payload}")
        except KeyboardInterrupt:
            print("\nStream stopped.")
        except Exception as e:
            print(f"[-] Stream error: {e}")

def main():
    parser = argparse.ArgumentParser(description="AgentTrace Multi-Node Autonomous Inspector")
    parser.add_argument("--gateways", default="http://localhost:8080", help="Comma-separated gateway URLs")
    parser.add_argument("--summary", action="store_true", help="Print cluster summary across all nodes")
    parser.add_argument("--audit", action="store_true", help="Run autonomous security & compliance audit")
    parser.add_argument("--node", default=None, help="Filter event log by node ID (e.g. PC-1, PC-2, PC-3)")
    parser.add_argument("--watch", action="store_true", help="Stream live cluster events in real-time")

    args = parser.parse_args()
    gw_list = [g.strip() for g in args.gateways.split(",") if g.strip()]

    inspector = ClusterInspector(gw_list)

    if args.watch:
        inspector.stream_live_events()
    elif args.audit:
        inspector.print_audit_report()
    elif args.node:
        events = inspector.fetch_recent_events(limit=30, node_filter=args.node)
        print(f"--- Event Log for Node: {args.node} ({len(events)} events) ---")
        for e in events:
            print(f"[{e.get('timestamp')[:19]}] {e.get('event_type')} -> {e.get('target')}")
    else:
        # Default action: summary
        inspector.print_summary_table()

if __name__ == "__main__":
    main()
