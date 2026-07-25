use assert_cmd::Command;
use predicates::prelude::*;

// Both tests hit the real GitHub releases API, so they are ignored by default.
// CI runs them via `cargo test -- --include-ignored`.

#[test]
#[ignore = "hits the GitHub releases API"]
fn upgrade_check_prints_current_version() {
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .args(["upgrade", "--check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Current version: v"));
}

#[test]
#[ignore = "hits the GitHub releases API"]
fn self_update_alias_works() {
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .args(["self-update", "--check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Current version: v"));
}
