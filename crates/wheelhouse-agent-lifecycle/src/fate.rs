use wheelhouse_task_manager::types::AgentFate;

use crate::pool::{AgentPool, AgentPoolError};

/// Apply the agent's fate after task teardown.
///
/// Maps AgentFate to pool operations:
///   Persist  -> keep Idle (no action)
///   Standby  -> keep Idle (could add cooldown in future)
///   Recycle  -> terminate + spawn new
///   Terminate -> terminate
pub fn apply_fate(
    pool: &AgentPool,
    agent_id: &str,
    fate: AgentFate,
    tokens_used: u64,
) -> Result<(), AgentPoolError> {
    // First, complete the current task
    pool.complete_task(agent_id, tokens_used)?;

    match fate {
        AgentFate::Persist | AgentFate::Standby => {
            // Agent stays in pool as Idle, ready for next task
            tracing::debug!(agent_id = %agent_id, fate = ?fate, "Agent remains in pool");
            Ok(())
        }
        AgentFate::Recycle => {
            // Terminate and remove from pool (orchestrator will spawn replacement if needed)
            pool.terminate(agent_id)?;
            tracing::info!(agent_id = %agent_id, "Agent recycled (removed from pool)");
            Ok(())
        }
        AgentFate::Terminate => {
            pool.terminate(agent_id)?;
            tracing::info!(agent_id = %agent_id, "Agent terminated");
            Ok(())
        }
    }
}
