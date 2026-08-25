use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::common;

#[test]
fn remove_no_args_errors() {
    let temp = common::project_with_empty_hmm_json();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("remove")
        .assert()
        .failure();
}

#[test]
fn remove_drops_entry_from_hmm_json() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": "1.0.0"},
            {"name": "lib-b", "type": "haxelib", "version": "2.0.0"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("lib-a", "1.0.0"), ("lib-b", "2.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["remove", "lib-a"])
        .assert()
        .success()
        .stdout(predicate::str::contains("removed"));

    let updated_json = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert!(!updated_json.contains("lib-a"));
    assert!(updated_json.contains("lib-b"));
}

#[test]
fn remove_deletes_haxelib_folder() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("lib-a", "1.0.0")]);

    let lib_dir = temp.child(".haxelib/lib-a");
    assert!(lib_dir.path().exists());

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["remove", "lib-a"])
        .assert()
        .success();

    assert!(!lib_dir.path().exists());
}

#[test]
fn remove_multiple_libs() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": "1.0.0"},
            {"name": "lib-b", "type": "haxelib", "version": "2.0.0"},
            {"name": "lib-c", "type": "haxelib", "version": "3.0.0"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(
        json,
        &[("lib-a", "1.0.0"), ("lib-b", "2.0.0"), ("lib-c", "3.0.0")],
    );

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["remove", "lib-a", "lib-c"])
        .assert()
        .success();

    let updated_json = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert!(!updated_json.contains("lib-a"));
    assert!(updated_json.contains("lib-b"));
    assert!(!updated_json.contains("lib-c"));
}

#[test]
fn remove_unknown_warns_and_proceeds() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("lib-a", "1.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["remove", "lib-a", "nonexistent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not found in hmm.json"));

    let updated_json = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert!(!updated_json.contains("lib-a"));
}

#[test]
fn remove_alias_rm_works() {
    let json = r#"{
        "dependencies": [
            {"name": "lib-a", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("lib-a", "1.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["rm", "lib-a"])
        .assert()
        .success();
}

// --- dotted library names (`funkin.vis`-style) ---

/// `remove` on a dotted name must delete the comma-encoded directory (and
/// leave everything else alone).
#[test]
fn remove_dotted_name_deletes_comma_dir() {
    let json = r#"{
        "dependencies": [
            {"name": "rmdot.vis", "type": "haxelib", "version": "1.0.0"},
            {"name": "keep-lib", "type": "haxelib", "version": "2.0.0"}
        ]
    }"#;
    let temp =
        common::project_with_installed_haxelibs(json, &[("rmdot.vis", "1.0.0"), ("keep-lib", "2.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["remove", "rmdot.vis"])
        .assert()
        .success()
        .stdout(predicate::str::contains("removed"));

    let updated_json = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert!(!updated_json.contains("rmdot.vis"));
    assert!(updated_json.contains("keep-lib"));

    temp.child(".haxelib/rmdot,vis")
        .assert(predicate::path::missing());
    temp.child(".haxelib/keep-lib/.current")
        .assert(predicate::path::is_file());
}
