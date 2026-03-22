# PANORAMA — System Master Plan

**fail.academy · Flickersong · L. Casinelli Jr.**
Revision 0.1 — For Agent and Human Consumption

---

## 1. Purpose of This Document

This document is the canonical meta-structural reference for the Panorama monorepo and the broader fail.academy system. It is written to be machine-readable by agents and human-readable by developers. Any agent operating within this system should be able to read this document and determine: what the final system looks like, which repos exist, what each one does, how they connect, which pieces are complete versus missing, and what needs to change to reach the target state.

Panorama is not an application. It is a bootstrap and integration harness — a meta-repo that knows how to download, configure, connect, and verify all constituent services. When Panorama runs on a fresh machine, it produces a fully-operational fail.academy system. Nothing in the final state should require manual configuration that Panorama does not encode.

---

## 2. Design Axioms

These principles are non-negotiable constraints on all architectural decisions. Any design that violates one of these requires explicit justification.

- **Hardware ownership first.** No hyperscaler lock-in. The system runs on owned hardware. Cloud is an emergency fallback or a transit path for inbound webhooks, never the primary runtime.
- **Rust as the primary language.** All new services are written in Rust unless there is a categorical technical blocker. Python and Node.js remain for tooling glue, n8n automation, and legacy compatibility only. The long-term vision is a fully Rust-native control plane.
- **Self-contained repos, orchestrated by Panorama.** Each constituent repo must be buildable and testable in isolation. Panorama supplies the wiring — it does not own the logic. Coupling is explicit and documented.
- **Conservative defaults everywhere.** Agent lifetimes are short and scoped. VRAM allocations are fixed plates, not dynamic. Permissions are narrowly granted. Fail loudly rather than silently.
- **Structural correctness before optimization.** Get the architecture right first. Performance tuning and premature optimization are deferred until the design is stable.
- **Security boundary is the Gateway, not the model.** No agent or model should be granted broader authority than needed for its specific task. The control plane enforces this; the model is untrusted within its task scope.
- **Schema and path conventions aid model reasoning.** Conventional OS paths (e.g., `/opt/idea/`, `/secure/`) and explicit schema definitions make it easier for agents to reason about the system topology.

---

## 3. System Map — Full Target State

```
╔══════════════════════════════════════════════════════╗
  EXTERNAL INGRESS LAYER
  Telnyx TFN (SMS) → Cloudflare Tunnel → n8n (Docker)
  Telnyx Webhook   → Cloudflare Tunnel → Wheelhouse Gateway
╚══════════════════════════════════════════════════════╝
                          ↓
╔══════════════════════════════════════════════════════╗
  WHEELHOUSE — Multi-Tier Agent Orchestration (Rust)
  Hub  →  Orchestrator  →  Specialist  →  Micro
  task-manager  |  agent-lifecycle-service  |  Foreman
╚══════════════════════════════════════════════════════╝
           ↓                          ↓
╔══════════════╗          ╔═══════════════════════════╗
  IDEA PIPELINE             EPISTEME / CEREBRO STORE
  n8n + Telnyx              FastAPI Gateway (:8000)
  SMS → Structured Data     /secure/ IronWolf 8TB
╚══════════════╝          ╚═══════════════════════════╝
                          ↓
╔══════════════════════════════════════════════════════╗
  MODEL LAYER
  Ollama / vLLM  |  LiteLLM Proxy  |  Model Registry
  NVLinked RTX 3090 x2 (48GB VRAM)  |  Plate Allocator
╚══════════════════════════════════════════════════════╝
```

---

## 4. Repo Inventory

Every repository in the ecosystem is listed below with its current status. An agent modifying this system should verify each repo's status against this table before taking action.

**Status legend:** `LIVE` = deployed and running · `DESIGNED` = architecture complete, implementation pending · `EMPTY` = repo exists, no implementation · `PLANNED` = not yet created · `EXTERNAL` = third-party or tooling not owned by this project.

| Repo / Service | Lang | Status | Purpose |
|---|---|---|---|
| `panorama` | Rust/TOML | **EMPTY** | Bootstrap & integration harness. Downloads, configures, and wires all services. This repo. |
| `wheelhouse` | Rust | **DESIGNED** | Multi-tier agent orchestration. Hub → Orchestrator → Specialist → Micro. Plate-based VRAM allocation, cascade routing, Foreman skill library. |
| `task-manager` | Rust | **EMPTY** | Atomic task lifecycle management. Tracks TaskBrief state machine, issues ResolutionCodes, feeds ExecutionArchive. |
| `agent-lifecycle-service` | Rust | **EMPTY** | Agent pool state machine. Controls AgentFate transitions (Spawn → Idle → Active → Retiring → Dead). Enforces conservative lifetime defaults. |
| `episteme` | Multi | **LIVE** | Skill and knowledge library. GitHub repo + `/secure/` working copy. Provides reusable capabilities to agents via FastAPI gateway. |
| `cerebro` | Multi | **PLANNED** | Personal knowledge graph. Dual sync layer: external write intake + internal agent-readable store. Obsidian Sync compatible. |
| `idea-pipeline` | n8n/Node | **LIVE** | SMS-to-structured-data capture. Telnyx TFN → Cloudflare Tunnel → n8n. Running at https://n8n.fail.academy. |
| `secure-gateway` | Python | **LIVE** | FastAPI data plane gateway on `:8000`. Routes `/episteme` `/cerebro` `/db` `/memory` `/health`. YubiKey FIDO2 root auth. |
| `model-registry` | YAML/JSON | **DESIGNED** | Canonical model ID registry. `CCC-FFF-MMMM-XXXB` fixed-width scheme. SHA-256 self-referential hash in markdown via `hash_ref`. |
| `response-codes` | YAML | **DESIGNED** | HTTP-analogous 3-digit external + 4-digit CCEE internal codes. `wheelhouse-response-codes.yaml` + `wheelhouse-internal-events.yaml`. |
| Ollama | External | **EXTERNAL** | Local model inference backend. Primary runtime for Specialist/Micro tiers. |
| LiteLLM | External | **EXTERNAL** | Proxy and governance shim. Round-robin routing, rate limiting, model aliasing. |
| Telnyx | External | **EXTERNAL** | SMS/TFN provider. TFN 833-433-2269 pending carrier verification. |
| Cloudflare Tunnel | External | **LIVE** | `ideabox-tunnel` (ID: `12bfa40c`). HTTP/HTTPS ingress only. Raw TCP/SMTP not supported. |
| Tailscale | External | **PLANNED** | Admin SSH replacement for `ssh.fail.academy` DDNS. Planned rollout to replace router port forwarding. |
| Mailcow | External | **PLANNED** | Self-hosted mail. Blocked on ISP port 25 / PTR record check. Relay VPS (~$4–6/mo) likely required for SMTP ingress. |

---

## 5. Wheelhouse Architecture

Wheelhouse is the core of the system. Everything else feeds into or is orchestrated by it.

### 5.1 Four-Tier Hierarchy

| Tier | Role |
|---|---|
| **Hub (Tier 1)** | Receives external requests. Routes to Orchestrators. Invokes external API-tier models (e.g., Claude) only when cascade routing determines local tiers are insufficient. Hub frugality is a formal invariant. |
| **Orchestrator (Tier 2)** | Decomposes Jobs into Task sequences. Manages Job lifecycle. Issues AgentBriefs to Specialists. Tracks `proof_chain` for verifiable completion. |
| **Specialist (Tier 3)** | Executes TaskBriefs against specific capability domains. Pulls skills from Episteme. Reports ResolutionCodes back to Orchestrator. |
| **Micro (Tier 4)** | Minimal, single-purpose agents. Maximum task isolation. Shortest permitted lifetime. No cross-task state. |

### 5.2 Key Data Structures (Designed — Not Yet Implemented)

- **`AgentBrief`** — Task assignment contract passed from Orchestrator to Specialist. Contains scope, deadline, resource ceiling, and allowed tool surface.
- **`TaskLifecycleService`** — State machine governing a TaskBrief from `Pending → Active → Complete | Failed | Expired`.
- **`AgentResolution` / `ResolutionCode` / `AgentFate`** — Formal enum types. ResolutionCode drives AgentFate transitions. Fate values: `Spawn`, `Idle`, `Active`, `Retiring`, `Dead`.
- **`ExecutionArchive` / `RefinementCorpus`** — Separate stores. Archive = immutable execution records. Corpus = curated refinement data for Foreman mining.
- **`proof_chain: Vec<Option<Box<dyn SuccessToken>>>`** — On Job structs. `is_provably_complete()` gates Job completion. Verifiable execution record.
- **Plate (VRAM allocation)** — Fixed-width VRAM configuration. Swaps as a complete unit. Active parameters (not total) are the relevant metric, especially for MoE models like DeepSeek-R1.

### 5.3 Constituent Repos

- **`wheelhouse`** — Core orchestration engine. Contains Hub, Orchestrator, Foreman, cascade router, plate allocator.
- **`task-manager`** — Owns TaskBrief state machine. Should be a separate crate within the Wheelhouse workspace or a standalone service called via IPC/local socket. Decision pending.
- **`agent-lifecycle-service`** — Owns AgentFate state machine and agent pool. Same coupling question as task-manager — internal crate vs. sidecar service. Prefer internal Rust crate unless there is a clear operational reason to split.

### 5.4 Open Implementation Items (as of this revision)

Approximately 22 open items exist. The most structurally significant:

- Implement `AgentBrief` struct and serialization (serde)
- Implement `TaskLifecycleService` state machine
- Implement `AgentResolution`, `ResolutionCode`, `AgentFate` enums
- Implement `ExecutionArchive` and `RefinementCorpus` separation
- Implement Foreman skill mining pipeline (reads from SQLite archive)
- Define `Plate` struct and VRAM allocation logic
- Implement cascade routing with Hub frugality invariant
- Integrate LiteLLM proxy as Tier 3/4 inference backend
- Wire `response-codes` YAML into Rust error/result types
- Implement `proof_chain` and `is_provably_complete()`
- Formalize 429/timeout provider-exhaustion states as explicit external codes
- Crystallization quality gate for Foreman library (open research problem)

---

## 6. IDEA Pipeline

The IDEA pipeline is the primary external intake surface. It converts unstructured SMS voice-to-text into structured data that feeds Cerebro and/or triggers Wheelhouse workflows.

| Field | Value |
|---|---|
| **Ingress** | Telnyx TFN (833-433-2269). Inbound SMS. Carrier verification pending. |
| **Transport** | Telnyx webhook → Cloudflare Tunnel (`ideabox-tunnel`) → n8n on Docker at https://n8n.fail.academy |
| **Processing** | n8n workflow: parse SMS body, extract structured fields, route to appropriate store or trigger. |
| **Output targets** | Cerebro (knowledge graph), Episteme (skill suggestions), Wheelhouse (task triggers) |
| **Security note** | This is an untrusted-content ingestion surface. Telnyx/n8n webhook path must have explicit scope limits. The model is not the security boundary. |
| **Remaining work** | Confirm inbound SMS flow once TFN carrier approval completes. Design explicit scope for what IDEA-ingested content can trigger. |

---

## 7. Data Plane — Secure Gateway & Storage

### 7.1 Secure Drive

| Field | Value |
|---|---|
| **Hardware** | IronWolf Pro 8TB at `/secure/` |
| **Gateway** | FastAPI on `:8000`. Routes: `/episteme`, `/cerebro`, `/db`, `/memory`, `/health` |
| **Peer vault** | FastAPI on `:8001` |
| **Auth** | YubiKey FIDO2/WebAuthn. Vault re-locks on key removal. `python-fido2`, `yubikey-manager`, `pcscd`. |

### 7.2 Episteme

Episteme is the skill and knowledge library. It lives as a GitHub repo with a working copy synced to `/secure/`. Agents pull capabilities from it via the FastAPI gateway. Automated GitHub backup via deploy key + systemd timer is a pending to-do.

### 7.3 Cerebro

Cerebro is the personal knowledge graph. The sync layer design — between the external write intake (IDEA pipeline output) and the internal agent-readable store — is an open design problem. The two stores have different consistency requirements: the write intake can be eventually consistent; the agent-readable store should be strongly consistent at query time.

### 7.4 Vector Storage / RAG

Postgres + pgvector is the recommended RAG backend for self-hosted context. CockroachDB was evaluated and rejected — identical capability with higher operational complexity for a single-node deployment.

---

## 8. Model Layer

| Field | Value |
|---|---|
| **Current hardware** | Intel Core 2 Quad Q6600 — no AVX. Severely limited. Used for tooling and orchestration, not inference. |
| **Target hardware** | AMD Threadripper Pro + NVLinked RTX 3090 x2 = 48GB unified VRAM. Build not yet complete. |
| **Inference runtime** | Ollama (primary). vLLM with `--tensor-parallel-size 2 --enforce-eager --dtype bfloat16` for the 3090 NVLink setup. |
| **Proxy/governance** | LiteLLM — round-robin routing, rate limiting, model aliasing, governance shim. |
| **Stop-gap** | Vast.ai for large-model testing until 3090 build is complete. |
| **Model registry** | `CCC-FFF-MMMM-XXXB` canonical IDs. Active parameters (not total) are the VRAM-relevant metric. |
| **Trusted GGUF sources** | bartowski, unsloth, ggml-org on Hugging Face. |

---

## 9. Networking

| Field | Value |
|---|---|
| **Domain** | `fail.academy` on Cloudflare |
| **Public ingress** | Cloudflare Tunnel (`ideabox-tunnel`). HTTP/HTTPS only. Telnyx webhooks and n8n. Cannot proxy raw TCP/SMTP. |
| **Admin SSH** | Currently `ssh.fail.academy` DDNS + router port forwarding. Replacing with Tailscale. |
| **Home A record** | `home.fail.academy` — non-proxied A record. DDNS via Cloudflare API + cron script. |
| **Tailscale** | Planned. Removes the DDNS/port-forward dependency for admin access. External services (Telnyx) cannot join tailnet — tunnel remains for that path. |
| **Mail** | Migrating off Google Workspace. Mailcow recommended; Stalwart noted as modern alternative. Blocked on ISP port 25 / PTR availability. Relay VPS likely needed. |
| **Firewall** | UFW |

---

## 10. Panorama Bootstrap Design

Panorama is written in Rust. Its job is to bring a fresh machine to a fully-operational fail.academy system state. It should be the only thing a new operator needs to run.

### 10.1 Responsibilities

- Clone or update all constituent repos to their canonical paths
- Check system prerequisites (Rust toolchain, Docker, Ollama, Tailscale, etc.) and report missing dependencies
- Apply configuration templates for each service (Cloudflare Tunnel, n8n, FastAPI gateway, LiteLLM)
- Wire inter-service connections (API keys, endpoint URLs, Tailscale addresses)
- Run health checks against each live service and report system status
- Provide a diff-based update path: detect what has changed since last bootstrap and apply only the delta

### 10.2 Non-Responsibilities

- Panorama does not own business logic. It calls into constituent repos.
- Panorama does not manage runtime state. Services manage their own state.
- Panorama does not replace service-level config files. It generates them from templates.

### 10.3 Suggested Internal Structure

```
panorama/
  Cargo.toml                # workspace root
  panorama-cli/             # main binary — setup, status, update commands
  panorama-config/          # config schema, template engine, validation
  panorama-health/          # health check runners per service
  panorama-bootstrap/       # repo clone/update, prereq checks
  templates/                # per-service config templates (TOML/YAML/JSON)
  docs/                     # this document and related specs
  scripts/                  # shell glue for non-Rust tooling (n8n, Docker)
```

### 10.4 What Is Currently Missing

> ⚠️ **EMPTY REPOS — Immediate Action Required**
> - `panorama`: No implementation. Start with `panorama-cli` skeleton and config schema.
> - `task-manager`: No implementation. Begin with `TaskBrief` struct and state machine.
> - `agent-lifecycle-service`: No implementation. Begin with `AgentFate` enum and pool struct.

---

## 11. Rust Conversion Plan

The long-term target is a Rust-native control plane. The following table maps current non-Rust components to their Rust replacement target.

| Component | Current Lang | Priority | Notes |
|---|---|---|---|
| `secure-gateway` (FastAPI) | Python | **MEDIUM** | Axum is the natural replacement. Blocked on stabilizing auth interface with YubiKey. |
| n8n workflows (IDEA) | Node.js | **LOW** | n8n handles complex visual workflow logic. Replace only if operational cost becomes prohibitive. A Rust-native SMS ingestor is feasible long-term. |
| Panorama scripts | Shell/Python | **HIGH** | These should become Rust from the start. No legacy to migrate. |
| LiteLLM proxy | Python | **LOW** | External tool. Replace only if governance requirements outgrow it. A thin Rust proxy (e.g., using hyper) is feasible. |
| Model registry | YAML/JSON | **N/A** | Schema files are language-agnostic. Rust consumers read them via `serde_yaml` / `serde_json`. |
| Mailcow | PHP/Go | **NEVER** | Self-hosted mail server. Not a candidate for internal rewrite. Treat as external service. |

---

## 12. Open Problems

These are unresolved design or research questions. They are not implementation tasks — they require a decision before implementation can proceed.

- **Crystallization quality gate.** How does the Foreman validate that a crystallized skill is non-redundant and non-degrading? Silent library degradation over time is a real risk. No solution yet.
- **Cerebro sync layer consistency model.** The external write intake (IDEA output) and internal agent-readable store have different consistency requirements. The sync layer design — including conflict resolution and ordering — is an open problem.
- **`task-manager` coupling.** Should `task-manager` be an internal Cargo crate within the Wheelhouse workspace, or a standalone service on a local socket/IPC? Internal crate is simpler; standalone service is more independently deployable. No decision yet.
- **`agent-lifecycle-service` coupling.** Same question as `task-manager`. Same undecided status.
- **ISP port 25 / PTR record availability.** Mail migration to Mailcow is blocked on whether the ISP provides PTR record control and allows port 25. This determines whether a relay VPS is required or merely preferred.
- **Backblaze backup integration.** Noted as to-do. No design yet for what gets backed up, at what frequency, and how restore is validated.

---

## 13. Instructions for Agents Reading This Document

> ✅ **READ THIS SECTION FIRST if you are an agent.**

1. This document describes the **TARGET state**. Not all of it exists yet.
2. Before modifying any file, check **Section 4** (Repo Inventory) for the repo's current status.
3. **EMPTY** repos (`panorama`, `task-manager`, `agent-lifecycle-service`) need initial implementation, not modification.
4. **DESIGNED** repos (`wheelhouse`, `model-registry`, `response-codes`) have specifications — read them before writing code.
5. Do not introduce new external dependencies without checking the design axioms in **Section 2**.
6. All new code is Rust unless **Section 11** explicitly categorizes the component as non-Rust.
7. If you encounter a decision point not covered by this document, consult **Section 12** (Open Problems) first.
8. Update the Open Problems section when a problem is resolved. Update the Repo Inventory when a status changes.
9. The security boundary is the Gateway and control plane, not the model. Do not grant broader authority than required.
10. When in doubt about VRAM allocation, use **active parameters**, not total. MoE models report inflated totals.

---

## 14. Revision History

| Revision | Notes |
|---|---|
| 0.1 — Initial | First complete meta-structural draft. Covers all repos, architecture, open problems, agent instructions. Rust conversion plan included. |
| Next revision | Update when: any EMPTY repo gets initial implementation, any DESIGNED repo reaches LIVE, any Open Problem is resolved, or Panorama bootstrap design is finalized. |

---

*— end of document —*
