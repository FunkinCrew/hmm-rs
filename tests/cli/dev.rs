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

/// Pins the exact `.dev` file format: the canonicalized absolute path, no
/// trailing newline. Divergence from real haxelib (documented, intentional for
/// now): haxelib normalizes with a trailing slash; both agree on no newline,
/// and haxelib re-normalizes on read so the missing slash is tolerated.
#[test]
fn dev_file_contains_exact_absolute_path() {
    let temp = common::initialized_project();
    let source_dir = temp.child("my-lib-src");
    source_dir.create_dir_all().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["dev", "my-lib", "my-lib-src"])
        .assert()
        .success();

    let expected = std::fs::canonicalize(source_dir.path()).unwrap();
    let dev_content = std::fs::read_to_string(temp.child(".haxelib/my-lib/.dev").path()).unwrap();
    assert_eq!(dev_content, expected.to_string_lossy());
}

#[test]
fn dev_fails_with_nonexistent_path() {
    let temp = common::initialized_project();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["dev", "my-lib", "/nonexistent/path/to/lib"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No such file"));
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

#[test]
fn dev_creates_haxelib_dir_with_repo_version_marker() {
    let temp = common::project_with_empty_hmm_json();
    temp.child("src-lib").create_dir_all().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["dev", "src-lib", "src-lib"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Creating .haxelib/ folder"));

    temp.child(".haxelib/src-lib/.dev")
        .assert(predicate::path::is_file());
    let marker = std::fs::read_to_string(temp.child(".haxelib/.repo-version").path()).unwrap();
    assert_eq!(marker, "1\n");
}
