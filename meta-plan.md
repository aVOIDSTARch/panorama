# Panorama System — Full Implementation Plan

## Context

Panorama is a personal-scale, self-hosted AI infrastructure monorepo for fail.academy. It encompasses 12 projects in `work-so-far/` at varying stages of completion — from 90% done (Cerebro, Cloak) to completely empty (Wheelhouse, Datastore, admin-interface, analog-communications). The system currently has **2 of 15+ required service connections working** (Episteme <-> Cloak only).

This plan takes the system from its current fragmented state to a fully integrated monorepo where all services communicate through Cloak (auth) and Cortex (proxy), Wheelhouse orchestrates agents, and the full data pipeline (analog intake -> knowledge graph -> agent work -> LLM access) is operational.

### Locked Decisions
1. **Cortex** stays as separate proxy layer at :9000
2. **Wheelhouse** in Rust, with task-manager + agent-lifecycle as internal crates
3. **Gateway** runs parallel to Cortex with Cloak integration (keep existing 3886 LoC)
4. **Monorepo** — all projects move into panorama/ root
5. **Panopticon** absorbed into Episteme
6. **Admin interface** uses Axum + HTMX
7. **Analog-communications** targets Rust-native runtime

### Conflicts to Resolve During Implementation
- Cargo dependency versions: Cortex/Gateway use thiserror 1, tower 0.4, tower-http 0.5 vs Cloak's thiserror 2, tower 0.5, tower-http 0.6
- cortex-cloak crate is a stub server — must become cloak-sdk client library
- Cerebro port :3000 must change to :8101
- Cerebro auth (env-var bearer) must become Cloak HMAC verification
- Gateway's independent token system must be replaced with Cloak
- No reusable Rust Cloak SDK exists — must be extracted from Cloak
- Episteme framework duplicated in panopticon/ — must declare canonical location
- Cerebro design docs scattered across 3 locations — must consolidate

### Important: Migration Safety
- **Do NOT delete working code files during migration.** Copy to new locations first. Only remove originals after full verification that the new structure compiles and tests pass.
- `work-so-far/` is preserved until all phases are verified.

---

## Phase 0: Monorepo Restructuring
**Status:** COMPLETE

**Goal:** Reorganize all 12 projects into a unified monorepo with a root Cargo workspace.

### Target Structure
```
panorama/
  Cargo.toml                    # Root Rust workspace
  Cargo.lock

  crates/                       # All Rust crates
    cloak-core/                 # from work-so-far/cloak/cloak-core/
    cloak-registry/             # from work-so-far/cloak/cloak-registry/
    cloak-permissions/          # from work-so-far/cloak/cloak-permissions/
    cloak-secrets/              # from work-so-far/cloak/cloak-secrets/
    cloak-tokens/               # from work-so-far/cloak/cloak-tokens/
    cloak-server/               # from work-so-far/cloak/cloak-server/
    cloak-sdk/                  # NEW — reusable Cloak client library
    cortex-core/                # from work-so-far/cortex/cortex-core/
    cortex-api/                 # from work-so-far/cortex/cortex-api/
    cortex-mcp/                 # from work-so-far/cortex/cortex-mcp/
    cortex-auth/                # from work-so-far/cortex/cortex-auth/
    gateway/                    # from work-so-far/gateway/
    wheelhouse/                 # NEW — future
    wheelhouse-task-manager/    # NEW — future
    wheelhouse-agent-lifecycle/ # NEW — future
    admin-interface/            # NEW — future (Axum + HTMX)
    analog-communications/      # NEW — future (Rust-native)
    datastore/                  # NEW — future

  services/                     # Non-Rust services
    cerebro/                    # from work-so-far/cerebro/ (TypeScript)
    episteme/                   # from work-so-far/episteme/ (Python + framework)

  docs/
    design-docs/                # from work-so-far/design-docs/
    specs/                      # consolidated spec files (cortex-spec, task-manager, etc.)
    wheelhouse-designs/         # from work-so-far/wheelhouse/ design docs

  master-plan-1.2.md
  governance.md
  meta-plan.md                  # THIS FILE
```

### Key Operations
1. Create root `Cargo.toml` with `[workspace]` listing all crate members and unified `[workspace.dependencies]` using Cloak's versions (thiserror 2, tower 0.5, tower-http 0.6, axum 0.7, tokio 1, serde 1, reqwest 0.12, etc.)
2. **Copy** (not move) each Cloak sub-crate from `work-so-far/cloak/` to `crates/`
3. **Copy** Cortex sub-crates (drop cortex-cloak — replaced by cloak-sdk in Phase 1)
4. **Copy** Gateway to `crates/gateway/`
5. **Copy** Cerebro to `services/cerebro/`, Episteme to `services/episteme/`
6. **Copy** design docs into `docs/`
7. Update all new `Cargo.toml` files to use `workspace.dependencies` syntax
8. Fix Cortex/Gateway dependency versions (thiserror 1->2, tower 0.4->0.5, tower-http 0.5->0.6)
9. `work-so-far/` is preserved as reference — do NOT delete

### Code Transforms Required
- **Gateway `error.rs`**: thiserror v1->v2 (check `#[error(transparent)]` on tuple variants)
- **Cortex `cortex-core/src/lib.rs`**: thiserror v1->v2 on CortexError enum
- **All crate Cargo.toml files**: Change from inline dependency versions to `dep.workspace = true`

### Verification
- `cargo check --workspace` compiles all Rust crates
- `cd services/cerebro && npm test` — 30 tests pass
- `cd services/episteme/interface/service && python -m pytest` — tests pass

### Critical Files
- `work-so-far/cloak/Cargo.toml` — canonical dependency versions
- `work-so-far/cortex/cortex-core/src/lib.rs` — 72 lines, needs thiserror v2 update
- `work-so-far/gateway/src/error.rs` — needs thiserror v2 update
- `work-so-far/gateway/Cargo.toml` — needs version alignment

---

## Phase 1: Cloak Completion + cloak-sdk
**Status:** COMPLETE (Phase 1A done — cloak-sdk crate. Phase 1B integration tests deferred to Phase 9)

**Goal:** Extract a reusable Rust `cloak-sdk` crate. Finish Cloak to production readiness.

### Phase 1A: cloak-sdk crate (NEW)

Create `crates/cloak-sdk/` providing:
- `CloakClient` struct: `register()`, `listen_halt_stream()`, `verify_token()`
- Registration: POST `/cloak/services/register` -> receive session_id + signing_key
- SSE listener: persistent connection with reconnect, self-halt on failures
- Token verification: local HMAC-SHA256 using signing key from registration
- `CloakState`: session_id, signing_key, halted flag, halt_reason
- Axum middleware: token verification + halt guard (reusable by any Rust service)

**Reference implementation to port:** `services/episteme/interface/service/cloak/client.py` (152 lines — registration, SSE halt, key rotation, token verify)

**Dependencies:** `cloak-core` (shared types), `cloak-tokens` (signing module). Does NOT depend on `cloak-server`.

Files:
- `crates/cloak-sdk/Cargo.toml`
- `crates/cloak-sdk/src/lib.rs`
- `crates/cloak-sdk/src/client.rs`
- `crates/cloak-sdk/src/state.rs`
- `crates/cloak-sdk/src/sse.rs`
- `crates/cloak-sdk/src/middleware.rs`

### Phase 1B: Cloak Integration Tests

- `crates/cloak-server/tests/integration_test.rs` — full lifecycle (register, token issue, validate, halt)
- `crates/cloak-sdk/tests/integration_test.rs` — SDK connects, registers, verifies tokens

### Verification
- `cargo test -p cloak-sdk -p cloak-server`
- Manual: Cloak on :8300, cloak-sdk registers, verifies a token, receives halt event

**Unblocks:** Phases 2, 3, 4, 5, 6 (everything needs cloak-sdk)

---

## Phase 2: Cortex — From Stub to Minimum Viable Proxy
**Status:** COMPLETE (cortex-core rebuilt with manifest loading, cortex-api is a full Axum proxy, cortex-auth delegates to cloak-sdk, cortex-manifest.toml created)

**Goal:** Transform Cortex from `println!` stub into a functioning proxy at :9000.

### Phase 2A: Cortex Core Rebuild

- Delete `cortex-cloak` crate entirely (replaced by cloak-sdk)
- Update `cortex-core/src/lib.rs`: keep ServiceManifest, HealthState, FailureState, CortexError types. Remove Token/ServiceScope/OperationClass (live in cloak-core now). Add TOML manifest loading.
- Resolve type duplication: cortex-core references cloak-core types where they overlap

Files modified:
- `crates/cortex-core/src/lib.rs`
- `crates/cortex-core/Cargo.toml` — add cloak-core dep, add toml

### Phase 2B: Cortex API Server

Build the actual HTTP proxy:
1. Load service manifest at startup (Episteme :8100, Cerebro :8101, Datastore :8102)
2. Register with Cloak via cloak-sdk
3. Health FSM per downstream: Healthy -> Queuing -> Degraded -> PartialFail -> HardFail
4. Proxy routes: `/{service_name}/**` -> configured base_url
5. Token validation via cloak-sdk middleware
6. Health check polling (10s interval, 2-fail threshold)

Files (rewrite `cortex-api`):
- `crates/cortex-api/src/lib.rs` — full rewrite
- `crates/cortex-api/src/config.rs` — manifest loading
- `crates/cortex-api/src/proxy.rs` — HTTP reverse proxy
- `crates/cortex-api/src/health.rs` — health FSM
- `crates/cortex-api/src/middleware.rs` — auth (delegates to cloak-sdk)
- `crates/cortex-api/src/state.rs` — AppState
- `crates/cortex-api/src/router.rs` — route builder
- `cortex-manifest.toml` — default service manifest

### Phase 2C: cortex-auth update

Thin wrapper around cloak-sdk for Cortex-specific patterns.

### Phase 2D: cortex-mcp (stub only)

Minimal MCP server setup — full implementation deferred to Phase 9.

### Verification
- `cargo test -p cortex-api`
- **First integrated request path:** Start Cloak -> Start Episteme -> Start Cortex. `curl http://localhost:9000/episteme/api/v1/framework/tree` returns data through Cortex with Cloak auth.

**Unblocks:** Phase 4 (downstream services via Cortex)

---

## Phase 3: Gateway Cloak Integration
**Status:** COMPLETE

**Goal:** Replace Gateway's independent token system with Cloak. Preserve all 3886 LoC of existing functionality.

**Can run in parallel with Phase 2.**

### Phase 3A: Add cloak-sdk to Gateway

- Gateway already in `crates/gateway/` with aligned deps (from Phase 0)
- Add `cloak-sdk` and `cloak-core` dependencies

### Phase 3B: Replace Token System

- Delete `src/identity/tokens.rs` (198 lines) — replaced by cloak-sdk
- Create `src/identity/cloak_auth.rs` — thin adapter around cloak-sdk middleware
- Update `server.rs`: replace `authenticate()` with cloak-sdk, update AppState
- Update `main.rs`: add Cloak registration at startup, remove token CLI subcommands
- Update affected tests

Files modified:
- `crates/gateway/src/server.rs`
- `crates/gateway/src/main.rs`
- `crates/gateway/src/identity/mod.rs`

Files deleted:
- `crates/gateway/src/identity/tokens.rs` — replaced by cloak-sdk

Files created:
- `crates/gateway/src/identity/cloak_auth.rs`

### Verification
- `cargo test -p gateway`
- Start Cloak + Gateway. Gateway registers. Cloak-issued token accepted by Gateway.

**Unblocks:** Phase 7 (Wheelhouse calls Gateway for LLM access)

---

## Phase 4: Downstream Service Integration
**Status:** COMPLETE

**Goal:** Get Cerebro and Episteme properly integrated with Cloak/Cortex.

### Phase 4A: Cerebro Cloak Integration

- Fix port: 3000 -> 8101 in `services/cerebro/src/api/server.ts`
- Create TypeScript Cloak client (mirror Python implementation):
  - `services/cerebro/src/cloak/client.ts` — register, SSE halt, verify
  - `services/cerebro/src/cloak/types.ts` — shared types
- Replace `services/cerebro/src/api/auth.ts` env-var check with Cloak HMAC verification
- Add Cloak registration at server startup

### Phase 4B: Episteme Panopticon Absorption

- Absorb Panopticon's 10 MCP tools as new API routes in Episteme service
- Declare `services/episteme/episteme-framework/` as canonical framework location
- Remove framework duplication

Files created:
- `services/episteme/interface/service/routers/mcp_tools.py`

### Phase 4C: Documentation Consolidation

- Consolidate Cerebro design docs from 3 locations into `docs/specs/cerebro/`
- Fix gateway design doc naming collision (titled "Cloak" but is about Gateway)

### Verification
- Start full stack: Cloak -> Cortex -> Cerebro + Episteme
- `GET http://localhost:9000/cerebro/health` returns 200 through Cortex
- `GET http://localhost:9000/episteme/api/v1/framework/tree` returns data
- Both services appear in `GET http://localhost:8300/cloak/services`

**Unblocks:** Phase 7 (agents access Cerebro/Episteme via Cortex)

---

## Phase 5: Datastore Service (NEW)
**Status:** COMPLETE

**Goal:** Build from scratch. Rust crate at :8102 per cortex-spec.md section 4.

**Can run in parallel with Phase 3 and Phase 4** after Phase 1 completes.

### What Gets Built
- Axum HTTP server with Cloak registration via cloak-sdk
- SQLite backend (WAL mode) for lightweight state/metadata
- PostgreSQL + pgvector for vector/RAG workloads
- Blob storage (local filesystem under `/secure/`, metadata in DB)
- Three-tier access: ORM CRUD (safe), named queries (curated), raw SQL (privileged)
- Routes: `/objects`, `/queries`, `/schema`, `/blobs`
- Cortex manifest entry at :8102

Files:
- `crates/datastore/Cargo.toml`
- `crates/datastore/src/{main,lib,config,sqlite,postgres,blob,routes,state}.rs`

### Verification
- `cargo test -p datastore`
- Registers with Cloak, accessible through Cortex at `/datastore/**`

---

## Phase 6: Admin Interface (NEW)
**Status:** COMPLETE

**Goal:** Axum + HTMX single-user admin panel per design spec.

**Can run in parallel with Phases 5 and 7.**

### What Gets Built
- Axum server serving HTMX-rendered HTML
- YubiKey FIDO2/WebAuthn auth (webauthn-rs crate)
- Panels: Health dashboard, Wheelhouse monitor, Identity manager, Log viewer, Config viewer
- Reads from other services' health/status endpoints
- Tailscale/LAN only (not public)
- Templates via askama or minijinja

Files:
- `crates/admin-interface/Cargo.toml`
- `crates/admin-interface/src/{main,lib}.rs`
- `crates/admin-interface/src/auth/{mod,session,webauthn}.rs`
- `crates/admin-interface/src/api/{health,wheelhouse,gateway,identity,logs}.rs`
- `crates/admin-interface/src/templates/*.html`
- `crates/admin-interface/src/static/{style.css,htmx.min.js}`

### Verification
- Browser access on Tailscale shows health dashboard
- YubiKey gates all access

---

## Phase 7: Wheelhouse Core (NEW — Largest Build)
**Status:** COMPLETE

**Goal:** Implement the 4-tier agent orchestration engine in Rust.

### Phase 7A: wheelhouse-task-manager crate

Implement spec from `docs/specs/task-manager.md` (23K words):
- `TaskLifecycleService` with `create()` and `teardown()`
- Types: AgentBrief (17 fields), TaskObject, SuccessContract, OutputContract, AgentResolution, ResolutionCode (18 codes), AgentFate

Files:
- `crates/wheelhouse-task-manager/src/{lib,brief,deconstructor,types}.rs`

### Phase 7B: wheelhouse-agent-lifecycle crate

- Agent pool management: Spawn -> Idle -> Active -> Retiring -> Dead
- Conservative lifetime defaults, fail-loudly semantics

Files:
- `crates/wheelhouse-agent-lifecycle/src/{lib,pool,fate}.rs`

### Phase 7C: wheelhouse core

- Hub tier (external requests, cascade routing, API-tier frugality invariant)
- Orchestrator tier (Job decomposition, proof_chain tracking)
- Foreman (skill crystallization from RefinementCorpus, quality gate)
- Plate-based VRAM allocation
- Resource manifest schema (YAML, per planning.md decisions)
- Cloak registration + Cortex integration

Files:
- `crates/wheelhouse/src/{main,hub,orchestrator,job,cascade,foreman,plates,config,state}.rs`

### Verification
- `cargo test --workspace`
- Integration: Wheelhouse registers with Cloak, creates a task, dispatches agent through Cortex to Episteme, agent calls Gateway for LLM, result committed to Datastore, task teardown completes

---

## Phase 8: Analog Communications (NEW)
**Status:** COMPLETE

**Goal:** Rust-native replacement for n8n IDEA pipeline.

### What Gets Built
- Telnyx webhook receiver (SMS intake, Ed25519 signature verification)
- Sanitization: replay prevention, length limits, control chars, E.164 validation, label allowlist
- Identity system: 4 levels, TOTP for owner commands
- Dispatch: IDEA_CAPTURE trigger to Wheelhouse (scoped)
- Quarantine system for unverified senders
- Cloak registration

### Pre-requisite
- Export n8n workflows from Docker to JSON (RESTORE.md procedure)
- TFN carrier verification must be complete

Files:
- `crates/analog-communications/src/{main,lib,inbound,sanitization,identity,pipeline,dispatch,config}.rs`

**Lowest priority — n8n is live and functional. This replaces it when ready.**

---

## Phase 9: Full Integration + Polish
**Status:** DEFERRED (remaining items noted below)

**Goal:** End-to-end system verification and remaining items.

### Integration Tests
1. **Cold start sequence:** Infisical -> Cloak -> Cortex -> all downstream register
2. **Agent E2E:** Wheelhouse task -> agent -> Cerebro via Cortex -> Gateway for LLM -> Datastore commit -> teardown
3. **Halt cascade:** Cloak admin halt -> SSE propagates -> all services stop
4. **Admin visibility:** admin-interface shows all services, job queue, agents
5. **Analog intake:** SMS -> quarantine -> approved -> Cerebro write via Cortex

### Remaining Items
- cortex-mcp full implementation (auto-generate MCP tools from service manifest)
- CI/CD pipeline (cargo check, npm test, pytest)
- Backblaze backup integration
- Episteme GitHub automated backup (deploy key + systemd timer)
- Cross-language token format test (Cloak Rust mints -> Episteme Python verifies -> Cerebro TS verifies)

---

## Dependency Graph

```
Phase 0 (monorepo restructure)
    │
Phase 1 (Cloak + cloak-sdk)
    │
    ├──► Phase 2 (Cortex MVP)
    │        │
    │        ├──► Phase 4 (Cerebro + Episteme integration)
    │        │
    │        └──► Phase 5 (Datastore) ─────────┐
    │                                           │
    ├──► Phase 3 (Gateway Cloak) ──────────────┤
    │                                           │
    ├──► Phase 6 (Admin interface) ◄────────────┤
    │                                           │
    └───────────────────────────────────────────┴──► Phase 7 (Wheelhouse)
                                                         │
                                                    Phase 8 (Analog comms)
                                                         │
                                                    Phase 9 (Integration)
```

**Parallelization:** After Phase 1, Phases 2+3 run in parallel. After Phase 2, Phases 4+5 run in parallel. Phase 6 can start anytime after Phase 1. Phase 8 can start anytime after Phase 1.

---

## Risk Register

| Risk | Mitigation |
|------|-----------|
| Cargo version conflicts during Phase 0 | Align to Cloak's versions first. `cargo check --workspace` before any code changes. |
| Token format drift (Rust/Python/TS) | Cross-language integration test in Phase 9. Same token, three languages. |
| Gateway refactoring breaks existing 3886 LoC | Phase 3 is surgical: only token system replaced. All other modules untouched. Run existing tests throughout. |
| Wheelhouse scope creep (22+ open items) | Phase 7 builds minimum viable: Hub + one Orchestrator + one Specialist. Foreman and advanced routing deferred. |
| n8n workflow data loss | Export n8n workflows to JSON before Phase 8 begins. |
| Single-person bus factor | These docs + integration tests as living documentation. |

---

## Progress Log

_Update this section as phases are completed._

| Phase | Started | Completed | Notes |
|-------|---------|-----------|-------|
| 0 | 2026-03-22 | 2026-03-22 | All crates compile with unified deps. Cerebro 30/30, Episteme 125/125 tests pass. thiserror 1->2, tower 0.4->0.5, tower-http 0.5->0.6 resolved without code changes. |
| 1 | 2026-03-22 | — | Phase 1A complete (cloak-sdk crate). Phase 1B (integration tests) deferred to Phase 9. |
| 2 | 2026-03-22 | 2026-03-22 | cortex-core rebuilt, cortex-api full proxy server, cortex-auth delegates to cloak-sdk. 113 tests pass across workspace. |
| 3 | 2026-03-22 | 2026-03-22 | TokenStore replaced with cloak-sdk. CloakState in AppState, cloak_auth + halt_guard middleware on request routes. Token CLI removed. cloak_auth.rs adapter bridges TokenClaims -> CallerIdentity. |
| 4 | 2026-03-22 | 2026-03-22 | 4A: Cerebro port 3000->8101, TypeScript Cloak client (register, SSE halt, HMAC auth), env-var auth replaced. 4B: Panopticon already absorbed (30+ endpoints in Episteme). 4C: Docs already consolidated in Phase 0. 30/30 Cerebro tests pass. |
| 5 | 2026-03-22 | 2026-03-22 | Datastore crate built from scratch: SQLite WAL backend, blob storage with path-traversal prevention, CRUD routes (/objects, /queries, /schema, /blobs), Cloak integration, health endpoint. Added to workspace. |
| 6 | 2026-03-22 | 2026-03-22 | Admin interface crate built: Axum + HTMX, session auth (WebAuthn future), dashboard with live health/service panels, dark theme. Password-gated, Tailscale/LAN only. |
| 7 | 2026-03-22 | 2026-03-22 | Three crates: task-manager (types, brief construction, SHA-256 integrity, teardown/deconstructor, 18 resolution codes, agent fate), agent-lifecycle (pool with DashMap, spawn/assign/complete/retire/terminate, fate application), wheelhouse core (hub API, orchestrator with job decomposition, cascade routing with complexity estimation, Cloak integration). |
| 8 | 2026-03-22 | 2026-03-22 | analog-communications crate: Telnyx webhook handler, E.164 validation, control char sanitization, 4-level identity (Owner/Known/Recognized/Unknown), quarantine for unknown senders, dispatch to Cerebro via Cortex. Cloak integration. |
| 9 | — | — | Deferred. Remaining: integration tests (cold start, agent E2E, halt cascade), cortex-mcp full implementation, CI/CD, cross-language token tests, Backblaze backup, admin WebAuthn upgrade. |
