use std::sync::Arc;

use wheelhouse_agent_lifecycle::{AgentPool, AgentStatus};
use wheelhouse_task_manager::types::{AttestationEnvelope, ResolutionCode};
use wheelhouse_task_manager::TaskLifecycleService;

use crate::cascade;
use crate::job::{Job, JobPriority, JobStatus};

/// The Orchestrator decomposes jobs into tasks and dispatches them to agents.
pub struct Orchestrator {
    task_service: TaskLifecycleService,
    agent_pool: Arc<AgentPool>,
}

impl Orchestrator {
    pub fn new(default_model_id: &str, agent_pool: Arc<AgentPool>) -> Self {
        Self {
            task_service: TaskLifecycleService::new(default_model_id),
            agent_pool,
        }
    }

    /// Submit a new job for orchestration.
    ///
    /// Creates the job, decomposes into tasks, dispatches to agents.
    pub async fn submit_job(
        &self,
        description: &str,
        requester: &str,
        priority: JobPriority,
    ) -> Result<Job, OrchestratorError> {
        let mut job = Job::new(description, requester, priority);
        job.status = JobStatus::Active;

        // Estimate complexity and determine tier
        let complexity = cascade::estimate_complexity(description);
        let tier = cascade::route_to_tier(complexity);

        tracing::info!(
            job_id = %job.job_id,
            complexity = ?complexity,
            tier = ?tier,
            "Job submitted, routing to {tier:?}"
        );

        // Create initial task from the job description
        let task = job.decompose_simple(description, tier).clone();

        // Create the agent brief
        let brief = self
            .task_service
            .create(&task)
            .map_err(|e| OrchestratorError::TaskCreation(e.to_string()))?;

        // Find or spawn an agent
        let agent_id = match self.agent_pool.find_idle(tier) {
            Some(id) => id,
            None => {
                // Spawn a new agent
                self.agent_pool
                    .spawn(tier, &brief.model_id)
                    .map_err(|e| OrchestratorError::AgentSpawn(e.to_string()))?
            }
        };

        // Assign the task to the agent
        self.agent_pool
            .assign_task(&agent_id, &task.task_id, &brief.brief_id)
            .map_err(|e| OrchestratorError::AgentAssignment(e.to_string()))?;

        tracing::info!(
            job_id = %job.job_id,
            task_id = %task.task_id,
            agent_id = %agent_id,
            "Task dispatched to agent"
        );

        Ok(job)
    }

    /// Complete a task and record the result.
    pub fn complete_task(
        &self,
        job: &mut Job,
        task_index: usize,
        agent_id: &str,
        resolution_code: ResolutionCode,
        evidence: AttestationEnvelope,
    ) -> Result<(), OrchestratorError> {
        let task_id = job.tasks.get(task_index)
            .map(|t| t.task_id.clone())
            .ok_or_else(|| OrchestratorError::TaskNotFound(task_index))?;

        // Find the brief for this task
        let brief = self.task_service.create(
            job.tasks.get(task_index).unwrap()
        ).map_err(|e| OrchestratorError::TaskCreation(e.to_string()))?;

        // Teardown the task
        let resolution = self.task_service.teardown(&brief, resolution_code, evidence);

        // Apply agent fate
        wheelhouse_agent_lifecycle::apply_fate(
            &self.agent_pool,
            agent_id,
            resolution.agent_fate,
            resolution.evidence.tokens_used,
        )
        .map_err(|e| OrchestratorError::AgentFate(e.to_string()))?;

        // Record proof if successful
        if resolution.resolution_code.is_success() {
            job.record_proof(task_index, &task_id);
        }

        // Check if job is complete
        if job.is_provably_complete() {
            job.status = JobStatus::Completed;
            tracing::info!(job_id = %job.job_id, "Job provably complete");
        }

        Ok(())
    }

    /// Get agent pool stats.
    pub fn pool_stats(&self) -> PoolStats {
        PoolStats {
            total: self.agent_pool.len(),
            idle: self.agent_pool.count_by_status(AgentStatus::Idle),
            active: self.agent_pool.count_by_status(AgentStatus::Active),
            retiring: self.agent_pool.count_by_status(AgentStatus::Retiring),
        }
    }
}

pub struct PoolStats {
    pub total: usize,
    pub idle: usize,
    pub active: usize,
    pub retiring: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error("Task creation failed: {0}")]
    TaskCreation(String),
    #[error("Agent spawn failed: {0}")]
    AgentSpawn(String),
    #[error("Agent assignment failed: {0}")]
    AgentAssignment(String),
    #[error("Agent fate application failed: {0}")]
    AgentFate(String),
    #[error("Task not found at index {0}")]
    TaskNotFound(usize),
}
