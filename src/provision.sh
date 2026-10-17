#!/usr/bin/env bash
# ==============================================================================
# AgentTrace Full Stack Linux Provisioning Script
# Installs Ollama, pulls target LLM (Gemma 4), and deploys the Edge Gateway
# ==============================================================================

set -e

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
GRAY='\033[0;90m'
NC='\033[0m'

echo -e "${CYAN}==========================================================${NC}"
echo -e "${CYAN} 🌐 AGENTTRACE FULL-STACK LINUX PROVISIONER${NC}"
echo -e "${CYAN}==========================================================${NC}"

LEAD_HOST="${LEAD_HOST:-127.0.0.1}"
LEAD_PORT="${LEAD_PORT:-8080}"
LEAD_URL="http://${LEAD_HOST}:${LEAD_PORT}"
TARGET_MODEL="${TARGET_MODEL:-gemma4:latest}"

# 1. Install Ollama if not present
if ! command -v ollama >/dev/null 2>&1; then
    echo -e "${YELLOW}[*] Installing Ollama for Linux (GPU-accelerated)...${NC}"
    curl -fsSL https://ollama.com/install.sh | sh
    echo -e "${GREEN}[+] Ollama installed successfully!${NC}"
else
    echo -e "${GREEN}[+] Ollama is already installed.${NC}"
fi

# 2. Ensure Ollama service is running
echo -e "${CYAN}[*] Ensuring Ollama service is active...${NC}"
if command -v systemctl >/dev/null 2>&1; then
    sudo systemctl start ollama 2>/dev/null || true
fi

# Fallback: start daemon if port 11434 is not open
if ! nc -z 127.0.0.1 11434 2>/dev/null && ! (exec 3<>/dev/tcp/127.0.0.1/11434) 2>/dev/null; then
    echo -e "${YELLOW}[*] Starting Ollama serve daemon in background...${NC}"
    nohup ollama serve > /tmp/ollama.log 2>&1 &
    sleep 3
fi

# 3. Pull Target Model
echo -e "${CYAN}[*] Pulling model ${TARGET_MODEL} (this may take a few moments)...${NC}"
ollama pull "${TARGET_MODEL}"
echo -e "${GREEN}[+] Model ${TARGET_MODEL} ready for zero-token inference!${NC}"

# 4. Run Edge Gateway Installer
echo -e "${CYAN}[*] Connecting node to AgentTrace Lead Mesh...${NC}"
curl -sSL "${LEAD_URL}/install.sh" | bash
