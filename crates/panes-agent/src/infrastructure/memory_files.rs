use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use crate::domain::memory::MemoryFragment;

const MAX_MEMORY_FILE_BYTES: u64 = 40 * 1024;
const AGENTS_DIR: &str = ".agents";
const AGENTS_FILE: &str = "AGENTS.md";

pub fn load_memory_fragments(cwd: &Path) -> Vec<MemoryFragment> {
    load_memory_fragments_with_home(cwd, home_dir().as_deref())
}

fn load_memory_fragments_with_home(cwd: &Path, home: Option<&Path>) -> Vec<MemoryFragment> {
    let mut visited = HashSet::new();
    candidate_memory_files(cwd, home)
        .into_iter()
        .filter_map(|path| load_memory_file(&path, &mut visited))
        .collect()
}

fn candidate_memory_files(cwd: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = home {
        let agents_home = home.join(AGENTS_DIR);
        extend_rule_files(&mut paths, &agents_home.join("rules"));
        paths.push(agents_home.join(AGENTS_FILE));
    }

    paths.push(cwd.join(AGENTS_FILE));
    let workspace_agents = cwd.join(AGENTS_DIR);
    extend_rule_files(&mut paths, &workspace_agents.join("rules"));
    paths.push(workspace_agents.join(AGENTS_FILE));
    paths
}

fn extend_rule_files(paths: &mut Vec<PathBuf>, rules_dir: &Path) {
    let Ok(entries) = fs::read_dir(rules_dir) else {
        return;
    };
    let mut rule_files = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("md"))
        .collect::<Vec<_>>();
    rule_files.sort();
    paths.extend(rule_files);
}

fn load_memory_file(path: &Path, visited: &mut HashSet<PathBuf>) -> Option<MemoryFragment> {
    if !path.is_file() {
        return None;
    }
    let canonical = path.canonicalize().ok()?;
    if !visited.insert(canonical.clone()) {
        return None;
    }
    let metadata = fs::metadata(&canonical).ok()?;
    if metadata.len() > MAX_MEMORY_FILE_BYTES {
        return Some(MemoryFragment {
            source: display_path(&canonical),
            content: format!(
                "<skipped memory file larger than {} bytes>",
                MAX_MEMORY_FILE_BYTES
            ),
        });
    }
    let content = fs::read_to_string(&canonical).ok()?;
    let expanded = expand_includes(&canonical, &content, visited);
    Some(MemoryFragment {
        source: display_path(&canonical),
        content: expanded,
    })
}

fn expand_includes(path: &Path, content: &str, visited: &mut HashSet<PathBuf>) -> String {
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("@include ") {
            let include_path = resolve_include_path(base_dir, rest.trim());
            if let Some(fragment) = load_memory_file(&include_path, visited) {
                out.push(fragment.content);
            } else {
                out.push(format!("<!-- skipped include: {} -->", rest.trim()));
            }
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

fn resolve_include_path(base_dir: &Path, raw: &str) -> PathBuf {
    if let Some(stripped) = raw.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(stripped);
        }
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn expands_includes_and_skips_cycles() {
        let root = temp_dir("panes-agent-memory");
        fs::create_dir_all(&root).expect("temp root");
        fs::write(root.join("AGENTS.md"), "Root\n@include ./extra.md").expect("agents");
        fs::write(root.join("extra.md"), "Extra\n@include ./AGENTS.md").expect("extra");

        let fragments = load_memory_fragments_with_home(&root, None);
        let combined = fragments
            .iter()
            .map(|fragment| fragment.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(combined.contains("Root"));
        assert!(combined.contains("Extra"));
        assert!(combined.contains("skipped include"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn loads_agents_rules_and_ignores_claude_and_claurst() {
        let root = temp_dir("panes-agent-memory-agents");
        let home = root.join("home");
        let workspace = root.join("workspace");
        fs::create_dir_all(home.join(".agents/rules")).expect("home agents rules");
        fs::create_dir_all(workspace.join(".agents/rules")).expect("workspace agents rules");
        fs::create_dir_all(home.join(".claurst/rules")).expect("legacy home rules");
        fs::create_dir_all(workspace.join(".claurst")).expect("legacy workspace");

        fs::write(home.join(".agents/rules/01-home.md"), "home rule").expect("home rule");
        fs::write(home.join(".agents/AGENTS.md"), "home agents").expect("home agents");
        fs::write(home.join(".agents/CLAUDE.md"), "home claude").expect("home claude");
        fs::write(home.join(".claurst/rules/legacy.md"), "legacy home rule").expect("legacy rule");
        fs::write(home.join(".claurst/AGENTS.md"), "legacy home agents").expect("legacy agents");
        fs::write(workspace.join("AGENTS.md"), "workspace agents").expect("workspace agents");
        fs::write(workspace.join("CLAUDE.md"), "workspace claude").expect("workspace claude");
        fs::write(workspace.join(".agents/rules/01-local.md"), "local rule").expect("local rule");
        fs::write(workspace.join(".agents/AGENTS.md"), "local agents").expect("local agents");
        fs::write(workspace.join(".agents/CLAUDE.md"), "local claude").expect("local claude");
        fs::write(workspace.join(".claurst/AGENTS.md"), "legacy local agents")
            .expect("legacy local");

        let fragments = load_memory_fragments_with_home(&workspace, Some(&home));
        let combined = fragments
            .iter()
            .map(|fragment| fragment.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(combined.contains("home rule"));
        assert!(combined.contains("home agents"));
        assert!(combined.contains("workspace agents"));
        assert!(combined.contains("local rule"));
        assert!(combined.contains("local agents"));
        assert!(!combined.contains("claude"));
        assert!(!combined.contains("legacy"));

        let _ = fs::remove_dir_all(root);
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{prefix}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }
}
