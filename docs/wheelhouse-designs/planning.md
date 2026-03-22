# Wheelhouse — Planning

**Source:** [project-seed.md](project-seed.md) (concept, architecture, design decisions).
**Immediate next step (from seed):** Draft the resource manifest schema.

- [Wheelhouse — Planning](#wheelhouse--planning)
  - [Goals](#goals)
  - [Questions to Resolve (before building)](#questions-to-resolve-before-building)
    - [1. Manifest location](#1-manifest-location)
    - [2. Agent branch strategy](#2-agent-branch-strategy)
    - [3. Wheelhouse deployment model](#3-wheelhouse-deployment-model)
    - [4. Resource identity](#4-resource-identity)
    - [5. Constraint language (v1)](#5-constraint-language-v1)
    - [6. Scope of the first wheelhouse](#6-scope-of-the-first-wheelhouse)
  - [Approach](#approach)
  - [Decisions](#decisions)
  - [Files / artifacts](#files--artifacts)
  - [Risks and mitigations](#risks-and-mitigations)
  - [Open questions (ongoing)](#open-questions-ongoing)
  - [Progress](#progress)

---

## Goals

- Define and build the resource manifest so the agent knows what is available.
- Establish the wheelhouse as the access control and routing layer between agent(s) and resources (GitHub first, then APIs, local tools, etc.).

---

## Questions to Resolve (before building)

Answer these to lock scope and schema; then we can draft the manifest and implementation plan.

### 1. Manifest location

Where should the resource manifest live?

- Repo root (e.g. `wheelhouse.yaml` or `wheelhouse.json`)?
- A dedicated config directory (e.g. `.wheelhouse/manifest.yaml`)?
- Alongside an existing config file?

**Trend (research):** Project root or a named directory at root (e.g. `config/`, `resources/`) is common; YAML is widely used for readability and version control. Frameworks often use a top-level dir plus an index/map file and per-item files (e.g. Symfony `config/packages/`, Kubernetes-style `k8s-manifests/apps/`).

**Resolved:** YAML. Use a **`resources/`** directory with:
- **`resources-map.yaml`** — index/registry of available resources (ids, types, pointers to per-resource files).
- **Per-resource YAML files** — one file per resource, standard naming (e.g. `<resource-id>.yaml` or `<type>-<name>.yaml`); each file holds that resource’s definition, interface pointer, and usage.

**Map file format (YAML vs JSON):** YAML = same format as per-resource files, commentable, hand-editable; one parser for all of `resources/`. JSON = unambiguous, universal parse, easier to emit from code if map is machine-generated. **Recommendation:** YAML for the map so the whole `resources/` surface is one format; use JSON only if the map is primarily machine-generated or consumed by many non-YAML runtimes. (Per-resource files stay YAML.)

Schema design will define the exact names and fields for the map and per-resource files.

### 2. Agent branch strategy

One long-lived `agent` branch, or branch-per-session / per-task (e.g. `agent/session-123`) with merges back to main? How do you want to handle conflicts and history?

**Three proposals (1–10, 10 = best):**

| # | Strategy | Description | Pros | Cons | Rating |
|---|----------|-------------|------|------|--------|
| **A** | **Long-lived agent branch** | Single branch `agent` (or `ai/agent`); all agent commits go there; optional periodic merge to `main`. | Simple; one place to audit (`git log agent`); minimal branch management. | History is a single line; conflicts accumulate; hard to isolate “what changed in session X”; reverting is coarse. | **5** |
| **B** | **Branch per session** | Create `agent/session-<id>` or `agent/YYYY-MM-DD-HHmm` per run; merge to `main` (or to `agent`) when session ends. | Clear attribution per session; easy revert of a session; aligns with “captain” model (one session = one helm). | More branches; need policy for when to merge/delete; merge conflicts if multiple agents touch same files. | **8** |
| **C** | **Short-lived branch + PR per task** | Agent works on `agent/task-<id>` (or `ai/agent-name/task-description`); opens PR; human or automation merges to `main`. | Best audit trail (PR = review + context); fits governance “eject if rules broken”; no direct push to `main`. | Heavier workflow; depends on PR merge cadence; agent must handle PR creation/update. | **7** |

**Recommendation for research stage:** **B (branch per session)** — good balance of auditability and simplicity; matches “captain takes the helm” and “eject if rules broken” (session boundary = natural eject). C is stronger for strict human-in-the-loop; A is acceptable only if you want minimal complexity and rarely merge agent work to main.

**Chosen:** **B** — locked in Decisions.

### 3. Wheelhouse deployment model

For the “first thing to build,” should the manifest + routing logic be:

- A **library** the agent process imports and calls in-process, or
- A small **service** (HTTP or RPC) that the agent calls?

This affects the manifest schema (e.g. whether it includes “how to reach” the wheelhouse).

**Resolved (mental model):** The wheelhouse is a **plugin/channel** system:

- **Agent at the desk** — Agent is anchored at a “closed circular desk”; it consults the resource collection (manifest + descriptions), matches need via description + its own conceptualization, then opens a **channel** to one (or the) instance of that resource.
- **Resource groups & interfaces** — Resources are grouped by kind (e.g. “persistent storage”, subtype “database”) with an **internally consistent interface** optimized for the agent. The YAML points to the **hotswappable plugin** (where to load it, how to use the interface). Flow: read manifest → launch plugin → use interface → document → shutdown → move on.
- **Central intelligence + queue** — A **central coordinator and queue** receives requests and processes activity/artifacts from a **cadre of agents**. Multi-agent from the start; wheelhouse is the broker + rule enforcer; agents are “captains” that request channels and must follow rules or be ejected.

**Build order:** Wheelhouse + GitHub plugin first; then the mechanism for an **external** agent to “touch” the wheelhouse (request channel, get plugin ref, use interface) without being embedded. Manifest schema must support: resource id, type/subtype, plugin location/loader, interface contract, and (stubbed) constraints.

### 4. Resource identity

Should the manifest identify resources by **path/URL + type** only, or do you want explicit **stable names/IDs** (e.g. `github:my-org/my-repo`, `api:weather`) for routing and constraints?

**Resolved:** Resources live in the repo (GitHub); prioritize GitHub/agent interface after core manifest. Use a **universal identifier** standard. Options that could work:

| Option | Form | Pros | Cons |
|--------|------|------|------|
| **URN (RFC 8141)** | `urn:wheelhouse:github:org/repo` or `urn:wheelhouse:resource:<id>` | Standard, location-independent, globally unique in namespace | Requires defining NID and NSS; resolution is separate |
| **Custom URI scheme** | `wheelhouse:github:org/repo` or `wh:resource:<id>` | Short, clear ownership; easy to parse | Non-standard scheme; tooling may not recognize |
| **GitHub-style** | `github:org/repo` or `github:org/repo#path` | Familiar, matches GitHub API idioms | Tied to one backend; less generic for “database”, “api” |
| **URI path in repo** | `resources/<id>.yaml` or `/resources/<type>/<name>` | Simple, filesystem-aligned; id = path | Not globally unique across wheelhouses; fine for single-repo |

**Recommendation:** Use a **stable id** in the manifest (e.g. `id: my-repo-storage`) and optionally expose a **URI form** for references (e.g. `wheelhouse:resource:my-repo-storage` or URN). Schema can require `id` and optionally `uri`; start with `id` + type, add formal URI once we add multi-repo.

### 5. Constraint language (v1)

For v1, is **simple flags** (e.g. `read_only: true`, `allowed_ops: [read]`) plus resource-type scoping enough, with a richer DSL later? Or do you need expression-style constraints (e.g. time-based, user-based) from the start?

**Resolved (deferred):** Hold detailed constraint language until other structures are fleshed out. **Stub in schema** with a placeholder so it’s not forgotten, e.g.:

- `constraints: [ ]` or `constraints: { approval: null }` in resource/manifest, or
- A reserved key like `constraints: gain-approval` / `constraints: TBD` with a short comment in the schema doc.

We’ll replace the stub when we decide allowlist/denylist, read-only flags, or a DSL.

### 6. Scope of the first wheelhouse

Is the first target **only** “this one GitHub repo + one agent,” or should the manifest and code be designed for multi-repo / multi-resource from day one (even if only one repo is used initially)?

**Resolved:** With the plugin/channel model we **must** build in this order:

1. **Wheelhouse core** + **GitHub plugin** (manifest, resources dir, GitHub as first resource type).
2. **Agent interface** — the way an external agent “touches” the wheelhouse (request channel, get plugin, use interface) **without** being built into the wheelhouse.

Agents are **captains**: they take the helm of the “ship of knowledge,” agree to follow the rules, or the system ejects them. So: single wheelhouse, multi-agent from the start; first implementation can still be “one repo + one agent” for simplicity, but the design is external-agent + rules-based ejection.

## Approach

**Research stage (current):** Questions 1–6 processed; branch strategy locked (B: branch per session). Next: draft resource manifest schema.

**Schema & structure:**

- **Manifest layout:** `resources/resources-map.yaml` (index) + `resources/<resource-id>.yaml` (or `<type>-<name>.yaml`) per resource. Schema defines fields for map and per-resource files.
- **Resource identity:** Required stable `id` in manifest; optional `uri` (e.g. `wheelhouse:resource:<id>` or URN) when we go multi-repo. Prioritize GitHub as first resource type in the schema.
- **Plugin/channel model:** Each resource entry points to a hotswappable plugin (location, loader, interface contract). Agent: read manifest → match by description + need → open channel → use interface → document → shutdown.
- **Constraint model:** Stub only (e.g. `constraints: [ ]` or `constraints: TBD`); flesh out later (allowlist/denylist, read-only, DSL).
- **Versioning:** Pin resource/plugin versions (refs, tags); agent references specific versions to avoid drift. Detail in schema.
- **Audit:** Every wheelhouse interaction produces a commit or log entry; design for it from the start (GitHub gives this for repo ops).

**Build order:** (1) Wheelhouse core + resource manifest schema. (2) GitHub plugin (first resource type). (3) Agent interface (external agent requests channel, gets plugin ref, follows rules or is ejected). Central queue/cadre in a later phase.

---

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Per-resource files | YAML | Readable, commentable, version-control friendly; fixed. |
| Map file (index) | YAML (recommended) or JSON | YAML: consistency with per-resource files, comments. JSON: unambiguous, universal parse, good if map is machine-generated. See Q1 in doc. |
| Manifest layout | `resources/` + map file + per-resource YAML | Index + one YAML file per resource; clear, extensible. |
| Deployment model | Plugin/channel; external “captain” agents | Agent at desk; requests channel; hotswappable plugins; central queue later. |
| Resource identity | **URN** for formal identity; stable `id` in schema | Use URN (e.g. `urn:wheelhouse:resource:<id>`) — namespace-based, expansible beyond digital: physical locations, places, real-world entities via sub-namespaces or NIDs (e.g. `urn:wheelhouse:location:...`). Resolution stays a separate layer. |
| Constraints (v1) | Stub in schema | Defer full language; placeholder so we don’t forget. |
| Scope | Multi-agent design; first impl can be 1 repo + 1 agent | Captain model + ejection; wheelhouse + GitHub plugin first. |
| Branch strategy | **B: branch per session** | Create `agent/session-<id>` or `agent/YYYY-MM-DD-HHmm` per run; merge to `main` (or `agent`) when session ends. Clear attribution, easy revert by session, fits captain model. |

---

## Files / artifacts

| Path | Purpose |
|------|---------|
| `resources/resources-map.yaml` (or `.json`) | Index of resources (ids, types, pointers to per-resource files). Format per Decisions (YAML recommended). |
| `resources/<id>.yaml` (or similar) | Per-resource definition: id, type, plugin ref, interface, constraints stub. |
| Schema doc (TBD) | Formal schema for map + per-resource YAML (fields, types). |
| Wheelhouse core (TBD) | Load manifest, resolve resources, enforce rules. |
| GitHub plugin (TBD) | First resource type; repo as storage; agent branch strategy per Q2. |

---

## Risks and mitigations

| Risk | Mitigation |
|------|-------------|
| *TBD* | |

---

## Open questions (ongoing)

- **Branch strategy:** Locked — B (branch per session). See Decisions.
- **Resource types:** What beyond GitHub? (APIs, local fs, external services.) Will emerge from plugin model. **Scope principle:** The wheelhouse should ultimately be capable of presenting any kind of interface for access to data, tasks, etc. that an agent can use at all — not limited to storage; extend to any resource type the plugin model can describe.
- **Constraint language:** Full DSL or expression language — deferred; stub in schema.
- **URI scheme:** **Leaning URN** (see Decisions). Expansibility beyond digital: URN’s NID model allows sub-namespaces for physical locations, places, real-world entities (e.g. `urn:wheelhouse:location:...`) without changing the overall scheme. For v1, opaque `id` in manifest; emit URN form when we add logs/external refs.
- **Central queue:** Design/scope for "central intelligence and queue" for cadre of agents — post–first impl.

---

## Progress

*Append at milestones: what’s done, what’s left. Get agreement before next step.*

- **Research iteration (this pass):** Processed all six planning questions from comments. Resolved: manifest location (resources/ + map + per-resource YAML), deployment model (plugin/channel, captain, central queue), resource identity (stable id + options for URI), constraint (stub), scope (build order + external agent). Added three branch-strategy proposals (A/B/C); **branch strategy locked: B (branch per session).** Updated Approach, Decisions, Files, Open questions. **Next:** Draft resource manifest schema.
