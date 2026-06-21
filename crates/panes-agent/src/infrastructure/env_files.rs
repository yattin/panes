use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

pub fn load_dotenv_for_dir(cwd: &Path) {
    for path in dotenv_candidates(cwd) {
        load_dotenv_file(&path);
    }
}

fn dotenv_candidates(cwd: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut current = Some(cwd);
    while let Some(dir) = current {
        let candidate = dir.join(".env");
        if seen.insert(candidate.clone()) {
            out.push(candidate);
        }
        current = dir.parent();
    }
    out.reverse();
    out
}

fn load_dotenv_file(path: &Path) {
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    for line in content.lines() {
        if let Some((key, value)) = parse_dotenv_line(line) {
            if std::env::var_os(&key).is_none() {
                std::env::set_var(key, value);
            }
        }
    }
}

fn parse_dotenv_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let trimmed = trimmed.strip_prefix("export ").unwrap_or(trimmed).trim();
    let (key, raw_value) = trimmed.split_once('=')?;
    let key = key.trim();
    if key.is_empty()
        || !key
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return None;
    }
    let value = strip_inline_comment(raw_value.trim());
    Some((key.to_string(), unquote(value).to_string()))
}

fn strip_inline_comment(value: &str) -> &str {
    if value.starts_with('"') || value.starts_with('\'') {
        return value;
    }
    value
        .split_once(" #")
        .map(|(left, _)| left.trim_end())
        .unwrap_or(value)
}

fn unquote(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn loads_dotenv_without_overwriting_existing_environment() {
        let root = std::env::temp_dir().join(format!(
            "panes-agent-dotenv-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("temp root");
        fs::write(
            root.join(".env"),
            concat!(
                "PANES_TEST_DOTENV_LOADED='from-file'\n",
                "PANES_TEST_DOTENV_EXISTING=from-file\n",
                "export PANES_TEST_DOTENV_BASE_URL=https://third-party.example/v1 # comment\n",
            ),
        )
        .expect("dotenv");
        std::env::set_var("PANES_TEST_DOTENV_EXISTING", "from-env");

        load_dotenv_for_dir(&root);

        assert_eq!(
            std::env::var("PANES_TEST_DOTENV_LOADED").as_deref(),
            Ok("from-file")
        );
        assert_eq!(
            std::env::var("PANES_TEST_DOTENV_EXISTING").as_deref(),
            Ok("from-env")
        );
        assert_eq!(
            std::env::var("PANES_TEST_DOTENV_BASE_URL").as_deref(),
            Ok("https://third-party.example/v1")
        );

        std::env::remove_var("PANES_TEST_DOTENV_LOADED");
        std::env::remove_var("PANES_TEST_DOTENV_EXISTING");
        std::env::remove_var("PANES_TEST_DOTENV_BASE_URL");
        let _ = fs::remove_dir_all(root);
    }
}
