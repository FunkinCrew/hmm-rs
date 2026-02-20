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
fn init_fails_when_haxelib_already_exists() {
    let temp = assert_fs::TempDir::new().unwrap();
    temp.child(".haxelib").create_dir_all().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn init_does_not_overwrite_existing_hmm_json() {
    let temp = assert_fs::TempDir::new().unwrap();
    temp.child(".haxelib").create_dir_all().unwrap();
    let original = r#"{"dependencies":[{"name":"test","type":"haxelib","version":"1.0.0"}]}"#;
    temp.child("hmm.json").write_str(original).unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .failure();

    // hmm.json should be unchanged since init failed at the .haxelib step
    let content = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert_eq!(content, original);
}
