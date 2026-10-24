# 👥 How to Use Problem Workgroups, Attach `.MD` Rules & 5 Real-World Engineering Use Cases

> **Generated & Verified via AgentTrace Workgroup `wg-reasoning-debug`** (`DeepSeek-R1 1.5B CUDA` on `PC-2` $\rightarrow$ `Qwen 3.5 9B` on `PC-1`, Ledger Events `hub-37f4f9459d65` & `hub-7075252a211a`).

Instead of manually deciding which agent (`qwen3.8:27b`, `qwen3.5:9b`, `deepseek-r1:1.5b`, `copilot-cli`, `gemini-pro`) should handle a task—or repeating your coding and security rules in every prompt—**AgentTrace Problem Workgroups** let you graphically bind:
1. **A Problem Domain** (e.g., *Image Generation*, *Coding & Architecture*, *Reasoning & Coding Issues*, *Security & Zone Audit*, *Work-First TUI*)
2. **An Assigned Multi-Agent Roster** with explicit roles (`LeadArchitect`, `PrimaryBuilder`, `SanityVerifier`, `ToolExecutor`)
3. **Attached Markdown (`.md`) Rule Files** (from your repository, uploaded from disk, or written inline) that are **automatically compiled and injected** into every prompt dispatched to that workgroup.

---

## 1. Quick Overview of the Built-In Problem Workgroups

| Workgroup ID | Problem Domain | Assigned Agents & Roles | Default Attached `.MD` Rule Files |
| :--- | :--- | :--- | :--- |
| **`wg-image-gen`** | 🎨 **Image & SVG Generation** | **Stage 1**: `DeepSeek-R1 (1.5B CUDA)` (*LeadArchitect*)<br>**Stage 2**: `GitHub Copilot CLI` (*PrimaryBuilder*) | `docs/UI_DESIGN_PHILOSOPHY_AND_PROMPT.md`<br>*(Zero device frames, `#0d1117` dark palette, crisp SVG geometry)* |
| **`wg-coding`** | 💻 **Coding & Architecture** | **Stage 1**: `Qwen 3.5 (9B Code)` (*PrimaryBuilder*)<br>**Stage 2**: `Gemini / Antigravity` (*LeadArchitect*) + `Qwen 3.8 (27B)` (*SanityVerifier*) | `docs/RUST_ENGINEERING_GUIDE_CHANNELS_AND_SSRF.md`<br>*(Anti-Monolith SRP, $\le 3$ args/fn, $\ge 85\%$ test coverage)* |
| **`wg-code-refactor`** | 🔨 **Code Refactor & Architecture** | **Stage 1**: `DeepSeek-R1 (1.5B CUDA)` (*LeadArchitect*)<br>**Stage 2**: `Qwen 3.8 (27B Heavy)` (*PrimaryBuilder*) | `docs/CODE_REFACTOR_AND_ENGINEERING_RULES.md`<br>*(Anti-Monolith, SRP < 800 lines, pure math invariant, zero framework bloat)* |
| **`wg-reasoning-debug`** | 🧠 **Reasoning & Coding Issues** | **Stage 1**: `DeepSeek-R1 (1.5B CUDA)` (*SanityVerifier*)<br>**Stage 2**: `Qwen 3.5 (9B)` / `Qwen 3.8 (27B)` (*LeadArchitect*) + `GitHub Copilot CLI` (*ToolExecutor*) | `docs/RUST_ENGINEERING_GUIDE_CHANNELS_AND_SSRF.md`<br>*(Isolate minimal failure, inspect async lock contention, add regression test)* |
| **`wg-security-audit`** | 🛡️ **Security & Zone Audit** | **Swarm**: `DeepSeek-R1 (1.5B)` + `Qwen 3.5 (9B)` + `GitHub Copilot CLI` | `docs/HOW_TO_SAFE_ZONES_AND_PLAYGROUNDS.md`<br>*(Block `VaultNoAccess`, collapse lexical `..` traversals, RFC 3986 SSRF check)* |
| **`wg-ui-development`** | 🖥️ **UI Development & Ergonomics** | **Stage 1**: `DeepSeek-R1 (1.5B CUDA)` (*SanityVerifier*)<br>**Stage 2**: `Qwen 3.5 (9B Fast Code)` (*PrimaryBuilder*) | `docs/UI_DESIGN_PHILOSOPHY_AND_PROMPT.md`<br>*(High density 30–50 records, scenario toolbars, j/k/P/M/F/W operator tools)* |
| **`wg-documentation`** | 📚 **Documentation & Technical Writing** | **Solo / Pipeline**: `DeepSeek-R1 (1.5B CUDA)` (*LeadArchitect / TechnicalWriter*) | `docs/DOCUMENTATION_STANDARDS.md`<br>*(Line 0 How-To index, feature-per-file modular docs, no personal paths)* |
| **`wg-cluster-ops`** | 🌐 **Cluster Infrastructure & Node Ops** | **Stage 1**: `Qwen 3.8 (27B Heavy)` (*LeadArchitect*)<br>**Stage 2**: `Hermes Agent` / `Copilot CLI` (*ToolExecutor*) + `DeepSeek-R1 (1.5B CUDA)` (*SanityVerifier*) | `rules/rule-cluster-ops.md`<br>*(Zero-walk bootstrap, port sanity check, read-only worker verification, WinRM fallback)* |

---

## 2. How to Create & Customize Workgroups Graphically (`/dashboard`)

1. Open **`http://localhost:8080/dashboard`** and click **`[👥 Workgroups & .MD Rules (W)]`** in the top toolbar (or press <kbd>W</kbd>).
2. **Choose a Template or Start Fresh**:
   - Click **`[🎨 Image Gen (DeepSeek + Copilot CLI)]`**, **`[💻 Coding (Qwen + Gemini + Rules)]`**, **`[🧠 Reasoning & Bugs (DeepSeek + Qwen 27B)]`**, or **`[➕ New Blank Workgroup]`**.
3. **Graphically Assign Agents & Roles**:
   - Click any agent tile in the **Assign Agents to This Problem** grid (`Qwen 3.5 9B`, `Qwen 3.8 27B`, `DeepSeek-R1 1.5B CUDA`, `GitHub Copilot CLI`, `Gemini / Antigravity`, `Hermes Daemon`, `Gemma 4`).
   - Use the dropdown on each active tile to set its role (`PrimaryBuilder`, `LeadArchitect`, `SanityVerifier`, `ToolExecutor`).
4. **Attach Markdown (`.md`) Rule Files (3 Ways)**:
   - **Workspace `.MD` Picker**: Select any discovered `.md` file in your repo (`README.md`, `AGENTS.md`, `docs/*.md`) and click **`[📎 Attach Workspace .MD]`**.
   - **Upload Local `.MD` File**: Click **`[📂 Upload Local .MD File]`** to attach a `.md` file from your workstation.
   - **Inline Markdown Rule**: Enter a title and Markdown bullets in the inline boxes and click **`[➕ Add .MD Rule]`**.
   - *Security Note*: All attached `.md` file paths pass through `zones::resolve_canonical_target`, blocking any `..` path traversal into `.secrets`, `.ssh`, or `.env`.
5. **Preview & Run**:
   - Expand **`🔍 Inspect Compiled System Prompt & Injected .MD Rules Preview`** to verify the exact system prompt.
   - Click **`[💾 Save Workgroup]`** and **`[🚀 Run Task in Group]`**.

---

## 3. Five Real-World Engineering Use Cases (With `.MD` Rule Enforcement)

### Use Case 1: 🎨 Image, SVG & UI Asset Generation (`wg-image-gen`)
- **The Problem**: Generic LLM prompts produce bloated mockups wrapped in fake laptop/phone frames or inconsistent color palettes that clash with dark operator UIs.
- **Assigned Agents**: `DeepSeek-R1 (1.5B CUDA)` refines the geometric specification $\rightarrow$ `GitHub Copilot CLI` synthesizes the deterministic SVG/asset script.
- **Attached `.MD` Rules**: `docs/UI_DESIGN_PHILOSOPHY_AND_PROMPT.md`
  - *Rule Enforced*: Generate only the crisp vector/UI asset itself without surrounding device frames; strictly use `#0d1117` background, `#58a6ff` cyan, `#3fb950` green, and `#bc8cff` purple.
- **CLI Command**:
  ```bash
  python sdk/agent_trace_client.py --workgroup wg-image-gen --ask "Generate a dark-mode SVG topology icon for a GPU worker node"
  ```

---

### Use Case 2: 💻 Multi-File Coding & Anti-Monolith Refactoring (`wg-coding`)
- **The Problem**: Autonomous coding agents often dump 1,200+ lines into a single file or write functions with 7+ parameters that trigger Clippy warnings and heap copies.
- **Assigned Agents**: `Qwen 3.5 (9B)` drafts the implementation $\rightarrow$ `Gemini / Antigravity` & `Qwen 3.8 (27B)` enforce module boundaries and test coverage.
- **Attached `.MD` Rules**: `docs/RUST_ENGINEERING_GUIDE_CHANNELS_AND_SSRF.md` (Chapter 4: Zero-Copy Parameter Structs & Anti-Monolith SRP)

| Code Smell Prevented | Rule-Enforced Correction | Verification |
| :--- | :--- | :--- |
| Monolithic files & `too_many_arguments` (> 3 args) | Feature-per-File modules + lifetime-bound parameter structs (`RawEventFields<'a>`) | `cargo check` zero warnings + $\ge 85\%$ unit test coverage |

- **CLI Command**:
  ```bash
  python sdk/agent_trace_client.py --workgroup wg-coding --ask "Implement a zero-copy telemetry parser struct with unit tests"
  ```

---

### Use Case 3: 🧠 Deep Reasoning & Coding Issue Diagnosis (`wg-reasoning-debug`)
- **The Problem**: Subtle async deadlocks (such as holding a `Mutex` across `.await` points in Tokio), borrow-checker errors, or broken mathematical invariants take hours to diagnose manually.
- **Assigned Agents**: `DeepSeek-R1 (1.5B CUDA)` performs rapid chain-of-thought root-cause isolation $\rightarrow$ `Qwen 3.5 (9B)` / `Qwen 3.8 (27B)` authors the bounded actor fix + `GitHub Copilot CLI` runs the regression test suite.
- **Attached `.MD` Rules**: `docs/RUST_ENGINEERING_GUIDE_CHANNELS_AND_SSRF.md` (Chapter 1: Async Lock Contention & Chapter 5: Deterministic Testing)

| Before `.MD` Rule Enforcement | After `wg-reasoning-debug` Enforcement | Risk Eliminated |
| :--- | :--- | :--- |
| `Arc<Mutex<Connection>>` held across `.await` boundaries | Bounded `tokio::sync::mpsc` actor channel (capacity `1024`, batch drain `32`) | **Tokio Worker Starvation & Deadlocks** |
| Flaky network-bound tests binding port `:8080` | In-memory `tower::ServiceExt::oneshot` regression test | **CI Port Collisions** |

- **CLI Command**:
  ```bash
  python sdk/agent_trace_client.py --workgroup wg-reasoning-debug --ask "Diagnose why our SQLite writer blocks Tokio worker threads under burst load and write the bounded mpsc fix"
  ```

---

### Use Case 4: 🛡️ Security Forensics, SSRF & Spatial Zone Auditing (`wg-security-audit`)
- **The Problem**: Naive `url.starts_with("http://127.0.0.1")` checks are bypassed by prefix collisions (`http://127.0.0.1.evil.com`), and string whitelists are bypassed by `../.secrets` path traversal.
- **Assigned Agents**: Parallel swarm across `DeepSeek-R1 (1.5B)` + `Qwen 3.5 (9B)` + `GitHub Copilot CLI`.
- **Attached `.MD` Rules**: `docs/HOW_TO_SAFE_ZONES_AND_PLAYGROUNDS.md` & `docs/RUST_ENGINEERING_GUIDE_CHANNELS_AND_SSRF.md` (Chapter 3: RFC 3986 Origin Security)

| Vulnerable Pattern | Rule-Enforced Fix | Attack Blocked |
| :--- | :--- | :--- |
| `url.starts_with("http://example.com")` | Canonical `url::Url::origin()` tuple `(scheme, host, port)` comparison | **SSRF Subdomain Prefix Collision** |
| Unchecked path `./playground/../../.secrets/keys.env` | Lexical `resolve_canonical_target` + `VaultNoAccess` precedence | **Credential Vault Escape** |

- **CLI Command**:
  ```bash
  python sdk/agent_trace_client.py --workgroup wg-security-audit --ask "Audit this URL validator and file reader for SSRF and path traversal bypasses"
  ```

---

### Use Case 5: 🖥️ Work-First Operator UI & TUI Engineering (`wg-ui-development`)
- **The Problem**: AI agents default to generating SaaS marketing fluff (giant gradient cards, huge padding, 4 rows visible per screen, and no keyboard navigation).
- **Assigned Agents**: `DeepSeek-R1 (1.5B CUDA)` (*SanityVerifier*) $\rightarrow$ `Qwen 3.5 (9B Fast Code)` (*PrimaryBuilder*).
- **Attached `.MD` Rules**: `docs/UI_DESIGN_PHILOSOPHY_AND_PROMPT.md`
  - *Rule Enforced*: 30–50 visible records without scrolling, monospace operator aesthetics, `[⚡ Live Scenario Demos]` toolbar, and single-key shortcuts (`j`/`k`, `P`, `M`, `F`, `H`, `W`, `?`).
- **CLI Command**:
  ```bash
  python sdk/agent_trace_client.py --workgroup wg-ui-development --ask "Design a compact keyboard-driven alert triage table row with j/k selection"
  ```

---

### Use Case 6: 🔨 Code Refactoring & Anti-Monolith Architecture (`wg-code-refactor`)
- **The Problem**: Codebases degrade over time into monolithic 1,000+ line "god objects" with tangled UI, math, and engine side-effects.
- **Assigned Agents**: `DeepSeek-R1 (1.5B CUDA)` (*LeadArchitect*) $\rightarrow$ `Qwen 3.8 (27B Heavy)` (*PrimaryBuilder*).
- **Attached `.MD` Rules**: `docs/CODE_REFACTOR_AND_ENGINEERING_RULES.md`
  - *Rule Enforced*: Decompose files $> 800$ lines into SRP modules; feature-per-file architecture; pure math library invariants (zero framework dependencies); headless invariant tests ($\chi = V - E + F = 2$, watertightness, volume divergence).
- **CLI Command**:
  ```bash
  python sdk/agent_trace_client.py --workgroup wg-code-refactor --ask "Refactor this file into single-responsibility modules and extract pure math algorithms"
  ```

---

### Use Case 7: 📚 Documentation & Technical Writing (`wg-documentation`)
- **The Problem**: Documentation drifts from reality, buries quick starts under marketing fluff, hardcodes machine-specific local paths, or reports outdated test metrics.
- **Assigned Agents**: `DeepSeek-R1 (1.5B CUDA)` (*Lead Technical Writer*).
- **Attached `.MD` Rules**: `docs/DOCUMENTATION_STANDARDS.md`
  - *Rule Enforced*: Root README must place the How-To table and 30s quick recipes on Line 0; feature-per-file modular docs in `docs/`; zero personal paths; every guide must follow Problem $\rightarrow$ Command $\rightarrow$ Concrete Proof; test badges must reflect live test suites (87+ Passed).
- **CLI Command**:
  ```bash
  python sdk/agent_trace_client.py --workgroup wg-documentation --ask "Audit and update the How-To index on Line 0 of README.md"
  ```

---

### Use Case 8: 🌐 Cluster Infrastructure & Remote Node Ops (`wg-cluster-ops`)
- **The Problem**: Configuring edge worker nodes manually requires walking between machines, transferring tokens via email/messengers, or opening insecure, unauthenticated remote command-lines.
- **Assigned Agents**: `Qwen 3.8 (27B Heavy)` (*LeadArchitect*) plans deployment $\rightarrow$ `Hermes Agent` / `Copilot CLI` (*ToolExecutor*) executes zero-walk commands $\rightarrow$ `DeepSeek-R1 (1.5B CUDA)` (*SanityVerifier*) audits service health on `:8080` & `:8642`.
- **Attached `.MD` Rules**: `rules/rule-cluster-ops.md`
  - *Rule Enforced*: Zero-walk bootstrap scripts (`/join.cmd`, `/worker.cmd`, `/winrm.cmd`); verify worker telemetry proxying to Lead Node; safe LAN execution without unrestricted shell elevation.
- **CLI Command**:
  ```bash
  python sdk/agent_trace_client.py --workgroup wg-cluster-ops --ask "Verify PC-2 worker node connectivity and run health check on port 8080 and 8642"
  ```

---

## 🔗 Related Guides
- [How to Use the Central Prompt Hub & Contribution Graph (`docs/HOW_TO_PROMPT_HUB_AND_GRAPH.md`)](./HOW_TO_PROMPT_HUB_AND_GRAPH.md)
- [How to Configure Safe Zones & Agent Playgrounds (`docs/HOW_TO_SAFE_ZONES_AND_PLAYGROUNDS.md`)](./HOW_TO_SAFE_ZONES_AND_PLAYGROUNDS.md)
- [Manual-First Operator Guide (`docs/MANUAL_FIRST_OPERATOR_GUIDE.md`)](./MANUAL_FIRST_OPERATOR_GUIDE.md)
- [Code Refactoring & Engineering Rules (`docs/CODE_REFACTOR_AND_ENGINEERING_RULES.md`)](./CODE_REFACTOR_AND_ENGINEERING_RULES.md)
- [Documentation Standards (`docs/DOCUMENTATION_STANDARDS.md`)](./DOCUMENTATION_STANDARDS.md)
