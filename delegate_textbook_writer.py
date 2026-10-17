import urllib.request
import json
import sys
import os
import time

# Ensure UTF-8 output
sys.stdout.reconfigure(encoding='utf-8')

OUTPUT_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "docs", "textbook")
os.makedirs(OUTPUT_DIR, exist_ok=True)

GATEWAY_URL = "http://localhost:8080/v1/chat/completions"
MODEL = "gemma4:latest"

CHAPTERS = [
    {
        "filename": "chapter_01_networking_stack.md",
        "title": "Chapter 1: The Networking Stack & Protocols",
        "prompt": """You are an expert distributed systems and computer networking professor.
Write a comprehensive, in-depth academic and practical textbook chapter titled:
'Chapter 1: The Networking Stack & Protocols'.

Cover in thorough detail:
1. The OSI 7-Layer Model vs. The 4-Layer TCP/IP Model (Physical, Data Link, Network, Transport, Session, Presentation, Application).
   Include clear ASCII diagrams comparing both models.
2. IP Addressing & Subnetting:
   - IPv4 vs. IPv6 header structures.
   - Private subnets (RFC 1918: 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16).
   - CIDR notation, subnet masks, network address vs broadcast address.
3. Transport Protocols:
   - TCP: Connection lifecycle (SYN, SYN-ACK, ACK, 3-way handshake, 4-way FIN teardown, TIME_WAIT).
   - Flow control, sliding window, congestion control (CUBIC, BBR).
   - UDP: Datagram structure, connectionless semantics, MTU, packet fragmentation, and why UDP is preferred for real-time telemetry beacons.
4. Sockets & Operating System Multiplexing:
   - BSD socket API (socket, bind, listen, accept).
   - Non-blocking I/O, epoll (Linux) and IOCP (Windows).
5. Practical connection to AI agent swarms:
   - How network latency impacts token generation loops and cluster RPC overhead.

Format as clean, comprehensive GitHub Markdown with formulas, ASCII diagrams, and code snippets where appropriate.
"""
    },
    {
        "filename": "chapter_02_application_protocols.md",
        "title": "Chapter 2: Application Protocols & Streaming",
        "prompt": """You are an expert distributed systems engineer and networking professor.
Write a comprehensive, in-depth academic and practical textbook chapter titled:
'Chapter 2: Application Protocols & Streaming'.

Cover in thorough detail:
1. Evolution of HTTP:
   - HTTP/1.1: Request/response model, pipelining, head-of-line blocking, keep-alive connections.
   - HTTP/2: Binary framing layer, stream multiplexing over a single TCP connection, HPACK header compression, server push.
   - HTTP/3: QUIC transport layer over UDP, eliminating TCP head-of-line blocking across lossy networks.
2. RESTful Architecture & Resource Modeling:
   - Principles of REST (statelessness, uniform interface, idempotency).
   - HTTP methods (GET, POST, PUT, DELETE) and status code semantics (200, 201, 401, 403, 404, 500).
3. JSON-RPC 2.0 & Model Context Protocol (MCP):
   - JSON-RPC 2.0 specification: Request, Response, Notification, Batching, and error objects.
   - Model Context Protocol (MCP): Architectural roles of Host (IDE/Client), Server (Tools/Resources), and Protocol Handlers.
4. Real-Time Streaming for AI Generation:
   - Server-Sent Events (SSE): `text/event-stream` MIME type, event formatting (`data: ...\n\n`), connection keep-alives.
   - WebSockets vs. SSE: Why unidirectional SSE is optimal for token streaming, whereas WebSockets excel at full duplex interactive controls.
   - Rust implementation patterns for SSE streams using Tokio and Axum (`async-stream`).

Format as clean, comprehensive GitHub Markdown with ASCII flow diagrams and code examples.
"""
    },
    {
        "filename": "chapter_03_edge_proxies_routing.md",
        "title": "Chapter 3: Edge Architectures, Reverse Proxies & Routing",
        "prompt": """You are an expert distributed systems and proxy infrastructure architect.
Write a comprehensive, in-depth academic and practical textbook chapter titled:
'Chapter 3: Edge Architectures, Reverse Proxies & Routing'.

Cover in thorough detail:
1. Proxies & Gateways:
   - Forward Proxy vs. Reverse Proxy vs. API Gateway vs. Service Mesh Sidecar.
   - High-throughput asynchronous event loops in Rust: Tokio runtime, Axum framework, and Tower middleware stack.
2. Intercepting & Multiplexing LLM Inferences:
   - Streaming Reverse Proxying: Reading chunked SSE byte streams from upstream LLMs and forwarding to clients without buffering.
   - On-the-fly telemetry extraction: Parsing partial JSON deltas in real-time to compute token count, latency, and duration without inducing client latency.
3. Intelligent Route Arbitration:
   - Model capability matrix: Classifying prompts by intent (e.g. Complex Reasoning vs. Fast Boilerplate vs. Sanity Verification).
   - Dynamic routing fallback cascades: Primary node failure handling, health-check probing, and circuit breaking.
4. Concrete Rust Implementation Blueprint:
   - How `ProxyConfig` and `handle_chat_completions` coordinate HTTP request translation, SSE streaming, and ledger ingestion.

Format as clean, comprehensive GitHub Markdown with detailed architecture diagrams.
"""
    },
    {
        "filename": "chapter_04_mesh_discovery_clustering.md",
        "title": "Chapter 4: Local Mesh Discovery & Zero-Conf Clustering",
        "prompt": """You are a senior systems engineer specializing in distributed networking and edge clusters.
Write a comprehensive, in-depth academic and practical textbook chapter titled:
'Chapter 4: Local Mesh Discovery & Zero-Conf Clustering'.

Cover in thorough detail:
1. Zero-Configuration Networking (Zero-Conf):
   - Principles of LAN discovery without centralized DNS or hardcoded IP tables.
   - UDP Broadcast (`255.255.255.255`) vs. UDP Multicast (e.g., mDNS / SSDP).
   - Designing the UDP Discovery Beacon in Rust: Socket binding (`0.0.0.0:0`), enabling broadcast flags (`set_broadcast(true)`), and cyclic packet emission.
2. Node Lifecycle & Cluster Topology:
   - Edge Worker registration: Ingesting node hardware metadata, CUDA capabilities, and available local weights.
   - Heartbeat protocols: TTL-based last-seen timestamps and autonomous pruning of stale edge workers.
3. LAN Security & Network Traversal:
   - Subnet boundary constraints (Layer 2 vs. Layer 3 broadcast domains).
   - Firewall considerations: Inbound port opening (`:8080`, `:8642`, `:8089`).
   - Token-based mutual authentication between Lead Gateways and Edge Workers (`x-agenttrace-token`).

Format as clean, comprehensive GitHub Markdown with network packet diagrams and Rust code examples.
"""
    },
    {
        "filename": "chapter_05_anatomy_of_ai_agents.md",
        "title": "Chapter 5: Anatomy of Autonomous AI Agents",
        "prompt": """You are a leading AI researcher and agentic systems architect.
Write a comprehensive, in-depth academic and practical textbook chapter titled:
'Chapter 5: Anatomy of Autonomous AI Agents'.

Cover in thorough detail:
1. The Evolution of Machine Agency:
   - Beyond Next-Token Prediction: The transition from passive text generators to proactive autonomous agents.
   - Agent cognitive architectures: ReAct (Reason + Act), Chain-of-Thought (CoT), Reflexion, and Plan-and-Solve loops.
2. Tool Calling & External World Grounding:
   - OpenAI tool/function calling specification: JSON Schema parameters, type checking, and system prompt engineering.
   - The execution loop: Intercepting `tool_calls`, executing sandboxed system commands or APIs, and feeding observation payloads back into context.
3. The Model Context Protocol (MCP) Standard:
   - Universal interoperability: Decoupling LLM frontends from database and tool backends.
   - MCP Server lifecycle: `initialize`, `tools/list`, `tools/call`, `resources/list`, and `prompts/list`.
4. Agent Context & Memory Systems:
   - Context window constraints and quadratic attention complexity.
   - Ephemeral working memory vs. persistent vector/relational memories.

Format as clean, comprehensive GitHub Markdown with state transition diagrams and flowcharts.
"""
    },
    {
        "filename": "chapter_06_swarms_and_token_economy.md",
        "title": "Chapter 6: Multi-Agent Swarms & Token Economies",
        "prompt": """You are an AI infrastructure economist and swarm orchestration architect.
Write a comprehensive, in-depth academic and practical textbook chapter titled:
'Chapter 6: Multi-Agent Swarms & Token Economies'.

Cover in thorough detail:
1. Multi-Agent Topology:
   - Hierarchical Swarms (Lead Orchestrator vs. Specialist Worker Nodes) vs. Peer-to-Peer Consensus Swarms.
   - Specialization by parameter tier: High-parameter models (27B+) for system design and architecture; quantized edge models (1.5B - 7.5B) for test execution and syntax validation.
2. The Zero-Token Economy Directive:
   - The economic and security case for local offline compute: Eliminating external API token consumption, avoiding data leakage to cloud providers, and achieving zero marginal inference cost.
   - Balancing frontier reasoning engines (Gemini/Claude) with local edge swarms (`qwen3.8:27b`, `gemma4:latest`, `deepseek-r1:1.5b`).
3. Autonomous Routing & Dynamic Arbitration:
   - Route override mechanisms: Session-based, Agent identity-based, and Tag-based matching.
   - The AgentTrace Delegation Engine: How supervisory agents programmatically re-route swarms based on real-time task complexity.

Format as clean, comprehensive GitHub Markdown with swarm topology diagrams and economics formulas.
"""
    },
    {
        "filename": "chapter_07_cryptographic_provenance.md",
        "title": "Chapter 7: Cryptographic Provenance & Tamper-Proof Audit Trails",
        "prompt": """You are a senior cryptographer and distributed systems security researcher.
Write a comprehensive, in-depth academic and practical textbook chapter titled:
'Chapter 7: Cryptographic Provenance & Tamper-Proof Audit Trails'.

Cover in thorough detail:
1. The Vulnerability of Conventional Logging:
   - Why flat text logs and standard databases fail in autonomous AI environments: Mutable state, log injection, rogue agent self-erasure, lack of non-repudiation.
2. Asymmetric Cryptography & Node Identity:
   - Ed25519 (Edwards-curve Digital Signature Algorithm): Public/private key generation, performance benefits, resistance to timing attacks.
   - Persistent node identities: Generating and loading `.pk8` keypairs on boot.
3. Cryptographic Hash Chaining:
   - The Genesis Block and SHA-256 state transitions (`prev_hash` -> `event_hash`).
   - Canonical Length-Prefixed Hashing (V2): Why simple delimiters (`|`) are vulnerable to field-extension attacks and how length-prefixed bytes guarantee collision resistance.
   - Bounding `security_verdict`, `target`, `event_type`, `duration_ms`, and `diff` into the signature.
4. Tamper Detection & Verification:
   - The mathematical verification loop: Detecting modified verdicts, altered targets, or inserted blocks in SQLite WAL tables.
   - Real-world attack scenario: A rogue agent modifies an access record in SQLite; the cryptographic verification engine immediately identifies the exact broken block ID.

Format as clean, comprehensive GitHub Markdown with cryptographic schemas and mathematical explanations.
"""
    },
    {
        "filename": "chapter_08_governance_sandboxing_analytics.md",
        "title": "Chapter 8: Agent Governance, Sandboxing & Behavioral Analytics",
        "prompt": """You are a chief AI safety officer and infrastructure security architect.
Write a comprehensive, in-depth academic and practical textbook chapter titled:
'Chapter 8: Agent Governance, Sandboxing & Behavioral Analytics'.

Cover in thorough detail:
1. Sandboxing & Access Control:
   - Dual-layer defense: Whitelisting authorized directories (`C:/cada/projects`) vs. strict blacklisting of sensitive targets (`.env`, `.secrets`, `id_rsa`, `system32`).
   - The Blacklist Precedence Invariant: Why blacklists must be checked before whitelists to prevent data exfiltration within allowed working trees.
   - Gateway Ingestion-Time Enforcement: Cryptographically recording blocked attempts into the hash chain for forensic audit, then returning `HTTP 403 Forbidden`.
2. Behavioral Profiling & Dynamic Threat Scoring:
   - Metrics: Read volume, Write volume, Command execution, Read-to-Write ratio.
   - The Blast Radius Score formula: `score = min(100, writes * 10 + commands * 5 + violations * 25)`.
   - Threat level categorization: `LOW`, `MEDIUM`, `HIGH`, `CRITICAL`.
3. Anomaly Detection & Temporal Analysis:
   - Target Hotspot analysis: Identifying top accessed/modified files and probing patterns.
   - 24-window hourly temporal bucketing for sudden spikes in write or command activity.

Format as clean, comprehensive GitHub Markdown with threat model matrices, algorithms, and metric tables.
"""
    },
    {
        "filename": "chapter_09_agent_trace_implementation.md",
        "title": "Chapter 9: The AgentTrace Gateway Implementation",
        "prompt": """You are the lead architect of the AgentTrace Gateway project.
Write a comprehensive, in-depth engineering textbook chapter titled:
'Chapter 9: The AgentTrace Gateway Implementation'.

Cover in thorough detail:
1. System Architecture Overview:
   - The unified gateway: Reverse proxy, cryptographic ledger, model registry, policy engine, and MCP server in a single high-performance Rust binary.
2. Deep Dive into the Modular Codebase:
   - `src/main.rs`: Lightweight bootstrap orchestrator.
   - `src/handlers.rs`: Modular Axum router and HTTP endpoints.
   - `src/services.rs`: Background services (model introspector, worker forwarder, UDP discovery beacon).
   - `src/db.rs`: SQLite WAL persistence and ledger verification engine.
   - `src/ledger.rs`: Ed25519 cryptographic signing and V2 canonical hashing.
   - `src/policy.rs`: Path normalization and sandbox access control engine.
   - `src/analytics.rs`: Multi-dimensional telemetry aggregation and threat evaluation.
   - `src/agent_interface.rs`: AI agent control interface, dynamic route overrides, and MCP JSON-RPC protocol.
3. Deployment & Operational Topologies:
   - Single Lead Node deployment.
   - Dual-PC Heterogeneous Cluster (PC-1 Lead + PC-2 CUDA Worker).
   - Live Dashboard (`/dashboard`) monitoring and real-time SSE streaming.
4. Summary & Future Frontiers:
   - Cryptographic zero-trust networks for autonomous agent civilizations.

Format as clean, comprehensive GitHub Markdown with complete structural diagrams and engineering insights.
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
            {"role": "system", "content": "You are a distinguished computer science professor, systems architect, and author of definitive engineering textbooks."},
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
    print("  AGENTTRACE TEXTBOOK DELEGATION WRITER (ZERO TOKEN ECONOMY)")
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
    print(f"[✓] TEXTBOOK DRAFTING COMPLETE: {success_count}/{len(CHAPTERS)} chapters written.")
    print(f"    Location: {OUTPUT_DIR}")
    print("=================================================================")

if __name__ == "__main__":
    main()
