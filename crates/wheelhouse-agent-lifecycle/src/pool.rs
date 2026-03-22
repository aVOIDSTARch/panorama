use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use wheelhouse_task_manager::types::AgentTier;

/// Agent lifecycle states.
///
/// Spawn -> Idle -> Active -> Retiring -> Dead
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// Agent spawned but not yet assigned work.
    Spawning,
    /// Agent is idle and available for task assignment.
    Idle,
    /// Agent is actively executing a task.
    Active,
    /// Agent is winding down (completing current work before shutdown).
    Retiring,
    /// Agent has terminated.
    Dead,
}

/// A handle to a managed agent in the pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHandle {
    pub agent_id: String,
    pub tier: AgentTier,
    pub model_id: String,
    pub status: AgentStatus,
    pub current_task_id: Option<String>,
    pub current_brief_id: Option<String>,
    pub spawned_at: DateTime<Utc>,
    pub last_active_at: Option<DateTime<Utc>>,
    pub tasks_completed: u32,
    pub total_tokens_used: u64,
}

/// Agent pool — manages the set of live agents across all tiers.
///
/// Conservative defaults, fail-loudly semantics.
pub struct AgentPool {
    agents: Arc<DashMap<String, AgentHandle>>,
    max_agents: usize,
}

impl AgentPool {
    pub fn new(max_agents: usize) -> Self {
        Self {
            agents: Arc::new(DashMap::new()),
            max_agents,
        }
    }

    /// Spawn a new agent in the pool.
    pub fn spawn(
        &self,
        tier: AgentTier,
        model_id: &str,
    ) -> Result<String, AgentPoolError> {
        if self.agents.len() >= self.max_agents {
            return Err(AgentPoolError::PoolFull {
                max: self.max_agents,
            });
        }

        let agent_id = uuid::Uuid::new_v4().to_string();
        let handle = AgentHandle {
            agent_id: agent_id.clone(),
            tier,
            model_id: model_id.to_string(),
            status: AgentStatus::Idle,
            current_task_id: None,
            current_brief_id: None,
            spawned_at: Utc::now(),
            last_active_at: None,
            tasks_completed: 0,
            total_tokens_used: 0,
        };

        self.agents.insert(agent_id.clone(), handle);
        tracing::info!(agent_id = %agent_id, tier = ?tier, "Agent spawned");
        Ok(agent_id)
    }

    /// Assign a task to an idle agent. Transitions Idle -> Active.
    pub fn assign_task(
        &self,
        agent_id: &str,
        task_id: &str,
        brief_id: &str,
    ) -> Result<(), AgentPoolError> {
        let mut agent = self
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| AgentPoolError::NotFound(agent_id.to_string()))?;

        if agent.status != AgentStatus::Idle {
            return Err(AgentPoolError::InvalidTransition {
                agent_id: agent_id.to_string(),
                from: agent.status,
                to: AgentStatus::Active,
            });
        }

        agent.status = AgentStatus::Active;
        agent.current_task_id = Some(task_id.to_string());
        agent.current_brief_id = Some(brief_id.to_string());
        agent.last_active_at = Some(Utc::now());
        Ok(())
    }

    /// Complete a task. Transitions Active -> Idle.
    pub fn complete_task(
        &self,
        agent_id: &str,
        tokens_used: u64,
    ) -> Result<(), AgentPoolError> {
        let mut agent = self
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| AgentPoolError::NotFound(agent_id.to_string()))?;

        if agent.status != AgentStatus::Active {
            return Err(AgentPoolError::InvalidTransition {
                agent_id: agent_id.to_string(),
                from: agent.status,
                to: AgentStatus::Idle,
            });
        }

        agent.status = AgentStatus::Idle;
        agent.current_task_id = None;
        agent.current_brief_id = None;
        agent.tasks_completed += 1;
        agent.total_tokens_used += tokens_used;
        Ok(())
    }

    /// Retire an agent. Transitions to Retiring (then Dead after completion).
    pub fn retire(&self, agent_id: &str) -> Result<(), AgentPoolError> {
        let mut agent = self
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| AgentPoolError::NotFound(agent_id.to_string()))?;

        agent.status = AgentStatus::Retiring;
        tracing::info!(agent_id = %agent_id, "Agent retiring");
        Ok(())
    }

    /// Terminate an agent. Removes from pool.
    pub fn terminate(&self, agent_id: &str) -> Result<AgentHandle, AgentPoolError> {
        let (_, handle) = self
            .agents
            .remove(agent_id)
            .ok_or_else(|| AgentPoolError::NotFound(agent_id.to_string()))?;
        tracing::info!(
            agent_id = %agent_id,
            tasks_completed = handle.tasks_completed,
            "Agent terminated"
        );
        Ok(handle)
    }

    /// Find an idle agent for a given tier.
    pub fn find_idle(&self, tier: AgentTier) -> Option<String> {
        self.agents
            .iter()
            .find(|entry| entry.tier == tier && entry.status == AgentStatus::Idle)
            .map(|entry| entry.agent_id.clone())
    }

    /// Get a snapshot of all agents.
    pub fn list(&self) -> Vec<AgentHandle> {
        self.agents.iter().map(|entry| entry.clone()).collect()
    }

    /// Count agents by status.
    pub fn count_by_status(&self, status: AgentStatus) -> usize {
        self.agents
            .iter()
            .filter(|entry| entry.status == status)
            .count()
    }

    /// Total number of agents in the pool.
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AgentPoolError {
    #[error("Agent not found: {0}")]
    NotFound(String),

    #[error("Agent pool full (max {max})")]
    PoolFull { max: usize },

    #[error("Invalid state transition for agent {agent_id}: {from:?} -> {to:?}")]
    InvalidTransition {
        agent_id: String,
        from: AgentStatus,
        to: AgentStatus,
    },
}

impl From<AgentPoolError> for panorama_errors::PanoramaError {
    fn from(err: AgentPoolError) -> Self {
        let (code, detail) = match &err {
            AgentPoolError::NotFound(id) => ("WH-003", Some(id.clone())),
            AgentPoolError::PoolFull { max } => {
                ("WH-004", Some(format!("max capacity: {max}")))
            }
            AgentPoolError::InvalidTransition {
                agent_id,
                from,
                to,
            } => (
                "WH-005",
                Some(format!("agent {agent_id}: {from:?} -> {to:?}")),
            ),
        };
        panorama_errors::PanoramaError::from_code(code, "wheelhouse", detail)
    }
}
