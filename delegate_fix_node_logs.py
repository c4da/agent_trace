import urllib.request
import json
import sys
import os
import time

sys.stdout.reconfigure(encoding='utf-8')

prompt = """You are Qwen 3.8 (27B), Lead Systems Architect and Full-Stack Rust/Web Engineer for AgentTrace Gateway.

### Bug Report:
"Local nodes shows no logs in the UI"

### Root Cause Analysis:
1. In `cluster_telemetry.db`, we have:
   - `PC-1-Antigravity`: 7,744 events (high frequency Antigravity IDE watcher)
   - `PC-1` (Local Ollama / CLI): 70 events
   - `PC-2` (Worker node CUDA / Hermes): 27 events
2. In `src/handlers.rs`:
   ```rust
   pub async fn get_events(
       State(state): State<AppState>,
       Query(q): Query<LimitQuery>,
   ) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
       let limit = q.limit.unwrap_or(50).clamp(1, 500);
       let events = state.db.get_recent_events(limit)...;
   ```
   The backend `/api/v1/events` only supports `limit`. It lacks `node_id` or `agent_id` or `event_type` filtering query parameters!
3. In `src/dashboard.html`:
   ```javascript
   async function fetchEvents() {
       const res = await fetch('/api/v1/events?limit=100');
       allEvents = await res.json();
       ...
   }
   function renderFilteredEvents() {
       let filtered = allEvents.filter(ev => {
           if (currentFilter !== 'ALL') {
               if (currentFilter === 'PC-1') {
                   if (ev.node_id !== 'PC-1') return false;
               } else {
                   if (!ev.node_id.includes(currentFilter)) return false;
               }
           }
       ...
   ```
   Because `PC-1-Antigravity` generates dozens of events per minute, the latest 100 events returned by `/api/v1/events?limit=100` are 100% `PC-1-Antigravity` events.
   When the user clicks the filter pill "PC-1 (Local)" or "PC-2 (Worker)", `renderFilteredEvents()` finds ZERO events in `allEvents`, showing a completely empty table!

### Your Tasks:
1. Provide the complete Rust backend implementation:
   - Update `LimitQuery` in `src/handlers.rs` to support `node_id: Option<String>`, `event_type: Option<String>`, and `limit: Option<usize>`.
   - Update `src/db.rs` to add `get_recent_events_filtered(&self, limit: usize, node_id: Option<&str>, event_type: Option<&str>) -> Result<Vec<serde_json::Value>, ...>` using parameterized SQL.
2. Provide the complete JavaScript frontend implementation in `src/dashboard.html`:
   - Update `fetchEvents(filter)` and `setFilter(node)` so when a node filter is selected, it fetches with `?node_id=...` (or when ALL, fetches recent mix).
   - Ensure `PC-1` filter matches both `node_id = 'PC-1'` and any local node actions without being drowned out by `PC-1-Antigravity`.
   - Ensure `PC-2` filter fetches `?node_id=PC-2`.
   - Ensure the table dynamically updates and shows active events for each node with accurate counts.
3. Write clean, exact Rust code and JavaScript snippets ready for immediate integration.
"""

payload = {
    "model": "qwen3.8:27b",
    "messages": [
        {"role": "system", "content": "You are Qwen 3.8 (27B), Lead Systems Architect. Provide the complete, exact code solution to fix the node filtering and logs display bug."},
        {"role": "user", "content": prompt}
    ],
    "stream": True,
    "options": {
        "temperature": 0.2,
        "num_predict": 2500
    }
}

print("[*] Delegating Node Logs UI Bug to Qwen 3.8 (27B) on PC-1 (zero-token)...")
t0 = time.time()

req = urllib.request.Request(
    "http://127.0.0.1:11434/api/chat",
    data=json.dumps(payload).encode("utf-8"),
    headers={"Content-Type": "application/json"}
)

report_path = "docs/QWEN_NODE_LOGS_FIX_SPEC.md"
os.makedirs("docs", exist_ok=True)

try:
    with urllib.request.urlopen(req, timeout=600) as resp:
        with open(report_path, "w", encoding="utf-8") as out_f:
            total_chars = 0
            for line in resp:
                if not line:
                    continue
                try:
                    chunk = json.loads(line.decode("utf-8"))
                    text = chunk.get("message", {}).get("content", "")
                    if text:
                        out_f.write(text)
                        out_f.flush()
                        total_chars += len(text)
                    if chunk.get("done", False):
                        break
                except Exception:
                    continue

    elapsed = time.time() - t0
    print(f"[+] Qwen 3.8 fix spec completed in {elapsed:.2f}s! Total characters written: {total_chars}")
    print(f"[+] Saved to: {report_path}")

except Exception as e:
    print(f"[-] Error delegating to Qwen 3.8: {e}")
    sys.exit(1)
