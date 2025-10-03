use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use yansi::Paint;

use crate::hmm::dependencies::Dependancies;
use crate::hmm::haxelib::{Haxelib, HaxelibType};
use crate::hmm::json;

pub fn lock_dependencies(
    deps: &Dependancies,
    libs: &Option<Vec<String>>,
    json_path: PathBuf,
    long_id: bool,
) -> Result<()> {
    let mut updated_deps = deps.clone();

    // Determine which libraries to lock
    let libs_to_lock: Vec<&Haxelib> = if let Some(lib_names) = libs {
        // Lock only specified libraries
        lib_names
            .iter()
            .map(|name| {
                deps.get_haxelib(name)
                    .map_err(|_| anyhow!("Library '{}' not found in hmm.json", name))
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        // Lock all libraries
        deps.dependencies.iter().collect()
    };

    println!("Locking {} dependencies...", libs_to_lock.len().bold());

    let mut locked_count = 0;
    let mut skipped_count = 0;
    let mut error_count = 0;

    for lib in updated_deps.dependencies.iter_mut() {
        // Check if this library should be locked
        if !libs_to_lock.iter().any(|l| l.name == lib.name) {
            continue;
        }

        match lock_dependency(lib, long_id) {
            Ok(LockResult::Locked(version)) => {
                println!(
                    "{} {} locked to {}",
                    lib.name.green().bold(),
                    format!("[{:?}]", lib.haxelib_type).green().dim(),
                    version.green()
                );
                locked_count += 1;
            }
            Ok(LockResult::Skipped(reason)) => {
                println!(
                    "{} {} skipped: {}",
                    lib.name.yellow().bold(),
                    format!("[{:?}]", lib.haxelib_type).yellow().dim(),
                    reason.yellow()
                );
                skipped_count += 1;
            }
            Ok(LockResult::AlreadyLocked(_version)) => {
                // Don't print anything for already locked dependencies
                skipped_count += 1;
            }
            Err(e) => {
                println!(
                    "{} {} error: {}",
                    lib.name.red().bold(),
                    format!("[{:?}]", lib.haxelib_type).red().dim(),
                    e.to_string().red()
                );
                error_count += 1;
            }
        }
    }

    if locked_count > 0 {
        json::save_json(updated_deps, json_path)?;
    }

    println!();
    println!(
        "Summary: {} locked, {} skipped/already locked, {} errors",
        locked_count.bold(),
        skipped_count.bold(),
        error_count.bold()
    );

    if error_count > 0 {
        return Err(anyhow!(
            "Failed to lock {} dependencies. Run `hmm install` to ensure all dependencies are installed.",
            error_count
        ));
    }

    Ok(())
}

enum LockResult {
    Locked(String),
    Skipped(String),
    AlreadyLocked(String),
}

fn lock_dependency(lib: &mut Haxelib, long_id: bool) -> Result<LockResult> {
    match lib.haxelib_type {
        HaxelibType::Haxelib => lock_haxelib_dependency(lib),
        HaxelibType::Git => lock_git_dependency(lib, long_id),
        HaxelibType::Dev => Ok(LockResult::Skipped(
            "dev dependencies are already locked by path".to_string(),
        )),
        HaxelibType::Mecurial => Ok(LockResult::Skipped(
            "mercurial not yet supported".to_string(),
        )),
    }
}

fn lock_haxelib_dependency(lib: &mut Haxelib) -> Result<LockResult> {
    // Check if already locked
    if lib.version.is_some() {
        return Ok(LockResult::AlreadyLocked(
            lib.version.as_ref().unwrap().clone(),
        ));
    }

    // Read the .current file to get installed version
    let lib_path = get_lib_path(&lib.name);
    let current_file = lib_path.join(".current");

    if !current_file.exists() {
        return Err(anyhow!(
            "Library not installed (no .current file found). Run `hmm install` first."
        ));
    }

    let mut current_version = String::new();
    File::open(&current_file)?.read_to_string(&mut current_version)?;

    // Update the library with the locked version
    lib.version = Some(current_version.clone());

    Ok(LockResult::Locked(current_version))
}

fn lock_git_dependency(lib: &mut Haxelib, long_id: bool) -> Result<LockResult> {
    let lib_path = get_lib_path(&lib.name);
    let git_path = lib_path.join("git");

    if !git_path.exists() {
        return Err(anyhow!(
            "Git repository not cloned. Run `hmm install` first."
        ));
    }

    let repo = gix::discover(&git_path)?;
    let head_commit = repo.head_commit()?;

    // Use full or short commit ID based on flag
    let commit_sha = if long_id {
        head_commit.id().to_string()
    } else {
        head_commit.id().shorten_or_id().to_string()
    };

    // Check if already locked to this exact commit
    if let Some(ref current_ref) = lib.vcs_ref {
        if current_ref == &commit_sha {
            return Ok(LockResult::AlreadyLocked(commit_sha));
        }
    }

    // Update the ref to the commit SHA
    lib.vcs_ref = Some(commit_sha.clone());

    Ok(LockResult::Locked(commit_sha))
}

fn get_lib_path(lib_name: &str) -> PathBuf {
    let comma_replace = lib_name.replace(".", ",");
    Path::new(".haxelib").join(comma_replace)
}

pub fn check_locked(deps: &Dependancies) -> Result<()> {
    let mut unlocked_libs = Vec::new();
    let mut locked_count = 0;

    for lib in deps.dependencies.iter() {
        match is_locked(lib) {
            LockStatus::Locked => {
                // Don't print anything for locked dependencies
                locked_count += 1;
            }
            LockStatus::NotLocked(reason) => {
                println!(
                    "{} {} is not locked: {}",
                    lib.name.red().bold(),
                    format!("[{:?}]", lib.haxelib_type).red().dim(),
                    reason.red()
                );
                unlocked_libs.push(&lib.name);
            }
            LockStatus::NotApplicable => {
                // Don't print anything for dev dependencies
                locked_count += 1;
            }
        }
    }

    println!();
    println!(
        "{} / {} dependencies are locked",
        locked_count.bold(),
        deps.dependencies.len().bold()
    );

    if !unlocked_libs.is_empty() {
        println!();
        println!("Run {} to lock all dependencies", "hmm lock".yellow().bold());
        return Err(anyhow!(
            "{} dependencies are not locked",
            unlocked_libs.len()
        ));
    }

    Ok(())
}

enum LockStatus {
    Locked,
    NotLocked(String),
    NotApplicable,
}

fn is_locked(lib: &Haxelib) -> LockStatus {
    match lib.haxelib_type {
        HaxelibType::Haxelib => {
            if lib.version.is_some() {
                LockStatus::Locked
            } else {
                LockStatus::NotLocked("no version specified".to_string())
            }
        }
        HaxelibType::Git => {
            if lib.vcs_ref.is_some() {
                LockStatus::Locked
            } else {
                LockStatus::NotLocked("no ref specified".to_string())
            }
        }
        HaxelibType::Dev => LockStatus::NotApplicable,
        HaxelibType::Mecurial => {
            if lib.vcs_ref.is_some() {
                LockStatus::Locked
            } else {
                LockStatus::NotLocked("no ref specified".to_string())
            }
        }
    }
}
