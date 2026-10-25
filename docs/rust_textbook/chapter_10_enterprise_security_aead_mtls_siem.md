# Chapter 10: Practical Rust Security Engineering — Optional AEAD Vaults (`ChaCha20-Poly1305`), Constant-Time Auth, mTLS HMACs & SIEM Streaming

---

## 🦀 Designing Optional, Zero-Cost Security in Rust

In Chapter 8, you learned how AgentTrace uses `ring` to sign ledger blocks with Ed25519 and hash chains with SHA-256. In this chapter, we will dissect the four enterprise security modules added in `v1.0.0`—and see how idiomatic Rust lets us keep **every security lock 100% optional by default** (so developers have zero setup friction) while guaranteeing memory-safe, sub-millisecond enforcement when enabled:

1. **Optional Secrets-at-Rest Encryption (`ChaCha20-Poly1305` AEAD)** in [`src/security_hardening.rs`](../../src/security_hardening.rs) & [`src/secrets.rs`](../../src/secrets.rs).
2. **Constant-Time Token Verification (`constant_time_eq`)** to prevent timing side-channel leaks.
3. **Inter-Gateway TLS / mTLS Thumbprint Pinning & Anti-Replay HMACs**.
4. **Zero-Allocation SIEM / Elasticsearch (`/_bulk` ECS 8.11) NDJSON Streaming** in [`src/siem_exporter.rs`](../../src/siem_exporter.rs).

---

## 1. Plaintext by Default, `ChaCha20-Poly1305` AEAD When You Need It

Why shouldn't a local developer tool encrypt `.secrets/agenttrace.env` by default? Because during local development, engineers want to open `.secrets/agenttrace.env` in a text editor and read or change `WORKER_HERMES_KEY=my-secret-key-12345` directly!

In Rust, we can support **both** plaintext and military-grade authenticated encryption seamlessly using prefix dispatch (`ENCv1:`):

```rust
// From src/secrets.rs:
// Secrets are stored in plaintext by default; only decrypt if user explicitly encrypted with ENCv1:
let val = if crate::security_hardening::EncryptedSecretVault::is_encrypted(&raw_val) {
    vault.decrypt_or_passthrough(&raw_val).unwrap_or(raw_val)
} else {
    raw_val
};
```

### How `ring::aead::CHACHA20_POLY1305` Works in Rust
When a user opts in to encrypting a secret via `POST /api/v1/vault/encrypt`, [`EncryptedSecretVault::encrypt`](../../src/security_hardening.rs) performs **Authenticated Encryption with Associated Data (AEAD)**:
1. Generates a random 12-byte (96-bit) nonce using the OS CSPRNG (`SystemRandom`).
2. Copies the plaintext into a mutable byte buffer (`let mut in_out = plaintext.as_bytes().to_vec()`).
3. Calls `key.seal_in_place_append_tag(nonce, Aad::from(b"agenttrace-secret-v1"), &mut in_out)`, which encrypts the bytes in-place and appends a 16-byte `Poly1305` authentication tag.
4. If anyone modifies even 1 bit of the ciphertext on disk, `key.open_in_place(...)` returns an `Err`, preventing tampered secrets from ever loading!

---

## 2. Writing a Constant-Time Comparator in Rust (`constant_time_eq`)

By default, AgentTrace leaves API authentication disabled (`AGENTTRACE_REQUIRE_AUTH=false`) so local tools work without headers. When `AGENTTRACE_REQUIRE_AUTH=true` is enabled, why don't we use `provided_token == expected_token`?

Standard `==` stops at the very first mismatched byte. An attacker measuring microsecond response times can guess a secret token one character at a time. Instead, we accumulate bitwise XOR (`^`) differences across the entire slice:

```rust
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.is_empty() || b_bytes.is_empty() || a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (&x, &y) in a_bytes.iter().zip(b_bytes.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
```
Notice how the loop **always** inspects every byte before checking `diff == 0`!

---

## 3. Inter-Gateway mTLS & Anti-Replay HMAC Signing

When connecting multiple AgentTrace Gateways across an enterprise network (`AGENTTRACE_MTLS_REQUIRED=true`), [`InterGatewayMtlsConfig`](../../src/security_hardening.rs) combines:
- **`HashSet<String>` O(1) Thumbprint Pinning**: Checks the peer's X.509 client certificate SHA-256 fingerprint (`x-client-cert-sha256`).
- **HMAC-SHA256 + Clock-Skew Window (`ring::hmac`)**:
  ```rust
  let body_hash = hex::encode(digest(&SHA256, payload_bytes).as_ref());
  let signing_input = format!("{}|{}|{}", node_id, timestamp_epoch, body_hash);
  let tag = hmac::sign(&key, signing_input.as_bytes());
  ```
  By binding `timestamp_epoch` inside the HMAC signature and asserting `(now_epoch - req_ts).abs() <= 60`, any captured packet replayed over 60 seconds later is rejected.

---

## 4. Formatting Elastic Common Schema (ECS 8.11) NDJSON for ELK (`src/siem_exporter.rs`)

Elasticsearch's `/_bulk` API expects **Newline-Delimited JSON (NDJSON)** where every document is preceded by an action metadata line:
```json
{"index":{"_index":"agenttrace-ecs-events","_id":"..."}}
{"@timestamp":"2026-10-24T01:00:00Z","ecs":{"version":"8.11.0"},"event":{"kind":"alert",...}}
```
In [`src/siem_exporter.rs`](../../src/siem_exporter.rs), we build this in pure Rust using `serde_json::to_string` without needing any heavy third-party Elasticsearch SDK crates—keeping compile times fast and the binary footprint at `12.45 MB`.
