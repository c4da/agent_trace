//! JSON tool schemas exposed to AI agents and MCP clients.

use axum::Json;
use serde_json::{json, Value};

/// Returns the OpenAPI/JSON schemas of all available agent tools.
pub async fn get_agent_tools() -> Json<Value> {
    let tools = json!({
        "schema_version": "1.0.0",
        "protocol": "AgentTrace-Agent-Interface",
        "description": "Autonomous governance, telemetry auditing, and cluster management tools for AI agents.",
        "tools": [
            {
                "name": "record_project_decision",
                "description": "Records an architectural decision, milestone, or learning summary to the cryptographic ledger and Studio Project Book. Automatically signed and chained into the SHA-256 ledger.",
                "parameters": {
                    "type": "object",
                    "required": ["title", "rationale"],
                    "properties": {
                        "title": {
                            "type": "string",
                            "description": "Short milestone or decision title."
                        },
                        "rationale": {
                            "type": "string",
                            "description": "Why this decision was made and architectural rationale."
                        },
                        "tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Classification tags (e.g. ['#architecture', '#srp-refactor', '#benchmark', '#governance'])."
                        },
                        "lessons_learned": {
                            "type": "string",
                            "description": "Educational findings for human learning and future agent runs."
                        },
                        "rules_verified": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Governance rules verified (e.g. ['rule-work-first-ui', 'rule-srp-coding'])."
                        },
                        "diff": {
                            "type": "string",
                            "description": "Optional unified code diff or affected files."
                        }
                    }
                }
            },
            {
                "name": "get_workgroup_rules",
                "description": "Retrieves active Problem Workgroups and their attached Markdown governance rules (such as Work-First UI and SRP coding).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "workgroup_id": {
                            "type": "string",
                            "description": "Optional specific workgroup ID (e.g. 'wg-coding', 'wg-ui-workfirst'). Omit for all."
                        }
                    }
                }
            },
            {
                "name": "upsert_workgroup",
                "description": "Creates or updates a Problem Workgroup hierarchy with assigned agents, models, and domain objectives in the cluster.",
                "parameters": {
                    "type": "object",
                    "required": ["name", "problem_domain"],
                    "properties": {
                        "id": {
                            "type": "string",
                            "description": "Unique workgroup ID (e.g. 'wg-cad-strux', 'wg-governance'). Auto-generated if omitted."
                        },
                        "name": {
                            "type": "string",
                            "description": "Human-readable workgroup name."
                        },
                        "problem_domain": {
                            "type": "string",
                            "description": "Target problem domain or subsystem."
                        },
                        "description": {
                            "type": "string",
                            "description": "Detailed architectural mission and purpose of the workgroup."
                        },
                        "members": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "agent_id": { "type": "string" },
                                    "role": { "type": "string", "enum": ["Lead", "Worker", "Critic", "Observer"] },
                                    "preferred_model": { "type": "string" },
                                    "target_node": { "type": "string" }
                                }
                            },
                            "description": "Team member agents assigned to this workgroup."
                        },
                        "trigger_keywords": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Routing keywords that trigger this workgroup."
                        }
                    }
                }
            },
            {
                "name": "attach_workgroup_rule",
                "description": "Attaches a Markdown (.md) rule file (e.g. Work-First UI, Anti-Monolith SRP) or inline rule text to a Problem Workgroup.",
                "parameters": {
                    "type": "object",
                    "required": ["workgroup_id", "title"],
                    "properties": {
                        "workgroup_id": {
                            "type": "string",
                            "description": "Target workgroup ID."
                        },
                        "title": {
                            "type": "string",
                            "description": "Rule title (e.g. 'Work-First UI Standards', 'Anti-Monolith SRP Invariants')."
                        },
                        "file_path": {
                            "type": "string",
                            "description": "Optional relative or absolute path to a Markdown rule file."
                        },
                        "inline_markdown": {
                            "type": "string",
                            "description": "Optional inline Markdown rule specification content."
                        },
                        "enforce_strict": {
                            "type": "boolean",
                            "description": "Whether violation of this rule should strictly block execution. Default true."
                        }
                    }
                }
            },
            {
                "name": "deploy_cluster_worker",
                "description": "Triggers zero-walk Over-the-Air (OTA) binary distribution and remote push provisioning of an agent_trace_gw worker node over the LAN.",
                "parameters": {
                    "type": "object",
                    "required": ["target_ip"],
                    "properties": {
                        "target_ip": {
                            "type": "string",
                            "description": "Target worker node IP address on the LAN (e.g. '192.168.1.102')."
                        },
                        "port": {
                            "type": "integer",
                            "description": "Target listening port (default: 8642)."
                        },
                        "method": {
                            "type": "string",
                            "description": "Provisioning method: 'hermes_or_ssh' or 'powershell'. Default: 'hermes_or_ssh'."
                        }
                    }
                }
            },
            {
                "name": "check_worker_connection",
                "description": "Performs live network reachability and health checks against a worker node (or all cluster nodes), probing ports 8080 (Gateway), 8642 (Hermes), and 11434 (Ollama), returning latency and gateway installation status.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "target_ip": {
                            "type": "string",
                            "description": "Target worker node IP (e.g. '192.168.1.102') or 'all' to check all registered/discovered cluster nodes. Defaults to 'all'."
                        }
                    }
                }
            },
            {
                "name": "check_cluster_health",
                "description": "Performs an autonomous cluster health check across all registered nodes, detecting sleep mode/standby, power-offs, and service degradation, and updating model routing availability.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "force_reprobe": {
                            "type": "boolean",
                            "description": "Whether to force an active real-time network probe across all nodes instead of returning cached watchdog results. Defaults to true.",
                            "default": true
                        }
                    }
                }
            },
            {
                "name": "get_cluster_bootstrap_command",
                "description": "Generates the exact 1-liner network bootstrap command to install and connect a remote worker node to the Lead Node over the LAN.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "platform": {
                            "type": "string",
                            "enum": ["powershell", "bash", "all"],
                            "description": "Target OS shell format: 'powershell' (Windows), 'bash' (Linux/macOS), or 'all'. Default: 'all'."
                        }
                    }
                }
            },
            {
                "name": "dispatch_cluster_prompt",
                "description": "Dispatches a code review, boilerplate, or reasoning prompt to the local PC-1/PC-2 cluster at zero token cost via the Central Prompt Hub.",
                "parameters": {
                    "type": "object",
                    "required": ["prompt"],
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "The prompt or instruction to execute."
                        },
                        "workgroup_id": {
                            "type": "string",
                            "description": "Target workgroup (e.g. 'wg-coding', 'wg-image-gen', 'auto'). Default: 'auto'."
                        },
                        "mode": {
                            "type": "string",
                            "description": "Execution mode: 'auto', 'pipeline' (draft -> verify), or 'swarm' (parallel). Default: 'auto'."
                        },
                        "requested_model": {
                            "type": "string",
                            "description": "Optional explicit model override (e.g. 'qwen3.8:27b', 'hermes-agent')."
                        }
                    }
                }
            },
            {
                "name": "verify_ledger",
                "description": "Cryptographically verifies the entire SHA-256 hash-chain and Ed25519 signatures of the telemetry ledger. Returns block count, chain validity, head hash, and broken block details if tampered.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "query_telemetry",
                "description": "Searches and filters recorded agent actions (file reads, writes, commands, LLM calls) with optional target substring, event type, or limit.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "target_contains": {
                            "type": "string",
                            "description": "Optional substring to filter targets (e.g. '.env', 'rm -rf', 'config.yaml')."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of events to return (default 50, max 200).",
                            "default": 50
                        }
                    }
                }
            },
            {
                "name": "inspect_cluster",
                "description": "Returns current cluster topology, all self-announced models across nodes, parameter tiers, and health status.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "flag_incident",
                "description": "Allows an AI agent auditor to flag a suspicious telemetry event or security policy violation in the ledger.",
                "parameters": {
                    "type": "object",
                    "required": ["event_id", "reason"],
                    "properties": {
                        "event_id": {
                            "type": "string",
                            "description": "The unique event ID of the block being flagged."
                        },
                        "reason": {
                            "type": "string",
                            "description": "Reasoning explaining why this event was flagged as a violation or anomaly."
                        }
                    }
                }
            },
            {
                "name": "explain_route",
                "description": "Tests which cluster model and node would be selected for a given prompt without executing the inference.",
                "parameters": {
                    "type": "object",
                    "required": ["prompt"],
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "The user prompt or instruction to classify and route."
                        },
                        "requested_model": {
                            "type": "string",
                            "description": "Optional model override (e.g. 'auto', 'qwen3.8:27b'). Defaults to 'auto'.",
                            "default": "auto"
                        }
                    }
                }
            },
            {
                "name": "decide_route",
                "description": "Arbitrates and determines optimal prompt routing across the cluster, taking into account model capabilities, node availability, and active agent override rules.",
                "parameters": {
                    "type": "object",
                    "required": ["prompt"],
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "The user prompt or instruction to arbitrate and route."
                        },
                        "session_id": {
                            "type": "string",
                            "description": "Optional session ID to match against session override rules."
                        },
                        "agent_id": {
                            "type": "string",
                            "description": "Optional agent ID to match against agent override rules."
                        },
                        "requested_model": {
                            "type": "string",
                            "description": "Optional model requested (e.g. 'auto', 'qwen3.8:27b'). Defaults to 'auto'."
                        },
                        "tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional classification or context tags."
                        }
                    }
                }
            },
            {
                "name": "set_route_override",
                "description": "Registers an autonomous routing override rule into the gateway so subsequent prompts matching session, agent, or tag criteria are forced to a specific model or node.",
                "parameters": {
                    "type": "object",
                    "required": ["criteria_type", "criteria_value", "target_model", "reason"],
                    "properties": {
                        "rule_id": {
                            "type": "string",
                            "description": "Optional unique rule ID. Auto-generated if omitted."
                        },
                        "criteria_type": {
                            "type": "string",
                            "description": "Type of criteria to match: 'session', 'agent', 'tag', 'model', or 'prompt_pattern'.",
                            "enum": ["session", "agent", "tag", "model", "prompt_pattern"]
                        },
                        "criteria_value": {
                            "type": "string",
                            "description": "Value to match (e.g. session ID 'sec-101', agent ID 'hermes-coder', model 'gemini*', prompt pattern '*test*')."
                        },
                        "target_model": {
                            "type": "string",
                            "description": "Target model to force routing to (e.g. 'qwen3.8:27b', 'deepseek-r1:1.5b')."
                        },
                        "target_node": {
                            "type": "string",
                            "description": "Optional target cluster node ID (e.g. 'PC-1', 'PC-2')."
                        },
                        "reason": {
                            "type": "string",
                            "description": "Justification for this routing override rule."
                        }
                    }
                }
            },
            {
                "name": "list_route_overrides",
                "description": "Lists all currently active agent routing overrides in the gateway.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "remove_route_override",
                "description": "Removes an active routing override rule by its rule ID.",
                "parameters": {
                    "type": "object",
                    "required": ["rule_id"],
                    "properties": {
                        "rule_id": {
                            "type": "string",
                            "description": "The unique rule ID of the override to remove."
                        }
                    }
                }
            },
            {
                "name": "analyze_telemetry",
                "description": "Computes multi-dimensional telemetry analytics across trace events in the ledger, returning target hotspots, agent behavioral profiles, activity time-series, and security anomalies.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "get_access_policies",
                "description": "Lists all active agent sandbox and whitelist policies governing allowed read, write, and network pull paths.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "set_access_policy",
                "description": "Creates or updates an access policy restricting or permitting where a specific agent can read, write, or pull data.",
                "parameters": {
                    "type": "object",
                    "required": ["agent_id"],
                    "properties": {
                        "agent_id": {
                            "type": "string",
                            "description": "Agent identifier (e.g. 'hermes-agent', 'claude-code', or '*' for global baseline)."
                        },
                        "read_whitelist": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of allowed path prefixes/patterns for reading data."
                        },
                        "write_whitelist": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of allowed path prefixes/patterns for writing data."
                        },
                        "network_whitelist": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of allowed domain or URL prefixes for outbound network requests."
                        },
                        "blocked_patterns": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of forbidden sensitive substrings/patterns (e.g. '.env', 'id_rsa')."
                        },
                        "enforcing": {
                            "type": "boolean",
                            "description": "Whether violations are strictly Blocked (true) or Flagged for audit only (false). Defaults to true."
                        }
                    }
                }
            },
            {
                "name": "evaluate_access_policy",
                "description": "Evaluates whether a proposed data read/write or network pull is permitted under current agent sandbox rules without executing it.",
                "parameters": {
                    "type": "object",
                    "required": ["agent_id", "action", "target"],
                    "properties": {
                        "agent_id": {
                            "type": "string",
                            "description": "Agent ID proposing the action."
                        },
                        "action": {
                            "type": "string",
                            "enum": ["Read", "Write", "Execute", "NetworkPull"],
                            "description": "Type of access action."
                        },
                        "target": {
                            "type": "string",
                            "description": "The file path or URL destination to evaluate."
                        }
                    }
                }
            },
            {
                "name": "get_delegation_config",
                "description": "Retrieves the current cluster delegation and zero-token economy enforcement settings.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "set_delegation_config",
                "description": "Updates cluster delegation and zero-token economy enforcement settings.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "intercept_external_models": {
                            "type": "boolean",
                            "description": "Whether to intercept external cloud models (gemini*, gpt*, claude*) and divert them to the local cluster."
                        },
                        "zero_token_mode": {
                            "type": "boolean",
                            "description": "Whether strict zero-token economy is actively enforced."
                        },
                        "auto_switch_offline_models": {
                            "type": "boolean",
                            "description": "Whether to auto-switch unavailable models to active downloaded cluster candidates."
                        },
                        "default_fallback_model": {
                            "type": "string",
                            "description": "Default fallback model ID."
                        }
                    }
                }
            },
            {
                "name": "get_telemetry_handoff",
                "description": "Returns a single-call Telemetry Pickup & Handoff Snapshot for another AI agent joining or auditing the session: cluster health, SHA-256/Ed25519 ledger validity, active unresolved security alerts, recent unified code diffs, Safe Zones / Playgrounds, and agent policies.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "recent_limit": {
                            "type": "integer",
                            "description": "Max recent events/diffs to include in the snapshot (default 15).",
                            "default": 15
                        }
                    }
                }
            },
            {
                "name": "get_security_alerts",
                "description": "Retrieves security policy violations (Blocked or Flagged events). By default returns only active unresolved alerts; set include_acked=true for full historical alerts.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "include_acked": {
                            "type": "boolean",
                            "description": "Include already-acknowledged/dismissed alerts.",
                            "default": false
                        }
                    }
                }
            },
            {
                "name": "acknowledge_alert",
                "description": "Non-destructively acknowledges/dismisses a security alert by event_id (or all active alerts when reset_all=true) without mutating the SHA-256 ledger hash chain.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "event_id": {
                            "type": "string",
                            "description": "Event ID of the alert to acknowledge."
                        },
                        "reset_all": {
                            "type": "boolean",
                            "description": "If true, acknowledges all currently active alerts.",
                            "default": false
                        }
                    }
                }
            },
            {
                "name": "list_containment_zones",
                "description": "Lists all configured Safe Zones (VaultNoAccess, GuardedReadOnly) and Danger Zone Playgrounds (AgentPlayground).",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "upsert_containment_zone",
                "description": "Creates or updates a Spatial Containment Zone (VaultNoAccess, GuardedReadOnly, or AgentPlayground) enforced globally ('*') or for a specific agent.",
                "parameters": {
                    "type": "object",
                    "required": ["name", "path_prefix", "kind"],
                    "properties": {
                        "zone_id": {
                            "type": "string",
                            "description": "Optional zone ID (auto-generated if empty)."
                        },
                        "name": {
                            "type": "string",
                            "description": "Human-readable name for the zone."
                        },
                        "path_prefix": {
                            "type": "string",
                            "description": "Filesystem path prefix for the zone."
                        },
                        "kind": {
                            "type": "string",
                            "enum": ["VaultNoAccess", "GuardedReadOnly", "AgentPlayground"],
                            "description": "Zone posture: VaultNoAccess (blocks all access), GuardedReadOnly (allows Read, blocks Write), or AgentPlayground (allows Read/Write/Execute)."
                        },
                        "applies_to": {
                            "type": "string",
                            "description": "'*' for all agents, or a specific agent_id.",
                            "default": "*"
                        },
                        "exempt_agents": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional list of trusted agent IDs exempt from VaultNoAccess/GuardedReadOnly restrictions."
                        }
                    }
                }
            },
            {
                "name": "confine_agent_to_playground",
                "description": "Locks or unlocks an agent (or '*' all agents) so they can ONLY write or execute commands inside Danger Zone Playgrounds.",
                "parameters": {
                    "type": "object",
                    "required": ["agent_id", "confined"],
                    "properties": {
                        "agent_id": {
                            "type": "string",
                            "description": "Target agent ID (or '*' for all agents)."
                        },
                        "confined": {
                            "type": "boolean",
                            "description": "True to confine writes/executes strictly to AgentPlaygrounds; false to lift confinement."
                        }
                    }
                }
            },
            {
                "name": "intercept_ai_guard",
                "description": "Runs the synchronous Pre-Execution Hard AI Guard (<1.5ms) on any proposed prompt, shell command, or file path to detect DLP secret leaks (Anthropic/OpenAI/AWS/GCP/GitHub/PEM keys), destructive OS commands, prompt injections, and spatial zone violations before execution.",
                "parameters": {
                    "type": "object",
                    "required": ["action"],
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["LlmInference", "Execute", "Write", "Read"],
                            "description": "Type of action to inspect."
                        },
                        "prompt_text": {
                            "type": "string",
                            "description": "Optional outbound prompt text to scan/scrub for leaked credentials or prompt injection."
                        },
                        "command": {
                            "type": "string",
                            "description": "Optional shell command to inspect against destructive/exfiltration rules."
                        },
                        "target_path": {
                            "type": "string",
                            "description": "Optional filesystem path to verify against Safe Zones and Playgrounds."
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["HardBlock", "RedactAndForward", "AuditOnly"],
                            "description": "Enforcement mode (default: 'HardBlock')."
                        }
                    }
                }
            },
            {
                "name": "protect_or_revert_cas_file",
                "description": "Manages the in-memory SHA-256 Pre-Image Content-Addressable Storage (CAS) Guard: snapshot a file before risky refactoring ('protect'), verify/auto-revert unauthorized modifications or deletions in <1ms ('enforce'), or list all protected pre-image files ('list').",
                "parameters": {
                    "type": "object",
                    "required": ["operation"],
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["protect", "enforce", "list"],
                            "description": "'protect' snapshots a file in memory, 'enforce' checks and auto-reverts tampering/deletion, 'list' returns all protected snapshots."
                        },
                        "path": {
                            "type": "string",
                            "description": "File path to snapshot or verify (required for 'protect' and 'enforce')."
                        },
                        "strictly_guarded": {
                            "type": "boolean",
                            "description": "Whether any out-of-band modification or deletion should be automatically reverted. Default true."
                        }
                    }
                }
            },
            {
                "name": "encrypt_vault_secret",
                "description": "Encrypts a plaintext secret using the Gateway's ChaCha20-Poly1305 AEAD Vault into an authenticated 'ENCv1:<nonce>:<ciphertext>' envelope for safe storage in .secrets/agenttrace.env.",
                "parameters": {
                    "type": "object",
                    "required": ["plaintext"],
                    "properties": {
                        "plaintext": {
                            "type": "string",
                            "description": "Plaintext API key or token to encrypt."
                        }
                    }
                }
            },
            {
                "name": "export_siem_elk",
                "description": "Exports cryptographically signed telemetry and AI Guard security events formatted in Elastic Common Schema (ECS 8.11.0) NDJSON for Elasticsearch /_bulk or Filebeat/Splunk ingestion.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of recent events to export (default 25, max 500)."
                        },
                        "format": {
                            "type": "string",
                            "enum": ["bulk", "stream"],
                            "description": "'bulk' for Elasticsearch /_bulk NDJSON, or 'stream' for raw ECS NDJSON lines."
                        }
                    }
                }
            },
            {
                "name": "manage_admin_model_governance",
                "description": "Admin Governance Plane tool to inspect global/per-model restrictions ('list'), upsert model-specific restrictions ('upsert_model'), or evaluate how the Shielded Client layer enforces restrictions on a given model ('evaluate').",
                "parameters": {
                    "type": "object",
                    "required": ["operation"],
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["list", "upsert_model", "evaluate"],
                            "description": "'list' returns global + per-model profiles; 'upsert_model' updates restrictions for a model pattern; 'evaluate' dry-runs a request against a model profile."
                        },
                        "model": {
                            "type": "string",
                            "description": "Target model name or pattern (e.g., 'claude-*', 'deepseek-r1:1.5b', 'qwen3.8:27b', 'gpt-*')."
                        },
                        "command": {
                            "type": "string",
                            "description": "Optional shell command to test during 'evaluate'."
                        },
                        "target_path": {
                            "type": "string",
                            "description": "Optional file path to test during 'evaluate'."
                        },
                        "profile": {
                            "type": "object",
                            "description": "Full ModelRestrictionProfile JSON object when operation='upsert_model'."
                        }
                    }
                }
            },
            {
                "name": "manage_learned_skills",
                "description": "Auto-Skill Forge & De Facto Learning tool: list learned skills ('list'), mine telemetry trajectories to distill new skills ('distill'), upsert a central or per-agent/model skill ('upsert'), export a skill to disk as .agents/skills/<slug>/SKILL.md and .claude/commands/<slug>.md ('export'), or retrieve the v1.1.0 Token-Savings ROI benchmark ('roi_benchmark').",
                "parameters": {
                    "type": "object",
                    "required": ["operation"],
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["list", "distill", "upsert", "export", "roi_benchmark"],
                            "description": "'list' returns all learned skills; 'distill' mines telemetry events; 'upsert' saves a LearnedSkill; 'export' writes SKILL.md and .claude/commands/<slug>.md; 'roi_benchmark' returns token compression metrics (v1.1.0)."
                        },
                        "skill_id": {
                            "type": "string",
                            "description": "Skill ID or slug (required for 'export')."
                        },
                        "skill": {
                            "type": "object",
                            "description": "LearnedSkill JSON object when operation='upsert'."
                        }
                    }
                }
            }
        ]
    });

    Json(tools)
}
