use crate::commands::check_command::InstallType;
use crate::hmm::dependencies::Dependancies;
use crate::hmm::haxelib::Haxelib;
use crate::hmm::haxelib::HaxelibType;
use anyhow::Ok;
use anyhow::{anyhow, Context, Result};
use console::Emoji;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client as ReqwestClient;
use std::env;
use std::fs::File;
use std::io::{stdin, stdout, Write};
use std::path::Path;
use std::path::PathBuf;
use yansi::Paint;
use zip::ZipArchive;

use super::check_command::compare_haxelib_to_hmm;
use super::check_command::HaxelibStatus;

/// User's choice for resolving git conflicts
enum ConflictResolution {
    Stash,   // Stash changes, update, restore
    Discard, // Discard all changes and update
    Commit,  // Commit changes first, then update
    Skip,    // Skip this library
}

pub fn install_from_hmm(deps: &Dependancies) -> Result<()> {
    let installs_needed = compare_haxelib_to_hmm(deps)?;
    println!(
        "{} dependencies need to be installed",
        installs_needed.len().to_string().bold()
    );

    for install_status in installs_needed.iter() {
        match &install_status.install_type {
            InstallType::Missing => handle_install(install_status)?,
            InstallType::MissingGit => handle_install(install_status)?,
            InstallType::Outdated => match &install_status.lib.haxelib_type {
                HaxelibType::Haxelib => install_from_haxelib(install_status.lib)?,
                HaxelibType::Git => install_or_update_git_cli(install_status.lib)?,
                lib_type => println!(
                    "{}: Installing from {:?} not yet implemented",
                    install_status.lib.name.red(),
                    lib_type
                ),
            },
            InstallType::Conflict => {
                // Handle git conflicts interactively
                handle_git_conflict(install_status)?;
            }
            InstallType::AlreadyInstalled => (), // do nothing on things already installed at the right version
            _ => println!(
                "{} {:?}: Not implemented",
                install_status.lib.name, install_status.install_type
            ),
        }
    }

    Ok(())
}

pub fn handle_install(haxelib_status: &HaxelibStatus) -> Result<()> {
    match &haxelib_status.lib.haxelib_type {
        HaxelibType::Haxelib => install_from_haxelib(haxelib_status.lib)?,
        HaxelibType::Git => install_or_update_git_cli(haxelib_status.lib)?,
        lib_type => println!(
            "{}: Installing from {:?} not yet implemented",
            haxelib_status.lib.name.red(),
            lib_type
        ),
    }

    Ok(())
}

// Preserved for reference - replaced with CLI implementation below
// pub fn install_from_git_using_gix_clone(haxelib: &Haxelib) -> Result<()> {
//     println!("Installing {} from git using clone", haxelib.name);
//
//     let path_with_no_https = haxelib.url().replace("https://", "");
//
//     let clone_url = GixUrl::from_parts(
//         gix::url::Scheme::Https,
//         None,
//         None,
//         None,
//         None,
//         BString::from(path_with_no_https),
//         false,
//     )
//     .context(format!("error creating gix url for {}", haxelib.url()))?;
//
//     let mut clone_path = PathBuf::from(".haxelib").join(&haxelib.name);
//
//     create_current_file(&clone_path, &String::from("git"))?;
//
//     clone_path = clone_path.join("git");
//
//     if let Err(e) = std::fs::create_dir_all(&clone_path) {
//         if e.kind() == std::io::ErrorKind::AlreadyExists {
//             println!("Directory already exists: {:?}", clone_path.as_path());
//         } else {
//             return Err(anyhow!(
//                 "Error creating directory: {:?}",
//                 clone_path.as_path()
//             ));
//         }
//     };
//
//     let mut da_fetch = clone::PrepareFetch::new(
//         clone_url,
//         clone_path,
//         create::Kind::WithWorktree,
//         create::Options::default(),
//         gix::open::Options::default(),
//     )
//     .context("error preparing clone")?;
//
//     let repo = da_fetch
//         .fetch_then_checkout(Discard, &AtomicBool::new(false))?
//         .0
//         .main_worktree(Discard, &AtomicBool::new(false))
//         .expect("Error checking out worktree")
//         .0;
//
//     let submodule_result = repo.submodules()?;
//
//     if let Some(submodule_list) = submodule_result {
//         for submodule in submodule_list {
//             let submodule_path = submodule.path()?;
//             let submodule_url = submodule.url()?;
//             println!("Submodule: {} - {}", submodule_path, submodule_url);
//         }
//     }
//
//     do_commit_checkout(&repo, haxelib)?;
//
//     Ok(())
// }

#[tokio::main]
pub async fn install_from_haxelib(haxelib: &Haxelib) -> Result<()> {
    println!(
        "Downloading: {} - {} - {}",
        haxelib.name.bold(),
        "lib.haxe.org".yellow().bold(),
        haxelib.download_url()?.bold()
    );

    let response = ReqwestClient::new()
        .get(haxelib.download_url()?)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!("Failed to download: HTTP {}", response.status()));
    }

    let expected_total_size = response
        .content_length()
        .ok_or_else(|| anyhow!("Server didn't provide content length"))?;

    let pb: ProgressBar = ProgressBar::new(expected_total_size);
    pb.set_style(ProgressStyle::with_template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.yellow/red}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
             .unwrap());

    let tmp_dir = env::temp_dir().join(format!("{}.zip", haxelib.name));

    let _ = {
        let mut file = File::create(&tmp_dir)?;
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            let chunk = item?;
            file.write_all(&chunk)?;
            let new = std::cmp::min(downloaded + (chunk.len() as u64), expected_total_size);
            downloaded = new;
            pb.set_position(new);
        }

        file.flush()?;
        downloaded
    };

    let finish_message = format!(
        "{}: {} done downloading from {}",
        haxelib.name.green().bold(),
        haxelib.version().bright_green(),
        "Haxelib".yellow().bold()
    );
    pb.finish_with_message(finish_message);

    let metadata = std::fs::metadata(&tmp_dir)?;
    if metadata.len() != expected_total_size {
        return Err(anyhow!(
            "Download incomplete: expected {} bytes, got {} bytes",
            expected_total_size,
            metadata.len()
        ));
    }

    let output_dir: PathBuf = [".haxelib", haxelib.name_as_commas().as_str()]
        .iter()
        .collect();

    if let Err(e) = std::fs::create_dir(&output_dir) {
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(anyhow!(
                "Error creating directory: {:?}",
                output_dir.as_path()
            ));
        }
    }

    create_current_file(&output_dir, &haxelib.version().to_string())?;

    // unzipping
    let archive =
        File::open(&tmp_dir).context(format!("Failed to open downloaded zip: {:?}", tmp_dir))?;

    let mut zip_file =
        ZipArchive::new(archive).context("Error opening zip file - file may be corrupted")?;

    let unzipped_output_dir = output_dir.join(haxelib.version_as_commas());
    zip_file
        .extract(&unzipped_output_dir)
        .context("Error extracting zip file")?;

    std::fs::remove_file(&tmp_dir)?;

    print_success(haxelib)?;
    Ok(())
}

// Preserved for reference - replaced with CLI implementation below
// pub fn install_from_git_using_gix_checkout(haxelib: &Haxelib) -> Result<()> {
//     println!("Updating {} from git using checkout", haxelib.name);
//
//     let discover_result = gix::discover(
//         Path::new(".haxelib")
//             .join(haxelib.name.as_str())
//             .join("git"),
//     );
//
//     let repo = match discover_result {
//         core::result::Result::Ok(r) => r,
//         Err(e) => {
//             if e.to_string().contains("not a git repository") {
//                 return install_from_git_using_gix_clone(haxelib);
//             } else {
//                 return Err(anyhow!("Error discovering git repo: {:?}", e));
//             }
//         }
//     };
//
//     // let fetch_url = repo
//     //     .find_fetch_remote(None)?
//     //     .url(gix::remote::Direction::Fetch)
//     //     .unwrap()
//     //     .clone();
//
//     do_commit_checkout(&repo, haxelib)?;
//
//     print_success(haxelib)?;
//     Ok(())
// }

// Preserved for reference - replaced with CLI implementation below
// fn do_commit_checkout(repo: &gix::Repository, haxelib: &Haxelib) -> Result<()> {
//     print!("Checking out {}", haxelib.name);
//     if let Some(target_ref) = haxelib.vcs_ref.as_ref() {
//         println!(" at {}", target_ref);
//         let reflog_msg = BString::from("derp?");
//
//         let target_gix_ref = repo.find_reference(target_ref)?.id();
//
//         repo.head_ref()
//             .unwrap()
//             .unwrap()
//             .set_target_id(target_gix_ref, reflog_msg)?;
//     }
//
//     Ok(())
// }

/// Unified git installer using git CLI for optimal performance and reliability
/// - Uses blobless clone (--filter=blob:none) for fast initial download with full history
/// - Smart checkout: tries local first, fetches only if commit not found
/// - Properly handles submodules with --init --recursive
pub fn install_or_update_git_cli(haxelib: &Haxelib) -> Result<()> {
    let git_dir_path = PathBuf::from(".haxelib")
        .join(haxelib.name_as_commas())
        .join("git");

    let parent_dir = git_dir_path.parent().unwrap();

    // Ensure repository exists (clone if needed)
    if !git_dir_path.exists() {
        println!(
            "Cloning {} (blobless for speed + full history)...",
            haxelib.name
        );
        clone_blobless_git_repo(haxelib, &git_dir_path)?;

        // Create .current file indicating this is a git install
        create_current_file(parent_dir, &String::from("git"))?;
    } else {
        println!("Repository exists, checking out {}...", haxelib.name);
    }

    // Checkout the specified commit/ref (if provided)
    if haxelib.vcs_ref.is_some() {
        smart_checkout_git_ref(haxelib, &git_dir_path)?;
    } else {
        println!("No ref specified, using repository's default branch");
    }

    // Update submodules to match the checked out commit
    update_git_submodules(&git_dir_path)?;

    print_success(haxelib)?;
    Ok(())
}

/// Clone with --filter=blob:none for fast download with full commit history
/// Falls back to regular clone if blobless is not supported
fn clone_blobless_git_repo(haxelib: &Haxelib, target_path: &Path) -> Result<()> {
    let url = haxelib.url();

    // Try blobless clone first (fast, full history)
    let blobless_result = std::process::Command::new("git")
        .args([
            "clone",
            "--filter=blob:none",
            url,
            target_path.to_str().unwrap(),
        ])
        .status()
        .context("Failed to execute git clone")?;

    if blobless_result.success() {
        println!("✓ Blobless clone completed");
    } else {
        // Fallback to regular clone if blobless not supported
        println!("Blobless clone failed, falling back to regular clone...");
        let regular_result = std::process::Command::new("git")
            .args(["clone", url, target_path.to_str().unwrap()])
            .status()
            .context("Failed to execute git clone")?;

        if !regular_result.success() {
            return Err(anyhow!("Git clone failed for {}", haxelib.name));
        }

        println!("✓ Clone completed");
    }

    // Parse remote name from URL and rename origin
    let remote_name = parse_remote_name_from_url(url)?;
    rename_origin_remote(target_path, &remote_name)?;

    Ok(())
}

/// Smart checkout: try local first, fetch if commit not found
fn smart_checkout_git_ref(haxelib: &Haxelib, repo_path: &Path) -> Result<()> {
    let target_ref = haxelib.vcs_ref();
    let url = haxelib.url();

    println!("Checking out {} at {}...", haxelib.name, target_ref);

    // Ensure remote exists with correct name and URL
    let remote_name = parse_remote_name_from_url(url)?;
    ensure_git_remote(repo_path, &remote_name, url)?;

    // Try to checkout locally first
    let checkout_result = std::process::Command::new("git")
        .args(["-C", repo_path.to_str().unwrap(), "checkout", target_ref])
        .output()
        .context("Failed to execute git checkout")?;

    if checkout_result.status.success() {
        println!("✓ Checked out {} (local)", target_ref);
        return Ok(());
    }

    // Commit not found locally - fetch from managed remote and retry
    println!(
        "Commit {} not found locally, fetching from {}...",
        target_ref, remote_name
    );

    let fetch_result = std::process::Command::new("git")
        .args(["-C", repo_path.to_str().unwrap(), "fetch", &remote_name])
        .status()
        .context("Failed to execute git fetch")?;

    if !fetch_result.success() {
        return Err(anyhow!(
            "Git fetch failed for {} from {}",
            haxelib.name,
            remote_name
        ));
    }

    // Try checkout again after fetch
    let checkout_retry = std::process::Command::new("git")
        .args(["-C", repo_path.to_str().unwrap(), "checkout", target_ref])
        .status()
        .context("Failed to execute git checkout after fetch")?;

    if !checkout_retry.success() {
        return Err(anyhow!(
            "Commit {} not found even after fetch for {}",
            target_ref,
            haxelib.name
        ));
    }

    println!("✓ Checked out {} (after fetch)", target_ref);
    Ok(())
}

/// Initialize and update submodules recursively
fn update_git_submodules(repo_path: &Path) -> Result<()> {
    let result = std::process::Command::new("git")
        .args([
            "-C",
            repo_path.to_str().unwrap(),
            "submodule",
            "update",
            "--init",
            "--recursive",
        ])
        .status()
        .context("Failed to execute git submodule update")?;

    if !result.success() {
        return Err(anyhow!("Git submodule update failed"));
    }

    Ok(())
}

fn print_success(haxelib: &Haxelib) -> Result<()> {
    // print empty line for readability
    println!();

    let version_str = haxelib.version_or_ref().unwrap_or("(default)"); // For git repos without explicit ref

    println!(
        "{}: {} installed {}",
        haxelib.name.green().bold(),
        version_str.bright_green(),
        Emoji("✅", "[✔️]")
    );
    // print an empty line, for readability between downloads
    println!();
    Ok(())
}

/// Parse a remote name from a git URL (format: username/repo)
fn parse_remote_name_from_url(url: &str) -> Result<String> {
    // Handle various URL formats:
    // https://github.com/user/repo.git
    // https://github.com/user/repo
    // git@github.com:user/repo.git
    // ssh://git@github.com/user/repo.git

    let url = url.trim();

    // Remove common prefixes
    let path = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("ssh://")
        .trim_start_matches("git@");

    // Split by colon (for ssh format) or slash
    let parts: Vec<&str> = if path.contains(':') {
        path.split(':').collect()
    } else {
        vec![path]
    };

    // Get the path part (after domain)
    let repo_path = parts
        .last()
        .ok_or_else(|| anyhow!("Invalid git URL: {}", url))?;

    // Split by slashes and get last two parts (user/repo)
    let path_parts: Vec<&str> = repo_path.split('/').filter(|s| !s.is_empty()).collect();

    if path_parts.len() < 2 {
        return Err(anyhow!("Could not parse username/repo from URL: {}", url));
    }

    let username = path_parts[path_parts.len() - 2];
    let mut repo = path_parts[path_parts.len() - 1];

    // Remove .git suffix if present
    repo = repo.trim_end_matches(".git");

    Ok(format!("{}/{}", username, repo))
}

/// Ensure a git remote exists with the proper name
fn ensure_git_remote(repo_path: &Path, remote_name: &str, url: &str) -> Result<()> {
    // Check if remote exists
    let check_remote = std::process::Command::new("git")
        .args([
            "-C",
            repo_path.to_str().unwrap(),
            "remote",
            "get-url",
            remote_name,
        ])
        .output()
        .context("Failed to check git remote")?;

    if check_remote.status.success() {
        // Remote exists - verify URL matches
        let existing_url = String::from_utf8_lossy(&check_remote.stdout)
            .trim()
            .to_string();

        if existing_url != url {
            println!("Updating remote {} URL...", remote_name.cyan());

            let update_result = std::process::Command::new("git")
                .args([
                    "-C",
                    repo_path.to_str().unwrap(),
                    "remote",
                    "set-url",
                    remote_name,
                    url,
                ])
                .status()
                .context("Failed to update remote URL")?;

            if !update_result.success() {
                return Err(anyhow!("Failed to update remote {} URL", remote_name));
            }
        }
    } else {
        // Remote doesn't exist - create it
        println!("Adding remote {}...", remote_name.cyan());

        let add_result = std::process::Command::new("git")
            .args([
                "-C",
                repo_path.to_str().unwrap(),
                "remote",
                "add",
                remote_name,
                url,
            ])
            .status()
            .context("Failed to add git remote")?;

        if !add_result.success() {
            return Err(anyhow!("Failed to add remote {}", remote_name));
        }
    }

    Ok(())
}

/// Rename 'origin' remote to a better name after cloning
fn rename_origin_remote(repo_path: &Path, new_name: &str) -> Result<()> {
    // Check if origin exists
    let check_origin = std::process::Command::new("git")
        .args([
            "-C",
            repo_path.to_str().unwrap(),
            "remote",
            "get-url",
            "origin",
        ])
        .output()
        .context("Failed to check origin remote")?;

    if check_origin.status.success() {
        println!("Renaming remote origin → {}...", new_name.cyan());

        let rename_result = std::process::Command::new("git")
            .args([
                "-C",
                repo_path.to_str().unwrap(),
                "remote",
                "rename",
                "origin",
                new_name,
            ])
            .status()
            .context("Failed to rename remote")?;

        if !rename_result.success() {
            // If rename fails, origin might not exist or new name already exists
            // Not critical, continue
            println!("{}", "Note: Could not rename origin remote".yellow());
        }
    }

    Ok(())
}

/// Handle a git conflict by prompting user and executing their choice
fn handle_git_conflict(haxelib_status: &HaxelibStatus) -> Result<()> {
    let haxelib = haxelib_status.lib;
    let repo_path = PathBuf::from(".haxelib")
        .join(haxelib.name_as_commas())
        .join("git");

    // Prompt user for resolution strategy
    let choice = prompt_conflict_resolution(haxelib, haxelib_status)?;

    match choice {
        ConflictResolution::Stash => {
            git_stash_push(&repo_path, haxelib)?;
            install_or_update_git_cli(haxelib)?;
            git_stash_pop(&repo_path, haxelib)?;
        }
        ConflictResolution::Discard => {
            git_discard_changes(&repo_path, haxelib)?;
            install_or_update_git_cli(haxelib)?;
        }
        ConflictResolution::Commit => {
            git_commit_changes(&repo_path, haxelib)?;
            install_or_update_git_cli(haxelib)?;
        }
        ConflictResolution::Skip => {
            println!("Skipping {}", haxelib.name.yellow());
        }
    }

    Ok(())
}

/// Stash changes in the git repository
fn git_stash_push(repo_path: &Path, haxelib: &Haxelib) -> Result<()> {
    println!("Stashing changes in {}...", haxelib.name);

    let stash_message = format!(
        "hmm-rs: auto-stash before updating to {}",
        haxelib.try_vcs_ref().unwrap_or("latest")
    );

    let result = std::process::Command::new("git")
        .args([
            "-C",
            repo_path.to_str().unwrap(),
            "stash",
            "push",
            "-m",
            &stash_message,
        ])
        .output()
        .context("Failed to execute git stash")?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(anyhow!("Failed to stash changes: {}", stderr));
    }

    println!("✓ Changes stashed");
    Ok(())
}

/// Restore stashed changes
fn git_stash_pop(repo_path: &Path, haxelib: &Haxelib) -> Result<()> {
    println!("Restoring stashed changes in {}...", haxelib.name);

    let result = std::process::Command::new("git")
        .args(["-C", repo_path.to_str().unwrap(), "stash", "pop"])
        .output()
        .context("Failed to execute git stash pop")?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);

        if stderr.contains("CONFLICT") {
            println!();
            println!(
                "{}",
                "⚠ Warning: Stash pop created merge conflicts"
                    .yellow()
                    .bold()
            );
            println!("You'll need to resolve them manually in:");
            println!("  {}", repo_path.display().to_string().cyan());
            println!();
            println!("After resolving, run:");
            println!(
                "  {}",
                format!("git -C {} stash drop", repo_path.display())
                    .cyan()
                    .bold()
            );
            println!();
            // Don't return error - let user continue with other libs
            return Ok(());
        }

        return Err(anyhow!("Failed to restore stash: {}", stderr));
    }

    println!("✓ Changes restored");
    Ok(())
}

/// Discard all local changes
fn git_discard_changes(repo_path: &Path, haxelib: &Haxelib) -> Result<()> {
    println!("Discarding changes in {}...", haxelib.name);

    // Reset tracked files
    let reset_result = std::process::Command::new("git")
        .args(["-C", repo_path.to_str().unwrap(), "reset", "--hard", "HEAD"])
        .status()
        .context("Failed to execute git reset")?;

    if !reset_result.success() {
        return Err(anyhow!("Failed to reset changes in {}", haxelib.name));
    }

    // Clean untracked files
    let clean_result = std::process::Command::new("git")
        .args(["-C", repo_path.to_str().unwrap(), "clean", "-fd"])
        .status()
        .context("Failed to execute git clean")?;

    if !clean_result.success() {
        return Err(anyhow!(
            "Failed to clean untracked files in {}",
            haxelib.name
        ));
    }

    println!("✓ Changes discarded");
    Ok(())
}

/// Prompt for commit message and commit changes
fn git_commit_changes(repo_path: &Path, haxelib: &Haxelib) -> Result<()> {
    println!();
    print!("Enter commit message: ");
    stdout().flush()?;

    let mut message = String::new();
    stdin().read_line(&mut message)?;
    let message = message.trim();

    if message.is_empty() {
        return Err(anyhow!("Commit message cannot be empty"));
    }

    println!("Committing changes in {}...", haxelib.name);

    // Stage all changes
    let add_result = std::process::Command::new("git")
        .args(["-C", repo_path.to_str().unwrap(), "add", "-A"])
        .status()
        .context("Failed to execute git add")?;

    if !add_result.success() {
        return Err(anyhow!("Failed to stage changes in {}", haxelib.name));
    }

    // Commit
    let commit_result = std::process::Command::new("git")
        .args(["-C", repo_path.to_str().unwrap(), "commit", "-m", message])
        .output()
        .context("Failed to execute git commit")?;

    if !commit_result.status.success() {
        let stderr = String::from_utf8_lossy(&commit_result.stderr);
        if stderr.contains("nothing to commit") {
            println!(
                "{}",
                "Note: Nothing to commit (changes may have been staged already)".yellow()
            );
            return Ok(());
        }
        return Err(anyhow!("Failed to commit changes: {}", stderr));
    }

    println!("✓ Changes committed");
    Ok(())
}

/// Get a summary of changed files in the git repository
fn get_git_diff_stat(repo_path: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["-C", repo_path.to_str().unwrap(), "diff", "--stat"])
        .output()
        .context("Failed to get git diff stat")?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Ok(String::from("(unable to get diff)"))
    }
}

/// Prompt user for how to resolve a git conflict
fn prompt_conflict_resolution(
    haxelib: &Haxelib,
    status: &HaxelibStatus,
) -> Result<ConflictResolution> {
    let repo_path = PathBuf::from(".haxelib")
        .join(haxelib.name_as_commas())
        .join("git");

    // Get diff stat to show what changed
    let diff_stat = get_git_diff_stat(&repo_path)?;

    println!();
    println!(
        "{}",
        "┌─────────────────────────────────────────────────────".bright_black()
    );
    println!(
        "{} {} {}",
        "│".bright_black(),
        haxelib.name.yellow().bold(),
        "has uncommitted changes".yellow()
    );
    println!(
        "{}",
        "├─────────────────────────────────────────────────────".bright_black()
    );
    println!(
        "{} Current:  {}",
        "│".bright_black(),
        status.installed.as_ref().unwrap().red()
    );
    println!(
        "{} Expected: {}",
        "│".bright_black(),
        status.wants.as_ref().unwrap().green()
    );

    if !diff_stat.trim().is_empty() {
        println!(
            "{}",
            "├─────────────────────────────────────────────────────".bright_black()
        );
        println!("{} Changed files:", "│".bright_black());
        for line in diff_stat.lines() {
            if !line.trim().is_empty() {
                println!("{}  {}", "│".bright_black(), line.bright_black());
            }
        }
    }

    println!(
        "{}",
        "├─────────────────────────────────────────────────────".bright_black()
    );
    println!("{} What would you like to do?", "│".bright_black());
    println!("{}", "│".bright_black());
    println!(
        "{}  {} {} - Save changes temporarily, update, restore",
        "│".bright_black(),
        "[s]".cyan().bold(),
        "Stash".cyan()
    );
    println!(
        "{}  {} {} - Discard all local changes and update",
        "│".bright_black(),
        "[d]".red().bold(),
        "Discard".red()
    );
    println!(
        "{}  {} {} - Commit changes first, then update",
        "│".bright_black(),
        "[c]".green().bold(),
        "Commit".green()
    );
    println!(
        "{}  {} {} - Skip this library for now",
        "│".bright_black(),
        "[k]".yellow().bold(),
        "Skip".yellow()
    );
    println!(
        "{}",
        "└─────────────────────────────────────────────────────".bright_black()
    );

    print!("Choice (s/d/c/k): ");
    stdout().flush()?;

    let mut input = String::new();
    stdin().read_line(&mut input)?;

    match input.trim().to_lowercase().as_str() {
        "s" | "stash" => Ok(ConflictResolution::Stash),
        "d" | "discard" => Ok(ConflictResolution::Discard),
        "c" | "commit" => Ok(ConflictResolution::Commit),
        "k" | "skip" => Ok(ConflictResolution::Skip),
        _ => {
            println!("Invalid choice. Skipping {}.", haxelib.name);
            Ok(ConflictResolution::Skip)
        }
    }
}

pub fn create_current_file(path: &Path, content: &String) -> Result<()> {
    std::fs::create_dir_all(path)?;
    let mut current_version_file = File::create(path.join(".current"))?;
    write!(current_version_file, "{}", content)?;
    Ok(())
}
