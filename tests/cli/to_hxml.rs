use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::common;

#[test]
fn to_hxml_outputs_libs_to_stdout() {
    let json = common::sample_fixture_content("hmm.json");
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("to-hxml")
        .assert()
        .success()
        .stdout(
            predicate::str::contains(
                "-lib flixel:git:https://github.com/haxeflixel/flixel#master",
            )
            .and(predicate::str::contains("-lib flixel-addons:3.3.0"))
            .and(predicate::str::contains(
                "-lib funkin.vis:git:https://github.com/FunkinCrew/funkVis#main",
            ))
            .and(predicate::str::contains(
                "-lib hxcpp:git:https://github.com/HaxeFoundation/hxcpp#v4.3.68",
            )),
        );
}

#[test]
fn to_hxml_writes_to_file() {
    let json = common::sample_fixture_content("hmm.json");
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["to-hxml", "output.hxml"])
        .assert()
        .success();

    temp.child("output.hxml").assert(predicate::path::is_file());
    let content = std::fs::read_to_string(temp.child("output.hxml").path()).unwrap();
    assert!(content.contains("-lib flixel:git:"));
    assert!(content.contains("-lib flixel-addons:3.3.0"));
}

#[test]
fn to_hxml_with_json_flag() {
    let json = common::sample_fixture_content("flixel.json");
    let temp = assert_fs::TempDir::new().unwrap();
    temp.child("custom.json").write_str(&json).unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["--json", "custom.json", "to-hxml"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "-lib flixel:git:https://github.com/haxeflixel/flixel#master",
        ));
}

#[test]
fn to_hxml_fails_when_no_hmm_json() {
    let temp = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("to-hxml")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}
