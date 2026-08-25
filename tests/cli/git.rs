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
fn git_without_ref_detects_default_branch() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let temp = common::initialized_project();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["git", "mylib", &url])
        .assert()
        .success()
        .stdout(predicate::str::contains("Detected ref: main"));

    let deps = hmm_rs::hmm::json::read_json(&temp.path().join("hmm.json")).unwrap();
    assert_eq!(deps.dependencies[0].vcs_ref.as_deref(), Some("main"));
}

#[test]
fn git_existing_entry_warns_and_replaces() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let json = r#"{
        "dependencies": [
            {"name": "mylib", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);
    temp.child(".haxelib").create_dir_all().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["git", "mylib", &url, "main"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists in hmm.json"));

    let deps = hmm_rs::hmm::json::read_json(&temp.path().join("hmm.json")).unwrap();
    assert_eq!(deps.dependencies.len(), 1, "entry should be replaced, not duplicated");
    assert_eq!(
        deps.dependencies[0].haxelib_type,
        hmm_rs::hmm::haxelib::HaxelibType::Git
    );
}

#[test]
fn git_add_appends_entry_without_rewriting_the_rest() {
    // Regression: adding one git dep used to re-sort every entry, emit
    // `"dir": null` on all of them and drop the trailing newline. The file must
    // come back byte-identical apart from the appended entry.
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let original = r#"{
  "dependencies": [
    {
      "name": "zeta",
      "type": "haxelib",
      "version": "1.0.0"
    },
    {
      "name": "Alpha",
      "type": "git",
      "ref": "abc123",
      "url": "https://example.com/alpha"
    }
  ]
}
"#;
    let temp = common::project_with_hmm_json(original);
    temp.child(".haxelib").create_dir_all().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["git", "mylib", &url, "main"])
        .assert()
        .success();

    let expected = original.replace(
        "    }\n  ]\n}\n",
        &format!(
            "    }},\n    {{\n      \"name\": \"mylib\",\n      \"type\": \"git\",\n      \"ref\": \"main\",\n      \"url\": \"{url}\"\n    }}\n  ]\n}}\n"
        ),
    );
    let actual = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert_eq!(actual, expected);
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

// --- dotted library names (`funkin.vis`-style) ---

#[test]
fn git_dotted_name_uses_comma_dir() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let temp = common::initialized_project();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["git", "gitdot.vis", &url, "main"])
        .assert()
        .success();

    temp.child(".haxelib/gitdot,vis/git")
        .assert(predicate::path::is_dir());
    let current =
        std::fs::read_to_string(temp.child(".haxelib/gitdot,vis/.current").path()).unwrap();
    assert_eq!(current, "git");

    let json_content = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert!(json_content.contains("\"name\": \"gitdot.vis\""));
}

/// Comma names are rejected up front: `a,b` would alias `a.b` on disk.
#[test]
fn git_rejects_comma_name() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let temp = common::initialized_project();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["git", "a,b", &url, "main"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("is not allowed"));

    temp.child(".haxelib/a,b")
        .assert(predicate::path::missing());
}
