use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::hmm::{
    self,
    dependencies::Dependancies,
    haxelib::{lib_dir_path_for_name, Haxelib, HaxelibType},
};

pub fn add_dev_dependency(
    name: &str,
    path: &str,
    mut deps: Dependancies,
    json_path: PathBuf,
) -> Result<()> {
    // Convert to absolute path
    let absolute_path = Path::new(path).canonicalize()?;

    let dev_haxelib = Haxelib {
        name: name.to_string(),
        haxelib_type: HaxelibType::Dev,
        vcs_ref: None,
        dir: None,
        path: Some(path.to_string()),
        url: None,
        version: None,
    };

    // Create the library directory inside .haxelib (create_dir_all handles parent creation)
    let lib_dir = lib_dir_path_for_name(name);
    if !lib_dir.exists() {
        fs::create_dir_all(&lib_dir)?;
    }

    // Create the .dev file with the absolute path
    let dev_file_path = lib_dir.join(".dev");
    let mut dev_file = fs::File::create(dev_file_path)?;
    dev_file.write_all(absolute_path.to_string_lossy().as_bytes())?;

    deps.dependencies.push(dev_haxelib);
    hmm::json::save_json(deps, json_path)?;
    Ok(())
}
