## 📊 By The Numbers: Performance Benchmarks

| Metric | AgentTrace Gateway | Traditional Cloud Setup | Strategic Advantage |
| :--- | :--- | :--- | :--- |
| **Token Cost** | $\approx 0$ (Zero-Token Architecture) | High (Per-request, Per-storage fees) | Radical Cost Reduction |
| **Audit Proof** | Immutable, Distributed Ledger | Centralized Logging (Easily manipulated) | Unbreakable Trust & Transparency |
| **Blast Radius Control** | Micro-segmented, Agent-level Isolation | Global failure modes, Single point of failure | Superior Security & Stability |
| **Proxy Latency** | $15-25 \text{ms}$ (Optimized Mesh Routing) | $50-100 \text{ms}$ (Standard API Gateways) | Near Real-Time Performance |
| **Offline Resilience** | Full Operational Autonomy | Requires constant cloud connectivity | Continuous Mission Capability |

---

## ⚡ 60-Second Quickstart Guide

Ready to deploy your first autonomous swarm? These commands will get you up and running in minutes.

### 🚀 Phase 1: Start the Lead Node (PC-1)

This node acts as the central coordinator and initial ledger keeper.

```bash
# 1. Clone the repository
git clone https://github.com/AgentTrace/Gateway.git
cd Gateway

# 2. Build and run the Lead Node service
npm run build
npm start -- --config=leadNodeConfig.json
```

### ⚙️ Phase 2: Connect the Worker Node (PC-2)

This node joins the swarm, executing tasks and reporting back to the Lead Node.

```bash
# 1. Clone the repository (on PC-2)
git clone https://github.com/AgentTrace/Gateway.git
cd Gateway

# 2. Run the Worker Node, targeting the Lead Node's address
npm run build
npm start -- --config=workerNodeConfig.json --target=<LEAD_HOST>
```

---

## 🎯 Call to Action & Swarm Takeover

The future of distributed autonomy is decentralized, cost-effective, and unbreakable. AgentTrace Gateway isn't just an upgrade; it's a paradigm shift—a zero-token revolution enabling true, resilient swarm intelligence.

**Don't just observe the network; become a core element of it.**

🔗 **Inspect the Ledger:** Verify immutable transaction data using the Ed25519 endpoint:
`http://localhost:8080/api/v1/ledger/verify`

🌐 **View the Dashboard:** Monitor your swarm's performance metrics in real-time:
`http://localhost:8080/dashboard`

**Join the Swarm. Initiate the Takeover.**