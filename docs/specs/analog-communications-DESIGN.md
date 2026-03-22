# analog-communications — Design Specification

**Project:** Panorama / fail.academy · Flickersong · L. Casinelli Jr.
**Status:** EMPTY — design complete, implementation pending
**Version:** 0.1
**Last Updated:** 2026-03-22

> **Patching note:** Use `str_replace` against specific sections. Do not regenerate in full. Update version and date on every patch.

---

## 1. Purpose and Scope

`analog-communications` owns all interfaces between the Panorama system and analog or telephony-based communication channels. It is the system's primary inbound surface for unstructured human input — the place where words spoken into a phone or tapped on a screen enter the structured world of the fail.academy data plane.

**What belongs here:**
- Telnyx webhook receiver and message dispatch
- IDEA pipeline (SMS-to-structured-data) — previously scoped as a standalone `idea-pipeline` repo; that framing is retired
- Voice ingestion and transcription pipeline (future)
- Any future analog channel (voicemail, fax, etc.)
- Identity/access model for external callers and senders
- Payload sanitization boundary (the system's untrusted-content ingress gate)

**What does not belong here:**
- Agent orchestration — that is Wheelhouse
- Data storage — that is `secure-gateway` and the data plane
- Administrative control — that is `admin-interface`
- Model inference — that is the model layer

---

## 2. Design Axioms (Local)

In addition to the Panorama-wide axioms, the following constraints apply specifically to this repo:

- **This is the untrusted boundary.** Every byte entering through this service is assumed hostile until sanitized. No raw inbound content may reach Wheelhouse or the data plane without passing through the sanitization stage.
- **Telnyx is a dependency, not the architecture.** The internal processing pipeline must be decoupled from Telnyx specifics. If Telnyx is replaced, only the inbound adapter changes.
- **n8n is the current runtime; Rust is the target.** n8n handles the workflow logic today. A Rust-native replacement is feasible and planned for when operational cost or capability limits demand it. The design should not assume n8n will always be present.
- **Scope limits on triggers are explicit and documented.** What an inbound SMS can cause to happen in Wheelhouse must be enumerated and enforced. "It triggers whatever the workflow says" is not a sufficient design.

---

## 3. Architecture

### 3.1 Ingress Flow

```
Telnyx TFN (833-433-2269)
        │
        │  HTTPS webhook (POST /sms-inbound)
        ▼
Cloudflare Tunnel (ideabox-tunnel, ID: 12bfa40c)
        │
        │  HTTP/HTTPS only — raw TCP not supported
        ▼
n8n (Docker) at https://n8n.fail.academy
  /opt/idea/ on OS drive
        │
        ├── Sanitization node
        ├── Identity resolution node
        ├── Label extraction node
        ├── Structured data construction node
        └── Output routing node
                │
                ├──→ Cerebro intake (write)
                ├──→ Episteme (skill suggestion candidates)
                └──→ Wheelhouse Hub (scoped task triggers only)
```

### 3.2 Current Runtime

| Component | Technology | Location |
|-----------|-----------|----------|
| Workflow engine | n8n (Docker) | `/opt/idea/` on OS drive |
| Public endpoint | Cloudflare Tunnel | `n8n.fail.academy` |
| TFN | Telnyx (833-433-2269) | Carrier verification pending |
| Webhook auth | Telnyx Ed25519 signature verification | Signed string: `timestamp|payload` |

### 3.3 Target Runtime (Rust-Native)

When n8n is retired, the replacement is a Rust service with the following shape:

```
analog-communications/
  src/
    main.rs               # Axum HTTP server, mounts all routes
    inbound/
      telnyx.rs           # Telnyx webhook adapter (Ed25519 verification)
      voice.rs            # STUB — voice/transcription adapter
    sanitization/
      mod.rs              # Sanitization pipeline — replay prevention, length limits,
                          # control character stripping, E.164 validation, label allowlist
    identity/
      mod.rs              # Identity resolution — number registry, level assignment,
                          # quarantine, onboarding state
    pipeline/
      mod.rs              # Processing pipeline — label extraction, structured data construction
      labels/             # Per-label processing logic
    dispatch/
      mod.rs              # Output routing — Cerebro, Episteme, Wheelhouse Hub
      scope.rs            # Explicit allowed-trigger registry for Wheelhouse
    config/
      mod.rs              # Service configuration
  Cargo.toml
  DESIGN.md               # This file
```

---

## 4. Sanitization Boundary

This is the most critical component in the repo. All inbound content passes through sanitization before anything else. Sanitization is a discrete, independently testable module — not inline logic.

### 4.1 Required Checks

| Check | Description |
|-------|-------------|
| **Replay prevention** | Reject requests with timestamps outside a 5-minute window |
| **Signature verification** | Validate Telnyx Ed25519 signature (`telnyx-signature-ed25519` + `telnyx-timestamp` headers). Signed string: `timestamp\|payload`. |
| **Input length limits** | Enforce maximum message length. Reject oversized payloads before any processing. |
| **Control character stripping** | Strip non-printable characters. Log occurrences. |
| **E.164 validation** | Reject malformed phone numbers. The `from` field must be valid E.164. |
| **Label allowlist enforcement** | Only recognized labels pass. Unknown labels are quarantined, not forwarded. |

### 4.2 What Sanitization Does Not Do

Sanitization is not a semantic filter. It does not decide whether content is "safe" or "appropriate" — that is a scope limit enforced at the dispatch layer (Section 6). Sanitization's job is structural and format correctness only.

---

## 5. Identity Model

### 5.1 Identity Levels

| Level | Name | Description |
|-------|------|-------------|
| 0 | Owner/Admin | L. Casinelli Jr. Sole operator. Full capability ceiling. TOTP session authentication for privileged commands. |
| 1 | Testing | Internal test numbers. Used for workflow development. |
| 2 | Verified | Approved external users. Scoped to own namespace. No system access. |
| 3 | Restricted | Minimal access. Quarantine candidate. |

### 5.2 Identity Resolution Flow

```
Inbound number (E.164)
    │
    ├── Known number? → Lookup level from registry
    │
    ├── Completing onboarding? → Onboarding state machine
    │
    ├── Unknown number → Quarantine (zero response, suspension_log entry)
    │
    └── Suspended → No response (abuse log entry only)
```

### 5.3 Privilege Policy

Level ceiling policy files define the capability boundary for each identity level. These are managed by Cloak and read at service startup. Any capability request above the level ceiling is silently clamped — never an error, never forwarded.

Per-service granularity (e.g., `ideabox.capture.sms`, `cerebro.read_own`) is a planned extension to the current flat capability model.

### 5.4 Owner Command Authentication

Owner-level commands require TOTP session authentication:
- Trigger: `/auth XXXXXX` (TOTP code)
- Session TTL: 30 minutes
- Drift tolerance: ±30 seconds
- Lockout: 5 failed attempts → email alert to `director@fail.academy`
- Authenticator: Microsoft Authenticator (TOTP)

---

## 6. Dispatch Scope

The dispatch layer is responsible for enforcing scope limits on what inbound content can cause. This is the second security gate after sanitization.

### 6.1 Allowed Output Targets

| Target | Allowed Actions | Notes |
|--------|----------------|-------|
| Cerebro intake | Write structured entry | Always allowed for verified+ identities. Content is externally-supplied — treated as untrusted by Cerebro's sync layer. |
| Episteme | Write skill candidate | Requires explicit label. Candidates are queued for Foreman review, not directly written to the live library. |
| Wheelhouse Hub | Scoped task triggers only | See Section 6.2. |

### 6.2 Wheelhouse Trigger Scope

What an inbound SMS is permitted to trigger in Wheelhouse must be an explicit, enumerated allowlist. The current defined scope:

- `IDEA_CAPTURE` — Store structured idea entry. No agent action required.
- (All other triggers are UNDEFINED and blocked until explicitly added to this list.)

The allowlist lives in `dispatch/scope.rs` (Rust target) or as a config file in the n8n runtime. It is not embedded in workflow logic — it is a first-class data structure that can be audited independently.

---

## 7. IDEA Pipeline — Current State

The IDEA pipeline is the first and currently only workflow within `analog-communications`. It converts inbound SMS into structured data entries.

| Field | Value |
|-------|-------|
| **Inbound** | Telnyx TFN (833-433-2269) via n8n webhook |
| **Processing** | Label extraction (`/label` prefix regex), identity resolution, sanitization, structured data construction |
| **Output** | SQLite (dual-write: structured + raw markdown), Cerebro intake |
| **Status** | Partially built. TFN carrier verification pending. Inbound SMS confirmation pending TFN approval. |
| **Remaining work** | Complete workflow once TFN verified. Test end-to-end. Define Wheelhouse trigger scope. |

### 7.1 Label System

Labels route messages to the appropriate processing path. The current active label set:

| Label | Active | Phase |
|-------|--------|-------|
| `idea` | Yes | 1 |
| `todo` | No | 2 |
| `research` | No | 2 |
| `project` | No | 2 |

Label configuration documents live in `/opt/idea/config/labels/`. Each label is a YAML file defining storage schema, classification hints, and agent consumption parameters.

---

## 8. Voice Ingestion (Future)

Voice ingestion is entirely undesigned. The following is a placeholder for the eventual design.

**Known requirements:**
- Telnyx supports voice calls on TFNs
- Voicemail transcription or real-time call transcription must produce text output that the existing text pipeline can consume
- The voice path must pass through the same sanitization and identity resolution pipeline as SMS

**Open questions:**
- Transcription provider: local (Whisper) vs. Telnyx-native vs. third-party
- Latency requirements: real-time vs. async (voicemail-style)
- Authentication: how does the voice path authenticate against the identity model?

---

## 9. n8n Configuration Management

Until the Rust-native replacement is built, n8n workflow configuration must be version-controlled.

**Required:**
- All n8n workflows exported as JSON and committed to this repo under `n8n-workflows/`
- A restore procedure documented in `n8n-workflows/RESTORE.md`
- Panorama bootstrap includes n8n workflow import step

**Current gap:** n8n workflows exist in the live Docker instance but are not yet committed to this repo. This is a data loss risk — the n8n configuration is currently not recoverable from the repo alone.

---

## 10. Open Items

| # | Item | Blocking |
|---|------|---------|
| 1 | Confirm inbound SMS flow once TFN carrier approval completes | TFN approval |
| 2 | Export n8n workflows to repo (data loss risk until done) | Nothing |
| 3 | Define explicit Wheelhouse trigger scope allowlist | Wheelhouse Hub API design |
| 4 | Design voice ingestion pipeline | Nothing (future phase) |
| 5 | Rust-native rewrite design | n8n operational cost threshold |
| 6 | Per-service capability granularity (`ideabox.capture.sms`, etc.) | Flat capability model stability |
| 7 | Formalize provider-exhaustion states (Telnyx 429s) as explicit dispatch errors | response-codes system |

---

*— end of document —*
