use std::path::PathBuf;

use anyhow::{Ok, Result};

use crate::{
    commands,
    hmm::{
        self,
        dependencies::Dependancies,
        haxelib::{Haxelib, HaxelibType},
    },
};

/// Install a git-based library and add it to hmm.json
///
/// # Arguments
/// * `name` - The name of the library (e.g., "flixel")
/// * `url` - The git repository URL (e.g., "https://github.com/HaxeFlixel/flixel")
/// * `git_ref` - Optional git ref (branch, tag, or commit SHA). If None, uses repository's default branch
/// * `deps` - Current dependencies from hmm.json
/// * `json_path` - Path to hmm.json file
///
/// # Example
/// ```bash
/// hmm-rs git flixel https://github.com/HaxeFlixel/flixel dev
/// hmm-rs git lime https://github.com/openfl/lime
/// ```
pub fn install_git(
    name: &str,
    url: &str,
    git_ref: &Option<String>,
    mut deps: Dependancies,
    json_path: PathBuf,
) -> Result<()> {
    // Check if library already exists in dependencies
    if let Some(existing) = deps.dependencies.iter().find(|lib| lib.name == name) {
        println!(
            "Warning: {} already exists in hmm.json as {:?}",
            name, existing.haxelib_type
        );
        println!("This will update the dependency to use the git repository");
    }

    let mut haxelib_install = Haxelib {
        name: name.to_string(),
        haxelib_type: HaxelibType::Git,
        vcs_ref: git_ref.clone(),
        dir: None,
        path: None,
        url: Some(url.to_string()),
        version: None,
    };

    // If no ref specified, detect the default branch
    if haxelib_install.vcs_ref.is_none() {
        println!("No ref specified, will use repository's default branch");
        // We could query the remote here to get the default branch, but it's easier
        // to let git clone handle it and then query the checked out branch
    }

    // Install the git repository
    commands::install_command::install_or_update_git_cli(&haxelib_install)?;

    // If we didn't have a ref, get the current HEAD after clone
    if haxelib_install.vcs_ref.is_none() {
        let detected_ref = detect_current_git_ref(name)?;
        println!("Detected ref: {}", detected_ref);
        haxelib_install.vcs_ref = Some(detected_ref);
    }

    // Remove existing entry if present, then add new one
    deps.dependencies.retain(|lib| lib.name != name);
    deps.dependencies.push(haxelib_install);

    // Save to hmm.json
    hmm::json::save_json(deps, json_path)?;

    Ok(())
}

/// Detect the current git ref (branch/tag/commit) after cloning
fn detect_current_git_ref(name: &str) -> Result<String> {
    let repo_path = format!(".haxelib/{}/git", name.replace(".", ","));

    // Try to get the current branch name
    let branch_output = std::process::Command::new("git")
        .args(["-C", &repo_path, "rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;

    if branch_output.status.success() {
        let branch = String::from_utf8_lossy(&branch_output.stdout)
            .trim()
            .to_string();

        // If we're in detached HEAD state, get the commit SHA
        if branch == "HEAD" {
            let commit_output = std::process::Command::new("git")
                .args(["-C", &repo_path, "rev-parse", "HEAD"])
                .output()?;

            if commit_output.status.success() {
                let commit = String::from_utf8_lossy(&commit_output.stdout)
                    .trim()
                    .to_string();
                return Ok(commit);
            }
        }

        return Ok(branch);
    }

    // Fallback: just get the commit SHA
    let commit_output = std::process::Command::new("git")
        .args(["-C", &repo_path, "rev-parse", "HEAD"])
        .output()?;

    if commit_output.status.success() {
        let commit = String::from_utf8_lossy(&commit_output.stdout)
            .trim()
            .to_string();
        return Ok(commit);
    }

    // Last resort: return "main"
    Ok("main".to_string())
}
