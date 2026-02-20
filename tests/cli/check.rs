use assert_cmd::Command;
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
