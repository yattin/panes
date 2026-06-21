use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    pub action_id: String,
    pub action_type: String,
    pub summary: String,
    pub details: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow,
    AllowForSession,
    Deny,
}

impl PermissionDecision {
    pub fn allows(&self) -> bool {
        matches!(
            self,
            PermissionDecision::Allow | PermissionDecision::AllowForSession
        )
    }
}
