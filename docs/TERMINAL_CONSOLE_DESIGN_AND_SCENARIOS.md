# Practical Agent Observability: Real-World Scenarios and Rust Architecture for AgentTrace Terminal Console

## Introduction

Autonomous AI agents (Copilot CLI, Hermes, Antigravity) introduce a new class of risk: **opacity**. When an agent writes code, it does so asynchronously. If it hallucinates, leaks secrets, or modifies critical paths, you need to know *immediately*.

**AgentTrace** is a Rust-based observability layer designed for the terminal console and web dashboard. It solves five core operator scenarios using safe, performant Rust patterns.

---

## Part 1: The Operator's Questions (Real-World Scenarios)

Developers and security operators ask specific questions when running AI agents. AgentTrace answers them directly via its UI and API.

### 1. Secret & Sensitive Path Intrusion Detection
*   **Question:** "Did the agent read or write my `.env`, `id_rsa`, or config files?"
*   **Solution:** A runtime filter checks every file access event against a denylist (e.g., `*.env`, `~/.ssh/`). If a match occurs, the agent is halted and an alert is pushed.

### 2. Unified Diff Forensics
*   **Question:** "What exactly did it change in `src/main.rs`?"
*   **Solution:** The console streams unified diffs (`diff -u`) as they happen, rather than waiting for a batch job. This allows human-in-the-loop approval before commits.

### 3. Zero-Token Routing & Model Interception Audit
*   **Question:** "Did it call the LLM or just execute local code?"
*   **Solution:** We intercept API calls to LLM providers. The UI shows a routing log: `System Prompt` vs `User Token`. If tokens are sent without approval, it flags a policy violation.

### 4. Cryptographic Ledger Integrity Proof
*   **Question:** "Is the agent's history tampered with?"
*   **Solution:** Every action is hashed (SHA-256). A `/verify` endpoint reconstructs the chain. If the hash doesn't match, the ledger is compromised.

### 5. Runtime Agent Sandboxing & Policy Control
*   **Question:** "Can I stop this agent right now?"
*   **Solution:** The UI provides a `kill` command that sends a signal to the Tokio runtime task associated with the agent process, enforcing sandbox boundaries immediately.

---

## Part 2: Rust Concepts Under the Hood

To build AgentTrace, you need specific Rust patterns. Here is how we implement them for performance and safety.

### 1. Compile-Time Asset Embedding (`include_str!`) vs Runtime I/O
**Concept:** Never read sensitive config or templates from disk at runtime if possible. Use `include_str!` to embed assets into the binary. This prevents path traversal attacks and reduces I/O latency.

```rust
// ❌ Bad: Reading template from disk (Risk of injection, slower)
let html = fs::read_to_string("templates/dashboard.html").unwrap();

// ✅ Good: Embed at compile time
const DASHBOARD_HTML: &str = include_str!("templates/dashboard.html");

// Usage in handler
pub fn dashboard_handler() -> Result<Html<&'static str>, Error> {
    Ok(Html(DASHBOARD_HTML)) // Returns static reference, zero-copy
}
```

### 2. Axum Route Handlers Returning `Html<&'static str>`
**Concept:** Axum expects response bodies to be owned or borrowed. Returning `&'static str` avoids heap allocation for the HTML shell.

```rust
use axum::{response::Html, routing::get};

// The handler returns a static reference. 
// No memory is allocated on every request.
#[utoipa::path(
    get,
    path = "/console",
    responses((status = 200, body = String))
)]
async fn console_ui() -> Html<&'static str> {
    // Content embedded via include_str! in a const
    Html(include_str!("../assets/console.html")) 
}
```

### 3. Real-Time SSE with Tokio Broadcast Channels
**Concept:** Server-Sent Events (SSE) require pushing data from the agent thread to the client stream. We use `tokio::sync::broadcast` to decouple the agent logic from the HTTP response stream.

```rust
use tokio::sync::broadcast;
use axum::{extract::ws, response::IntoResponse, Json};

// Create a channel with capacity 100
let (tx, mut rx) = broadcast::<String>::new(100);

// Agent logic pushes updates to the channel
async fn agent_worker() {
    let tx_clone = tx.clone();
    // ... agent runs ...
    if let Ok(msg) = "Agent step 1".to_string().try_into() {
        let _ = tx_clone.send(msg).await;
    }
}

// SSE Stream Adapter
pub async fn sse_stream(tx: broadcast::Sender<String>) -> impl IntoResponse {
    // Convert channel to a stream for Axum
    use tokio_stream::wrappers::BroadcastStream;
    let stream = BroadcastStream::new(tx);
    
    (
        "text/event-stream",
        axum::body::Body(
            stream.map(|msg| msg).map(axum::body::Bytes::from)
        )
    )
}
```

### 4. Clean Functions: Single Responsibility & <= 3 Arguments
**Concept:** Keep functions small. Max 3 arguments (Self, Context, Payload). This makes testing and reasoning easier.

```rust
// ❌ Violation: Too many args, mixed concerns
pub fn check_and_log(agent_id: u64, path: &str, content: &str, 
                      is_sensitive: bool, logger: &Logger) -> Result<(), Error> { ... }

// ✅ Adherence: Single Responsibility, <= 3 args
pub struct SecretFilter;

impl SecretFilter {
    // Only takes the path and content. Logging handled elsewhere.
    pub fn check_path(path: &str, content: &str) -> bool {
        let sensitive_paths = ["~/.env", "/etc/shadow"];
        sensitive_paths.iter().any(|p| path.contains(p))
    }
}

// Usage
if SecretFilter::check_path(&path, &content) {
    // Trigger alert
}
```

---

## Part 3: Architecture Implementation Details

Below is a concrete implementation of the **Secret Detection** and **SSE Stream** logic combined.

### Project Structure
```text
src/
├── main.rs
├── handlers/
│   ├── mod.rs
│   ├── secrets.rs
│   └── sse.rs
└── assets/
    └── dashboard.html (embedded)
```

### `src/handlers/secrets.rs`
**Scenario 1 & 4:** Detecting sensitive paths and verifying integrity.

```rust
use sha2::{Sha256, Digest};
use std::path::Path;

pub struct SecretPolicy {
    // Compile-time denylist embedded in binary
    pub denylist: &'static [&'static str], 
}

impl SecretPolicy {
    pub fn new() -> Self {
        Self {
            denylist: &["*.env", "id_rsa", ".git"],
        }
    }

    // Single Responsibility: Check path only.
    pub fn is_sensitive(&self, path: &Path) -> bool {
        let p_str = path.to_string_lossy();
        self.denylist.iter().any(|pattern| p_str.contains(pattern))
    }

    // Integrity Proof: Generate hash for ledger chain
    pub fn compute_hash(&self, content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }
}
```

### `src/handlers/sse.rs`
**Scenario 2:** Streaming diffs to the console.

```rust
use tokio::sync::broadcast;
use axum::{response::IntoResponse, extract::State};

pub struct AgentTraceState {
    pub tx: broadcast::Sender<String>,
}

// SSE Endpoint
pub async fn stream_agent_output(
    State(state): State<AgentTraceState>
) -> impl IntoResponse {
    let mut rx = state.tx.subscribe();
    
    // Stream loop for SSE
    while let Ok(msg) = rx.recv().await {
        // Format as SSE event
        let data = format!("data: {}\n\n", msg);
        // Yield to runtime to prevent blocking
        tokio::task::yield_now().await; 
    }
    
    axum::response::Response::builder()
        .header("Content-Type", "text/event-stream")
        .body(axum::body::Body::from(""))
}
```

### `src/main.rs`
**Scenario 5:** Routing and Policy Control.

```rust
use axum::{Router, routing::get};
use handlers::sse;

#[tokio::main]
async fn main() {
    let (tx, _rx) = broadcast::channel::<String>(100);
    
    // Mount routes
    let app = Router::new()
        .route("/console", get(console_ui))
        .route("/agent/stream", get(sse::stream_agent_output))
        .with_state(AgentTraceState { tx });

    println!("AgentTrace Console listening on 0.0.0.0:3000");
    // ... serve app ...
}
```

---

## Summary Checklist for Developers

1.  **Security First:** Use `include_str!` to embed templates and denylists. Do not read `.env` at runtime unless necessary.
2.  **Performance:** Return `Html<&'static str>` in Axum handlers to avoid heap allocations on every request.
3.  **Observability:** Use `tokio::sync::broadcast` to decouple agent logic from the HTTP response stream for real-time updates.
4.  **Maintainability:** Keep functions under 3 arguments and single responsibility (e.g., separate hashing from logging).

By following these patterns, AgentTrace provides a secure, observable foundation for running autonomous AI agents in production environments.