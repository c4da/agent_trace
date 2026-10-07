import urllib.request
import json
import sys
import os
import time

# Ensure UTF-8 output
sys.stdout.reconfigure(encoding='utf-8')

OUTPUT_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "docs", "rust_textbook")
os.makedirs(OUTPUT_DIR, exist_ok=True)

GATEWAY_URL = "http://localhost:8080/v1/chat/completions"
MODEL = "gemma4:latest"

CHAPTERS = [
    {
        "filename": "chapter_01_getting_started.md",
        "title": "Chapter 1: The Rust Philosophy & First Steps",
        "prompt": """You are an exceptional, friendly, and world-class Rust programming educator writing for a beginner who is learning Rust for the first time.
Write a comprehensive, engaging, and beginner-friendly textbook chapter titled:
'Chapter 1: The Rust Philosophy & First Steps'.

Cover in detail:
1. Welcome to Rust:
   - Why Rust was created: solving memory safety bugs (null pointer dereferences, buffer overflows, use-after-free) without sacrificing speed or using a garbage collector.
   - The Rust toolchain: `rustup` (toolchain installer), `rustc` (compiler), and `cargo` (package manager & build system).
   - What `Cargo.toml` is and how it manages dependencies.
2. First Steps & Variables:
   - `let` vs `let mut`: Immutability by default and why it prevents bugs.
   - Variable shadowing: How `let x = 5; let x = x + 1;` works.
   - Constants (`const`) vs immutable variables.
3. Primitive Types:
   - Integers: Signed (`i8`, `i16`, `i32`, `i64`) vs Unsigned (`u8`, `u16`, `u32`, `u64`). Explain when to use `u64` (counts, byte lengths) vs `u16` (network ports).
   - Floats: `f32`, `f64` (used in latency and threat scores).
   - Booleans: `bool`.
   - Characters & Strings: `char`, string slice (`&str`) vs owned heap string (`String`).
4. Functions & Control Flow:
   - Function declaration: `fn name(param: Type) -> ReturnType`.
   - Expressions vs Statements: The magic of returning without the `return` keyword (omitting the semicolon).
   - Control flow: `if`, `else if`, `else`, `loop`, `while`, and `for item in collection`.
5. Code walkthrough from `agent_trace_gw`:
   - Show how `let port: u16 = 8080;` and simple helper functions are written.
6. Practice Exercises for the beginner.

Tone: Warm, encouraging, crystal-clear, with syntax breakdowns and comments on every line of code.
Format: Clean GitHub Markdown.
"""
    },
    {
        "filename": "chapter_02_ownership_borrowing.md",
        "title": "Chapter 2: The Heart of Rust: Ownership & Borrowing",
        "prompt": """You are an exceptional, friendly, and world-class Rust programming educator writing for a beginner who is learning Rust for the first time.
Write a comprehensive, engaging, and beginner-friendly textbook chapter titled:
'Chapter 2: The Heart of Rust: Ownership & Borrowing'.

Cover in detail:
1. The Core Problem: Why Memory Management is Hard:
   - Manual memory management (C/C++: `malloc`/`free`, segfaults, double free, memory leaks).
   - Garbage collection (Python/Java/Go: pause times, runtime overhead).
   - Rust's solution: Ownership at compile time with zero runtime cost!
2. The Stack vs. The Heap:
   - Clear ASCII diagram of the Stack (fast, fixed-size, LIFO) and the Heap (dynamic, pointer-accessed).
3. The Three Golden Rules of Ownership:
   - Rule 1: Each value in Rust has an owner.
   - Rule 2: There can only be one owner at a time.
   - Rule 3: When the owner goes out of scope, the value will be dropped.
4. Move Semantics vs. Copy vs. Clone:
   - What happens when you assign `let s2 = s1;` with a `String` (Move) vs an integer `let x2 = x1;` (`Copy`).
   - Explicit duplication with `.clone()`: When to use it and what it costs.
5. Borrowing & References:
   - Why we don't always want to give away ownership.
   - Shared references (`&T`): Read-only, multiple readers allowed.
   - Mutable references (`&mut T`): Read-write, only ONE active writer at a time!
   - The Cardinal Rule: You can have either any number of immutable references OR exactly one mutable reference, never both simultaneously.
6. Real AgentTrace Examples:
   - Passing `&TraceEvent` to compute a hash vs passing `&mut TraceEvent` to set `event.prev_hash`.
7. Friendly compiler error walkthrough: What a borrow-checker error looks like and how to fix it with confidence.
8. Practice Exercises.

Format: Clean GitHub Markdown with ASCII diagrams and clear annotations.
"""
    },
    {
        "filename": "chapter_03_structs_enums_matching.md",
        "title": "Chapter 3: Structs, Enums & Pattern Matching",
        "prompt": """You are an exceptional, friendly, and world-class Rust programming educator writing for a beginner who is learning Rust for the first time.
Write a comprehensive, engaging, and beginner-friendly textbook chapter titled:
'Chapter 3: Structs, Enums & Pattern Matching'.

Cover in detail:
1. Custom Data Types: Structs:
   - Classic Structs: Grouping related fields together.
   - Building structs: `struct TraceEvent { event_id: String, target: String, duration_ms: f64 }`.
   - Field init shorthand and struct update syntax (`..other`).
   - Adding methods with `impl`: The difference between `&self`, `&mut self`, and associated functions (like `Self::new()`).
2. Enums: More Powerful Than in Any Other Language:
   - Basic enums vs Enums holding rich data!
   - Real AgentTrace example:
     ```rust
     enum EventType { FileRead, FileWrite, CommandRun, LlmCompletion }
     enum SecurityVerdict { Allowed, Flagged, Blocked }
     ```
3. Eliminating Null: The `Option<T>` Enum:
   - Tony Hoare's "Billion-Dollar Mistake" (null pointers).
   - Rust's `Option<T>`: `Some(T)` or `None`.
   - Why this forces you to handle missing data at compile time.
4. The Power of Pattern Matching:
   - The `match` expression: Exhaustive checking (compiler ensures you never miss a case!).
   - Matching on enums with data extraction.
   - The concise `if let Some(val) = option` syntax.
5. Practice Exercises modeling an agent telemetry event from scratch.

Format: Clean GitHub Markdown with annotated code examples.
"""
    },
    {
        "filename": "chapter_04_traits_and_generics.md",
        "title": "Chapter 4: Traits, Generics & Serde",
        "prompt": """You are an exceptional, friendly, and world-class Rust programming educator writing for a beginner who is learning Rust for the first time.
Write a comprehensive, engaging, and beginner-friendly textbook chapter titled:
'Chapter 4: Traits, Generics & Serde'.

Cover in detail:
1. Generics: Code That Works for Any Type:
   - Generic functions: `fn print_item<T>(item: T)`.
   - Zero-cost abstractions: Monomorphization (how Rust generates specialized machine code for each type without runtime slowdown).
2. Traits: Shared Behavior (Rust's Supercharged Interfaces):
   - Defining a trait: `trait Summary { fn summarize(&self) -> String; }`.
   - Implementing a trait for a struct.
   - Common standard library traits: `Display`, `Debug`, `Clone`, `Default`.
3. Deriving Traits with `#[derive(...)]`:
   - What happens when you put `#[derive(Clone, Debug, PartialEq)]` above a struct.
4. Serde: The Gold Standard for Serialization in Rust:
   - Why Serde is used in every modern Rust project.
   - `Serialize` and `Deserialize` traits.
   - Converting Rust structs to JSON strings with `serde_json::to_string(&event)`.
   - Parsing JSON into Rust structs with `serde_json::from_str::<TraceEvent>(&json)`.
   - Real examples from `agent_trace_gw` payload exchange.
5. Practice Exercises.

Format: Clean GitHub Markdown with step-by-step walkthroughs.
"""
    },
    {
        "filename": "chapter_05_error_handling_idioms.md",
        "title": "Chapter 5: Error Handling: Never Crashing in Production",
        "prompt": """You are an exceptional, friendly, and world-class Rust programming educator writing for a beginner who is learning Rust for the first time.
Write a comprehensive, engaging, and beginner-friendly textbook chapter titled:
'Chapter 5: Error Handling: Never Crashing in Production'.

Cover in detail:
1. Philosophy of Rust Error Handling:
   - Why Rust has no exceptions (`try/catch`).
   - Unrecoverable errors (`panic!`): When something is irreparably broken (out of bounds, broken invariant).
   - Recoverable errors (`Result<T, E>`): Expected failure modes (file not found, network timeout, invalid JSON).
2. The `Result<T, E>` Enum:
   - `Ok(value)` vs `Err(error)`.
   - Handling results with `match`, `unwrap()`, and `expect("message")`—and why you should avoid `unwrap()` in production!
3. The Question Mark Operator (`?`):
   - The greatest quality-of-life feature in Rust!
   - How `let file = File::open("path")?;` automatically unwraps or returns early with `Err`.
   - Type conversions with the `From` trait under the hood.
4. Working with Errors in Real Applications:
   - Using `Box<dyn std::error::Error>` in `main()` for quick scripts.
   - Mapping errors with `.map_err(...)`.
   - How `agent_trace_gw` handles SQLite and HTTP errors safely without crashing.
5. Practice Exercises.

Format: Clean GitHub Markdown with side-by-side good vs bad examples.
"""
    },
    {
        "filename": "chapter_06_async_tokio_concurrency.md",
        "title": "Chapter 6: Asynchronous Programming with Tokio",
        "prompt": """You are an exceptional, friendly, and world-class Rust programming educator writing for a beginner who is learning Rust for the first time.
Write a comprehensive, engaging, and beginner-friendly textbook chapter titled:
'Chapter 6: Asynchronous Programming with Tokio'.

Cover in detail:
1. Concurrency vs Parallelism:
   - OS Threads: Heavyweight (each thread takes ~1-2 MB stack), context switching overhead.
   - Async Tasks (Green Threads): Lightweight (thousands of tasks on a small thread pool).
   - The Reactor/Executor model.
2. The Tokio Runtime:
   - What `#[tokio::main]` actually does (starting the multi-threaded work-stealing executor).
   - `async fn` and `.await`: What a `Future` is (poll-based, lazy evaluation in Rust).
   - `tokio::spawn`: Launching background concurrent tasks.
3. Concurrency Superpowers: Thread-Safe Shared State:
   - Why standard pointers fail across threads: `Send` and `Sync` traits.
   - `Arc<T>` (Atomic Reference Counting): Sharing ownership across async tasks.
   - `Mutex<T>` vs `RwLock<T>`: Interior mutability and locking strategies.
   - The famous combination: `Arc<RwLock<ClusterModelRegistry>>` explained simply!
4. Channels and Message Passing:
   - `tokio::sync::broadcast`: Sending telemetry events from the ingestion handler to SSE dashboard subscribers.
5. Real code walkthrough of `AppState` in `src/handlers.rs`.
6. Practice Exercises.

Format: Clean GitHub Markdown with concurrency diagrams and code explanations.
"""
    },
    {
        "filename": "chapter_07_building_web_apis_axum.md",
        "title": "Chapter 7: Building High-Performance Web Services with Axum",
        "prompt": """You are an exceptional, friendly, and world-class Rust programming educator writing for a beginner who is learning Rust for the first time.
Write a comprehensive, engaging, and beginner-friendly textbook chapter titled:
'Chapter 7: Building High-Performance Web Services with Axum'.

Cover in detail:
1. Introducing Axum:
   - Why Axum is the modern standard for Rust web development (built on Tokio, Hyper, and Tower).
   - Type-safe routing without reflection or macros magic.
2. Anatomy of an Axum App:
   - The `Router`: `Router::new().route("/path", get(handler).post(handler))`.
   - Binding and serving: `tokio::net::TcpListener::bind(addr)` and `axum::serve`.
3. Axum Extractors: How Handlers Receive Data:
   - `State(state)`: Accessing global database and configuration.
   - `Json(payload)`: Automatically parsing incoming JSON into strong Rust structs.
   - `Path(id)`: Extracting URL parameters like `/api/v1/policy/:agent_id`.
   - `Query(q)`: Extracting query strings like `?limit=50`.
   - `HeaderMap`: Inspecting headers (e.g. `x-agenttrace-token`).
4. Returning Responses:
   - `Json(data)`: Serializing structs to JSON with `200 OK`.
   - `StatusCode`: Returning HTTP codes (`StatusCode::CREATED`, `StatusCode::FORBIDDEN`).
   - Server-Sent Events (SSE) streaming with `Sse` and `KeepAlive`.
5. Real walkthrough of `src/handlers.rs`: `ingest_event` and `create_router`.
6. Practice Exercises building a mini REST API.

Format: Clean GitHub Markdown with request/response flowcharts.
"""
    },
    {
        "filename": "chapter_08_systems_crypto_sqlite.md",
        "title": "Chapter 8: Systems Programming, Cryptography & SQLite",
        "prompt": """You are an exceptional, friendly, and world-class Rust programming educator writing for a beginner who is learning Rust for the first time.
Write a comprehensive, engaging, and beginner-friendly textbook chapter titled:
'Chapter 8: Systems Programming, Cryptography & SQLite'.

Cover in detail:
1. Low-Level File and System Operations:
   - Standard library `std::path::Path` and `PathBuf`.
   - Cross-platform path handling (Windows backslashes vs Unix slashes).
   - Environment variables with `std::env::var`.
2. Applied Cryptography with `ring`:
   - What is Ed25519? High-speed asymmetric signatures.
   - Generating a keypair, saving as PKCS#8 bytes, and loading on boot.
   - Signing a message digest: `identity.sign_event(digest)`.
   - Verifying signatures: `NodeIdentity::verify_signature(...)`.
3. Length-Prefixed Canonical Hashing:
   - Why hashing is the backbone of block chains.
   - SHA-256 with `ring::digest::Context`.
   - Encoding bytes into hex strings with the `hex` crate.
4. SQLite Databases in Rust with `rusqlite`:
   - Embedding SQLite directly into your binary (zero external database server needed!).
   - Creating tables, migrations, and SQLite WAL (Write-Ahead Logging) mode.
   - Prepared statements and type-safe query mapping.
   - Wrapping `Connection` in a `Mutex` for safe access across threads.
5. Real code breakdown from `src/ledger.rs` and `src/db.rs`.
6. Practice Exercises.

Format: Clean GitHub Markdown with step-by-step crypto verification diagrams.
"""
    },
    {
        "filename": "chapter_09_testing_and_architecture.md",
        "title": "Chapter 9: Testing, Idiomatic Modularity & Building Your First App",
        "prompt": """You are an exceptional, friendly, and world-class Rust programming educator writing for a beginner who is learning Rust for the first time.
Write a comprehensive, engaging, and beginner-friendly textbook chapter titled:
'Chapter 9: Testing, Idiomatic Modularity & Building Your First App'.

Cover in detail:
1. Structuring a Real-World Rust Project:
   - Single binary vs Library + Binary (`src/lib.rs` and `src/main.rs`).
   - Module hierarchy: `mod sub_module;`, `pub use`, and folder layout.
   - The lesson from AgentTrace: Why keeping `main.rs` as a thin orchestrator is the gold standard.
2. Rust Testing Culture:
   - Unit tests: Using `#[cfg(test)] mod tests` in the same file.
   - The `#[test]` attribute and assertions (`assert!`, `assert_eq!`, `assert_ne!`).
   - Asynchronous tests with `#[tokio::test]`.
   - Isolated integration tests with `tempfile`: testing real SQLite databases without leaving junk files.
3. Step-by-Step Hands-On Project:
   - "Build your own Mini-Telemetry Logger in 50 lines of Rust":
     - Define a struct `LogEntry`.
     - Compute its SHA-256 hash.
     - Save to a temporary SQLite database.
     - Verify its hash chain integrity.
4. Where to Go From Here:
   - Becoming a confident Rustacean: Clippy (`cargo clippy`), Rust Analyzer, and open-source contributions.
5. Final Words of Encouragement.

Format: Clean GitHub Markdown with complete runnable code project.
"""
    }
]

def generate_chapter(ch_idx, ch):
    target_path = os.path.join(OUTPUT_DIR, ch["filename"])
    print(f"\n=======================================================")
    print(f"[*] [{ch_idx + 1}/{len(CHAPTERS)}] Generating: {ch['title']}")
    print(f"[*] Target File: {target_path}")
    print(f"=======================================================")

    payload = {
        "model": MODEL,
        "messages": [
            {"role": "system", "content": "You are a distinguished computer science educator and author of the world's most accessible, thorough, and beloved programming textbooks."},
            {"role": "user", "content": ch["prompt"]}
        ],
        "max_tokens": 4000,
        "temperature": 0.3
    }

    req = urllib.request.Request(
        GATEWAY_URL,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"}
    )

    t0 = time.time()
    try:
        with urllib.request.urlopen(req, timeout=300) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            content = data["choices"][0]["message"].get("content") or data["choices"][0]["message"].get("reasoning")
            if not content:
                print("[-] Empty response received!")
                return False

            with open(target_path, "w", encoding="utf-8") as f:
                f.write(content)

            elapsed = time.time() - t0
            print(f"[+] Successfully wrote {len(content)} bytes in {elapsed:.2f}s to {ch['filename']}")
            return True
    except Exception as e:
        print(f"[-] Error generating chapter {ch['filename']}: {e}")
        return False

def main():
    print("=================================================================")
    print("  AGENTTRACE BEGINNER RUST TEXTBOOK WRITER (ZERO TOKEN ECONOMY)")
    print(f"  Target Model:   {MODEL}")
    print(f"  Gateway URL:    {GATEWAY_URL}")
    print(f"  Total Chapters: {len(CHAPTERS)}")
    print("=================================================================")

    success_count = 0
    for idx, ch in enumerate(CHAPTERS):
        ok = generate_chapter(idx, ch)
        if ok:
            success_count += 1
        time.sleep(1)

    print("\n=================================================================")
    print(f"[✓] RUST TEXTBOOK DRAFTING COMPLETE: {success_count}/{len(CHAPTERS)} chapters written.")
    print(f"    Location: {OUTPUT_DIR}")
    print("=================================================================")

if __name__ == "__main__":
    main()
