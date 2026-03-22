pub mod brief;
pub mod deconstructor;
pub mod types;

use types::{
    AgentBrief, AgentResolution, AttestationEnvelope, ResolutionCode, TaskObject,
};

/// The two entry-point interface the orchestrator uses.
///
/// ```text
/// TaskLifecycleService.create(task)     -> AgentBrief
/// TaskLifecycleService.teardown(brief, result) -> AgentResolution
/// ```
pub struct TaskLifecycleService {
    default_model_id: String,
}

impl TaskLifecycleService {
    pub fn new(default_model_id: impl Into<String>) -> Self {
        Self {
            default_model_id: default_model_id.into(),
        }
    }

    /// Create an AgentBrief from a TaskObject.
    ///
    /// Validates the task, resolves skill/knowledge hints, constructs the
    /// immutable context envelope, and returns it ready for agent dispatch.
    pub fn create(&self, task: &TaskObject) -> Result<AgentBrief, TaskValidationError> {
        validate_task(task)?;
        let brief = brief::construct_brief(task, &self.default_model_id);
        tracing::info!(
            task_id = %task.task_id,
            job_id = %task.job_id,
            brief_id = %brief.brief_id,
            "Brief constructed"
        );
        Ok(brief)
    }

    /// Tear down a completed task execution.
    ///
    /// Records the resolution, determines agent fate, and returns
    /// the terminal AgentResolution for archive.
    pub fn teardown(
        &self,
        brief: &AgentBrief,
        resolution_code: ResolutionCode,
        evidence: AttestationEnvelope,
    ) -> AgentResolution {
        let resolution = deconstructor::teardown(brief, resolution_code, evidence);
        tracing::info!(
            task_id = %resolution.task_id,
            resolution = ?resolution.resolution_code,
            fate = ?resolution.agent_fate,
            "Task teardown complete"
        );
        resolution
    }
}

/// Validate a TaskObject before brief construction.
fn validate_task(task: &TaskObject) -> Result<(), TaskValidationError> {
    if task.task_id.is_empty() {
        return Err(TaskValidationError::MissingField("task_id".into()));
    }
    if task.job_id.is_empty() {
        return Err(TaskValidationError::MissingField("job_id".into()));
    }
    if task.description.is_empty() {
        return Err(TaskValidationError::MissingField("description".into()));
    }
    if task.success_condition.criteria.is_empty() {
        return Err(TaskValidationError::MissingField(
            "success_condition.criteria".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum TaskValidationError {
    #[error("Missing required field: {0}")]
    MissingField(String),
    #[error("Invalid value: {0}")]
    InvalidValue(String),
}

impl From<TaskValidationError> for panorama_errors::PanoramaError {
    fn from(err: TaskValidationError) -> Self {
        let (code, detail) = match &err {
            TaskValidationError::MissingField(f) => ("WH-001", Some(f.clone())),
            TaskValidationError::InvalidValue(v) => ("WH-002", Some(v.clone())),
        };
        panorama_errors::PanoramaError::from_code(code, "wheelhouse", detail)
    }
}
