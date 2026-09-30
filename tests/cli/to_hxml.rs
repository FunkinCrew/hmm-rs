use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::common;

const HMM_JSON_HXML: &str = "-lib flixel:git:https://github.com/haxeflixel/flixel#master\n\
-lib flixel-addons:3.3.0\n\
-lib funkin.vis:git:https://github.com/FunkinCrew/funkVis#main\n\
-lib hxcpp:git:https://github.com/HaxeFoundation/hxcpp#v4.3.68\n";

#[test]
fn to_hxml_outputs_libs_to_stdout() {
    let json = common::sample_fixture_content("hmm.json");
    let temp = common::project_with_hmm_json(&json);

    // A git dep without `dir` carries its url and ref so that
    // `haxelib install <file>.hxml` can clone it. One line per dep, and no
    // blank line after the last one.
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("to-hxml")
        .assert()
        .success()
        .stdout(predicate::eq(HMM_JSON_HXML));
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
        .success()
        .stdout(predicate::str::is_empty());

    temp.child("output.hxml").assert(predicate::path::is_file());
    let content = std::fs::read_to_string(temp.child("output.hxml").path()).unwrap();
    assert_eq!(content, HMM_JSON_HXML);
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
        .stdout(predicate::eq(
            "-lib flixel:git:https://github.com/haxeflixel/flixel#master\n",
        ));
}

/// A git dep with `dir` is installed as `.current = git` plus a `.dev` marker
/// pointing into `git/<dir>`. Only a bare `-lib name` makes `haxelib path`
/// follow `.dev`; `name:git:...` is the explicit version `git`, which resolves
/// `<lib>/git/` (the repo root). No hxml syntax carries a subdirectory, so
/// `haxelib install <file>.hxml` could not reproduce this dep either way.
#[test]
fn to_hxml_git_dep_with_dir_emits_bare_name() {
    let json = common::sample_fixture_content("git_subdir.json");
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("to-hxml")
        .assert()
        .success()
        .stdout(predicate::eq("-lib funkin.vis\n"));
}

#[test]
fn to_hxml_git_dep_without_ref_has_no_hash() {
    let temp = common::project_with_hmm_json(
        r#"{"dependencies":[{"name":"noref","type":"git","url":"https://example.com/x/noref"}]}"#,
    );

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("to-hxml")
        .assert()
        .success()
        .stdout(predicate::eq(
            "-lib noref:git:https://example.com/x/noref\n",
        ));
}

/// Like hmm: a haxelib dep with no version, a git dep with no url and a dev
/// dep all render as a bare `-lib name` (resolved through `.dev`/`.current`)
/// instead of failing the whole command.
#[test]
fn to_hxml_emits_bare_name_for_unpinned_deps() {
    let temp = common::project_with_hmm_json(
        r#"{"dependencies":[
            {"name":"format","type":"haxelib","version":null},
            {"name":"nourl","type":"git","ref":"main"},
            {"name":"mylocal","type":"dev","path":"../mylocal"},
            {"name":"pinned","type":"haxelib","version":"1.0.0"}
        ]}"#,
    );

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("to-hxml")
        .assert()
        .success()
        .stdout(predicate::eq(
            "-lib format\n-lib nourl\n-lib mylocal\n-lib pinned:1.0.0\n",
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
