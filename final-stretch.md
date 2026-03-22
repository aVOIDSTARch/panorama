# Panorama — Final Stretch Plan

Everything below is ordered by priority. Each phase unlocks the next — the system is already code-complete and test-passing, so this is hardening, UX, and production readiness.

---

## Phase A: Operator Visibility (Admin Interface) ✅ COMPLETE
**Why first:** You can't debug or operate a system you can't see into. The logging DB and error catalog are already writing data — these panels just expose it. Everything here is pure frontend (HTMX fragments hitting existing APIs or SQLite).

### A1. Log Viewer panel ✅
- `GET /api/logs` returning HTMX table fragment
- Queries `_system_logs` table with filters (service, level, error_code)
- Auto-refresh via `hx-trigger="every 5s"`

### A2. Error Report Browser panel ✅
- `GET /api/errors/summary` (grouped by code) + `GET /api/errors/recent`
- Filters by severity, service, code prefix
- Full detail with suggestion text

### A3. Halt Controls panel ✅
- Dashboard card with halt status indicator (green/red)
- "Halt All" / "Resume" / per-service halt buttons
- Uses `hx-confirm` for destructive actions

### A4. Permission Manager panel ✅
- Lists current rules from Cloak
- Add form + per-rule delete buttons

---

## Phase B: Security Hardening ✅ COMPLETE

### B1. Telnyx Ed25519 webhook verification ✅
- `ed25519-dalek` in analog-communications
- Verifies `telnyx-signature-ed25519` header before JSON parsing
- Raw body signature verification over `{timestamp}|{body}`

### B2. Tailscale interface binding ✅
- `resolve_bind_address()` in admin-interface main.rs
- Reads `TAILSCALE_INTERFACE` env var, parses `ip addr show` output
- Fail-closed to `127.0.0.1` if interface not found

### B3. YubiKey FIDO2/WebAuthn for admin auth ✅
- `webauthn-rs` 0.5 with passkey registration + authentication ceremonies
- Credentials stored in `data/webauthn_credentials.json`
- Login page shows WebAuthn button + password fallback
- First visit redirects to `/auth/register` for initial setup
- Counter tracking to detect credential cloning

---

## Phase C: Analog Communications Completion ✅ COMPLETE

### C1. TOTP owner verification ✅
- RFC 6238 HMAC-SHA1 TOTP with base32 secrets
- Extracts 6-digit code from SMS body
- +-1 time step skew for clock drift
- Owner + valid TOTP → `Owner` level; owner without TOTP → `Known`

### C2. Recognized sender list ✅
- In-memory cache loaded from Datastore at startup
- Unknown senders recorded via `_recognized_senders` collection upsert
- Next message from same number → `Recognized` level

---

## Phase D: Admin Interface — Monitoring Panels ✅ COMPLETE

### D1. Wheelhouse Monitor ✅
- Added `GET /agents` endpoint to wheelhouse (returns agent list with full metadata)
- Admin panel shows pool summary (total/idle/active/retiring) + agent state table
- Polls every 10s

### D2. Service Config Viewer ✅
- Reads `cortex-manifest.toml` directly (parsed TOML, not raw)
- Shows service name, base URL, health path, timeout, queue TTL

### D3. Identity Panel ✅
- Shows owner number, allowed senders from config
- Shows recognized senders from Datastore with last_seen timestamps
- Polls every 30s

---

## Phase E: cortex-mcp Implementation ✅ COMPLETE

### E1. Auto-generate MCP tool definitions from cortex-manifest.toml ✅
- Parses manifest at startup → generates `{service}_request` proxy tool per service
- JSON-RPC 2.0 over stdio transport (standard MCP protocol)
- Supports `initialize`, `tools/list`, `tools/call`, `ping`
- Logs to stderr (stdout is the transport)

### E2. Service-specific tool enrichment ✅
- Cerebro: `cerebro_search`, `cerebro_ingest`, `cerebro_query`
- Episteme: `episteme_list_projects`, `episteme_get_document`, `episteme_search`
- Datastore: `datastore_query`, `datastore_upsert`, `datastore_delete`
- 12 total tools (3 generic proxy + 9 enriched)

---

## Phase F: CI/CD and Cross-Language Testing ✅ COMPLETE (F1+F2)

### F1. GitHub Actions pipeline ✅
- `.github/workflows/ci.yml` — 4 jobs:
  - Rust: check + test + clippy
  - Cerebro: npm ci + test + build
  - Cross-language token verification
  - Formatting: cargo fmt check
- Triggers on push to main, PR to main

### F2. Cross-language token format tests ✅
- Rust: mints token with known key → writes fixture.json
- Python: verifies HMAC-SHA256 + decodes claims ✅
- TypeScript: verifies HMAC-SHA256 + decodes claims ✅
- All three languages produce identical claims from the same token

### F3. Integration test expansion
- Cold start sequence test (boot Cloak → register all services → verify)
- Agent E2E test (create job → spawn agent → complete → resolve)
- Full analog intake flow (mock Telnyx webhook → sanitize → dispatch → Cerebro)
- **Note:** Integration test harness exists in `crates/test-harness/` + `tests/integration/` — see the plan in `.claude/plans/` for the full design

---

## Phase G: Infrastructure
**Why separate:** These are deployment concerns, not code.

### G1. Infisical deployment
- Stand up Infisical instance (self-hosted or cloud)
- Populate secrets: API keys, TOTP secrets, database credentials
- Update .env with real URL + token

### G2. Tailscale network setup
- Install Tailscale on the server
- Add machine to tailnet
- Configure admin-interface to bind to tailscale0

### G3. Episteme service setup
- Python venv creation, dependency install
- Startup script / systemd unit
- Add to install.sh

---

## Summary

| Phase | Items | Status |
|-------|-------|--------|
| **A** Operator Visibility | 4 panels | ✅ Complete |
| **B** Security Hardening | 3 items | ✅ Complete |
| **C** Analog Completion | 2 items | ✅ Complete |
| **D** Monitoring Panels | 3 panels | ✅ Complete |
| **E** cortex-mcp | 2 items | ✅ Complete |
| **F** CI/CD + Testing | F1+F2 done, F3 pending | ✅ Mostly complete |
| **G** Infrastructure | 3 items | Pending (deployment) |

**Remaining:** F3 (integration test expansion — harness exists, tests designed) and Phase G (infrastructure — requires server access).
