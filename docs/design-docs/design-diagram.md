# fail.academy — Architecture Design Diagram

> Generated: 2026-03-21 | Source of truth: `design-docs/cortex/cortex-spec.md`

---

## 1. Master Topology

```
                            EXTERNAL ACCESS
                       ┌──────────────────────┐
                       │   Cloudflare Tunnel   │
                       │   (HTTPS termination) │
                       └──────────┬───────────┘
                                  │
                                  │  NOT IMPLEMENTED
                                  │
         ┌────────────────────────▼─────────────────────────┐
         │              CORTEX  :9000                       │
         │              Rust / Axum                         │
         │                                                  │
         │  Unified proxy · Health FSM · MCP surface        │
         │  YubiKey kill switch · Service manifest (TOML)   │
         │                                                  │
         │  STATUS: ██░░░░░░░░  5%  STUBS ONLY              │
         │  (cortex-api/src/lib.rs = println stub)          │
         └───┬──────────┬──────────┬──────────┬─────────────┘
             │          │          │          │
             │     Tailscale internal mesh (100.64.0.0/10)
             │          │          │          │
   ┌─────────▼───┐ ┌───▼────────┐ ┌▼─────────┐ ┌▼──────────┐
   │ CLOAK       │ │ EPISTEME   │ │ CEREBRO   │ │ DATASTORE │
   │ :8300       │ │ :8100      │ │ :8101*    │ │ :8102     │
   │ Rust/Axum   │ │ Python/    │ │ TS/Node/  │ │           │
   │             │ │ FastAPI    │ │ Fastify   │ │           │
   │ Auth engine │ │ Knowledge  │ │ Knowledge │ │ Blob +    │
   │ Token mint  │ │ framework  │ │ graph     │ │ SQL store │
   │ Secrets     │ │ + service  │ │           │ │           │
   │ Registry    │ │            │ │           │ │           │
   │             │ │            │ │ *runs on  │ │           │
   │ ████████░░  │ │ ████████░░ │ │  :3000    │ │           │
   │ 85%  PARTIAL│ │ 80% PARTIAL│ │  today    │ │  MISSING  │
   └──────┬──────┘ └────────────┘ │           │ │           │
          │                       │ █████████░│ │           │
          │                       │ 90% PARTIAL└───────────┘
   ┌──────▼──────┐                └───────────┘
   │ INFISICAL   │
   │ :8200       │      ┌──────────────────┐     ┌────────────────┐
   │ (self-host) │      │ GATEWAY  :8800   │     │ PANOPTICON     │
   │             │      │ Rust/Axum        │     │ MCP Server     │
   │ Cloak is    │      │                  │     │ TypeScript     │
   │ sole client │      │ LLM provider     │     │                │
   │             │      │ routing, cost,   │     │ 10 tools       │
   │ ░░░░░░░░░░  │      │ sanitization     │     │ Auth + index   │
   │ EXTERNAL    │      │                  │     │                │
   └─────────────┘      │ █████░░░░░       │     │ █████████░     │
                        │ 50%  PARTIAL     │     │ 90%  LIVE      │
                        └──────────────────┘     └────────────────┘

   ┌────────────────────┐     ┌──────────────────┐
   │ WHEELHOUSE         │     │ IDEA Pipeline     │
   │ (agent-depot)      │     │ (n8n webhooks)    │
   │                    │     │                   │
   │ Resource broker    │     │                   │
   │ Agent coordinator  │     │                   │
   │ Manifest registry  │     │                   │
   │                    │     │                   │
   │ █░░░░░░░░░         │     │ ░░░░░░░░░░        │
   │ 5%  DESIGN ONLY    │     │ MISSING           │
   └────────────────────┘     └───────────────────┘
```

**Legend:** `█` = implemented, `░` = remaining

---

## 2. Authentication Flow

### Layer 1 — Service Registration (at startup)

Only Episteme implements this today. Cerebro, Cortex, Gateway, and Datastore do not.

```
  SERVICE                          CLOAK :8300                    INFISICAL :8200
     │                                │                               │
     │  POST /cloak/services/register │                               │
     │  { service_id,                 │                               │
     │    manifest_token }            │                               │
     │───────────────────────────────►│  validate manifest_token      │
     │                                │──────────────────────────────►│
     │                                │◄──────────────────────────────│
     │  Response:                     │                               │
     │  { session_id,                 │                               │
     │    signing_key (base64),       │                               │
     │    halt_stream_url }           │                               │
     │◄───────────────────────────────│                               │
     │                                │                               │
     │  GET {halt_stream_url}         │                               │
     │  (SSE, persistent connection)  │                               │
     │───────────────────────────────►│                               │
     │  ◄── heartbeat (15s) ─────────│                               │
     │  ◄── halt event ──────────────│                               │
     │  ◄── key_rotation ────────────│                               │
```

**Reference implementation:** `episteme/interface/service/cloak/client.py`

### Layer 2 — Per-Request Token Verification (local, no network hop)

```
  AGENT                     SERVICE                    (no Cloak call)
    │                          │
    │  Request + Bearer token  │
    │  (HMAC-SHA256 signed)    │
    │─────────────────────────►│
    │                          │  1. Split token: base64url(claims).hex(signature)
    │                          │  2. Verify HMAC-SHA256 with signing_key from registration
    │                          │  3. Check expiration (expires_at)
    │                          │  4. Check scope (services[], operation_class, resources[])
    │                          │
    │  200 OK / 401 / 403      │
    │◄─────────────────────────│
```

**Token format:** `base64url(json_claims) + "." + hex(hmac_sha256(claims, signing_key))`
- Rust impl: `cloak/cloak-tokens/src/signing.rs`
- Python impl: `episteme/interface/service/cloak/auth.py`

### Who Implements Cloak Auth Today

| Service | Registration | SSE Halt | Token Verify | File |
|---------|:---:|:---:|:---:|------|
| Episteme | YES | YES | YES | `episteme/interface/service/cloak/client.py` |
| Cerebro | NO | NO | NO | `cerebro/src/api/auth.ts` — env-var bearer token |
| Cortex | NO | NO | NO | `cortex/cortex-auth/src/lib.rs` — empty stub |
| Gateway | NO | NO | NO | `gateway/src/identity/tokens.rs` — own HMAC system |
| Datastore | — | — | — | Does not exist |

---

## 3. Data Flow

### Spec'd Architecture (target state)

```
  HUMAN / AGENT
       │
       ▼
  Cloudflare Tunnel ──► CORTEX :9000
                            │
                ┌───────────┼───────────────────┐
                │           │                   │
                ▼           ▼                   ▼
           CLOAK :8300  EPISTEME :8100    CEREBRO :8101
           (auth gate)  (knowledge lib)  (knowledge graph)
                │                               │
                ▼                               │
           INFISICAL :8200              DATASTORE :8102
           (secrets)                    (blob + SQL)
                                                │
                                                ▼
                                        GATEWAY :8800
                                        (LLM providers)
                                                │
                            ┌───────────────────┼───────────┐
                            ▼                   ▼           ▼
                        Anthropic           OpenAI      Mistral/Groq

  WHEELHOUSE (agent-depot)
       │
       ▼
  CORTEX MCP Surface ──► dispatches to all downstream services
```

### What Actually Works Today

```
  ┌──────────────┐    filesystem     ┌──────────────┐
  │ PANOPTICON   │◄─────────────────│ EPISTEME     │
  │ MCP Server   │  reads framework │ FRAMEWORK    │
  │ (10 tools)   │  files directly  │ (55+ skills) │
  └──────────────┘                  └──────────────┘

  ┌──────────────┐    HTTP (Cloak    ┌──────────────┐
  │ EPISTEME     │    registration   │ CLOAK        │
  │ SERVICE      │───────────────►  │ :8300        │
  │ :8100        │    SSE halt      │              │
  │ (FastAPI)    │◄───────────────  │              │
  └──────────────┘                  └──────┬───────┘
                                          │ HTTP
  ┌──────────────┐                  ┌─────▼────────┐
  │ CEREBRO      │  STANDALONE      │ INFISICAL    │
  │ :3000        │  (no integration │ :8200        │
  │ (Fastify)    │   with anything) │              │
  └──────────────┘                  └──────────────┘

  Everything else: NOT CONNECTED
```

---

## 4. Integration Status Matrix

| From → To | Protocol | Status | Blocker |
|-----------|----------|--------|---------|
| Episteme → Cloak | HTTP + SSE | **IMPLEMENTED** | Needs running Cloak instance |
| Cloak → Infisical | HTTP | **IMPLEMENTED** | Needs running Infisical instance |
| Panopticon → Episteme framework | Filesystem | **WORKING** | None (same host) |
| Cerebro → Cloak | — | **NOT IMPLEMENTED** | Need TS Cloak client, rewrite auth.ts |
| Cortex → Cloak | — | **NOT IMPLEMENTED** | cortex-cloak is 28-line stub |
| Cortex → Episteme | HTTP proxy | **NOT IMPLEMENTED** | cortex-api is println stub |
| Cortex → Cerebro | HTTP proxy | **NOT IMPLEMENTED** | cortex-api is println stub |
| Cortex → Datastore | HTTP proxy | **NOT IMPLEMENTED** | Both sides missing |
| Gateway → Cloud LLMs | HTTPS | **PARTIAL** | Provider dispatchers incomplete |
| Gateway → Cloak | — | **NOT IMPLEMENTED** | Gateway has own token system |
| Wheelhouse → Cortex MCP | — | **NOT IMPLEMENTED** | Neither side exists |
| Human → Cortex | CF Tunnel | **NOT IMPLEMENTED** | Cortex not running |

---

## 5. Wheelhouse Position

The Wheelhouse sits above Cortex as the agent coordination layer. Agents don't access services directly — they request resources through the Wheelhouse's manifest-driven registry.

```
                    ┌─────────────────────────────────┐
                    │         WHEELHOUSE               │
                    │    (Resource Broker / Agent Hub)  │
                    │                                  │
                    │  ┌───────────┐  ┌─────────────┐  │
                    │  │ Resource  │  │ Agent       │  │
                    │  │ Manifest  │  │ Coordinator │  │
                    │  │ (YAML)    │  │ (cadre/     │  │
                    │  │           │  │  captain)   │  │
                    │  └─────┬─────┘  └──────┬──────┘  │
                    │        │               │         │
                    └────────┼───────────────┼─────────┘
                             │               │
                    ┌────────▼───────────────▼─────────┐
                    │       CORTEX :9000                │
                    │       MCP Surface                 │
                    │                                   │
                    │  Tools auto-generated from         │
                    │  service manifest:                 │
                    │   • episteme_search                │
                    │   • cerebro_query                  │
                    │   • datastore_list_databases       │
                    │   • gateway_complete               │
                    │   • cortex_service_status          │
                    └───────────────────────────────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
        ┌─────▼─────┐ ┌─────▼─────┐ ┌──────▼────┐
        │ Episteme  │ │ Cerebro   │ │ Datastore │ ...
        │ :8100     │ │ :8101     │ │ :8102     │
        └───────────┘ └───────────┘ └───────────┘
```

**Wheelhouse design decisions (locked):**
- Resource manifest: YAML files in `resources/` directory with `resources-map.yaml` index
- Agent branch strategy: one branch per session (`agent/session-<id>`)
- Deployment: plugin/channel model with captain agents + central queue
- Resource identity: stable `id` field + optional URN
- Constraint language: stubbed for v1
- First resource type: GitHub repository plugin

Source: `agent-depot/wheelhouse/planning.md`
