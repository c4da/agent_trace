# 🏰 How to Configure Safe Zones (Vault / Read-Only) & Agent Playgrounds

Whitelist regex rules alone are not enough when autonomous agents run file edits or shell commands. AgentTrace Gateway includes a **Spatial Zone Containment Engine (`src/zones.rs`)** that enforces directory-level access control with **Most-Specific-Prefix-Wins** resolution and **lexical path-traversal collapse** (`..`).

---

## 1. Understanding the 3 Zone Tiers

| Zone Level | Read / List | Write / Edit / Delete | Shell Execution | Typical Directories |
| :--- | :---: | :---: | :---: | :--- |
| **🔒 Vault (`VaultNoAccess`)** | ❌ **DENIED** | ❌ **DENIED** | ❌ **DENIED** | `.secrets/`, `.ssh/`, `.env`, `.git/config`, `.agent_trace/node_identity.key` |
| **🛡️ Guarded (`GuardedReadOnly`)** | ✅ **ALLOWED** | ❌ **DENIED** | ⚠️ **Read-Only** | `src/`, `crates/`, `docs/`, `Cargo.toml` (when locked) |
| **🏖️ Playground (`AgentPlayground`)** | ✅ **ALLOWED** | ✅ **ALLOWED** | ✅ **ALLOWED** | `playground/`, `scratch/`, `tests/sandbox/`, `/tmp/agent_playground` |

### How Path Resolution Works
1. **Lexical Traversal Collapse**: Before checking any zone rule, paths like `./playground/../../.secrets/keys.env` or `C:\repo\playground\..\src\main.rs` are normalized lexically without touching the filesystem, neutralizing symlink/dot-dot escapes.
2. **Shell Token Extraction**: When an agent executes a shell command (e.g., `cat .secrets/master.key` or `rm -rf src/lib.rs`), the zone engine inspects path tokens inside the command line and enforces the matching zone policy.
3. **Vault Precedence & Most-Specific Prefix**: A `VaultNoAccess` rule inside a workspace always overrides broader parent rules.

---

## 2. How to Manage Zones in the Operator UI (`Z` Key)

1. Open **`http://localhost:8080/dashboard`**.
2. Click **`[🏰 Zones (Z)]`** in the top header (or press **`Z`** on your keyboard).
3. Inside the **Safe Zones vs. Danger Zone (Agent Playground)** drawer, you can:
   - **Toggle Global Playground Confinement**: Force **all** agents to write *only* inside `AgentPlayground` directories (turning the rest of your repository into `GuardedReadOnly`).
   - **Confine Specific Agents**: Add an agent name (e.g., `untrusted-scraper`, `swe-agent-trial`) to the **Confined Agents** list so only that agent is locked into the Playground while trusted lead agents retain normal write access.
   - **Add Custom Zone Rules**: Specify a path prefix (e.g., `/srv/billing_db` or `./customer_data`), choose `VaultNoAccess`, `GuardedReadOnly`, or `AgentPlayground`, and apply it to `*` (all agents) or a specific agent ID.
   - **Live Zone Evaluator**: Test any combination of `(Agent Name, Operation: Read/Write/Delete/Execute, Target Path)` right in the drawer before letting an agent run.

---

## 3. How to Configure Zones via REST API / CLI

### A. List Current Zone Policy & Confined Agents
```bash
curl -s http://localhost:8080/api/v1/zones | python -m json.tool
```

### B. Lock a Specific Agent (or All Agents) into the Playground
Lock a single untrusted agent into `AgentPlayground` directories:
```bash
curl -X POST http://localhost:8080/api/v1/zones/confine \
  -H "Content-Type: application/json" \
  -d '{
    "agent_name": "experimental-coding-bot",
    "confined": true
  }'
```

Or enable **Global Playground Confinement** for every agent on the cluster:
```bash
curl -X POST http://localhost:8080/api/v1/zones/confine \
  -H "Content-Type: application/json" \
  -d '{
    "confine_all": true,
    "confined": true
  }'
```

### C. Add a New Vault, Read-Only, or Playground Directory Rule
```bash
curl -X POST http://localhost:8080/api/v1/zones/rules \
  -H "Content-Type: application/json" \
  -d '{
    "id": "vault-prod-creds",
    "path_prefix": "config/production_keys",
    "zone": "VaultNoAccess",
    "applies_to": ["*"],
    "description": "Block all agents from reading or mutating production keys"
  }'
```

### D. Test a Path Against the Zone Engine ("What-If" Check)
```bash
curl -X POST http://localhost:8080/api/v1/zones/evaluate \
  -H "Content-Type: application/json" \
  -d '{
    "agent_name": "experimental-coding-bot",
    "operation": "write",
    "target": "src/main.rs"
  }'
```

---

## 4. How to Query Zones from Another AI Agent (MCP / Python SDK)

Agents connecting to AgentTrace can check their own permitted boundaries before attempting file operations so they don't trigger security blocks:

```bash
# View active spatial zones in the agent handoff briefing
python sdk/agent_trace_client.py --briefing
```

Or via MCP tool call (`evaluate_zone_access`):
```json
{
  "name": "evaluate_zone_access",
  "arguments": {
    "agent_name": "qwen3.5:9b",
    "operation": "write",
    "target": "./playground/prototype.rs"
  }
}
```
