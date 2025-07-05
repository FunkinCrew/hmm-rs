use std::path::PathBuf;

use common::remove_haxelib_folder;
use hmm_rs::{commands::*, hmm};
mod common;

#[test]
fn test_clean_haxelib_folder() {
    common::setup_haxelib_folder();
    assert!(clean_command::remove_haxelib_folder().is_ok());
    assert!(clean_command::remove_haxelib_folder().is_err());
}

#[test]
fn test_create_haxelib_folder() {
    remove_haxelib_folder();
    assert!(init_command::create_haxelib_folder().is_ok());
    assert!(init_command::create_haxelib_folder().is_err());

    remove_haxelib_folder();
}

#[test]
fn test_hmm_json_read() {
    // common::setup();
    let flixel_json = PathBuf::new()
        .join(common::get_samples_dir())
        .join("flixel.json");
    assert!(hmm::json::read_json(&flixel_json).is_ok());
}
