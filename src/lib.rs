pub mod commands;
pub mod hmm;

use std::path::PathBuf;

use anyhow::{Ok, Result};

use clap::{Parser, Subcommand};
use shadow_rs::shadow;

shadow!(build);

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(version = build::CLAP_LONG_VERSION)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,

    /// Sets a custom hmm.json file to use
    #[arg(short, long, value_name = "JSON", default_value = "hmm.json")]
    json: Option<PathBuf>,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Lists the dependencies in the hmm.json file (or a file of your choice with --path)
    /// use `hmm-rs check` to see if the dependencies are installed at the correct versions
    #[command(visible_alias = "ls")]
    List {
        /// Specific libraries you want to list, can be multiple
        /// `hmm-rs list lime openfl` will list lime and openfl
        #[arg(value_name = "LIBS")]
        lib: Option<Vec<String>>,
    },
    /// Creates an empty .haxelib/ folder, and an empty hmm.json file
    Init,
    /// Removes local .haxelib directory, useful for full clean reinstalls
    #[command(visible_alias = "cl")]
    Clean,
    /// dumps the dependencies in hmm.json, either to a .hxml file or stdout
    ToHxml {
        /// The path to the hxml file you want to write to
        #[arg(value_name = "HXML")]
        hxml: Option<PathBuf>,
    },
    /// Checks if the dependencies are installed at their correct hmm.json versions
    #[command(visible_alias = "ch")]
    Check,
    /// Installs the dependencies from hmm.json, if they aren't already installed.
    #[command(visible_alias = "i")]
    Install,
    /// Installs a haxelib from lib.haxe.org
    Haxelib {
        /// The name of the haxelib to install
        name: String,
        /// The version of the haxelib to install
        version: Option<String>,
    },
    /// Removes one or more library dependencies from `hmm.json` and the `.haxelib/` folder
    #[command(visible_alias = "rm")]
    Remove {
        /// The library(s) you wish to remove, can be multiple
        #[arg(value_name = "LIBS")]
        lib: Vec<String>,
    },
    /// Adds a local development dependency to hmm.json
    Dev {
        /// The name of the haxelib
        name: String,
        /// The file system path (absolute or relative)
        path: String,
    },
}

pub fn run() -> Result<()> {
    let args = Cli::parse();

    let path = args.json.clone().unwrap();
    let load_deps = || hmm::json::read_json(&path);

    match args.cmd {
        Commands::List { lib } => hmm::json::read_json(&path)?.print_string_list(&lib)?,
        Commands::Init => commands::init_command::init_hmm()?,
        Commands::Clean => commands::clean_command::remove_haxelib_folder()?,
        Commands::ToHxml { hxml } => commands::tohxml_command::dump_to_hxml(&load_deps()?, hxml)?,
        Commands::Check => commands::check_command::check(&load_deps()?)?,
        Commands::Install => commands::install_command::install_from_hmm(&load_deps()?)?,
        Commands::Haxelib { name, version } => {
            commands::haxelib_command::install_haxelib(&name, &version, load_deps()?, path)?
        }
        Commands::Remove { lib: _ } => commands::remove_command::remove_haxelibs()?,
        Commands::Dev { name, path } => commands::dev_command::add_dev_dependency(
            &name,
            &path,
            load_deps()?,
            args.json.unwrap(),
        )?,
    }
    Ok(())
}

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert();
}
