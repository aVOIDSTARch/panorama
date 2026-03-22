# task-manager

**Project:** Wheelhouse (sub-component, standalone repo)
**Status:** Design specification — pre-implementation
**Version:** 0.1.0
**Last Updated:** 2026-03-22

---

## Overview

`task-manager` is the atomic work-unit lifecycle layer of Wheelhouse. It owns everything
from the moment a Task is created by the orchestrator to the moment the execution record
is committed to the archive and the agent is released or terminated.

It does not orchestrate — that is the Hub and Orchestrator's domain. It does not route
or plan — the Job layer owns that. It is a pure lifecycle manager: it constructs the
context envelope for an agent, governs the agent's execution, and tears down cleanly
regardless of outcome.

The orchestrator calls exactly two public entry points:

```
TaskLifecycleService.create(task: TaskObject)   → AgentBrief
TaskLifecycleService.teardown(brief, result)    → AgentResolution
```

All internal complexity is encapsulated behind this interface. The orchestrator never
touches lifecycle internals.

---

## Position in Wheelhouse Architecture

```
Hub
 └── Orchestrator
      ├── JobQueue              ← Job objects live here
      │    └── Job
      │         └── Vec<Task>   ← Tasks dispatched from Job plans
      │
      └── task-manager          ← THIS COMPONENT
           ├── TaskLifecycleService
           ├── AgentBrief (constructed here)
           ├── Deconstructor
           └── ExecutionArchive / RefinementCorpus write
```

The Job/Task bifurcation is a hard architectural boundary:
- **Jobs** are high-level orchestration plans that await and dispatch Tasks. A Job owns
  its `proof_chain: Vec<Option<Box<dyn SuccessToken>>>` and is only complete when
  `is_provably_complete()` returns true.
- **Tasks** are atomic work units dispatched to a single agent for a single execution.
  `task-manager` owns the Task's entire lifespan.

---

## Core Types

### AgentBrief

The canonical context envelope constructed by `TaskLifecycleService` and delivered
to a spawned agent. Immutable once sealed. Carries a SHA-256 hash to detect tampering.

```
AgentBrief {
  // Identity
  brief_id:          UUIDv4
  correlation_id:    String        // trace thread: logs ↔ task record ↔ archive
  task_id:           String
  job_id:            String
  created_at:        ISO8601
  brief_hash:        String        // SHA-256; tamper detection

  // Agent targeting
  agent_tier:        AgentTier     // HUB | ORCHESTRATOR | SPECIALIST | MICRO
  model_id:          String        // Wheelhouse canonical ID (CCC-FFF-MMMM-XXXB)
  plate_id:          Option<String>// Wheelhouse plate config ID for local inference

  // Task definition
  task_object:       TaskObject
  success_condition: SuccessContract
  output_contract:   OutputContract
  retry_policy:      RetryPolicy

  // Memory and knowledge
  working_memory:    MemoryBlock
  skill_collection:  Vec<Skill>
  knowledge_refs:    Vec<KnowledgeRef>

  // Role and constraints
  role_def:          RoleDefinition
  constraints:       Vec<Constraint>
  credentials:       Vec<ScopedCredential> // revoked during teardown

  // Resource budget
  resource_budget:   ResourceBudget {
    max_tokens:         u64
    max_wall_clock_s:   u64
    token_warn_pct:     f32   // warn at this threshold (default 0.8)
    wall_clock_warn_pct f32
  }

  // Observability
  checkpoint_policy: CheckpointPolicy
  dep_manifest:      DependencyManifest  // blocked_by / blocks
}
```

**Model ID field:** `model_id` must reference a registered entry in the Wheelhouse
model ID registry (`CCC-FFF-MMMM-XXXB` scheme). The registry is the single source of
truth; unregistered models are rejected at `create()` time.

---

### TaskObject

The input the orchestrator hands to `TaskLifecycleService.create()`. Validated eagerly;
a malformed `TaskObject` throws `TaskValidationError` and never produces a brief.

```
TaskObject {
  task_id:          String        // UUID, must be unique
  job_id:           String        // parent job
  description:      String        // non-empty, human-readable task statement
  success_condition: SuccessContract
  output_contract:  OutputContract
  agent_tier:       AgentTier
  resource_budget:  ResourceBudget
  retry_policy:     RetryPolicy
  skill_hints:      Vec<String>   // capability tags for foreman skill resolution
  knowledge_hints:  Vec<String>   // context pointers for brief assembly
}
```

---

### SuccessContract and OutputContract

`SuccessContract` is the declarative criteria a completed execution must satisfy.
`OutputContract` specifies the shape of the valid completion artifact.

```
SuccessContract {
  criteria:         Vec<String>           // natural language criteria
  validation_mode:  ValidationMode        // AUTO | HUMAN | CONSENSUS | FOREMAN
  confidence_floor: f32                   // minimum auto-validation confidence (0.0–1.0)
  validate_fn:      Box<dyn Fn(&Task, &AttestationEnvelope) -> ValidationResult>
}

OutputContract {
  format:           OutputFormat          // TEXT | JSON | STRUCTURED | BINARY
  schema:           Option<JsonSchema>    // required if format is JSON or STRUCTURED
  max_size_bytes:   Option<u64>
}
```

---

### AgentResolution

The terminal record produced by `TaskLifecycleService.teardown()`. Contains the
`ResolutionCode`, the agent's fate, and the evidence record committed to the archive.

```
AgentResolution {
  resolution_code:  ResolutionCode        // four-category classification
  agent_fate:       AgentFate             // PERSIST | STANDBY | RECYCLE | TERMINATE
  brief_id:         String               // back-reference
  task_id:          String
  job_id:           String
  resolved_at:      ISO8601
  evidence:         AttestationEnvelope
  archive_ref:      String               // ID of the ExecutionArchive write
  corpus_eligible:  bool                 // promoted to RefinementCorpus?
}
```

---

### ResolutionCode

18 codes across 6 categories. HTTP-analogous classification for deterministic routing.

**2xx — Success**

| Code | Label                  | Notes                                          |
|------|------------------------|------------------------------------------------|
| 200  | COMPLETE               | Full success, output validated                 |
| 201  | COMPLETE_WITH_WARNINGS | Output valid, non-blocking issues noted        |
| 202  | PARTIAL_ACCEPTED       | Output incomplete but usable; Job decides fate |
| 203  | DELEGATED              | Task handed to sibling; this agent is clean    |
| 204  | SKIPPED                | Task voided by upstream Job replanning         |

**4xx — Client / Input Error**

| Code | Label                  | Notes                                          |
|------|------------------------|------------------------------------------------|
| 400  | BAD_TASK               | Malformed task object; validation rejected it  |
| 401  | AUTH_FAILURE           | Credential resolution failed at dispatch       |
| 408  | TIMEOUT                | Hit wall_clock_timeout ceiling                 |
| 409  | DUPLICATE              | Foreman/dedup identified redundant execution   |
| 422  | VALIDATION_FAILED      | Output produced but failed success contract    |
| 429  | RATE_LIMITED           | Provider returned 429; provider exhausted      |

**5xx — System / Infrastructure Error**

| Code | Label                  | Notes                                          |
|------|------------------------|------------------------------------------------|
| 500  | SYSTEM_ERROR           | Catch-all; if frequent, a specific code is     |
|      |                        | missing from this registry                     |
| 502  | INFERENCE_UNAVAILABLE  | Endpoint unreachable at dispatch or mid-task   |
| 503  | RUNTIME_NOT_READY      | Wheelhouse still in boot window                |
| 507  | STORAGE_FAILURE        | Archive write failed; serious, notify ops      |

**6xx — Cascade / Propagated Error**

| Code | Label                  | Notes                                          |
|------|------------------------|------------------------------------------------|
| 520  | CASCADE_ERROR          | Error propagated across task boundary          |
| 521  | UPSTREAM_FAILURE       | Blocked-by task failed; this task never ran    |
| 522  | PARTIAL_CASCADE        | Partial upstream failure; degraded execution   |

**7xx — Escalation**

| Code | Label                  | Notes                                          |
|------|------------------------|------------------------------------------------|
| 701  | ESCALATED              | Routed up to Orchestrator or Hub               |
| 702  | HUMAN_REVIEW           | Validation confidence below floor; queued      |

---

### AgentFate

Derived deterministically from `ResolutionCode`. Conservative defaults; shorter lives
when the decision is opaque.

| Fate       | Trigger Codes            | Behavior                                       |
|------------|--------------------------|------------------------------------------------|
| PERSIST    | 200, 201                 | Agent remains alive; eligible for next task    |
| STANDBY    | 202, 203, 204            | Agent idles; awaits Job-level replanning       |
| RECYCLE    | 408, 422, 429, 701, 702  | Credentials revoked; agent state cleared       |
| TERMINATE  | All 4xx, 5xx, 6xx        | Full teardown; agent dropped from pool         |

**Persistence policy:** More capable models (higher tiers, API-class) may persist
between Jobs if `agent_tier` is HUB or ORCHESTRATOR and the resolution is 2xx.
Micro agents (3B class) default to TERMINATE after task completion regardless of
success code. Scope is defined in the Job's `AgentLifetimePolicy`.

Conservative defaults apply when the scope is ambiguous: prefer TERMINATE over
PERSIST when the lifetime decision is not explicitly set.

---

### RetryPolicy

```
RetryPolicy {
  max_attempts:     u32
  backoff_strategy: BackoffStrategy   // NONE | FIXED | EXPONENTIAL
  backoff_base_ms:  u64
  escalation_path:  EscalationPath {
    mode:           EscalationMode    // FAIL | ESCALATE_TIER | HUMAN_REVIEW
    target_tier:    Option<AgentTier>
    notify_job_id:  Option<String>
  }
}
```

**Provider exhaustion (429):** A 429 from an inference provider is an explicit
`RATE_LIMITED` resolution code, not a generic retry trigger. The retry policy must
account for provider-exhaustion states distinctly from transient failures. Fallback
to an alternate provider plate is the correct path, not blind retry.

---

## TaskLifecycleService

### create(task: TaskObject) → AgentBrief

**Phase 1 — Validation**
- Validate all required `TaskObject` fields (task_id, job_id, description,
  success_condition, agent_tier, resource_budget)
- Validate `resource_budget.max_wall_clock_s > 0` and `max_tokens > 0`
- Validate `success_condition` is parseable and `output_contract` schema (if JSON)
  is valid JSON Schema
- Validate `model_id` against the Wheelhouse model ID registry
- Fail fast on any violation; throw `TaskValidationError(field, reason)`

**Phase 2 — Assembly**
- Resolve skills from Foreman's skill library using `skill_hints`
- Fetch knowledge references using `knowledge_hints`
- Resolve and scope credentials; attach as `Vec<ScopedCredential>`
- Allocate `MemoryBlock` from the working memory pool
- Construct `RoleDefinition` from task tier and job context
- Populate `DependencyManifest` (blocked_by, blocks)

**Phase 3 — Sealing**
- Serialize the assembled brief
- Compute SHA-256 hash over the serialized form
- Write `brief_hash` field
- Mark brief as immutable; any mutation attempt invalidates the hash

**Phase 4 — Registration**
- Register the brief in the active brief registry (task_id → brief_id mapping)
- Emit internal event `0030 agent_spawned`

---

### teardown(brief: AgentBrief, result: ExecutionResult) → AgentResolution

**Phase 1 — Harvest**
- Collect execution outputs from the agent's working memory
- Collect checkpoint records if `checkpoint_policy.mode != NONE`
- Collect token usage, wall clock elapsed, any partial outputs
- Produce `AttestationEnvelope` from harvested evidence

**Phase 2 — Classify**
- Apply `success_condition.validate_fn` against outputs
- Map validation result to `ResolutionCode`
- Determine `agent_fate` from the ResolutionCode→AgentFate mapping table
- If `validation_mode == HUMAN`, emit `702 HUMAN_REVIEW` regardless of validate_fn output

**Phase 3 — Garbage Collect**
- Revoke all `ScopedCredential` entries
- Release `MemoryBlock` allocation
- Clear agent context window state
- Deregister brief from active registry
- Emit internal event `0031 agent_dropped`

**Phase 4 — Archive Write**
- Construct `ExecutionArchiveEntry` from brief + attestation + resolution
- Write to `ExecutionArchive` (indexed SQLite)
- Evaluate corpus promotion criteria; if eligible, write to `RefinementCorpus`
- Return `AgentResolution`

**Idempotency:** `teardown()` is safe to call multiple times on the same brief.
Duplicate calls after the initial archive write are no-ops.

---

## ExecutionArchive and RefinementCorpus

Two distinct write destinations. The separation is intentional and must not collapse.

### ExecutionArchive

Every task execution writes here unconditionally — success and failure alike.
The archive is the raw execution log. It feeds the Foreman's mining operations.

```
ExecutionArchiveEntry {
  archive_id:      String         // UUID
  brief_id:        String
  task_id:         String
  job_id:          String
  resolution_code: ResolutionCode
  agent_fate:      AgentFate
  brief_snapshot:  AgentBrief     // full brief at time of execution
  attestation:     AttestationEnvelope
  token_usage:     TokenUsage
  wall_clock_ms:   u64
  archived_at:     ISO8601
}
```

Storage: indexed SQLite. Queried by Foreman for skill library mining.

### RefinementCorpus

A curated subset of the archive. Entries must meet the promotion criteria before
being written here. The corpus is the quality-gated input for skill crystallization.

**Promotion criteria (all must pass):**
1. `resolution_code` is in the 2xx range
2. `success_condition.confidence_floor` was met or exceeded
3. Output is non-redundant with existing corpus entries (dedup gate)
4. `agent_tier` is SPECIALIST or MICRO (Hub/Orchestrator outputs are orchestration,
   not skills)

**Silent library degradation risk:** If the non-redundancy criterion is applied too
loosely, the corpus accumulates near-duplicate skill exemplars and the Foreman's
crystallization quality degrades silently over time. The dedup gate is a critical
quality control point, not a minor filter.

---

## Observability

### Internal Event Codes (CCEE scheme)

Internal four-digit codes for logging and routing. Format: `CC` (originating command
prefix) + `EE` (specific event index within that command's lifecycle).

`CC 00` = system-level events, not user-initiated.

| Code | Event Label                | Log Level | Maps to External |
|------|----------------------------|-----------|------------------|
| 0001 | runtime_boot_started       | info      | —                |
| 0002 | runtime_ready              | info      | —                |
| 0010 | service_connected          | info      | —                |
| 0011 | service_connection_failed  | error     | 502              |
| 0020 | api_spec_loaded            | debug     | —                |
| 0021 | api_spec_hot_loaded        | info      | —                |
| 0022 | api_spec_rejected          | warn      | —                |
| 0030 | agent_spawned              | debug     | —                |
| 0031 | agent_dropped              | debug     | —                |
| 0099 | system_error_generic       | error     | 500              |

Full CCEE registry lives in `wheelhouse-internal-events.yaml`.

### CheckpointPolicy

```
CheckpointPolicy {
  mode:              CheckpointMode  // NONE | INTERVAL | ON_MILESTONE
  interval_secs:     Option<u64>
  milestone_markers: Vec<String>     // named points agent MUST write state at
  destination:       String          // path or queue for checkpoint writes
}
```

---

## Error Propagation

Cascade failures (6xx codes) are the dominant failure mode in multi-agent systems.
The `task-manager` design addresses this at two levels:

1. **Explicit cascade codes** — `520 CASCADE_ERROR`, `521 UPSTREAM_FAILURE`,
   `522 PARTIAL_CASCADE` are distinct ResolutionCodes with distinct AgentFate mappings.
   They are never silently collapsed into generic 5xx errors.

2. **`proof_chain` on Job** — `proof_chain: Vec<Option<Box<dyn SuccessToken>>>` on the
   parent Job provides the verifiable execution record. `is_provably_complete()` is the
   gate predicate. A Job that claims completion without a valid `proof_chain` will not
   pass this gate.

The `SuccessToken` trait exposes `validate()` which dispatches on `token_kind`
internally (`ExternalWitnessed`, `ConsensusRequired`, `ForemansBlessing`). Adding
new validation strategies is filling in code, not restructuring.

---

## External Response Codes

The external code layer translates internal `ResolutionCode` events into user-facing
SMS/API responses. Three-digit, HTTP-analogous. Full registry in
`wheelhouse-response-codes.yaml`.

| External | Class        | Verb                                   | Retryable |
|----------|--------------|----------------------------------------|-----------|
| 200      | success      | ok                                     | false     |
| 201      | success      | idea preserved                         | false     |
| 202      | accepted     | idea queued — processing               | false     |
| 400      | client_error | unknown or malformed command           | true      |
| 404      | client_error | command not recognized                 | true      |
| 408      | client_error | timed out waiting for your input       | true      |
| 409      | client_error | duplicate — this idea already exists   | false     |
| 422      | client_error | understood but unprocessable           | true      |
| 429      | client_error | slow down — rate limited               | true      |
| 500      | system_error | something went wrong — try again       | true      |
| 502      | system_error | inference engine unreachable           | true      |
| 503      | system_error | wheelhouse is starting up              | true      |
| 504      | system_error | timed out — job took too long          | true      |
| 507      | system_error | storage full or unavailable            | false     |
| 520      | system_error | cascade failure — job partially done   | false     |

---

## Agent Lifetime Policy

Defined at the Job level; consumed by `task-manager` when determining `AgentFate`.

```
AgentLifetimePolicy {
  default_fate:            AgentFate       // applied when resolution is ambiguous
  persist_tiers:           Vec<AgentTier>  // tiers eligible for PERSIST fate
  max_idle_secs:           u64             // STANDBY expiry before forced recycle
  micro_always_terminate:  bool            // default true; Micro agents never persist
}
```

Conservative defaults:
- `default_fate`: TERMINATE
- `micro_always_terminate`: true
- `max_idle_secs`: 300

---

## Open Implementation Items

The following are unresolved at spec time and require design decisions before
implementation:

1. **SuccessToken mint path** — `ExternalWitnessed`, `ConsensusRequired`, and
   `ForemansBlessing` variants are structurally wired but `todo!()` at implementation.
   Mint logic for each needs to be specified.

2. **Foreman non-redundancy gate** — The corpus dedup criterion is named but the
   similarity metric and threshold are undefined. Silent library degradation is the
   risk if this is left loose.

3. **Crystallization quality gate** — Foreman skill library compounding is validated
   by research (SkillRL), but the quality gate conditions for promotion from archive
   to corpus are not yet formally specified.

4. **Provider fallback routing** — `429 RATE_LIMITED` should trigger fallback to an
   alternate provider plate, not a blind retry. The routing rule for provider exhaustion
   needs to be encoded in `RetryPolicy` or a separate `ProviderFallbackPolicy`.

5. **Credential scoping model** — `ScopedCredential` revocation is specified at the
   `teardown()` interface but the credential scope model (what a task-level credential
   can and cannot access) is not yet defined. Minimum-privilege scoping is required.

6. **Checkpoint destination format** — `CheckpointPolicy.destination` is a string
   (path or queue). The concrete write format and consumer (Foreman? Debug tooling?)
   is unspecified.

7. **`briefHash` recomputation tooling** — Brief integrity verification requires
   tooling that computes the SHA-256 over the canonical serialization form. The
   canonicalization rule (field ordering, null handling) needs to be locked.

8. **`DependencyManifest` cycle detection** — `blocked_by` / `blocks` graphs can
   form cycles if the Job planner produces a malformed plan. Cycle detection at
   `create()` time is not currently specified.

9. **Human review queue** — `702 HUMAN_REVIEW` fate implies a queue that a human
   can inspect and resolve. The queue design and resolution flow are out of scope for
   `task-manager` but the interface contract (what `task-manager` writes, what the
   reviewer returns) needs to be defined.

10. **`RefinementCorpus` write concurrency** — Multiple parallel task teardowns may
    attempt corpus writes simultaneously. Write ordering and dedup under concurrency
    is unspecified.

---

## Repository Structure (Proposed)

```
task-manager/
├── Cargo.toml
├── README.md
├── task-manager.md             ← this file
├── src/
│   ├── lib.rs
│   ├── brief.rs                ← AgentBrief construction and sealing
│   ├── lifecycle.rs            ← TaskLifecycleService
│   ├── deconstructor.rs        ← teardown phases
│   ├── resolution.rs           ← ResolutionCode, AgentFate, mapping
│   ├── archive.rs              ← ExecutionArchive and RefinementCorpus writes
│   ├── token.rs                ← SuccessToken trait and variants
│   └── types/
│       ├── task.rs
│       ├── brief.rs
│       ├── contract.rs
│       ├── policy.rs
│       └── checkpoint.rs
├── schemas/
│   ├── wheelhouse-response-codes.yaml
│   ├── wheelhouse-internal-events.yaml
│   └── agent-brief.schema.json
└── tests/
    ├── lifecycle_integration.rs
    ├── resolution_mapping.rs
    └── archive_write.rs
```

---

## Related Artifacts

| Artifact                             | Location                    | Status       |
|--------------------------------------|-----------------------------|--------------|
| `wheelhouse-response-codes.yaml`     | `schemas/`                  | Designed     |
| `wheelhouse-internal-events.yaml`    | `schemas/`                  | Designed     |
| `model-id-registry.md`               | Wheelhouse root             | v1.0 — live  |
| `model.schema.json`                  | Wheelhouse root             | v1.0 — live  |
| `wheelhouse-boot-sequence.md`        | Wheelhouse root             | Designed     |
| `06_orchestration_flow.md`           | Wheelhouse docs             | Designed     |
| `05_archive_corpus.md`               | Wheelhouse docs             | Designed     |

---

*This document is the canonical design specification for the `task-manager` repo.
It will be patched incrementally as open items are resolved. Do not regenerate in full —
use `str_replace` patches against specific sections.*
