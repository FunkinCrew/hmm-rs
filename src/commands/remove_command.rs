use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use owo_colors::OwoColorize;

use crate::hmm::{
    dependencies::Dependancies,
    haxelib::lib_dir_path_for_name,
    json,
};

pub fn remove_haxelibs(
    mut deps: Dependancies,
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
        let lib_path = lib_dir_path_for_name(name);
        if lib_path.exists() {
            std::fs::remove_dir_all(&lib_path)
                .with_context(|| format!("Failed to remove {}", lib_path.display()))?;
        }
        println!("removed {}", name.green().bold());
    }

    deps.dependencies.retain(|h| !to_remove.contains(&h.name));
    json::save_json(deps, json_path)?;

    Ok(())
}
