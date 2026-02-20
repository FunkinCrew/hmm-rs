use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

#[test]
fn clean_removes_haxelib_dir() {
    let temp = assert_fs::TempDir::new().unwrap();
    temp.child(".haxelib").create_dir_all().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("clean")
        .assert()
        .success()
        .stdout(predicate::str::contains("Removing"));

    temp.child(".haxelib").assert(predicate::path::missing());
}

#[test]
fn clean_removes_haxelib_with_contents() {
    let temp = assert_fs::TempDir::new().unwrap();
    temp.child(".haxelib/some-lib/.current")
        .write_str("1.0.0")
        .unwrap();
    temp.child(".haxelib/other-lib/git/.git/HEAD")
        .write_str("ref: refs/heads/main")
        .unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("clean")
        .assert()
        .success();

    temp.child(".haxelib").assert(predicate::path::missing());
}

#[test]
fn clean_fails_when_no_haxelib_dir() {
    let temp = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("clean")
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn clean_alias_cl_works() {
    let temp = assert_fs::TempDir::new().unwrap();
    temp.child(".haxelib").create_dir_all().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("cl")
        .assert()
        .success();
}
