use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::{
    commands::{git_command, haxelib_command},
    hmm::dependencies::Dependancies,
    AddArgs,
};

pub fn add_dependency(
    add_args: AddArgs,
    deps: Dependancies,
    path: PathBuf,
    separator: &str,
) -> Result<()> {
    match &add_args.git {
        Some(git_url) => {
            if add_args.names.len() != 1 {
                return Err(anyhow!(
                    "--git installs accept exactly one library name (got {})",
                    add_args.names.len()
                ));
            }
            git_command::install_git(
                &add_args.names[0],
                git_url.as_str(),
                &add_args.git_ref,
                &add_args.dir,
                deps,
                path,
                separator,
            )?;
        }
        None => {
            haxelib_command::install_haxelibs(&add_args.names, deps, path)?;
        }
    }

    Ok(())
}
