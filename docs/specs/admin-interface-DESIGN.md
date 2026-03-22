# admin-interface — Design Specification

**Project:** Panorama / fail.academy · Flickersong · L. Casinelli Jr.
**Status:** EMPTY — design in progress, implementation pending
**Version:** 0.1
**Last Updated:** 2026-03-22

> **Patching note:** Use `str_replace` against specific sections. Do not regenerate in full. Update version and date on every patch.

---

## 1. Purpose and Scope

`admin-interface` is the operator's control surface for the full Panorama system. It is the human's primary interaction point with Wheelhouse and the data plane outside of the analog intake path.

Its job is to make the system visible, controllable, and debuggable without requiring the operator to SSH into the server, inspect raw files, or write ad-hoc scripts. It is not a public product. It has exactly one user: L. Casinelli Jr.

**What belongs here:**
- System health dashboard (all services, all repos)
- Wheelhouse job and task monitoring (queue depth, active agents, recent resolutions)
- Service configuration management (read config, apply changes via Panorama templates)
- Identity and privilege management (number registry, level assignments, policy files)
- Model registry browser (active plates, VRAM allocation state)
- Log viewer (structured log access across services)
- Operator command dispatch (send commands to Wheelhouse Hub, trigger workflows)
- Auth surface (YubiKey FIDO2 gate)

**What does not belong here:**
- Business logic — this is a control surface, not an application
- Direct database writes — all mutations go through the appropriate service APIs
- Public-facing anything — this is operator-only, not exposed

---

## 2. Design Axioms (Local)

In addition to the Panorama-wide axioms, the following constraints apply specifically to this repo:

- **Local-first access.** Admin-interface is accessible only over Tailscale or direct LAN. It is never exposed to the public internet. This is not a feature to be added later — it is a hard architectural constraint from day one.
- **Read-mostly, write-through-APIs.** The interface reads system state directly where sensible (health endpoints, log files). All writes go through the service APIs they belong to. Admin-interface does not own any data.
- **Auth is non-negotiable.** YubiKey FIDO2/WebAuthn gates all access. There is no password fallback. There is no "skip auth in dev mode." The auth model is consistent with secure-gateway.
- **Fail visibly.** If a service is unreachable or misconfigured, the interface shows that clearly. It does not mask partial failures or return stale data silently.

---

## 3. Open Decision: Frontend Model

The frontend architecture is the primary unresolved decision blocking implementation. Three options are on the table:

### Option A: TUI — Ratatui

A terminal-based interface running directly on the server or over SSH/Tailscale.

**Pros:** Zero web stack. Pure Rust. No browser dependency. Fast to build. No auth surface on a network port.
**Cons:** Limited layout flexibility. No charts or rich data visualization without significant effort. Mobile access requires SSH client.
**Best if:** The operator spends most time at a terminal and values simplicity over richness.

### Option B: Web UI — Axum + HTMX

A lightweight web application. Axum serves HTML fragments; HTMX handles dynamic updates without a heavy JS framework.

**Pros:** Browser-accessible from any device on the tailnet. Rich layout. Charts via a small JS library (e.g., Chart.js). Minimal JavaScript complexity.
**Cons:** Web stack adds surface area. Requires HTTPS on the tailnet (manageable with Tailscale's built-in TLS). Auth must be handled at the HTTP layer.
**Best if:** The operator wants visibility from mobile or multiple devices.

### Option C: Native App

A native macOS/iOS application.

**Pros:** Best UX. Integrates with system auth (Touch ID as second factor alongside YubiKey). Native notifications.
**Cons:** Significant build investment. Requires a backend API regardless (so Axum backend is needed anyway). Tighter coupling to Apple platform.
**Best if:** Long-term investment is justified and the operator wants a polished personal tool.

### Current Recommendation

**Option B (Axum + HTMX)** is the most pragmatic starting point. It delivers cross-device visibility, keeps the Rust-native backend constraint, and avoids the complexity of a native app. Option C can be layered on top of the Option B backend later if warranted.

> **This decision must be made before implementation begins.** Log the decision in this document when resolved.

---

## 4. Backend Architecture

Regardless of frontend choice, the backend is Axum (Rust). It is a thin API and template server — it does not own state.

### 4.1 Structure

```
admin-interface/
  src/
    main.rs               # Axum server, route mounting, auth middleware
    auth/
      mod.rs              # YubiKey FIDO2/WebAuthn session management
      session.rs          # Session token issuance and validation
    api/
      health.rs           # Aggregated health endpoint — polls all services
      wheelhouse.rs       # Wheelhouse Hub API proxy (job queue, agent pool)
      gateway.rs          # secure-gateway proxy (:8000/:8001)
      config.rs           # Configuration read/template-apply via Panorama
      identity.rs         # Number registry, level management
      models.rs           # Model registry browser, plate state
      logs.rs             # Log aggregation and structured viewing
    frontend/             # HTML templates (Askama or MiniJinja) + static assets
      templates/
      static/
    config/
      mod.rs              # Service configuration (endpoint addresses, Tailscale)
  Cargo.toml
  DESIGN.md               # This file
```

### 4.2 Service Integration Points

| Service | Integration | Protocol |
|---------|------------|---------|
| Wheelhouse Hub | Job queue depth, active agents, recent resolutions, command dispatch | HTTP (local) |
| secure-gateway (:8000) | Episteme, Cerebro, DB, Memory state | HTTP (local) |
| secure-gateway (:8001) | Vault status, identity registry | HTTP (local) |
| n8n / analog-communications | Workflow status, recent ingestion events | HTTP (n8n API or Cloudflare Tunnel) |
| Ollama | Model list, active model state | HTTP (Ollama API) |
| LiteLLM | Proxy health, route table, usage stats | HTTP (LiteLLM API) |
| System logs | Structured log files across services | File (on-disk read) |

Admin-interface does not call Telnyx, Cloudflare, or any external service directly. External service status is proxied through the appropriate internal service.

---

## 5. Authentication

### 5.1 Model

Auth is YubiKey FIDO2/WebAuthn, consistent with secure-gateway. This is not a new auth system — it is the same hardware-bound root auth used throughout the system.

Session flow:
1. Operator navigates to admin-interface (Tailscale-gated or LAN)
2. FIDO2 challenge issued
3. YubiKey responds to challenge
4. Session token issued (short TTL, e.g. 8 hours)
5. All subsequent requests validate session token
6. Vault re-lock behavior: if YubiKey is removed, active sessions are invalidated on next request

### 5.2 No Fallback

There is no password fallback. There is no "forgot YubiKey" path. If the YubiKey is unavailable, admin access requires physical presence at the server. This is the correct threat model for a single-operator personal system.

### 5.3 Libraries

Consistent with the rest of the system: `python-fido2` is used in the Python services; the Rust equivalent for Axum is `webauthn-rs` crate.

---

## 6. Health Dashboard

The health dashboard is the default view. It aggregates the status of all Panorama services and presents them as a single-screen overview.

### 6.1 Required Panels

| Panel | Data Source | Refresh |
|-------|------------|---------|
| Service status grid | Polls each service's `/health` endpoint | 30s |
| Wheelhouse job queue | Wheelhouse Hub API | 10s |
| Active agent count | Wheelhouse Hub API | 10s |
| Recent ResolutionCodes | Wheelhouse Hub API | 30s |
| analog-communications ingestion count | n8n or analog-comms API | 60s |
| Model layer status | Ollama API + LiteLLM API | 60s |
| Secure drive status | secure-gateway `/health` | 30s |
| Recent errors / alerts | Aggregated log scan | 60s |

### 6.2 Status Conventions

Services report one of: `HEALTHY`, `DEGRADED`, `UNREACHABLE`, `UNKNOWN`. Admin-interface does not infer status — it reports what each service returns. If a service does not respond, it is `UNREACHABLE`, not `HEALTHY`.

---

## 7. Wheelhouse Monitor

A dedicated view for Wheelhouse operational state.

**Required:**
- Job queue: pending jobs, job IDs, creation time, current tier
- Active agents: agent IDs, assigned task, elapsed time, tier
- Recent completions: ResolutionCode, AgentFate, task summary, timestamp
- Plate allocation: current VRAM plate assignments, active models per plate
- Foreman corpus: recent crystallization events, corpus size, last mining run

**Commands dispatched from this view:**
- Pause/resume job queue
- Cancel specific job (with confirmation)
- Force agent retirement (with confirmation)
- Trigger Foreman mining run

---

## 8. Identity Manager

View and manage the number registry and identity levels for `analog-communications`.

**Required:**
- Number registry table: E.164, assigned level, status (active/quarantine/suspended), registration date
- Add number with level assignment
- Change level for existing number
- Move to quarantine / suspension
- View onboarding state for numbers in progress

---

## 9. Log Viewer

Structured log access across all services.

**Required:**
- Service selector (Wheelhouse, analog-comms, secure-gateway, n8n, etc.)
- Time range filter
- Level filter (ERROR, WARN, INFO, DEBUG)
- Free-text search
- Raw log line display

The log viewer reads from on-disk log files. It does not require a separate log aggregation service for the initial implementation.

---

## 10. Configuration Management

Read-only view of current configuration state, with template-apply capability via Panorama.

**Required:**
- View current config for each service (redacted secrets)
- Diff view: compare current config against Panorama template
- Apply: trigger Panorama to regenerate config from template and restart affected service (with confirmation)

Configuration writes always go through Panorama, not through direct file edits from this interface.

---

## 11. Open Items

| # | Item | Blocking |
|---|------|---------|
| 1 | Resolve frontend model decision (Section 3) | All implementation |
| 2 | Design YubiKey FIDO2 session flow in Rust (`webauthn-rs`) | Auth implementation |
| 3 | Define Wheelhouse Hub API contract (endpoints admin-interface will call) | Wheelhouse Hub implementation |
| 4 | Define secure-gateway admin routes (if any are needed beyond existing `:8000`) | secure-gateway |
| 5 | Tailscale deployment (required for cross-device access) | Tailscale rollout |
| 6 | HTTPS on tailnet for web UI option | Tailscale TLS configuration |
| 7 | Define log file locations and format conventions across all services | Each service independently |

---

*— end of document —*
