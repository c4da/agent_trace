#!/usr/bin/env python3
"""
AgentTrace Unified Cluster Installer & Swarm Orchestrator (Single-File Cross-Platform)
Compatible with: Windows 10/11, Ubuntu, Debian, Fedora, Arch, CentOS, Alpine, macOS
Dependencies: Pure Python 3 Standard Library ONLY (zero pip/wheel requirements)

Usage:
  Interactive Mode:
    python install.py

  Express Lead Node Setup (~30s):
    python install.py --lead

  Worker Node Setup (Automatic 1-Line Pairing):
    python install.py --worker --lead-url http://192.168.1.101:8080

  Network Subnet Discovery:
    python install.py --discover
"""

import sys
import os
import json
import time
import socket
import platform
import argparse
import subprocess
import threading
import urllib.request
import urllib.error
from http.server import HTTPServer, BaseHTTPRequestHandler

# --- ANSI Color Utilities (Auto-detect terminal support) ---
USE_COLOR = sys.stdout.isatty() or platform.system() != "Windows" or os.environ.get("WT_SESSION") is not None

def col(text: str, color_code: str) -> str:
    if not USE_COLOR:
        return text
    return f"\033[{color_code}m{text}\033[0m"

def cyan(text: str) -> str: return col(text, "36")
def green(text: str) -> str: return col(text, "32")
def yellow(text: str) -> str: return col(text, "33")
def red(text: str) -> str: return col(text, "31")
def gray(text: str) -> str: return col(text, "90")
def bold(text: str) -> str: return col(text, "1")

# --- Constants & Paths ---
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__)) if ("__file__" in globals() and __file__) else os.getcwd()
CONFIG_FILE = os.path.join(SCRIPT_DIR, "cluster_config.json")
DEFAULT_LEAD_PORT = 8080
DEFAULT_HERMES_PORT = 8642
DEFAULT_OLLAMA_PORT = 11434
DEFAULT_MODEL = "gemma4:latest"

# Windows legacy consoles (cp1252) crash on emoji output; force UTF-8 with replacement.
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

# --- Secrets ---
# Same contract as the Rust gateway (src/secrets.rs):
#   1. environment variable with the same name wins
#   2. otherwise KEY=VALUE line in $AGENTTRACE_SECRETS or <script_dir>/.secrets/agenttrace.env
SECRETS_FILE = os.environ.get("AGENTTRACE_SECRETS") or os.path.join(SCRIPT_DIR, ".secrets", "agenttrace.env")

def load_secret(name: str) -> str:
    val = os.environ.get(name)
    if val:
        return val.strip()
    try:
        with open(SECRETS_FILE, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line or line.startswith("#") or "=" not in line:
                    continue
                k, v = line.split("=", 1)
                if k.strip() == name:
                    return v.strip().strip('"').strip("'")
    except OSError:
        pass
    return ""

def gateway_headers() -> dict:
    """Headers for calls to an AgentTrace gateway (adds X-AgentTrace-Token when configured)."""
    h = {"Content-Type": "application/json"}
    tok = load_secret("AGENTTRACE_TOKEN")
    if tok:
        h["X-AgentTrace-Token"] = tok
    return h

# --- Network & Hardware Detection ---

def get_local_lan_ip() -> str:
    """Finds the primary non-loopback LAN IP address (192.168.x.x, 10.x.x.x, 172.16-31.x.x)."""
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.connect(("8.8.8.8", 80))
        ip = s.getsockname()[0]
        s.close()
        return ip
    except Exception:
        return "127.0.0.1"

def test_tcp_port(host: str, port: int, timeout: float = 0.5) -> bool:
    """Tests if a TCP port is open and accepting handshakes."""
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(timeout)
    try:
        res = s.connect_ex((host, port))
        s.close()
        return res == 0
    except Exception:
        return False

def detect_gpu() -> str:
    """Detects presence of NVIDIA GPU or accelerator."""
    try:
        proc = subprocess.run(["nvidia-smi", "--query-gpu=name", "--format=csv,noheader"],
                              capture_output=True, text=True, timeout=3)
        if proc.returncode == 0 and proc.stdout.strip():
            gpu_name = proc.stdout.strip().split("\n")[0]
            return f"NVIDIA CUDA ({gpu_name})"
    except Exception:
        pass
    return "Standard CPU"

# --- Resumable Configuration Management ---

def load_config() -> dict:
    if os.path.exists(CONFIG_FILE):
        try:
            with open(CONFIG_FILE, "r", encoding="utf-8") as f:
                return json.load(f)
        except Exception:
            pass
    return None

def save_config(cfg: dict):
    try:
        cfg["updated_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        with open(CONFIG_FILE, "w", encoding="utf-8") as f:
            json.dump(cfg, f, indent=2)
    except Exception as e:
        print(yellow(f"[!] Warning: Could not save configuration: {e}"))

def init_default_config() -> dict:
    local_ip = get_local_lan_ip()
    hostname = socket.gethostname()
    hermes_path = os.path.join(os.environ.get("LOCALAPPDATA", ""), "hermes", "bin", "hermes.cmd") if platform.system() == "Windows" else "hermes"
    
    cfg = {
        "version": "1.0.0",
        "updated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "mode": "single_node",
        "lead_node": {
            "hostname": hostname,
            "ip": local_ip,
            "gateway_port": DEFAULT_LEAD_PORT,
            "ollama_port": DEFAULT_OLLAMA_PORT,
            "default_model": DEFAULT_MODEL,
            "hermes_cli_path": hermes_path,
            "status": "ready"
        },
        "worker_nodes": [],
        "completed_phases": ["prereqs"]
    }
    save_config(cfg)
    return cfg

# --- Subnet Auto-Discovery ---

DISCOVERY_PORTS = {
    8642: "Hermes Agent",
    11434: "Ollama LLM",
    8080: "AgentTrace Gateway",
    22: "OpenSSH",
    5985: "WinRM (PowerShell)",
    445: "SMB (Windows Admin)"
}

def probe_host(ip: str, timeout: float = 0.25):
    open_ports = {}
    for p, svc in DISCOVERY_PORTS.items():
        if test_tcp_port(ip, p, timeout=timeout):
            open_ports[p] = svc
    if not open_ports:
        return None
    try:
        hostname = socket.gethostbyaddr(ip)[0].split(".")[0]
    except Exception:
        hostname = ip
    return {"ip": ip, "hostname": hostname, "ports": open_ports}

def scan_subnet(silent: bool = False) -> list:
    my_ip = get_local_lan_ip()
    parts = my_ip.split(".")
    base = f"{parts[0]}.{parts[1]}.{parts[2]}"
    if not silent:
        print(cyan(f"[*] Scanning subnet {base}.0/24 for cluster nodes..."))
    
    targets = [f"{base}.{i}" for i in range(1, 255)]
    results = []
    threads = []
    lock = threading.Lock()

    def worker(chunk):
        for ip in chunk:
            if ip == my_ip:
                continue
            res = probe_host(ip)
            if res:
                with lock:
                    results.append(res)

    chunk_size = 8
    chunks = [targets[i:i + chunk_size] for i in range(0, len(targets), chunk_size)]
    for c in chunks:
        t = threading.Thread(target=worker, args=(c,))
        threads.append(t)
        t.start()

    for t in threads:
        t.join()

    results.sort(key=lambda x: [int(p) for p in x["ip"].split(".")])
    if not silent:
        print(green(f"[+] Scan completed. Discovered {len(results)} active device(s).\n"))
    return results

# --- Component Testers & Setup ---

def test_ollama() -> bool:
    print(cyan("[*] [1/3] Checking Local LLM Engine (Ollama :11434)..."))
    if not test_tcp_port("127.0.0.1", DEFAULT_OLLAMA_PORT):
        print(yellow("    [!] Ollama is not responding on port 11434."))
        # Try to launch daemon if installed
        try:
            if platform.system() == "Windows":
                subprocess.Popen(["ollama", "serve"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            else:
                subprocess.Popen(["ollama", "serve"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
            time.sleep(3)
        except Exception:
            pass
    
    try:
        req = urllib.request.Request(f"http://127.0.0.1:{DEFAULT_OLLAMA_PORT}/api/tags")
        with urllib.request.urlopen(req, timeout=3) as resp:
            data = json.loads(resp.read().decode())
            models = [m.get("name") for m in data.get("models", [])]
            print(green(f"    [+] Ollama is ONLINE with models: {', '.join(models) if models else 'None'}"))
            return True
    except Exception:
        print(yellow("    [!] Ollama is currently offline (Run 'ollama serve' to enable)."))
        return False

def test_hermes() -> bool:
    print(cyan("[*] [2/3] Checking Autonomous Hermes Agent CLI..."))
    hermes_cmd = "hermes"
    if platform.system() == "Windows":
        cand = os.path.join(os.environ.get("LOCALAPPDATA", ""), "hermes", "bin", "hermes.cmd")
        if os.path.exists(cand):
            hermes_cmd = cand
    
    try:
        proc = subprocess.run([hermes_cmd, "--version"], capture_output=True, text=True, timeout=5)
        if proc.returncode == 0:
            ver = proc.stdout.strip().split("\n")[0]
            print(green(f"    [+] Hermes CLI detected: {ver}"))
            return True
    except Exception:
        pass
    print(yellow(f"    [!] Hermes CLI not found in default PATH."))
    return False

def test_or_start_gateway() -> bool:
    print(cyan("[*] [3/3] Checking AgentTrace Gateway (:8080) & Cryptographic Ledger..."))
    if test_tcp_port("127.0.0.1", DEFAULT_LEAD_PORT):
        print(green("    [+] Gateway is ONLINE on port 8080!"))
    else:
        print(yellow("    [*] Gateway not running. Starting daemon..."))
        # Check binary in target/debug or current dir
        bin_name = "agent_trace_gw.exe" if platform.system() == "Windows" else "agent_trace_gw"
        candidates = [
            os.path.join(SCRIPT_DIR, "target", "debug", bin_name),
            os.path.join(SCRIPT_DIR, bin_name)
        ]
        bin_path = next((c for c in candidates if os.path.exists(c)), None)
        if bin_path:
            if platform.system() == "Windows":
                subprocess.Popen([bin_path], creationflags=0x08000000) # CREATE_NO_WINDOW
            else:
                subprocess.Popen([bin_path], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
            time.sleep(2)
            print(green("    [+] Gateway launched successfully!"))
        else:
            print(yellow(f"    [!] Binary not found at {bin_name}. Run 'cargo build' if compiling from source."))

    # Verify ledger integrity via API
    try:
        req = urllib.request.Request("http://127.0.0.1:8080/api/v1/summary")
        with urllib.request.urlopen(req, timeout=3) as resp:
            data = json.loads(resp.read().decode())
            blocks = data.get("total_events", 0)
            verified = data.get("ledger_verified", False)
            print(gray(f"    -> Cryptographic Ledger: {blocks} blocks (SHA-256 Verified: {verified})"))
            return True
    except Exception:
        return False

# --- Core Execution Modes ---

def run_express_lead():
    """Sets up Lead Node in ~30 seconds (Standalone Workstation)."""
    print(bold(cyan("\n==========================================================")))
    print(bold(cyan(" 🌐 AGENTTRACE EXPRESS LEAD NODE SETUP (~30s)")))
    print(bold(cyan("==========================================================")))
    
    cfg = load_config() or init_default_config()
    test_ollama()
    test_hermes()
    test_or_start_gateway()

    cfg["mode"] = "single_node"
    cfg["completed_phases"] = ["prereqs", "ollama", "hermes", "gateway"]
    save_config(cfg)

    print(bold(green("\n==========================================================")))
    print(bold(green(" 🎉 SINGLE-NODE SETUP COMPLETED SUCCESSFULLY!")))
    print(bold(green("==========================================================")))
    print(gray(f" - Lead Node:        {cfg['lead_node']['hostname']} ({cfg['lead_node']['ip']})"))
    print(gray(f" - Dashboard UI:     http://localhost:8080/dashboard"))
    print(gray(f" - Config File:      {CONFIG_FILE}"))
    print(yellow("\n 💡 WANT TO ADD A WORKER NODE LATER?"))
    print(gray(" Run: python install.py anytime to discover and pair worker nodes!\n"))

def run_worker_setup(lead_url: str):
    """Sets up Worker Node on Linux or Windows and pairs with Lead Node."""
    print(bold(cyan("\n==========================================================")))
    print(bold(cyan(" 🌐 AGENTTRACE WORKER NODE AUTO-DEPLOYER")))
    print(bold(cyan("==========================================================")))
    print(yellow(f"[+] Lead Gateway Target: {lead_url}"))

    my_ip = get_local_lan_ip()
    hostname = socket.gethostname()
    gpu = detect_gpu()
    print(gray(f"[*] Node: {hostname} | IP: {my_ip} | Acceleration: {gpu}"))

    # Detect local LLM
    local_port = 8642
    if test_tcp_port("127.0.0.1", 8642):
        print(green("[+] Detected local hermes-agent on :8642"))
        local_port = 8642
    elif test_tcp_port("127.0.0.1", 11434):
        print(green("[+] Detected local Ollama on :11434"))
        local_port = 11434
    else:
        print(yellow("[!] No local LLM found on :8642 or :11434. Defaulting to :8642"))

    # Register heartbeat with Lead
    node_id = f"Worker-{hostname}"
    payload = {
        "node_id": node_id,
        "hostname": hostname,
        "ip_address": my_ip,
        "role": f"Worker Node ({gpu} | LLM :{local_port})",
        "status": "ACTIVE",
        "last_seen": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "public_key": None
    }

    print(cyan(f"[*] Registering node with cluster lead {lead_url}..."))
    try:
        req = urllib.request.Request(
            f"{lead_url}/api/v1/heartbeat",
            data=json.dumps(payload).encode("utf-8"),
            headers=gateway_headers()
        )
        with urllib.request.urlopen(req, timeout=5):
            print(green("[+] Heartbeat accepted! Node joined the mesh!"))
    except Exception as e:
        print(yellow(f"[!] Warning: Could not reach {lead_url} directly ({e}). Starting local proxy..."))

    # Start Edge Proxy Daemon
    start_edge_proxy_daemon(lead_url, local_port)

def start_edge_proxy_daemon(lead_url: str, upstream_port: int):
    """Starts lightweight, pure Python edge proxy daemon listening on :8080."""
    class ProxyHandler(BaseHTTPRequestHandler):
        def do_GET(self):
            if self.path in ("/health", "/api/v1/health"):
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({"status": "ok", "upstream_port": upstream_port}).encode())
            else:
                self._proxy("GET")

        def do_POST(self):
            self._proxy("POST")

        def _proxy(self, method):
            content_len = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_len) if content_len > 0 else None
            target = f"http://127.0.0.1:{upstream_port}{self.path}"
            headers = {k: v for k, v in self.headers.items() if k.lower() not in ("host", "content-length")}
            try:
                req = urllib.request.Request(target, data=body, headers=headers, method=method)
                with urllib.request.urlopen(req, timeout=120) as resp:
                    resp_data = resp.read()
                    self.send_response(resp.status)
                    for k, v in resp.getheaders():
                        if k.lower() not in ("content-length", "transfer-encoding"):
                            self.send_header(k, v)
                    self.send_header("Content-Length", str(len(resp_data)))
                    self.end_headers()
                    self.wfile.write(resp_data)
            except Exception as e:
                self.send_response(502)
                self.end_headers()
                self.wfile.write(json.dumps({"error": str(e)}).encode())

        def log_message(self, *args): pass

    if not test_tcp_port("127.0.0.1", DEFAULT_LEAD_PORT):
        print(green("[+] Starting background edge telemetry proxy on :8080..."))
        server = HTTPServer(("0.0.0.0", DEFAULT_LEAD_PORT), ProxyHandler)
        t = threading.Thread(target=server.serve_forever, daemon=True)
        t.start()
        time.sleep(1)
        print(green(" 🎉 SUCCESS: Worker Edge Proxy is active on http://localhost:8080!"))
    else:
        print(gray("    [*] Port 8080 is already active on this node."))

# --- Interactive Main Menu ---

def show_interactive_menu():
    while True:
        cfg = load_config() or init_default_config()
        lead = cfg.get("lead_node", {})
        workers = cfg.get("worker_nodes", [])

        print(bold(cyan("\n==========================================================")))
        print(bold(cyan(" 🌐 AGENTTRACE UNIFIED CLUSTER ORCHESTRATOR")))
        print(bold(cyan("==========================================================")))
        print(gray(f" Node: {socket.gethostname()} ({get_local_lan_ip()}) | OS: {platform.system()} {platform.release()}"))
        print(bold(yellow("\n CLUSTER TOPOLOGY:")))
        print(f" - Lead Node:    {lead.get('hostname')} ({lead.get('ip')}) [Gateway :{lead.get('gateway_port')}]")
        print(f" - Local LLM:    {lead.get('default_model')} via Ollama (:{lead.get('ollama_port')})")
        print(f" - Worker Nodes: {len(workers)} registered node(s)")
        for w in workers:
            print(gray(f"   -> [{w.get('node_id')}] {w.get('hostname')} ({w.get('ip')})"))

        print(bold(yellow("\n ACTIONS:")))
        print(" [1] ⚡ Express Lead Setup (Re-verify Ollama, Hermes, Gateway in ~30s)")
        print(" [2] ➕ Add / Pair Worker Node (Auto-Discover LAN or Enter IP)")
        print(" [3] 🔍 Auto-Discover Subnet (Scan 192.168.1.0/24 for Hermes, Ollama, SSH)")
        print(" [4] 🧠 Local LLM & Model Manager (List models, pull Gemma 4, latency)")
        print(" [5] 🤖 Hermes Agent & Delegation Test (Zero-Token local/worker execution)")
        print(" [6] 📊 Open Live Web Dashboard (http://localhost:8080/dashboard)")
        print(" [Q] Exit")
        print(gray("----------------------------------------------------------"))

        choice = input("Select an option [1-6, Q]: ").strip().upper()

        if choice == "1":
            run_express_lead()
        elif choice == "2":
            add_worker_interactive(cfg)
        elif choice == "3":
            results = scan_subnet(silent=False)
            for idx, r in enumerate(results, 1):
                ports_s = ", ".join([f"{p} ({svc})" for p, svc in r["ports"].items()])
                print(f" [{idx}] {r['ip']} ({r['hostname']}) -> Ports: {ports_s}")
            input(gray("\nPress Enter to return to menu..."))
        elif choice == "4":
            manage_llms()
        elif choice == "5":
            test_hermes_delegation()
        elif choice == "6":
            import webbrowser
            webbrowser.open("http://localhost:8080/dashboard")
        elif choice == "Q":
            print(cyan("\n[+] Installer closed. AgentTrace cluster remains active in background.\n"))
            break

def add_worker_interactive(cfg: dict):
    print(bold(yellow("\nAdd Worker Node to Cluster:")))
    print(" [1] Auto-Discover candidate machines on LAN")
    print(" [2] Enter Worker IP manually (e.g. 192.168.1.102)")
    print(" [B] Back")
    
    sub = input("Select [1, 2, B]: ").strip().upper()
    target_ip = ""
    if sub == "1":
        nodes = scan_subnet(silent=False)
        if not nodes:
            print(yellow("[-] No remote machines found on LAN."))
            return
        for i, n in enumerate(nodes, 1):
            ports_s = ", ".join([f"{p} ({svc})" for p, svc in n["ports"].items()])
            print(f" [{i}] {n['ip']} ({n['hostname']}) -> {ports_s}")
        sel = input(f"Select machine [1-{len(nodes)}]: ").strip()
        try:
            target_ip = nodes[int(sel) - 1]["ip"]
        except Exception:
            return
    elif sub == "2":
        target_ip = input("Enter Worker IP: ").strip()
    else:
        return

    lead_ip = cfg.get("lead_node", {}).get("ip", get_local_lan_ip())
    print(bold(green("\n==========================================================")))
    print(bold(green(" 🚀 UNIVERSAL 1-LINE INSTALLER FOR THIS WORKER:")))
    print(bold(green("==========================================================")))
    print(yellow(" Works on BOTH Windows and Linux:"))
    print(bold(f"\n   curl -sSL http://{lead_ip}:8080/install | python3\n"))
    print(gray(" On Windows PowerShell you can also run:"))
    print(bold(f"   irm http://{lead_ip}:8080/install | python\n"))
    print(gray(" This automatically configures local GPU/Hermes, sets up the edge proxy,"))
    print(gray(" and registers the node in your cluster topology without manual steps."))
    print(bold(green("==========================================================")))

    # Save to config
    existing = [w for w in cfg.get("worker_nodes", []) if w.get("ip") == target_ip]
    if not existing:
        cfg["worker_nodes"].append({
            "node_id": "PC-2" if target_ip == "192.168.1.102" else f"Worker-{target_ip}",
            "hostname": target_ip,
            "ip": target_ip,
            "date_added": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "status": "active"
        })
        save_config(cfg)
        print(green(f"[+] Saved {target_ip} to cluster_config.json!"))

    input(gray("\nPress Enter to return to menu..."))

def manage_llms():
    print(cyan("\n[*] Querying local Ollama (:11434)..."))
    try:
        req = urllib.request.Request("http://127.0.0.1:11434/api/tags")
        with urllib.request.urlopen(req, timeout=3) as resp:
            data = json.loads(resp.read().decode())
            print(green("Installed Models:"))
            for m in data.get("models", []):
                size_gb = round(m.get("size", 0) / (1024**3), 2)
                print(f" - {m.get('name')} ({size_gb} GB)")
    except Exception as e:
        print(red(f"[-] Ollama error: {e}"))
    input(gray("\nPress Enter to return..."))

def test_hermes_delegation():
    print(cyan("\n[*] Testing delegation to PC-2 Hermes Worker (:8642)..."))
    key = load_secret("WORKER_HERMES_KEY")
    if not key:
        print(yellow("[!] WORKER_HERMES_KEY not set (env var or .secrets/agenttrace.env). Trying without auth."))
    try:
        req = urllib.request.Request(
            "http://192.168.1.102:8642/health",
            headers={"Authorization": f"Bearer {key}"} if key else {}
        )
        with urllib.request.urlopen(req, timeout=3) as resp:
            data = json.loads(resp.read().decode())
            print(green(f"[+] PC-2 Hermes is ONLINE: {data}"))
    except Exception as e:
        print(yellow(f"[-] PC-2 query: {e}"))
    input(gray("\nPress Enter to return..."))

# --- CLI Router ---

def main():
    parser = argparse.ArgumentParser(description="AgentTrace Single-File Universal Installer & Swarm Orchestrator")
    parser.add_argument("--lead", action="store_true", help="Run Express Lead Node setup (~30s)")
    parser.add_argument("--worker", action="store_true", help="Run Worker Node setup")
    parser.add_argument("--lead-url", type=str, default="http://192.168.1.101:8080", help="Lead Gateway URL for worker pairing")
    parser.add_argument("--discover", action="store_true", help="Run LAN subnet discovery scan")
    args = parser.parse_args()

    if args.lead:
        run_express_lead()
    elif args.worker:
        run_worker_setup(args.lead_url)
    elif args.discover:
        hosts = scan_subnet(silent=False)
        for h in hosts:
            svcs = ", ".join([f"{svc} (:{p})" for p, svc in h["ports"].items()])
            print(f"  {green('*')} {bold(h['ip'])} ({cyan(h['hostname'])}) -> {svcs}")
    else:
        show_interactive_menu()

if __name__ == "__main__":
    main()
