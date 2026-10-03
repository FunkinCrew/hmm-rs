use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn completions_fish_prints_script_without_hmm_json() {
    let temp = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete -c hmm-rs"))
        .stdout(predicate::str::contains(r#"-a "install""#))
        // visible aliases complete too
        .stdout(predicate::str::contains(r#"-a "rm""#));
}

#[test]
fn completions_prints_script_for_every_shell() {
    let temp = assert_fs::TempDir::new().unwrap();

    for shell in ["bash", "elvish", "fish", "powershell", "zsh"] {
        Command::cargo_bin("hmm-rs")
            .unwrap()
            .current_dir(temp.path())
            .args(["completions", shell])
            .assert()
            .success()
            .stdout(predicate::str::contains("hmm-rs"));
    }
}

#[test]
fn completions_rejects_unknown_shell() {
    let temp = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["completions", "tcsh"])
        .assert()
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("invalid value 'tcsh'"));
}

#[test]
fn completions_requires_a_shell() {
    let temp = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("completions")
        .assert()
        .failure()
        .stderr(predicate::str::contains("<SHELL>"));
}
