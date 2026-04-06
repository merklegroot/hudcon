//! `PATH` environment (aligned with [hudsse](https://github.com/merklegroot/hudsse) `parsePath`).

use serde::Serialize;
use std::collections::HashSet;
use std::env;

/// Parsed `PATH`: raw string plus folder list (deduped, sorted case-insensitively).
#[derive(Debug, Clone, Serialize)]
pub struct PathInfo {
    pub path: String,
    pub folders: Vec<String>,
}

fn path_separator() -> char {
    if cfg!(windows) {
        ';'
    } else {
        ':'
    }
}

/// Reads `PATH` / `Path`, splits like hudsse (`:` vs `;`), dedupes (first wins), sorts case-insensitively.
pub fn gather_path_info() -> PathInfo {
    let raw = env::var("PATH")
        .or_else(|_| env::var("Path"))
        .unwrap_or_default();
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        return PathInfo {
            path: String::new(),
            folders: Vec::new(),
        };
    }

    let sep = path_separator();
    let parts: Vec<String> = trimmed
        .split(sep)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for f in parts {
        if seen.insert(f.clone()) {
            unique.push(f);
        }
    }

    unique.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

    PathInfo {
        path: trimmed,
        folders: unique,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_splits_dedupes_sorts() {
        let info = parse_path_for_test("/bin:/usr/bin:/bin", ':');
        assert_eq!(info.path, "/bin:/usr/bin:/bin");
        assert_eq!(info.folders, vec!["/bin", "/usr/bin"]);
    }

    #[test]
    fn windows_separator() {
        let info = parse_path_for_test(r"C:\a;D:\b;C:\a", ';');
        assert_eq!(info.folders, vec![r"C:\a", r"D:\b"]);
    }

    fn parse_path_for_test(raw: &str, sep: char) -> PathInfo {
        let trimmed = raw.trim().to_string();
        let parts: Vec<String> = trimmed
            .split(sep)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let mut seen = HashSet::new();
        let mut unique = Vec::new();
        for f in parts {
            if seen.insert(f.clone()) {
                unique.push(f);
            }
        }
        unique.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        PathInfo {
            path: trimmed,
            folders: unique,
        }
    }
}
