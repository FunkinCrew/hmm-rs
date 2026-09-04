use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::hmm::{
    self,
    haxelib::{lib_dir_path_for_name, Haxelib, HaxelibType},
};

/// Writes the `.dev` marker for library `name` pointing at `absolute_path`.
///
/// Creates `.haxelib/<name-with-commas>/` if needed and writes
/// `.haxelib/<name-with-commas>/.dev` containing the given path. `absolute_path`
/// is expected to already be absolute (callers canonicalize before calling).
pub fn write_dev_file(name: &str, absolute_path: &Path) -> Result<()> {
    let lib_dir = lib_dir_path_for_name(name);
    if !lib_dir.exists() {
        fs::create_dir_all(&lib_dir)?;
    }

    let dev_file_path = lib_dir.join(".dev");
    let mut dev_file = fs::File::create(dev_file_path)?;
    dev_file.write_all(absolute_path.to_string_lossy().as_bytes())?;
    Ok(())
}

/// Removes the `.dev` marker for library `name`, if one exists.
/// Returns whether a marker was removed.
pub fn remove_dev_file(name: &str) -> Result<bool> {
    match fs::remove_file(lib_dir_path_for_name(name).join(".dev")) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub fn add_dev_dependency(name: &str, path: &str, json_path: PathBuf) -> Result<()> {
    hmm::haxelib::validate_lib_name(name)?;

    // Convert to absolute path
    let absolute_path = Path::new(path).canonicalize()?;

    let dev_haxelib = Haxelib {
        name: name.to_string(),
        haxelib_type: HaxelibType::Dev,
        vcs_ref: None,
        dir: None,
        path: Some(path.to_string()),
        url: None,
        version: None,
    };

    write_dev_file(name, &absolute_path)?;

    // Replaces any existing entry with the same name in place rather than appending a duplicate.
    hmm::json::upsert_dependencies(&json_path, &[dev_haxelib])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // write_dev_file writes to a cwd-relative `.haxelib/` path, so these tests
    // temporarily change the process cwd and must not run concurrently.
    static CWD_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn write_dev_file_creates_dev_file() {
        let _guard = CWD_LOCK.lock().unwrap();
        let temp = assert_fs::TempDir::new().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let abs = temp.path().join("some").join("dev").join("dir");
        let result = write_dev_file("mylib", &abs);

        std::env::set_current_dir(&original).unwrap();
        result.unwrap();

        let dev_path = temp.path().join(".haxelib/mylib/.dev");
        assert!(dev_path.is_file());
        let content = fs::read_to_string(&dev_path).unwrap();
        assert_eq!(content, abs.to_string_lossy());
    }

    #[test]
    fn write_dev_file_dotted_name_uses_comma_folder() {
        let _guard = CWD_LOCK.lock().unwrap();
        let temp = assert_fs::TempDir::new().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let abs = temp.path().join("funkin-vis-src");
        let result = write_dev_file("funkin.vis", &abs);

        std::env::set_current_dir(&original).unwrap();
        result.unwrap();

        assert!(temp.path().join(".haxelib/funkin,vis/.dev").is_file());
    }

    #[test]
    fn remove_dev_file_removes_marker_and_reports_absence() {
        let _guard = CWD_LOCK.lock().unwrap();
        let temp = assert_fs::TempDir::new().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let written = write_dev_file("mylib", &temp.path().join("src"));
        let first = remove_dev_file("mylib");
        let second = remove_dev_file("mylib");

        std::env::set_current_dir(&original).unwrap();
        written.unwrap();
        assert!(first.unwrap());
        assert!(!second.unwrap());
        assert!(!temp.path().join(".haxelib/mylib/.dev").exists());
    }
}
