use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::common;

#[test]
fn lock_haxelib_writes_version_to_json() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": null}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("lib-a", "3.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("lock")
        .assert()
        .success()
        .stdout(predicate::str::contains("locked to"));

    let updated_json = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert!(updated_json.contains("3.0.0"));
}

#[test]
fn lock_already_locked_is_skipped() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("lock")
        .assert()
        .success()
        .stdout(predicate::str::contains("skipped/already locked"));
}

#[test]
fn lock_check_detects_unlocked() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": null}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["lock", "check"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("is not locked"));
}

#[test]
fn lock_check_passes_when_all_locked() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["lock", "check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dependencies are locked"));
}

#[test]
fn lock_selective_locks_only_named_libs() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": null},
            {"name": "lib-b", "type": "haxelib", "version": null}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(
        json,
        &[("lib-a", "3.0.0"), ("lib-b", "4.0.0")],
    );

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["lock", "lib-a"])
        .assert()
        .success();

    let updated_json = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert!(updated_json.contains("3.0.0"));
    assert!(!updated_json.contains("4.0.0"));
}

// --- git deps (hermetic: local repos over file://) ---

/// Project with a git dep installed from a local repo, ref "main".
/// Returns (host_repo_tempdir, project_tempdir, full_head_sha).
fn installed_git_project() -> (assert_fs::TempDir, assert_fs::TempDir, String) {
    let (repo_temp, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let head_sha = common::git_rev_parse(&repo_path, "HEAD");

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

    (repo_temp, temp, head_sha)
}

/// Reads the `ref` value of the single dependency in the project's hmm.json.
fn read_locked_ref(temp: &assert_fs::TempDir) -> String {
    let deps = hmm_rs::hmm::json::read_json(&temp.path().join("hmm.json")).unwrap();
    deps.dependencies[0].vcs_ref.clone().unwrap()
}

#[test]
fn lock_git_writes_short_commit_sha() {
    let (_repo, temp, head_sha) = installed_git_project();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("lock")
        .assert()
        .success()
        .stdout(predicate::str::contains("locked to"));

    let locked = read_locked_ref(&temp);
    assert_ne!(locked, "main");
    assert!(
        head_sha.starts_with(&locked) && locked.len() < head_sha.len(),
        "expected a shortened prefix of {head_sha}, got {locked}"
    );
}

#[test]
fn lock_git_long_id_writes_full_sha() {
    let (_repo, temp, head_sha) = installed_git_project();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["lock", "--long-id"])
        .assert()
        .success()
        .stdout(predicate::str::contains("locked to"));

    assert_eq!(read_locked_ref(&temp), head_sha);
}

#[test]
fn lock_git_not_installed_errors() {
    let json = r#"{
        "dependencies": [
            {"name": "gitlib", "type": "git", "ref": "main", "url": "https://example.com/repo.git"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("lock")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Git repository not cloned"))
        .stderr(predicate::str::contains("Failed to lock"));
}

#[test]
fn lock_check_detects_unlocked_git_ref() {
    let json = r#"{
        "dependencies": [
            {"name": "gitlib", "type": "git", "url": "https://example.com/repo.git"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["lock", "check"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("no ref specified"));
}

#[test]
fn lock_skips_dev_dependency() {
    let json = r#"{
        "dependencies": [
            {"name": "devlib", "type": "dev", "path": "some/path"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("lock")
        .assert()
        .success()
        .stdout(predicate::str::contains("dev dependencies are already locked by path"));
}

#[test]
fn lock_unknown_lib_warns_and_proceeds() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": null}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("lib-a", "3.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["lock", "lib-a", "nonexistent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not found in hmm.json"))
        .stdout(predicate::str::contains("locked to"));
}

// --- dotted library names (`funkin.vis`-style) ---

#[test]
fn lock_dotted_haxelib_writes_version_to_json() {
    let json = r#"{
        "dependencies": [
            {"name": "lockdot.vis", "type": "haxelib", "version": null}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("lockdot.vis", "3.1.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("lock")
        .assert()
        .success()
        .stdout(predicate::str::contains("locked to"));

    let updated_json = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert!(updated_json.contains("\"name\": \"lockdot.vis\""));
    assert!(updated_json.contains("\"version\": \"3.1.0\""));
}

/// Regression: `.current` content is trimmed before locking, so a trailing
/// newline never lands inside the hmm.json version string.
#[test]
fn lock_trims_trailing_newline_from_current() {
    let json = r#"{
        "dependencies": [
            {"name": "trimlock-a", "type": "haxelib", "version": null}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("trimlock-a", "3.1.0")]);
    std::fs::write(temp.child(".haxelib/trimlock-a/.current").path(), "3.1.0\n").unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("lock")
        .assert()
        .success()
        .stdout(predicate::str::contains("locked to"));

    let updated_json = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert!(
        updated_json.contains("\"version\": \"3.1.0\""),
        "version should be locked without the trailing newline, got: {updated_json}"
    );
}

#[test]
fn lock_dotted_git_writes_commit_sha() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let head_sha = common::git_rev_parse(&repo_path, "HEAD");
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "lockgitdot.vis", "type": "git", "ref": "main", "url": "{}"}}
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

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("lock")
        .assert()
        .success()
        .stdout(predicate::str::contains("locked to"));

    let locked = read_locked_ref(&temp);
    assert_ne!(locked, "main");
    assert!(
        head_sha.starts_with(&locked),
        "expected a prefix of {head_sha}, got {locked}"
    );

    let updated_json = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert!(updated_json.contains("\"name\": \"lockgitdot.vis\""));
}
