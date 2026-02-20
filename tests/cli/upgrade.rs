use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn upgrade_check_prints_current_version() {
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .args(["upgrade", "--check"])
        .assert()
        .stdout(predicate::str::contains("Current version: v"));
}

#[test]
fn self_update_alias_works() {
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .args(["self-update", "--check"])
        .assert()
        .stdout(predicate::str::contains("Current version: v"));
}
