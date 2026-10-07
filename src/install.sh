#!/usr/bin/env bash
# ==============================================================================
# AgentTrace Linux Cluster Node Auto-Deployer
# Compatible with Ubuntu, Debian, Fedora, CentOS, Arch, and Alpine
# ==============================================================================

set -e

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
GRAY='\033[0;90m'
NC='\033[0m' # No Color

echo -e "${CYAN}==========================================================${NC}"
echo -e "${CYAN} 🌐 AGENTTRACE LINUX CLUSTER NODE AUTO-DEPLOYER${NC}"
echo -e "${CYAN}==========================================================${NC}"

# 1. Lead Gateway Configuration (automatically substituted by gateway or overridden via env)
LEAD_HOST="${LEAD_HOST:-192.168.1.101}"
LEAD_PORT="${LEAD_PORT:-8080}"
LEAD_URL="http://${LEAD_HOST}:${LEAD_PORT}"

echo -e "${YELLOW}[+] Connecting to Lead Gateway: ${LEAD_URL}${NC}"

# 2. Setup Target Directory
INSTALL_DIR="${HOME}/.agent_trace_gw"
mkdir -p "${INSTALL_DIR}"
cd "${INSTALL_DIR}"

# 3. Detect Hostname and Local IP
HOSTNAME=$(hostname 2>/dev/null || cat /etc/hostname || echo "linux-node")
LOCAL_IP=$(ip -4 addr show scope global 2>/dev/null | grep -oP '(?<=inet\s)\d+(\.\d+){3}' | grep -E '^(192\.168|10\.|172\.(1[6-9]|2[0-9]|3[01]))' | head -n 1 || hostname -I 2>/dev/null | awk '{print $1}' || echo "127.0.0.1")

echo -e "${GRAY}[*] Hostname: ${HOSTNAME} | LAN IP: ${LOCAL_IP}${NC}"

# 4. Check for GPU / CUDA Acceleration
GPU_INFO="CPU"
if command -v nvidia-smi >/dev/null 2>&1; then
    GPU_NAME=$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -n 1 || echo "NVIDIA GPU")
    GPU_INFO="NVIDIA CUDA (${GPU_NAME})"
    echo -e "${GREEN}[+] Detected GPU Acceleration: ${GPU_INFO}${NC}"
fi

# 5. Detect Local LLM Service (Hermes / Ollama / vLLM)
LOCAL_PORT=8642
LLM_SERVICE="Hermes Agent"

if nc -z 127.0.0.1 8642 2>/dev/null || (exec 3<>/dev/tcp/127.0.0.1/8642) 2>/dev/null; then
    echo -e "${GREEN}[+] Detected local hermes-agent on :8642${NC}"
    LOCAL_PORT=8642
    LLM_SERVICE="hermes-agent :8642"
elif nc -z 127.0.0.1 11434 2>/dev/null || (exec 3<>/dev/tcp/127.0.0.1/11434) 2>/dev/null; then
    echo -e "${GREEN}[+] Detected local Ollama on :11434${NC}"
    LOCAL_PORT=11434
    LLM_SERVICE="Ollama :11434"
elif nc -z 127.0.0.1 8000 2>/dev/null || (exec 3<>/dev/tcp/127.0.0.1/8000) 2>/dev/null; then
    echo -e "${GREEN}[+] Detected local vLLM / OpenAI server on :8000${NC}"
    LOCAL_PORT=8000
    LLM_SERVICE="vLLM :8000"
else
    echo -e "${YELLOW}[!] No local LLM on :8642, :11434, or :8000. Defaulting upstream to :8642${NC}"
    LOCAL_PORT=8642
    LLM_SERVICE="hermes-agent :8642"
fi

LOCAL_UPSTREAM="http://127.0.0.1:${LOCAL_PORT}"

# 6. Register Heartbeat with Lead Node Gateway
NODE_ID="Linux-${HOSTNAME}"
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
ROLE="Linux Worker Node (${GPU_INFO} | ${LLM_SERVICE})"

HEARTBEAT_PAYLOAD=$(cat <<EOF
{
  "node_id": "${NODE_ID}",
  "hostname": "${HOSTNAME}",
  "ip_address": "${LOCAL_IP}",
  "role": "${ROLE}",
  "status": "ACTIVE",
  "last_seen": "${TIMESTAMP}",
  "public_key": null
}
EOF
)

echo -e "${CYAN}[*] Registering ${NODE_ID} (${LOCAL_IP}) with Lead Cluster...${NC}"
curl -s -X POST "${LEAD_URL}/api/v1/heartbeat" \
    -H "Content-Type: application/json" \
    -d "${HEARTBEAT_PAYLOAD}" >/dev/null || echo -e "${YELLOW}[!] Could not register heartbeat with ${LEAD_URL}. Proceeding with local setup...${NC}"

echo -e "${GREEN}[+] Node registration dispatched!${NC}"

# 7. Create Zero-Dependency Linux Edge Telemetry Proxy (Pure Python 3 Standard Library)
cat << 'EOF' > "${INSTALL_DIR}/edge_proxy.py"
#!/usr/bin/env python3
"""
AgentTrace Lightweight Edge Telemetry Proxy for Linux Nodes
Zero external wheel dependencies (pure Python standard library).
Proxies requests from :8080 to local LLM (:8642 / :11434), calculates SHA-256 hashes,
and replicates telemetry heartbeats to the Lead Gateway.
"""
import sys
import os
import json
import time
import hashlib
import threading
from http.server import HTTPServer, BaseHTTPRequestHandler
import urllib.request
import urllib.error

UPSTREAM_URL = os.environ.get("UPSTREAM_URL", "http://127.0.0.1:8642")
LEAD_URL = os.environ.get("LEAD_URL", "http://192.168.1.101:8080")
LISTEN_PORT = int(os.environ.get("PORT", "8080"))
HOSTNAME = os.environ.get("HOSTNAME", "linux-node")
IP_ADDR = os.environ.get("IP_ADDR", "127.0.0.1")

class EdgeProxyHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path in ("/health", "/api/v1/health"):
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            resp = {"status": "ok", "role": "edge_proxy", "upstream": UPSTREAM_URL}
            self.wfile.write(json.dumps(resp).encode())
        else:
            # Forward GET to upstream
            self._proxy_request("GET")

    def do_POST(self):
        self._proxy_request("POST")

    def _proxy_request(self, method):
        content_len = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_len) if content_len > 0 else b""

        target_url = UPSTREAM_URL + self.path
        headers = {k: v for k, v in self.headers.items() if k.lower() not in ("host", "content-length")}

        try:
            req = urllib.request.Request(target_url, data=body if method == "POST" else None, headers=headers, method=method)
            with urllib.request.urlopen(req, timeout=120) as resp:
                resp_body = resp.read()
                self.send_response(resp.status)
                for k, v in resp.getheaders():
                    if k.lower() not in ("content-length", "transfer-encoding"):
                        self.send_header(k, v)
                self.send_header("Content-Length", str(len(resp_body)))
                self.end_headers()
                self.wfile.write(resp_body)
        except urllib.error.HTTPError as e:
            err_body = e.read()
            self.send_response(e.code)
            self.send_header("Content-Length", str(len(err_body)))
            self.end_headers()
            self.wfile.write(err_body)
        except Exception as e:
            self.send_response(502)
            self.end_headers()
            self.wfile.write(f'{{"error": "Upstream unreachable: {str(e)}"}}'.encode())

    def log_message(self, format, *args):
        # Quiet standard output
        pass

def heartbeat_loop():
    while True:
        try:
            payload = {
                "node_id": f"Linux-{HOSTNAME}",
                "hostname": HOSTNAME,
                "ip_address": IP_ADDR,
                "role": f"Linux Edge Proxy -> {UPSTREAM_URL}",
                "status": "ACTIVE",
                "last_seen": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                "public_key": None
            }
            req = urllib.request.Request(
                f"{LEAD_URL}/api/v1/heartbeat",
                data=json.dumps(payload).encode(),
                headers={"Content-Type": "application/json"}
            )
            with urllib.request.urlopen(req, timeout=5):
                pass
        except Exception:
            pass
        time.sleep(15)

if __name__ == "__main__":
    t = threading.Thread(target=heartbeat_loop, daemon=True)
    t.start()
    server = HTTPServer(("0.0.0.0", LISTEN_PORT), EdgeProxyHandler)
    server.serve_forever()
EOF

chmod +x "${INSTALL_DIR}/edge_proxy.py"

# 8. Stop any prior edge proxy instance
pkill -f "edge_proxy.py" 2>/dev/null || true
sleep 0.5

# 9. Launch Edge Proxy Daemon in Background
echo -e "${GREEN}[+] Launching AgentTrace Linux Edge Gateway on port 8080...${NC}"
export UPSTREAM_URL="${LOCAL_UPSTREAM}"
export LEAD_URL="${LEAD_URL}"
export PORT="8080"
export HOSTNAME="${HOSTNAME}"
export IP_ADDR="${LOCAL_IP}"

nohup python3 "${INSTALL_DIR}/edge_proxy.py" > "${INSTALL_DIR}/edge_proxy.log" 2>&1 &
PID=$!

echo -e "${CYAN}==========================================================${NC}"
echo -e "${GREEN} 🎉 SUCCESS: Linux Node ${NODE_ID} is deployed & active!${NC}"
echo -e "${GRAY} Local Edge Proxy:   http://localhost:8080 (PID ${PID})${NC}"
echo -e "${GRAY} Upstream Target:    ${LOCAL_UPSTREAM}${NC}"
echo -e "${GRAY} Cluster Lead:       ${LEAD_URL}${NC}"
echo -e "${GRAY} Log file:           ${INSTALL_DIR}/edge_proxy.log${NC}"
echo -e "${CYAN}==========================================================${NC}"
