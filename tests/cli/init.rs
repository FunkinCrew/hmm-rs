use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

#[test]
fn init_creates_haxelib_dir_and_hmm_json() {
    let temp = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Creating .haxelib/ folder"));

    temp.child(".haxelib").assert(predicate::path::is_dir());
    temp.child("hmm.json").assert(predicate::path::is_file());

    let content = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert!(content.contains("\"dependencies\""));
}

#[test]
fn init_succeeds_when_haxelib_already_exists() {
    let temp = assert_fs::TempDir::new().unwrap();
    temp.child(".haxelib").create_dir_all().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .success();

    temp.child("hmm.json").assert(predicate::path::is_file());
}

/// Regression: in a fresh clone (hmm.json committed, no .haxelib/), `init`
/// replaced hmm.json with an empty dependency list.
#[test]
fn init_does_not_overwrite_existing_hmm_json() {
    let temp = assert_fs::TempDir::new().unwrap();
    let original = r#"{"dependencies":[{"name":"test","type":"haxelib","version":"1.0.0"}]}"#;
    temp.child("hmm.json").write_str(original).unwrap();

    // First run creates .haxelib/, second run finds both already present.
    for _ in 0..2 {
        Command::cargo_bin("hmm-rs")
            .unwrap()
            .current_dir(temp.path())
            .arg("init")
            .assert()
            .success()
            .stdout(predicate::str::contains("hmm.json already exists"));
    }

    temp.child(".haxelib").assert(predicate::path::is_dir());
    let content = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert_eq!(content, original);
}

#[test]
fn init_writes_repo_version_marker() {
    let temp = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .success();

    // haxelib >= 4.2.0 reads this on every command and nags about `fixrepo`
    // when it is missing; the format is the integer plus a newline.
    let content = std::fs::read_to_string(temp.child(".haxelib/.repo-version").path()).unwrap();
    assert_eq!(content, "1\n");
}
