use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::common;

#[test]
fn add_git_with_multiple_names_errors() {
    let temp = common::project_with_empty_hmm_json();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args([
            "add",
            "lib-a",
            "lib-b",
            "--git",
            "https://example.com/repo",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--git installs accept exactly one"));
}

#[test]
fn add_no_args_errors() {
    let temp = common::project_with_empty_hmm_json();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("add")
        .assert()
        .failure();
}

// --- dotted library names (`funkin.vis`-style) ---

#[test]
fn add_dotted_git_name_installs_to_comma_dir() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let temp = common::initialized_project();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["add", "adddot.vis", "--git", &url])
        .assert()
        .success();

    temp.child(".haxelib/adddot,vis/git")
        .assert(predicate::path::is_dir());

    let json_content = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert!(json_content.contains("\"name\": \"adddot.vis\""));
}
