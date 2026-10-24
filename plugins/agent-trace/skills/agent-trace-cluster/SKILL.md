---
name: agent-trace-cluster
description: Inspect, deploy, bootstrap, and coordinate distributed AI agent clusters across LAN machines (PC-1 Lead Node, PC-2 Worker Node) with zero-walk join scripts, health diagnostics, and prompt routing.
---

# AgentTrace Cluster Management & Multi-Node Orchestration Skill

Use this skill when managing cluster nodes, bootstrapping edge machines, running connectivity diagnostics, or configuring workgroups.

## Cluster Node Endpoints

- **Lead Node (PC-1)**:
  - AgentTrace Gateway: `http://localhost:8080` (or `http://192.168.1.101:8080`)
  - Ollama Engine: `http://127.0.0.1:11434` (`qwen3.8:27b`, `qwen3:14b`)
- **Worker Node (PC-2)**:
  - AgentTrace Gateway (Worker Mode): `http://192.168.1.102:8080`
  - Hermes Agent / Ollama CUDA: `http://192.168.1.102:8642` (`deepseek-r1:1.5b`)

## Autonomous Cluster Health & Sleep Mode Watchdog

AgentTrace includes an autonomous background watchdog that probes all registered LAN nodes every 10 seconds:
- **Port Probing**: Tests `:8080` (AgentTrace Gateway), `:8642` (Hermes / CUDA), and `:11434` (Ollama) with non-blocking sub-second TCP checks.
- **Sleep Mode Detection**:
  - `Online`: Gateway and LLM engine responding.
  - `Degraded`: One service active, but another down (e.g. LLM running, gateway offline).
  - `Sleeping`: Node was previously active, but failed 1–3 probe cycles consecutively (identifies Windows Modern Standby / Sleep mode).
  - `Offline`: Host unreachable for $\ge 4$ consecutive probe cycles.
- **Auto-Failover**: Sleeping and Offline nodes are automatically flagged (`is_healthy = false`) in the Prompt Hub router so requests do not deadlock or timeout.

### Tool & API Endpoints:
- **MCP Tool**: `check_cluster_health` (returns live health report of all cluster nodes with latency and sleep status)
- **REST Endpoints**:
  - `GET /api/v1/cluster/health`: Live health status of all nodes
  - `POST /api/v1/cluster/health/probe`: Triggers an immediate, synchronous cluster-wide probe cycle
- **Web Dashboard**: Interactive `[⚡ Probe Health]` button at `http://localhost:8080/dashboard`

## Zero-Walk Node Bootstrap & Join Methods

1. **Instant Web UI Join**:
   - Open `http://<lead-ip>:8080/join` on any machine in the LAN for copy-paste bootstrap commands.

2. **Command-Line Bootstrap**:
   - Windows 1-liner:
     ```powershell
     irm http://192.168.1.101:8080/join.cmd | iex
     ```
   - Direct worker daemon launcher:
     ```cmd
     curl -s http://192.168.1.101:8080/worker.cmd -o worker.cmd && worker.cmd
     ```

3. **Remote Management (WinRM)**:
   - On PC-2 (Worker), run once as Administrator:
     ```cmd
     irm http://192.168.1.101:8080/winrm.cmd | iex
     ```
   - From PC-1, push commands remotely via:
     ```powershell
     Invoke-Command -ComputerName 192.168.1.102 -ScriptBlock { Get-Process agent_trace_gw }
     ```

4. **Cluster Control Workgroup (`wg-cluster-ops`)**:
   - Built-in multi-agent pipeline for cluster operations:
     - Lead Architect: `qwen3.8:27b` (PC-1)
     - Safe Tool Executor: `hermes-agent` / Copilot CLI (PC-2)
     - Sanity Verifier: `deepseek-r1:1.5b` CUDA (PC-2)
   - Dispatches tasks across the LAN without opening unrestricted remote shells.
