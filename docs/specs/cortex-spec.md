# Cortex — Architecture Specification
**Project:** Flickersong / fail.academy  
**Version:** 0.2 — Per-Service Cloak Gating Added  
**Status:** Pre-implementation  
**Date:** 2026-03-20

---

## 1. Overview

Cortex is the central nervous system of the fail.academy self-hosted AI infrastructure. It is a Rust/Axum service that acts as a unified meta-interface over all downstream services — Episteme, Cerebro, Datastore, IDEA pipeline, and all future additions. It exposes a single HTTP API for human and agent consumers and a federated MCP server for Claude and Wheelhouse agents.

Cortex does not own business logic. It owns routing, service health, failure orchestration, and delegation to Cloak for all access control decisions.

### 1.1 Core Properties

- **Dumb proxy, not smart router.** Cortex forwards requests to downstream services. It does not fan-out, join results, or execute cross-service queries. Smart routing is a future extension, not a day-one target.
- **Fail closed on auth.** If Cloak is unreachable, Cortex returns `503 auth_service_unavailable` immediately. No queue, no retry, no degradation for auth failures.
- **Graceful degradation on data services.** Downstream service failures follow a four-stage cascade: queue → degrade → partial → hard fail.
- **Conservative defaults throughout.** Structural correctness over premature optimization.

---

## 2. System Topology

```
                    HUMAN INTERFACE
              (Cloudflare Tunnel — HTTPS only)
                          │
               ┌──────────▼──────────┐
               │    CORTEX  :9000    │
               │  Router · Health FSM│──────────────────────┐
               │  MCP Surface        │  (agent token        │
               │  YubiKey Kill Switch│   validates via Cloak)│
               └──────────┬──────────┘                      │
                          │                                  │
              ┌───────────┴────────────────────────┐        │
              │       Tailscale (internal mesh)     │        │
              └──┬──────┬──────┬──────┬────────┬───┘        │
                 │      │      │      │        │             │
            Cloak    Episteme Cerebro Datastore IDEA/n8n     │
            :8300    :8100    :8101   :8102   (webhook)      │
              ▲ │      │◄─SSE  │◄─SSE  │◄─SSE               │
              │ │      │       │       │  (halt channel)     │
              │ └──────┴───────┴───────┘                     │
              │   (startup registration;                      │
              │    signing key issued once)                   │
              └──────────────────────────────────────────────┘
                          │
            Infisical :8200
            (Cloak's backend only)
```

### Trust Model

Tailscale provides **transport security only** — network presence does not confer authorization. Every service on the mesh is independently gated by Cloak. A compromised Episteme cannot call Datastore simply by being on the tailnet.

Two distinct Cloak relationships exist:

**Layer 1 — Service registration (startup, session-based)**
Each service connects to Cloak at startup, registers its identity, and receives a signing key. Cloak maintains a persistent SSE channel to each registered service and can push a halt signal at any time. No per-request round-trips required.

**Layer 2 — Request authorization (per-request, agent tokens)**
Agent tokens presented to any service are verified **locally** using the signing key issued at registration — no network hop to Cloak at request time. Cloak remains authoritative because it issued the signing key; services remain independent because they verify without calling out.

### 2.1 Network Boundary Rules

| Traffic | Path | Auth |
|---|---|---|
| Human → Cortex | Cloudflare Tunnel → :9000 | Double-door (see §5) |
| Human → Cloak admin UI | Tailscale → :8300/admin | YubiKey + session auth |
| Agent → Cortex | Tailscale → :9000 | Disposable scoped token |
| Cortex → downstream services | Tailscale → service ports | Internal Tailscale identity |
| Cortex → Cloak | Tailscale → :8300 | Internal service token |
| Cloak → Infisical | Tailscale → :8200 | Infisical service token |
| External webhooks (Telnyx) | Cloudflare Tunnel → n8n | Webhook secret |

**Infisical is never exposed to anything except Cloak.** No other service, agent, or human has a path to Infisical's API.

---

## 3. Cortex Internal Architecture

### 3.1 Rust Workspace Structure

```
cortex/
├── Cargo.toml               # workspace root
├── cortex-core/             # shared types: ServiceManifest, HealthState,
│                            #   Token, FailureState, CortexError
├── cortex-api/              # Axum HTTP server, routing, proxy logic
├── cortex-mcp/              # MCP server, federated tool registry
└── cortex-auth/             # token validation, YubiKey kill switch,
                             #   Cloak client
```

`cortex-core` types are designed for eventual sharing with Wheelhouse via path dependency or published crate.

### 3.2 Service Manifest

Cortex reads a static TOML manifest at startup. This is the authoritative registry of known services.

```toml
# /etc/cortex/manifest.toml

[services.episteme]
name        = "Episteme"
base_url    = "http://episteme.tailnet:8100"
health_path = "/health"
timeout_ms  = 3000
queue_ttl_s = 30

[services.cerebro]
name        = "Cerebro"
base_url    = "http://cerebro.tailnet:8101"
health_path = "/health"
timeout_ms  = 3000
queue_ttl_s = 30

[services.datastore]
name        = "Datastore"
base_url    = "http://datastore.tailnet:8102"
health_path = "/health"
timeout_ms  = 5000
queue_ttl_s = 60

[services.cloak]
name        = "Cloak"
base_url    = "http://cloak.tailnet:8300"
health_path = "/health"
timeout_ms  = 2000
# No queue_ttl — Cloak failures are hard fails only
```

Health checks poll each service at a configurable interval (default: 10s). A service transitions to `Unhealthy` after two consecutive failed checks. It returns to `Healthy` after one successful check.

### 3.3 Failure State Machine

Each data service (non-Cloak) maintains an independent failure FSM:

```
         healthy request
              │
         ┌────▼─────┐
    ┌────►│ HEALTHY  │◄────────────────────────────┐
    │    └────┬─────┘                               │
    │         │ health check fails                  │ recovery confirmed
    │    ┌────▼─────────┐                           │
    │    │   QUEUING    │ queue requests, TTL timer  │
    │    │              │ retry connection           │
    │    └────┬─────────┘                           │
    │         │ TTL exceeded / queue full            │
    │    ┌────▼──────────────┐                      │
    │    │    DEGRADED       │ return partial results│
    │    │                   │ from healthy services │
    │    └────┬──────────────┘                      │
    │         │ no partial possible                  │
    │    ┌────▼──────────────┐                      │
    │    │    PARTIAL FAIL   │ structured payload:   │
    │    │                   │ service status map    │
    │    └────┬──────────────┘                      │
    │         │ unrecoverable                        │
    │    ┌────▼──────────────┐                      │
    │    │    HARD FAIL      │ 503, full status map  ├─────┘
    │    └───────────────────┘
```

**Queue implementation:** In-memory `asyncio.Queue` equivalent — `tokio::sync::mpsc` bounded channel per service. No durable queue. Queue state is lost on Cortex restart. This is intentional for v0.1.

**Cloak failure:** Bypasses FSM entirely. Any Cloak unreachability → immediate `503 auth_service_unavailable`. No queue, no degradation.

---

## 4. Datastore Service

Datastore runs as a standalone FastAPI service on `:8102`. It is registered in Cortex's manifest and exposed through Cortex's routing layer. Cortex sees a healthy service on a port — it has no awareness of Datastore's internal storage engine choices.

### 4.1 Storage Engine Strategy

| Use Case | Engine | Rationale |
|---|---|---|
| Lightweight named databases | SQLite | Zero infra, per-file isolation, portable |
| Heavy / vector / multi-user | PostgreSQL + pgvector | Full relational power, vector search |
| Blob storage | Filesystem under `/secure/` | DB stays lean, files stay portable |
| Blob metadata | SQLite or Postgres (per DB weight) | Path + hash + mime stored in DB |

PostgreSQL complexity is fully encapsulated inside Datastore. A single install script provisions Postgres, creates the Datastore role, and configures the connection. **You never touch `psql` unless you choose to.** Datastore owns its Postgres instance.

### 4.2 Schema Discipline

Every table in every Datastore database — SQLite or Postgres — follows this pattern:

```sql
CREATE TABLE <name> (
    id          TEXT PRIMARY KEY,   -- structured ID (see §4.3)
    created_at  INTEGER NOT NULL,   -- unix timestamp ms
    updated_at  INTEGER NOT NULL,
    -- ... typed core columns defined at table creation ...
    meta        JSONB               -- flexible sidecar, always present
);
```

**No pure schema-on-read.** Callers define table structure at creation time. The `meta` JSONB column absorbs overflow without sacrificing core column discipline. This prevents the silent chaos of a document store while retaining flexibility.

### 4.3 Query Interface Layers

Datastore exposes three tiers of query interface, all through its HTTP API:

**Tier 1 — ORM-style CRUD** (default, safe)  
`list`, `get`, `insert`, `update`, `delete` operations on named tables. Caller specifies table name, filter predicates, and field map. No raw SQL exposure.

**Tier 2 — Named queries** (curated, maintainer-controlled)  
Pre-defined query templates registered in a query library. Callers invoke by name with typed parameters. Zero SQL injection surface. New named queries require a manifest update.

**Tier 3 — Raw SQL passthrough** (privileged, explicit)  
Available only to tokens with `operation_class: admin` scope. Requires an explicit `X-Raw-SQL: true` header. Every raw SQL call is logged with full token identity and query text. This is the escape hatch, not the default path.

### 4.4 Blob Strategy

Blobs are never stored in the database. The flow:

```
Client → POST /datastore/{db}/blobs
       → Datastore writes file to /secure/blobs/{db}/{uuid}.{ext}
       → Datastore writes metadata row to DB:
         { id, path, sha256, mime_type, size_bytes, created_at, meta }
       → Returns metadata record to client

Client → GET /datastore/{db}/blobs/{id}
       → Datastore reads metadata, streams file from /secure/
       → Client receives raw bytes with correct Content-Type
```

The FastAPI gateway at `:8000` is not involved. Datastore owns its blob I/O path directly against `/secure/`.

### 4.5 Vector Strategy

pgvector on Postgres for any database that declares vector columns. SQLite databases do not support vector columns in v0.1 — a vector column declaration on a SQLite-backed DB returns a schema error at creation time with a clear message directing the caller to use a Postgres-backed DB. `sqlite-vec` is noted as a future extension.

---

## 5. Cloak Service

Cloak is a fully independent Rust/Axum process on `:8300`. It is the unified control plane for all access in the system: address resolution, permission enforcement, and secrets brokerage. No other service in the ecosystem talks to Infisical. No agent ever reaches Cloak directly — all agent interactions with Cloak are mediated by Cortex.

### 5.1 Responsibilities

| Responsibility | Description |
|---|---|
| **Address Registry** | Live map of all internal service endpoints and external API endpoints. Agents query *Cortex*, Cortex queries Cloak: "where is Episteme right now?" |
| **Permission Registry** | What any given token identity is allowed to reach, at what operation class, on what resources |
| **Secrets Broker** | Pulls from Infisical at startup and on TTL refresh. Serves secrets to authorized callers. Infisical is never exposed to anything else |
| **Token Validation** | Per-request validation of all agent tokens via Infisical API. Cloak validates; Cortex enforces |

### 5.2 Token Model

Tokens are issued by Infisical through a bespoke Cloak-mediated issuance interface. The issuance flow:

```
Wheelhouse Hub → POST /cloak/tokens/issue
              → Cloak validates Hub identity
              → Cloak constructs scope payload
              → Cloak calls Infisical to mint token
              → Infisical returns opaque token string
              → Cloak returns token + structured scope record to Hub
              → Hub attaches token to AgentBrief for the job
```

Agents never request their own tokens. Tokens are granted top-down by the Hub.

**Token scope payload** (what Cloak constructs and Infisical stores against the token):

```json
{
  "job_id":         "...",
  "agent_class":    "specialist",
  "issued_at":      1234567890,
  "expires_at":     1234571490,
  "services": [
    {
      "service":          "datastore",
      "operation_class":  "read",
      "resources":        ["/databases/episteme-cache", "/databases/agent-memory"]
    },
    {
      "service":          "episteme",
      "operation_class":  "read",
      "resources":        ["*"]
    }
  ]
}
```

`operation_class` values: `read` | `write` | `admin`  
`resources`: list of explicit endpoint paths or `"*"` for full service access.

**Validation per request:**

```
Agent request → Cortex
Cortex → POST /cloak/validate { token, service, operation, resource }
Cloak  → POST Infisical /tokens/validate
Infisical → { valid: true/false, scope: {...} }
Cloak  → scope check against requested (service, operation, resource)
Cloak  → { allowed: true/false, reason: "..." }
Cortex → forward request or return 403
```

Infisical is in the hot path by design. This is a deliberate security posture: there is no cached trust window. A revoked token is dead on the next request. The operational cost is one extra network hop per request — acceptable within Tailscale's latency profile.

### 5.3 Cloak Failure Behavior

Cloak maintains a local cache of Infisical state with a short TTL (configurable, default: 30s) **for secrets only** — not for token validation. If Infisical is unreachable during token validation, Cloak returns `503 infisical_unavailable` and Cortex hard-fails the request. Token validation is never served from cache.

If Cloak itself is unreachable, Cortex hard-fails with `503 auth_service_unavailable`. No queue. No degradation.

### 5.4 YubiKey Kill Switch

Two kill switch levels:

**Level 1 — Cloak halt** (`POST /cloak/admin/halt`)  
Requires YubiKey-signed FIDO2 challenge. Sets `HALTED` flag in Cloak. All token validations return `503 operator_halt`. Cortex begins hard-failing all authenticated requests immediately. Cloak's admin UI remains accessible for status and resume.

**Level 2 — Cortex halt** (`POST /cortex/admin/halt`)  
Requires YubiKey-signed challenge. Sets global `HALTED` flag in Cortex. All requests return `503 operator_halt` regardless of auth state. Cloak remains running.

Resume: `POST /cloak/admin/resume` or `POST /cortex/admin/resume`, both YubiKey-gated.

### 5.5 Cloak Admin UI

A dedicated web interface served at `http://cloak.tailnet:8300/admin`. Accessible only over Tailscale — never exposed through Cloudflare Tunnel.

**Auth:** Double-door. First: Tailscale network presence (network = identity gate). Second: session-based auth (username + password or passkey) enforced by Cloak's own auth layer. YubiKey required for all destructive operations (delete permission, revoke token, halt, modify address registry).

**Capabilities:**
- View and edit Address Registry (live endpoint map)
- View and edit Permission Registry (token scopes, agent classes)
- Issue and revoke tokens manually
- View token audit log
- Halt / resume controls
- Infisical sync status

### 5.6 Service Registration Protocol

Every service in the ecosystem embeds the `cloak-sdk` crate (see §5.7) and executes this registration sequence at startup before accepting any requests:

```
Service boots
  → cloak_sdk::register(service_id, manifest_token)
  → POST /cloak/services/register
      { service_id, service_type, version, capabilities[] }
  → Cloak validates manifest_token (pre-provisioned in Infisical)
  → Cloak returns:
      { session_id, signing_key, halt_stream_url }
  → Service stores signing_key in memory (never on disk)
  → Service opens persistent SSE connection to halt_stream_url
  → Service begins accepting requests
```

If registration fails, the service **does not start**. There is no unauthenticated fallback mode.

**Signing key usage:** The signing key is an HMAC secret or asymmetric public key (design detail TBD in open items). Services use it to verify agent token signatures locally on every inbound request — no Cloak network call at request time. Cloak can rotate the key by pushing a `key_rotation` event over the SSE channel; services swap the key atomically and continue without downtime.

**Halt signal delivery (SSE channel):**

```
Cloak → SSE event: { type: "halt", service_id: "datastore", reason: "operator" }
  → Service: stop accepting new connections immediately
  → Service: finish zero in-flight requests (existing connections close naturally)
  → Service: return 503 { "halted": true } to all new requests
  → Service: remains running, process alive, state intact
  → Restart: operator action required (systemctl start / manual)
```

The service process stays alive so logs remain accessible and state is not destroyed. Restart is an explicit operator decision, not automatic.

**Per-service vs. global halt:**

| Signal | Scope | Delivery |
|---|---|---|
| `halt` | Single named service | SSE push to that service's channel |
| `halt_all` | All registered services | SSE broadcast to all channels |
| YubiKey Level 1 | Cloak itself | Internal HALTED flag, all validations rejected |
| YubiKey Level 2 | Cortex itself | Internal HALTED flag, all routing rejected |

A per-service halt from Cloak does **not** require YubiKey. It is an operator action through the Cloak admin UI, gated by session auth. YubiKey is only required for halting Cloak or Cortex themselves.

---

### 5.7 Cloak SDK Crate

The `cloak-sdk` crate lives inside the `cortex` Rust workspace as a sub-crate of `cortex-core`:

```
cortex/
├── Cargo.toml                    # workspace root
├── cortex-core/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── manifest.rs           # ServiceManifest, HealthState
│   │   ├── token.rs              # Token types, scope structs
│   │   └── failure.rs            # FailureState FSM types
│   └── cloak-sdk/                # sub-crate
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── registration.rs   # startup registration, session mgmt
│           ├── verification.rs   # local token signature verification
│           ├── halt.rs           # SSE channel listener, halt handler
│           └── middleware.rs     # Axum middleware (drop-in for Rust services)
├── cortex-api/
├── cortex-mcp/
└── cortex-auth/
```

**Non-Rust services** (Datastore is FastAPI/Python, n8n is Node): the `cloak-sdk` Rust crate ships a companion **HTTP microprotocol spec** — a minimal OpenAPI document describing the registration, verification, and SSE endpoints. Python and Node services implement a thin client against this spec. The Rust SDK is the reference implementation; language ports must be spec-identical. Drift between implementations is a named risk — the spec document is the contract, not the Rust code.

---

## 6. MCP Surface

Cortex exposes a federated MCP server. Tools are auto-generated from the service manifest — when a new service is registered, its tools appear in the MCP surface automatically.

### 6.1 Tool Namespace Convention

```
{service_name}_{operation}

Examples:
  datastore_list_databases
  datastore_query
  datastore_insert
  episteme_search
  cerebro_add_node
  cortex_service_status
  cloak_issue_token         ← admin-only, requires admin token
```

### 6.2 MCP Auth

MCP connections present a token in the connection handshake. Cortex validates through Cloak. The MCP surface is not a privileged bypass — it is subject to identical token scope enforcement as the HTTP API.

Claude (claude.ai / Claude Code) connects with a long-lived human-operator token that has broad read/write scope across all services. This token is stored in Infisical and retrieved by the operator — never hardcoded.

---

## 7. Future Extensions (Flagged, Not Scheduled)

| Item | Notes |
|---|---|
| Smart router (fan-out, result joining) | Architecture supports it — Cortex's proxy layer can be extended without restructuring |
| sqlite-vec for SQLite vector columns | Waiting on sqlite-vec maturity |
| Durable request queue (persist across restarts) | In-memory queue is intentionally v0.1. Redis or SQLite-backed queue when needed |
| Cloak token addressing scheme | Structured canonical token IDs encoding agent class + job context. Design deferred — open item |
| Resource-level scope granularity expansion | Currently: explicit endpoint paths. Future: table-level, row-level |
| Wheelhouse ↔ Cortex shared crate | `cortex-core` types shared via path dependency once Wheelhouse build is active |
| Backblaze backup for Datastore `/secure/` blobs | Noted dependency |
| Infisical → Vault migration path | Infisical Community is the right call now. Vault noted as future option if audit logs become necessary |

---

## 8. Port Registry

| Service | Port | Notes |
|---|---|---|
| Cortex | :9000 | Human + agent API, MCP surface |
| Episteme | :8100 | |
| Cerebro | :8101 | |
| Datastore | :8102 | |
| Cloak | :8300 | Admin UI at :8300/admin (Tailscale only) |
| Infisical | :8200 | Cloak's backend only |
| Existing FastAPI gateway | :8000 | To be absorbed or replaced by Cortex |
| Existing peer vault/registry | :8001 | To be absorbed by Cloak |

---

## 9. Open Design Items

| # | Item | Notes |
|---|---|---|
| 1 | Cloak token addressing scheme | Structured canonical IDs encoding agent class, job context, and scope. Required before Wheelhouse integration. |
| 2 | Bootstrap manifest token strategy | Pre-provisioned in Infisical, loaded by each service at startup. Rotation strategy needed. |
| 3 | Datastore install script | Single script provisions Postgres, creates role, installs pgvector, starts service. Zero `psql` interaction. |
| 4 | MCP tool schema generation | Each service exposes `/openapi.json`; Cortex reads at startup and generates tool wrappers. Generation strategy needs spec. |
| 5 | `:8000` FastAPI gateway fate | Absorb into Cortex or run parallel during migration? Deferred until Cortex is stable. |
| 6 | Signing key algorithm | HMAC vs. asymmetric (Ed25519 preferred — services hold public key only, Cloak holds private). Rotation via SSE `key_rotation` event. |
| 7 | Non-Rust SDK language ports | Python (Datastore/FastAPI) and Node (n8n) need thin Cloak clients against the microprotocol spec. Drift between implementations is a named risk. |
| 8 | SSE channel resilience | Reconnect backoff strategy on transient drop. Self-halt threshold if reconnect fails beyond N attempts — fail closed, not open. Threshold values TBD. |
