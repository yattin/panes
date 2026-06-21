use std::path::{Component, Path, PathBuf};

use anyhow::Context;

use crate::domain::tools::{ToolCall, ToolResult};

use super::NativeToolExecutor;

pub(crate) enum WorkspacePath {
    Inside(PathBuf),
    Rejected(String),
}

impl NativeToolExecutor {
    pub(crate) fn resolve_existing_workspace_path(
        &self,
        path: &str,
        tool_name: &str,
    ) -> anyhow::Result<WorkspacePath> {
        let requested_path = Path::new(path);
        if requested_path.is_absolute() {
            return Ok(WorkspacePath::Rejected(format!(
                "{tool_name} path must be relative"
            )));
        }

        let root = self.workspace_root().canonicalize().with_context(|| {
            format!(
                "failed to resolve workspace root {:?}",
                self.workspace_root()
            )
        })?;
        let target = root.join(requested_path);
        let resolved_target = target
            .canonicalize()
            .with_context(|| format!("failed to resolve path {:?}", target))?;

        if !resolved_target.starts_with(&root) {
            return Ok(WorkspacePath::Rejected(format!(
                "{tool_name} path escapes workspace root"
            )));
        }

        Ok(WorkspacePath::Inside(resolved_target))
    }

    pub(crate) fn resolve_new_workspace_file_path(
        &self,
        path: &str,
        tool_name: &str,
    ) -> anyhow::Result<WorkspacePath> {
        let requested_path = Path::new(path);
        if requested_path.is_absolute() {
            return Ok(WorkspacePath::Rejected(format!(
                "{tool_name} path must be relative"
            )));
        }
        if requested_path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Ok(WorkspacePath::Rejected(format!(
                "{tool_name} path escapes workspace root"
            )));
        }

        let root = self.workspace_root().canonicalize().with_context(|| {
            format!(
                "failed to resolve workspace root {:?}",
                self.workspace_root()
            )
        })?;
        let target = root.join(requested_path);
        let parent = target.parent().unwrap_or(root.as_path());
        let existing_parent = nearest_existing_parent(parent);
        let resolved_parent = existing_parent
            .canonicalize()
            .with_context(|| format!("failed to resolve parent path {:?}", existing_parent))?;

        if !resolved_parent.starts_with(&root) {
            return Ok(WorkspacePath::Rejected(format!(
                "{tool_name} path escapes workspace root"
            )));
        }

        Ok(WorkspacePath::Inside(target))
    }
}

fn nearest_existing_parent(path: &Path) -> &Path {
    let mut candidate = path;
    while !candidate.exists() {
        let Some(parent) = candidate.parent() else {
            break;
        };
        candidate = parent;
    }
    candidate
}

pub(crate) fn input_path(call: &ToolCall) -> Option<&str> {
    input_string(call, &["path", "file_path", "filePath"])
}

pub(crate) fn input_string<'a>(call: &'a ToolCall, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| call.input.get(*key).and_then(|value| value.as_str()))
}

pub(crate) fn tool_error(tool_use_id: impl Into<String>, content: impl Into<String>) -> ToolResult {
    ToolResult {
        tool_use_id: tool_use_id.into(),
        content: content.into(),
        is_error: true,
    }
}
