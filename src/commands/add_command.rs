use std::path::PathBuf;

use anyhow::Result;

use crate::{
    commands::{git_command, haxelib_command},
    hmm::dependencies::Dependancies,
    AddArgs,
};

pub fn add_dependency(add_args: AddArgs, deps: Dependancies, path: PathBuf) -> Result<()> {
    // parse_library_name(&add_args.name);
    match &add_args.git {
        Some(git_url) => {
            git_command::install_git(&add_args.name, git_url.as_str(), &None, deps, path)?
        }
        None => haxelib_command::install_haxelib(&add_args.name, &None, deps, path)?,
    }

    Ok(())
}
