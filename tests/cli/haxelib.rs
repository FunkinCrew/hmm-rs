use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::common;

// Hermetic tests for `hmm-rs haxelib`: the registry is a local
// `common::RegistryStub` reached via HMM_HAXELIB_URL. Library names are unique
// per test (downloads land in the shared OS temp dir as `<name>.zip`).
// Note: the haxelib command expects `.haxelib/` to already exist (unlike
// `install`, it does not create it), hence `initialized_project()`.

#[test]
fn haxelib_with_version_installs_from_registry() {
    let stub = common::RegistryStub::serve(&[("regstub-a", "1.2.3")]);
    let temp = common::initialized_project();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .args(["haxelib", "regstub-a@1.2.3"])
        .assert()
        .success();

    let current =
        std::fs::read_to_string(temp.child(".haxelib/regstub-a/.current").path()).unwrap();
    assert_eq!(current, "1.2.3");
    temp.child(".haxelib/regstub-a/1,2,3/haxelib.json")
        .assert(predicate::path::is_file());

    let json_content = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert!(json_content.contains("\"name\": \"regstub-a\""));
    assert!(json_content.contains("\"version\": \"1.2.3\""));
}

#[test]
fn haxelib_without_version_queries_latest() {
    let stub = common::RegistryStub::serve(&[("regstub-b", "2.0.0")]);
    let temp = common::initialized_project();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .args(["haxelib", "regstub-b"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Latest version of regstub-b is 2.0.0"));

    let current =
        std::fs::read_to_string(temp.child(".haxelib/regstub-b/.current").path()).unwrap();
    assert_eq!(current, "2.0.0");
}

#[test]
fn haxelib_unknown_lib_fails_on_latest_query() {
    let stub = common::RegistryStub::serve(&[]);
    let temp = common::initialized_project();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .args(["haxelib", "regstub-nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No such Project"));
}

#[test]
fn haxelib_download_404_fails() {
    let stub = common::RegistryStub::serve(&[("regstub-c", "1.0.0")]);
    let temp = common::initialized_project();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .args(["haxelib", "regstub-c@9.9.9"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to download: HTTP 404"));
}

#[test]
fn haxelib_no_args_errors() {
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .arg("haxelib")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}
