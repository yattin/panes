use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub(super) enum ContentBlock {
    #[serde(rename = "text")]
    Text {
        content: String,
        #[serde(rename = "planMode", skip_serializing_if = "Option::is_none")]
        plan_mode: Option<bool>,
        #[serde(rename = "isSteer", skip_serializing_if = "Option::is_none")]
        is_steer: Option<bool>,
    },

    #[serde(rename = "diff")]
    Diff { diff: String, scope: String },

    #[serde(rename = "action")]
    Action {
        #[serde(rename = "actionId")]
        action_id: String,
        #[serde(rename = "engineActionId", skip_serializing_if = "Option::is_none")]
        engine_action_id: Option<String>,
        #[serde(rename = "actionType")]
        action_type: String,
        summary: String,
        #[serde(rename = "displayLabel", skip_serializing_if = "Option::is_none")]
        display_label: Option<String>,
        #[serde(rename = "displaySubtitle", skip_serializing_if = "Option::is_none")]
        display_subtitle: Option<String>,
        details: Box<RawValue>,
        #[serde(rename = "outputChunks")]
        output_chunks: Vec<ActionOutputChunk>,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<ActionBlockResult>,
    },

    #[serde(rename = "approval")]
    Approval {
        #[serde(rename = "approvalId")]
        approval_id: String,
        #[serde(rename = "actionType")]
        action_type: String,
        summary: String,
        details: Box<RawValue>,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        decision: Option<String>,
    },

    #[serde(rename = "thinking")]
    Thinking {
        content: String,
        #[serde(rename = "startedAt", skip_serializing_if = "Option::is_none")]
        started_at: Option<f64>,
        #[serde(rename = "durationMs", skip_serializing_if = "Option::is_none")]
        duration_ms: Option<f64>,
    },

    #[serde(rename = "notice")]
    Notice {
        kind: String,
        level: String,
        title: String,
        message: String,
    },

    #[serde(rename = "error")]
    Error { message: String },

    #[serde(rename = "attachment")]
    Attachment {
        #[serde(rename = "fileName")]
        file_name: String,
        #[serde(rename = "filePath")]
        file_path: String,
        #[serde(rename = "sizeBytes")]
        size_bytes: u64,
        #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },

    #[serde(rename = "skill")]
    Skill { name: String, path: String },

    #[serde(rename = "mention")]
    Mention { name: String, path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ActionOutputChunk {
    pub(super) stream: String,
    pub(super) content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ActionBlockResult {
    pub(super) success: bool,
    pub(super) output: Option<String>,
    pub(super) error: Option<String>,
    pub(super) diff: Option<String>,
    pub(super) duration_ms: u64,
}

#[derive(Default)]
pub(super) struct EventProgress {
    pub(super) message_status: Option<MessageStatusDto>,
    pub(super) thread_status: Option<ThreadStatusDto>,
    pub(super) token_usage: Option<(u64, u64)>,
    pub(super) turn_model_id: Option<String>,
    pub(super) blocks_changed: bool,
    pub(super) force_persist: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ThreadUpdatedEvent {
    pub(super) thread_id: String,
    pub(super) workspace_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) thread: Option<ThreadDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ChatTurnFinishedEvent {
    pub(super) thread_id: String,
    pub(super) workspace_id: String,
    pub(super) engine_id: String,
    pub(super) thread_title: String,
    pub(super) status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAttachmentPayload {
    pub file_name: String,
    pub file_path: String,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentPreviewPayload {
    pub mime_type: String,
    pub data_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ChatInputItemPayload {
    Text { text: String },
    Skill { name: String, path: String },
    Mention { name: String, path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CodexReviewTargetPayload {
    #[serde(rename = "uncommittedChanges")]
    UncommittedChanges,
    #[serde(rename = "baseBranch")]
    BaseBranch { branch: String },
    #[serde(rename = "commit")]
    Commit {
        sha: String,
        #[serde(default)]
        title: Option<String>,
    },
    #[serde(rename = "custom")]
    Custom { instructions: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CodexReviewDeliveryPayload {
    Inline,
    Detached,
}
