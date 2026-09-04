use crate::commands::check_command::InstallType;
use crate::hmm::dependencies::Dependancies;
use crate::hmm::haxelib::Haxelib;
use crate::hmm::haxelib::HaxelibType;
use anyhow::Ok;
use anyhow::{anyhow, Context, Result};
use console::Emoji;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressFinish, ProgressStyle};
use reqwest::Client as ReqwestClient;
use std::env;
use std::fs::File;
use std::io::{self, stdin, stdout, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::time::Duration;
use owo_colors::OwoColorize;
use zip::ZipArchive;

use super::check_command::compare_haxelib_to_hmm;
use super::check_command::HaxelibStatus;

pub const DEFAULT_REMOTE_SEPARATOR: &str = ".";

/// Resolve the remote-name separator: CLI flag > $HMM_REMOTE_SEPARATOR > default.
/// Empty strings are treated as "not set" and fall through.
pub fn resolve_remote_separator(flag: Option<&str>) -> String {
    if let Some(s) = flag {
        if !s.is_empty() {
            return s.to_string();
        }
    }
    if let std::result::Result::Ok(s) = env::var("HMM_REMOTE_SEPARATOR") {
        if !s.is_empty() {
            return s;
        }
    }
    DEFAULT_REMOTE_SEPARATOR.to_string()
}

fn path_to_str(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| anyhow!("Path contains invalid UTF-8: {}", path.display()))
}

/// Spinner refresh interval while a bar waits on the network or on git.
const BAR_TICK: Duration = Duration::from_millis(100);

fn download_bar_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{spinner:.green} {prefix:.bold} downloading [{wide_bar:.yellow/red}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
    )
    .expect("valid progress template")
}

fn download_spinner_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{spinner:.green} {prefix:.bold} downloading {bytes} ({bytes_per_sec}, {elapsed})",
    )
    .expect("valid progress template")
}

fn extract_bar_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{spinner:.green} {prefix:.bold} extracting [{wide_bar:.cyan/blue}] {pos}/{len}",
    )
    .expect("valid progress template")
}

fn git_bar_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{spinner:.green} {prefix:.bold} {msg} [{wide_bar:.yellow/red}] {percent}% ({pos}/{len}) {elapsed}",
    )
    .expect("valid progress template")
}

fn git_spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.green} {prefix:.bold} {msg} {elapsed}")
        .expect("valid progress template")
}

/// `[2/5]` during a batch install, `None` for a single-library command.
fn counter_tag(counter: Option<(usize, usize)>) -> Option<String> {
    counter.map(|(i, n)| format!("[{i}/{n}]"))
}

/// Label in front of every progress bar: `[2/5] flixel` or plain `flixel`.
fn bar_prefix(haxelib: &Haxelib, counter: Option<(usize, usize)>) -> String {
    match counter_tag(counter) {
        Some(tag) => format!("{tag} {}", haxelib.name),
        None => haxelib.name.clone(),
    }
}

/// Bold `[2/5] ` for the lead line printed before a bar, empty otherwise (so
/// single-library commands emit no stray style codes).
fn counter_lead(counter: Option<(usize, usize)>) -> String {
    counter_tag(counter)
        .map(|tag| format!("{} ", tag.bold()))
        .unwrap_or_default()
}

/// Common setup for every install bar: label, clear-on-drop (so an early `?`
/// return leaves no stale bar row) and a spinner that keeps moving between
/// updates. Hidden bars (no TTY, e.g. under the test runner) get no ticker
/// thread.
fn install_bar(pb: ProgressBar, style: ProgressStyle, prefix: &str) -> ProgressBar {
    let pb = pb
        .with_style(style)
        .with_prefix(prefix.to_string())
        .with_finish(ProgressFinish::AndClear);
    if !pb.is_hidden() {
        pb.enable_steady_tick(BAR_TICK);
    }
    pb
}

/// A bar for one git invocation; starts as a spinner and turns into a bar as
/// soon as git reports a phase with a known total.
fn git_bar(prefix: &str, msg: &str) -> ProgressBar {
    install_bar(ProgressBar::no_length(), git_spinner_style(), prefix).with_message(msg.to_string())
}

/// One progress update parsed from `git --progress` output, e.g.
/// `Receiving objects:  45% (1234/2742), 1.20 MiB | 3.40 MiB/s`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitProgress {
    /// Phase name as git prints it (`Receiving objects`, `Resolving deltas`, …).
    pub title: String,
    pub current: u64,
    /// `None` for the count-only form (`Enumerating objects: 1234`).
    pub total: Option<u64>,
    /// Whatever follows the counts, normally the throughput tail
    /// (`, 1.20 MiB | 3.40 MiB/s`); empty when there is none.
    pub detail: String,
    /// The line carried git's `, done.` suffix.
    pub done: bool,
}

/// Parses one `\r`/`\n`-delimited segment of git's stderr.
///
/// Accepts the two shapes git's `progress.c` prints, `Title: NN% (cur/total)`
/// and `Title: cur`, each optionally followed by a throughput tail and
/// `, done.`, with or without the `remote: ` sideband prefix and its padding.
/// Anything else (`fatal: …`, `HEAD is now at …`, hints) is `None`.
pub fn parse_git_progress(line: &str) -> Option<GitProgress> {
    let line = line.trim();
    let line = line.strip_prefix("remote: ").unwrap_or(line).trim();
    let (title, rest) = line.split_once(": ")?;
    if !is_progress_title(title) {
        return None;
    }
    let (rest, done) = match rest.strip_suffix(", done.") {
        Some(rest) => (rest, true),
        None => (rest, false),
    };
    // git pads the percentage (`%3u%%`), so there may be extra spaces here.
    let rest = rest.trim_start();
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let (number, after) = rest.split_at(digits);

    if let Some(counts) = after.strip_prefix("% (") {
        let close = counts.find(')')?;
        let (current, total) = counts[..close].split_once('/')?;
        return Some(GitProgress {
            title: title.to_string(),
            current: current.parse().ok()?,
            total: Some(total.parse().ok()?),
            detail: counts[close + 1..].to_string(),
            done,
        });
    }

    if !after.is_empty() && !after.starts_with(", ") {
        return None;
    }
    Some(GitProgress {
        title: title.to_string(),
        current: number.parse().ok()?,
        total: None,
        detail: after.to_string(),
        done,
    })
}

/// Progress titles are plain words (`Receiving objects`); this keeps
/// `fatal: …`, `HEAD is now at 1a2b3c fix: …` and a bare sideband prefix from
/// being taken for progress.
fn is_progress_title(title: &str) -> bool {
    !title.is_empty()
        && title != "remote"
        && title.bytes().all(|b| b.is_ascii_alphabetic() || b == b' ')
}

/// On a narrow terminal git prints the title alone (`Receiving objects:`) and
/// the counters on the following lines; returns the title in that case.
fn split_progress_title(line: &str) -> Option<&str> {
    let line = line.strip_prefix("remote: ").unwrap_or(line);
    let title = line.strip_suffix(':')?;
    is_progress_title(title).then_some(title)
}

/// Exit status of a git invocation plus the stderr lines that were not
/// progress updates (errors, hints, notes), for error reporting.
struct GitRun {
    status: ExitStatus,
    stderr: Vec<String>,
}

impl GitRun {
    /// Our message followed by git's own explanation, when it gave one.
    fn error(&self, msg: impl std::fmt::Display) -> anyhow::Error {
        if self.stderr.is_empty() {
            anyhow!("{msg}")
        } else {
            anyhow!("{msg}\n{}", self.stderr.join("\n"))
        }
    }
}

/// Routes git's stderr into a progress bar: progress updates drive the bar,
/// everything else is kept for error reporting. `warning:` lines are echoed
/// right away (e.g. "filtering not recognized by server, ignoring").
struct GitStderrSink<'a> {
    pb: &'a ProgressBar,
    lines: Vec<String>,
    /// Title of a split progress line (`Receiving objects:` on its own row,
    /// which git emits when the terminal is narrow) awaiting its counters.
    pending_title: Option<String>,
    bar_shown: bool,
}

impl<'a> GitStderrSink<'a> {
    fn new(pb: &'a ProgressBar) -> Self {
        Self {
            pb,
            lines: Vec::new(),
            pending_title: None,
            bar_shown: false,
        }
    }

    fn push(&mut self, raw: &[u8]) {
        let text = String::from_utf8_lossy(raw);
        let text = text.trim();
        if text.is_empty() {
            return;
        }

        let progress = parse_git_progress(text).or_else(|| {
            let title = self.pending_title.as_deref()?;
            parse_git_progress(&format!("{title}: {text}"))
        });
        if let Some(progress) = progress {
            self.apply(&progress);
            return;
        }
        if let Some(title) = split_progress_title(text) {
            self.pending_title = Some(title.to_string());
            return;
        }
        if text.starts_with("warning:") {
            self.pb.suspend(|| eprintln!("{text}"));
        }
        self.lines.push(text.to_string());
    }

    fn apply(&mut self, progress: &GitProgress) {
        match progress.total {
            Some(total) => {
                if !self.bar_shown {
                    self.pb.set_style(git_bar_style());
                    self.bar_shown = true;
                }
                if self.pb.length() != Some(total) {
                    self.pb.set_length(total);
                }
                self.pb.set_position(progress.current);
                let throughput = progress.detail.trim_start_matches(", ");
                self.pb.set_message(if throughput.is_empty() {
                    progress.title.clone()
                } else {
                    format!("{} ({throughput})", progress.title)
                });
            }
            None => {
                if self.bar_shown {
                    self.pb.set_style(git_spinner_style());
                    self.pb.unset_length();
                    self.bar_shown = false;
                }
                self.pb.set_message(format!(
                    "{}: {}{}",
                    progress.title, progress.current, progress.detail
                ));
            }
        }
    }
}

/// Runs `git <args>` (which should include `--progress`) with stderr piped
/// through `pb`. stdout is discarded so nothing interleaves with the bar;
/// stdin stays inherited so credential and ssh prompts, which go through the
/// tty, keep working.
fn run_git_with_progress(args: &[&str], pb: &ProgressBar) -> Result<GitRun> {
    let mut child = std::process::Command::new("git")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;

    let stderr = child.stderr.take().expect("stderr is piped");
    let mut sink = GitStderrSink::new(pb);
    let mut segment = Vec::new();
    for byte in BufReader::new(stderr).bytes() {
        match byte? {
            b'\r' | b'\n' => {
                sink.push(&segment);
                segment.clear();
            }
            byte => segment.push(byte),
        }
    }
    sink.push(&segment);

    let status = child.wait()?;
    Ok(GitRun {
        status,
        stderr: sink.lines,
    })
}

/// User's choice for resolving git conflicts
enum ConflictResolution {
    Stash,   // Stash changes, update, restore
    Discard, // Discard all changes and update
    Commit,  // Commit changes first, then update
    Skip,    // Skip this library
}

pub fn install_from_hmm(deps: &Dependancies, libs: &[String], separator: &str) -> Result<()> {
    super::init_command::ensure_haxelib_folder()?;

    let filtered = deps.filter_by_names(libs);
    let installs_needed = compare_haxelib_to_hmm(&filtered, false)?;
    // compare_haxelib_to_hmm reports every dep; only the ones needing work get
    // a slot in the [n/N] counter.
    let pending: Vec<&HaxelibStatus> = installs_needed
        .iter()
        .filter(|s| s.install_type != InstallType::AlreadyInstalled)
        .collect();
    let total = pending.len();
    println!(
        "{} dependencies need to be installed",
        total.to_string().bold()
    );

    let mut failures: Vec<(String, anyhow::Error)> = Vec::new();

    for (i, install_status) in pending.iter().enumerate() {
        let counter = Some((i + 1, total));
        let result = match &install_status.install_type {
            InstallType::Missing => handle_install(install_status, separator, counter),
            InstallType::MissingGit => handle_install(install_status, separator, counter),
            InstallType::MissingDevLink | InstallType::StaleDevLink => {
                ensure_git_subdir_dev_link(install_status.lib)
            }
            InstallType::Outdated => match &install_status.lib.haxelib_type {
                HaxelibType::Haxelib => install_from_haxelib(install_status.lib, counter),
                HaxelibType::Git => {
                    install_or_update_git_cli(install_status.lib, separator, counter)
                }
                lib_type => {
                    println!(
                        "{}: Installing from {:?} not yet implemented",
                        install_status.lib.name.red(),
                        lib_type
                    );
                    Ok(())
                }
            },
            InstallType::Conflict => {
                // Handle git conflicts interactively
                handle_git_conflict(install_status, separator, counter)
            }
            InstallType::AlreadyInstalled => Ok(()), // do nothing on things already installed at the right version
            _ => {
                println!(
                    "{} {:?}: Not implemented",
                    install_status.lib.name, install_status.install_type
                );
                Ok(())
            }
        };

        if let Err(e) = result {
            println!(
                "⚠ {} {}: {:#}",
                "Failed to install".red(),
                install_status.lib.name.red().bold(),
                e
            );
            failures.push((install_status.lib.name.clone(), e));
        }
    }

    if !failures.is_empty() {
        println!();
        println!(
            "⚠ {} of {} dependencies failed to install:",
            failures.len().to_string().red().bold(),
            total.to_string().bold()
        );
        for (name, err) in &failures {
            println!("  - {}: {:#}", name.red(), err);
        }
        let noun = if failures.len() == 1 {
            "dependency"
        } else {
            "dependencies"
        };
        return Err(anyhow!("{} {} failed to install", failures.len(), noun));
    }

    Ok(())
}

pub fn handle_install(
    haxelib_status: &HaxelibStatus,
    separator: &str,
    counter: Option<(usize, usize)>,
) -> Result<()> {
    match &haxelib_status.lib.haxelib_type {
        HaxelibType::Haxelib => install_from_haxelib(haxelib_status.lib, counter)?,
        HaxelibType::Git => install_or_update_git_cli(haxelib_status.lib, separator, counter)?,
        lib_type => println!(
            "{}: Installing from {:?} not yet implemented",
            haxelib_status.lib.name.red(),
            lib_type
        ),
    }

    Ok(())
}

#[tokio::main]
pub async fn install_from_haxelib(haxelib: &Haxelib, counter: Option<(usize, usize)>) -> Result<()> {
    println!(
        "{}Downloading: {} - {} - {}",
        counter_lead(counter),
        haxelib.name.bold(),
        "lib.haxe.org".yellow().bold(),
        haxelib.download_url()?.bold()
    );

    let response = ReqwestClient::new()
        .get(haxelib.download_url()?)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!("Failed to download: HTTP {}", response.status()));
    }

    // No Content-Length (chunked transfer) only means no ETA: count bytes on a
    // spinner instead of failing the install.
    let expected_total_size = response.content_length();
    let prefix = bar_prefix(haxelib, counter);
    let pb = match expected_total_size {
        Some(len) => install_bar(ProgressBar::new(len), download_bar_style(), &prefix),
        None => install_bar(ProgressBar::no_length(), download_spinner_style(), &prefix),
    };

    let tmp_dir = env::temp_dir().join(format!("{}.zip", haxelib.name));

    {
        let mut file = File::create(&tmp_dir)?;
        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            let chunk = item?;
            file.write_all(&chunk)?;
            pb.inc(chunk.len() as u64);
        }

        file.flush()?;
    }

    pb.finish_and_clear();
    vprintln!(
        "{}: {} done downloading from {}",
        haxelib.name.green().bold(),
        haxelib.version()?.bright_green(),
        "Haxelib".yellow().bold()
    );

    if let Some(expected_total_size) = expected_total_size {
        let metadata = std::fs::metadata(&tmp_dir)?;
        if metadata.len() != expected_total_size {
            return Err(anyhow!(
                "Download incomplete: expected {} bytes, got {} bytes",
                expected_total_size,
                metadata.len()
            ));
        }
    }

    let output_dir = haxelib.lib_dir_path();

    if let Err(e) = std::fs::create_dir(&output_dir) {
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(anyhow!(
                "Error creating directory: {:?}",
                output_dir.as_path()
            ));
        }
    }

    // unzipping
    let archive =
        File::open(&tmp_dir).context(format!("Failed to open downloaded zip: {:?}", tmp_dir))?;

    let mut zip_file =
        ZipArchive::new(archive).context("Error opening zip file - file may be corrupted")?;

    let unzipped_output_dir = output_dir.join(haxelib.version_as_commas()?);

    // Find the base path by locating the shallowest haxelib.json in the ZIP.
    // Some haxelib packages nest all files under a wrapper directory (e.g. "release/"),
    // and we need to strip that prefix to match how `haxelib install` behaves.
    let base_path = {
        let mut best: Option<String> = None;
        let mut best_depth = usize::MAX;
        for i in 0..zip_file.len() {
            let entry = zip_file.by_index(i)?;
            let name = entry.name().replace('\\', "/");
            if name.ends_with("haxelib.json") {
                let depth = name.split('/').count();
                if depth < best_depth || (depth == best_depth && Some(&name) < best.as_ref()) {
                    best_depth = depth;
                    best = Some(name[..name.len() - "haxelib.json".len()].to_string());
                }
            }
        }
        best.unwrap_or_default()
    };

    // Extract entries, stripping the base_path prefix
    let extract_pb = install_bar(
        ProgressBar::new(zip_file.len() as u64),
        extract_bar_style(),
        &prefix,
    );
    for i in 0..zip_file.len() {
        extract_pb.inc(1);
        let mut entry = zip_file.by_index(i)?;

        let Some(out_path) = sanitize_zip_entry(&base_path, entry.name(), &unzipped_output_dir)
        else {
            continue;
        };

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = File::create(&out_path)?;
            io::copy(&mut entry, &mut outfile)?;
        }
    }
    extract_pb.finish_and_clear();

    // Written only after extraction succeeds: a mid-extraction failure must not
    // leave a .current claiming a version whose directory is partial or absent
    // (`check` never inspects the version dir).
    create_current_file(&output_dir, &haxelib.version()?.to_string())?;

    // hmm.json pins a haxelib version now, so a `.dev` marker left over from
    // when this lib was a git dep with a `dir` must go: `haxelib path`
    // prefers `.dev` over `.current`.
    if crate::commands::dev_command::remove_dev_file(&haxelib.name)? {
        println!(
            "{}: development directory unset",
            haxelib.name.green().bold()
        );
    }

    std::fs::remove_file(&tmp_dir)?;

    print_success(haxelib)?;
    Ok(())
}

/// Unified git installer using git CLI for optimal performance and reliability
/// - Uses blobless clone (--filter=blob:none) for fast initial download with full history
/// - Smart checkout: tries local first, fetches only if commit not found
/// - Properly handles submodules with --init --recursive
pub fn install_or_update_git_cli(
    haxelib: &Haxelib,
    separator: &str,
    counter: Option<(usize, usize)>,
) -> Result<()> {
    let git_dir_path = haxelib.git_repo_path();
    let parent_dir = haxelib.lib_dir_path();
    let prefix = bar_prefix(haxelib, counter);

    // Ensure repository exists (clone if needed)
    if !git_dir_path.exists() {
        println!(
            "{}Cloning {} (blobless for speed + full history)...",
            counter_lead(counter),
            haxelib.name
        );
        clone_blobless_git_repo(haxelib, &git_dir_path, separator, &prefix)?;
    } else {
        println!(
            "{}Repository exists, checking out {}...",
            counter_lead(counter),
            haxelib.name
        );
    }

    // Written on every git install, not just a fresh clone: git/ can already
    // exist from before the lib was switched to a haxelib version, leaving
    // .current pointing at that version dir instead of git/.
    create_current_file(&parent_dir, &String::from("git"))?;

    // Checkout the specified commit/ref (if provided)
    if haxelib.vcs_ref.is_some() {
        smart_checkout_git_ref(haxelib, &git_dir_path, separator, &prefix)?;
    } else {
        vprintln!("No ref specified, using repository's default branch");
    }

    // Update submodules to match the checked out commit
    update_git_submodules(&git_dir_path, &prefix)?;

    // If a subdirectory is configured, point a `.dev` marker into it.
    ensure_git_subdir_dev_link(haxelib)?;

    print_success(haxelib)?;
    Ok(())
}

/// Clone with --filter=blob:none for fast download with full commit history
/// Falls back to regular clone if blobless is not supported
fn clone_blobless_git_repo(
    haxelib: &Haxelib,
    target_path: &Path,
    separator: &str,
    prefix: &str,
) -> Result<()> {
    let url = haxelib.url()?;
    let target = path_to_str(target_path)?;

    // Try blobless clone first (fast, full history)
    let pb = git_bar(prefix, "cloning (blobless)");
    let blobless_result = run_git_with_progress(
        &["clone", "--progress", "--filter=blob:none", url, target],
        &pb,
    )
    .context("Failed to execute git clone")?;
    pb.finish_and_clear();

    if blobless_result.status.success() {
        vprintln!("✓ Blobless clone completed");
    } else {
        // Fallback to regular clone if blobless not supported
        println!("Blobless clone failed, falling back to regular clone...");
        for line in &blobless_result.stderr {
            vprintln!("  {}", line.bright_black());
        }
        let pb = git_bar(prefix, "cloning");
        let regular_result = run_git_with_progress(&["clone", "--progress", url, target], &pb)
            .context("Failed to execute git clone")?;
        pb.finish_and_clear();

        if !regular_result.status.success() {
            return Err(regular_result.error(format!("Git clone failed for {}", haxelib.name)));
        }

        vprintln!("✓ Clone completed");
    }

    // Parse remote name from URL and rename origin
    let remote_name = parse_remote_name_from_url(url, separator)?;
    rename_origin_remote(target_path, &remote_name)?;

    Ok(())
}

/// Smart checkout: try local first, fetch if commit not found
fn smart_checkout_git_ref(
    haxelib: &Haxelib,
    repo_path: &Path,
    separator: &str,
    prefix: &str,
) -> Result<()> {
    let target_ref = haxelib.vcs_ref()?;
    let url = haxelib.url()?;
    let repo = path_to_str(repo_path)?;

    vprintln!("Checking out {} at {}...", haxelib.name, target_ref);

    // Ensure remote exists with correct name and URL
    let remote_name = parse_remote_name_from_url(url, separator)?;
    ensure_git_remote(repo_path, &remote_name, url)?;

    // Try to checkout locally first. In a blobless clone this is also where
    // the file contents get downloaded, silently: git's lazy blob fetch never
    // reports progress to a pipe, so the spinner is what shows the wait.
    let pb = git_bar(prefix, &format!("checking out {target_ref}"));
    let checkout_result =
        run_git_with_progress(&["-C", repo, "checkout", "--progress", target_ref], &pb)
            .context("Failed to execute git checkout")?;
    pb.finish_and_clear();

    if checkout_result.status.success() {
        vprintln!("✓ Checked out {} (local)", target_ref);
        return Ok(());
    }

    // Commit not found locally - fetch from managed remote and retry
    vprintln!(
        "Commit {} not found locally, fetching from {}...",
        target_ref, remote_name
    );

    let pb = git_bar(prefix, &format!("fetching from {remote_name}"));
    let fetch_result = run_git_with_progress(&["-C", repo, "fetch", "--progress", &remote_name], &pb)
        .context("Failed to execute git fetch")?;
    pb.finish_and_clear();

    if !fetch_result.status.success() {
        vprintln!(
            "Standard fetch failed, retrying with {} (skips negotiation)...",
            "--refetch".cyan()
        );

        let pb = git_bar(prefix, &format!("refetching from {remote_name}"));
        let refetch_result = run_git_with_progress(
            &["-C", repo, "fetch", "--progress", "--refetch", &remote_name],
            &pb,
        )
        .context("Failed to execute git fetch --refetch")?;
        pb.finish_and_clear();

        if !refetch_result.status.success() {
            return Err(refetch_result.error(format!(
                "Git fetch failed for {} from {} (tried both standard and --refetch)",
                haxelib.name, remote_name
            )));
        }
    }

    // Try checkout again after fetch
    let pb = git_bar(prefix, &format!("checking out {target_ref}"));
    let checkout_retry =
        run_git_with_progress(&["-C", repo, "checkout", "--progress", target_ref], &pb)
            .context("Failed to execute git checkout after fetch")?;
    pb.finish_and_clear();

    if !checkout_retry.status.success() {
        return Err(checkout_retry.error(format!(
            "Commit {} not found even after fetch for {}",
            target_ref, haxelib.name
        )));
    }

    vprintln!("✓ Checked out {} (after fetch)", target_ref);
    Ok(())
}

/// Initialize and update submodules recursively
fn update_git_submodules(repo_path: &Path, prefix: &str) -> Result<()> {
    let pb = git_bar(prefix, "updating submodules");
    let result = run_git_with_progress(
        &[
            "-C",
            path_to_str(repo_path)?,
            "submodule",
            "update",
            "--init",
            "--recursive",
            "--progress",
        ],
        &pb,
    )
    .context("Failed to execute git submodule update")?;
    pb.finish_and_clear();

    if !result.status.success() {
        return Err(result.error("Git submodule update failed"));
    }

    Ok(())
}

fn print_success(haxelib: &Haxelib) -> Result<()> {
    // print empty line for readability
    println!();

    let version_str = haxelib.version_or_ref().unwrap_or("(default)"); // For git repos without explicit ref

    println!(
        "{}: {} installed {}",
        haxelib.name.green().bold(),
        version_str.bright_green(),
        Emoji("✅", "[✔️]")
    );
    // print an empty line, for readability between downloads
    println!();
    Ok(())
}

/// Maps a zip entry name to the path it should be extracted to under `dest`,
/// or `None` if the entry must be skipped.
///
/// Entries are skipped when they fall outside `base_path`, are empty once the
/// prefix is stripped, or would escape `dest` (absolute paths, drive prefixes,
/// and `..` components). Only `Normal` path components are ever joined onto
/// `dest`, so the returned path is always strictly inside it.
pub fn sanitize_zip_entry(base_path: &str, entry_name: &str, dest: &Path) -> Option<PathBuf> {
    let full_name = entry_name.replace('\\', "/");
    let relative = full_name.strip_prefix(base_path)?;
    if relative.is_empty() {
        return None;
    }

    let mut out_path = dest.to_path_buf();
    let mut pushed = false;
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => {
                out_path.push(part);
                pushed = true;
            }
            // A `.` is harmless and simply skipped; anything else could escape.
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    pushed.then_some(out_path)
}

/// Parse a remote name from a git URL (format: username<sep>repo).
pub fn parse_remote_name_from_url(url: &str, separator: &str) -> Result<String> {
    // Handle various URL formats:
    // https://github.com/user/repo.git
    // https://github.com/user/repo
    // git@github.com:user/repo.git
    // ssh://git@github.com/user/repo.git

    let url = url.trim();

    // Remove common prefixes
    let path = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("ssh://")
        .trim_start_matches("git@");

    // Split by colon (for ssh format) or slash
    let parts: Vec<&str> = if path.contains(':') {
        path.split(':').collect()
    } else {
        vec![path]
    };

    // Get the path part (after domain)
    let repo_path = parts
        .last()
        .ok_or_else(|| anyhow!("Invalid git URL: {}", url))?;

    // Split by slashes and get last two parts (user/repo)
    let path_parts: Vec<&str> = repo_path.split('/').filter(|s| !s.is_empty()).collect();

    if path_parts.len() < 2 {
        return Err(anyhow!("Could not parse username/repo from URL: {}", url));
    }

    let username = path_parts[path_parts.len() - 2];
    let mut repo = path_parts[path_parts.len() - 1];

    // Remove .git suffix if present
    repo = repo.trim_end_matches(".git");

    Ok(format!("{}{}{}", username, separator, repo))
}

fn is_partial_clone(repo_path: &Path) -> bool {
    let output = std::process::Command::new("git")
        .args([
            "-C",
            path_to_str(repo_path).unwrap_or("."),
            "config",
            "--get-regexp",
            r"remote\..*\.partialclonefilter",
        ])
        .output();

    match output {
        std::result::Result::Ok(o) => o.status.success(),
        std::result::Result::Err(_) => false,
    }
}

fn configure_remote_as_promisor(repo_path: &Path, remote_name: &str) -> Result<()> {
    let repo_path_str = path_to_str(repo_path)?;

    let check = std::process::Command::new("git")
        .args([
            "-C",
            repo_path_str,
            "config",
            &format!("remote.{}.promisor", remote_name),
        ])
        .output()
        .context("Failed to check promisor config")?;

    if check.status.success() {
        return Ok(());
    }

    let promisor_result = std::process::Command::new("git")
        .args([
            "-C",
            repo_path_str,
            "config",
            &format!("remote.{}.promisor", remote_name),
            "true",
        ])
        .status()
        .context("Failed to set promisor config")?;
    if !promisor_result.success() {
        return Err(anyhow!("git config remote.{}.promisor failed", remote_name));
    }

    let filter_result = std::process::Command::new("git")
        .args([
            "-C",
            repo_path_str,
            "config",
            &format!("remote.{}.partialclonefilter", remote_name),
            "blob:none",
        ])
        .status()
        .context("Failed to set partialclonefilter config")?;
    if !filter_result.success() {
        return Err(anyhow!(
            "git config remote.{}.partialclonefilter failed",
            remote_name
        ));
    }

    Ok(())
}

/// Ensure a git remote exists with the proper name
fn ensure_git_remote(repo_path: &Path, remote_name: &str, url: &str) -> Result<()> {
    // Check if remote exists
    let check_remote = std::process::Command::new("git")
        .args([
            "-C",
            path_to_str(repo_path)?,
            "remote",
            "get-url",
            remote_name,
        ])
        .output()
        .context("Failed to check git remote")?;

    if check_remote.status.success() {
        // Remote exists - verify URL matches
        let existing_url = String::from_utf8_lossy(&check_remote.stdout)
            .trim()
            .to_string();

        if existing_url != url {
            vprintln!("Updating remote {} URL...", remote_name.cyan());

            let update_result = std::process::Command::new("git")
                .args([
                    "-C",
                    path_to_str(repo_path)?,
                    "remote",
                    "set-url",
                    remote_name,
                    url,
                ])
                .status()
                .context("Failed to update remote URL")?;

            if !update_result.success() {
                return Err(anyhow!("Failed to update remote {} URL", remote_name));
            }
        }
    } else {
        // Remote doesn't exist - create it
        vprintln!("Adding remote {}...", remote_name.cyan());

        let add_result = std::process::Command::new("git")
            .args([
                "-C",
                path_to_str(repo_path)?,
                "remote",
                "add",
                remote_name,
                url,
            ])
            .status()
            .context("Failed to add git remote")?;

        if !add_result.success() {
            return Err(anyhow!("Failed to add remote {}", remote_name));
        }
    }

    if is_partial_clone(repo_path) {
        configure_remote_as_promisor(repo_path, remote_name)?;
    }

    Ok(())
}

/// Rename 'origin' remote to a better name after cloning
fn rename_origin_remote(repo_path: &Path, new_name: &str) -> Result<()> {
    // Check if origin exists
    let check_origin = std::process::Command::new("git")
        .args([
            "-C",
            path_to_str(repo_path)?,
            "remote",
            "get-url",
            "origin",
        ])
        .output()
        .context("Failed to check origin remote")?;

    if check_origin.status.success() {
        vprintln!("Renaming remote origin → {}...", new_name.cyan());

        let rename_result = std::process::Command::new("git")
            .args([
                "-C",
                path_to_str(repo_path)?,
                "remote",
                "rename",
                "origin",
                new_name,
            ])
            .status()
            .context("Failed to rename remote")?;

        if !rename_result.success() {
            // If rename fails, origin might not exist or new name already exists
            // Not critical, continue
            println!("{}", "Note: Could not rename origin remote".yellow());
        }
    }

    Ok(())
}

/// Handle a git conflict by prompting user and executing their choice
fn handle_git_conflict(
    haxelib_status: &HaxelibStatus,
    separator: &str,
    counter: Option<(usize, usize)>,
) -> Result<()> {
    let haxelib = haxelib_status.lib;
    let repo_path = haxelib.git_repo_path();

    // Prompt user for resolution strategy
    let choice = prompt_conflict_resolution(haxelib, haxelib_status)?;

    match choice {
        ConflictResolution::Stash => {
            git_stash_push(&repo_path, haxelib)?;
            install_or_update_git_cli(haxelib, separator, counter)?;
            git_stash_pop(&repo_path, haxelib)?;
        }
        ConflictResolution::Discard => {
            git_discard_changes(&repo_path, haxelib)?;
            install_or_update_git_cli(haxelib, separator, counter)?;
        }
        ConflictResolution::Commit => {
            git_commit_changes(&repo_path, haxelib)?;
            install_or_update_git_cli(haxelib, separator, counter)?;
        }
        ConflictResolution::Skip => {
            println!("Skipping {}", haxelib.name.yellow());
        }
    }

    Ok(())
}

/// Stash changes in the git repository
fn git_stash_push(repo_path: &Path, haxelib: &Haxelib) -> Result<()> {
    println!("Stashing changes in {}...", haxelib.name);

    let stash_message = format!(
        "hmm-rs: auto-stash before updating to {}",
        haxelib.try_vcs_ref().unwrap_or("latest")
    );

    let result = std::process::Command::new("git")
        .args([
            "-C",
            path_to_str(repo_path)?,
            "stash",
            "push",
            "-m",
            &stash_message,
        ])
        .output()
        .context("Failed to execute git stash")?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(anyhow!("Failed to stash changes: {}", stderr));
    }

    println!("✓ Changes stashed");
    Ok(())
}

/// Restore stashed changes
fn git_stash_pop(repo_path: &Path, haxelib: &Haxelib) -> Result<()> {
    println!("Restoring stashed changes in {}...", haxelib.name);

    let result = std::process::Command::new("git")
        .args(["-C", path_to_str(repo_path)?, "stash", "pop"])
        .output()
        .context("Failed to execute git stash pop")?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);

        if stderr.contains("CONFLICT") {
            println!();
            println!(
                "{}",
                "⚠ Warning: Stash pop created merge conflicts"
                    .yellow()
                    .bold()
            );
            println!("You'll need to resolve them manually in:");
            println!("  {}", repo_path.display().to_string().cyan());
            println!();
            println!("After resolving, run:");
            println!(
                "  {}",
                format!("git -C {} stash drop", repo_path.display())
                    .cyan()
                    .bold()
            );
            println!();
            // Don't return error - let user continue with other libs
            return Ok(());
        }

        return Err(anyhow!("Failed to restore stash: {}", stderr));
    }

    println!("✓ Changes restored");
    Ok(())
}

/// Discard all local changes
fn git_discard_changes(repo_path: &Path, haxelib: &Haxelib) -> Result<()> {
    println!("Discarding changes in {}...", haxelib.name);

    // Reset tracked files
    let reset_result = std::process::Command::new("git")
        .args(["-C", path_to_str(repo_path)?, "reset", "--hard", "HEAD"])
        .status()
        .context("Failed to execute git reset")?;

    if !reset_result.success() {
        return Err(anyhow!("Failed to reset changes in {}", haxelib.name));
    }

    // Clean untracked files
    let clean_result = std::process::Command::new("git")
        .args(["-C", path_to_str(repo_path)?, "clean", "-fd"])
        .status()
        .context("Failed to execute git clean")?;

    if !clean_result.success() {
        return Err(anyhow!(
            "Failed to clean untracked files in {}",
            haxelib.name
        ));
    }

    println!("✓ Changes discarded");
    Ok(())
}

/// Prompt for commit message and commit changes
fn git_commit_changes(repo_path: &Path, haxelib: &Haxelib) -> Result<()> {
    println!();
    print!("Enter commit message: ");
    stdout().flush()?;

    let mut message = String::new();
    stdin().read_line(&mut message)?;
    let message = message.trim();

    if message.is_empty() {
        return Err(anyhow!("Commit message cannot be empty"));
    }

    println!("Committing changes in {}...", haxelib.name);

    // Stage all changes
    let add_result = std::process::Command::new("git")
        .args(["-C", path_to_str(repo_path)?, "add", "-A"])
        .status()
        .context("Failed to execute git add")?;

    if !add_result.success() {
        return Err(anyhow!("Failed to stage changes in {}", haxelib.name));
    }

    // Commit
    let commit_result = std::process::Command::new("git")
        .args(["-C", path_to_str(repo_path)?, "commit", "-m", message])
        .output()
        .context("Failed to execute git commit")?;

    if !commit_result.status.success() {
        let stderr = String::from_utf8_lossy(&commit_result.stderr);
        if stderr.contains("nothing to commit") {
            println!(
                "{}",
                "Note: Nothing to commit (changes may have been staged already)".yellow()
            );
            return Ok(());
        }
        return Err(anyhow!("Failed to commit changes: {}", stderr));
    }

    println!("✓ Changes committed");
    Ok(())
}

/// Get a summary of changed files in the git repository
fn get_git_diff_stat(repo_path: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["-C", path_to_str(repo_path)?, "diff", "--stat"])
        .output()
        .context("Failed to get git diff stat")?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Ok(String::from("(unable to get diff)"))
    }
}

/// Prompt user for how to resolve a git conflict
fn prompt_conflict_resolution(
    haxelib: &Haxelib,
    status: &HaxelibStatus,
) -> Result<ConflictResolution> {
    let repo_path = haxelib.git_repo_path();

    // Get diff stat to show what changed
    let diff_stat = get_git_diff_stat(&repo_path)?;

    println!();
    println!(
        "{}",
        "┌─────────────────────────────────────────────────────".bright_black()
    );
    println!(
        "{} {} {}",
        "│".bright_black(),
        haxelib.name.yellow().bold(),
        "has uncommitted changes".yellow()
    );
    println!(
        "{}",
        "├─────────────────────────────────────────────────────".bright_black()
    );
    println!(
        "{} Current:  {}",
        "│".bright_black(),
        status.installed.as_deref().unwrap_or("unknown").red()
    );
    println!(
        "{} Expected: {}",
        "│".bright_black(),
        status.wants.as_deref().unwrap_or("unknown").green()
    );

    if !diff_stat.trim().is_empty() {
        println!(
            "{}",
            "├─────────────────────────────────────────────────────".bright_black()
        );
        println!("{} Changed files:", "│".bright_black());
        for line in diff_stat.lines() {
            if !line.trim().is_empty() {
                println!("{}  {}", "│".bright_black(), line.bright_black());
            }
        }
    }

    println!(
        "{}",
        "├─────────────────────────────────────────────────────".bright_black()
    );
    println!("{} What would you like to do?", "│".bright_black());
    println!("{}", "│".bright_black());
    println!(
        "{}  {} {} - Save changes temporarily, update, restore",
        "│".bright_black(),
        "[s]".cyan().bold(),
        "Stash".cyan()
    );
    println!(
        "{}  {} {} - Discard all local changes and update",
        "│".bright_black(),
        "[d]".red().bold(),
        "Discard".red()
    );
    println!(
        "{}  {} {} - Commit changes first, then update",
        "│".bright_black(),
        "[c]".green().bold(),
        "Commit".green()
    );
    println!(
        "{}  {} {} - Skip this library for now",
        "│".bright_black(),
        "[k]".yellow().bold(),
        "Skip".yellow()
    );
    println!(
        "{}",
        "└─────────────────────────────────────────────────────".bright_black()
    );

    print!("Choice (s/d/c/k): ");
    stdout().flush()?;

    let mut input = String::new();
    stdin().read_line(&mut input)?;

    match input.trim().to_lowercase().as_str() {
        "s" | "stash" => Ok(ConflictResolution::Stash),
        "d" | "discard" => Ok(ConflictResolution::Discard),
        "c" | "commit" => Ok(ConflictResolution::Commit),
        "k" | "skip" => Ok(ConflictResolution::Skip),
        _ => {
            println!("Invalid choice. Skipping {}.", haxelib.name);
            Ok(ConflictResolution::Skip)
        }
    }
}

pub fn create_current_file(path: &Path, content: &String) -> Result<()> {
    std::fs::create_dir_all(path)?;
    let mut current_version_file = File::create(path.join(".current"))?;
    write!(current_version_file, "{}", content)?;
    Ok(())
}

/// If the git dependency specifies a subdirectory (`dir`), set up a dev link so that
/// `-lib <name>` resolves to `.haxelib/<name>/git/<dir>/`. This mirrors real haxelib,
/// which sets a dev path to `<versionPath>/<subDir>` for subdirectory git installs.
pub fn ensure_git_subdir_dev_link(haxelib: &Haxelib) -> Result<()> {
    let subdir = match haxelib.dir.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        Some(d) => d,
        None => {
            // No subdir now, but one may have been set before (or this lib
            // was a dev dep): a stale `.dev` would keep `haxelib path` away
            // from git/.
            if crate::commands::dev_command::remove_dev_file(&haxelib.name)? {
                println!(
                    "{}: development directory unset",
                    haxelib.name.green().bold()
                );
            }
            return Ok(());
        }
    };

    let abs_git = std::fs::canonicalize(haxelib.git_repo_path())
        .with_context(|| format!("Failed to resolve git repo path for {}", haxelib.name))?;
    let abs_subdir = abs_git.join(subdir);

    if !abs_subdir.exists() {
        println!(
            "{}: subdirectory '{}' was not found in the repo; the dev path may be invalid",
            haxelib.name.yellow(),
            subdir
        );
    }

    crate::commands::dev_command::write_dev_file(&haxelib.name, &abs_subdir)?;
    println!(
        "{}: development directory set to {}",
        haxelib.name.green().bold(),
        abs_subdir.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- sanitize_zip_entry ---

    fn dest() -> PathBuf {
        PathBuf::from(".haxelib/flixel/5,0,0")
    }

    #[test]
    fn sanitize_zip_entry_strips_base_path() {
        assert_eq!(
            sanitize_zip_entry("release/", "release/src/Main.hx", &dest()),
            Some(dest().join("src").join("Main.hx"))
        );
    }

    #[test]
    fn sanitize_zip_entry_handles_empty_base_path() {
        assert_eq!(
            sanitize_zip_entry("", "haxelib.json", &dest()),
            Some(dest().join("haxelib.json"))
        );
    }

    #[test]
    fn sanitize_zip_entry_normalizes_backslashes() {
        assert_eq!(
            sanitize_zip_entry("", "src\\Main.hx", &dest()),
            Some(dest().join("src").join("Main.hx"))
        );
    }

    #[test]
    fn sanitize_zip_entry_skips_outside_base_and_empty() {
        assert_eq!(sanitize_zip_entry("release/", "other/x", &dest()), None);
        assert_eq!(sanitize_zip_entry("release/", "release/", &dest()), None);
        assert_eq!(sanitize_zip_entry("", "", &dest()), None);
    }

    /// Regression: the old guard only rejected the substring "..", so an
    /// absolute entry name slipped through and `join` discarded the base dir.
    #[test]
    fn sanitize_zip_entry_rejects_escaping_entries() {
        for entry in ["/etc/passwd", "../../evil", "a/../../evil", "/", "//x"] {
            assert_eq!(
                sanitize_zip_entry("", entry, &dest()),
                None,
                "{entry:?} should be rejected"
            );
        }
    }

    /// The invariant that actually matters: whatever comes back is inside
    /// `dest`. (`C:\evil` is a Windows drive prefix there, but merely an odd
    /// directory name on unix, so assert containment rather than rejection.)
    #[test]
    fn sanitize_zip_entry_output_always_inside_dest() {
        let dest = dest();
        for entry in [
            "/etc/passwd",
            "../../evil",
            "C:\\evil",
            "a/./b",
            "release/../../x",
            "ok.hx",
        ] {
            if let Some(out) = sanitize_zip_entry("", entry, &dest) {
                assert!(out.starts_with(&dest), "{entry:?} escaped to {out:?}");
            }
        }
    }

    /// The old substring check also rejected perfectly legal file names.
    #[test]
    fn sanitize_zip_entry_allows_dots_inside_names() {
        assert_eq!(
            sanitize_zip_entry("", "foo..bar.txt", &dest()),
            Some(dest().join("foo..bar.txt"))
        );
    }

    // --- parse_git_progress / run_git_with_progress ---

    #[test]
    fn git_progress_parses_percent_form_with_throughput() {
        let p = parse_git_progress("Receiving objects:  45% (1234/2742), 1.20 MiB | 3.40 MiB/s")
            .unwrap();
        assert_eq!(
            p,
            GitProgress {
                title: "Receiving objects".into(),
                current: 1234,
                total: Some(2742),
                detail: ", 1.20 MiB | 3.40 MiB/s".into(),
                done: false,
            }
        );
    }

    #[test]
    fn git_progress_strips_sideband_prefix_and_padding() {
        let p = parse_git_progress("remote: Compressing objects: 100% (500/500), done.        ")
            .unwrap();
        assert_eq!(p.title, "Compressing objects");
        assert_eq!((p.current, p.total), (500, Some(500)));
        assert_eq!(p.detail, "");
        assert!(p.done);
    }

    #[test]
    fn git_progress_parses_count_only_form() {
        let p = parse_git_progress("remote: Enumerating objects: 1234, done.").unwrap();
        assert_eq!(p.title, "Enumerating objects");
        assert_eq!((p.current, p.total), (1234, None));
        assert!(p.done);
    }

    #[test]
    fn git_progress_accepts_every_phase_title() {
        for title in [
            "Counting objects",
            "Compressing objects",
            "Receiving objects",
            "Resolving deltas",
            "Updating files",
        ] {
            let p = parse_git_progress(&format!("{title}:   0% (0/10)")).unwrap();
            assert_eq!(p.title, title);
            assert!(!p.done);
        }
    }

    #[test]
    fn git_progress_rejects_non_progress_lines() {
        for line in [
            "",
            "Cloning into '/tmp/x'...",
            "fatal: bad object deadbeef",
            "HEAD is now at 1a2b3c fix: 12% (1/2)",
            "Submodule path 'lib': checked out 'abc'",
            "remote: Total 12 (delta 0), reused 0 (delta 0)",
            "remote: 5",
            "Receiving objects: abc",
            "Receiving objects: 45% (1234/)",
            "Receiving objects: 45 objects",
            "warning: filtering not recognized by server, ignoring",
        ] {
            assert!(parse_git_progress(line).is_none(), "{line:?}");
        }
    }

    #[test]
    fn git_stderr_sink_drives_bar_and_keeps_other_lines() {
        let pb = ProgressBar::hidden();
        let mut sink = GitStderrSink::new(&pb);
        sink.push(b"Cloning into '/tmp/x'...");
        sink.push(b"remote: Enumerating objects: 30, done.        ");
        assert_eq!(pb.message(), "Enumerating objects: 30");
        sink.push(b"Receiving objects:  50% (10/20), 1.00 MiB | 2.00 MiB/s");
        assert_eq!(pb.position(), 10);
        assert_eq!(pb.length(), Some(20));
        assert_eq!(pb.message(), "Receiving objects (1.00 MiB | 2.00 MiB/s)");
        // Narrow-terminal split form: the title alone, then indented counters.
        sink.push(b"Resolving deltas:");
        sink.push(b"  75% (3/4)");
        assert_eq!(pb.position(), 3);
        assert_eq!(pb.length(), Some(4));
        assert_eq!(pb.message(), "Resolving deltas");
        sink.push(b"fatal: early EOF");
        assert_eq!(sink.lines, vec!["Cloning into '/tmp/x'...", "fatal: early EOF"]);
    }

    #[test]
    fn run_git_with_progress_collects_stderr_on_failure() {
        let missing = tempfile::TempDir::new().unwrap().path().join("nope");
        let pb = ProgressBar::hidden();
        let run =
            run_git_with_progress(&["-C", missing.to_str().unwrap(), "status"], &pb).unwrap();
        assert!(!run.status.success());
        assert!(
            run.stderr.iter().any(|l| l.starts_with("fatal:")),
            "{:?}",
            run.stderr
        );
        let err = run.error("boom").to_string();
        assert!(err.starts_with("boom\n"), "{err:?}");
        assert!(err.contains("fatal:"), "{err:?}");
    }

    #[test]
    fn run_git_with_progress_success_is_quiet() {
        let pb = ProgressBar::hidden();
        let run = run_git_with_progress(&["--version"], &pb).unwrap();
        assert!(run.status.success());
        assert!(run.stderr.is_empty(), "{:?}", run.stderr);
        assert_eq!(run.error("boom").to_string(), "boom");
    }

    #[test]
    fn test_parse_remote_https_with_git_suffix() {
        let result = parse_remote_name_from_url("https://github.com/haxeflixel/flixel.git", ".");
        assert_eq!(result.unwrap(), "haxeflixel.flixel");
    }

    #[test]
    fn test_parse_remote_https_without_git_suffix() {
        let result = parse_remote_name_from_url("https://github.com/haxeflixel/flixel", ".");
        assert_eq!(result.unwrap(), "haxeflixel.flixel");
    }

    #[test]
    fn test_parse_remote_ssh_colon_format() {
        let result = parse_remote_name_from_url("git@github.com:user/repo.git", ".");
        assert_eq!(result.unwrap(), "user.repo");
    }

    #[test]
    fn test_parse_remote_ssh_protocol() {
        let result = parse_remote_name_from_url("ssh://git@github.com/user/repo.git", ".");
        assert_eq!(result.unwrap(), "user.repo");
    }

    #[test]
    fn test_parse_remote_http() {
        let result = parse_remote_name_from_url("http://github.com/FunkinCrew/funkVis", ".");
        assert_eq!(result.unwrap(), "FunkinCrew.funkVis");
    }

    #[test]
    fn test_parse_remote_invalid_url() {
        let result = parse_remote_name_from_url("not-a-url", ".");
        assert!(result.is_err());
    }

    proptest::proptest! {
        #[test]
        fn parse_remote_name_never_panics(url in ".{0,60}", sep in "[-._]") {
            let _ = parse_remote_name_from_url(&url, &sep);
        }
    }

    #[test]
    fn test_parse_remote_with_dash_separator() {
        let result = parse_remote_name_from_url("https://github.com/haxeflixel/flixel.git", "-");
        assert_eq!(result.unwrap(), "haxeflixel-flixel");
    }

    #[test]
    fn test_parse_remote_with_double_underscore_separator() {
        let result = parse_remote_name_from_url("git@github.com:user/repo.git", "__");
        assert_eq!(result.unwrap(), "user__repo");
    }

    #[test]
    fn test_resolve_separator_flag_wins_when_non_empty() {
        // Non-empty flag short-circuits before any env read, so this is safe to run in parallel.
        assert_eq!(resolve_remote_separator(Some("-")), "-");
        assert_eq!(resolve_remote_separator(Some("__")), "__");
    }

    #[test]
    fn test_resolve_separator_env_and_default() {
        // This test mutates HMM_REMOTE_SEPARATOR. Keep all env-var-reading
        // resolver assertions inside this single test so they don't race with
        // each other under cargo's parallel runner.
        let prior = env::var("HMM_REMOTE_SEPARATOR").ok();

        env::remove_var("HMM_REMOTE_SEPARATOR");
        assert_eq!(resolve_remote_separator(None), DEFAULT_REMOTE_SEPARATOR);
        assert_eq!(resolve_remote_separator(Some("")), DEFAULT_REMOTE_SEPARATOR);

        env::set_var("HMM_REMOTE_SEPARATOR", "_");
        assert_eq!(resolve_remote_separator(None), "_");
        assert_eq!(resolve_remote_separator(Some("")), "_");
        // Non-empty flag still wins over env var.
        assert_eq!(resolve_remote_separator(Some("-")), "-");

        env::set_var("HMM_REMOTE_SEPARATOR", "");
        assert_eq!(resolve_remote_separator(None), DEFAULT_REMOTE_SEPARATOR);

        match prior {
            Some(v) => env::set_var("HMM_REMOTE_SEPARATOR", v),
            None => env::remove_var("HMM_REMOTE_SEPARATOR"),
        }
    }

    #[test]
    fn test_is_partial_clone_false_for_regular_repo() {
        let temp = tempfile::TempDir::new().unwrap();
        std::process::Command::new("git")
            .args(["init", temp.path().to_str().unwrap()])
            .output()
            .unwrap();
        assert!(!is_partial_clone(temp.path()));
    }

    #[test]
    fn test_is_partial_clone_true_when_filter_configured() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().to_str().unwrap();
        std::process::Command::new("git")
            .args(["init", path])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", path, "remote", "add", "origin", "https://example.com/repo"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", path, "config", "remote.origin.partialclonefilter", "blob:none"])
            .output()
            .unwrap();
        assert!(is_partial_clone(temp.path()));
    }

    #[test]
    fn test_configure_remote_as_promisor_sets_config() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().to_str().unwrap();
        std::process::Command::new("git")
            .args(["init", path])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", path, "remote", "add", "test-remote", "https://example.com/repo"])
            .output()
            .unwrap();

        configure_remote_as_promisor(temp.path(), "test-remote").unwrap();

        let promisor = std::process::Command::new("git")
            .args(["-C", path, "config", "remote.test-remote.promisor"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&promisor.stdout).trim(), "true");

        let filter = std::process::Command::new("git")
            .args(["-C", path, "config", "remote.test-remote.partialclonefilter"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&filter.stdout).trim(), "blob:none");
    }

    #[test]
    fn test_configure_remote_as_promisor_is_idempotent() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().to_str().unwrap();
        std::process::Command::new("git")
            .args(["init", path])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", path, "remote", "add", "test-remote", "https://example.com/repo"])
            .output()
            .unwrap();

        configure_remote_as_promisor(temp.path(), "test-remote").unwrap();
        configure_remote_as_promisor(temp.path(), "test-remote").unwrap();

        let promisor = std::process::Command::new("git")
            .args(["-C", path, "config", "remote.test-remote.promisor"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&promisor.stdout).trim(), "true");
    }
}
