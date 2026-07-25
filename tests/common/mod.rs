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

pub fn run_git(repo: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(["-C", repo.to_str().unwrap()])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {:?} failed", args);
}

/// Resolves `rev` to a full commit SHA in the given repo.
pub fn git_rev_parse(repo: &std::path::Path, rev: &str) -> String {
    let out = std::process::Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "rev-parse", rev])
        .output()
        .unwrap();
    assert!(out.status.success(), "git rev-parse {rev} failed");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

pub fn local_git_repo_with_lib_subdir(subdir: &str) -> (TempDir, PathBuf) {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().join("host").join("mylib-repo");
    std::fs::create_dir_all(repo_path.join(subdir)).unwrap();
    std::fs::write(repo_path.join("README.md"), "root\n").unwrap();
    std::fs::write(
        repo_path.join(subdir).join("haxelib.json"),
        "{\"name\":\"mylib\"}\n",
    )
    .unwrap();

    run_git(&repo_path, &["init", "-q", "-b", "main"]);
    run_git(&repo_path, &["config", "user.email", "test@example.com"]);
    run_git(&repo_path, &["config", "user.name", "test"]);
    run_git(&repo_path, &["add", "-A"]);
    run_git(&repo_path, &["commit", "-qm", "init"]);
    (temp, repo_path)
}

/// Returns a `file://` clone URL for a local repo path.
pub fn file_url(path: &std::path::Path) -> String {
    format!("file://{}", path.to_str().unwrap())
}

/// A local HTTP stub of the lib.haxe.org registry for hermetic haxelib-install
/// tests. Point the CLI at it with `.env("HMM_HAXELIB_URL", &stub.base_url)`.
///
/// Serves:
/// - `GET /p/<name>/<version>/download` — a zip with a minimal haxelib.json for
///   known (name, version) pairs, HTTP 404 otherwise
/// - `GET /api/3.0/index.n/?__x=...` — a Haxe-remoting `getLatestVersion` reply
///   for known names, a "No such Project" reply otherwise
///
/// Use a unique library name per test: downloads land in the shared OS temp dir
/// as `<name>.zip`, so parallel tests using the same name would race.
pub struct RegistryStub {
    pub base_url: String,
    server: std::sync::Arc<tiny_http::Server>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl RegistryStub {
    pub fn serve(libs: &[(&str, &str)]) -> Self {
        let libs: Vec<(String, String)> = libs
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect();
        let server = std::sync::Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let addr = server.server_addr().to_ip().unwrap();
        let base_url = format!("http://{addr}");
        let srv = server.clone();
        let handle = std::thread::spawn(move || {
            for request in srv.incoming_requests() {
                let url = request.url().to_string();
                let _ = request.respond(route(&url, &libs));
            }
        });
        Self {
            base_url,
            server,
            handle: Some(handle),
        }
    }
}

impl Drop for RegistryStub {
    fn drop(&mut self) {
        self.server.unblock();
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn route(url: &str, libs: &[(String, String)]) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let path = url.split('?').next().unwrap_or(url);
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    if parts.len() == 4 && parts[0] == "p" && parts[3] == "download" {
        let (name, version) = (parts[1], parts[2]);
        if libs.iter().any(|(n, v)| n == name && v == version) {
            return tiny_http::Response::from_data(build_lib_zip(name, version));
        }
        return tiny_http::Response::from_data(b"not found".to_vec()).with_status_code(404);
    }
    if path.starts_with("/api/3.0/index.n") {
        // The remoting query serializes the library name inside `__x`; the stub just
        // matches on the name appearing anywhere in the (unreserved-char) query.
        for (n, v) in libs {
            if url.contains(n.as_str()) {
                let body = format!("hxry{}:{}", v.len(), v);
                return tiny_http::Response::from_data(body.into_bytes());
            }
        }
        let msg = "No%20such%20Project";
        let body = format!("hxry{}:{}", msg.len(), msg);
        return tiny_http::Response::from_data(body.into_bytes());
    }
    tiny_http::Response::from_data(b"bad request".to_vec()).with_status_code(404)
}

/// Builds an in-memory zip shaped like a haxelib release archive.
fn build_lib_zip(name: &str, version: &str) -> Vec<u8> {
    use std::io::Write as _;
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut cursor);
        let opts = zip::write::SimpleFileOptions::default();
        z.start_file("haxelib.json", opts).unwrap();
        write!(z, r#"{{"name":"{name}","version":"{version}"}}"#).unwrap();
        z.start_file("Main.hx", opts).unwrap();
        write!(z, "class Main {{}}").unwrap();
        z.finish().unwrap();
    }
    cursor.into_inner()
}
