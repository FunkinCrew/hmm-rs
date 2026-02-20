use assert_cmd::Command;

#[test]
fn remove_is_a_noop_stub() {
    let temp = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["remove", "anything"])
        .assert()
        .success();
}

#[test]
fn remove_alias_rm_works() {
    let temp = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["rm", "anything"])
        .assert()
        .success();
}
