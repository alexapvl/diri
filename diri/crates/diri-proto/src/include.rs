//! The user-owned `.diri-include` file that Quick Open and MCP share.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Active gitignore-style lines: comments and blanks are dropped.
pub fn pattern_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

pub fn load(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

pub fn store(path: &Path, text: &str) -> io::Result<()> {
    if text.trim().is_empty() {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        };
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncludeMutation {
    pub path: PathBuf,
    pub text: String,
    pub patterns: Vec<String>,
    pub added: Vec<String>,
}

/// Append unique patterns, preserving comments and existing order.
pub fn add_patterns(path: &Path, patterns: &[String]) -> io::Result<IncludeMutation> {
    let mut text = load(path);
    let mut existing: HashSet<String> = pattern_lines(&text)
        .iter()
        .map(|pattern| pattern_key(pattern))
        .collect();
    let mut added = Vec::new();
    for pattern in patterns {
        let trimmed = pattern.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let key = pattern_key(trimmed);
        if !existing.insert(key) {
            continue;
        }
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(trimmed);
        text.push('\n');
        added.push(trimmed.to_owned());
    }
    if !added.is_empty() {
        store(path, &text)?;
    }
    let patterns = pattern_lines(&text);
    Ok(IncludeMutation {
        path: path.to_path_buf(),
        text,
        patterns,
        added,
    })
}

fn pattern_key(pattern: &str) -> String {
    pattern.trim_end_matches('/').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn add_patterns_appends_unique_gitignore_lines() {
        let temp = tempdir().unwrap();
        let path = temp.path().join(".diri-include");
        fs::write(&path, "# keep\n*/.worktrees/\n").unwrap();

        let first = add_patterns(
            &path,
            &[
                "  **/.worktrees/  ".into(),
                "*/.worktrees".into(),
                "# ignored".into(),
                String::new(),
            ],
        )
        .unwrap();
        assert_eq!(first.added, ["**/.worktrees/"]);
        assert_eq!(first.patterns, ["*/.worktrees/", "**/.worktrees/"]);
        assert_eq!(load(&path), "# keep\n*/.worktrees/\n**/.worktrees/\n");

        let again = add_patterns(&path, &["**/.worktrees".into()]).unwrap();
        assert!(again.added.is_empty());
        assert_eq!(load(&path), "# keep\n*/.worktrees/\n**/.worktrees/\n");
    }

    #[test]
    fn empty_store_deletes_the_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join(".diri-include");
        store(&path, "**/.hidden/\n").unwrap();
        assert!(path.exists());
        store(&path, " \n").unwrap();
        assert!(!path.exists());
        assert_eq!(load(&path), "");
    }
}
