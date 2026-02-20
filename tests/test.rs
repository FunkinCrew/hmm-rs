use std::path::PathBuf;

use hmm_rs::commands::*;
use hmm_rs::hmm;
use hmm_rs::hmm::haxelib::HaxelibType;

mod common;

#[test]
fn test_clean_haxelib_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let haxelib_dir = tmp.path().join(".haxelib");
    std::fs::create_dir(&haxelib_dir).unwrap();

    // Should succeed when .haxelib exists
    assert!(clean_command::remove_haxelib_folder_at(tmp.path()).is_ok());
    assert!(!haxelib_dir.exists());

    // Should fail when .haxelib is already gone
    assert!(clean_command::remove_haxelib_folder_at(tmp.path()).is_err());
}

#[test]
fn test_create_haxelib_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let haxelib_dir = tmp.path().join(".haxelib");

    // Should succeed when .haxelib doesn't exist
    assert!(init_command::create_haxelib_folder_at(tmp.path()).is_ok());
    assert!(haxelib_dir.exists());

    // Should fail when .haxelib already exists
    assert!(init_command::create_haxelib_folder_at(tmp.path()).is_err());
}

#[test]
fn test_hmm_json_read_flixel() {
    let flixel_json = PathBuf::new()
        .join(common::get_samples_dir())
        .join("flixel.json");
    let deps = hmm::json::read_json(&flixel_json).unwrap();

    assert_eq!(deps.dependencies.len(), 1);
    let dep = &deps.dependencies[0];
    assert_eq!(dep.name, "flixel");
    assert_eq!(dep.haxelib_type, HaxelibType::Git);
    assert_eq!(dep.vcs_ref.as_deref(), Some("master"));
    assert_eq!(
        dep.url.as_deref(),
        Some("https://github.com/haxeflixel/flixel")
    );
    assert_eq!(dep.version, None);
}

#[test]
fn test_hmm_json_read_mixed_types() {
    let hmm_json = PathBuf::new()
        .join(common::get_samples_dir())
        .join("hmm.json");
    let deps = hmm::json::read_json(&hmm_json).unwrap();

    assert_eq!(deps.dependencies.len(), 4);

    // flixel (git)
    assert_eq!(deps.dependencies[0].name, "flixel");
    assert_eq!(deps.dependencies[0].haxelib_type, HaxelibType::Git);
    assert_eq!(deps.dependencies[0].vcs_ref.as_deref(), Some("master"));

    // flixel-addons (haxelib)
    assert_eq!(deps.dependencies[1].name, "flixel-addons");
    assert_eq!(deps.dependencies[1].haxelib_type, HaxelibType::Haxelib);
    assert_eq!(deps.dependencies[1].version.as_deref(), Some("3.3.0"));

    // funkin.vis (git, name with dots)
    assert_eq!(deps.dependencies[2].name, "funkin.vis");
    assert_eq!(deps.dependencies[2].haxelib_type, HaxelibType::Git);
    assert_eq!(deps.dependencies[2].vcs_ref.as_deref(), Some("main"));

    // hxcpp (git, tagged ref)
    assert_eq!(deps.dependencies[3].name, "hxcpp");
    assert_eq!(deps.dependencies[3].haxelib_type, HaxelibType::Git);
    assert_eq!(deps.dependencies[3].vcs_ref.as_deref(), Some("v4.3.68"));
}

#[test]
fn test_hmm_json_read_version_null() {
    let version_null_json = PathBuf::new()
        .join(common::get_samples_dir())
        .join("version_null.json");
    let deps = hmm::json::read_json(&version_null_json).unwrap();

    assert_eq!(deps.dependencies.len(), 1);
    let dep = &deps.dependencies[0];
    assert_eq!(dep.name, "format");
    assert_eq!(dep.haxelib_type, HaxelibType::Haxelib);
    assert_eq!(dep.version, None);
}
