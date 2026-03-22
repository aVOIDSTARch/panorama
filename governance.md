# Wheelhouse / Flickersong Agentic AI Governance Checklist

> Mapped to Singapore IMDA Model AI Governance Framework for Agentic AI, v1.0 (22 January 2026)  
> System under review: **Wheelhouse** (Hub → Orchestrator → Specialist → Micro hierarchy)  
> Supporting systems: IDEA pipeline, Episteme, Cerebro, Secure Drive Gateway  
> Last updated: March 2026

---

## How to Use This Document

Each item maps to one of IMDA's four governance dimensions. Mark each item:

- `[x]` — Implemented  
- `[~]` — Partially implemented / in progress  
- `[ ]` — Not yet implemented  
- `[N/A]` — Not applicable to this deployment

Review this checklist before any new agent tier or tool is brought online, and after any significant architectural change.

---

## Autonomy Level Classification

Before deploying or modifying any agent, classify it against the IMDA autonomy scale:

| Level | Label | Description | Wheelhouse Tier |
|-------|-------|-------------|-----------------|
| L1 | Human-Led | Agent proposes, human executes | Hub (API-tier, conservative invocation) |
| L2 | Collaborative | Agent and human work jointly | Orchestrator with checkpoint gates |
| L3 | Supervised | Agent operates, human approves at checkpoints | Specialist agents with proof_chain gates |
| L4 | Autonomous | Agent operates, human observes | Micro agents (narrow, reversible scope only) |

**Rule:** Any agent operating at L3 or L4 requires explicit justification and active entries in Sections 1 and 3 of this checklist before deployment.

---

## Dimension 01 — Assess & Bound the Risks Upfront

### Action-Space Mapping

- [ ] Every agent tier has a documented action-space: tools available, databases accessible, read vs. write permissions enumerated
- [ ] IDEA pipeline inbound SMS surface is classified as **untrusted-content ingestion** and its downstream action-space is explicitly scoped
- [ ] Cerebro write-intake path and Episteme skill library are treated as distinct trust surfaces with separate access grants

### Reversibility Assessment

- [ ] All agent actions are classified as reversible or irreversible before deployment
- [ ] Irreversible actions (file deletion, external API calls with side effects, payment triggers) are blocked from Micro and Specialist tiers without explicit Orchestrator authorization
- [ ] `proof_chain: Vec<Option<Box<dyn SuccessToken>>>` entries include a reversibility annotation for each completed task

### Least-Privilege Access

- [ ] Each agent plate configuration grants only the tools and data paths required for its assigned task class — no ambient broad access
- [ ] Secure Drive Gateway (`:8000`, `:8001`) enforces per-route authorization; agents do not receive root gateway credentials
- [ ] YubiKey FIDO2 hardware-bound auth governs vault access; no agent tier can re-lock or unlock the vault autonomously
- [ ] Scoped API keys and per-agent identity tokens are used where external services (Telnyx, Cloudflare) are involved
- [ ] LiteLLM proxy governance shim enforces model-access policy per tier — no tier directly calls a model outside its sanctioned route

### Autonomy Bounding via SOPs

- [ ] Hub cascade routing rules are documented and conservative by default (costly API-tier models invoked only when genuinely necessary)
- [ ] Orchestrator has defined escalation conditions that route control back to the Hub rather than proceeding
- [ ] Agent lifetimes are set to conservative defaults; no agent runs indefinitely without a re-authorization checkpoint
- [ ] Crystallization quality gate for the Foreman skill library is defined; criteria for silent library degradation detection are documented

### Identity & Access Controls

- [ ] Each active agent has a unique identity token for the duration of its task
- [ ] Agent identity tokens are scoped to the task, not the session or system lifetime
- [ ] Model ID registry (`CCC-FFF-MMMM-XXXB` canonical scheme) is current; no unregistered models are running in production plates

---

## Dimension 02 — Make Humans Meaningfully Accountable

### Responsibility Allocation

- [ ] A single accountable owner (Louie / Flickersong) is named for each agent tier's operational scope
- [ ] Responsibility boundaries between Wheelhouse tiers are documented: what the Hub owns vs. Orchestrator vs. Specialist
- [ ] External vendor obligations (Telnyx, Cloudflare, Ollama model providers) are reviewed for security feature availability (tool call logging, scoped keys)

### Checkpoints for High-Stakes Actions

- [ ] `is_provably_complete()` gate is enforced before any Job is marked done — no silent completion
- [ ] Checkpoints are defined for any action that: writes to Episteme, modifies Cerebro, triggers external HTTP calls, or alters plate configurations
- [ ] IDEA pipeline SMS-to-structured-data path has a defined human review checkpoint before any structured output is acted upon downstream

### Automation Bias Mitigation

- [ ] Regular review cadence is scheduled for agent outputs; no output stream is treated as inherently correct without periodic spot-checks
- [ ] `ExecutionArchive` and `RefinementCorpus` are reviewed periodically for systematic error patterns — not just individual task failures
- [ ] Foreman mining of the SQLite indexed archive is audited for skill crystallization quality drift

### Adaptive Governance

- [ ] This document is treated as a living document; a review is triggered by: new agent tier deployment, new tool grant, new external integration, or any observed cascading failure event
- [ ] Provider-exhaustion states (429s, timeouts) are logged as explicit governance events, not silent retries

---

## Dimension 03 — Implement Technical Controls & Processes

### Guardrails

- [ ] Planning and reasoning outputs from Orchestrator and Specialist tiers are validated against task scope before tool execution — no unrestricted plan execution
- [ ] MCP servers, if used, are explicitly whitelisted; no agent can invoke an unlisted MCP endpoint
- [ ] Code execution by agents is sandboxed; agents cannot execute arbitrary shell commands on the host system
- [ ] n8n webhook path (Telnyx → `https://n8n.fail.academy` via Cloudflare Tunnel) is treated as an untrusted ingress surface; payload sanitization is enforced before any downstream action

### Testing & Compliance

- [ ] Task execution accuracy is tested for each new Specialist or Micro agent before promotion to active plate
- [ ] Policy compliance tests cover: scope adherence, least-privilege enforcement, reversibility classification, and proof_chain integrity
- [ ] Episteme skill library entries are tested for non-redundancy and quality before crystallization

### Rollout Controls

- [ ] New agent capabilities are rolled out incrementally, not system-wide at once
- [ ] Rollback procedure is documented and tested for each new plate configuration

### Logging & Alerting

- [ ] All agent actions are logged with sufficient fidelity to reconstruct task execution post-hoc (`ExecutionArchive`)
- [ ] Alert thresholds are defined for: repeated task failures, unusual tool invocation patterns, proof_chain gaps, and provider exhaustion events
- [ ] Logs are stored on the Secure Drive and are not accessible to agent tiers that generated them (no self-modifying audit trail)
- [ ] Backblaze backup integration covers agent execution logs (noted dependency, implementation pending)

---

## Dimension 04 — Enable End-User Responsibility

> For a personal/sole-proprietorship deployment, "end user" is Louie himself in non-agent contexts — e.g., when reviewing IDEA pipeline output or consuming Cerebro knowledge graph entries.

### Transparency of Agent Capabilities

- [ ] A current summary of active agent capabilities and data access is maintained and readable without consulting source code
- [ ] Any external-facing surface (IDEA SMS intake, future web app) declares upfront that it involves automated agent processing

### Failure Mode Awareness

- [ ] Common failure modes for each tier are documented: erroneous actions, unauthorized scope creep, hallucinated planning, prompt injection via untrusted input, rogue tool use
- [ ] The IDEA pipeline's SMS ingestion path is specifically documented as a prompt-injection risk surface; mitigations are enumerated

### Escalation Paths

- [ ] A defined escalation path exists for any agent failure: what halts, what logs, what requires manual intervention
- [ ] `AgentFate` enum values map to concrete operational responses (not just internal state transitions)
- [ ] `ResolutionCode` / `AgentResolution` failure states trigger appropriate logging and human notification, not silent retry loops

---

## Systemic Threat Register

The following threats are identified in the IMDA framework as specific to agentic systems. Each must have a documented mitigation before L3/L4 deployment.

| Threat | Mitigation Approach | Status |
|--------|---------------------|--------|
| **Cascading failures** | Conservative agent lifetime defaults; Hub frugality rules; proof_chain integrity gates | `[ ]` |
| **Unpredictable outcomes** | Scope bounding at plate level; reversibility classification; incremental rollout | `[ ]` |
| **Hallucinated planning** | Orchestrator plan validation before tool execution; RefinementCorpus for replay analysis | `[ ]` |
| **Prompt injection** | IDEA/SMS path treated as untrusted surface; explicit payload sanitization; restricted downstream action-space | `[ ]` |
| **Rogue tool use** | Explicit tool whitelist per agent tier; MCP server whitelist; no ambient broad tool grants | `[ ]` |

---

## Open Implementation Items Blocking Governance Compliance

The following items from the Wheelhouse open implementation list have direct governance implications and should be prioritized:

1. **Provider-exhaustion formal codes** — 429s and timeouts as explicit `ResolutionCode` entries (blocks Dimension 01 reversibility classification and Dimension 02 checkpoint design)
2. **Foreman crystallization quality gate** — Without this, silent library degradation is undetectable (blocks Dimension 03 testing compliance)
3. **Backblaze log backup** — Audit trail durability is unguaranteed without offsite backup (blocks Dimension 03 logging compliance)
4. **Cerebro sync layer scope design** — Write-intake and agent-readable store are not yet formally separated trust surfaces (blocks Dimension 01 action-space mapping)
5. **IDEA pipeline payload sanitization** — Prompt injection mitigation for SMS ingestion is not yet formally implemented (blocks Systemic Threat Register: Prompt Injection)

---

## Review Log

| Date | Reviewer | Scope | Notes |
|------|----------|-------|-------|
| — | — | Initial creation | Pre-implementation baseline |

---

*Source: Model AI Governance Framework for Agentic AI, v1.0, IMDA Singapore, 22 January 2026.*  
*Framework summary: Nesibe Kırış Can, AIGP — techletter.co, February 2026.*
