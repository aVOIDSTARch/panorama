# Panorama — Master Plan
**Version:** 1.2  
**Entity:** Flickersong / fail.academy  
**Operator:** L. Casinelli Jr.  
**Last Updated:** 2026-03-22  
**Status:** Active development — pre-hardware-complete

---

## 1. System Identity

Panorama is the monorepo umbrella for the full fail.academy self-hosted AI infrastructure ecosystem. It is not a product. It is a personal-scale, hardware-owned, anti-cloud-dependency system for:

- Capturing and structuring ideas (IDEA pipeline / analog-communications)
- Orchestrating agentic AI work across locally-run models (Wheelhouse)
- Maintaining a reusable skill library (Episteme)
- Maintaining a personal knowledge graph (Cerebro)
- Exposing administrative control surfaces (admin-interface)
- Managing all external communications infrastructure (analog-communications)

All services are self-hosted or use purpose-justified external dependencies (Cloudflare Tunnel for public ingress, Telnyx for PSTN access). No hyperscaler dependency, no behavioral data leakage.

---

## 2. Repo Inventory

### 2.1 Active Repos

| Repo | Language | Status | Role |
|------|----------|--------|------|
| `panorama` | — | Active | Monorepo root, cross-cutting docs, shared specs |
| `wheelhouse` | Rust | Design complete, pre-implementation | Multi-tier agent orchestration engine |
| `task-manager` | Rust | Design complete, pre-implementation | Atomic task lifecycle sub-component of Wheelhouse |
| `episteme` | Markdown / mixed | Active (GitHub + `/secure/` working copy) | Reusable skill and knowledge library for agents |
| `analog-communications` | Python / n8n | Active (n8n live at n8n.fail.academy) | All PSTN/SMS/voice interfaces — Telnyx, TFN, n8n workflows, identity/quarantine system |
| `admin-interface` | HTML/JS / React | Pre-build | Web-based administrative control surface for the full system |

### 2.2 Planned / Referenced

| Component | Location | Notes |
|-----------|----------|-------|
| Cerebro | `/secure/cerebro/` | Knowledge graph — sync layer design pending |
| Secure Drive Gateway | `/secure/gateway/` | FastAPI on :8000, vault/registry on :8001 |
| Model ID Registry | `panorama` root | `model-id-registry.md` + `model.schema.json` — v1.0 live |
| Response Code Registry | `panorama` root | `wheelhouse-response-codes.yaml` + `wheelhouse-internal-events.yaml` — v1.0 live |
| LiteLLM Proxy | OS drive | Governance shim + round-robin routing — planned |
| Cloud Model Access Gateway | Wheelhouse sub-component | Design in progress |
| Governance Checklist | `panorama` docs | `governance.md` — mapped to IMDA Framework v1.0 |

---

## 3. Infrastructure Topology

```
EXTERNAL
  │
  ├── Telnyx TFN (833-433-2269) ──► Cloudflare Tunnel ──► n8n.fail.academy
  │     └── analog-communications: n8n workflows, quarantine, identity system
  │
  ├── flickersong.io ──────────────► Cloudflare Tunnel ──► nginx (home server)
  │     └── admin-interface served here at /admin/
  │
  └── fail.academy ──► Cloudflare (DNS, CDN)

HOME SERVER — OS Drive (Intel Core 2 Quad Q6600, Ubuntu)
  ├── /opt/idea/         n8n (Docker), cloudflared, nginx
  ├── Wheelhouse runtime (future — current server lacks AVX)
  └── Claude Code (npm path, AVX workaround active)

HOME SERVER — Secure Drive (IronWolf Pro 8TB, /secure/)
  ├── /secure/gateway/   FastAPI :8000 — sole data plane entry point
  ├── /secure/vault/     Peer vault + service registry :8001
  ├── /secure/episteme/  Working copy (GitHub = daily automated backup)
  ├── /secure/cerebro/   Knowledge graph (intake/ + store/)
  ├── /secure/db/        SQLite collections
  └── /secure/memory/    Agent memory store

TAILSCALE (planned)
  └── Replaces ssh.fail.academy DDNS for admin SSH; Telnyx cannot join tailnet

MODEL SERVER (planned — not yet built)
  ├── AMD Threadripper Pro
  ├── 2× RTX 3090 NVLinked (48GB unified VRAM)
  ├── vLLM (--tensor-parallel-size 2, --enforce-eager, --dtype bfloat16)
  └── Ollama (local inference backend)

INTERIM MODEL ACCESS
  ├── Claude API — Hub tier only, cascade routing, frugal invocation
  └── Vast.ai — large-model testing stop-gap pending build completion
```

---

## 4. Component Summaries

### 4.1 Wheelhouse
Multi-tier agent orchestration system. Four-tier hierarchy: Hub → Orchestrator → Specialist → Micro. Plate-based VRAM allocation. Cascade routing with formal optimality proof (Hub invokes API-tier models only when genuinely necessary). Foreman component manages crystallizable skill library.

Key design artifacts: `AgentBrief`, `TaskLifecycleService`, `AgentResolution` / `ResolutionCode` / `AgentFate`, `ExecutionArchive` / `RefinementCorpus` separation, Job/Task bifurcation, `proof_chain: Vec<Option<Box<dyn SuccessToken>>>`.

Design status: substantially complete. ~22 open implementation items. No code written yet.

### 4.2 Task Manager
Standalone sub-repo, part of Wheelhouse. Owns atomic task lifecycle from creation to archive commit. Two public entry points: `TaskLifecycleService.create()` and `TaskLifecycleService.teardown()`. Design spec complete in `task-manager.md`.

### 4.3 Episteme
Skill and knowledge library. GitHub is the remote backup (daily automated push via deploy key + systemd timer — not yet active). `/secure/episteme/` is the working copy. Agents access via secure drive gateway `:8000/episteme`.

### 4.4 Analog Communications
Owns everything that touches the PSTN or analog communication protocols. This is the formalized repo for what was previously called the "IDEA pipeline."

Scope: SMS intake, voice interface (future), TFN management, n8n workflows, Telnyx configuration, quarantine system, identity enrollment flow, operator SMS command interface, and the FastAPI endpoints that support number registry and identity management.

Current state: n8n live in Docker at `https://n8n.fail.academy`. Telnyx TFN (833-433-2269) in carrier verification. Test long-codes: 843-970-4758 and 843-883-8181. Cloudflare Tunnel (`ideabox-tunnel`, ID `12bfa40c`) active.

See `analog-communications.md` for full design specification.

### 4.5 Admin Interface
Web-based control surface for the full Panorama system. Served via nginx at `https://flickersong.io/admin/`. Consolidated visibility into: identity and privilege management, quarantine queue, system status and health, capture data browsing, and (eventually) Wheelhouse job visibility.

Current state: pre-build. Design specified.

See `admin-interface.md` for full design specification.

### 4.6 Cerebro
Personal knowledge graph. Dual-use node: receives writes from the IDEA pipeline via analog-communications, serves reads to Wheelhouse agents. A sync layer between external write intake (`/secure/cerebro/intake/`) and internal agent-readable store (`/secure/cerebro/store/`) enforces the trust boundary. Sync validation logic (`sync.py`) is a key open item.

---

## 5. Identity and Access Model

Four identity levels with ceiling-gated capabilities:

| Level | Label | Description |
|-------|-------|-------------|
| 0 | Owner/Admin | Sole operator (L. Casinelli Jr.) — full capability ceiling |
| 1 | Testing | Internal test identities — elevated ceiling, no production data |
| 2 | Verified | Approved external users — standard capability set |
| 3 | Restricted | Minimal access — capture only by default |

Capability model: master list of all system capabilities. Level ceiling controls visibility in privilege editor (above ceiling = invisible, not disabled). Per-identity toggle map: Allow / Deny / Inherit. Cloak stores policy files per level. All access logged.

Owner authentication for high-privilege SMS commands: TOTP session via `/auth XXXXXX` (Microsoft Authenticator), 30-minute TTL, ±30-second drift, 5-attempt lockout triggering email alert to `director@fail.academy`.

Per-service, per-capability granularity is a planned future extension of the current flat capability list.

---

## 6. Security Principles

- The model is not the security boundary — the Gateway and control plane are
- Telnyx/n8n webhook path is an untrusted-content ingestion surface with explicit scope enforcement
- Telnyx uses Ed25519 signature verification (signed string: `timestamp|payload`)
- Sanitization pipeline: replay prevention (5-minute timestamp window), length limits, control character stripping, E.164 format validation, label allowlist enforcement
- YubiKey FIDO2/WebAuthn provides hardware-bound root auth; vault re-locks on key removal
- Cloudflare Tunnel is HTTP/HTTPS only — raw TCP/SMTP requires separate ingress
- UFW governs all host-level port exposure

---

## 7. Open Items (System-Level)

Cross-cutting blockers and decisions affecting multiple components.

- [ ] Complete NVLinked 3090 model server build; Vast.ai stop-gap for large-model testing in interim
- [ ] Confirm inbound SMS flow once Telnyx TFN carrier approval completes
- [ ] Finalize Tailscale rollout; remove `ssh.fail.academy` DNS record + router port forwarding
- [ ] Migrate `fail.academy` email off Google Workspace to self-hosted (Mailcow preferred; Stalwart noted) — blocked on ISP port 25 / PTR record check; relay VPS (~$4–6/mo) likely required regardless
- [ ] Migrate Node/Python web app from VPS to home server via Cloudflare Tunnel
- [ ] Formalize provider-exhaustion states (429s, timeouts) as explicit codes in Wheelhouse external response system
- [ ] Backblaze backup integration
- [ ] Episteme GitHub automated backup (deploy key + systemd timer) — not yet active
- [ ] Cerebro sync layer design — intake → store validation and promotion logic (`sync.py`)
- [ ] Foreman crystallization quality gate and non-redundancy criterion — open research problem; silent library degradation is a real risk
- [ ] Wheelhouse-to-secure-drive authentication method — identity tokens vs. mutual TLS, decision pending

---

## 8. Technology Stack

| Layer | Technology |
|-------|------------|
| Primary language (Wheelhouse) | Rust |
| Automation / workflows | n8n (Docker) |
| Data gateway | FastAPI (Python) |
| Admin interface | HTML/JS or React (TBD) |
| Local inference | Ollama, llama.cpp |
| Inference server (planned) | vLLM |
| Proxy / governance shim | LiteLLM (planned) |
| Communications | Telnyx (SMS / TFN) |
| Tunneling | Cloudflare Tunnel |
| Admin mesh (planned) | Tailscale |
| Hardware auth | YubiKey FIDO2/WebAuthn |
| Vector / RAG | Postgres + pgvector |
| Archive DB | SQLite (WAL mode) |
| Secrets | Cloak |
| Domain | fail.academy (Cloudflare) |
| OS | Ubuntu (Linux) |
| Dev environment | zsh, Docker, Claude Code (npm path) |

---

## 9. Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | ~2026-03-09 | Initial architecture — Wheelhouse, IDEA pipeline, Episteme, Cerebro, secure drive topology |
| 1.1 | ~2026-03-21 | `task-manager` added as standalone repo; `governance.md` produced; cloud model access gateway design initiated; identity/access model formalized; response code and model ID registries live |
| 1.2 | 2026-03-22 | `admin-interface` repo added; `analog-communications` repo added (absorbs IDEA pipeline / Telnyx / n8n); topology and component summaries updated to reflect both |

---

*This is the canonical system-level orientation document for Panorama. Component-level detail lives in the respective repo design docs. Update via `str_replace` patch against specific sections — do not regenerate in full.*
