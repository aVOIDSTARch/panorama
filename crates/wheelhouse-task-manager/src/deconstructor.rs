use chrono::Utc;

use crate::types::{
    AgentBrief, AgentFate, AgentResolution, AttestationEnvelope, ResolutionCode,
};

/// Deconstruct a completed task execution into an AgentResolution.
///
/// This is the teardown path: the orchestrator calls this after an agent
/// finishes (successfully or not), providing the evidence.
pub fn teardown(
    brief: &AgentBrief,
    resolution_code: ResolutionCode,
    evidence: AttestationEnvelope,
) -> AgentResolution {
    let agent_fate = determine_fate(resolution_code, &evidence);
    let corpus_eligible = is_corpus_eligible(resolution_code, &evidence);
    let archive_ref = format!(
        "archive-{}-{}",
        brief.task_id,
        Utc::now().format("%Y%m%dT%H%M%S")
    );

    AgentResolution {
        resolution_code,
        agent_fate,
        brief_id: brief.brief_id.clone(),
        task_id: brief.task_id.clone(),
        job_id: brief.job_id.clone(),
        resolved_at: Utc::now(),
        evidence,
        archive_ref,
        corpus_eligible,
    }
}

/// Determine the agent's fate based on resolution code and evidence.
fn determine_fate(code: ResolutionCode, evidence: &AttestationEnvelope) -> AgentFate {
    match code {
        // Success cases: agent performed well
        ResolutionCode::Complete | ResolutionCode::CompleteWithWarnings => {
            if evidence.warnings.is_empty() {
                AgentFate::Persist // Keep agent hot for reuse
            } else {
                AgentFate::Standby // Cool down, reusable later
            }
        }
        ResolutionCode::PartialAccepted | ResolutionCode::Delegated | ResolutionCode::Skipped => {
            AgentFate::Standby
        }

        // Agent errors: recycle (reset state, try different agent)
        ResolutionCode::AgentCrash | ResolutionCode::AgentTimeout => AgentFate::Recycle,

        // Resource/infra errors: standby (not agent's fault)
        ResolutionCode::ResourceExhausted
        | ResolutionCode::ServiceUnavailable
        | ResolutionCode::ModelUnavailable
        | ResolutionCode::NetworkFailure
        | ResolutionCode::DependencyFailed => AgentFate::Standby,

        // Client errors: terminate (bad input, not worth keeping)
        ResolutionCode::BadTask
        | ResolutionCode::Unauthorized
        | ResolutionCode::Forbidden
        | ResolutionCode::NotFound => AgentFate::Terminate,

        // Policy violations: terminate
        ResolutionCode::PolicyViolation | ResolutionCode::ManualAbort => AgentFate::Terminate,
    }
}

/// Determine if this execution is eligible for the RefinementCorpus.
///
/// Only successful executions with validated output are promoted.
fn is_corpus_eligible(code: ResolutionCode, evidence: &AttestationEnvelope) -> bool {
    code == ResolutionCode::Complete && evidence.output.is_some() && evidence.warnings.is_empty()
}
