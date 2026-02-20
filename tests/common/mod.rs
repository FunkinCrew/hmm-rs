#![allow(dead_code)]

use std::path::PathBuf;

use assert_fs::TempDir;
use assert_fs::prelude::*;

pub fn get_samples_dir() -> PathBuf {
    let crate_dir = PathBuf::new().join(env!("CARGO_MANIFEST_DIR"));
    let tests_dir = crate_dir.join("tests");
    tests_dir.join("samples")
}

/// Creates a TempDir with an empty hmm.json (`{"dependencies":[]}`)
pub fn project_with_empty_hmm_json() -> TempDir {
    let temp = TempDir::new().unwrap();
    temp.child("hmm.json")
        .write_str("{\"dependencies\":[]}")
        .unwrap();
    temp
}

/// Creates a TempDir with hmm.json + .haxelib/ directory
pub fn initialized_project() -> TempDir {
    let temp = project_with_empty_hmm_json();
    temp.child(".haxelib").create_dir_all().unwrap();
    temp
}

/// Creates a TempDir with a custom hmm.json content
pub fn project_with_hmm_json(json: &str) -> TempDir {
    let temp = TempDir::new().unwrap();
    temp.child("hmm.json").write_str(json).unwrap();
    temp
}

/// Creates a TempDir with hmm.json and .haxelib/<lib>/.current files
pub fn project_with_installed_haxelibs(json: &str, libs: &[(&str, &str)]) -> TempDir {
    let temp = project_with_hmm_json(json);
    temp.child(".haxelib").create_dir_all().unwrap();
    for (name, version) in libs {
        let lib_name = name.replace(".", ",");
        temp.child(format!(".haxelib/{lib_name}/.current"))
            .write_str(version)
            .unwrap();
    }
    temp
}

/// Reads a sample fixture file content
pub fn sample_fixture_content(name: &str) -> String {
    std::fs::read_to_string(get_samples_dir().join(name)).unwrap()
}
