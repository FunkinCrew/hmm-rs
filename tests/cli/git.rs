use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::common;

#[test]
fn git_install_with_subdir_creates_dev_file() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let temp = common::initialized_project();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["git", "mylib", &url, "main", "mylib"])
        .assert()
        .success();

    let dev_file = temp.child(".haxelib/mylib/.dev");
    dev_file.assert(predicate::path::is_file());

    let dev_content = std::fs::read_to_string(dev_file.path()).unwrap();
    assert!(
        dev_content.contains("git/mylib"),
        "dev file should point into git/mylib subdir, got: {dev_content}"
    );

    let json_content = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert!(json_content.contains("\"dir\": \"mylib\""));
}

#[test]
fn git_install_without_subdir_creates_no_dev_file() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let temp = common::initialized_project();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["git", "mylib", &url, "main"])
        .assert()
        .success();

    temp.child(".haxelib/mylib/.dev")
        .assert(predicate::path::is_file().not());
    temp.child(".haxelib/mylib/.current")
        .assert(predicate::path::is_file());
}

#[test]
fn install_from_hmm_json_with_subdir_creates_dev_file() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);

    let json = format!(
        r#"{{
  "dependencies": [
    {{ "name": "mylib", "type": "git", "dir": "mylib", "ref": "main", "url": "{url}" }}
  ]
}}"#
    );
    let temp = common::project_with_hmm_json(&json);
    temp.child(".haxelib").create_dir_all().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success();

    let dev_file = temp.child(".haxelib/mylib/.dev");
    dev_file.assert(predicate::path::is_file());

    let dev_content = std::fs::read_to_string(dev_file.path()).unwrap();
    assert!(
        dev_content.contains("git/mylib"),
        "dev file should point into git/mylib subdir, got: {dev_content}"
    );
}

#[test]
fn install_relinks_missing_dev_file_for_subdir_git() {
    // Simulates a checkout where the git repo is present at the correct commit but the
    // `.dev` subdir link is missing (e.g. installed by an older hmm-rs). `hmm-rs install`
    // should detect and re-create the `.dev` link without a full re-clone.
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);

    let json = format!(
        r#"{{
  "dependencies": [
    {{ "name": "mylib", "type": "git", "dir": "mylib", "ref": "main", "url": "{url}" }}
  ]
}}"#
    );
    let temp = common::project_with_hmm_json(&json);
    temp.child(".haxelib").create_dir_all().unwrap();

    // First install creates the git clone + the `.dev` link.
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success();
    let dev_file = temp.child(".haxelib/mylib/.dev");
    dev_file.assert(predicate::path::is_file());

    // Delete the dev link to simulate the pre-fix / broken state.
    std::fs::remove_file(dev_file.path()).unwrap();

    // check should flag the missing dev link.
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .stdout(predicate::str::contains("missing its dev link"));

    // install should re-create it.
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success();
    dev_file.assert(predicate::path::is_file());
    let dev_content = std::fs::read_to_string(dev_file.path()).unwrap();
    assert!(dev_content.contains("git/mylib"));
}
