use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use wheelhouse_task_manager::types::{AgentTier, TaskObject};

/// A Job is a high-level orchestration plan that decomposes into Tasks.
///
/// Jobs own their proof_chain and are only complete when all tasks resolve
/// successfully and the chain is provably complete.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub job_id: String,
    pub description: String,
    pub requester: String,
    pub priority: JobPriority,
    pub created_at: DateTime<Utc>,
    pub status: JobStatus,
    pub tasks: Vec<TaskObject>,
    pub proof_chain: Vec<Option<String>>, // task_id of completed proof, None = pending
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Active,
    Completed,
    Failed,
    Cancelled,
}

impl Job {
    /// Create a new job from an external request.
    pub fn new(description: &str, requester: &str, priority: JobPriority) -> Self {
        Self {
            job_id: uuid::Uuid::new_v4().to_string(),
            description: description.to_string(),
            requester: requester.to_string(),
            priority,
            created_at: Utc::now(),
            status: JobStatus::Pending,
            tasks: Vec::new(),
            proof_chain: Vec::new(),
        }
    }

    /// Add a task to this job's plan.
    pub fn add_task(&mut self, task: TaskObject) {
        self.proof_chain.push(None);
        self.tasks.push(task);
    }

    /// Record a task completion in the proof chain.
    pub fn record_proof(&mut self, task_index: usize, task_id: &str) -> bool {
        if task_index < self.proof_chain.len() {
            self.proof_chain[task_index] = Some(task_id.to_string());
            true
        } else {
            false
        }
    }

    /// Check if all proofs are present (job is provably complete).
    pub fn is_provably_complete(&self) -> bool {
        !self.proof_chain.is_empty() && self.proof_chain.iter().all(|p| p.is_some())
    }

    /// Decompose a high-level request into tasks.
    ///
    /// This is a simplified decomposition — in a full system, the Orchestrator
    /// would use an LLM to plan the decomposition.
    pub fn decompose_simple(
        &mut self,
        description: &str,
        tier: AgentTier,
    ) -> &TaskObject {
        use wheelhouse_task_manager::types::*;

        let task = TaskObject {
            task_id: uuid::Uuid::new_v4().to_string(),
            job_id: self.job_id.clone(),
            description: description.to_string(),
            success_condition: SuccessContract {
                criteria: vec!["Task completed as described".into()],
                validation_mode: ValidationMode::Auto,
                confidence_floor: 0.8,
            },
            output_contract: OutputContract {
                format: OutputFormat::Json,
                schema: None,
                max_size_bytes: Some(1_000_000),
            },
            agent_tier: tier,
            resource_budget: ResourceBudget::default(),
            retry_policy: RetryPolicy::default(),
            skill_hints: Vec::new(),
            knowledge_hints: Vec::new(),
        };

        self.add_task(task);
        self.tasks.last().unwrap()
    }
}
