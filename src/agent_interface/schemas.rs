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
                    "required": [
                        "title",
                        "rationale"
                    ],
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
                            "items": {
                                "type": "string"
                            },
                            "description": "Classification tags (e.g. ['#architecture', '#srp-refactor', '#benchmark', '#governance'])."
                        },
                        "lessons_learned": {
                            "type": "string",
                            "description": "Educational findings for human learning and future agent runs."
                        },
                        "rules_verified": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
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
                "name": "dispatch_cluster_prompt",
                "description": "Dispatches a code review, boilerplate, or reasoning prompt to the local PC-1/PC-2 cluster at zero token cost via the Central Prompt Hub.",
                "parameters": {
                    "type": "object",
                    "required": [
                        "prompt"
                    ],
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "The prompt or instruction to execute."
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
                    "required": [
                        "event_id",
                        "reason"
                    ],
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
                    "required": [
                        "prompt"
                    ],
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
                    "required": [
                        "prompt"
                    ],
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
                            "items": {
                                "type": "string"
                            },
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
                    "required": [
                        "criteria_type",
                        "criteria_value",
                        "target_model",
                        "reason"
                    ],
                    "properties": {
                        "rule_id": {
                            "type": "string",
                            "description": "Optional unique rule ID. Auto-generated if omitted."
                        },
                        "criteria_type": {
                            "type": "string",
                            "description": "Type of criteria to match: 'session', 'agent', 'tag', 'model', or 'prompt_pattern'.",
                            "enum": [
                                "session",
                                "agent",
                                "tag",
                                "model",
                                "prompt_pattern"
                            ]
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
                    "required": [
                        "rule_id"
                    ],
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
            }
        ]
    });

    Json(tools)
}
