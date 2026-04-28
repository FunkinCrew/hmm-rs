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
