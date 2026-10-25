# ⚡ How to Run & Verify the 7 Live Security, Admin Governance & CAS Scenarios

AgentTrace Gateway is built around **Scenario-First Verification**: you don't have to take anyone's word that it blocks credential leaks, enforces per-model Admin restrictions, shields the Claude Code terminal, or restores deleted files in `< 2 ms`. You can verify all **7 live scenarios** automatically via CLI or interactively in the UI.

---

## ⚡ Automated 1-Command Verification (Recommended for Agents & Engineers)

With `agent_trace_gw` running on `http://localhost:8080`, run the automated live scenario harness:

```bash
python scripts/run_live_unhappy_and_security_scenarios.py
```

This script executes all **7 live adversarial & unhappy scenarios** against the running Gateway and verifies:
1. **`[Scenario 1]` Live DLP Secret Exfiltration `HardBlock`**: Attempts to send an Anthropic key (`sk-ant-api03-...`) through `/v1/chat/completions` in `hard` mode $\rightarrow$ physically blocked (`HTTP 403`).
2. **`[Scenario 2]` Live Destructive Shell Command Interception**: Submits `rm -rf / --no-preserve-root` to `/api/v1/guard/intercept` $\rightarrow$ blocked in **`~1.1 ms`** (`GUARD-CMD-001-ROOT-WIPE` + `GUARD-ZONE-EXEC`).
3. **`[Scenario 3]` Live `RedactAndForward` Mode Sanitization**: Sends an AWS IAM key (`AKIAIOSFODNN7EXAMPLE`) in `RedactAndForward` mode $\rightarrow$ scrubbed in-flight to `[REDACTED_API_KEY_BY_AGENTTRACE_GUARD]` (`HTTP 200`) without interrupting the session.
4. **`[Scenario 4]` Live Distributed Cluster State Parity (`PC-1` $\leftrightarrow$ `PC-2`)**: Queries `/api/v1/ledger/verify` across both Lead (`127.0.0.1:8080`) and Worker (`192.168.1.102:8080`) nodes $\rightarrow$ confirms 100% block height and SHA-256 hash parity.
5. **`[Scenario 5]` Autonomous Cluster Health Watchdog**: Queries `/api/v1/cluster/health` $\rightarrow$ verifies `PC-2` Worker Node is `ONLINE`.
6. **`[Scenario 6]` Live Pre-Image CAS Snapshot, Rogue Deletion Recovery & 35 MCP Tools**: Snapshots a temporary SQL schema via `POST /api/v1/cas/protect`, deletes the file from disk, triggers `POST /api/v1/cas/enforce` $\rightarrow$ **recreates the deleted file on disk in `~1.1 ms`** with exact SHA-256 parity and verifies all **35 native MCP tools**.
7. **`[Scenario 7]` Live Admin Per-Model Governance & Shielded Claude Terminal Client**:
   - Verifies `/api/v1/client/shield-status` exposes zero internal regexes to the client.
   - Verifies `/api/v1/admin/governance/evaluate` blocks `deepseek-r1:1.5b` (1.5B worker) from running shell commands or writing to `src/main.rs` (`HTTP 403`), while allowing `qwen3.8:27b` (sovereign lead model) (`HTTP 200`).
   - Sends a restricted `bash` tool call to `POST /v1/messages` (Claude Code Terminal endpoint) $\rightarrow$ verifies the terminal receives a graceful, shielded Anthropic response (`🛡️ [AgentTrace Shielded Terminal]`) so the user's CLI session never crashes.

---

## 🖥️ Interactive One-Click Verification in the Web UI

1. Open **`http://localhost:8080/dashboard`** or **`http://localhost:8080/console`**.
2. At the top of the screen, locate the **`[⚡ Live Scenario Demos]`** bar.
3. Click any scenario button:
   - The gateway executes the real interception pipeline against its policy engine and cryptographic ledger.
   - An interactive **3-Part Scenario Explainer Card** opens showing:
     1. **The Problem Solved** (what goes wrong in unmonitored agent setups)
     2. **How AgentTrace Intercepted It** (exact rule, hash, or containment boundary triggered)
     3. **Live Visual Proof** (colorized unified code diff, blocked payload, or Ed25519 signature)

---

## 🛠️ Manual `curl` Commands for Each Capability

### 1. Admin Per-Model Governance vs. Shielded Client (`src/admin_governance.rs`)
```bash
# Admin: View global restrictions and per-model profiles (claude-*, gpt-*, qwen3.8:27b, deepseek-r1:1.5b)
curl -s http://localhost:8080/api/v1/admin/governance

# Admin: Evaluate a command & file write against the 1.5B sandbox worker profile (Blocked!)
curl -X POST http://localhost:8080/api/v1/admin/governance/evaluate \
  -H "Content-Type: application/json" \
  -d '{"model": "deepseek-r1:1.5b", "command": "cargo test", "target_path": "src/main.rs"}'

# Client: Query the shielded terminal status (zero internal rules exposed)
curl -s http://localhost:8080/api/v1/client/shield-status
```

### 2. Pre-Image CAS File Protection & Deletion Recovery (`src/cas_guard.rs`)
```bash
# Protect a file in memory:
curl -X POST http://localhost:8080/api/v1/cas/protect \
  -H "Content-Type: application/json" \
  -d '{"path": "src/main.rs", "strictly_guarded": true}'

# Enforce / auto-restore if modified or deleted (< 2 ms):
curl -X POST http://localhost:8080/api/v1/cas/enforce \
  -H "Content-Type: application/json" \
  -d '{"path": "src/main.rs"}'
```

### 3. Synchronous Hard AI Guard Interception (`src/ai_guard.rs`)
```bash
curl -X POST http://localhost:8080/api/v1/guard/intercept \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "claude-code:terminal@DEV-WS01#pid4812",
    "action": "Execute",
    "command": "rm -rf / --no-preserve-root",
    "prompt_text": "Deploy with sk-ant-api03-abcdefgh1234567890ijklmnop"
  }'
```

### 4. Cryptographic Ledger Verification (`src/ledger.rs`)
```bash
curl -s http://localhost:8080/api/v1/ledger/verify
```

---

## 🔗 Related Guides
- [Native Rust MCP Server Guide (35 Tools) (`docs/MCP_SERVER_GUIDE.md`)](./MCP_SERVER_GUIDE.md)
- [How to Configure Safe Zones & Agent Playgrounds (`docs/HOW_TO_SAFE_ZONES_AND_PLAYGROUNDS.md`)](./HOW_TO_SAFE_ZONES_AND_PLAYGROUNDS.md)
- [Manual-First Operator Guide (`docs/MANUAL_FIRST_OPERATOR_GUIDE.md`)](./MANUAL_FIRST_OPERATOR_GUIDE.md)
