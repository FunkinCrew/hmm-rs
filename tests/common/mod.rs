use std::path::PathBuf;

pub fn get_samples_dir() -> PathBuf {
    let crate_dir = PathBuf::new().join(env!("CARGO_MANIFEST_DIR"));
    let tests_dir = crate_dir.join("tests");
    tests_dir.join("samples")
}
