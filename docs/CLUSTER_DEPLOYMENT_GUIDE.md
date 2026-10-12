# 🌐 AgentTrace Cluster Deployment & Orchestration Guide

Complete guide for deploying, discovering, and orchestrating distributed AI agent clusters across multiple local Windows PCs.

---

## 1. Cluster Topology Overview

```
                 +-------------------------------------------------------------+
                 |                  PC-1: LEAD NODE (LEAD-NODE)                |
                 |  IP: 192.168.1.101                                          |
                 |  Roles:                                                     |
                 |   - Antigravity IDE (Gemini Cloud RPC)                      |
                 |   - Local Ollama (Gemma 4 :11434)                           |
                 |   - Local Hermes CLI                                        |
                 |   - Central AgentTrace Gateway (:8080)                      |
                 |   - Native Antigravity Session Watcher (Auto-Ingest)        |
                 |   - Master Cryptographic Ledger (SQLite WAL)                |
                 +-------------------------------------------------------------+
                                                │
                                                ▼  LAN (192.168.1.0/24)
                 +-------------------------------------------------------------+
                 |                  PC-2: WORKER NODE (CUDA)                   |
                 |  IP: 192.168.1.102                                          |
                 |  Roles:                                                     |
                 |   - hermes-agent (:8642 with NVIDIA GPU acceleration)       |
                 |   - Local Edge Gateway (:8080) (Eliminates Hairpin Route)   |
                 |   - Background Telemetry Sync to Lead Node                  |
                 +-------------------------------------------------------------+
```

---

## 2. The Unified Setup Wizard (`Setup-Cluster.ps1`)

On **PC-1 (Lead Node)**, open PowerShell in the `agent_trace_gw` repository root and run:

```powershell
.\Setup-Cluster.ps1
```

### Modes Supported:
1. **Express Single-Node Setup (~30 seconds):**
   - Ideal if you only have one PC right now or want a fast standalone workstation setup.
   - Run directly with:
     ```powershell
     .\Setup-Cluster.ps1 -Express
     ```
   - Verifies/starts Ollama on `:11434` and verifies the target model (`gemma4:latest`).
   - Verifies the Hermes CLI (`hermes.cmd`).
   - Starts the AgentTrace Gateway daemon on `:8080` and verifies the SHA-256 cryptographic ledger.
   - Saves state to `cluster_config.json` and launches the live dashboard.

2. **Resumable Cluster Orchestrator (Multi-Node & Adding Nodes Later):**
   - The installer saves all topology and environment state in `cluster_config.json`.
   - When you re-launch `.\Setup-Cluster.ps1` later, it **resumes immediately**:
     - Displays current lead node & worker node health.
     - Lets you **Add / Connect Another Worker Node** at any time.
     - Includes a built-in **Local LLM & Model Manager** (pull models, test latency).
     - Includes a built-in **Hermes Agent Manager** (test offline execution, test remote offloading to PC-2).
     - Provides live WinRM diagnostics and 1-click push deployment.

---

## 3. PowerShell Remoting (WinRM): What, Why, and How

### A. What is WinRM?
**Windows Remote Management (WinRM)** is Microsoft's implementation of the WS-Management protocol. It runs over TCP port **5985** (HTTP) and **5986** (HTTPS).
Think of it as **Windows' native equivalent of SSH**. It allows one Windows computer to run commands on another Windows computer over PowerShell via:

```powershell
Invoke-Command -ComputerName 192.168.1.102 -ScriptBlock { ... }
```

### B. Why Use It for Agent Clusters?
- **Zero-Touch Push Deployment:** You sit at **PC-1 (Lead Node)** and push software, start services, and configure agents on **PC-2** without touching PC-2's keyboard or using flash drives.
- **Automated Updates:** The Lead Node can roll out new gateway binaries or pull new Ollama models to all worker nodes simultaneously.

### C. Why is it Stopped by Default?
For security, modern Windows installs disable WinRM out-of-the-box. Furthermore, on home or workgroup LANs (machines not joined to an Active Directory domain), Windows blocks remote connections unless the target IP is added to the **TrustedHosts** list.

### D. How to Enable It in 2 Simple Steps

#### Step 1: Run on the Worker PC (PC-2)
Open **PowerShell as Administrator** on PC-2 and run:

```powershell
# 1. Start WinRM and configure firewall rules
Enable-PSRemoting -Force

# 2. Allow connections from other workgroup computers on the LAN
Set-Item WSMan:\localhost\Client\TrustedHosts -Value "*" -Force

# 3. Restart the WinRM service
Restart-Service WinRM
```

#### Step 2: Run on the Lead PC (PC-1)
Open **PowerShell as Administrator** on PC-1 and run:

```powershell
# Trust all IPs on your local subnet
Set-Item WSMan:\localhost\Client\TrustedHosts -Value "192.168.1.*" -Force
```

### E. How to Push Deploy from PC-1
Once enabled, you can run silent deployment directly from PC-1:

```powershell
Invoke-Command -ComputerName 192.168.1.102 -Credential (Get-Credential) -ScriptBlock {
    irm http://192.168.1.101:8080/install.ps1 | iex
}
```

---

## 4. Alternative: Zero-Config Web Pairing (No WinRM Required)

If you do not want to configure WinRM or don't have admin rights on the worker PC, you can deploy in 5 seconds using the Lead Gateway's self-serving web installer.

### Profile A: Fast Edge Gateway (For PC-2 with existing Hermes)
On PC-2, open any standard PowerShell terminal and run:

```powershell
irm http://192.168.1.101:8080/install.ps1 | iex
```

**What happens automatically:**
1. Downloads `agent_trace_gw.exe` directly from PC-1 over the LAN.
2. Detects local `hermes-agent` on port `8642`.
3. Registers node heartbeat with the Lead Gateway (`POST /api/v1/heartbeat`).
4. Launches the local edge proxy on port `8080`.

### Profile B: Full Stack Provisioning (For Clean / Fresh PCs)
On any new machine added to your lab without AI software:

```powershell
irm http://192.168.1.101:8080/provision.ps1 | iex
```

**What happens automatically:**
1. Downloads and installs **Ollama for Windows** (`/silent`).
2. Pulls **Gemma 4** (`ollama pull gemma4:latest`).
3. Downloads and starts the **AgentTrace Gateway** edge proxy.
4. Auto-registers into the Lead Node cluster topology.

---

## 5. Eliminating the "Hairpin Route"

### Without Local Gateway on PC-2 (Hairpin):
- PC-2 agent makes call -> Sends packet to PC-1 (:8080) -> PC-1 proxies packet back to PC-2 (:8642).
- **Penalty:** 2 unnecessary LAN round-trips and dependency on PC-1's network stack.

### With Local Edge Gateway on PC-2:
- PC-2 agent makes call -> Hits PC-2 `localhost:8080` -> Proxies immediately to `localhost:8642` (**0 LAN hops**, sub-millisecond).
- Edge Gateway captures telemetry in local SQLite WAL and replicates block summaries asynchronously to PC-1.

---

## 6. Verifying Cluster Health & Proof of Execution

- **Web Dashboard:** [http://localhost:8080/dashboard](http://localhost:8080/dashboard)
  - Real-time event inspector (prompts, model completions, token counts, diffs).
  - Multi-node topology flowchart.
- **Cryptographic Ledger API:**
  ```powershell
  curl http://localhost:8080/api/v1/ledger/verify
  ```
  Returns `is_valid: true`, block count, and SHA-256 block hash.
- **Cluster Summary API:**
  ```powershell
  curl http://localhost:8080/api/v1/summary
  ```
  Reports active node inventory, files touched, commands run, and blast radius scores.
