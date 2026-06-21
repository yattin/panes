mod apply_patch;
mod batch_edit;
mod command;
mod file_edit;
mod file_read;
mod file_write;
mod glob;
mod grep;
mod list_files;
mod monitor;
mod paths;
mod search;
mod skill;
mod specs;
mod tasks;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::{
    application::ports::{PermissionGateway, ToolExecutor},
    domain::{
        permission::{PermissionDecision, PermissionRequest},
        skills::SkillDefinition,
        tools::{ToolCall, ToolResult, ToolSpec},
    },
};

use command::BackgroundCommandStore;
pub(crate) use paths::{input_path, input_string, tool_error, WorkspacePath};
use tasks::TaskStore;

const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct NativeToolExecutor {
    workspace_root: PathBuf,
    tasks: Arc<Mutex<TaskStore>>,
    background_commands: Arc<Mutex<BackgroundCommandStore>>,
    skills: Arc<Vec<SkillDefinition>>,
    permissions: Arc<dyn PermissionGateway>,
    command_timeout: Duration,
}

impl NativeToolExecutor {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self::with_permissions(workspace_root, Arc::new(AllowAllPermissionGateway))
    }

    pub fn with_permissions(
        workspace_root: impl Into<PathBuf>,
        permissions: Arc<dyn PermissionGateway>,
    ) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            tasks: Arc::new(Mutex::new(TaskStore::default())),
            background_commands: Arc::new(Mutex::new(BackgroundCommandStore::default())),
            skills: Arc::new(Vec::new()),
            permissions,
            command_timeout: DEFAULT_COMMAND_TIMEOUT,
        }
    }

    pub fn with_skills(mut self, skills: Vec<SkillDefinition>) -> Self {
        self.skills = Arc::new(skills);
        self
    }

    pub fn with_command_timeout(mut self, timeout: Duration) -> Self {
        self.command_timeout = timeout;
        self
    }

    pub(crate) fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub(crate) fn task_store(&self) -> &Arc<Mutex<TaskStore>> {
        &self.tasks
    }

    pub(crate) fn background_commands(&self) -> &Arc<Mutex<BackgroundCommandStore>> {
        &self.background_commands
    }

    pub(crate) fn permissions(&self) -> &dyn PermissionGateway {
        self.permissions.as_ref()
    }

    pub(crate) fn skills(&self) -> &[SkillDefinition] {
        self.skills.as_ref()
    }

    pub(crate) fn command_timeout(&self) -> Duration {
        self.command_timeout
    }
}

pub fn tool_specs() -> Vec<ToolSpec> {
    specs::native_tool_specs()
}

#[derive(Debug, Clone)]
pub struct AllowAllPermissionGateway;

#[async_trait]
impl PermissionGateway for AllowAllPermissionGateway {
    async fn request(&self, _request: PermissionRequest) -> anyhow::Result<PermissionDecision> {
        Ok(PermissionDecision::Allow)
    }
}

#[async_trait]
impl ToolExecutor for NativeToolExecutor {
    async fn execute(
        &self,
        call: ToolCall,
        cancellation: &CancellationToken,
    ) -> anyhow::Result<ToolResult> {
        match call.name.as_str() {
            "file_read" | "read_file" => file_read::execute(self, call).await,
            "list_files" => list_files::execute(self, call).await,
            "glob" => glob::execute(self, call).await,
            "grep" => grep::execute(self, call).await,
            "search" => search::execute(self, call).await,
            "file_write" | "write_file" => file_write::execute(self, call).await,
            "file_edit" => file_edit::execute(self, call).await,
            "batch_edit" => batch_edit::execute(self, call).await,
            "apply_patch" => apply_patch::execute(self, call).await,
            "execute_command" => command::execute(self, call, cancellation).await,
            "monitor" => monitor::execute(self, call).await,
            "skill" => skill::execute(self, call).await,
            "task_management" => tasks::execute(self, call).await,
            _ => Ok(tool_error(
                call.id,
                format!("unsupported native tool: {}", call.name),
            )),
        }
    }
}
