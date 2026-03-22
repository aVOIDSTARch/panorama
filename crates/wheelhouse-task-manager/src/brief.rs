use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::types::{AgentBrief, TaskObject};

/// Construct an AgentBrief from a validated TaskObject.
///
/// The brief is the immutable context envelope delivered to a spawned agent.
/// Once constructed, the brief_hash prevents tampering.
pub fn construct_brief(task: &TaskObject, model_id: &str) -> AgentBrief {
    let brief_id = uuid::Uuid::new_v4().to_string();
    let correlation_id = format!("{}-{}-{}", task.job_id, task.task_id, &brief_id[..8]);

    let mut brief = AgentBrief {
        brief_id,
        correlation_id,
        task_id: task.task_id.clone(),
        job_id: task.job_id.clone(),
        created_at: Utc::now(),
        brief_hash: String::new(), // computed below
        agent_tier: task.agent_tier,
        model_id: model_id.to_string(),
        plate_id: None,
        task_object: task.clone(),
        resource_budget: task.resource_budget.clone(),
        skill_ids: task.skill_hints.clone(),
        knowledge_refs: task.knowledge_hints.clone(),
        constraints: Vec::new(),
    };

    brief.brief_hash = compute_brief_hash(&brief);
    brief
}

/// SHA-256 hash of the brief's critical fields for tamper detection.
fn compute_brief_hash(brief: &AgentBrief) -> String {
    let mut hasher = Sha256::new();
    hasher.update(brief.brief_id.as_bytes());
    hasher.update(brief.task_id.as_bytes());
    hasher.update(brief.job_id.as_bytes());
    hasher.update(brief.model_id.as_bytes());
    hasher.update(brief.task_object.description.as_bytes());
    let hash = hasher.finalize();
    hash.iter().fold(String::new(), |mut s, b| {
        use std::fmt::Write;
        write!(s, "{b:02x}").unwrap();
        s
    })
}

/// Verify a brief has not been tampered with.
pub fn verify_brief_integrity(brief: &AgentBrief) -> bool {
    let expected = compute_brief_hash(brief);
    brief.brief_hash == expected
}
