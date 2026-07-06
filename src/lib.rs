pub mod commands;
pub mod hmm;

use std::path::PathBuf;

use anyhow::{Ok, Result};

use clap::{Args, Parser, Subcommand, ValueEnum};
use shadow_rs::shadow;

use crate::commands::add_command;

shadow!(build);

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(version = build::CLAP_LONG_VERSION)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,

    #[command(flatten)]
    global_opts: GlobalOpts,
}

/// Optional list of library names to scope a command to a subset of `hmm.json` deps.
/// Empty means: apply to all dependencies. Names not in `hmm.json` print a warning and are skipped.
#[derive(Debug, Args, Clone)]
pub struct LibraryFilter {
    /// Library names to operate on. If omitted, applies to all dependencies in hmm.json.
    #[arg(value_name = "LIBS")]
    pub lib: Vec<String>,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Lists the dependencies in the hmm.json file (or a file of your choice with --path)
    /// use `hmm-rs check` to see if the dependencies are installed at the correct versions
    #[command(visible_alias = "ls")]
    List {
        #[command(flatten)]
        filter: LibraryFilter,
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
    /// Checks if the dependencies are installed at their correct hmm.json versions.
    /// Optionally specify library names to check only those.
    #[command(visible_alias = "ch")]
    Check {
        #[command(flatten)]
        filter: LibraryFilter,
    },
    /// Installs the dependencies from hmm.json, if they aren't already installed.
    /// Optionally specify library names to install only those.
    #[command(visible_alias = "i")]
    Install {
        #[command(flatten)]
        filter: LibraryFilter,
    },
    Add(AddArgs),
    /// Installs one or more haxelibs from lib.haxe.org. Each name may be `lib` or `lib@version`.
    Haxelib {
        /// Library specs to install. Each is `name` or `name@version` (e.g. `lime` or `lime@5.0.0`).
        #[arg(required = true, num_args = 1..)]
        names: Vec<String>,
    },
    /// Installs a library from a git repository
    Git {
        /// The name of the library
        name: String,
        /// The git repository URL (e.g., https://github.com/user/repo)
        url: String,
        /// Optional git ref (branch, tag, or commit SHA). If not specified, uses default branch
        #[arg(value_name = "REF")]
        git_ref: Option<String>,
        /// Optional subdirectory in the repo where the library (haxelib.json) lives
        #[arg(value_name = "DIR")]
        dir: Option<String>,
    },
    /// Removes one or more library dependencies from `hmm.json` and the `.haxelib/` folder
    #[command(visible_alias = "rm")]
    Remove {
        #[command(flatten)]
        filter: LibraryFilter,
    },
    /// Adds a local development dependency to hmm.json
    Dev {
        /// The name of the haxelib
        name: String,
        /// The file system path (absolute or relative)
        path: String,
    },
    /// Check for and install updates to hmm-rs itself
    #[command(visible_alias = "self-update")]
    Upgrade {
        /// Only check for updates without installing
        #[arg(long)]
        check: bool,
    },
    /// Locks dependencies to their currently installed versions
    Lock {
        #[command(subcommand)]
        subcommand: Option<LockCommands>,

        /// Use full commit IDs instead of shortened ones for git repositories
        #[arg(short = 'l', long = "long-id")]
        long_id: bool,

        #[command(flatten)]
        filter: LibraryFilter,
    },
}

#[derive(Debug, Args, Clone)]
pub struct AddArgs {
    /// One or more library names. With `--git URL`, exactly one name is required.
    #[arg(required = true, num_args = 1..)]
    pub names: Vec<String>,
    #[arg(long, value_name = "URL")]
    pub git: Option<String>,
    /// Optional git ref (branch, tag, or commit SHA), used with `--git`. If not specified, uses default branch.
    #[arg(long = "ref", value_name = "REF")]
    pub git_ref: Option<String>,
    /// Optional subdirectory in the repo where the library lives, used with `--git`.
    #[arg(long = "dir", value_name = "DIR")]
    pub dir: Option<String>,
}

#[derive(Debug, Args)]
struct GlobalOpts {
    /// Color
    #[arg(long, value_enum, global = true, default_value_t = Color::Auto)]
    color: Color,
    /// Sets a custom hmm.json file to use
    #[arg(short, long, value_name = "JSON", default_value = "hmm.json")]
    json: Option<PathBuf>,
    /// Verbosity level (can be specified multiple times, -v or -vvvv)
    #[arg(long, short, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
    /// Separator used in git remote names derived from URLs (default: ".").
    /// Falls back to $HMM_REMOTE_SEPARATOR if unset.
    #[arg(long, global = true, value_name = "SEP")]
    remote_separator: Option<String>,
    //... other global options
}

#[derive(Clone, Debug, ValueEnum)]
enum Color {
    Always,
    Auto,
    Never,
}

#[derive(Subcommand, Debug, Clone)]
enum LockCommands {
    /// Check if all dependencies are locked to specific versions
    Check,
}

pub fn run() -> Result<()> {
    let args = Cli::parse();

    let path = args.global_opts.json.clone().unwrap();
    let load_deps = || hmm::json::read_json(&path);
    let remote_separator = commands::install_command::resolve_remote_separator(
        args.global_opts.remote_separator.as_deref(),
    );

    match args.cmd {
        Commands::Add(add_args) => {
            add_command::add_dependency(add_args, load_deps()?, path, &remote_separator)?
        }
        Commands::List { filter } => hmm::json::read_json(&path)?.print_string_list(&filter.lib)?,
        Commands::Init => commands::init_command::init_hmm()?,
        Commands::Clean => commands::clean_command::remove_haxelib_folder()?,
        Commands::ToHxml { hxml } => commands::tohxml_command::dump_to_hxml(&load_deps()?, hxml)?,
        Commands::Check { filter } => commands::check_command::check(&load_deps()?, &filter.lib)?,
        Commands::Install { filter } => commands::install_command::install_from_hmm(
            &load_deps()?,
            &filter.lib,
            &remote_separator,
        )?,
        Commands::Haxelib { names } => {
            commands::haxelib_command::install_haxelibs(&names, load_deps()?, path)?
        }
        Commands::Git {
            name,
            url,
            git_ref,
            dir,
        } => commands::git_command::install_git(
            &name,
            &url,
            &git_ref,
            &dir,
            load_deps()?,
            path,
            &remote_separator,
        )?,
        Commands::Remove { filter } => {
            commands::remove_command::remove_haxelibs(load_deps()?, &filter.lib, path)?
        }
        Commands::Upgrade { check } => commands::upgrade_command::upgrade(check)?,
        Commands::Dev { name, path } => commands::dev_command::add_dev_dependency(
            &name,
            &path,
            load_deps()?,
            args.global_opts.json.clone().unwrap(),
        )?,
        Commands::Lock {
            subcommand,
            long_id,
            filter,
        } => match subcommand {
            Some(LockCommands::Check) => commands::lock_command::check_locked(&load_deps()?)?,
            None => commands::lock_command::lock_dependencies(
                &load_deps()?,
                &filter.lib,
                args.global_opts.json.unwrap(),
                long_id,
            )?,
        },
    }
    Ok(())
}

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert();
}
