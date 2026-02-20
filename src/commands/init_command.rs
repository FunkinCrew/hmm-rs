use crate::hmm;
use anyhow::{anyhow, Context, Result};
use std::path::Path;

pub fn init_hmm() -> Result<()> {
    create_haxelib_folder()?;
    hmm::json::create_empty_hmm_json()
}

pub fn create_haxelib_folder() -> Result<()> {
    create_haxelib_folder_at(Path::new("."))
}

pub fn create_haxelib_folder_at(base: &Path) -> Result<()> {
    let haxelib_path = base.join(".haxelib");
    if haxelib_path.exists() {
        let err_message = format!(
            "{} \n{}",
            "A .haxelib folder already exists in this directory, so it won't be created.",
            "use `hmm-rs clean` to remove the folder"
        );
        Err(anyhow!(err_message))?
    }
    println!("Creating .haxelib/ folder");
    std::fs::create_dir(&haxelib_path).context("Failed to create .haxelib folder")
}

/// Ensures .haxelib/ exists, creating it if missing. Unlike create_haxelib_folder(),
/// this does NOT error if the folder already exists.
pub fn ensure_haxelib_folder() -> Result<()> {
    ensure_haxelib_folder_at(Path::new("."))
}

pub fn ensure_haxelib_folder_at(base: &Path) -> Result<()> {
    let haxelib_path = base.join(".haxelib");
    if !haxelib_path.exists() {
        println!("Creating .haxelib/ folder");
        std::fs::create_dir(&haxelib_path).context("Failed to create .haxelib folder")?;
    }
    Ok(())
}
