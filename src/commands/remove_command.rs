use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use owo_colors::OwoColorize;

use crate::hmm::{
    dependencies::Dependancies,
    haxelib::{exact_case_lib_dir_path_for_name, lib_dir_path_for_name},
    json,
};

pub fn remove_haxelibs(
    deps: Dependancies,
    names: &[String],
    json_path: PathBuf,
) -> Result<()> {
    if names.is_empty() {
        return Err(anyhow!("'remove' requires at least one library name"));
    }

    let to_remove: Vec<String> = deps
        .filter_by_names(names)
        .iter()
        .map(|h| h.name.clone())
        .collect();

    if to_remove.is_empty() {
        return Ok(());
    }

    for name in &to_remove {
        // On a case-sensitive filesystem a mixed-case name also has an
        // exact-case symlink, or an exact-case dir from before hmm-rs
        // lowercased. `remove_dir_all` unlinks a symlink without following it.
        for lib_path in [
            lib_dir_path_for_name(name),
            exact_case_lib_dir_path_for_name(name),
        ] {
            if lib_path.symlink_metadata().is_ok() {
                std::fs::remove_dir_all(&lib_path)
                    .with_context(|| format!("Failed to remove {}", lib_path.display()))?;
            }
        }
        println!("removed {}", name.green().bold());
    }

    json::remove_dependencies(&json_path, &to_remove)?;

    Ok(())
}
