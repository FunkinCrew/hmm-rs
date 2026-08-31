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
        .success()
        .stdout(predicate::str::contains("is not installed"));
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
        .success()
        .stdout(predicate::str::contains("is not at the correct version"));
}

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
        .success()
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
        .success()
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
        .success()
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
        .success()
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
        .success()
        .stdout(predicate::str::contains("is not cloned / installed (via git)"));
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
