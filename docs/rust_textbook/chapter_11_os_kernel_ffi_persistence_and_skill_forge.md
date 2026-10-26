# Chapter 11: Practical Systems Rust — Zero-Cost OS Kernel FFI (`SetProcessMitigationPolicy`), Pluggable Persistence Traits & Sliding-Window Trajectory Mining

> **What You Will Learn in This Chapter**
> 1. **Foreign Function Interface (FFI) in Rust (`src/os_hardening.rs`)**: How to call Windows NT Kernel APIs (`SetProcessMitigationPolicy` and `GetProcessMitigationPolicy` in `kernel32.dll`) directly from Rust with **zero external crates**, using `#[link(name = "kernel32")]`, `extern "system"`, `#[repr(C)]`, and tightly scoped `unsafe` blocks.
> 2. **Conditional Compilation (`#[cfg(...)]` and `cfg!(test)`)**: How to write cross-platform systems code that compiles cleanly on Windows and Linux while keeping `cargo test` harnesses isolated.
> 3. **Trait-Based Pluggable Storage & `DeserializeOwned` (`src/persistence.rs`)**: How to design a 3-tier persistence engine where local JSON files automatically self-heal from SQLite SHA-256 digests.
> 4. **Idiomatic Slice Windows (`slice::windows`) for Event Mining (`src/skill_forge.rs`)**: How Rust's zero-allocation `windows(3)` iterator lets us detect `Fail -> Fix -> Pass` agent trajectories in microseconds.

---

## 1. Calling the OS Kernel Directly from Rust (Zero-Dependency FFI)

In Chapter 11 of the Systems Textbook, we saw *why* [`src/os_hardening.rs`](../../src/os_hardening.rs) uses OS-kernel process mitigations (`0 bytes` of RAM) instead of wrapping the gateway in a `1.5 GB` Docker/VM sandbox.

Now let's look at **how Rust talks directly to the Windows NT Kernel (`kernel32.dll`) without pulling in giant third-party bindings crates**.

### 1.1 Anatomy of `#[link(name = "kernel32")] extern "system"`

Open [`src/os_hardening.rs`](../../src/os_hardening.rs#L96-L169):

```rust
#[cfg(target_os = "windows")]
mod win32_kernel {
    use std::ffi::c_void;

    pub const PROCESS_DYNAMIC_CODE_POLICY: i32 = 2;
    pub const PROCESS_EXTENSION_POINT_DISABLE_POLICY: i32 = 6;
    pub const PROCESS_IMAGE_LOAD_POLICY: i32 = 10;
    pub const PROCESS_CHILD_PROCESS_POLICY: i32 = 13;

    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct PolicyFlags32 {
        pub flags: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub fn SetProcessMitigationPolicy(
            mitigation_policy: i32,
            lp_buffer: *const c_void,
            dw_length: usize,
        ) -> i32;

        pub fn GetProcessMitigationPolicy(
            h_process: *mut c_void,
            mitigation_policy: i32,
            lp_buffer: *mut c_void,
            dw_length: usize,
        ) -> i32;

        pub fn GetCurrentProcess() -> *mut c_void;
        pub fn GetLastError() -> u32;
    }
}
```

Let's dissect every Rust systems keyword used here:

1. **`#[cfg(target_os = "windows")]`**:
   - Tells `rustc` to compile this module *only* when targeting Windows. On Linux or macOS, this entire module is stripped at compile time with zero overhead.
2. **`#[repr(C)]` on `struct PolicyFlags32`**:
   - By default, Rust's compiler is free to reorder struct fields or add padding to optimize memory alignment (`repr(Rust)`).
   - When passing a struct pointer across an FFI boundary to the Windows C/C++ ABI, we **must** annotate it with `#[repr(C)]` so its memory layout matches the exact 4-byte `DWORD` bitfield expected by `ntoskrnl.exe`.
3. **`#[link(name = "kernel32")]` & `extern "system"`**:
   - `kernel32.dll` is the core Win32 subsystem DLL mapped into every Windows process.
   - Why `extern "system"` instead of `extern "C"`? On 64-bit Windows (`x86_64-pc-windows-msvc`), both use the Microsoft x64 calling convention, but on 32-bit x86 Windows, Win32 APIs use `stdcall` while C uses `cdecl`. Writing `extern "system"` tells Rust to automatically pick the OS-native syscall ABI on every architecture!
4. **Encapsulating `unsafe` Inside a Safe Rust Function**:
   - Calling an `extern` C/OS function is always `unsafe` in Rust because the compiler cannot verify what the OS kernel does with raw pointers (`*const c_void`).
   - Best practice in Rust is to **never leak `unsafe` to callers**. Instead, we wrap the raw syscall inside a tiny, safe function (`set_policy_u32`) where we prove the pointer and length invariants ourselves:

```rust
pub fn set_policy_u32(policy_id: i32, flags: u32) -> Result<(), u32> {
    let payload = PolicyFlags32 { flags };
    // SAFETY: `payload` is a valid repr(C) 4-byte struct on the stack matching the
    // Windows SDK PROCESS_MITIGATION_*_POLICY layout, and `size_of::<PolicyFlags32>()`
    // is guaranteed to be 4 bytes.
    let ok = unsafe {
        SetProcessMitigationPolicy(
            policy_id,
            (&payload as *const PolicyFlags32).cast::<c_void>(),
            std::mem::size_of::<PolicyFlags32>(),
        )
    };
    if ok != 0 {
        Ok(())
    } else {
        Err(unsafe { GetLastError() })
    }
}
```

Notice how callers of `set_policy_u32(PROCESS_IMAGE_LOAD_POLICY, img_flags)` get a clean, idiomatic `Result<(), u32>` without writing a single line of `unsafe` code!

---

## 2. Compile-Time vs. Runtime Test Awareness (`#[cfg(test)]` vs. `cfg!(test)`)

In [`src/os_hardening.rs`](../../src/os_hardening.rs) and [`src/persistence.rs`](../../src/persistence.rs), we use `cfg!(test)` (the macro that evaluates to a `bool` at compile time):

```rust
if !cfg!(test) {
    // Only lock irreversible kernel flags (like NoChildProcessCreation)
    // when running the production binary, not inside the `cargo test` runner!
    let _ = set_policy_u32(PROCESS_CHILD_PROCESS_POLICY, NO_CHILD_PROCESS_CREATION);
}
```

Why is this distinction so important?
- When you run `cargo test --lib`, Cargo compiles all your unit tests into a single test-runner executable that may spawn helper threads or subprocesses.
- If a unit test permanently flipped the kernel's one-way `NoChildProcessCreation` bit on the `cargo test` process itself, subsequent tests in the same binary wouldn't be able to spawn test subprocesses!
- Using `if !cfg!(test)` lets us test our `WardenSelfGuard` state machine in unit tests while applying the irreversible kernel lock in the real `agent_trace_gw.exe` binary.

---

## 3. Designing Pluggable Persistence with Traits & `DeserializeOwned` (`src/persistence.rs`)

In [`src/persistence.rs`](../../src/persistence.rs), we needed a unified interface that can store state in:
1. Local JSON files (`LocalFileBackend`),
2. Embedded SQLite WAL with SHA-256 verification (`SqliteKvBackend`), and
3. An optional external database (`ExternalDbBackend`).

### 3.1 Defining the Thread-Safe Trait
```rust
pub trait PersistenceBackend: Send + Sync {
    fn backend_type(&self) -> &'static str;
    fn is_connected(&self) -> bool;
    fn save_json(&self, namespace: &str, key: &str, payload_json: &str) -> Result<(), String>;
    fn load_json(&self, namespace: &str, key: &str) -> Result<Option<String>, String>;
    fn list_namespace(&self, namespace: &str) -> Result<Vec<(String, String)>, String>;
    fn delete_key(&self, namespace: &str, key: &str) -> Result<bool, String>;
}
```
Notice the supertrait bound `: Send + Sync`. In an async Tokio/Axum server (`#[tokio::main]`), requests may move across worker threads at any `.await` point. Requiring `Send + Sync` guarantees at compile time that every persistence backend can be safely shared inside an `Arc<UnifiedPersistenceManager>`.

### 3.2 Generic Self-Healing with `serde::de::DeserializeOwned`
Why does `load_struct<T: DeserializeOwned>` use `DeserializeOwned` instead of `Deserialize<'de>`?
- `Deserialize<'de>` allows zero-copy borrowing from the input string buffer (e.g., `&'de str`).
- However, inside `load_struct`, `json_str` is a local `String` loaded from disk or SQLite that is dropped before `load_struct` returns!
- Therefore, the returned struct `T` must **own** all of its data (`DeserializeOwned`), allowing us to deserialize from a temporary `json_str` and even **self-heal a corrupted local file from SQLite**:

```rust
pub fn load_struct<T: DeserializeOwned>(&self, namespace: &str, key: &str) -> Option<T> {
    // 1. Fast path: try Tier 1 Local JSON file
    if let Ok(Some(json_str)) = self.local_file.load_json(namespace, key) {
        if let Ok(val) = serde_json::from_str::<T>(&json_str) {
            return Some(val);
        }
    }

    // 2. Self-Healing path: if the local file was deleted or corrupted on disk,
    // recover the SHA-256 verified payload from Tier 2 SQLite and rewrite Tier 1!
    if let Ok(Some(sqlite_json)) = self.sqlite_kv.load_json(namespace, key) {
        if let Ok(val) = serde_json::from_str::<T>(&sqlite_json) {
            let _ = self.local_file.save_json(namespace, key, &sqlite_json);
            return Some(val);
        }
    }
    None
}
```

---

## 4. Zero-Allocation Trajectory Mining with `slice::windows` (`src/skill_forge.rs`)

How does [`src/skill_forge.rs`](../../src/skill_forge.rs) scan a chronological stream of telemetry events to spot a 3-step `Fail -> FileWrite -> Pass` sequence without complex index math or out-of-bounds panics?

Rust slices (`&[T]`) provide a built-in iterator method `.windows(n)`:
```rust
// Pattern 1: ErrorRecoveryLoop (Fail Command -> FileWrite with Diff -> Pass Command)
for window in chron.windows(3) {
    let ev1 = window[0];
    let ev2 = window[1];
    let ev3 = window[2];
    // ... inspect the 3 consecutive events safely with zero allocations!
}
```
- If `chron` has fewer than 3 elements, `chron.windows(3)` simply yields 0 iterations (never panics!).
- Each `window` is a borrowed slice `&[&Value]` of length 3 sliding across the vector one step at a time (`[0,1,2]`, `[1,2,3]`, `[2,3,4]`, ...), giving us crystal-clear pattern matching with zero heap allocations.

---

## 5. Summary & Exercises

1. **Run the OS-Kernel Hardening Unit Tests**:
   ```bash
   cargo test --lib os_hardening
   ```
2. **Inspect the Benchmark Suite (`v1.1.0`)**:
   ```bash
   cargo test --lib test_and_export_empirical_sub_5ms_benchmarks -- --nocapture
   ```
   Observe how `OS-Kernel Warden Self-Guard & Write-Boundary Check (0 KB RAM)` executes in **`~0.001 ms` (`1 µs`)** while adding **`0 bytes`** to the gateway's memory footprint.
