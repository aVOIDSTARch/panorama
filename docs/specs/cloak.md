# Cloak Auth System — Implementation Plan

## Context

Cloak is the unified control plane and access management system for fail.academy infrastructure. It runs as a Rust/Axum service on port 8300, acting as the sole gateway to a self-hosted Infisical instance at :8200 on the Tailscale mesh. The `/a1/cloak/` repo is currently empty (template files only). The existing `cortex-cloak` crate will become a thin client that calls this standalone service.

Four primary responsibilities, each as a separate crate in one Cargo workspace, behind one unified interface.

---

## Workspace Structure

```
cloak/
├── Cargo.toml                    # workspace root
├── cloak-core/                   # shared types, errors, config
│   └── src/
│       ├── lib.rs
│       ├── types.rs              # ServiceId, TokenClaims, ServiceScope, OperationClass, etc.
│       ├── error.rs              # CloakError enum with IntoResponse
│       └── config.rs             # CloakConfig loaded from CLOAK_* env vars
├── cloak-registry/               # 1. Address Registry
│   └── src/
│       ├── lib.rs
│       ├── store.rs              # DashMap<ServiceId, RegisteredService>
│       ├── registration.rs       # register handler: validate manifest token, gen session + signing key
│       ├── sse.rs                # SSE channels (broadcast), halt/key-rotation push, heartbeat
│       └── routes.rs             # POST /cloak/services/register, GET halt-stream, GET list
├── cloak-permissions/            # 2. Permission Registry
│   └── src/
│       ├── lib.rs
│       ├── model.rs              # PermissionEntry, PermissionStore (RwLock<Vec>)
│       ├── engine.rs             # check_permission(claims, service, operation, resource) -> bool
│       └── routes.rs             # admin CRUD under /cloak/admin/permissions
├── cloak-secrets/                # 3. Secrets Broker
│   └── src/
│       ├── lib.rs
│       ├── infisical.rs          # InfisicalClient — sole code that talks to :8200
│       ├── cache.rs              # SecretCache with TTL (secrets only, never tokens)
│       └── routes.rs             # GET /cloak/secrets/:key
├── cloak-tokens/                 # 4. Token Validation & Issuance
│   └── src/
│       ├── lib.rs
│       ├── validation.rs         # validate flow: Infisical call + scope check (never cached)
│       ├── issuance.rs           # mint tokens via Infisical, return token + scope
│       ├── signing.rs            # HMAC-SHA256 sign/verify, key generation
│       └── routes.rs             # POST /cloak/validate, POST /cloak/tokens/issue
└── cloak-server/                 # Unified interface binary
    └── src/
        ├── main.rs               # tokio::main — config, init subsystems, serve
        ├── lib.rs                # re-export run()
        ├── state.rs              # AppState aggregating all subsystem states
        ├── router.rs             # merge all sub-routers into one Axum Router
        ├── middleware.rs          # halt_guard, tailscale_guard, session_auth
        ├── admin.rs              # halt/resume endpoints, YubiKey FIDO2
        └── health.rs             # GET /health with subsystem status
```

---

## Key Endpoints

| Endpoint | Method | Crate | Description |
|---|---|---|---|
| `/health` | GET | cloak-server | Subsystem status, uptime, halt state |
| `/cloak/services/register` | POST | cloak-registry | Service registration, returns session + signing key |
| `/cloak/services/:id/halt-stream` | GET | cloak-registry | SSE channel for halt/key-rotation signals |
| `/cloak/validate` | POST | cloak-tokens | Per-request token validation (always hits Infisical) |
| `/cloak/tokens/issue` | POST | cloak-tokens | Token minting via Infisical |
| `/cloak/secrets/:key` | GET | cloak-secrets | Serve secrets to authorized callers |
| `/cloak/admin/halt` | POST | cloak-server | YubiKey Level 1 halt — all validations rejected |
| `/cloak/admin/resume` | POST | cloak-server | Resume from halt |
| `/cloak/admin/halt/:service_id` | POST | cloak-server | Per-service halt via SSE push |
| `/cloak/admin/permissions` | CRUD | cloak-permissions | Manage permission entries |

---

## Implementation Phases

### Phase 1: Foundation
1. Create workspace `Cargo.toml` with all 6 members
2. Implement `cloak-core`: types, CloakError with IntoResponse, CloakConfig from env
3. Implement `cloak-server` skeleton: main.rs, AppState stub, router with `/health` only
4. Verify compiles and serves health on :8300

### Phase 2: Infisical Integration (Secrets Broker)
1. `cloak-secrets/infisical.rs`: InfisicalClient with `fetch_secrets()`, `validate_token()`, `mint_token()`
2. `cloak-secrets/cache.rs`: SecretCache with TTL refresh, background refresh task
3. Wire into AppState and startup (verify Infisical connectivity, initial pull)
4. Add `GET /cloak/secrets/:key` route
5. Integration test with mock Infisical server

### Phase 3: Token System
1. `cloak-tokens/signing.rs`: HMAC-SHA256 sign/verify (must produce tokens compatible with Episteme's Python `_verify_and_decode`)
2. `cloak-permissions/model.rs` + `engine.rs`: PermissionStore, operation class hierarchy, scope matching
3. `cloak-tokens/validation.rs`: full validate flow (Infisical + scope check, never cached)
4. `cloak-tokens/issuance.rs`: token minting via Infisical
5. Add `POST /cloak/validate` and `POST /cloak/tokens/issue` routes
6. End-to-end test: issue token → validate token

### Phase 4: Service Registry & SSE
1. `cloak-registry/store.rs`: ServiceStore with DashMap
2. `cloak-registry/registration.rs`: validate manifest token, gen session/signing key, store
3. `cloak-registry/sse.rs`: broadcast channels, halt/key-rotation push, 15s heartbeat
4. Add registration and halt-stream routes
5. End-to-end test: register → receive key → listen SSE → send halt

### Phase 5: Admin & Hardening
1. `cloak-server/middleware.rs`: halt_guard, tailscale_guard (check 100.64.0.0/10 CGNAT range)
2. `cloak-server/admin.rs`: halt/resume, per-service halt, YubiKey FIDO2 (webauthn-rs)
3. Session auth for admin routes
4. Permission admin CRUD routes
5. Structured logging, request tracing

---

## Configuration (env vars)

| Variable | Default | Required | Description |
|---|---|---|---|
| `CLOAK_PORT` | `8300` | No | Listen port |
| `CLOAK_INFISICAL_URL` | — | Yes | e.g. `http://100.x.y.z:8200` |
| `CLOAK_INFISICAL_TOKEN` | — | Yes | Machine identity token |
| `CLOAK_INFISICAL_PROJECT` | — | Yes | Infisical project slug |
| `CLOAK_INFISICAL_ENV` | `production` | No | Infisical environment |
| `CLOAK_SECRET_CACHE_TTL` | `30` | No | Seconds |
| `CLOAK_LOG_LEVEL` | `info` | No | RUST_LOG filter |
| `CLOAK_TAILSCALE_INTERFACE` | `tailscale0` | No | For admin route guard |
| `CLOAK_ADMIN_PASSWORD_HASH` | — | Yes (prod) | Argon2 hash |

---

## Key Dependencies

```toml
tokio = { version = "1", features = ["full"] }
axum = "0.7"
axum-extra = { version = "0.9", features = ["typed-header"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
reqwest = { version = "0.12", features = ["json"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["trace", "cors"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
chrono = { version = "0.4", features = ["serde"] }
dashmap = "6"
uuid = { version = "1", features = ["v4"] }
base64 = "0.22"
hmac = "0.12"
sha2 = "0.10"
```

---

## cortex-cloak Transition

After Phase 4, rewrite `cortex-cloak` from a server to a client library:
- `CloakClient` struct with `register()`, `listen_halt_stream()`, `verify_token_locally()`, `validate_token_remote()`
- Remove axum/tower deps, keep only reqwest + serde + tokio
- Remove `main.rs` (no longer a binary)
- `cortex-auth/AuthClient` either merges into or delegates to `CloakClient`

---

## Critical Reference Files

- [cortex-spec.md](../design-docs/cortex/cortex-spec.md) — canonical protocol spec
- [episteme cloak/client.py](../episteme/interface/service/cloak/client.py) — Python reference for registration + SSE
- [episteme cloak/auth.py](../episteme/interface/service/cloak/auth.py) — Python reference for HMAC-SHA256 token format
- [episteme cloak/models.py](../episteme/interface/service/cloak/models.py) — Wire format models
- [cortex-core/src/lib.rs](../cortex/cortex-core/src/lib.rs) — Shared domain types to mirror

---

## Verification

1. `cargo build` — workspace compiles clean
2. `cargo test` — all unit + integration tests pass
3. Start cloak-server, hit `GET /health` → 200 with subsystem status
4. Mock Infisical: issue token → validate token → confirmed allowed
5. Register mock service → receive signing key → verify token locally with that key
6. SSE halt test: register → listen → send halt → confirm receipt
7. Halt guard: set halted → all requests 503 except /health and /admin/resume
8. Compatibility: Episteme Python client can register and verify tokens against the running Cloak server
