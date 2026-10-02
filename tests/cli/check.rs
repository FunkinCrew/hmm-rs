use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::common;

#[test]
fn check_all_haxelibs_correct() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": "1.0.0"},
            {"name": "lib-b", "type": "haxelib", "version": "2.0.0"}
        ]
    }"#;
    let temp =
        common::project_with_installed_haxelibs(json, &[("lib-a", "1.0.0"), ("lib-b", "2.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "dependencie(s) are installed at the correct versions",
        ));
}

#[test]
fn check_detects_missing_haxelib() {
    let json = r#"{
        "dependencies": [
            {"name": "missing-lib", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("is not installed"))
        .stderr(predicate::str::contains(
            "not installed or have the wrong version: missing-lib",
        ));
}

#[test]
fn check_detects_wrong_version() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": "2.0.0"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("lib-a", "1.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("is not at the correct version"));
}

/// An unpinned lib is satisfied by whatever version is installed, so check
/// only hints at locking it and still exits 0.
#[test]
fn check_detects_unlocked_version() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": null}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("lib-a", "1.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("is not locked"));
}

#[test]
fn check_alias_ch_works() {
    let temp = common::project_with_empty_hmm_json();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("ch")
        .assert()
        .success();
}

#[test]
fn check_filtered_only_processes_named_libs() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": "1.0.0"},
            {"name": "lib-b", "type": "haxelib", "version": "2.0.0"},
            {"name": "lib-c", "type": "haxelib", "version": "3.0.0"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("lib-a", "1.0.0")]);

    // bold ANSI codes wrap each digit; check only structural pieces around it
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", "--verbose", "lib-a"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dependencie(s) are installed"))
        .stdout(predicate::str::contains("Checking lib-b").not())
        .stdout(predicate::str::contains("Checking lib-c").not());
}

/// Libs are checked on worker threads but reported in hmm.json order.
#[test]
fn check_reports_libs_in_hmm_json_order() {
    let names: Vec<String> = (0..24).map(|i| format!("order-{i:02}")).collect();
    let deps: Vec<String> = names
        .iter()
        .map(|n| format!(r#"{{"name": "{n}", "type": "haxelib", "version": "1.0.0"}}"#))
        .collect();
    let json = format!(r#"{{"dependencies": [{}]}}"#, deps.join(","));
    // every other lib installed; quiet mode then names only the missing ones
    let installed: Vec<(&str, &str)> = names
        .iter()
        .step_by(2)
        .map(|n| (n.as_str(), "1.0.0"))
        .collect();
    let temp = common::project_with_installed_haxelibs(&json, &installed);

    let output = Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    let positions: Vec<usize> = names
        .iter()
        .skip(1)
        .step_by(2)
        .map(|n| stdout.find(n.as_str()).expect(n))
        .collect();
    assert!(positions.is_sorted(), "out of order:\n{stdout}");
}

#[test]
fn check_default_hides_correct_libs() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("lib-a", "1.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "dependencie(s) are installed at the correct versions",
        ))
        .stdout(predicate::str::contains("lib-a").not())
        .stdout(predicate::str::contains("are out of date or have changes").not());
}

#[test]
fn check_default_shows_problems_and_count() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": "1.0.0"},
            {"name": "lib-b", "type": "haxelib", "version": "2.0.0"}
        ]
    }"#;
    let temp =
        common::project_with_installed_haxelibs(json, &[("lib-a", "1.0.0"), ("lib-b", "1.0.0")]);

    // bold ANSI codes wrap the count, so match the text after it
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("is not at the correct version"))
        .stdout(predicate::str::contains("lib-a").not())
        .stdout(predicate::str::contains(
            "dependencie(s) are out of date or have changes",
        ));
}

#[test]
fn check_verbose_shows_all_libs() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("lib-a", "1.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", "--verbose"])
        .assert()
        .success()
        .stdout(predicate::str::contains("lib-a"));

    // -v short form, before the subcommand (global flag)
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["-v", "check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("lib-a"));
}

// --- git-branch statuses (hermetic: local repos over file://) ---

/// Builds a project with a git dep cloned from a local two-commit repo.
/// Returns (host_repo_tempdir, project_tempdir, first_commit_sha).
fn installed_git_project() -> (assert_fs::TempDir, assert_fs::TempDir, String) {
    let (repo_temp, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let first_sha = common::git_rev_parse(&repo_path, "HEAD");
    std::fs::write(repo_path.join("second.txt"), "second\n").unwrap();
    common::run_git(&repo_path, &["add", "-A"]);
    common::run_git(&repo_path, &["commit", "-qm", "second"]);

    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "gitlib", "type": "git", "ref": "main", "url": "{}"}}
        ]
    }}"#,
        common::file_url(&repo_path)
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success();

    (repo_temp, temp, first_sha)
}

#[test]
fn check_git_correct_commit_passes() {
    let (_repo, temp, _first_sha) = installed_git_project();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "dependencie(s) are installed at the correct versions",
        ))
        .stdout(predicate::str::contains("has issues").not())
        .stdout(predicate::str::contains("is not at the correct version").not());
}

#[test]
fn check_git_detects_wrong_commit() {
    let (_repo, temp, first_sha) = installed_git_project();
    let clone = temp.path().join(".haxelib/gitlib/git");
    common::run_git(&clone, &["checkout", "-q", &first_sha]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("is not at the correct version"))
        .stdout(predicate::str::contains("(wrong commit)"));
}

#[test]
fn check_git_detects_local_changes_conflict() {
    let (_repo, temp, _first_sha) = installed_git_project();
    std::fs::write(temp.path().join(".haxelib/gitlib/git/README.md"), "dirty\n").unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("has issues"))
        .stdout(predicate::str::contains("(local changes)"));
}

#[test]
fn check_git_detects_wrong_commit_plus_local_changes_conflict() {
    let (_repo, temp, first_sha) = installed_git_project();
    let clone = temp.path().join(".haxelib/gitlib/git");
    common::run_git(&clone, &["checkout", "-q", &first_sha]);
    std::fs::write(clone.join("README.md"), "dirty\n").unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("has issues"))
        .stdout(predicate::str::contains("(wrong commit + local changes)"));
}

#[test]
fn check_git_detects_missing_clone() {
    // Lib dir with a `.current` saying "git" but no git/ checkout underneath.
    let json = r#"{
        "dependencies": [
            {"name": "gitlib", "type": "git", "ref": "main", "url": "https://example.com/repo.git"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("gitlib", "git")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "is not cloned / installed (via git)",
        ));
}

/// Regression: the git/ checkout is at the right commit, but `.current`
/// still names a haxelib version (left behind by a haxelib install of the
/// same lib before it was switched back to git). `haxelib path` follows
/// `.current`, so this must not pass as installed.
#[test]
fn check_git_detects_stale_current() {
    let (_repo, temp, _first_sha) = installed_git_project();
    std::fs::write(temp.path().join(".haxelib/gitlib/.current"), "1.0.0").unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("is not at the correct version"))
        .stdout(predicate::str::contains("1.0.0"));
}

/// Regression: a git dep with no `dir` but a leftover `.dev` (from an earlier
/// subdir install or a dev dep) must not pass as installed, since
/// `haxelib path` prefers `.dev` over the git/ checkout.
#[test]
fn check_git_detects_stale_dev_link() {
    let (_repo, temp, _first_sha) = installed_git_project();
    std::fs::write(temp.path().join(".haxelib/gitlib/.dev"), "/somewhere/else").unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            ".dev target is not a git checkout",
        ));
}

/// Rewrites the project's hmm.json: `gitlib`'s ref becomes `new_ref` (removed
/// when `None`) and a missing haxelib dep `lib-after` is appended after it.
fn set_gitlib_ref_with_sibling(temp: &assert_fs::TempDir, new_ref: Option<&str>) {
    let path = temp.path().join("hmm.json");
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let deps = json["dependencies"].as_array_mut().unwrap();
    match new_ref {
        Some(r) => deps[0]["ref"] = r.into(),
        None => {
            deps[0].as_object_mut().unwrap().remove("ref");
        }
    }
    deps.push(serde_json::json!({"name": "lib-after", "type": "haxelib", "version": "1.0.0"}));
    std::fs::write(&path, json.to_string()).unwrap();
}

/// Regression (H8): a ref the clone doesn't have (a tag or branch created
/// upstream after the clone) aborted the whole run with a bare gix error.
/// It is that lib's "wrong version", and the deps after it are still checked.
#[test]
fn check_git_ref_missing_locally_fails_that_lib_only() {
    let (_repo, temp, _first_sha) = installed_git_project();
    set_gitlib_ref_with_sibling(&temp, Some("v2"));

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("is not at the correct version"))
        .stdout(predicate::str::contains("(ref not found locally)"))
        .stdout(predicate::str::contains("lib-after"))
        .stderr(predicate::str::contains("gitlib, lib-after"));
}

/// Regression (H8): `ref: HEAD` panicked inside gix (symbolic ref).
#[test]
fn check_git_ref_head_passes() {
    let (_repo, temp, _first_sha) = installed_git_project();
    let path = temp.path().join("hmm.json");
    let json = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, json.replace(r#""ref": "main""#, r#""ref": "HEAD""#)).unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .success();
}

/// Refs resolve like `git rev-parse`: revision expressions and any unambiguous
/// short SHA pass, and a SHA is compared in full, not over gix's abbreviation.
#[test]
fn check_git_ref_resolves_like_rev_parse() {
    let (_repo, temp, first_sha) = installed_git_project();
    let clone = temp.path().join(".haxelib/gitlib/git");
    common::run_git(&clone, &["checkout", "-q", &first_sha]);
    let path = temp.path().join("hmm.json");
    let json = std::fs::read_to_string(&path).unwrap();
    let check_with_ref = |r: &str| {
        std::fs::write(
            &path,
            json.replace(r#""ref": "main""#, &format!(r#""ref": "{r}""#)),
        )
        .unwrap();
        let mut cmd = Command::cargo_bin("hmm-rs").unwrap();
        cmd.current_dir(temp.path()).arg("check");
        cmd
    };

    for good in ["main~1", &first_sha[..5], &first_sha] {
        check_with_ref(good).assert().success();
    }

    // Right first 39 digits, wrong last one: used to pass.
    let last = if first_sha.ends_with('0') { "1" } else { "0" };
    let wrong_tail = format!("{}{last}", &first_sha[..39]);
    check_with_ref(&wrong_tail).assert().failure();
}

/// Regression: an annotated tag's ref named the tag object, never HEAD's
/// commit, so a checkout exactly at the tag was reported as the wrong commit.
#[test]
fn check_git_annotated_tag_passes() {
    let (_repo, temp, _first_sha) = installed_git_project();
    let clone = temp.path().join(".haxelib/gitlib/git");
    common::run_git(&clone, &["tag", "-a", "v1", "-m", "v1"]);
    let path = temp.path().join("hmm.json");
    let json = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, json.replace(r#""ref": "main""#, r#""ref": "v1""#)).unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .success();
}

/// A lib that can't be checked at all (here a cloned git dep with no `ref`)
/// is reported under its own name and fails the run without skipping the
/// deps after it.
#[test]
fn check_uncheckable_dep_fails_that_lib_only() {
    let (_repo, temp, _first_sha) = installed_git_project();
    set_gitlib_ref_with_sibling(&temp, None);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("could not be checked"))
        .stdout(predicate::str::contains("'ref' field is required"))
        .stdout(predicate::str::contains("lib-after"))
        .stderr(predicate::str::contains("gitlib, lib-after"));
}

#[test]
fn check_unknown_lib_warns() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("lib-a", "1.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", "nonexistent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not found in hmm.json"));
}

// --- dev deps ---

/// A project with a dev dep on `./devsrc` (which exists) and `.haxelib/devlib/`
/// present but empty. Returns the project and the canonical `devsrc` path.
fn dev_project() -> (assert_fs::TempDir, std::path::PathBuf) {
    let json = r#"{
        "dependencies": [
            {"name": "devlib", "type": "dev", "path": "devsrc"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);
    temp.child("devsrc").create_dir_all().unwrap();
    temp.child(".haxelib/devlib").create_dir_all().unwrap();
    let target = temp.child("devsrc").path().canonicalize().unwrap();
    (temp, target)
}

#[test]
fn check_dev_dep_pointing_at_path_passes() {
    let (temp, target) = dev_project();
    // haxelib 4.2.0 writes `.dev` with a trailing slash; that is the same path.
    let dev = format!("{}/", target.display());
    temp.child(".haxelib/devlib/.dev").write_str(&dev).unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("is not").not());
}

/// Regression: a `.current` left by an earlier haxelib install of the same lib
/// counted as an installed dev dep, although `haxelib path` resolves the
/// version dir instead of the dev path.
#[test]
fn check_dev_dep_with_only_current_is_missing() {
    let (temp, _target) = dev_project();
    temp.child(".haxelib/devlib/.current")
        .write_str("1.0.0")
        .unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("is not installed"))
        .stdout(predicate::str::contains("devsrc"));
}

#[test]
fn check_dev_dep_pointing_elsewhere_is_outdated() {
    let (temp, _target) = dev_project();
    temp.child(".haxelib/devlib/.dev")
        .write_str("/somewhere/else")
        .unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("is not at the correct version"))
        .stdout(predicate::str::contains("/somewhere/else"));
}

#[test]
fn check_dev_dep_with_missing_target_fails() {
    let (temp, target) = dev_project();
    temp.child(".haxelib/devlib/.dev")
        .write_str(target.to_str().unwrap())
        .unwrap();
    std::fs::remove_dir(&target).unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("is not installed"));
}

// --- dotted library names (`funkin.vis`-style) ---

#[test]
fn check_dotted_haxelib_installed_ok() {
    let json = r#"{
        "dependencies": [
            {"name": "chkdot.vis", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("chkdot.vis", "1.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "dependencie(s) are installed at the correct versions",
        ));
}

#[test]
fn check_dotted_git_installed_ok() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "chkgitdot.vis", "type": "git", "ref": "main", "url": "{url}"}}
        ]
    }}"#
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "dependencie(s) are installed at the correct versions",
        ));
}

/// Regression: haxelib always trims `.current` on read. A trailing newline
/// written by another tool must not read as a different version.
#[test]
fn check_current_with_trailing_newline_is_not_outdated() {
    let json = r#"{
        "dependencies": [
            {"name": "trimchk-a", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("trimchk-a", "1.0.0")]);
    std::fs::write(temp.child(".haxelib/trimchk-a/.current").path(), "1.0.0\n").unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "dependencie(s) are installed at the correct versions",
        ));
}

/// Comma names are rejected at the hmm.json read choke point: `a,b` would
/// alias `a.b` on disk (both map to `.haxelib/a,b`).
#[test]
fn check_rejects_comma_name_in_hmm_json() {
    let json = r#"{
        "dependencies": [
            {"name": "a,b", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .stderr(predicate::str::contains("is not allowed"));
}

#[test]
fn check_finds_mixed_case_lib_in_lowercase_dir() {
    // The layout haxelib 4.2.0 and hmm-rs write: `.haxelib/chkmix/` plus
    // `.name`, with no exact-case entry.
    let json = r#"{
        "dependencies": [
            {"name": "ChkMix", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("ChkMix", "1.0.0")]);
    temp.child(".haxelib/chkmix/.name")
        .write_str("ChkMix")
        .unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .success();
}

// --- `.dev` redirects (a git worktree sharing another checkout's libs) ---

/// A project with `json` as its hmm.json and only a `.dev` marker for `name`
/// pointing at `target`, like a git worktree reusing the main checkout's libs.
fn worktree_with_dev_redirect(
    json: &str,
    name: &str,
    target: &std::path::Path,
) -> assert_fs::TempDir {
    let temp = common::project_with_hmm_json(json);
    temp.child(format!(".haxelib/{name}/.dev"))
        .write_str(target.to_str().unwrap())
        .unwrap();
    temp
}

/// Regression: a haxelib dep with a `.dev` marker always read as the wrong
/// version, since the marker's path was compared to the hmm.json version.
/// `haxelib path` reports the target's haxelib.json version, and resolves a
/// relative marker against the cwd.
#[test]
fn check_haxelib_dev_redirect_uses_target_version() {
    let json = r#"{
        "dependencies": [
            {"name": "redir-a", "type": "haxelib", "version": "1.3.0"}
        ]
    }"#;
    let main = common::project_with_installed_haxelibs(json, &[("redir-a", "1.3.0")]);
    main.child(".haxelib/redir-a/1,3,0/haxelib.json")
        .write_str(r#"{"name": "redir-a", "version": "1.3.0"}"#)
        .unwrap();
    let relative = std::path::Path::new("..")
        .join(main.path().file_name().unwrap())
        .join(".haxelib/redir-a/1,3,0");
    let worktree = worktree_with_dev_redirect(json, "redir-a", &relative);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(worktree.path())
        .arg("check")
        .assert()
        .success();

    worktree
        .child("hmm.json")
        .write_str(&json.replace("1.3.0", "1.4.0"))
        .unwrap();
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(worktree.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("is not at the correct version"))
        .stdout(predicate::str::contains("1.3.0 (via .dev"));
}

/// A git dep's `.dev` can point at another project's clone instead of a clone
/// of its own. It passes at the pinned commit, local changes there included:
/// install never touches a checkout it doesn't own.
#[test]
fn check_git_dev_redirect_to_other_checkout() {
    let (_repo, main, first_sha) = installed_git_project();
    let main_git = main.path().join(".haxelib/gitlib/git");
    let json = std::fs::read_to_string(main.path().join("hmm.json")).unwrap();
    let worktree = worktree_with_dev_redirect(&json, "gitlib", &main_git);
    std::fs::write(main_git.join("README.md"), "local edit\n").unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(worktree.path())
        .arg("check")
        .assert()
        .success();

    common::run_git(&main_git, &["checkout", "-qf", &first_sha]);
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(worktree.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains("wrong commit, via .dev"));
}

/// With `dir`, the `.dev` target must be that subdir of whichever checkout it
/// points into. In the lib's own checkout a wrong target is only a stale link.
#[test]
fn check_git_dev_redirect_must_reach_subdir() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "subredir", "type": "git", "ref": "main", "url": "{}", "dir": "mylib"}}
        ]
    }}"#,
        common::file_url(&repo_path)
    );
    let main = common::project_with_hmm_json(&json);
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(main.path())
        .arg("install")
        .assert()
        .success();
    let main_git = main.path().join(".haxelib/subredir/git");

    let worktree = worktree_with_dev_redirect(&json, "subredir", &main_git.join("mylib"));
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(worktree.path())
        .arg("check")
        .assert()
        .success();

    let worktree = worktree_with_dev_redirect(&json, "subredir", &main_git);
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(worktree.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "not the checkout's 'mylib' subdir",
        ));

    std::fs::write(
        main.path().join(".haxelib/subredir/.dev"),
        main_git.to_str().unwrap(),
    )
    .unwrap();
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(main.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "has a stale dev link outside its subdir 'mylib'",
        ));
}
