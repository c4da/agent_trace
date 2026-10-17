# 🧠 How to Use Problem Workgroups, `.MD` Rules, the Central Prompt Hub & Contribution Graph

Stop manually deciding which node IP (`127.0.0.1:11434` vs `127.0.0.1:8642`) or which model (`qwen3.8:27b` vs `qwen3.5:9b` vs `deepseek-r1:1.5b` vs `copilot-cli`) should handle each task—graphically define **Problem Workgroups**, attach **Markdown (`.md`) rule files**, and inspect each agent's contribution in `/graph`.

---

## Part 0: Graphical Problem Workgroups & `.MD` Rules Studio (`/dashboard`)

You can graphically group specific cluster agents around a problem domain and attach `.md` rule files that are automatically compiled and injected into every prompt sent to that workgroup:

| Preset Workgroup ID | Problem Domain | Assigned Agent Roster | Default Attached `.MD` Rule Files |
| :--- | :--- | :--- | :--- |
| **`wg-image-gen`** | 🎨 **Image & SVG Generation** | `DeepSeek-R1 (1.5B CUDA)` (`LeadArchitect`) $\rightarrow$ `GitHub Copilot CLI` (`PrimaryBuilder`) | `docs/UI_DESIGN_PHILOSOPHY_AND_PROMPT.md` (*No device frames, dark `#0d1117` palette, crisp SVG geometry*) |
| **`wg-coding`** | 💻 **Coding & Architecture** | `Qwen 3.5 (9B)` (`PrimaryBuilder`) $\rightarrow$ `Gemini / Antigravity` (`LeadArchitect`) + `Qwen 3.8 (27B)` (`SanityVerifier`) | `docs/RUST_ENGINEERING_GUIDE_CHANNELS_AND_SSRF.md` (*Anti-Monolith SRP, $\le 3$ args, $\ge 85\%$ test coverage*) |
| **`wg-reasoning-debug`** | 🧠 **Reasoning & Coding Issues** | `DeepSeek-R1 (1.5B CUDA)` (`SanityVerifier`) $\rightarrow$ `Qwen 3.8 (27B Heavy)` (`LeadArchitect`) + `GitHub Copilot CLI` (`ToolExecutor`) | `docs/RUST_ENGINEERING_GUIDE_CHANNELS_AND_SSRF.md` (*Root-cause bug diagnosis, borrow-checker/deadlock fixes & regression tests*) |
| **`wg-security-audit`** | 🛡️ **Security & Zone Audit** | `DeepSeek-R1 (1.5B)` + `Qwen 3.5 (9B)` + `GitHub Copilot CLI` | `docs/HOW_TO_SAFE_ZONES_AND_PLAYGROUNDS.md` (*Vault protection & lexical `..` traversal collapse*) |
| **`wg-ui-workfirst`** | 🖥️ **Work-First UI & TUI** | `Gemini / Antigravity` + `Qwen 3.5 (9B)` + `DeepSeek-R1 (1.5B)` | `docs/UI_DESIGN_PHILOSOPHY_AND_PROMPT.md` (*30–50 rows visible, keyboard-first `j`/`k`/`P`/`M`/`F`*) |

### How to Create or Edit a Workgroup Graphically in the UI
1. Open **`http://localhost:8080/dashboard`** and click **`[👥 Workgroups & .MD Rules (W)]`** in the top toolbar.
2. Click **`[➕ New Blank Workgroup]`** (or select an existing card on the left).
3. **Assign Agents Graphically**: Click the agent tiles (`Qwen 3.5 9B`, `Qwen 3.8 27B`, `DeepSeek-R1 1.5B CUDA`, `GitHub Copilot CLI`, `Gemini / Antigravity`, `Hermes Daemon`) to toggle them into the group and select each agent's role (`PrimaryBuilder`, `LeadArchitect`, `SanityVerifier`, `ToolExecutor`).
4. **Attach `.MD` Rule Files (3 Ways)**:
   - **Workspace `.md` Dropdown**: Pick any discovered `.md` file in your repo and click **`[📎 Attach Workspace .MD]`**.
   - **Upload Local `.md` File**: Click **`[📂 Upload Local .MD File]`** to attach a `.md` file directly from your computer.
   - **Inline Markdown Rule**: Type or paste Markdown rules directly and click **`[➕ Add .MD Rule]`**.
5. Click **`[💾 Save Workgroup]`**, inspect the live **`🔍 Compiled System Prompt & Injected .MD Rules Preview`**, and click **`[🚀 Run Task in Group]`**!

### How to Manage Workgroups & `.MD` Rules via CLI / REST API
```bash
# List all Problem Workgroups, assigned agents, and discovered workspace .md files
python sdk/agent_trace_client.py --workgroups

# Attach a .md rule file to an existing workgroup
python sdk/agent_trace_client.py --attach-rule wg-coding docs/RUST_ENGINEERING_GUIDE_CHANNELS_AND_SSRF.md

# Dispatch a prompt directly to a specific Problem Workgroup (auto-injects its .md rules!)
python sdk/agent_trace_client.py --workgroup wg-image-gen --ask "Create a crisp SVG topology icon for a GPU worker node"
```

---

## Part 1: Central Prompt Hub (`/api/v1/hub/dispatch`)

The **Central Prompt Hub** accepts your natural-language prompt and executes it across your local cluster using one of three orchestration strategies:

| Strategy | How It Works | Best For |
| :--- | :--- | :--- |
| **`auto`** *(Default)* | Classifies your prompt's intent (`FastVerification`, `FastCodeGeneration`, `HeavyArchitecture`, `SecurityAudit`, or `General`), scores online models by tier fitness, and dispatches with automatic failover. | Everyday coding, unit tests, regex checks, and architecture questions without specifying a model. |
| **`pipeline`** | **Stage 1**: Drafts implementation or analysis on PC-1 (`qwen3.5:9b` / `qwen3.8:27b`).<br>**Stage 2**: Automatically pipes the draft to PC-2 (`deepseek-r1:1.5b` CUDA) for chain-of-thought verification and bug auditing. | High-confidence code generation where you want a second reasoning model to audit the output automatically. |
| **`swarm`** | Dispatches your prompt in parallel to both Qwen (PC-1) and DeepSeek-R1 (PC-2) and returns a side-by-side comparison with latency metrics. | Comparing reasoning approaches or benchmarking model consensus on tricky bugs. |

> **Automatic 27B VRAM Hygiene**: Whenever the Hub invokes `qwen3.8:27b`, it automatically issues an Ollama unload request (`keep_alive: 0`) once inference completes so your workstation's VRAM and Windows commit charge stay free for parallel `cargo` builds.

### A. Using the Prompt Hub in the Browser UI (`H` Key)
1. Open **`http://localhost:8080/dashboard`**.
2. Click **`[🧠 Prompt Hub (H)]`** in the top navigation bar (or press **`H`**).
3. Select **`⚡ Auto-Route`**, **`🔗 Pipeline (Draft → Verify)`**, or **`🐝 Parallel Swarm`**, type your prompt, and press **Dispatch**.

### B. Using the Prompt Hub from the Command Line (Python SDK)
```bash
# 1. Auto-route (Gateway picks the optimal model based on prompt intent)
python sdk/agent_trace_client.py --ask "Write a Python function to parse unified git diffs"

# 2. Two-Stage Pipeline (Qwen drafts -> DeepSeek-R1 verifies)
python sdk/agent_trace_client.py --pipeline "Write a thread-safe Rust ring buffer using Mutex"

# 3. Parallel Swarm (Run on both Lead and Worker nodes simultaneously)
python sdk/agent_trace_client.py --swarm "What is the cleanest way to prevent SSRF in Rust reqwest?"
```

### C. Using the Prompt Hub via `curl` / REST API
```bash
curl -X POST http://localhost:8080/api/v1/hub/dispatch \
  -H "Content-Type: application/json" \
  -d '{
    "prompt": "Audit this SQL query for injection risks: SELECT * FROM users WHERE name = '\'' + input + '\''",
    "strategy": "pipeline",
    "agent_name": "cli-operator"
  }'
```

---

## Part 2: Multi-Agent Problem Contribution Graph (`/graph`)

When multiple agents (`Antigravity`, `qwen3.8:27b`, `qwen3.5:9b`, `deepseek-r1:1.5b`, `hermes-agent`) collaborate on a codebase, you need visibility into **who solved what** and **how much each agent contributed** to each problem domain.

### 1. Open the Dedicated Contribution Matrix UI
Navigate to **`http://localhost:8080/graph`** (or click **`[🕸️ /graph]`** in the top header of `/dashboard` or `/console`).

### 2. Switch Between 2 Topic Grouping Modes
- **🗂️ Mode A: Feature / Subsystem Problem Topics (`group_by=feature`)** — *Default*
  Automatically clusters every file edit, command, and security check into engineering problem domains based on target paths and payloads:
  - `Security & Spatial Zones` (`src/zones.rs`, `src/security.rs`, `.secrets`)
  - `Prompt Hub & Model Routing` (`src/hub.rs`, `src/proxy.rs`, `src/registry.rs`)
  - `Cryptographic Audit Ledger` (`src/crypto.rs`, `audit_ledger.jsonl`)
  - `Operator UI & Telemetry Graph` (`src/dashboard.html`, `src/console.html`, `src/graph.html`)
  - `Agent Handoff & Python SDK` (`src/agent_interface`, `sdk/`)
  - `Verification & Test Suites` (`tests/`, `cargo test`)
- **🧠 Mode B: Prompt & Cognitive Role Topics (`group_by=cognitive`)**
  Clusters telemetry by the cognitive work performed across the cluster:
  - `System Architecture & Planning`
  - `Code Implementation & Refactoring`
  - `Reasoning & Sanity Verification`
  - `Test Execution & QA`
  - `Security Auditing & Containment`

### 3. Switch Live Across 4 Contribution Metrics (Keys `1`–`4`)
Press **`1`**, **`2`**, **`3`**, or **`4`** in `/graph` to re-rank the bipartite network graph and 100% stacked contribution bars in real time:
1. **`[1] Weighted Impact Score`** *(Composite Engineering Formula)*:
   $$\text{Score} = (3.0 \times \text{Diffs}) + (2.0 \times \text{Commands}) + (2.5 \times \text{Security Audits}) + (1.5 \times \text{Prompts}) + 0.5 \times \ln(1 + \text{DiffLines})$$
2. **`[2] Action Touchpoints`**: Raw count of tool invocations, file mutations, and verifications per problem.
3. **`[3] Compute Time (ms)`**: Cumulative inference and execution milliseconds invested per problem.
4. **`[4] Code Diff Volume`**: Total lines of code added/modified (`+`/`-`) in `FileDiff` events.

### 4. Query the Contribution Matrix via REST API
```bash
# Group by Feature / Subsystem Problems (Default)
curl -s "http://localhost:8080/api/v1/telemetry/contributions?group_by=feature" | python -m json.tool

# Group by Prompt & Cognitive Role Topics
curl -s "http://localhost:8080/api/v1/telemetry/contributions?group_by=cognitive" | python -m json.tool
```

---

## 🔗 Related Guides
- [Agent Telemetry Pickup & MCP Integration (`docs/AGENT_TELEMETRY_PICKUP.md`)](./AGENT_TELEMETRY_PICKUP.md)
- [Manual-First Operator Guide (`docs/MANUAL_FIRST_OPERATOR_GUIDE.md`)](./MANUAL_FIRST_OPERATOR_GUIDE.md)
