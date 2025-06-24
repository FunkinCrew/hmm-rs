use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::{
    hmm::{
        self,
        dependencies::Dependancies,
        haxelib::{Haxelib, HaxelibType},
    },
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

    // Create .haxelib directory if it doesn't exist
    let haxelib_dir = Path::new(".haxelib");
    if !haxelib_dir.exists() {
        fs::create_dir_all(haxelib_dir)?;
    }

    // Create the library directory inside .haxelib
    let lib_dir = haxelib_dir.join(name);
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