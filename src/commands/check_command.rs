use std::fs::File;

use crate::hmm::dependencies::Dependancies;
use crate::hmm::haxelib::{Haxelib, HaxelibType};
use anyhow::{anyhow, Context, Result};
use console::Emoji;
use owo_colors::OwoColorize;
use std::io::Read;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;

pub struct HaxelibStatus<'a> {
    pub lib: &'a Haxelib,
    pub install_type: InstallType,
    pub wants: Option<String>,
    pub installed: Option<String>,
}

// First, define the install type enum
#[derive(Debug, PartialEq)]
pub enum InstallType {
    Missing,          // Needs to be installed
    MissingGit,       // Needs to be cloned
    MissingDevLink,   // Git repo present at correct commit, but subdir `.dev` link is missing
    StaleDevLink, // Git repo present at correct commit, but a `.dev` link exists with no subdir configured
    Outdated,     // Installed but wrong version
    AlreadyInstalled, // Correctly installed
    Conflict,     // Version conflicts between dependencies
    NotLocked,    // Version in hmm.json isn't locked to anything, prompt to lock?
    // The check itself errored; this lib fails without aborting the rest
    CheckFailed(String),
}

impl InstallType {
    /// Nothing for `install` to do and nothing for `check` to fail on. An
    /// unpinned lib is satisfied by whatever version is installed.
    pub fn is_satisfied(&self) -> bool {
        matches!(self, InstallType::AlreadyInstalled | InstallType::NotLocked)
    }
}

impl<'a> HaxelibStatus<'a> {
    pub fn new(
        lib: &'a Haxelib,
        install_type: InstallType,
        wants: Option<String>,
        installed: Option<String>,
    ) -> Self {
        Self {
            lib,
            install_type,
            wants,
            installed,
        }
    }
}

pub fn check(deps: &Dependancies, names: &[String], verbose: bool) -> Result<()> {
    let filtered = deps.filter_by_names(names);
    let total = filtered.len();
    let installs = compare_haxelib_to_hmm(&filtered, verbose)?;
    let failed: Vec<&str> = installs
        .iter()
        .filter(|i| !i.install_type.is_satisfied())
        .map(|i| i.lib.name.as_str())
        .collect();
    let installed_count = total - failed.len();
    println!(
        "{} / {} dependencie(s) are installed at the correct versions",
        installed_count.bold(),
        total.bold()
    );
    if !verbose && installed_count < total {
        println!(
            "{} dependencie(s) are out of date or have changes",
            (total - installed_count).bold()
        );
    }
    if !failed.is_empty() {
        return Err(anyhow!(
            "{} dependencie(s) are not installed or have the wrong version: {}",
            failed.len(),
            failed.join(", ")
        ));
    }
    Ok(())
}

pub fn compare_haxelib_to_hmm<'a>(
    haxelibs: &[&'a Haxelib],
    verbose: bool,
) -> Result<Vec<HaxelibStatus<'a>>> {
    // Libs are checked on worker threads; results are printed here, in
    // hmm.json order, as soon as every lib before them is done.
    let workers = thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(haxelibs.len());
    let next = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel();

    thread::scope(|s| {
        for _ in 0..workers {
            let (tx, next) = (tx.clone(), &next);
            s.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(&haxelib) = haxelibs.get(i) else {
                    break;
                };
                // One lib that can't be checked must not hide the state of the others.
                let haxelib_status = check_dependency(haxelib).unwrap_or_else(|e| {
                    HaxelibStatus::new(
                        haxelib,
                        InstallType::CheckFailed(format!("{e:#}")),
                        get_wants(haxelib),
                        None,
                    )
                });
                if tx.send((i, haxelib_status)).is_err() {
                    break;
                }
            });
        }
        drop(tx);

        let mut done: Vec<Option<HaxelibStatus>> = haxelibs.iter().map(|_| None).collect();
        let mut install_status = Vec::new();

        for (i, haxelib) in haxelibs.iter().enumerate() {
            if verbose {
                // transient progress line, cleared once the check below completes
                println!(
                    "Checking {} {}",
                    haxelib.name.bold().yellow(),
                    Emoji("🤔", "[...]")
                );
            }

            let haxelib_status = loop {
                if let Some(status) = done[i].take() {
                    break status;
                }
                let (j, status) = rx.recv()?;
                done[j] = Some(status);
            };

            if verbose {
                // clear the "Checking ..." progress line, then show the result
                print!("\x1B[1A\x1B[2K");
                print_install_status(&haxelib_status)?;
            } else if haxelib_status.install_type != InstallType::AlreadyInstalled {
                // quiet mode: only report libs that need attention
                print_install_status(&haxelib_status)?;
            }

            install_status.push(haxelib_status);
        }

        Ok(install_status)
    })
}

fn check_dependency(haxelib: &Haxelib) -> Result<HaxelibStatus<'_>> {
    let lib_path = haxelib.lib_dir_path();

    if !lib_path.exists() {
        return Ok(HaxelibStatus::new(
            haxelib,
            InstallType::Missing,
            get_wants(haxelib),
            None,
        ));
    }

    if haxelib.haxelib_type == HaxelibType::Dev {
        return Ok(check_dev_dependency(haxelib, &lib_path));
    }

    // `haxelib path` resolves a lib with a `.dev` marker to that path, not to
    // `.current`. A git worktree can point its libs at another checkout's
    // `.haxelib` this way instead of installing them again.
    let dev_path = super::dev_command::read_dev_file(&haxelib.name);
    if let (HaxelibType::Haxelib, Some(dev)) = (&haxelib.haxelib_type, &dev_path) {
        return Ok(check_haxelib_dev_redirect(haxelib, dev));
    }

    // Read the .current file
    let mut current_version = String::new();
    match File::open(lib_path.join(".current")) {
        Ok(mut f) => f.read_to_string(&mut current_version)?,
        // a `.dev` marker stands in for `.current`
        _ if dev_path.is_some() => 0,
        _ => {
            return Ok(HaxelibStatus::new(
                haxelib,
                InstallType::Missing,
                get_wants(haxelib),
                None,
            ));
        }
    };
    // haxelib always trims .current/.dev on read; a trailing newline written
    // by another tool must not read as a different version.
    current_version = current_version.trim().to_string();

    match haxelib.haxelib_type {
        HaxelibType::Haxelib => match haxelib.version.as_ref() {
            Some(v) => {
                if v != &current_version {
                    return Ok(HaxelibStatus::new(
                        haxelib,
                        InstallType::Outdated,
                        get_wants(haxelib),
                        Some(current_version.to_string()),
                    ));
                }
            }
            None => {
                return Ok(HaxelibStatus::new(
                    haxelib,
                    InstallType::NotLocked,
                    None,
                    Some(current_version.to_string()),
                ))
            }
        },
        HaxelibType::Git => {
            let repo_path = lib_path.join("git");

            // With a `.dev` marker, the checkout it points into is the one
            // that compiles; the lib's own git/ need not exist.
            if dev_path.is_none() && !repo_path.exists() {
                return Ok(HaxelibStatus::new(
                    haxelib,
                    InstallType::MissingGit,
                    get_wants(haxelib),
                    None,
                ));
            }

            let checkout = dev_path.as_deref().unwrap_or(&repo_path);
            let repo = match gix::discover(checkout) {
                Ok(r) => r,
                Err(_) if dev_path.is_some() => {
                    return Ok(HaxelibStatus::new(
                        haxelib,
                        InstallType::Outdated,
                        get_wants(haxelib),
                        Some(format!(
                            "{} (.dev target is not a git checkout)",
                            checkout.display()
                        )),
                    ));
                }
                // Reported with the status, not printed here: this runs on a
                // worker thread, so a print would land out of order.
                Err(e) => {
                    return Ok(HaxelibStatus::new(
                        haxelib,
                        InstallType::Missing,
                        get_wants(haxelib),
                        Some(format!("None ({e})")),
                    ));
                }
            };

            let head_ref = repo
                .head_commit()
                .context("could not read HEAD — repo may be empty or corrupt")?;

            let vcs_ref = haxelib
                .vcs_ref
                .as_ref()
                .ok_or_else(|| anyhow!("'ref' field is required for git type"))?;
            // Resolved like `git rev-parse <ref>^{commit}` in the local clone:
            // branches, tags (annotated ones peeled), `HEAD`, `main~1`, short
            // SHAs. A ref the clone doesn't have yet (a tag or branch created
            // upstream after the clone) can't match, so install fetches it.
            let intended_commit = repo
                .rev_parse_single(vcs_ref.as_str())
                .ok()
                .and_then(|id| id.object().ok()?.peel_to_commit().ok());
            let mismatch = match intended_commit {
                Some(c) if c.id == head_ref.id => None,
                Some(_) => Some("wrong commit"),
                None => Some("ref not found locally"),
            };

            // A checkout elsewhere (reached through `.dev`) is never touched by
            // install, which clones its own instead, so its local changes are
            // its owner's business and can't conflict with an update.
            let workdir = repo.workdir();
            let is_own_checkout = workdir.is_some_and(|w| same_dir(w, &repo_path));
            let has_local_changes = is_own_checkout && repo.is_dirty()?;
            let via_dev = match is_own_checkout {
                true => String::new(),
                false => format!(", via .dev {}", checkout.display()),
            };

            match (mismatch, has_local_changes) {
                (Some(mismatch), true) => {
                    return Ok(HaxelibStatus::new(
                        haxelib,
                        InstallType::Conflict,
                        get_wants(haxelib),
                        Some(format!("{} ({mismatch} + local changes)", head_ref.id())),
                    ));
                }
                (Some(mismatch), false) => {
                    return Ok(HaxelibStatus::new(
                        haxelib,
                        InstallType::Outdated,
                        get_wants(haxelib),
                        Some(format!("{} ({mismatch}{via_dev})", head_ref.id())),
                    ));
                }
                (None, true) => {
                    return Ok(HaxelibStatus::new(
                        haxelib,
                        InstallType::Conflict,
                        get_wants(haxelib),
                        Some(format!("{} (local changes)", head_ref.id())),
                    ));
                }
                (None, false) => {
                    // Continue to the end of the function - correct version
                }
            }

            // The checkout can be right while `.current` still names a haxelib
            // version dir (left behind when the lib was switched back to git
            // over an existing git/ checkout). `haxelib path` follows
            // `.current`, so it must select git/.
            let current = std::fs::read_to_string(lib_path.join(".current"))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if is_own_checkout && current != "git" {
                return Ok(HaxelibStatus::new(
                    haxelib,
                    InstallType::Outdated,
                    get_wants(haxelib),
                    Some(if current.is_empty() {
                        "none (.current missing)".to_string()
                    } else {
                        current
                    }),
                ));
            }

            // we have a correct version, so we're going to update the current_version to the vcs_ref
            current_version = vcs_ref.to_string();

            // A git dep with a `dir` subdirectory needs a `.dev` link into that subdir.
            // If the repo is at the right commit but the link is missing, flag it so
            // install can (re)create it without a full re-clone.
            let subdir = haxelib
                .dir
                .as_deref()
                .map(str::trim)
                .filter(|d| !d.is_empty());
            // `haxelib path` resolves to the `.dev` target itself, so it must be
            // the checkout's `dir`, or its root when there is none.
            let dev_at_subdir = dev_path.as_deref().is_some_and(|dev| {
                workdir.is_some_and(|w| same_dir(dev, &w.join(subdir.unwrap_or(""))))
            });
            match &dev_path {
                None if subdir.is_some() => {
                    return Ok(HaxelibStatus::new(
                        haxelib,
                        InstallType::MissingDevLink,
                        get_wants(haxelib),
                        None,
                    ));
                }
                Some(dev) if !dev_at_subdir => {
                    // In the lib's own checkout only the link is wrong (`dir`
                    // was changed or dropped, or the lib used to be a dev dep),
                    // so install rewrites or removes it without a re-clone.
                    if is_own_checkout {
                        return Ok(HaxelibStatus::new(
                            haxelib,
                            InstallType::StaleDevLink,
                            get_wants(haxelib),
                            None,
                        ));
                    }
                    return Ok(HaxelibStatus::new(
                        haxelib,
                        InstallType::Outdated,
                        get_wants(haxelib),
                        Some(format!(
                            "{} (.dev target is not the checkout's {})",
                            dev.display(),
                            subdir.map_or("root".to_string(), |d| format!("'{d}' subdir"))
                        )),
                    ));
                }
                _ => {}
            }
        }
        _ => {}
    }

    Ok(HaxelibStatus::new(
        haxelib,
        InstallType::AlreadyInstalled,
        Some(current_version),
        None,
    ))
}

/// A haxelib dep with a `.dev` marker compiles against that path, at the
/// version in its haxelib.json (what `haxelib path` reports as `-D name=version`).
fn check_haxelib_dev_redirect<'a>(haxelib: &'a Haxelib, dev_path: &Path) -> HaxelibStatus<'a> {
    let installed = std::fs::read_to_string(dev_path.join("haxelib.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|json| Some(json.get("version")?.as_str()?.to_string()));
    match (&haxelib.version, installed) {
        (None, installed) => HaxelibStatus::new(haxelib, InstallType::NotLocked, None, installed),
        (Some(wants), Some(installed)) if *wants == installed => HaxelibStatus::new(
            haxelib,
            InstallType::AlreadyInstalled,
            Some(installed),
            None,
        ),
        (Some(_), installed) => HaxelibStatus::new(
            haxelib,
            InstallType::Outdated,
            get_wants(haxelib),
            Some(format!(
                "{} (via .dev {})",
                installed.as_deref().unwrap_or("no haxelib.json version"),
                dev_path.display()
            )),
        ),
    }
}

/// Whether two paths name the same existing directory.
fn same_dir(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

/// A dev dep is installed when its `.dev` marker points at the hmm.json `path`.
/// A `.current` alone doesn't count, since `haxelib path` would then resolve a
/// version dir instead of the dev path.
fn check_dev_dependency<'a>(haxelib: &'a Haxelib, lib_path: &Path) -> HaxelibStatus<'a> {
    // A target that doesn't exist can't be linked; install reports why.
    let (Ok(installed), Ok(wants)) = (
        std::fs::read_to_string(lib_path.join(".dev")),
        super::dev_command::resolve_dev_path(haxelib),
    ) else {
        return HaxelibStatus::new(haxelib, InstallType::Missing, get_wants(haxelib), None);
    };
    let installed = installed.trim();
    let wants_str = wants.display().to_string();
    // Compared as paths so a trailing slash (haxelib 4.2.0 writes one) is not a difference.
    if Path::new(installed) != wants {
        return HaxelibStatus::new(
            haxelib,
            InstallType::Outdated,
            Some(wants_str),
            Some(installed.to_string()),
        );
    }
    HaxelibStatus::new(
        haxelib,
        InstallType::AlreadyInstalled,
        Some(wants_str),
        None,
    )
}

fn print_install_status(haxelib_status: &HaxelibStatus) -> Result<()> {
    match &haxelib_status.install_type {
        InstallType::Missing => {
            println!(
                "{} {}",
                haxelib_status.lib.name.red().bold(),
                "is not installed".red()
            );
            println!(
                "Expected: {} | Installed: {}",
                haxelib_status.wants.as_deref().unwrap_or("unknown").red(),
                haxelib_status.installed.as_deref().unwrap_or("None").red()
            );
        }
        InstallType::MissingGit => {
            println!(
                "{} {}",
                haxelib_status.lib.name.red().bold(),
                "is not cloned / installed (via git)".red()
            );
            println!(
                "Expected: {} | Installed: {}",
                haxelib_status.wants.as_deref().unwrap_or("unknown").red(),
                "None".red()
            );
        }
        InstallType::MissingDevLink => {
            println!(
                "{} {}",
                haxelib_status.lib.name.yellow().bold(),
                format!(
                    "is missing its dev link into subdir '{}'",
                    haxelib_status.lib.dir.as_deref().unwrap_or("")
                )
                .yellow()
            );
        }
        InstallType::StaleDevLink => {
            let detail = match haxelib_status.lib.dir.as_deref().map(str::trim) {
                Some(dir) if !dir.is_empty() => format!("outside its subdir '{dir}'"),
                _ => "but no subdir configured".to_string(),
            };
            println!(
                "{} {}",
                haxelib_status.lib.name.yellow().bold(),
                format!("has a stale dev link {detail}").yellow()
            );
        }
        InstallType::Outdated => {
            println!(
                "{} {}",
                haxelib_status.lib.name.red().bold(),
                "is not at the correct version".red()
            );
            println!(
                "Expected: {} | Installed: {}",
                haxelib_status.wants.as_deref().unwrap_or("unknown").red(),
                haxelib_status
                    .installed
                    .as_deref()
                    .unwrap_or("unknown")
                    .red()
            );
        }
        InstallType::AlreadyInstalled => {
            let inner = format!(
                "{} [{:?}]: {} {}",
                haxelib_status.lib.name.green().bold(),
                haxelib_status.lib.haxelib_type.green().dimmed(),
                haxelib_status
                    .wants
                    .as_deref()
                    .unwrap_or("unknown")
                    .green()
                    .dimmed(),
                Emoji("✅", "[✔️]")
            );
            println!("{}", inner.bright_green());
        }
        InstallType::Conflict => {
            println!(
                "{} {}",
                haxelib_status.lib.name.red().bold(),
                "has issues".red()
            );
            if let Some(details) = &haxelib_status.installed {
                println!("Current: {}", details.red());
            }
            if let Some(expected) = &haxelib_status.wants {
                println!("Expected: {}", expected.red());
            }
        }
        InstallType::CheckFailed(err) => {
            println!(
                "{} {} {}",
                haxelib_status.lib.name.red().bold(),
                "could not be checked:".red(),
                err.red()
            );
        }
        InstallType::NotLocked => {
            println!(
                "{} {}",
                haxelib_status.lib.name.yellow().bold(),
                "is not locked to a specific version (`\"version\": null` in json file). ".yellow(),
            );
            println!(
                "{} {}",
                "`hmm lock` to version:".bright_yellow(),
                haxelib_status
                    .installed
                    .as_deref()
                    .unwrap_or("unknown")
                    .yellow()
            )
        }
    }
    Ok(())
}

/// Returns the haxelib version, the git ref, or the dev path of the haxelib
fn get_wants(haxelib: &Haxelib) -> Option<String> {
    match haxelib.haxelib_type {
        HaxelibType::Haxelib => haxelib.version.clone(),
        HaxelibType::Git => haxelib.vcs_ref.clone(),
        HaxelibType::Dev => haxelib.path.clone(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_wants() {
        let haxelib = Haxelib {
            name: "test".to_string(),
            haxelib_type: HaxelibType::Haxelib,
            vcs_ref: None,
            dir: None,
            url: None,
            version: Some("1.0.0".to_string()),
            path: None,
        };
        assert_eq!(get_wants(&haxelib), Some("1.0.0".to_string()));

        let haxelib = Haxelib {
            name: "test".to_string(),
            haxelib_type: HaxelibType::Git,
            vcs_ref: Some("master".to_string()),
            dir: None,
            url: None,
            version: None,
            path: None,
        };
        assert_eq!(get_wants(&haxelib), Some("master".to_string()));
    }

    #[test]
    fn test_get_wants_dev() {
        let haxelib = Haxelib {
            name: "local-lib".to_string(),
            haxelib_type: HaxelibType::Dev,
            vcs_ref: None,
            dir: None,
            url: None,
            version: None,
            path: Some("/some/path".to_string()),
        };
        assert_eq!(get_wants(&haxelib), Some("/some/path".to_string()));
    }

    #[test]
    fn test_get_wants_mercurial() {
        let haxelib = Haxelib {
            name: "hg-lib".to_string(),
            haxelib_type: HaxelibType::Mecurial,
            vcs_ref: None,
            dir: None,
            url: None,
            version: None,
            path: None,
        };
        assert_eq!(get_wants(&haxelib), None);
    }
}
