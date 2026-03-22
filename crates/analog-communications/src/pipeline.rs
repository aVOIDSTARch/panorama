// Pipeline orchestration — coordinates the full inbound flow.
//
// Currently thin: the inbound handler does sanitization + identity inline.
// This module exists for future expansion (voice transcription pipeline,
// multi-channel routing, etc.).

/// Pipeline status for health reporting.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PipelineStatus {
    pub sms_active: bool,
    pub voice_active: bool,
    pub messages_processed: u64,
    pub messages_quarantined: u64,
}

impl Default for PipelineStatus {
    fn default() -> Self {
        Self {
            sms_active: true,
            voice_active: false,
            messages_processed: 0,
            messages_quarantined: 0,
        }
    }
}
