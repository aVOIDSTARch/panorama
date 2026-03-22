use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Agent execution tier in the Wheelhouse hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AgentTier {
    Hub,
    Orchestrator,
    Specialist,
    Micro,
}

/// Output format contract for task results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum OutputFormat {
    Text,
    Json,
    Structured,
    Binary,
}

/// How task success is validated.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ValidationMode {
    Auto,
    Human,
    Consensus,
    Foreman,
}

/// What happens to an agent after task completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AgentFate {
    Persist,
    Standby,
    Recycle,
    Terminate,
}

/// Resolution codes — HTTP-analogous classification (18 codes, 6 categories).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionCode {
    // 2xx — Success
    Complete,
    CompleteWithWarnings,
    PartialAccepted,
    Delegated,
    Skipped,
    // 4xx — Client/Input Error
    BadTask,
    Unauthorized,
    Forbidden,
    NotFound,
    // 5xx — Agent Execution Error
    AgentCrash,
    AgentTimeout,
    ResourceExhausted,
    DependencyFailed,
    // 6xx — Infrastructure Error
    ServiceUnavailable,
    ModelUnavailable,
    NetworkFailure,
    // 7xx — Policy / Governance
    PolicyViolation,
    ManualAbort,
}

impl ResolutionCode {
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            Self::Complete
                | Self::CompleteWithWarnings
                | Self::PartialAccepted
                | Self::Delegated
                | Self::Skipped
        )
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::AgentTimeout
                | Self::ResourceExhausted
                | Self::ServiceUnavailable
                | Self::ModelUnavailable
                | Self::NetworkFailure
        )
    }
}

/// Resource budget for a task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceBudget {
    pub max_tokens: u64,
    pub max_wall_clock_s: u64,
    pub token_warn_pct: f32,
    pub wall_clock_warn_pct: f32,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            max_tokens: 100_000,
            max_wall_clock_s: 300,
            token_warn_pct: 0.8,
            wall_clock_warn_pct: 0.8,
        }
    }
}

/// Retry policy for failed tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff_base_s: u64,
    pub backoff_max_s: u64,
    pub retryable_codes: Vec<ResolutionCode>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            backoff_base_s: 5,
            backoff_max_s: 60,
            retryable_codes: vec![
                ResolutionCode::AgentTimeout,
                ResolutionCode::ResourceExhausted,
                ResolutionCode::ServiceUnavailable,
                ResolutionCode::ModelUnavailable,
                ResolutionCode::NetworkFailure,
            ],
        }
    }
}

/// Success criteria contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessContract {
    pub criteria: Vec<String>,
    pub validation_mode: ValidationMode,
    pub confidence_floor: f32,
}

/// Output shape contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputContract {
    pub format: OutputFormat,
    pub schema: Option<serde_json::Value>,
    pub max_size_bytes: Option<u64>,
}

/// The input the orchestrator hands to TaskLifecycleService.create().
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskObject {
    pub task_id: String,
    pub job_id: String,
    pub description: String,
    pub success_condition: SuccessContract,
    pub output_contract: OutputContract,
    pub agent_tier: AgentTier,
    pub resource_budget: ResourceBudget,
    pub retry_policy: RetryPolicy,
    pub skill_hints: Vec<String>,
    pub knowledge_hints: Vec<String>,
}

/// The context envelope constructed by TaskLifecycleService and delivered to a spawned agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBrief {
    // Identity
    pub brief_id: String,
    pub correlation_id: String,
    pub task_id: String,
    pub job_id: String,
    pub created_at: DateTime<Utc>,
    pub brief_hash: String,

    // Agent targeting
    pub agent_tier: AgentTier,
    pub model_id: String,
    pub plate_id: Option<String>,

    // Task definition
    pub task_object: TaskObject,

    // Resource budget
    pub resource_budget: ResourceBudget,

    // Skill and knowledge hints (resolved from hints)
    pub skill_ids: Vec<String>,
    pub knowledge_refs: Vec<String>,

    // Constraints
    pub constraints: Vec<String>,
}

/// Evidence record for a completed task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationEnvelope {
    pub output: Option<serde_json::Value>,
    pub output_hash: Option<String>,
    pub tokens_used: u64,
    pub wall_clock_s: u64,
    pub retries: u32,
    pub warnings: Vec<String>,
}

/// The terminal record produced by TaskLifecycleService.teardown().
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResolution {
    pub resolution_code: ResolutionCode,
    pub agent_fate: AgentFate,
    pub brief_id: String,
    pub task_id: String,
    pub job_id: String,
    pub resolved_at: DateTime<Utc>,
    pub evidence: AttestationEnvelope,
    pub archive_ref: String,
    pub corpus_eligible: bool,
}
