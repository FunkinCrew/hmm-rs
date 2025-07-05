use std::{fs, path::PathBuf};

use yansi::Paint;

pub fn setup() {
    // some setup code, like creating required files/directories, starting
    // servers, etc.
    let crate_dir = PathBuf::new().join(env!("CARGO_MANIFEST_DIR"));
    let tests_dir = crate_dir.join("tests");
    let samples_dir = tests_dir.join("samples");
}

pub fn remove_haxelib_folder() {
    let haxelib_path = get_samples_dir().join(".haxelib");
    match fs::remove_dir_all(&haxelib_path) {
        Ok(_) => println!("{} .haxelib/ folder removed", "Removed".green().bold()),
        Err(e) => {
            println!(
                "{} .haxelib/ folder does not exist: {}",
                "Error".red().bold(),
                e
            );
        }
    }
}

pub fn setup_haxelib_folder() {
    let haxelib_path = get_samples_dir().join(".haxelib");
    if haxelib_path.exists() {
        fs::remove_dir_all(&haxelib_path).unwrap();
    }
    match fs::create_dir(haxelib_path) {
        Ok(_) => println!("{} .haxelib/ folder created", "Created".green().bold()),
        Err(e) => println!(
            "{} .haxelib/ folder already exists: {}",
            "Error".red().bold(),
            e
        ),
    }
}

pub fn get_samples_dir() -> PathBuf {
    let crate_dir = PathBuf::new().join(env!("CARGO_MANIFEST_DIR"));
    let tests_dir = crate_dir.join("tests");
    tests_dir.join("samples")
}
