use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::common;

#[test]
fn dev_adds_dependency_and_creates_dev_file() {
    let temp = common::initialized_project();
    let source_dir = temp.child("my-lib-src");
    source_dir.create_dir_all().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["dev", "my-lib", "my-lib-src"])
        .assert()
        .success();

    temp.child(".haxelib/my-lib/.dev")
        .assert(predicate::path::is_file());

    let dev_content = std::fs::read_to_string(temp.child(".haxelib/my-lib/.dev").path()).unwrap();
    assert!(dev_content.contains("my-lib-src"));

    let json_content = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert!(json_content.contains("my-lib"));
    assert!(json_content.contains("\"type\": \"dev\""));
}

#[test]
fn dev_with_dotted_name_converts_to_commas() {
    let temp = common::initialized_project();
    let source_dir = temp.child("funkin-vis-src");
    source_dir.create_dir_all().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["dev", "funkin.vis", "funkin-vis-src"])
        .assert()
        .success();

    temp.child(".haxelib/funkin,vis/.dev")
        .assert(predicate::path::is_file());
}

#[test]
fn dev_fails_with_nonexistent_path() {
    let temp = common::initialized_project();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["dev", "my-lib", "/nonexistent/path/to/lib"])
        .assert()
        .failure();
}

#[test]
fn dev_overwrites_existing_entry_instead_of_duplicating() {
    // hmm.json already has a git entry for `mylib`; `hmm-rs dev mylib <path>` should
    // replace it with a single dev entry, not append a duplicate.
    let json = r#"{
  "dependencies": [
    { "name": "mylib", "type": "git", "ref": "master", "url": "https://example.com/mylib" }
  ]
}"#;
    let temp = common::project_with_hmm_json(json);
    temp.child(".haxelib").create_dir_all().unwrap();
    temp.child("mylib-src").create_dir_all().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["dev", "mylib", "mylib-src"])
        .assert()
        .success();

    let json_content = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert_eq!(
        json_content.matches("\"name\": \"mylib\"").count(),
        1,
        "expected exactly one mylib entry, got: {json_content}"
    );
    assert!(json_content.contains("\"type\": \"dev\""));
    assert!(!json_content.contains("\"type\": \"git\""));
}
