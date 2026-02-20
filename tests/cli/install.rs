use assert_cmd::Command;
use predicates::prelude::*;

use crate::common;

/// Regression test: `install` used to panic with `unwrap()` on `None` in
/// `print_install_status()` when `.haxelib/` directory didn't exist.
/// See check_command.rs:234 — now uses `unwrap_or("unknown")`.
#[test]
fn install_does_not_panic_without_haxelib_dir() {
    let json = r#"{
        "dependencies": [
            {"name": "flixel", "type": "haxelib", "version": "5.0.0"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    let assert = Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert();

    // The process must not panic (exit code 101 on panic).
    // It may fail for network reasons, but it must not crash.
    assert
        .code(predicate::ne(101))
        .stdout(predicate::str::contains("Creating .haxelib/ folder"));
}

/// Same regression scenario but with a git-type dependency.
#[test]
fn install_git_dep_does_not_panic_without_haxelib_dir() {
    let json = r#"{
        "dependencies": [
            {"name": "flixel", "type": "git", "url": "https://github.com/HaxeFlixel/flixel.git"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    let assert = Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert();

    assert
        .code(predicate::ne(101))
        .stdout(predicate::str::contains("Creating .haxelib/ folder"));
}

/// Multiple deps, none installed, no .haxelib — verifies iteration doesn't
/// panic on any dep.
#[test]
fn install_multiple_deps_does_not_panic_without_haxelib_dir() {
    let json = r#"{
        "dependencies": [
            {"name": "flixel", "type": "haxelib", "version": "5.0.0"},
            {"name": "lime", "type": "haxelib", "version": "8.0.0"},
            {"name": "openfl", "type": "git", "url": "https://github.com/openfl/openfl.git"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    let assert = Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert();

    assert
        .code(predicate::ne(101))
        .stdout(predicate::str::contains("Creating .haxelib/ folder"));
}
