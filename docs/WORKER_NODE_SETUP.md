# Worker Node Setup Guide

This guide covers adding a **worker node** (for example PC-2) to an AgentTrace cluster.
A worker node runs its own LLM engine (Hermes Agent or Ollama). It reports to the **lead node**, which
runs the AgentTrace Gateway, the hash-chained ledger and the dashboard.

> **Status:** v0.1 prototype. Steps marked ⚠️ describe current limitations, not the intended final
> behaviour. See `docs/ROADMAP.md` (proposed) for the fixes.

---

## 1. How traffic flows

```
 Worker agent (PC-2)                          Lead node (PC-1, :8080)
 ┌──────────────────┐   POST /v1/chat/...     ┌────────────────────────────┐
 │ Hermes / script  │ ──────────────────────▶ │ AgentTrace Gateway         │
 │ X-Node-Id: PC-2  │   X-Upstream-Url: ──┐   │  1. logs request + hash    │
 └──────────────────┘                     │   │  2. forwards to upstream   │
                                          │   └─────────────┬──────────────┘
 ┌──────────────────┐ ◀───────────────────┘                 │
 │ LLM on PC-2      │ ◀─────────────────────────────────────┘
 │ :8642 / :11434   │   (response streams back through the gateway)
 └──────────────────┘
```

**Rule:** a request is recorded in the ledger **only if it passes through the lead gateway**.
If a worker agent calls its own `localhost:8642` directly, the dashboard never sees it.

---

## 2. Prerequisites

| Requirement | Windows | Linux |
| --- | --- | --- |
| Same LAN / subnet as the lead | ✅ | ✅ |
| LLM engine | Hermes Agent `:8642` or Ollama `:11434` | same |
| Python 3.8+ (installer only) | `winget install Python.Python.3.12` | `apt install python3` / `dnf install python3` |
| Inbound firewall rule for the LLM port | see §4 | see §4 |

Check that the worker can reach the lead:

```bash
curl http://<LEAD_IP>:8080/api/v1/summary
```

---

## 3. Install

### Option A: one-liner (recommended)

The lead gateway serves the installer and fills in its own IP:

```bash
# Linux
curl -sSL http://<LEAD_IP>:8080/install -o install.py && python3 install.py --worker --lead-url http://<LEAD_IP>:8080
```

```powershell
# Windows PowerShell
irm http://<LEAD_IP>:8080/install -OutFile install.py; python install.py --worker --lead-url http://<LEAD_IP>:8080
```

The installer:
1. Detects hostname, LAN IP and GPU (`nvidia-smi`).
2. Looks for a local LLM on `:8642` (Hermes), then `:11434` (Ollama).
3. Registers the node with `POST /api/v1/heartbeat` as `Worker-<hostname>`.
4. Starts a small edge proxy on `:8080` that forwards to the local LLM.

> ⚠️ The heartbeat is sent **once**, so the node shows its registration time as `last_seen`. Until periodic
> heartbeats ship, add the scheduled task from §6.
>
> ⚠️ The edge proxy does **not** send telemetry yet. Route agent traffic through the lead (§5).

### Option B: manual registration (no Python)

```bash
curl -X POST http://<LEAD_IP>:8080/api/v1/heartbeat \
  -H "Content-Type: application/json" \
  -d '{"node_id":"PC-2","hostname":"NODE-02","ip_address":"192.168.1.102",
       "role":"Worker Node (hermes-agent :8642 + CUDA)","status":"ACTIVE",
       "last_seen":"2026-10-07T12:00:00Z","public_key":null}'
```

---

## 4. Expose the worker LLM to the lead

The lead must be able to reach the worker's LLM port. Allow traffic **only from the lead IP**:

```powershell
# Windows (run as Administrator)
New-NetFirewallRule -DisplayName "AgentTrace LLM from Lead" -Direction Inbound `
  -Protocol TCP -LocalPort 8642 -RemoteAddress <LEAD_IP> -Action Allow
```

```bash
# Linux (ufw)
sudo ufw allow from <LEAD_IP> to any port 8642 proto tcp
```

For Ollama, use port `11434` and start it with `OLLAMA_HOST=0.0.0.0` so it doesn't bind only to localhost.

Check from the **lead**:

```bash
curl http://<WORKER_IP>:8642/health
curl -H "Authorization: Bearer <WORKER_KEY>" http://<WORKER_IP>:8642/v1/models
```

---

## 5. Point worker agents at the gateway

Any OpenAI-compatible client works. Set these three things:

| Setting | Value |
| --- | --- |
| Base URL | `http://<LEAD_IP>:8080/v1` |
| `X-Node-Id` header | the worker's node id, e.g. `PC-2` |
| `X-Upstream-Url` header | the worker's own LLM, e.g. `http://192.168.1.102:8642` |
| `Authorization` | the worker LLM's key (passed through unchanged) |

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://192.168.1.101:8080/v1",
    api_key="<WORKER_KEY>",
    default_headers={
        "X-Node-Id": "PC-2",
        "X-Upstream-Url": "http://192.168.1.102:8642",
    },
)
print(client.chat.completions.create(
    model="hermes-agent",
    messages=[{"role": "user", "content": "ping"}],
).choices[0].message.content)
```

```bash
curl http://<LEAD_IP>:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <WORKER_KEY>" \
  -H "X-Node-Id: PC-2" \
  -H "X-Upstream-Url: http://192.168.1.102:8642" \
  -d '{"model":"hermes-agent","messages":[{"role":"user","content":"ping"}]}'
```

For Hermes itself, set its OpenAI-compatible provider base URL to the lead gateway and add the two headers
in its provider config.

---

## 6. Keep the node "alive" (temporary workaround)

Until the agent binary sends heartbeats on its own, schedule one every minute:

```powershell
# Windows: Task Scheduler, every 1 minute
$body = '{"node_id":"PC-2","hostname":"NODE-02","ip_address":"192.168.1.102","role":"Worker Node","status":"ACTIVE","last_seen":"' + (Get-Date).ToUniversalTime().ToString("o") + '","public_key":null}'
Invoke-RestMethod -Method Post -Uri http://<LEAD_IP>:8080/api/v1/heartbeat -ContentType application/json -Body $body
```

```bash
# Linux: crontab -e
* * * * * curl -s -X POST http://<LEAD_IP>:8080/api/v1/heartbeat -H 'Content-Type: application/json' -d "{\"node_id\":\"PC-2\",\"hostname\":\"$(hostname)\",\"ip_address\":\"$(hostname -I | cut -d' ' -f1)\",\"role\":\"Worker Node\",\"status\":\"ACTIVE\",\"last_seen\":\"$(date -u +%FT%TZ)\",\"public_key\":null}"
```

---

## 7. Verify

| Check | Expected |
| --- | --- |
| `GET http://<LEAD_IP>:8080/api/v1/summary` | worker listed in `active_nodes` |
| Send one chat request (§5) | new `LlmInference` row with node `PC-2` on the dashboard |
| `GET http://<LEAD_IP>:8080/api/v1/ledger/verify` | `"is_valid": true` |
| Dashboard filter "PC-2 Hermes (Worker)" | only worker events shown |

---

## 8. Troubleshooting

| Symptom | Cause / fix |
| --- | --- |
| `502 Bad Gateway` from lead | Lead can't reach `X-Upstream-Url`. Check the firewall (§4) and that the LLM binds to `0.0.0.0`. |
| `504 Gateway Timeout` | Model is loading or the GPU is busy. The gateway allows 120 s. Retry. |
| `401` from worker | Wrong `Authorization` header. The gateway forwards it unchanged. |
| Worker events show as `PC-1` | `X-Node-Id` header missing. The gateway defaults to `PC-1`. |
| Worker requests not on the dashboard | Agent is calling `localhost` directly instead of the lead gateway. |
| `UnicodeEncodeError` running installer on Windows | Run `$env:PYTHONIOENCODING='utf-8'` first (fix planned). |

---

## 9. Security notes (read before deploying outside a home lab)

- The gateway has **no authentication** in v0.1. Anyone on the LAN can post events, register nodes and use
  `X-Upstream-Url` to make the gateway call any URL. Keep it on a trusted network or behind a firewall.
- Don't commit or share bearer keys (for example the example key in `AGENTS.md`). Rotate the PC-2 key
  if it has left your machine.
- Prompts and completions are stored **in plaintext** in `cluster_telemetry.db`.
