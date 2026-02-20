use anyhow::{anyhow, Context, Result};
use std::path::Path;
use yansi::Paint;

pub fn remove_haxelib_folder() -> Result<()> {
    remove_haxelib_folder_at(Path::new("."))
}

pub fn remove_haxelib_folder_at(base: &Path) -> Result<()> {
    let haxelib_path = base.join(".haxelib");
    if !haxelib_path.exists() {
        Err(anyhow!(
            "A .haxelib folder does not exist in this directory, so it cannot be removed."
                .bright_red()
                .bold()
        ))?
    }
    println!("Removing .haxelib/ folder");
    std::fs::remove_dir_all(&haxelib_path).context("Failed to remove .haxelib folder")
}
