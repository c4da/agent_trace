# Cryptographic Ledger & Studio Project Book Specification

AgentTrace maintains a local, append-only SQLite database backed by cryptographic hash-chaining:

## Hash-Chain Mathematics

For every event $E_i$:
$$\text{event\_hash}_i = \text{SHA-256}(\text{prev\_hash}_{i-1} \parallel \text{timestamp} \parallel \text{node\_id} \parallel \text{event\_type} \parallel \text{target} \parallel \text{canonical\_payload})$$

Where:
- $\text{prev\_hash}_0 = \text{"0"}^{64}$ (Genesis Block)
- Each block is signed by the host node's Ed25519 private key (`.secrets/node_key.pk8`).
- Verification traverses the entire chain backwards to Genesis, verifying all signatures and hash links in $< 5 \text{ ms}$ for 1,000+ blocks.

## Event Schema Reference

| Field | Type | Description |
| :--- | :--- | :--- |
| `event_id` | string | UUIDv4 identifier (`hub-...` or standard UUID). |
| `node_id` | string | Originating node (`PC-1`, `PC-2`, or custom node identity). |
| `session_id` | string | Agent session identifier for multi-turn grouping. |
| `timestamp` | string | ISO-8601 UTC timestamp. |
| `event_type` | string | `Decision`, `LlmInference`, `Write`, `Read`, `Execute`, `SecurityAlert`. |
| `target` | string | Affected file path, URL, command, or milestone title. |
| `action_details` | object | Structured JSON payload containing rationale, tags, and metrics. |
| `security_verdict` | string | `Allowed`, `Flagged`, or `Blocked`. |
| `prev_hash` | string | 64-char hex SHA-256 hash of previous block. |
| `event_hash` | string | 64-char hex SHA-256 hash of current block. |
| `signature` | string | Ed25519 signature of `event_hash`. |
| `public_key` | string | Ed25519 public key in hex. |
