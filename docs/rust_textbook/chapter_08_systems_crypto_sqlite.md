# 📚 Chapter 8: Systems Programming, Cryptography & SQLite

***

**(A Note from Your Educator):**

Welcome back! If the previous chapters taught you how to write elegant, safe, and concurrent programs in Rust, this chapter is where we take those skills and apply them to the *real world*.

We are moving beyond simple console applications. We are building systems. We are dealing with files, network-level security, and persistent data storage. This is the domain of "Systems Programming," and it's where Rust truly shines.

Don't let the buzzwords—*cryptography*, *SQLite*, *Ed25519*—intimidate you. We are going to break them down piece by piece. By the end of this chapter, you won't just *use* these tools; you'll understand *why* they are the bedrock of modern, secure applications, from cryptocurrencies to secure identity systems.

Let's dive in!

***

## 📂 Section 8.1: Low-Level File and System Operations

Before we can store data securely or manage complex systems, we need to talk to the operating system. We need to know where files are, and we need to read configuration settings.

### The Path to the Data: `std::path::Path` and `PathBuf`

In Rust, when you are dealing with file paths, you should never just use a string (`&str`). Paths are special because they need to handle the rules of the operating system they are running on.

*   **`&Path` (The View):** This is an immutable reference to a path. Think of it as a *view* of a path string. It's lightweight and used when you just need to read or check a path.
*   **`PathBuf` (The Container):** This is an owned, mutable buffer for a path. You use this when you need to build a path, modify it, or pass it around and own it.

**Why is this important?** Because a path like `my_data/user_files/config.txt` works perfectly on Linux/macOS (using forward slashes `/`), but on Windows, the native separator is the backslash (`\`). Rust handles this abstraction for you!

```rust
use std::path::{Path, PathBuf};

fn main() {
    // 1. Creating a PathBuf (we own this path)
    let mut config_path = PathBuf::new();
    config_path.push("config");
    config_path.push("settings.toml");

    println!("The full path is: {:?}", config_path);

    // 2. Using &Path (we are just viewing it)
    // We can check if the path exists or if it's a directory.
    if config_path.is_dir() {
        println!("This path points to a directory!");
    } else {
        println!("This path points to a file or doesn't exist.");
    }
}
```

### Talking to the Environment: `std::env::var`

Many applications need configuration that changes based on where they are run (e.g., a database connection string, or an API key). We store these in **environment variables**.

The `std::env::var` function lets your Rust program read these variables.

```rust
use std::env;

fn main() {
    // Try to read a variable named 'API_KEY'
    match env::var("API_KEY") {
        Ok(key) => {
            println!("✅ Successfully loaded API Key! (Length: {})", key.len());
            // In a real app, you would use this key for authentication.
        }
        Err(e) => {
            // This handles the case where the variable isn't set.
            eprintln!("❌ Error reading environment variable: {}", e);
            eprintln!("Please set the API_KEY environment variable to run this program.");
        }
    }
}
```

***

## 🔒 Section 8.2: Applied Cryptography with `ring`

This is where things get exciting. Cryptography is the science of secure communication. It allows us to prove *who* sent a message and that the message hasn't been tampered with.

### What is Ed25519?

We are using **Ed25519**, a modern, high-speed, and extremely secure asymmetric signature algorithm.

*   **Asymmetric Cryptography:** This means you have two keys: a **Private Key** (a secret, like a password) and a **Public Key** (like your username).
    *   The Private Key is used to **sign** a message.
    *   The Public Key is used by anyone to **verify** the signature.
*   **The Goal:** If someone has the Public Key, they can verify the signature, but they cannot generate a valid signature without the Private Key.

### Key Generation and Handling

We use the `ring` crate, a robust, audited cryptographic library.

1.  **Generate:** Create a pair of keys.
2.  **Save:** The private key must be saved securely (often encrypted). We save it as raw bytes (PKCS#8 format).
3.  **Load:** On startup, we load the private key back into memory.

*(Note: For brevity, we will focus on the signing/verification flow, assuming the keys are loaded.)*

### Signing and Verification: The Digital Signature Process

Imagine you want to prove you approved a transaction (the message digest).

**The Process:**

1.  **Digest:** The message (e.g., "Transfer 10 BTC") is run through a hashing algorithm (like SHA-256) to create a small, fixed-size fingerprint, called a **digest**.
2.  **Sign:** You use your **Private Key** to mathematically sign this digest. This signature is unique to your private key and the specific digest.
3.  **Verify:** Anyone can take the original message, generate its digest, and then use your **Public Key** to check if the provided signature matches that digest.

#### 🔬 Crypto Verification Diagram (Conceptual Flow)

| Step | Actor | Action | Input | Output |
| :--- | :--- | :--- | :--- | :--- |
| **1. Hashing** | Sender | Calculate Digest | Message | `Digest (D)` |
| **2. Signing** | Sender (Private Key) | Sign Digest | `D` + Private Key | `Signature (S)` |
| **3. Verification** | Receiver (Public Key) | Verify Signature | `D` + `S` + Public Key | **✅ Valid** or **❌ Invalid** |

### 💻 Code Example: Signing a Message

```rust
use ring::{signature, digest};
use std::io::{Cursor, Write};

// --- Simplified setup for demonstration ---

fn generate_and_sign(message: &[u8]) -> (Vec<u8>, Vec<u8>) {
    // 1. Generate Keypair
    let rng = rand::SystemRandom::new();
    let keypair = signature::KeyPair::generate(&rng).unwrap();
    let public_key = keypair.public_key();
    let private_key = keypair.private_key();

    // 2. Calculate the Message Digest (SHA-256)
    let mut context = digest::Context::new(&digest::SHA256);
    context.update(message);
    let digest_bytes = context.finish();

    // 3. Sign the Digest using the Private Key
    let signature_bytes = private_key.sign(&mut rng, &digest_bytes).unwrap();

    // In a real system, you would save the public key bytes and the signature bytes.
    (public_key.as_ref().to_vec(), signature_bytes.to_vec())
}

fn verify_signature(public_key_bytes: &[u8], message: &[u8], signature_bytes: &[u8]) -> bool {
    // 1. Load the Public Key
    let public_key = signature::UnparsedPublicKey::new(
        signature::ED25519,
        public_key_bytes,
    ).expect("Failed to load public key");

    // 2. Calculate the Message Digest (must match the original signing process)
    let mut context = digest::Context::new(&digest::SHA256);
    context.update(message);
    let digest_bytes = context.finish();

    // 3. Verify the Signature
    match public_key.verify(&digest_bytes, signature_bytes) {
        Ok(_) => true, // Success! The signature is valid.
        Err(_) => false, // Failure! The message or signature was tampered with.
    }
}

fn main() {
    let message = b"Node 42 approves the block hash 0xABC123...";
    
    // --- Signing Phase ---
    let (pub_key, signature) = generate_and_sign(message);
    println!("--- Signing Complete ---");
    println!("Public Key ({} bytes): {:?}", pub_key.len(), &pub_key[0..10]);
    println!("Signature ({} bytes): {:?}", signature.len(), &signature[0..10]);

    // --- Verification Phase (The Receiver) ---
    let is_valid = verify_signature(&pub_key, message, &signature);
    println!("\nVerification Result: {}", if is_valid { "✅ Signature is VALID!" } else { "❌ Signature is INVALID!" });

    // --- Tampering Test ---
    let tampered_message = b"Node 42 approves the block hash 0xXYZ987...";
    let is_tampered_valid = verify_signature(&pub_key, tampered_message, &signature);
    println!("Tampering Test Result: {}", if is_tampered_valid { "✅ Valid (ERROR!)" } else { "❌ Invalid (CORRECT!)" });
}
```

***

## 🔢 Section 8.3: Length-Prefixed Canonical Hashing

Hashing is the backbone of almost every modern secure system, especially blockchains.

### Why Hashing is Crucial

A hash function (like SHA-256) takes an input of *any* size (a single letter, a 1GB video file, or a complex transaction) and spits out a fixed-size string of characters (the digest).

*   **Deterministic:** The same input *always* produces the exact same output.
*   **Collision Resistant:** It is computationally infeasible to find two different inputs that produce the same output.
*   **Avalanche Effect:** Changing even a single character in the input results in a completely different output hash.

In a blockchain, each block contains the hash of the *previous* block. This chain of hashes makes the ledger immutable. If an attacker changes an old block, its hash changes, which breaks the link to the next block, invalidating the entire chain.

### Implementing SHA-256 with `ring::digest`

We already used SHA-256 in the crypto section, but let's look at it purely as a hashing utility.

```rust
use ring::digest;
use hex; // We need this crate to convert bytes to readable hex strings

fn calculate_sha256(data: &[u8]) -> String {
    // 1. Create a digest context for SHA-256
    let mut context = digest::Context::new(&digest::SHA256);
    
    // 2. Feed the data into the context
    context.update(data);
    
    // 3. Finalize the hash and get the raw bytes
    let hash_bytes = context.finish();

    // 4. Convert the raw bytes into a human-readable hexadecimal string
    hex::encode(hash_bytes)
}

fn main() {
    let transaction_data = b"Sender: Alice, Receiver: Bob, Amount: 100";
    
    let hash = calculate_sha256(transaction_data);
    
    println!("Original Data: {}", String::from_utf8_lossy(transaction_data));
    println!("SHA-256 Hash:  {}", hash);
    
    // Test the Avalanche Effect: Change one character
    let tampered_data = b"Sender: Alice, Receiver: Bob, Amount: 101";
    let tampered_hash = calculate_sha256(tampered_data);
    
    println!("\nTampered Data: {}", String::from_utf8_lossy(tampered_data));
    println!("Tampered Hash: {}", tampered_hash);
    
    // Notice how drastically different the hashes are!
}
```

***

## 💾 Section 8.4: SQLite Databases in Rust with `rusqlite`

Finally, we tackle persistence. How do we save our state so it survives a program restart? We use a database.

### Why SQLite? The Embedded Powerhouse

Most databases (like PostgreSQL or MySQL) require a separate, running server process. **SQLite** is different. It is a self-contained database engine that stores the entire database in a single file on your disk.

*   **Zero Setup:** You don't need to install a server. You just drop a file (`ledger.db`) and start coding.
*   **Perfect for Local Systems:** It is ideal for desktop applications, embedded systems, and, yes, our ledger application.

We use the `rusqlite` crate to interface with SQLite in Rust.

### Database Management Concepts

#### 1. Connection and WAL Mode
When you open a connection, you are talking to the database file. We often enable **WAL (Write-Ahead Logging)** mode. This mode is crucial for concurrent systems because it allows multiple readers to access the database while a single writer is active, drastically improving performance and stability.

#### 2. Prepared Statements and Type Safety
Instead of writing raw SQL strings and manually converting results, `rusqlite` allows us to use **prepared statements**. This means we define the query structure once, and then safely bind variables to it. This is the gold standard for preventing **SQL Injection** attacks.

#### 3. Thread Safety: The `Mutex` Wrapper
Databases are inherently stateful. If two threads try to write to the database file at the exact same time, you will get corruption. To ensure only one thread accesses the database at a time, we wrap the `rusqlite::Connection` object in a `std::sync::Mutex`.

### 💻 Code Breakdown: `src/db.rs`

This file handles all the low-level database interactions.

```rust
// src/db.rs

use rusqlite::{Connection, Result};
use std::sync::Mutex;
use std::thread;

// We wrap the Connection in a Mutex to ensure thread-safe access.
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Creates a new connection and ensures the necessary tables exist.
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        
        // Enable WAL mode for better concurrency
        conn.execute("PRAGMA journal_mode = WAL;", [])?;

        // Create the ledger table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS transactions (
                id INTEGER PRIMARY KEY,
                sender TEXT NOT NULL,
                recipient TEXT NOT NULL,
                amount REAL NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;
        
        Ok(Database { conn: Mutex::new(conn) })
    }

