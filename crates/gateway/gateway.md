# Cloak — Cloud Model Access Gateway

**Version:** 0.1.0-design  
**Status:** Pre-implementation specification  
**Language:** Rust  
**Role:** Standalone external node; receives sanitized prompts from authorized callers, routes to registered cloud model providers, returns sanitized responses. No business logic. No agent behavior. Pure gateway.

---

## Table of Contents

1. [Conceptual Boundaries](#1-conceptual-boundaries)
2. [System Architecture Overview](#2-system-architecture-overview)
3. [Component Inventory](#3-component-inventory)
4. [Route Store](#4-route-store)
5. [Sanitization Layer](#5-sanitization-layer)
6. [Request Lifecycle](#6-request-lifecycle)
7. [Cost & Token Accounting](#7-cost--token-accounting)
8. [Logging System](#8-logging-system)
9. [Error Reporting & Alert Routing](#9-error-reporting--alert-routing)
10. [Kill Switch Architecture](#10-kill-switch-architecture)
11. [Rate Limiting & Deduplication](#11-rate-limiting--deduplication)
12. [Health Probing & Fallback Chains](#12-health-probing--fallback-chains)
13. [Session & Caller Identity](#13-session--caller-identity)
14. [Configuration & Versioning](#14-configuration--versioning)
15. [Drain Mode & Graceful Shutdown](#15-drain-mode--graceful-shutdown)
16. [API Surface](#16-api-surface)
17. [Data Structures](#17-data-structures)
18. [Crate Dependencies](#18-crate-dependencies)
19. [Directory Layout](#19-directory-layout)
20. [Implementation TODO List](#20-implementation-todo-list)

---

## 1. Conceptual Boundaries

Cloak is not an agent. It does not plan, reason, or store knowledge. Its contract is narrow and must stay narrow:

**Accepts:** A sanitized prompt payload from an authorized caller with a route identifier.  
**Does:** Validates the payload, routes to the appropriate provider, collects the response, sanitizes it, logs everything, accounts for cost.  
**Returns:** A sanitized response payload with metadata.  
**Refuses:** Everything else.

The only things that cross Cloak's boundary in either direction are:
- Inbound: sanitized prompt + route key + caller identity token + request ID
- Outbound: sanitized response + token counts + cost + request ID + status

No raw user data. No tool calls. No agent state. No model-selection logic based on content analysis — that is the caller's job. The caller tells Cloak *which route* to use. Cloak executes the route or fails cleanly.

This boundary discipline is non-negotiable. Every pressure to add intelligence to the gateway should be treated as a design smell in the caller.

---

## 2. System Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    AUTHORIZED CALLERS                   │
│           (Wheelhouse Hub, CLI, test harness)           │
└───────────────────────────┬─────────────────────────────┘
                            │ HTTPS + caller token
                            ▼
┌─────────────────────────────────────────────────────────┐
│                      CLOAK GATEWAY                      │
│                                                         │
│  ┌─────────────┐   ┌──────────────┐  ┌──────────────┐  │
│  │  Inbound    │   │  Route       │  │  Outbound    │  │
│  │  Sanitizer  │──▶│  Dispatcher  │─▶│  Sanitizer   │  │
│  └─────────────┘   └──────┬───────┘  └──────────────┘  │
│                           │                             │
│  ┌─────────────┐   ┌──────▼───────┐  ┌──────────────┐  │
│  │  Kill Switch│   │  Route Store │  │  Cost        │  │
│  │  Controller │   │  + Fallbacks │  │  Accountant  │  │
│  └─────────────┘   └──────────────┘  └──────────────┘  │
│                                                         │
│  ┌─────────────────────────────────────────────────┐    │
│  │                 Logging Bus                     │    │
│  │  (SQLite WAL — operational + audit separation)  │    │
│  └─────────────────────────────────────────────────┘    │
│                                                         │
│  ┌─────────────────────────────────────────────────┐    │
│  │              Alert Router                      │    │
│  │   (Telnyx SMS | webhook | log-only)            │    │
│  └─────────────────────────────────────────────────┘    │
│                                                         │
│  ┌─────────────┐   ┌──────────────┐                     │
│  │  Health     │   │  Rate        │                     │
│  │  Prober     │   │  Limiter     │                     │
│  └─────────────┘   └──────────────┘                     │
└───────────────────────────┬─────────────────────────────┘
                            │ HTTPS (per-route API credentials)
                            ▼
┌─────────────────────────────────────────────────────────┐
│              CLOUD MODEL PROVIDERS                      │
│     (Anthropic, OpenAI, Mistral, Groq, etc.)           │
└─────────────────────────────────────────────────────────┘
```

---

## 3. Component Inventory

| Component | Responsibility |
|---|---|
| **Inbound Sanitizer** | Strips, validates, and normalizes the incoming prompt payload before any routing logic touches it |
| **Route Dispatcher** | Resolves route key → provider config, selects primary or fallback, dispatches the sanitized request |
| **Route Store** | Persistent, versioned registry of all known routes; source of truth for provider configs and fallback chains |
| **Outbound Sanitizer** | Validates and strips provider responses before returning to caller |
| **Cost Accountant** | Tracks token counts and estimated cost per request, per route, per caller, per time window |
| **Logging Bus** | Append-only structured log of all traffic; dual-table SQLite (operational + audit) |
| **Alert Router** | Classifies errors into alert levels; dispatches to configured destinations (SMS, webhook, log) |
| **Kill Switch Controller** | Accepts external and internal halt signals; manages drain and hard-stop states |
| **Health Prober** | Background process that periodically fires minimal probes at each route; flags dead routes before live traffic hits them |
| **Rate Limiter** | Per-caller × per-route request throttling with configurable windows and limits |
| **Deduplicator** | Fingerprints inbound sanitized prompts; rejects near-duplicate requests within a short window |
| **Config Manager** | Loads, validates, versions, and hot-reloads `cloak.toml` and route store |
| **CLI Interface** | Local administrative interface for inspection, kill switch, route management, log search |

---

## 4. Route Store

### Purpose

The Route Store is Cloak's persistent registry of all model routes. It is the only place where provider credentials, model identifiers, and fallback chains are defined. Nothing about provider configuration lives in code.

### Storage

SQLite table `routes` on the local data volume. Versioned — every write to a route record increments its `version` field and writes the prior state to `routes_history`. Route changes are never destructive.

### Route Record

```rust
pub struct Route {
    pub route_key: String,          // Stable identifier used by callers: "claude-sonnet", "gpt4o-mini", etc.
    pub display_name: String,
    pub provider: Provider,         // enum: Anthropic | OpenAI | Mistral | Groq | Custom
    pub model_id: String,           // Provider's model string, verbatim
    pub endpoint_url: String,       // Full base URL for the provider API
    pub api_key_env: String,        // Name of the env var that holds the credential — never the key itself
    pub max_input_tokens: u32,
    pub max_output_tokens: u32,
    pub cost_per_input_token_usd: f64,
    pub cost_per_output_token_usd: f64,
    pub fallback_chain: Vec<String>, // Ordered list of route_keys to try on failure; empty = reject on failure
    pub health_probe_interval_secs: u64,
    pub active: bool,
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<String>,          // Arbitrary labels for filtering and log grouping
}

pub enum Provider {
    Anthropic,
    OpenAI,
    Mistral,
    Groq,
    Custom { name: String },
}
```

### Route Management (CLI)

```
cloak route list
cloak route add --file route.json
cloak route update <route_key> --field max_output_tokens --value 4096
cloak route disable <route_key>
cloak route enable <route_key>
cloak route history <route_key>
cloak route rollback <route_key> --version <n>
cloak route probe <route_key>    # fire a manual health probe immediately
```

### Credential Handling

**API keys are never stored in the route record.** The `api_key_env` field holds the name of an environment variable (e.g., `ANTHROPIC_API_KEY`). At dispatch time, Cloak reads the key from the environment. This means:

- The route store can be inspected, logged, and version-controlled without ever exposing credentials
- Key rotation requires only updating the env var, not touching the route record
- The route store file is safe to back up without redaction

---

## 5. Sanitization Layer

### Architecture

Sanitization is a discrete, independently testable pipeline stage — not inline routing logic. Both inbound and outbound sanitizers implement the same `Sanitizer` trait with their own configured rule sets. They are replaceable without touching the dispatcher.

```rust
pub trait Sanitizer: Send + Sync {
    fn sanitize(&self, payload: &RawPayload) -> Result<SanitizedPayload, SanitizationError>;
}

pub struct SanitizationError {
    pub kind: SanitizationErrorKind,
    pub field: String,
    pub detail: String,
}

pub enum SanitizationErrorKind {
    SchemaViolation,       // Payload doesn't match expected structure
    ContentViolation,      // Content blocked by configured rules
    EncodingError,         // Non-UTF-8 or malformed encoding
    SizeLimitExceeded,     // Payload too large
    InjectionPattern,      // Known prompt injection signature detected
    MalformedJson,
}
```

### Inbound Sanitizer Rules

Applied to every request before the route dispatcher touches it:

1. **Schema validation** — payload must match `InboundRequest` schema exactly; unknown fields are rejected, not ignored
2. **Size limit** — configurable per-route `max_input_tokens`; hard reject if exceeded
3. **Encoding** — must be valid UTF-8; no binary blobs
4. **Injection pattern check** — configurable regex/string blacklist for known prompt injection signatures; logged and rejected
5. **Field stripping** — any field not in the schema is stripped before processing
6. **Caller token validation** — validated here before any processing begins; invalid token = immediate 401, logged

### Outbound Sanitizer Rules

Applied to every provider response before returning to caller:

1. **Schema validation** — response must match `OutboundResponse` schema; malformed provider responses are caught here
2. **Content policy strip** — provider-specific boilerplate, refusal headers, or metadata not intended for the caller
3. **Size validation** — response within expected bounds for the model/route
4. **Encoding normalization** — consistent UTF-8 output regardless of provider encoding quirks
5. **Credential scrub** — defensive scan for any string matching known API key patterns; if found, response is rejected and an alert fires at CRITICAL level

---

## 6. Request Lifecycle

```
1. Caller sends POST /dispatch
   └─ Includes: route_key, sanitized_prompt, caller_token, request_id (caller-generated UUID)

2. Inbound Sanitizer
   ├─ Schema validation
   ├─ Caller token verification
   ├─ Size check
   ├─ Injection pattern scan
   └─ Reject → log + alert if any check fails; return error response immediately

3. Rate Limiter check (caller × route window)
   └─ Reject → 429 response, logged

4. Deduplicator check (fingerprint × window)
   └─ Reject → 409 response, logged

5. Kill Switch check
   └─ If HALT or DRAIN state → 503 response, logged

6. Route Store lookup (route_key)
   └─ Not found → 404, logged
   └─ Route inactive → 503, logged

7. Cost budget check (caller ceiling, route ceiling, global ceiling)
   └─ Ceiling hit → 402, logged, alert fires

8. Health Prober last-known status check
   └─ Route marked unhealthy → skip to fallback chain resolution

9. Dispatch to primary route endpoint
   └─ On success → proceed to step 12
   └─ On failure (timeout, 429, 5xx) → fallback chain resolution

10. Fallback chain resolution
    ├─ Try next route_key in fallback_chain
    ├─ Each attempt logged with attempt number and reason
    └─ All fallbacks exhausted → return 502, log, alert fires

11. Provider response received

12. Outbound Sanitizer
    ├─ Schema validation
    ├─ Credential scrub
    └─ Reject → 502, log, CRITICAL alert (credential leak attempt)

13. Cost Accountant records:
    ├─ input_tokens, output_tokens, total_tokens
    ├─ estimated_cost_usd
    ├─ route_key, caller_id, request_id, timestamp
    └─ Running totals updated for caller and route

14. Logging Bus writes complete request record to:
    ├─ Operational log (searchable, rotatable)
    └─ Audit log (append-only, never rotated)

15. Response returned to caller
    └─ Includes: sanitized_response, request_id, input_tokens, output_tokens, cost_usd, route_key_used, latency_ms
```

---

## 7. Cost & Token Accounting

### Per-Request Record

Every dispatched request — successful or not — writes a cost record:

```rust
pub struct CostRecord {
    pub request_id: Uuid,
    pub caller_id: String,
    pub route_key: String,
    pub route_key_used: String,        // May differ from requested if fallback triggered
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
    pub estimated_cost_usd: f64,
    pub fallback_triggered: bool,
    pub fallback_attempt: u8,
    pub timestamp: DateTime<Utc>,
    pub outcome: RequestOutcome,       // Success | ProviderError | SanitizationReject | etc.
}
```

### Budget Ceilings

Three configurable ceiling levels, all checked before dispatch:

| Level | Scope | Config key |
|---|---|---|
| Per-caller | Rolling 24h spend per `caller_id` | `budgets.per_caller_daily_usd` |
| Per-route | Rolling 24h spend on a specific route | `budgets.per_route_daily_usd` |
| Global | Total gateway spend any rolling window | `budgets.global_daily_usd` |

Ceiling hit = 402 response + alert at WARN level. Configurable to escalate to ERROR after N consecutive ceiling hits.

### CLI Queries

```
cloak cost summary                          # Today's spend by route and caller
cloak cost summary --caller <id>            # Specific caller's spend
cloak cost summary --route <key>            # Specific route's spend
cloak cost summary --window 7d              # Last 7 days
cloak cost top-routes --window 30d          # Ranked by spend
cloak cost top-callers --window 30d
cloak budget set --caller <id> --daily 5.00
cloak budget set --global --daily 20.00
```

---

## 8. Logging System

### Dual-Table Architecture

All traffic logs land in a local SQLite database (`cloak_logs.db`) with WAL mode enabled. Two tables with distinct properties:

**`operational_log`**  
- Full structured records of every request
- Indexed on: `timestamp`, `caller_id`, `route_key`, `request_id`, `outcome`
- Subject to rotation: configurable retention (default 90 days)
- Optimized for fast search queries
- This is the table you query during normal operations

**`audit_log`**  
- Append-only; never updated, never deleted, never rotated
- Covers: every sanitization rejection, every kill switch event, every alert dispatch, every credential scrub trigger, every route change
- Exists exclusively for post-incident forensics and compliance
- Separate file (`cloak_audit.db`) for isolation

### Operational Log Schema

```sql
CREATE TABLE operational_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    request_id      TEXT NOT NULL,
    caller_id       TEXT NOT NULL,
    route_key       TEXT NOT NULL,
    route_key_used  TEXT,
    input_tokens    INTEGER,
    output_tokens   INTEGER,
    total_tokens    INTEGER,
    cost_usd        REAL,
    latency_ms      INTEGER,
    outcome         TEXT NOT NULL,       -- 'success' | 'sanitization_reject' | 'provider_error' | etc.
    error_code      TEXT,
    error_detail    TEXT,
    fallback_used   INTEGER DEFAULT 0,
    fallback_attempt INTEGER DEFAULT 0,
    inbound_hash    TEXT,               -- SHA-256 of sanitized prompt (not the prompt itself)
    timestamp       TEXT NOT NULL,
    tags            TEXT                -- JSON array
);

CREATE INDEX idx_timestamp    ON operational_log(timestamp);
CREATE INDEX idx_caller_id    ON operational_log(caller_id);
CREATE INDEX idx_route_key    ON operational_log(route_key);
CREATE INDEX idx_request_id   ON operational_log(request_id);
CREATE INDEX idx_outcome      ON operational_log(outcome);
```

### Audit Log Schema

```sql
CREATE TABLE audit_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type      TEXT NOT NULL,      -- 'sanitization_reject' | 'kill_switch' | 'alert_dispatched' | etc.
    request_id      TEXT,
    caller_id       TEXT,
    route_key       TEXT,
    severity        TEXT NOT NULL,      -- 'DEBUG' | 'INFO' | 'WARN' | 'ERROR' | 'CRITICAL'
    detail          TEXT NOT NULL,      -- Structured JSON blob
    timestamp       TEXT NOT NULL
);
-- No indexes on audit_log by design; sequential append only.
-- If forensic queries become slow, a read-only copy with indexes is acceptable.
-- Never modify the source.
```

### CLI Log Search

```
cloak log search --caller <id>
cloak log search --route <key>
cloak log search --outcome sanitization_reject
cloak log search --from 2026-03-01 --to 2026-03-10
cloak log search --request-id <uuid>
cloak log search --error-code 502 --window 1h
cloak log tail                              # Live stream of incoming log entries
cloak log tail --level ERROR                # Filtered live stream
cloak audit search --event kill_switch
cloak audit search --severity CRITICAL --window 7d
```

---

## 9. Error Reporting & Alert Routing

### Alert Levels

| Level | Meaning | Default destination |
|---|---|---|
| `DEBUG` | Verbose operational events | Log only |
| `INFO` | Normal lifecycle events (route probe success, rotation, etc.) | Log only |
| `WARN` | Degraded but not failing (cost ceiling approaching, fallback triggered) | Log + optional webhook |
| `ERROR` | A request failed in a way that warrants attention | Log + webhook |
| `CRITICAL` | System integrity concern (credential scrub hit, kill switch thrown, all fallbacks exhausted repeatedly) | Log + webhook + SMS |

### Alert Record

```rust
pub struct Alert {
    pub alert_id: Uuid,
    pub level: AlertLevel,
    pub source: AlertSource,        // which component raised it
    pub request_id: Option<Uuid>,
    pub route_key: Option<String>,
    pub caller_id: Option<String>,
    pub message: String,
    pub detail: serde_json::Value,  // Structured context for programmatic consumption
    pub timestamp: DateTime<Utc>,
    pub dispatched_to: Vec<AlertDestination>,
}

pub enum AlertSource {
    InboundSanitizer,
    OutboundSanitizer,
    RouteDispatcher,
    FallbackChain,
    HealthProber,
    KillSwitch,
    CostAccountant,
    RateLimiter,
}

pub enum AlertDestination {
    Log,
    Webhook { url: String },
    Sms { to: String, via: SmsProvider },
}

pub enum SmsProvider {
    Telnyx { from: String },
}
```

### Alert Routing Configuration

Defined in `cloak.toml` under `[alerts]`. The routing table maps alert levels to destinations. Fully configurable without code changes:

```toml
[alerts]

[alerts.destinations]
  [alerts.destinations.telnyx]
    enabled = true
    from_number = "+18334332269"
    to_numbers = ["+1XXXXXXXXXX"]
    api_key_env = "TELNYX_API_KEY"

  [alerts.destinations.webhook]
    enabled = false
    url = "https://your-webhook-endpoint.example.com/cloak-alerts"
    secret_env = "WEBHOOK_SECRET"

[alerts.routing]
  DEBUG    = ["log"]
  INFO     = ["log"]
  WARN     = ["log", "webhook"]
  ERROR    = ["log", "webhook"]
  CRITICAL = ["log", "webhook", "sms"]

# Per-source overrides — more granular than level-only routing
[alerts.source_overrides]
  OutboundSanitizer.WARN = ["log", "sms"]    # Any outbound sanitizer warn is treated as SMS-worthy
  HealthProber.WARN      = ["log", "webhook"]

# Suppression windows — avoid SMS flooding on cascading failures
[alerts.suppression]
  sms_cooldown_secs = 300          # Minimum gap between SMS alerts of the same level
  max_sms_per_hour  = 10           # Hard cap; excess alerts go to webhook only
```

### SMS Message Format

Messages are terse by design — this is a phone:

```
[CLOAK CRITICAL] Outbound sanitizer: credential pattern detected in response
Route: claude-sonnet | Req: 3f4a...b9c2
2026-03-21 14:32:07 UTC
```

```
[CLOAK ERROR] All fallbacks exhausted
Route: gpt4o-mini → gpt4o → FAIL
Caller: wheelhouse-hub | Req: 7a1d...44e1
2026-03-21 09:15:22 UTC
```

---

## 10. Kill Switch Architecture

### States

```
OPERATIONAL  ──────────────────────────────▶  DRAIN
     │         (soft stop: finish in-flight)    │
     │                                          │ (in-flight complete OR timeout)
     │                                          ▼
     └─────────────────────────────────────▶  HALTED
               (hard stop: immediate)
```

| State | Behavior |
|---|---|
| `OPERATIONAL` | Normal; all requests processed |
| `DRAIN` | No new requests accepted (503); in-flight requests complete normally; timeout configurable |
| `HALTED` | All activity stopped; socket closed; no requests in or out |

### Kill Switch Triggers

**External (remote):**
- HTTP DELETE to `/admin/kill` with admin token
- Configurable: `DRAIN` or `HALT` as the target state
- CLI: `cloak kill --mode drain` / `cloak kill --mode halt`

**Internal (automatic):**
- Consecutive CRITICAL alerts exceeding configurable threshold → auto-DRAIN
- Cost ceiling hit globally → configurable: warn only OR auto-DRAIN
- Health prober: all routes simultaneously unhealthy → auto-HALT
- Credential scrub trigger on outbound sanitizer → immediate auto-HALT (this is never acceptable)

**Recovery:**
```
cloak resume    # Returns from DRAIN or HALTED to OPERATIONAL
```
Recovery from auto-HALT requires explicit CLI confirmation — it cannot be triggered remotely without the admin token. This is intentional.

### Kill Switch Events

Every state transition is written to the audit log and dispatches a CRITICAL alert to all configured destinations, regardless of suppression windows. Kill switch events are never suppressed.

---

## 11. Rate Limiting & Deduplication

### Rate Limiting

Per-caller × per-route token bucket. Configurable independently per caller and per route:

```toml
[rate_limits.defaults]
  requests_per_minute = 60
  requests_per_hour   = 500

[rate_limits.callers.wheelhouse-hub]
  requests_per_minute = 120
  requests_per_hour   = 2000

[rate_limits.routes.claude-opus]
  requests_per_minute = 10    # Expensive route — tighter limit
  requests_per_hour   = 50
```

Rate limit hits return 429 with a `Retry-After` header. Logged. Never alerted at higher than WARN unless the hit rate is sustained (configurable consecutive-hit threshold).

### Deduplication

Inbound sanitized prompts are SHA-256 fingerprinted after sanitization. The fingerprint is checked against a short-window cache (configurable; default 30 seconds).

Fingerprint match → 409 response. The original `request_id` that produced the fingerprint is included in the 409 body so the caller can trace it.

This catches runaway loops, double-submits from retry logic, and agent bugs — not a content-equality check, just a structural hash. The fingerprint is logged, not the prompt content.

---

## 12. Health Probing & Fallback Chains

### Health Prober

Background async task. For each active route, at its configured `health_probe_interval_secs`:

1. Construct a minimal known-good prompt (configurable per route; default: single-token echo test)
2. Dispatch directly (bypassing rate limiter and deduplicator — probes are internal)
3. Record: latency, response validity, HTTP status
4. Update route health status: `Healthy | Degraded | Unhealthy`

Health status changes are logged. `Unhealthy` triggers a WARN alert. Route stays in the unhealthy state until a probe succeeds.

Probe traffic is marked in the cost records as `probe: true` and excluded from billing summaries by default (configurable).

### Fallback Chain Resolution

When primary dispatch fails (any non-2xx provider response, timeout, or connection error):

```
Route "gpt4o-mini" fails
  → Check fallback_chain: ["gpt4o", "claude-haiku"]
  → Attempt "gpt4o"
    → Fails
  → Attempt "claude-haiku"
    → Succeeds
  → Return response with route_key_used: "claude-haiku", fallback_attempt: 2
```

Each fallback attempt is independently logged with its own outcome. If all fallbacks fail, the final 502 response includes the full attempt chain.

A fallback chain of `[]` (empty) means: no fallback, fail immediately. Some routes should be configured this way deliberately.

---

## 13. Session & Caller Identity

### Caller Token

Every request must include a `caller_token` — a pre-issued bearer token that identifies the calling process. Tokens are not user identities; they are process identities.

```rust
pub struct CallerIdentity {
    pub caller_id: String,         // Stable label: "wheelhouse-hub", "cli-test", "benchmark-runner"
    pub token_hash: String,        // SHA-256 of the bearer token; token itself never stored
    pub issued_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub allowed_routes: Vec<String>, // Empty = all routes allowed; populated = allowlist
    pub active: bool,
}
```

Tokens are issued via CLI and stored (as hashes) in the route store database:

```
cloak token issue --caller-id wheelhouse-hub --routes "*"
cloak token issue --caller-id benchmark-runner --routes "gpt4o-mini,claude-haiku"
cloak token revoke --caller-id benchmark-runner
cloak token list
```

Validation at the inbound sanitizer: token hash must exist, not be expired, and be active. Invalid token = 401, logged, WARN alert.

`caller_id` is stamped on every log record, cost record, and alert. Attribution without it is impossible in a multi-caller setup.

---

## 14. Configuration & Versioning

### `cloak.toml`

Single TOML configuration file. All sections:

```toml
[server]
  host = "127.0.0.1"
  port = 8800
  admin_port = 8801       # Admin endpoints on separate port; not exposed externally
  tls_cert_path = ""      # Empty = no TLS (use Cloudflare Tunnel or Tailscale for transport security)
  request_timeout_secs = 30
  drain_timeout_secs = 60

[database]
  operational_db_path = "/var/cloak/cloak_logs.db"
  audit_db_path       = "/var/cloak/cloak_audit.db"
  route_store_path    = "/var/cloak/cloak_routes.db"
  operational_retention_days = 90

[budgets]
  global_daily_usd   = 20.00
  per_caller_daily_usd = 5.00    # Default; overridden per-caller
  per_route_daily_usd  = 10.00   # Default; overridden per-route

[health_probing]
  enabled = true
  default_interval_secs = 300
  probe_timeout_secs    = 10

[deduplication]
  enabled        = true
  window_secs    = 30

[kill_switch]
  auto_drain_on_consecutive_criticals = 5
  auto_halt_on_credential_scrub       = true
  auto_drain_on_global_budget_hit     = false

[alerts]
  # ... (see section 9)

[rate_limits]
  # ... (see section 11)
```

### Config Versioning

`cloak.toml` is not versioned automatically — it lives in version control (git). Route store versioning (per-route history) is managed internally by the database, as described in section 4.

On startup, Cloak validates the full config against a schema before proceeding. Invalid config = startup failure with a descriptive error. No partial startup.

Hot-reload: `cloak config reload` — re-reads `cloak.toml` and applies non-structural changes (alert routing, budget ceilings, rate limits) without restart. Route store changes take effect immediately. Server/database path changes require restart.

---

## 15. Drain Mode & Graceful Shutdown

On receiving a DRAIN signal:

1. Stop accepting new connections on the request port
2. Allow in-flight requests to complete (up to `drain_timeout_secs`)
3. Flush all pending log writes
4. Write DRAIN event to audit log
5. Dispatch CRITICAL alert
6. Transition to HALTED

On receiving a HALT signal directly:

1. Close request port immediately
2. In-flight requests receive 503 (connection close); they are logged as `outcome: halted`
3. Flush log buffer (best effort; non-blocking)
4. Write HALT event to audit log
5. Dispatch CRITICAL alert
6. Process exits

SIGTERM is mapped to DRAIN. SIGKILL is handled by the OS (no Cloak control). SIGINT (Ctrl-C during development) is mapped to DRAIN with a 5-second timeout, then HALT.

---

## 16. API Surface

### Request Port (default :8800)

All endpoints require `Authorization: Bearer <caller_token>`.

| Method | Path | Description |
|---|---|---|
| `POST` | `/dispatch` | Submit a prompt for routing |
| `GET` | `/health` | Liveness check (returns 200 or 503) |
| `GET` | `/routes` | List active routes (no credentials in response) |
| `GET` | `/routes/{route_key}` | Route details |

### Admin Port (default :8801)

Separate port, not exposed externally. Requires admin token (separate from caller tokens).

| Method | Path | Description |
|---|---|---|
| `GET` | `/admin/status` | Full system status |
| `DELETE` | `/admin/kill` | Trigger kill switch (body: `{"mode": "drain"\|"halt"}`) |
| `POST` | `/admin/resume` | Resume from DRAIN or HALTED |
| `POST` | `/admin/probe/{route_key}` | Trigger immediate health probe |
| `POST` | `/admin/config/reload` | Hot-reload config |
| `GET` | `/admin/cost/summary` | Cost summary |
| `GET` | `/admin/budget` | Current budget state |
| `POST` | `/admin/budget` | Update budget ceilings |

### Dispatch Request Schema

```json
{
  "request_id": "3f4a1b2c-...",
  "route_key": "claude-sonnet",
  "prompt": "...",
  "caller_metadata": {
    "caller_id": "wheelhouse-hub",
    "session_id": "optional-session-label"
  },
  "options": {
    "max_tokens": 1024,
    "temperature": 0.7
  }
}
```

### Dispatch Response Schema

```json
{
  "request_id": "3f4a1b2c-...",
  "status": "success",
  "route_key_used": "claude-sonnet",
  "fallback_triggered": false,
  "response": "...",
  "usage": {
    "input_tokens": 312,
    "output_tokens": 148,
    "total_tokens": 460,
    "estimated_cost_usd": 0.000892
  },
  "latency_ms": 1243,
  "timestamp": "2026-03-21T14:32:07Z"
}
```

---

## 17. Data Structures

### Core Types

```rust
pub struct InboundRequest {
    pub request_id: Uuid,
    pub route_key: String,
    pub prompt: String,
    pub caller_id: String,
    pub session_id: Option<String>,
    pub options: RequestOptions,
}

pub struct RequestOptions {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stop_sequences: Option<Vec<String>>,
}

pub struct SanitizedRequest {
    pub request_id: Uuid,
    pub route_key: String,
    pub prompt: String,              // After inbound sanitization
    pub caller_id: String,
    pub session_id: Option<String>,
    pub options: RequestOptions,
    pub inbound_hash: String,        // SHA-256 of prompt post-sanitization
    pub received_at: DateTime<Utc>,
}

pub struct ProviderResponse {
    pub request_id: Uuid,
    pub raw_response: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub provider_latency_ms: u64,
    pub route_key: String,
    pub fallback_attempt: u8,
}

pub struct OutboundResponse {
    pub request_id: Uuid,
    pub status: ResponseStatus,
    pub route_key_used: String,
    pub fallback_triggered: bool,
    pub response: Option<String>,    // None on error
    pub usage: Option<UsageSummary>,
    pub error: Option<GatewayError>,
    pub latency_ms: u64,
    pub timestamp: DateTime<Utc>,
}

pub enum ResponseStatus {
    Success,
    SanitizationRejected,
    RateLimited,
    Deduplicated,
    BudgetExceeded,
    RouteNotFound,
    RouteUnhealthy,
    ProviderError,
    AllFallbacksExhausted,
    GatewayHalted,
}

pub struct GatewayError {
    pub code: u16,
    pub kind: String,
    pub message: String,
    pub retryable: bool,
}

pub struct UsageSummary {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
    pub estimated_cost_usd: f64,
}
```

---

## 18. Crate Dependencies

```toml
[dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# HTTP server
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["trace", "timeout"] }

# HTTP client (provider dispatch)
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Configuration
toml = "0.8"

# Database
rusqlite = { version = "0.31", features = ["bundled", "chrono"] }

# Crypto / hashing
sha2 = "0.10"
uuid = { version = "1", features = ["v4"] }

# Time
chrono = { version = "0.4", features = ["serde"] }

# Logging / tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Error handling
thiserror = "1"
anyhow = "1"

# CLI
clap = { version = "4", features = ["derive"] }

# Regex (sanitization patterns)
regex = "1"

# Rate limiting (token bucket)
governor = "0.6"

# Environment variable handling
dotenvy = "0.15"
```

---

## 19. Directory Layout

```
cloak/
├── Cargo.toml
├── cloak.toml                      # Runtime configuration
├── cloak.example.toml              # Documented example config; committed to git
├── README.md
├── gateway.md                      # This document
│
├── src/
│   ├── main.rs                     # Entry point: parse args, load config, start server
│   ├── config.rs                   # Config loading, validation, hot-reload
│   ├── server.rs                   # Axum router setup, middleware stack
│   │
│   ├── sanitizer/
│   │   ├── mod.rs
│   │   ├── inbound.rs              # InboundSanitizer implementation
│   │   ├── outbound.rs             # OutboundSanitizer implementation
│   │   └── rules.rs                # Configurable rule definitions
│   │
│   ├── routes/
│   │   ├── mod.rs
│   │   ├── store.rs                # Route store: CRUD, versioning, fallback resolution
│   │   ├── dispatcher.rs           # Core dispatch logic, fallback chain execution
│   │   └── health.rs               # Health prober background task
│   │
│   ├── providers/
│   │   ├── mod.rs
│   │   ├── anthropic.rs            # Anthropic API client
│   │   ├── openai.rs               # OpenAI-compatible API client
│   │   └── custom.rs               # Generic HTTP provider adapter
│   │
│   ├── accounting/
│   │   ├── mod.rs
│   │   ├── cost.rs                 # Token counting, cost calculation
│   │   └── budget.rs               # Ceiling enforcement, running totals
│   │
│   ├── logging/
│   │   ├── mod.rs
│   │   ├── operational.rs          # Operational log writes and queries
│   │   └── audit.rs                # Append-only audit log
│   │
│   ├── alerts/
│   │   ├── mod.rs
│   │   ├── router.rs               # Level → destination mapping, suppression logic
│   │   ├── sms.rs                  # Telnyx SMS dispatch
│   │   └── webhook.rs              # Webhook dispatch
│   │
│   ├── kill_switch/
│   │   ├── mod.rs
│   │   └── controller.rs           # State machine: OPERATIONAL → DRAIN → HALTED
│   │
│   ├── rate_limit/
│   │   ├── mod.rs
│   │   └── limiter.rs              # Per-caller × per-route token buckets
│   │
│   ├── dedup/
│   │   ├── mod.rs
│   │   └── fingerprint.rs          # Prompt hashing and window cache
│   │
│   ├── identity/
│   │   ├── mod.rs
│   │   └── tokens.rs               # Caller token issuance, validation, storage
│   │
│   └── cli/
│       ├── mod.rs
│       └── commands.rs             # All cloak <subcommand> definitions
│
├── migrations/
│   ├── 001_operational_log.sql
│   ├── 002_audit_log.sql
│   └── 003_route_store.sql
│
└── tests/
    ├── sanitizer_tests.rs
    ├── dispatch_tests.rs
    ├── fallback_tests.rs
    └── accounting_tests.rs
```

---

## 20. Implementation TODO List

| # | Item | Module | Notes |
|---|---|---|---|
| 1 | Config loading and schema validation | `config.rs` | Full TOML parse + type-safe struct mapping; reject on any unknown field |
| 2 | SQLite initialization + migration runner | `logging/` | Run migrations on startup; WAL mode; separate operational and audit DB files |
| 3 | Route store CRUD + versioning | `routes/store.rs` | `routes` and `routes_history` tables; version increment on every write |
| 4 | Caller token issuance and validation | `identity/tokens.rs` | Hash storage only; route allowlist enforcement |
| 5 | Inbound sanitizer | `sanitizer/inbound.rs` | Schema validation, size check, encoding check, injection pattern scan |
| 6 | Outbound sanitizer | `sanitizer/outbound.rs` | Schema validation, credential scrub regex, encoding normalization |
| 7 | Anthropic provider client | `providers/anthropic.rs` | Messages API; token extraction from response |
| 8 | OpenAI-compatible provider client | `providers/openai.rs` | Covers OpenAI, Mistral, Groq, and any OpenAI-compatible endpoint |
| 9 | Route dispatcher + fallback chain | `routes/dispatcher.rs` | Primary dispatch, fallback resolution, per-attempt logging |
| 10 | Cost accountant | `accounting/cost.rs` | Per-request record; running totals; budget ceiling checks |
| 11 | Rate limiter | `rate_limit/limiter.rs` | Token bucket per caller × route; configurable windows |
| 12 | Deduplicator | `dedup/fingerprint.rs` | SHA-256 prompt fingerprint; short-window TTL cache |
| 13 | Health prober | `routes/health.rs` | Background tokio task; per-route interval; probe traffic flagged in cost records |
| 14 | Kill switch state machine | `kill_switch/controller.rs` | OPERATIONAL → DRAIN → HALTED; signal handling (SIGTERM, SIGINT) |
| 15 | Alert router | `alerts/router.rs` | Level → destination table; suppression windows; per-source overrides |
| 16 | Telnyx SMS dispatch | `alerts/sms.rs` | Telnyx REST API; message formatting; cooldown enforcement |
| 17 | Webhook alert dispatch | `alerts/webhook.rs` | HMAC-signed POST; retry on failure |
| 18 | Operational log queries | `logging/operational.rs` | Search by caller, route, outcome, window, request_id |
| 19 | Axum server + middleware | `server.rs` | Request timeout, tracing, admin port separation |
| 20 | Drain mode + graceful shutdown | `kill_switch/controller.rs` + `server.rs` | In-flight tracking; flush before exit |
| 21 | CLI command set | `cli/commands.rs` | All `cloak <subcommand>` commands listed in this doc |
| 22 | Config hot-reload | `config.rs` | Re-read and apply non-structural changes without restart |
| 23 | Audit log writes | `logging/audit.rs` | Append-only; every kill switch, sanitization reject, credential scrub trigger |
| 24 | Budget ceiling enforcement | `accounting/budget.rs` | Per-caller, per-route, global rolling windows |
| 25 | Startup validation | `main.rs` | Config valid, all DB files accessible, env vars present for all active routes |

---

*Cloak is a security boundary. Design decisions that blur its contract — adding intelligence, caching responses, inspecting content for routing — should be rejected. Its value is in what it refuses to do as much as what it does.*
