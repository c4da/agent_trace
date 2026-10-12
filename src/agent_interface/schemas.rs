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
            }
        ]
    });

    Json(tools)
}
