use std::path::PathBuf;

use assert_fs::prelude::*;
use hmm_rs::commands::*;
use hmm_rs::hmm;
use hmm_rs::hmm::haxelib::HaxelibType;
use predicates::prelude::*;

mod common;

#[test]
fn test_clean_haxelib_folder() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let haxelib_dir = tmp.path().join(".haxelib");
    std::fs::create_dir(&haxelib_dir).unwrap();

    // Should succeed when .haxelib exists
    assert!(clean_command::remove_haxelib_folder_at(tmp.path()).is_ok());
    tmp.child(".haxelib").assert(predicate::path::missing());

    // Should fail when .haxelib is already gone
    assert!(clean_command::remove_haxelib_folder_at(tmp.path()).is_err());
}

#[test]
fn test_create_haxelib_folder() {
    let tmp = assert_fs::TempDir::new().unwrap();

    // Should succeed when .haxelib doesn't exist
    assert!(init_command::create_haxelib_folder_at(tmp.path()).is_ok());
    tmp.child(".haxelib").assert(predicate::path::is_dir());

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

#[test]
fn test_hmm_json_read_dev() {
    let dev_json = PathBuf::new().join(common::get_samples_dir()).join("dev.json");
    let deps = hmm::json::read_json(&dev_json).unwrap();

    assert_eq!(deps.dependencies.len(), 1);
    let dep = &deps.dependencies[0];
    assert_eq!(dep.name, "mylocal");
    assert_eq!(dep.haxelib_type, HaxelibType::Dev);
    assert_eq!(dep.path.as_deref(), Some("../mylocal"));
}

#[test]
fn test_hmm_json_read_git_subdir() {
    let subdir_json = PathBuf::new()
        .join(common::get_samples_dir())
        .join("git_subdir.json");
    let deps = hmm::json::read_json(&subdir_json).unwrap();

    assert_eq!(deps.dependencies.len(), 1);
    let dep = &deps.dependencies[0];
    assert_eq!(dep.name, "funkin.vis");
    assert_eq!(dep.haxelib_type, HaxelibType::Git);
    assert_eq!(dep.dir.as_deref(), Some("src"));
    assert_eq!(dep.vcs_ref.as_deref(), Some("main"));
}

#[test]
fn test_hmm_json_malformed_errors() {
    let malformed_json = PathBuf::new()
        .join(common::get_samples_dir())
        .join("malformed.json");
    assert!(hmm::json::read_json(&malformed_json).is_err());
}

/// Compatibility with original hmm: empty and whitespace-only `version`/`ref`/`dir`
/// strings must deserialize to `None`, not `Some("")` — mirrors the normalization
/// table in hmm's TestHmmConfig.testDeserialize_Success
/// (`LibraryConfigs.parseOptionalStringProperty`).
#[test]
fn test_blank_strings_normalize_to_none() {
    let json = r#"{
        "dependencies": [
            {"name": "lib2", "type": "haxelib", "version": ""},
            {"name": "lib3", "type": "haxelib", "version": " "},
            {"name": "lib4", "type": "haxelib", "version": "1.0.0"},
            {"name": "lib8", "type": "git", "url": "url8", "ref": "", "dir": ""},
            {"name": "lib9", "type": "git", "url": "url9", "ref": "  ", "dir": "  "},
            {"name": "lib7", "type": "git", "url": "url7", "ref": "ref7", "dir": "dir7"}
        ]
    }"#;
    let deps: hmm::dependencies::Dependancies = serde_json::from_str(json).unwrap();

    assert_eq!(deps.dependencies[0].version, None, "empty version -> None");
    assert_eq!(deps.dependencies[1].version, None, "blank version -> None");
    assert_eq!(deps.dependencies[2].version.as_deref(), Some("1.0.0"));
    assert_eq!(deps.dependencies[3].vcs_ref, None, "empty ref -> None");
    assert_eq!(deps.dependencies[3].dir, None, "empty dir -> None");
    assert_eq!(deps.dependencies[4].vcs_ref, None, "blank ref -> None");
    assert_eq!(deps.dependencies[4].dir, None, "blank dir -> None");
    assert_eq!(deps.dependencies[5].vcs_ref.as_deref(), Some("ref7"));
    assert_eq!(deps.dependencies[5].dir.as_deref(), Some("dir7"));
}

fn haxelib_dep(name: &str, version: &str) -> hmm_rs::hmm::haxelib::Haxelib {
    hmm_rs::hmm::haxelib::Haxelib {
        name: name.to_string(),
        haxelib_type: HaxelibType::Haxelib,
        dir: None,
        vcs_ref: None,
        path: None,
        url: None,
        version: Some(version.to_string()),
    }
}

#[test]
fn test_save_json_preserves_order() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let json_path = tmp.path().join("hmm.json");

    let deps = hmm::dependencies::Dependancies {
        dependencies: vec![
            haxelib_dep("zeta", "1.0.0"),
            haxelib_dep("Alpha", "2.0.0"),
            haxelib_dep("beta", "3.0.0"),
        ],
    };
    hmm::json::save_json(deps, json_path.clone()).unwrap();

    let read_back = hmm::json::read_json(&json_path).unwrap();
    let names: Vec<&str> = read_back.dependencies.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, vec!["zeta", "Alpha", "beta"]);
    let text = std::fs::read_to_string(&json_path).unwrap();
    assert!(text.ends_with("}\n"), "save_json should end with a newline");
    assert!(!text.contains("\"dir\""), "dir: None must be omitted");
}

// --- upsert_dependencies / remove_dependencies ---
//
// These edit hmm.json in place, so the fixture deliberately has everything a
// whole-file rewrite would normalize: 4-space indent, unsorted names, an entry
// with `url` before `ref`, an unknown key, and a trailing newline. Assertions
// compare exact text.

const SURGICAL_FIXTURE: &str = "{
    \"dependencies\": [
        {
            \"name\": \"zeta\",
            \"type\": \"haxelib\",
            \"version\": \"1.0.0\",
            \"_comment\": \"keep me\"
        },
        {
            \"name\": \"alpha\",
            \"type\": \"git\",
            \"url\": \"https://example.com/alpha\",
            \"ref\": \"main\"
        }
    ]
}
";

fn write_fixture(content: &str) -> (assert_fs::TempDir, PathBuf) {
    let tmp = assert_fs::TempDir::new().unwrap();
    let json_path = tmp.path().join("hmm.json");
    std::fs::write(&json_path, content).unwrap();
    (tmp, json_path)
}

fn git_dep(name: &str, url: &str, vcs_ref: &str, dir: Option<&str>) -> hmm_rs::hmm::haxelib::Haxelib {
    hmm_rs::hmm::haxelib::Haxelib {
        name: name.to_string(),
        haxelib_type: HaxelibType::Git,
        dir: dir.map(str::to_string),
        vcs_ref: Some(vcs_ref.to_string()),
        path: None,
        url: Some(url.to_string()),
        version: None,
    }
}

#[test]
fn test_upsert_appends_new_entry_and_touches_nothing_else() {
    let (_tmp, json_path) = write_fixture(SURGICAL_FIXTURE);

    hmm::json::upsert_dependencies(&json_path, &[haxelib_dep("mid", "2.0.0")]).unwrap();

    let expected = SURGICAL_FIXTURE.replace(
        "        }
    ]
}
",
        "        },
        {
            \"name\": \"mid\",
            \"type\": \"haxelib\",
            \"version\": \"2.0.0\"
        }
    ]
}
",
    );
    assert_eq!(std::fs::read_to_string(&json_path).unwrap(), expected);
}

#[test]
fn test_upsert_updates_existing_entry_in_place() {
    // Lock-style change: only `ref` differs. `url` must stay before `ref`, the
    // entry must keep its position, and the unknown `_comment` key survives.
    let (_tmp, json_path) = write_fixture(SURGICAL_FIXTURE);

    let lib = git_dep("alpha", "https://example.com/alpha", "abc123", None);
    hmm::json::upsert_dependencies(&json_path, &[lib]).unwrap();

    let expected = SURGICAL_FIXTURE.replace("\"ref\": \"main\"", "\"ref\": \"abc123\"");
    assert_ne!(expected, SURGICAL_FIXTURE);
    assert_eq!(std::fs::read_to_string(&json_path).unwrap(), expected);
}

#[test]
fn test_upsert_type_change_drops_stale_keys_and_keeps_position() {
    let (_tmp, json_path) = write_fixture(SURGICAL_FIXTURE);

    let lib = git_dep("zeta", "https://example.com/zeta", "v2", None);
    hmm::json::upsert_dependencies(&json_path, &[lib]).unwrap();

    let expected = SURGICAL_FIXTURE.replace(
        "            \"type\": \"haxelib\",
            \"version\": \"1.0.0\",
            \"_comment\": \"keep me\"
",
        "            \"type\": \"git\",
            \"_comment\": \"keep me\",
            \"ref\": \"v2\",
            \"url\": \"https://example.com/zeta\"
",
    );
    assert_ne!(expected, SURGICAL_FIXTURE);
    assert_eq!(std::fs::read_to_string(&json_path).unwrap(), expected);
}

#[test]
fn test_upsert_omits_dir_when_none_and_writes_it_when_set() {
    let (_tmp, json_path) = write_fixture(SURGICAL_FIXTURE);

    let plain = git_dep("plain", "https://example.com/plain", "main", None);
    let subdir = git_dep("subdir", "https://example.com/subdir", "main", Some("src"));
    hmm::json::upsert_dependencies(&json_path, &[plain, subdir]).unwrap();

    let text = std::fs::read_to_string(&json_path).unwrap();
    assert!(!text.contains("\"dir\": null"), "got: {text}");
    assert_eq!(text.matches("\"dir\": \"src\"").count(), 1, "got: {text}");
}

#[test]
fn test_upsert_preserves_missing_trailing_newline() {
    let no_newline = SURGICAL_FIXTURE.trim_end_matches('\n');
    let (_tmp, json_path) = write_fixture(no_newline);

    hmm::json::upsert_dependencies(&json_path, &[haxelib_dep("mid", "2.0.0")]).unwrap();

    let text = std::fs::read_to_string(&json_path).unwrap();
    assert!(text.ends_with('}'), "got trailing: {:?}", &text[text.len() - 4..]);
    assert!(text.contains("\"name\": \"mid\""));
}

#[test]
fn test_remove_dependencies_drops_only_named_entries() {
    let (_tmp, json_path) = write_fixture(SURGICAL_FIXTURE);

    hmm::json::remove_dependencies(&json_path, &["zeta".to_string()]).unwrap();

    let expected = SURGICAL_FIXTURE.replace(
        "        {
            \"name\": \"zeta\",
            \"type\": \"haxelib\",
            \"version\": \"1.0.0\",
            \"_comment\": \"keep me\"
        },
",
        "",
    );
    assert_ne!(expected, SURGICAL_FIXTURE);
    assert_eq!(std::fs::read_to_string(&json_path).unwrap(), expected);
}

#[test]
fn test_save_json_read_json_round_trip() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let json_path = tmp.path().join("hmm.json");

    let git_dep = hmm_rs::hmm::haxelib::Haxelib {
        name: "gitlib".to_string(),
        haxelib_type: HaxelibType::Git,
        dir: Some("src".to_string()),
        vcs_ref: Some("main".to_string()),
        path: None,
        url: Some("https://example.com/repo.git".to_string()),
        version: None,
    };
    let deps = hmm::dependencies::Dependancies {
        dependencies: vec![haxelib_dep("somelib", "1.2.3"), git_dep],
    };
    hmm::json::save_json(deps, json_path.clone()).unwrap();

    let read_back = hmm::json::read_json(&json_path).unwrap();
    assert_eq!(read_back.dependencies.len(), 2);
    let lib = &read_back.dependencies[0];
    assert_eq!(lib.name, "somelib");
    assert_eq!(lib.version.as_deref(), Some("1.2.3"));
    let git = &read_back.dependencies[1];
    assert_eq!(git.name, "gitlib");
    assert_eq!(git.haxelib_type, HaxelibType::Git);
    assert_eq!(git.dir.as_deref(), Some("src"));
    assert_eq!(git.vcs_ref.as_deref(), Some("main"));
    assert_eq!(git.url.as_deref(), Some("https://example.com/repo.git"));
}
