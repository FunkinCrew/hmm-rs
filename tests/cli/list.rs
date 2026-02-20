use assert_cmd::Command;
use predicates::prelude::*;

use crate::common;

#[test]
fn list_shows_all_dependencies() {
    let json = common::sample_fixture_content("hmm.json");
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("list")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("flixel")
                .and(predicate::str::contains("flixel-addons"))
                .and(predicate::str::contains("funkin.vis"))
                .and(predicate::str::contains("hxcpp")),
        );
}

#[test]
fn list_alias_ls_works() {
    let json = common::sample_fixture_content("hmm.json");
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("ls")
        .assert()
        .success()
        .stdout(predicate::str::contains("flixel"));
}

#[test]
fn list_specific_library() {
    let json = common::sample_fixture_content("hmm.json");
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["list", "flixel-addons"])
        .assert()
        .success()
        .stdout(predicate::str::contains("flixel-addons"));
}

#[test]
fn list_empty_deps() {
    let temp = common::project_with_empty_hmm_json();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("list")
        .assert()
        .success();
}

#[test]
fn list_fails_without_hmm_json() {
    let temp = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("list")
        .assert()
        .failure();
}
