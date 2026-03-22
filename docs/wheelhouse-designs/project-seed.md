# Agent Wheelhouse — Project Idea

## Concept

A governed resource layer sitting between agents and the outside world. Rather than agents reaching out arbitrarily, they pull from a defined, permissioned set of resources curated by the owner. GitHub serves as one node in that registry: versioned, auditable, private, and API-accessible.

## Origin / Background

This project emerged from:

- Duplicating a GitHub repo using the template repository feature to preserve the original
- Connecting a single application to that private repo via the GitHub REST API + PAT to read/write files programmatically
- Recognizing GitHub as a viable "web reservoir" — persistent versioned backend storage for an agent

**GitHub API access pattern established:**

- Auth via Personal Access Token (PAT) scoped to repo only
- Token stored as env variable (`GITHUB_TOKEN`), never hardcoded
- PyGithub library preferred over raw HTTP requests
- Agent writes to a dedicated agent branch, not directly to main
- Full commit history = built-in audit trail of every agent action

## Architecture

```
Agent
  └── Wheelhouse (resource broker)
        ├── GitHub repo (files, configs, memory, prompts)
        ├── APIs (whatever is explicitly exposed)
        ├── Local tools (bash, fs, etc.)
        └── [future nodes]
```

The wheelhouse is not just storage — it is the access control and routing layer. The agent requests a resource from the wheelhouse; the wheelhouse decides if and how to serve it.

## Key Design Decisions

### 1. Resource Manifest

A structured file (JSON or YAML) living in the repo itself that declares:

- What resources exist
- Their types
- Access rules

The agent reads this first, then knows what is available. **This is the first thing to build.**

### 2. Constraint Model

Formalize before building:

- Allowlist vs. denylist approach
- Scope by resource type
- Read-only vs. read-write flags per resource

### 3. Versioning Discipline

- Pin resource versions explicitly
- Agent references specific commits or tags rather than always pulling HEAD
- Prevents silent drift in agent behavior

### 4. Audit Trail

- Every agent action touching the wheelhouse produces a commit or log entry
- GitHub provides this mostly for free
- Design for it explicitly from the start — don't bolt it on later

## Immediate Next Step

**Draft the resource manifest schema.**

This single file forces clarification of what a "resource" is in this system. Everything downstream — routing logic, constraint enforcement, versioning — becomes easier once the schema is locked.

## Open Questions

- What resource types beyond GitHub repos? (APIs, local fs, external services?)
- What is the constraint expression language? (Simple flags? A DSL?)
- Does the wheelhouse run as a service, a library, or a sidecar to the agent?
- Single agent or multi-agent access to the same wheelhouse?

## Stack Notes

- PyGithub for GitHub API interaction
- Private repo as the backing store
- PAT with minimal scope (repo only)
- Dedicated agent branch for writes
- Rate limit: 5,000 GitHub API requests/hour — sufficient for agent workloads
