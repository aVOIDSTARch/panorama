use wheelhouse_task_manager::types::AgentTier;

/// Cascade routing — determines which tier should handle a request.
///
/// The Hub tier applies the API-tier frugality invariant: always try the
/// cheapest capable tier first, escalating only when needed.
pub fn route_to_tier(complexity: TaskComplexity) -> AgentTier {
    match complexity {
        TaskComplexity::Trivial => AgentTier::Micro,
        TaskComplexity::Simple => AgentTier::Specialist,
        TaskComplexity::Moderate => AgentTier::Specialist,
        TaskComplexity::Complex => AgentTier::Orchestrator,
        TaskComplexity::MultiStep => AgentTier::Orchestrator,
    }
}

/// Estimated task complexity — used by the Hub for initial routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskComplexity {
    /// Single lookup or formatting operation.
    Trivial,
    /// Single-step task with clear input/output.
    Simple,
    /// Requires reasoning but single-pass.
    Moderate,
    /// Requires multi-step reasoning or tool use.
    Complex,
    /// Requires decomposition into sub-tasks.
    MultiStep,
}

/// Estimate complexity from task description heuristics.
///
/// In a full system, this would use an LLM classifier. For now,
/// simple keyword heuristics as a placeholder.
pub fn estimate_complexity(description: &str) -> TaskComplexity {
    let desc_lower = description.to_lowercase();
    let word_count = description.split_whitespace().count();

    if word_count < 10 {
        return TaskComplexity::Trivial;
    }

    if desc_lower.contains("decompose")
        || desc_lower.contains("multi-step")
        || desc_lower.contains("plan and execute")
        || desc_lower.contains("coordinate")
    {
        return TaskComplexity::MultiStep;
    }

    if desc_lower.contains("analyze")
        || desc_lower.contains("compare")
        || desc_lower.contains("evaluate")
        || desc_lower.contains("debug")
    {
        return TaskComplexity::Complex;
    }

    if desc_lower.contains("create")
        || desc_lower.contains("implement")
        || desc_lower.contains("write")
        || desc_lower.contains("build")
    {
        return TaskComplexity::Moderate;
    }

    TaskComplexity::Simple
}
