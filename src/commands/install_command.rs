use crate::commands::check_command::InstallType;
use crate::hmm::dependencies::Dependancies;
use crate::hmm::haxelib::Haxelib;
use crate::hmm::haxelib::HaxelibType;
use anyhow::Ok;
use anyhow::{anyhow, Context, Result};
use bstr::BString;
use console::Emoji;
use futures_util::StreamExt;
use gix::clone;
use gix::create;
use gix::progress::Discard;
use gix::Url as GixUrl;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client as ReqwestClient;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
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
                HaxelibType::Git => install_from_git_using_gix_checkout(install_status.lib)?,
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
        HaxelibType::Git => install_from_git_using_gix_clone(haxelib_status.lib)?,
        lib_type => println!(
            "{}: Installing from {:?} not yet implemented",
            haxelib_status.lib.name.red(),
            lib_type
        ),
    }

    Ok(())
}

pub fn install_from_git_using_gix_clone(haxelib: &Haxelib) -> Result<()> {
    println!("Installing {} from git using clone", haxelib.name);

    let path_with_no_https = haxelib.url().replace("https://", "");

    let clone_url = GixUrl::from_parts(
        gix::url::Scheme::Https,
        None,
        None,
        None,
        None,
        BString::from(path_with_no_https),
        false,
    )
    .context(format!("error creating gix url for {}", haxelib.url()))?;

    let mut clone_path = PathBuf::from(".haxelib").join(&haxelib.name);

    create_current_file(&clone_path, &String::from("git"))?;

    clone_path = clone_path.join("git");

    if let Err(e) = std::fs::create_dir_all(&clone_path) {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            println!("Directory already exists: {:?}", clone_path.as_path());
        } else {
            return Err(anyhow!(
                "Error creating directory: {:?}",
                clone_path.as_path()
            ));
        }
    };

    let mut da_fetch = clone::PrepareFetch::new(
        clone_url,
        clone_path,
        create::Kind::WithWorktree,
        create::Options::default(),
        gix::open::Options::default(),
    )
    .context("error preparing clone")?;

    let repo = da_fetch
        .fetch_then_checkout(Discard, &AtomicBool::new(false))?
        .0
        .main_worktree(Discard, &AtomicBool::new(false))
        .expect("Error checking out worktree")
        .0;

    let submodule_result = repo.submodules()?;

    if let Some(submodule_list) = submodule_result {
        for submodule in submodule_list {
            let submodule_path = submodule.path()?;
            let submodule_url = submodule.url()?;
            println!("Submodule: {} - {}", submodule_path, submodule_url);
        }
    }

    do_commit_checkout(&repo, haxelib)?;

    Ok(())
}

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

pub fn install_from_git_using_gix_checkout(haxelib: &Haxelib) -> Result<()> {
    println!("Updating {} from git using checkout", haxelib.name);

    let discover_result = gix::discover(
        Path::new(".haxelib")
            .join(haxelib.name.as_str())
            .join("git"),
    );

    let repo = match discover_result {
        core::result::Result::Ok(r) => r,
        Err(e) => {
            if e.to_string().contains("not a git repository") {
                return install_from_git_using_gix_clone(haxelib);
            } else {
                return Err(anyhow!("Error discovering git repo: {:?}", e));
            }
        }
    };

    // let fetch_url = repo
    //     .find_fetch_remote(None)?
    //     .url(gix::remote::Direction::Fetch)
    //     .unwrap()
    //     .clone();

    do_commit_checkout(&repo, haxelib)?;

    print_success(haxelib)?;
    Ok(())
}

fn do_commit_checkout(repo: &gix::Repository, haxelib: &Haxelib) -> Result<()> {
    print!("Checking out {}", haxelib.name);
    if let Some(target_ref) = haxelib.vcs_ref.as_ref() {
        println!(" at {}", target_ref);
        let reflog_msg = BString::from("derp?");

        let target_gix_ref = repo.find_reference(target_ref)?.id();

        repo.head_ref()
            .unwrap()
            .unwrap()
            .set_target_id(target_gix_ref, reflog_msg)?;
    }

    Ok(())
}

fn print_success(haxelib: &Haxelib) -> Result<()> {
    // print empty line for readability
    println!();
    println!(
        "{}: {} installed {}",
        haxelib.name.green().bold(),
        haxelib.version_or_ref()?.bright_green(),
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
