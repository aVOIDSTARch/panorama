# Panorama — Final Stretch Plan

Everything below is ordered by priority. Each phase unlocks the next — the system is already code-complete and test-passing, so this is hardening, UX, and production readiness.

---

## Phase A: Operator Visibility (Admin Interface)
**Why first:** You can't debug or operate a system you can't see into. The logging DB and error catalog are already writing data — these panels just expose it. Everything here is pure frontend (HTMX fragments hitting existing APIs or SQLite).

### A1. Log Viewer panel
- New route `GET /api/logs` returning HTMX fragment
- Query `_system_logs` table (panorama-logging already writes here)
- Filters: service dropdown, level dropdown, error_code text, time range
- Auto-refresh via `hx-trigger="every 5s"`
- Wire the existing `/logs` nav link

### A2. Error Report Browser panel
- New route `GET /api/errors` returning HTMX fragment
- Query `_error_reports` table — group by code, show count + last occurrence
- Click-to-expand: full detail, suggestion text, instance_id
- Filter by severity, service, code prefix

### A3. Halt Controls panel
- New card on dashboard with halt status indicator
- "Halt All" button → `POST /cloak/admin/halt` with confirmation modal
- Per-service halt buttons (one per registered service)
- "Resume" button → `POST /cloak/admin/resume`
- Live halt state via polling `/health` halted field

### A4. Permission Manager panel
- `GET /api/permissions` → list current rules from `/cloak/admin/permissions`
- Add form: identity_pattern, service, operation_class, resources
- Delete button per rule → `DELETE /cloak/admin/permissions`

---

## Phase B: Security Hardening
**Why second:** The system works but auth is passwords and webhook signatures are unchecked. These are the gaps that matter before any external traffic hits the box.

### B1. Telnyx Ed25519 webhook verification
- Add `ed25519-dalek` crate to analog-communications
- Implement actual signature verification in `inbound.rs` line 40
- Verify `telnyx-signature-ed25519` header against `TELNYX_PUBLIC_KEY` + `telnyx-timestamp` + body
- Reject requests with invalid or missing signatures when key is configured
- Small, isolated change — one file, ~30 lines

### B2. Tailscale interface binding
- Read `TAILSCALE_INTERFACE` env var (e.g. `tailscale0`)
- In admin-interface main.rs, resolve the interface IP and bind to it instead of `0.0.0.0`
- Fallback to `127.0.0.1` if interface not found (fail-closed)
- ~20 lines in one file

### B3. YubiKey FIDO2/WebAuthn for admin auth
- Add `webauthn-rs` crate to admin-interface
- Registration ceremony: `/auth/register` page, store credential in Datastore `_webauthn_credentials` table
- Authentication ceremony: `/login` replaced with WebAuthn challenge/response
- Fallback to password if no credentials registered (bootstrap flow)
- Session cookie upgraded to include credential ID
- This is the largest single item — webauthn-rs handles the crypto but the ceremony flow is ~200 lines

---

## Phase C: Analog Communications Completion
**Why third:** The SMS pipeline works end-to-end but has three TODO stubs that affect security and functionality.

### C1. TOTP owner verification
- Extract 6-digit TOTP token from SMS body (regex: leading/trailing digits)
- Verify against shared secret stored in Infisical (`OWNER_TOTP_SECRET`)
- Upgrade identity level from `Known` → `Owner` on match
- ~40 lines in identity.rs

### C2. Recognized sender list
- New Datastore table `_recognized_senders` (phone, first_seen, last_seen, message_count)
- On inbound: check table, if found → `Recognized` level, update last_seen
- On dispatch to quarantine: insert sender into table
- ~50 lines across identity.rs and dispatch.rs

---

## Phase D: Admin Interface — Monitoring Panels
**Why fourth:** These are useful for operations but don't block anything. They require new API endpoints on downstream services.

### D1. Wheelhouse Monitor
- Add `GET /status` endpoint to wheelhouse hub (expose pool size, active agents, queue depth)
- New admin panel polling `/api/wheelhouse` → proxied to wheelhouse `/status`
- Agent state table: ID, tier, status, assigned task, uptime

### D2. Service Config Viewer
- Read-only view of `cortex-manifest.toml` (parsed, not raw)
- Show gateway route config (from gateway config TOML)
- Future: editable with restart trigger

### D3. Identity Panel
- List analog-communications allowed senders from config
- Show quarantined messages (needs new Datastore table or analog-comms endpoint)
- Manage recognized sender list from C2

---

## Phase E: cortex-mcp Implementation
**Why fifth:** MCP tooling is a force multiplier but the system operates without it. This is the biggest greenfield work remaining.

### E1. Auto-generate MCP tool definitions from cortex-manifest.toml
- Parse manifest at startup
- For each service: generate tool with name, description, input schema
- Expose via MCP protocol (JSON-RPC over stdio or HTTP)

### E2. Service-specific tool enrichment
- Cerebro: search, ingest, query tools
- Episteme: document retrieval, project listing
- Datastore: CRUD operations as tools

---

## Phase F: CI/CD and Cross-Language Testing
**Why last:** The system is tested in Rust (129 tests pass). Cross-language and CI are important but don't block deployment.

### F1. GitHub Actions pipeline
- `cargo check --workspace` + `cargo test --workspace`
- `cd services/cerebro && npm ci && npm test`
- Episteme: `python -m pytest` (once Episteme tests exist)
- Trigger: push to main, PR

### F2. Cross-language token format tests
- Rust: mint token with known key → serialize
- Python: deserialize + verify HMAC
- TypeScript: deserialize + verify HMAC
- Ensures Cloak tokens work across all three service languages

### F3. Integration test expansion
- Cold start sequence test (boot Cloak → register all services → verify)
- Agent E2E test (create job → spawn agent → complete → resolve)
- Full analog intake flow (mock Telnyx webhook → sanitize → dispatch → Cerebro)

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

| Phase | Items | Effort | Blocks |
|-------|-------|--------|--------|
| **A** Operator Visibility | 4 panels | ~2 sessions | Nothing — pure HTMX |
| **B** Security Hardening | 3 items | ~2 sessions | B3 (WebAuthn) is the hardest |
| **C** Analog Completion | 2 items | ~1 session | Needs Infisical for TOTP secret |
| **D** Monitoring Panels | 3 panels | ~1 session | D1 needs new wheelhouse endpoint |
| **E** cortex-mcp | 2 items | ~2-3 sessions | Greenfield, needs MCP spec research |
| **F** CI/CD + Testing | 3 items | ~1 session | Needs GitHub repo access |
| **G** Infrastructure | 3 items | ~1 session | Needs server access |

**Recommended starting point:** Phase A (you get immediate visibility into the running system) then B1+B2 (quick security wins before touching WebAuthn).
