use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::common;

// All tests here are hermetic: haxelib-type deps download from a local
// `common::RegistryStub` (via HMM_HAXELIB_URL), git-type deps clone local
// repos over `file://`. Library names are unique per test because haxelib
// downloads land in the shared OS temp dir as `<name>.zip`.

/// Regression test: `install` used to panic with `unwrap()` on `None` in
/// `print_install_status()` when `.haxelib/` directory didn't exist.
/// See check_command.rs:234 — now uses `unwrap_or("unknown")`.
#[test]
fn install_does_not_panic_without_haxelib_dir() {
    let stub = common::RegistryStub::serve(&[("hermit-a", "1.0.0")]);
    let json = r#"{
        "dependencies": [
            {"name": "hermit-a", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("Creating .haxelib/ folder"));

    let current = std::fs::read_to_string(temp.child(".haxelib/hermit-a/.current").path()).unwrap();
    assert_eq!(current, "1.0.0");
    temp.child(".haxelib/hermit-a/1,0,0/haxelib.json")
        .assert(predicate::path::is_file());
}

/// Same regression scenario but with a git-type dependency.
#[test]
fn install_git_dep_does_not_panic_without_haxelib_dir() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "hermit-b", "type": "git", "ref": "main", "url": "{url}"}}
        ]
    }}"#
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("Creating .haxelib/ folder"));

    let current = std::fs::read_to_string(temp.child(".haxelib/hermit-b/.current").path()).unwrap();
    assert_eq!(current, "git");
    temp.child(".haxelib/hermit-b/git/README.md")
        .assert(predicate::path::is_file());
}

/// Multiple deps, none installed, no .haxelib — verifies iteration doesn't
/// panic on any dep and every dep really gets installed.
#[test]
fn install_multiple_deps_does_not_panic_without_haxelib_dir() {
    let stub = common::RegistryStub::serve(&[("hermit-c1", "1.0.0"), ("hermit-c2", "2.0.0")]);
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "hermit-c1", "type": "haxelib", "version": "1.0.0"}},
            {{"name": "hermit-c2", "type": "haxelib", "version": "2.0.0"}},
            {{"name": "hermit-c3", "type": "git", "ref": "main", "url": "{url}"}}
        ]
    }}"#
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("Creating .haxelib/ folder"));

    for (lib, expected) in [
        ("hermit-c1", "1.0.0"),
        ("hermit-c2", "2.0.0"),
        ("hermit-c3", "git"),
    ] {
        let current =
            std::fs::read_to_string(temp.child(format!(".haxelib/{lib}/.current")).path()).unwrap();
        assert_eq!(current, expected, "wrong .current for {lib}");
    }
}

#[test]
fn install_selective_single_lib() {
    let (_repo_a, repo_a) = common::local_git_repo_with_lib_subdir("mylib");
    let (_repo_b, repo_b) = common::local_git_repo_with_lib_subdir("mylib");
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "sel-a", "type": "git", "ref": "main", "url": "{}"}},
            {{"name": "sel-b", "type": "git", "ref": "main", "url": "{}"}}
        ]
    }}"#,
        common::file_url(&repo_a),
        common::file_url(&repo_b)
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["install", "sel-a"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sel-a"))
        .stdout(predicate::str::contains("sel-b").not());

    temp.child(".haxelib/sel-a/.current")
        .assert(predicate::path::is_file());
    temp.child(".haxelib/sel-b")
        .assert(predicate::path::exists().not());
}

#[test]
fn install_selective_multiple_libs() {
    let (_repo_a, repo_a) = common::local_git_repo_with_lib_subdir("mylib");
    let (_repo_b, repo_b) = common::local_git_repo_with_lib_subdir("mylib");
    let (_repo_c, repo_c) = common::local_git_repo_with_lib_subdir("mylib");
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "multi-a", "type": "git", "ref": "main", "url": "{}"}},
            {{"name": "multi-b", "type": "git", "ref": "main", "url": "{}"}},
            {{"name": "multi-c", "type": "git", "ref": "main", "url": "{}"}}
        ]
    }}"#,
        common::file_url(&repo_a),
        common::file_url(&repo_b),
        common::file_url(&repo_c)
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["install", "multi-a", "multi-b"])
        .assert()
        .success()
        .stdout(predicate::str::contains("multi-a"))
        .stdout(predicate::str::contains("multi-b"))
        .stdout(predicate::str::contains("multi-c").not());

    temp.child(".haxelib/multi-a/.current")
        .assert(predicate::path::is_file());
    temp.child(".haxelib/multi-b/.current")
        .assert(predicate::path::is_file());
    temp.child(".haxelib/multi-c")
        .assert(predicate::path::exists().not());
}

#[test]
fn install_unknown_lib_warns() {
    let json = r#"{
        "dependencies": [
            {"name": "known-a", "type": "haxelib", "version": "5.0.0"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["install", "nonexistent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not found in hmm.json"));
}

#[test]
fn install_mixed_known_and_unknown_libs() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "mix-a", "type": "git", "ref": "main", "url": "{url}"}},
            {{"name": "mix-b", "type": "git", "ref": "main", "url": "{url}"}}
        ]
    }}"#
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["install", "mix-a", "bogus"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not found in hmm.json"))
        .stdout(predicate::str::contains("mix-a"));

    temp.child(".haxelib/mix-a/.current")
        .assert(predicate::path::is_file());
    temp.child(".haxelib/mix-b")
        .assert(predicate::path::exists().not());
}

#[test]
fn install_selective_already_installed() {
    let json = r#"{
        "dependencies": [
            {"name": "done-a", "type": "haxelib", "version": "5.0.0"},
            {"name": "done-b", "type": "haxelib", "version": "8.0.0"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("done-a", "5.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["install", "done-a"])
        .assert()
        .success()
        // Quiet mode: already-installed libs produce no per-lib output.
        .stdout(predicate::str::contains("Checking done-a").not())
        .stdout(predicate::str::contains("is installed").not())
        // Non-selected lib should not be touched either.
        .stdout(predicate::str::contains("Checking done-b").not())
        .stdout(predicate::str::contains("done-b").not());
}
