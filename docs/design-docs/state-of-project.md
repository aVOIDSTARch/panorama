# fail.academy — State of Project

> Generated: 2026-03-21 | Covers all projects in `/a1/`

---

## 1. Per-Project Status

| Project | % | Language | What Works | What's Missing |
|---------|---|----------|------------|----------------|
| **cerebro** | 90% | TypeScript / Node.js / Fastify | REST API (7 routes: entities, assertions, sources, citations, search, quarantine, admin), Kùzu graph DB, Meilisearch full-text, Chroma vectors, SQLite quarantine, 30 tests, Docker Compose, bootstrap + export scripts | Cloak integration (uses env-var bearer token — `cerebro/src/api/auth.ts`), port mismatch (runs `:3000`, spec says `:8101`), no Cortex registration, no SSE halt listener, no service manifest entry |
| **cloak** | 85% | Rust / Axum | 6-crate workspace compiles, Infisical client, secret cache + background refresh, HMAC-SHA256 signing (format-compatible with Episteme Python), service registration + SSE broadcast, permission engine, health endpoint, halt/resume admin, Tailscale guard middleware | YubiKey FIDO2 admin auth, admin session management, integration tests against real Infisical, admin permission CRUD route handlers |
| **panopticon** | 90% | TypeScript / Node.js | 10 MCP tools (auth, style guides, skills, search, schemas, templates, queue-change, meta-agent, onboard), in-memory trigram search, usage tracking (JSONL), Docker support (stdio + SSE), all 30+ improvement suggestions resolved | Semantic/vector search (low priority), code diffing tool (low priority), ~50 code guides need master format processing |
| **episteme** | 80% | Python (FastAPI) + TypeScript (framework) | FastAPI service with Cloak registration/SSE/token verification, 5 routers, file indexing, metadata DB (SQLModel/SQLite), 55+ skills, 5 processed code guides, OpenAPI schema | React frontend (scaffolded only — no components), ~50 remaining code guides, semantic search, frontend routing |
| **gateway** | 50% | Rust / Axum | Full module structure (17 modules), config parsing, DB schema (3 migrations), provider dispatchers (Anthropic/OpenAI/custom), route store, CLI command definitions, sanitizer structure, `server.rs` (488 lines with AppState + routing) | HTTP server wiring incomplete, no Cloak integration (own token system), alert transport (Telnyx SMS), health probing logic, kill switch state machine, integration tests |
| **cortex** | 5% | Rust / Axum | Type definitions only — `ServiceManifest`, `HealthState`, `FailureState`, `Token`, `ServiceScope`, `OperationClass`, `CortexError` (72 lines in `cortex-core/src/lib.rs`), 5-crate workspace compiles | Everything: HTTP server (`cortex-api/src/lib.rs` = `println!` stub), proxy logic, manifest loading, health FSM, MCP server, Cloak client, routing, request queuing |
| **agent-depot** | 5% | Design only | Planning doc with 6 resolved design questions (`wheelhouse/planning.md`), project seed concept doc, meta-agent conventions (`ai-docs/meta-agent.md`), empty stub dirs for all projects | All code: resource manifest schema, wheelhouse core, GitHub plugin, agent interface, constraint enforcement. No language chosen (Python current, TypeScript preferred) |
| **datastore** | 0% | — | Does not exist as a project | Everything. Spec'd at `:8102` in `design-docs/cortex/cortex-spec.md` with SQLite + PostgreSQL + pgvector + blob storage |
| **IDEA pipeline** | 0% | — | Does not exist | Everything. Referenced in Cortex topology as n8n webhook target |

---

## 2. Critical Path

Nothing reaches users until Cortex is running. Nothing runs through Cortex without Cloak.

```
PHASE 1                PHASE 2                PHASE 3              PHASE 4
────────               ────────               ────────             ────────
Cloak ─────────► Cortex proxy ──────► Cerebro Cloak ──────► Wheelhouse
(verify against        (minimum viable         integration          (resource
 real Infisical)        proxy to Episteme)      + Datastore          broker)

 BLOCKER:              BLOCKER:               BLOCKER:             BLOCKER:
 Need Infisical        cortex-api is a        Cerebro auth.ts      No code exists
 running on            println stub           is env-var only      at all
 Tailnet                                      Datastore missing
```

**Dependency chain:**

```
Infisical ──► Cloak ──► Cortex ──► { Episteme, Cerebro, Datastore }
                                              │
                                              ▼
                                         Wheelhouse
                                              │
                                              ▼
                                     Gateway (LLM access)
```

**First integrated request path (minimum to prove the architecture):**
1. Cloak running on Tailnet, accepting service registrations
2. Episteme registers with Cloak (already implemented)
3. Cortex boots, registers with Cloak, loads manifest
4. Cortex proxies `GET /episteme/api/v1/framework/tree` to Episteme
5. Agent token validated at Cortex via Cloak

---

## 3. Misalignment Inventory

### 3.1 Cerebro port mismatch

- **Actual:** `PORT ?? 3000` in `cerebro/src/api/server.ts:35`
- **Spec:** `base_url = "http://cerebro.tailnet:8101"` in cortex-spec.md
- **Fix:** Change default port to `8101`

### 3.2 Cerebro auth model mismatch

- **Actual:** `cerebro/src/api/auth.ts` — 27 lines, compares `Bearer` header against `CEREBRO_API_TOKEN` env var. No Cloak registration, no HMAC verification, no SSE halt.
- **Spec:** All downstream services register with Cloak at startup, receive signing key, verify tokens locally with HMAC-SHA256.
- **Reference impl:** `episteme/interface/service/cloak/client.py` — full registration + SSE + token verification in Python. Needs porting to TypeScript for Cerebro.

### 3.3 Gateway design doc naming collision

- **Issue:** `design-docs/gateway/gateway.md` line 1 is titled "Cloak — Cloud Model Access Gateway", but Cloak is the auth/access control service at `:8300`. The gateway project is a separate LLM routing service.
- **Impact:** Confusing when cross-referencing docs.

### 3.4 Gateway's independent token system

- **Actual:** `gateway/src/identity/tokens.rs` (198 lines) implements its own HMAC-SHA256 token store with `TokenRecord`, `issue_token()`, `validate_token()`, `revoke_token()` — completely independent of Cloak.
- **Spec:** All services authenticate through Cloak. Gateway should receive tokens from Cortex/Cloak, not mint its own.
- **Decision needed:** If Gateway runs behind Cortex, it shouldn't need its own token system. If it runs standalone, it needs Cloak integration.

### 3.5 cortex-cloak crate identity crisis

- **Actual:** `cortex/cortex-cloak/src/lib.rs` (28 lines) is a standalone Axum server skeleton on `:8300` with only a `/health` endpoint. It was the original Cloak prototype before Cloak became its own project.
- **Spec:** cortex-cloak should be a thin HTTP client library that calls the standalone Cloak service.
- **See also:** `cloak/cloak.md` documents the "cortex-cloak Transition" — never completed.

### 3.6 Episteme framework duplication

- **Actual:** `episteme/episteme-framework/` and `panopticon/episteme-framework/` contain the same content. Neither is declared canonical.
- **Risk:** Content drift as guides/skills are added to one copy but not the other.
- **Fix:** Declare one canonical location (likely `episteme/`), symlink or git submodule the other.

### 3.7 Cerebro design docs scattered across 3 locations

| Location | Files |
|----------|-------|
| `agent-depot/wheelhouse/designs/` | cerebro-kg-design.md, cerebro-backend-implementation.md, extraction-design-schema.md, citation-inclusion-design-schema.md |
| `design-docs/cerebro/` | Same 4 filenames |
| `cerebro/ai-docs/cerebro/` | Same 4 + suggestions-2.md, final-plan.md |

- **Risk:** Edits to one copy don't propagate. Unclear which is authoritative.

### 3.8 Cargo dependency version conflicts

| Dependency | Cortex | Cloak | Impact |
|-----------|--------|-------|--------|
| `thiserror` | 1.0 | 2 | Incompatible Error trait impls |
| `tower` | 0.4 | 0.5 | Middleware layer incompatibility |
| `tower-http` | 0.5 | 0.6 | TraceLayer API differences |

If Cortex imports Cloak types (via a shared crate or cortex-cloak client), these version mismatches will cause compilation errors. Must align before integration.

### 3.9 FastAPI gateway at :8000 — phantom reference

- **Spec:** cortex-spec.md references "Existing FastAPI gateway :8000 — To be absorbed or replaced by Cortex"
- **Actual:** No Python FastAPI gateway project exists. The `gateway/` project is Rust on `:8800`/`:8801`.
- **Likely:** This referred to an earlier prototype that was replaced by the Rust gateway. The spec reference is stale.

### 3.10 cloak-sdk crate location mismatch

- **Spec:** cortex-spec.md section 5.7 says `cloak-sdk` should live inside `cortex/cortex-core/cloak-sdk/` as a sub-crate.
- **Actual:** No such directory exists. The Cloak client code is in the standalone `cloak/` repo. The Python client is in `episteme/interface/service/cloak/`.
- **Impact:** No reusable SDK crate for Rust services (Cortex, Gateway) to consume.

---

## 4. Gap Inventory

| Gap | Severity | Description | Where It's Spec'd |
|-----|----------|-------------|-------------------|
| **Cortex implementation** | CRITICAL | The entire orchestration layer is type stubs. No HTTP server, no proxy, no health FSM, no MCP surface. | `cortex/cortex-api/src/lib.rs` (4 lines) |
| **Datastore service** | HIGH | Referenced at `:8102` with SQLite + PostgreSQL + pgvector + blob storage. No project directory exists. | cortex-spec.md section 4 |
| **Cerebro Cloak client** | HIGH | Must be written from scratch in TypeScript. No reusable TS Cloak SDK exists. | cortex-spec.md section 5.6 |
| **Integration tests** | HIGH | Zero integration tests between any pair of services. Cloak/Episteme registration has never been tested end-to-end. | — |
| **Wheelhouse code** | HIGH | Zero implementation. Design decisions locked but no manifest schema file, no plugins, no agent interface. | `agent-depot/wheelhouse/planning.md` |
| **cloak-sdk (Rust)** | MEDIUM | No reusable Rust crate for Cloak client. Cortex and Gateway both need one. | cortex-spec.md section 5.7 |
| **Episteme React frontend** | MEDIUM | Vite scaffolded at `episteme/interface/frontend/`, no real components built. | `episteme/interface/interface.md` |
| **IDEA pipeline** | MEDIUM | Referenced in Cortex topology diagram. No project exists. n8n webhook target. | cortex-spec.md topology |
| **Gateway Cloak integration** | MEDIUM | Gateway has own token system. Needs decision: absorb into Cortex or add Cloak client. | — |
| **CI/CD pipeline** | MEDIUM | No CI/CD exists for any project. Manual testing only. | — |
| **Service health endpoints** | LOW | Cortex expects `GET /health` on all downstream services. Cerebro has one; Episteme has one; others unclear. | cortex-spec.md |

---

## 5. Integration Readiness Matrix

Can these two services actually communicate today?

| | Cloak | Episteme | Cerebro | Cortex | Gateway | Panopticon | Wheelhouse | Datastore |
|---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| **Cloak** | — | YES | NO | NO | NO | — | NO | — |
| **Episteme** | YES | — | — | NO | — | — | — | — |
| **Cerebro** | NO | — | — | NO | — | — | — | — |
| **Cortex** | NO | NO | NO | — | NO | — | NO | NO |
| **Gateway** | NO | — | — | NO | — | — | — | — |
| **Panopticon** | — | FS | — | — | — | — | — | — |
| **Wheelhouse** | NO | — | — | NO | — | — | — | — |
| **Datastore** | — | — | — | — | — | — | — | — |

**YES** = code exists and can connect | **NO** = code missing or incompatible | **FS** = filesystem (same host) | **—** = no direct relationship

**Working connections today: 2 out of 15+ required**

---

## 6. Prioritized Roadmap

### Phase 1 — Foundation: Cloak Verification
> Unblocks: everything

- Deploy Infisical on Tailnet
- Integration-test Cloak against real Infisical (registration, token mint, secret fetch)
- Verify Episteme Python client registers + verifies tokens against running Cloak
- Fix: admin permission CRUD route handlers in Cloak
- Deliverable: Cloak running, Episteme authenticated through it

### Phase 2 — Cortex Minimum Viable Proxy
> Unblocks: all downstream access, MCP surface, human access

- Align Cargo versions with Cloak (thiserror 2, tower 0.5, tower-http 0.6)
- Implement `cortex-api` HTTP server with Axum (replace println stub)
- Implement manifest loading from TOML
- Build `cortex-cloak` as a Rust HTTP client (register, validate, SSE halt)
- Implement basic proxy: Cortex → Episteme (first downstream)
- Health check polling (10s interval, 2-fail threshold)
- Deliverable: `curl cortex:9000/episteme/api/v1/framework/tree` returns data

### Phase 3 — Cerebro Integration
> Unblocks: knowledge graph access through Cortex

- Write TypeScript Cloak client for Cerebro (port the Python registration + SSE + verify pattern)
- Replace `cerebro/src/api/auth.ts` env-var auth with Cloak token verification
- Change default port from 3000 to 8101 in `cerebro/src/api/server.ts:35`
- Register with Cloak at startup, listen SSE halt channel
- Add Cerebro to Cortex service manifest
- Deliverable: Cerebro accessible through Cortex proxy with Cloak auth

### Phase 4 — Datastore Creation
> Unblocks: persistent storage for agents

- Create project directory at `a1/datastore/`
- Implement FastAPI service at `:8102` per cortex-spec section 4
- Three-tier access: ORM CRUD (safe), named queries (curated), raw SQL (privileged)
- SQLite for lightweight DBs, PostgreSQL + pgvector for heavy/vector
- Blob storage in `/secure/`, metadata in DB
- Cloak integration (reuse episteme Python client pattern)
- Add to Cortex service manifest
- Deliverable: Datastore running, accessible through Cortex

### Phase 5 — Gateway Decision + Integration
> Unblocks: LLM access for agents

- **Decision required:** Absorb Gateway into Cortex as a route type, or run as parallel service?
- If parallel: add Cloak client (Rust SDK from Phase 2), remove own token system
- If absorbed: migrate provider dispatchers + sanitizer into Cortex
- Wire through Cortex proxy either way
- Complete: health probing, cost accounting, rate limiting, kill switch
- Deliverable: agents can call LLMs through the authenticated stack

### Phase 6 — Wheelhouse Implementation
> Unblocks: multi-agent coordination

- Commit to TypeScript (per meta-agent guidance in CLAUDE.md)
- Define resource manifest schema (YAML, per `wheelhouse/planning.md` decisions)
- Build core resource broker with plugin/channel model
- Implement GitHub plugin (first resource type)
- Build agent interface to Cortex MCP surface
- Deliverable: single agent can request and use a GitHub resource through Wheelhouse

### Phase 7 — MCP Federation + Polish
> Completes: the full agent interaction model

- Build Cortex MCP server (`cortex-mcp`)
- Auto-generate MCP tools from service manifest TOML
- Connect Wheelhouse agents to Cortex MCP surface
- Episteme React frontend (if still needed — MCP may suffice)
- YubiKey FIDO2 for Cloak admin
- CI/CD pipeline
- Deliverable: full stack operational, agents work end-to-end

---

## 7. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|:---:|:---:|------------|
| Cargo dependency conflicts between Cortex and Cloak | HIGH | HIGH | Align versions in Phase 2 before any shared code. Test compilation early. |
| Token format drift (Rust ↔ Python ↔ TypeScript) | MEDIUM | HIGH | Write cross-language integration test: Cloak Rust mints → Episteme Python verifies → Cerebro TS verifies. Same token, three languages. |
| Gateway fate undecided (absorb vs. parallel) | HIGH | MEDIUM | Decide before Phase 5. Absorbing is cleaner but Gateway is 2800+ LoC. Running parallel adds another Cloak client but preserves existing work. |
| Wheelhouse language choice (Python planned, ecosystem is Rust+TS) | MEDIUM | MEDIUM | CLAUDE.md already says "prefer TypeScript per meta-agent." Commit to TS and close the question. |
| Episteme framework duplication | MEDIUM | MEDIUM | Declare canonical location in Phase 1. Symlink or git submodule. Stop dual maintenance. |
| No CI/CD for any project | HIGH | MEDIUM | Set up before Phase 2. At minimum: compile check for Rust crates, `npm test` for TS projects, `pytest` for Episteme. |
| Cortex spec outpaces implementation | LOW | LOW | Focus on critical path features only. The spec covers edge cases (queue TTLs, CGNAT guards) that can wait. |
| Single-person bus factor | HIGH | HIGH | These docs help. Keep design-docs/ updated. Prioritize integration tests as living documentation. |

---

## Summary

**Where we are:** 2 of 15+ service connections work. The security layer (Cloak) is nearly done. The knowledge graph (Cerebro) and knowledge library (Episteme/Panopticon) are functional but isolated. The central orchestrator (Cortex) is a type-definition skeleton. The agent coordination layer (Wheelhouse) is design-only. Two planned services (Datastore, IDEA) don't exist.

**What's needed next:** Get Cloak verified against real Infisical, then build Cortex's minimum viable proxy to Episteme. That proves the architecture end-to-end with real auth. Everything else follows from there.
