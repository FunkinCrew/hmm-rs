use crate::hmm;
use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::path::Path;

/// haxelib 4.2.0 repository format version (`RepoReformatter.CURRENT_REPO_VERSION`).
/// haxelib reads `<repo>/.repo-version` on every command and nags about
/// `haxelib fixrepo` when the file is missing (version 0).
pub const REPO_VERSION: u32 = 1;

const REPO_VERSION_FILE: &str = ".repo-version";

/// Like hmm's `init`: creates whichever of .haxelib/ and hmm.json is missing,
/// so running it in a fresh clone never touches the committed hmm.json.
pub fn init_hmm() -> Result<()> {
    ensure_haxelib_folder()?;
    if Path::new("hmm.json").exists() {
        println!("hmm.json already exists, leaving it as-is");
        return Ok(());
    }
    hmm::json::create_empty_hmm_json()
}

/// Ensures .haxelib/ exists, creating it if missing.
pub fn ensure_haxelib_folder() -> Result<()> {
    ensure_haxelib_folder_at(Path::new("."))
}

pub fn ensure_haxelib_folder_at(base: &Path) -> Result<()> {
    let haxelib_path = base.join(".haxelib");
    if !haxelib_path.exists() {
        println!("Creating .haxelib/ folder");
        std::fs::create_dir(&haxelib_path).context("Failed to create .haxelib folder")?;
    }
    ensure_repo_version_file_at(base)
}

/// Mirrors haxelib's `RepoReformatter.getRepositoryVersion`: trim, then parse.
pub fn parse_repo_version(content: &str) -> Option<u32> {
    content.trim().parse().ok()
}

/// Ensures `<base>/.haxelib/.repo-version` holds `REPO_VERSION`.
///
/// Missing or unparseable content is (re)written. A newer version is left
/// untouched with a warning, mirroring haxelib's own "incompatible" warning.
pub fn ensure_repo_version_file_at(base: &Path) -> Result<()> {
    let path = base.join(".haxelib").join(REPO_VERSION_FILE);
    let existing = std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| parse_repo_version(&c));
    match existing {
        Some(v) if v == REPO_VERSION => Ok(()),
        Some(v) => {
            eprintln!(
                "{}",
                format!(
                    "Warning: .haxelib/{REPO_VERSION_FILE} is {v}, newer than the {REPO_VERSION} this hmm-rs understands; leaving it alone"
                )
                .yellow()
            );
            Ok(())
        }
        None => std::fs::write(&path, format!("{REPO_VERSION}\n"))
            .with_context(|| format!("Failed to write {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_repo_version_trims_like_haxelib() {
        assert_eq!(parse_repo_version("1\n"), Some(1));
        assert_eq!(parse_repo_version(" 1 "), Some(1));
        assert_eq!(parse_repo_version("2"), Some(2));
        assert_eq!(parse_repo_version(""), None);
        assert_eq!(parse_repo_version("abc"), None);
        // haxelib's Std.parseInt would accept this; we rewrite it instead.
        assert_eq!(parse_repo_version("1abc"), None);
    }
}
