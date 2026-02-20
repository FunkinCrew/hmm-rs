use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use self_update::cargo_crate_version;
use self_update::backends::github;

const REPO_OWNER: &str = "FunkinCrew";
const REPO_NAME: &str = "hmm-rs";

pub fn upgrade(check_only: bool) -> Result<()> {
    let current = cargo_crate_version!();
    println!("Current version: v{}", current);

    if check_only {
        check_for_update(current)?;
    } else {
        perform_upgrade(current)?;
    }

    Ok(())
}

fn check_for_update(current: &str) -> Result<()> {
    let releases = github::ReleaseList::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .build()
        .context("Failed to configure release list")?
        .fetch()
        .context("Failed to fetch releases from GitHub")?;

    let latest = match releases.first() {
        Some(release) => release,
        None => {
            println!("No releases published yet.");
            return Ok(());
        }
    };

    let latest_version = &latest.version;

    if semver::Version::parse(latest_version)? > semver::Version::parse(current)? {
        println!(
            "{} v{} is available (you have v{})",
            "Update available:".green().bold(),
            latest_version,
            current
        );
        println!(
            "Run {} to install it.",
            "hmm-rs upgrade".cyan().bold()
        );
    } else {
        println!(
            "{} v{} is the latest version.",
            "Up to date:".green().bold(),
            current
        );
    }

    Ok(())
}

fn perform_upgrade(current: &str) -> Result<()> {
    let status = github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("hmm-rs")
        .show_download_progress(true)
        .no_confirm(true)
        .current_version(current)
        .build()
        .context("Failed to configure updater")?
        .update()
        .context("Failed to update binary")?;

    match status {
        self_update::Status::UpToDate(v) => {
            println!(
                "{} v{} is the latest version.",
                "Up to date:".green().bold(),
                v
            );
        }
        self_update::Status::Updated(v) => {
            println!(
                "{} hmm-rs has been updated to v{}!",
                "Updated:".green().bold(),
                v
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_valid_semver() {
        let current = cargo_crate_version!();
        assert!(
            semver::Version::parse(current).is_ok(),
            "Cargo version '{}' should be valid semver",
            current
        );
    }
}
