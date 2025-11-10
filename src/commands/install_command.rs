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
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use yansi::Paint;
use zip::ZipArchive;

use super::check_command::compare_haxelib_to_hmm;
use super::check_command::HaxelibStatus;

pub fn install_from_hmm(deps: &Dependancies) -> Result<()> {
    let installs_needed = compare_haxelib_to_hmm(deps)?;
    println!(
        "{} dependencies need to be installed",
        installs_needed.len().to_string().bold()
    );

    for install_status in installs_needed.iter() {
        match &install_status.install_type {
            InstallType::Missing => handle_install(install_status)?,
            InstallType::Outdated => match &install_status.lib.haxelib_type {
                HaxelibType::Haxelib => install_from_haxelib(install_status.lib)?,
                HaxelibType::Git => install_or_update_git_cli(install_status.lib)?,
                lib_type => println!(
                    "{}: Installing from {:?} not yet implemented",
                    install_status.lib.name.red(),
                    lib_type
                ),
            },
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

    let output_dir: PathBuf = [".haxelib", haxelib.name.as_str()].iter().collect();

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
        .join(&haxelib.name)
        .join("git");

    let parent_dir = git_dir_path.parent().unwrap();

    // Ensure repository exists (clone if needed)
    if !git_dir_path.exists() {
        println!("Cloning {} (blobless for speed + full history)...", haxelib.name);
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
        .args(&[
            "clone",
            "--filter=blob:none",
            url,
            target_path.to_str().unwrap(),
        ])
        .status()
        .context("Failed to execute git clone")?;

    if blobless_result.success() {
        println!("✓ Blobless clone completed");
        return Ok(());
    }

    // Fallback to regular clone if blobless not supported
    println!("Blobless clone failed, falling back to regular clone...");
    let regular_result = std::process::Command::new("git")
        .args(&[
            "clone",
            url,
            target_path.to_str().unwrap(),
        ])
        .status()
        .context("Failed to execute git clone")?;

    if !regular_result.success() {
        return Err(anyhow!("Git clone failed for {}", haxelib.name));
    }

    println!("✓ Clone completed");
    Ok(())
}

/// Smart checkout: try local first, fetch if commit not found
fn smart_checkout_git_ref(haxelib: &Haxelib, repo_path: &Path) -> Result<()> {
    let target_ref = haxelib.vcs_ref();

    println!("Checking out {} at {}...", haxelib.name, target_ref);

    // Try to checkout locally first
    let checkout_result = std::process::Command::new("git")
        .args(&["-C", repo_path.to_str().unwrap(), "checkout", target_ref])
        .output()
        .context("Failed to execute git checkout")?;

    if checkout_result.status.success() {
        println!("✓ Checked out {} (local)", target_ref);
        return Ok(());
    }

    // Commit not found locally - fetch and retry
    println!("Commit {} not found locally, fetching...", target_ref);

    let fetch_result = std::process::Command::new("git")
        .args(&["-C", repo_path.to_str().unwrap(), "fetch", "origin"])
        .status()
        .context("Failed to execute git fetch")?;

    if !fetch_result.success() {
        return Err(anyhow!("Git fetch failed for {}", haxelib.name));
    }

    // Try checkout again after fetch
    let checkout_retry = std::process::Command::new("git")
        .args(&["-C", repo_path.to_str().unwrap(), "checkout", target_ref])
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
        .args(&[
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

    let version_str = haxelib
        .version_or_ref()
        .unwrap_or("(default)"); // For git repos without explicit ref

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

pub fn create_current_file(path: &Path, content: &String) -> Result<()> {
    std::fs::create_dir_all(path)?;
    let mut current_version_file = File::create(path.join(".current"))?;
    write!(current_version_file, "{}", content)?;
    Ok(())
}
