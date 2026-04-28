use assert_cmd::Command;
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
