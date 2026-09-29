use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

use crate::common;

// All tests here are hermetic: haxelib-type deps download from a local
// `common::RegistryStub` (via HMM_HAXELIB_URL), git-type deps clone local
// repos over `file://`. Library names are unique per test because haxelib
// downloads land in the shared OS temp dir as `<name>.zip`.

/// Regression test: `install` used to panic with `unwrap()` on `None` in
/// `print_install_status()` when `.haxelib/` directory didn't exist.
/// See check_command.rs:234 — now uses `unwrap_or("unknown")`.
#[test]
fn install_does_not_panic_without_haxelib_dir() {
    let stub = common::RegistryStub::serve(&[("hermit-a", "1.0.0")]);
    let json = r#"{
        "dependencies": [
            {"name": "hermit-a", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("Creating .haxelib/ folder"));

    let current = std::fs::read_to_string(temp.child(".haxelib/hermit-a/.current").path()).unwrap();
    assert_eq!(current, "1.0.0");
    temp.child(".haxelib/hermit-a/1,0,0/haxelib.json")
        .assert(predicate::path::is_file());
}

/// Same regression scenario but with a git-type dependency.
#[test]
fn install_git_dep_does_not_panic_without_haxelib_dir() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "hermit-b", "type": "git", "ref": "main", "url": "{url}"}}
        ]
    }}"#
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("Creating .haxelib/ folder"));

    let current = std::fs::read_to_string(temp.child(".haxelib/hermit-b/.current").path()).unwrap();
    assert_eq!(current, "git");
    temp.child(".haxelib/hermit-b/git/README.md")
        .assert(predicate::path::is_file());
}

/// Multiple deps, none installed, no .haxelib — verifies iteration doesn't
/// panic on any dep and every dep really gets installed.
#[test]
fn install_multiple_deps_does_not_panic_without_haxelib_dir() {
    let stub = common::RegistryStub::serve(&[("hermit-c1", "1.0.0"), ("hermit-c2", "2.0.0")]);
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "hermit-c1", "type": "haxelib", "version": "1.0.0"}},
            {{"name": "hermit-c2", "type": "haxelib", "version": "2.0.0"}},
            {{"name": "hermit-c3", "type": "git", "ref": "main", "url": "{url}"}}
        ]
    }}"#
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("Creating .haxelib/ folder"));

    for (lib, expected) in [
        ("hermit-c1", "1.0.0"),
        ("hermit-c2", "2.0.0"),
        ("hermit-c3", "git"),
    ] {
        let current =
            std::fs::read_to_string(temp.child(format!(".haxelib/{lib}/.current")).path()).unwrap();
        assert_eq!(current, expected, "wrong .current for {lib}");
    }
}

#[test]
fn install_selective_single_lib() {
    let (_repo_a, repo_a) = common::local_git_repo_with_lib_subdir("mylib");
    let (_repo_b, repo_b) = common::local_git_repo_with_lib_subdir("mylib");
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "sel-a", "type": "git", "ref": "main", "url": "{}"}},
            {{"name": "sel-b", "type": "git", "ref": "main", "url": "{}"}}
        ]
    }}"#,
        common::file_url(&repo_a),
        common::file_url(&repo_b)
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["install", "sel-a"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sel-a"))
        .stdout(predicate::str::contains("sel-b").not());

    temp.child(".haxelib/sel-a/.current")
        .assert(predicate::path::is_file());
    temp.child(".haxelib/sel-b")
        .assert(predicate::path::exists().not());
}

#[test]
fn install_selective_multiple_libs() {
    let (_repo_a, repo_a) = common::local_git_repo_with_lib_subdir("mylib");
    let (_repo_b, repo_b) = common::local_git_repo_with_lib_subdir("mylib");
    let (_repo_c, repo_c) = common::local_git_repo_with_lib_subdir("mylib");
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "multi-a", "type": "git", "ref": "main", "url": "{}"}},
            {{"name": "multi-b", "type": "git", "ref": "main", "url": "{}"}},
            {{"name": "multi-c", "type": "git", "ref": "main", "url": "{}"}}
        ]
    }}"#,
        common::file_url(&repo_a),
        common::file_url(&repo_b),
        common::file_url(&repo_c)
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["install", "multi-a", "multi-b"])
        .assert()
        .success()
        .stdout(predicate::str::contains("multi-a"))
        .stdout(predicate::str::contains("multi-b"))
        .stdout(predicate::str::contains("multi-c").not());

    temp.child(".haxelib/multi-a/.current")
        .assert(predicate::path::is_file());
    temp.child(".haxelib/multi-b/.current")
        .assert(predicate::path::is_file());
    temp.child(".haxelib/multi-c")
        .assert(predicate::path::exists().not());
}

#[test]
fn install_unknown_lib_warns() {
    let json = r#"{
        "dependencies": [
            {"name": "known-a", "type": "haxelib", "version": "5.0.0"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["install", "nonexistent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not found in hmm.json"));
}

#[test]
fn install_mixed_known_and_unknown_libs() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "mix-a", "type": "git", "ref": "main", "url": "{url}"}},
            {{"name": "mix-b", "type": "git", "ref": "main", "url": "{url}"}}
        ]
    }}"#
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["install", "mix-a", "bogus"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not found in hmm.json"))
        .stdout(predicate::str::contains("mix-a"));

    temp.child(".haxelib/mix-a/.current")
        .assert(predicate::path::is_file());
    temp.child(".haxelib/mix-b")
        .assert(predicate::path::exists().not());
}

#[test]
fn install_selective_already_installed() {
    let json = r#"{
        "dependencies": [
            {"name": "done-a", "type": "haxelib", "version": "5.0.0"},
            {"name": "done-b", "type": "haxelib", "version": "8.0.0"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("done-a", "5.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["install", "done-a"])
        .assert()
        .success()
        // Quiet mode: already-installed libs produce no per-lib output.
        .stdout(predicate::str::contains("Checking done-a").not())
        .stdout(predicate::str::contains("is installed").not())
        // Non-selected lib should not be touched either.
        .stdout(predicate::str::contains("Checking done-b").not())
        .stdout(predicate::str::contains("done-b").not());
}

/// A git dep pinned to a nonexistent commit must not abort the run: the
/// remaining deps still install and a warning summary is printed at the end.
#[test]
fn install_continues_past_bad_git_ref() {
    let (_repo_a, repo_a_path) = common::local_git_repo_with_lib_subdir("liba");
    let (_repo_b, repo_b_path) = common::local_git_repo_with_lib_subdir("libb");
    let url_a = common::file_url(&repo_a_path);
    let url_b = common::file_url(&repo_b_path);
    let bogus_sha = "0123456789abcdef0123456789abcdef01234567";
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "contpast-a", "type": "git", "ref": "{bogus_sha}", "url": "{url_a}"}},
            {{"name": "contpast-b", "type": "git", "ref": "main", "url": "{url_b}"}}
        ]
    }}"#
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .failure()
        .stdout(predicate::str::contains("not found even after fetch"))
        .stdout(predicate::str::contains("failed to install"));

    // The good dep after the failing one still got installed
    let current =
        std::fs::read_to_string(temp.child(".haxelib/contpast-b/.current").path()).unwrap();
    assert_eq!(current, "git");
    temp.child(".haxelib/contpast-b/git/README.md")
        .assert(predicate::path::is_file());
}

/// Regression (H8): bumping a git dep's ref to a tag created upstream after
/// the clone aborted `install` before anything was fetched, and every other
/// dep was skipped with it. The tag (annotated here) is fetched and checked
/// out, and the missing dep listed before it still installs.
#[test]
fn install_fetches_tag_created_upstream_after_clone() {
    let (_repo_a, repo_a_path) = common::local_git_repo_with_lib_subdir("liba");
    let (_repo_b, repo_b_path) = common::local_git_repo_with_lib_subdir("libb");
    let url_a = common::file_url(&repo_a_path);
    let url_b = common::file_url(&repo_b_path);
    let temp = common::project_with_hmm_json(&format!(
        r#"{{
        "dependencies": [
            {{"name": "newtag-b", "type": "git", "ref": "main", "url": "{url_b}"}}
        ]
    }}"#
    ));
    let hmm = || {
        let mut cmd = Command::cargo_bin("hmm-rs").unwrap();
        cmd.current_dir(temp.path());
        cmd
    };
    hmm().arg("install").assert().success();

    std::fs::write(repo_b_path.join("second.txt"), "second\n").unwrap();
    common::run_git(&repo_b_path, &["add", "-A"]);
    common::run_git(&repo_b_path, &["commit", "-qm", "second"]);
    common::run_git(&repo_b_path, &["tag", "-a", "v2", "-m", "v2"]);
    std::fs::write(
        temp.child("hmm.json").path(),
        format!(
            r#"{{
        "dependencies": [
            {{"name": "newtag-a", "type": "git", "ref": "main", "url": "{url_a}"}},
            {{"name": "newtag-b", "type": "git", "ref": "v2", "url": "{url_b}"}}
        ]
    }}"#
        ),
    )
    .unwrap();

    hmm().arg("install").assert().success();
    temp.child(".haxelib/newtag-a/git/README.md")
        .assert(predicate::path::is_file());
    assert_eq!(
        common::git_rev_parse(&temp.path().join(".haxelib/newtag-b/git"), "HEAD"),
        common::git_rev_parse(&repo_b_path, "v2^{commit}")
    );
    hmm().arg("check").assert().success();
}

/// Regression (H8): the same for a branch that only exists upstream.
#[test]
fn install_checks_out_branch_created_upstream_after_clone() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let json = |r: &str| {
        format!(
            r#"{{
        "dependencies": [
            {{"name": "newbranch", "type": "git", "ref": "{r}", "url": "{url}"}}
        ]
    }}"#
        )
    };
    let temp = common::project_with_hmm_json(&json("main"));
    let hmm = || {
        let mut cmd = Command::cargo_bin("hmm-rs").unwrap();
        cmd.current_dir(temp.path());
        cmd
    };
    hmm().arg("install").assert().success();

    common::run_git(&repo_path, &["checkout", "-qb", "feature"]);
    std::fs::write(repo_path.join("feature.txt"), "feature\n").unwrap();
    common::run_git(&repo_path, &["add", "-A"]);
    common::run_git(&repo_path, &["commit", "-qm", "feature"]);
    std::fs::write(temp.child("hmm.json").path(), json("feature")).unwrap();

    hmm().arg("install").assert().success();
    assert_eq!(
        common::git_rev_parse(&temp.path().join(".haxelib/newbranch/git"), "HEAD"),
        common::git_rev_parse(&repo_path, "feature")
    );
    hmm().arg("check").assert().success();
}

/// A lib that can't be checked at all (here a cloned git dep with no `ref`)
/// fails on its own and the deps after it still install.
#[test]
fn install_continues_past_uncheckable_dep() {
    let (_repo_a, repo_a_path) = common::local_git_repo_with_lib_subdir("liba");
    let (_repo_b, repo_b_path) = common::local_git_repo_with_lib_subdir("libb");
    let url_a = common::file_url(&repo_a_path);
    let url_b = common::file_url(&repo_b_path);
    let temp = common::project_with_hmm_json(&format!(
        r#"{{
        "dependencies": [
            {{"name": "uncheck-a", "type": "git", "url": "{url_a}"}}
        ]
    }}"#
    ));
    let install = || {
        let mut cmd = Command::cargo_bin("hmm-rs").unwrap();
        cmd.current_dir(temp.path()).arg("install");
        cmd
    };
    install().assert().success();

    std::fs::write(
        temp.child("hmm.json").path(),
        format!(
            r#"{{
        "dependencies": [
            {{"name": "uncheck-a", "type": "git", "url": "{url_a}"}},
            {{"name": "uncheck-b", "type": "git", "ref": "main", "url": "{url_b}"}}
        ]
    }}"#
        ),
    )
    .unwrap();

    install()
        .assert()
        .failure()
        .stdout(predicate::str::contains("could not be checked"))
        .stdout(predicate::str::contains("'ref' field is required"))
        .stdout(predicate::str::contains("dependencies failed to install:"));
    temp.child(".haxelib/uncheck-b/git/README.md")
        .assert(predicate::path::is_file());
}

/// A haxelib dep whose download fails must not abort the run either.
#[test]
fn install_continues_past_failed_haxelib_download() {
    // Stub only serves contpast-d; contpast-c's download will 404
    let stub = common::RegistryStub::serve(&[("contpast-d", "1.0.0")]);
    let json = r#"{
        "dependencies": [
            {"name": "contpast-c", "type": "haxelib", "version": "1.0.0"},
            {"name": "contpast-d", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .arg("install")
        .assert()
        .failure()
        .stdout(predicate::str::contains("contpast-c"))
        .stdout(predicate::str::contains("failed to install"));

    // The good dep after the failing one still got installed
    let current =
        std::fs::read_to_string(temp.child(".haxelib/contpast-d/.current").path()).unwrap();
    assert_eq!(current, "1.0.0");
    temp.child(".haxelib/contpast-d/1,0,0/haxelib.json")
        .assert(predicate::path::is_file());
}

// --- dotted library names (`funkin.vis`-style) ---
// Every filesystem surface must use the comma-encoded directory
// (`instdot.vis` -> `.haxelib/instdot,vis`), matching haxelib's Data.safe.

#[test]
fn install_dotted_haxelib_uses_comma_dirs() {
    let stub = common::RegistryStub::serve(&[("instdot.vis", "1.0.0")]);
    let json = r#"{
        "dependencies": [
            {"name": "instdot.vis", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .arg("install")
        .assert()
        .success();

    // Comma-encoded name dir + comma-encoded version dir; .current keeps the
    // raw dotted version. This is the exact layout `haxelib path` resolves.
    let current =
        std::fs::read_to_string(temp.child(".haxelib/instdot,vis/.current").path()).unwrap();
    assert_eq!(current, "1.0.0");
    temp.child(".haxelib/instdot,vis/1,0,0/haxelib.json")
        .assert(predicate::path::is_file());
    temp.child(".haxelib/instdot.vis")
        .assert(predicate::path::missing());
}

#[test]
fn install_dotted_git_dep_uses_comma_dirs() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "instgitdot.vis", "type": "git", "ref": "main", "url": "{url}"}}
        ]
    }}"#
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success();

    let current =
        std::fs::read_to_string(temp.child(".haxelib/instgitdot,vis/.current").path()).unwrap();
    assert_eq!(current, "git");
    temp.child(".haxelib/instgitdot,vis/git/README.md")
        .assert(predicate::path::is_file());
}

#[test]
fn install_dotted_git_dep_with_dir_writes_dev_into_subdir() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "instdirdot.vis", "type": "git", "ref": "main", "url": "{url}", "dir": "mylib"}}
        ]
    }}"#
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success();

    let dev_file = temp.child(".haxelib/instdirdot,vis/.dev");
    dev_file.assert(predicate::path::is_file());
    let dev_content = std::fs::read_to_string(dev_file.path()).unwrap();
    assert!(
        dev_content
            .replace('\\', "/")
            .contains("instdirdot,vis/git/mylib"),
        "dev file should point into the comma-dir's git/mylib subdir, got: {dev_content}"
    );
}

/// Step-by-step git narration ("Checking out …", "✓ Checked out …") is
/// verbose-only; the default output is the lead line plus the result.
#[test]
fn install_git_narration_only_with_verbose() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("lib");
    let url = common::file_url(&repo_path);
    let json = format!(
        r#"{{"dependencies": [{{"name": "quiet-git", "type": "git", "ref": "main", "url": "{url}"}}]}}"#
    );
    let temp = common::project_with_hmm_json(&json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("Cloning quiet-git"))
        .stdout(predicate::str::contains("installed"))
        .stdout(predicate::str::contains("Checking out").not())
        .stdout(predicate::str::contains("Checked out").not())
        .stdout(predicate::str::contains("clone completed").not())
        .stdout(predicate::str::contains("Renaming remote").not());

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("clean")
        .assert()
        .success();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .args(["install", "-v"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Blobless clone completed"))
        .stdout(predicate::str::contains("Renaming remote origin"))
        .stdout(predicate::str::contains("Checking out quiet-git at main"))
        .stdout(predicate::str::contains("Checked out main (local)"));
}

/// The "done downloading" step line is verbose-only too.
#[test]
fn install_haxelib_narration_only_with_verbose() {
    let stub = common::RegistryStub::serve(&[("quiet-hx", "1.0.0")]);
    let json = r#"{"dependencies": [{"name": "quiet-hx", "type": "haxelib", "version": "1.0.0"}]}"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("Downloading: "))
        .stdout(predicate::str::contains("installed"))
        .stdout(predicate::str::contains("done downloading").not());

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("clean")
        .assert()
        .success();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .args(["install", "-v"])
        .assert()
        .success()
        .stdout(predicate::str::contains("done downloading"));
}

/// Regression: a lib that went git -> haxelib -> git kept the haxelib version
/// in `.current`. Installing the haxelib version leaves git/ in place, `check`
/// only inspected the checkout, and the git installer only wrote `.current`
/// on a fresh clone.
#[test]
fn install_rewrites_current_when_switching_back_to_git() {
    let stub = common::RegistryStub::serve(&[("switch-a", "1.0.0")]);
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let git_json = format!(
        r#"{{
        "dependencies": [
            {{"name": "switch-a", "type": "git", "ref": "main", "url": "{}"}}
        ]
    }}"#,
        common::file_url(&repo_path)
    );
    let haxelib_json = r#"{
        "dependencies": [
            {"name": "switch-a", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(&git_json);
    let current_path = temp.child(".haxelib/switch-a/.current");
    let install = || {
        let mut cmd = Command::cargo_bin("hmm-rs").unwrap();
        cmd.current_dir(temp.path())
            .env("HMM_HAXELIB_URL", &stub.base_url)
            .arg("install");
        cmd
    };

    install().assert().success();
    assert_eq!(std::fs::read_to_string(current_path.path()).unwrap(), "git");

    std::fs::write(temp.child("hmm.json").path(), haxelib_json).unwrap();
    install().assert().success();
    assert_eq!(
        std::fs::read_to_string(current_path.path()).unwrap(),
        "1.0.0"
    );
    temp.child(".haxelib/switch-a/git")
        .assert(predicate::path::is_dir());

    std::fs::write(temp.child("hmm.json").path(), &git_json).unwrap();
    install()
        .assert()
        .success()
        .stdout(predicate::str::contains("is not at the correct version"))
        .stdout(predicate::str::contains("Repository exists, checking out"));
    assert_eq!(std::fs::read_to_string(current_path.path()).unwrap(), "git");
}

/// Regression: switching a git dep that had a `dir` to a haxelib version
/// left its `.dev` marker behind, so `haxelib path` (and `check`) kept
/// resolving into the git subdir.
#[test]
fn install_haxelib_over_git_subdir_dep_clears_dev_link() {
    let stub = common::RegistryStub::serve(&[("switch-b", "1.0.0")]);
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let git_json = format!(
        r#"{{
        "dependencies": [
            {{"name": "switch-b", "type": "git", "ref": "main", "url": "{}", "dir": "mylib"}}
        ]
    }}"#,
        common::file_url(&repo_path)
    );
    let haxelib_json = r#"{
        "dependencies": [
            {"name": "switch-b", "type": "haxelib", "version": "1.0.0"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(&git_json);
    let dev_file = temp.child(".haxelib/switch-b/.dev");

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success();
    dev_file.assert(predicate::path::is_file());

    std::fs::write(temp.child("hmm.json").path(), haxelib_json).unwrap();
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("development directory unset"));
    dev_file.assert(predicate::path::missing());
    assert_eq!(
        std::fs::read_to_string(temp.child(".haxelib/switch-b/.current").path()).unwrap(),
        "1.0.0"
    );

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "dependencie(s) are installed at the correct versions",
        ))
        .stdout(predicate::str::contains("is not at the correct version").not());
}

/// Regression: dropping `dir` from a git dep left the `.dev` marker from the
/// earlier subdir install, so `haxelib path` still resolved into the subdir.
#[test]
fn install_git_dep_dropping_dir_clears_dev_link() {
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let with_dir = format!(
        r#"{{
        "dependencies": [
            {{"name": "switch-c", "type": "git", "ref": "main", "url": "{url}", "dir": "mylib"}}
        ]
    }}"#
    );
    let without_dir = format!(
        r#"{{
        "dependencies": [
            {{"name": "switch-c", "type": "git", "ref": "main", "url": "{url}"}}
        ]
    }}"#
    );
    let temp = common::project_with_hmm_json(&with_dir);
    let dev_file = temp.child(".haxelib/switch-c/.dev");

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success();
    dev_file.assert(predicate::path::is_file());

    std::fs::write(temp.child("hmm.json").path(), without_dir).unwrap();
    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("development directory unset"));
    dev_file.assert(predicate::path::missing());
}

// --- dev deps ---

/// Regression: dev entries printed "Installing from Dev not yet implemented"
/// and left no `.dev` marker behind.
#[test]
fn install_dev_dep_writes_dev_marker() {
    let json = r#"{
        "dependencies": [
            {"name": "devinst-a", "type": "dev", "path": "devsrc"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);
    temp.child("devsrc").create_dir_all().unwrap();
    let target = temp.child("devsrc").path().canonicalize().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("development directory set to"))
        .stdout(predicate::str::contains("not yet implemented").not());

    let dev = std::fs::read_to_string(temp.child(".haxelib/devinst-a/.dev").path()).unwrap();
    assert_eq!(dev, target.to_str().unwrap());

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .success();
}

/// A dev dep whose `path` changed in hmm.json, or that replaced a haxelib
/// install of the same lib, gets its `.dev` marker (re)written.
#[test]
fn install_dev_dep_repoints_stale_marker_and_haxelib_install() {
    let json = r#"{
        "dependencies": [
            {"name": "devinst-b", "type": "dev", "path": "devsrc"},
            {"name": "devinst-c", "type": "dev", "path": "devsrc"}
        ]
    }"#;
    let temp = common::project_with_installed_haxelibs(json, &[("devinst-c", "1.0.0")]);
    temp.child("devsrc").create_dir_all().unwrap();
    temp.child(".haxelib/devinst-b/.dev")
        .write_str("/somewhere/else")
        .unwrap();
    let target = temp.child("devsrc").path().canonicalize().unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success();

    for name in ["devinst-b", "devinst-c"] {
        let dev_file = temp.child(format!(".haxelib/{name}/.dev"));
        let dev = std::fs::read_to_string(dev_file.path()).unwrap();
        assert_eq!(dev, target.to_str().unwrap(), "{name}");
    }
}

#[test]
fn install_dev_dep_with_missing_target_fails() {
    let json = r#"{
        "dependencies": [
            {"name": "devinst-d", "type": "dev", "path": "nope"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .failure()
        .stdout(predicate::str::contains(r#"dev path "nope" not found"#));
    temp.child(".haxelib/devinst-d/.dev")
        .assert(predicate::path::missing());
}

// --- version-less haxelib deps ---

/// Regression: a haxelib dep with no version failed with "version required".
/// It now installs the registry's latest and pins it, like `hmm-rs haxelib <name>`.
#[test]
fn install_versionless_haxelib_installs_latest_and_pins_it() {
    let stub = common::RegistryStub::serve(&[("latest-a", "2.0.0")]);
    let json = r#"{
        "dependencies": [
            {"name": "latest-a", "type": "haxelib", "version": null}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Latest version of latest-a is 2.0.0",
        ));

    let current = std::fs::read_to_string(temp.child(".haxelib/latest-a/.current").path()).unwrap();
    assert_eq!(current, "2.0.0");
    let hmm_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(temp.child("hmm.json").path()).unwrap())
            .unwrap();
    assert_eq!(hmm_json["dependencies"][0]["version"], "2.0.0");
}

/// Regression: a version-less entry was pinned to the registry's
/// `getLatestVersion`, which returns a newer prerelease over an older stable.
#[test]
fn install_versionless_haxelib_pins_newest_stable_over_newer_prerelease() {
    let stub = common::RegistryStub::serve(&[
        ("latest-pre", "1.0.0"),
        ("latest-pre", "2.0.0"),
        ("latest-pre", "3.0.0-rc.1"),
    ]);
    let json = r#"{"dependencies":[{"name":"latest-pre","type":"haxelib","version":null}]}"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .arg("install")
        .assert()
        .success();

    let current =
        std::fs::read_to_string(temp.child(".haxelib/latest-pre/.current").path()).unwrap();
    assert_eq!(current, "2.0.0");
    let hmm_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(temp.child("hmm.json").path()).unwrap())
            .unwrap();
    assert_eq!(hmm_json["dependencies"][0]["version"], "2.0.0");
}

/// An unpinned lib that is already installed is left alone: no registry
/// query, no reinstall, no hmm.json change.
#[test]
fn install_leaves_installed_versionless_haxelib_alone() {
    // The stub knows no libs, so any registry query would fail the install.
    let stub = common::RegistryStub::serve(&[]);
    let json = r#"{"dependencies":[{"name":"latest-b","type":"haxelib","version":null}]}"#;
    let temp = common::project_with_installed_haxelibs(json, &[("latest-b", "1.0.0")]);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("Not implemented").not());

    let current = std::fs::read_to_string(temp.child(".haxelib/latest-b/.current").path()).unwrap();
    assert_eq!(current, "1.0.0");
    let hmm_json = std::fs::read_to_string(temp.child("hmm.json").path()).unwrap();
    assert_eq!(hmm_json, json);
}

// `.haxelib/.repo-version` is haxelib 4.2.0's repository format marker. A
// missing marker makes every haxelib command nag about `haxelib fixrepo`, and
// hmm-rs's layout (lowercase dirs plus `.name`) is already format 1, so
// hmm-rs writes the marker itself. No deps are declared in these tests, so
// `install` never touches the network.

#[test]
fn install_backfills_repo_version_marker_into_existing_haxelib_dir() {
    let temp = common::initialized_project();
    temp.child(".haxelib/.repo-version")
        .assert(predicate::path::missing());

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success();

    let marker = std::fs::read_to_string(temp.child(".haxelib/.repo-version").path()).unwrap();
    assert_eq!(marker, "1\n");
}

#[test]
fn install_leaves_current_repo_version_marker_untouched() {
    let temp = common::initialized_project();
    // Deliberately not byte-identical to what hmm-rs writes: same version,
    // different whitespace. It must survive untouched.
    temp.child(".haxelib/.repo-version")
        .write_str(" 1 \n")
        .unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success()
        .stderr(predicate::str::contains("newer").not());

    let marker = std::fs::read_to_string(temp.child(".haxelib/.repo-version").path()).unwrap();
    assert_eq!(marker, " 1 \n");
}

#[test]
fn install_warns_on_newer_repo_version_marker_and_keeps_it() {
    let temp = common::initialized_project();
    temp.child(".haxelib/.repo-version")
        .write_str("2\n")
        .unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            ".repo-version is 2, newer than the 1",
        ));

    let marker = std::fs::read_to_string(temp.child(".haxelib/.repo-version").path()).unwrap();
    assert_eq!(marker, "2\n");
}

// Mixed-case names: haxelib 4.2.0 only looks up lowercased lib and version
// dirs, haxelib 4.1.1 (bundled with haxe) only the exact case. hmm-rs stores
// the lowercase form and, on a case-sensitive filesystem, links the exact case
// to it; elsewhere the filesystem aliases the two by itself.

#[test]
fn install_mixed_case_haxelib_resolves_in_both_cases() {
    let stub = common::RegistryStub::serve(&[("CaseMix", "1.0.0-RC.1")]);
    let json = r#"{
        "dependencies": [
            {"name": "CaseMix", "type": "haxelib", "version": "1.0.0-RC.1"}
        ]
    }"#;
    let temp = common::project_with_hmm_json(json);

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .env("HMM_HAXELIB_URL", &stub.base_url)
        .arg("install")
        .assert()
        .success();

    let repo = temp.path().join(".haxelib");
    let lib = repo.join("casemix");
    // 4.2.0 lookups and 4.1.1 lookups both land on the extracted files.
    assert!(lib.join("1,0,0-rc,1/haxelib.json").is_file());
    assert!(repo.join("CaseMix/1,0,0-RC,1/haxelib.json").is_file());
    assert_eq!(
        std::fs::read_to_string(lib.join(".name")).unwrap(),
        "CaseMix"
    );
    assert_eq!(
        std::fs::read_to_string(lib.join(".current")).unwrap(),
        "1.0.0-RC.1"
    );

    if common::is_case_sensitive(&repo) {
        assert_eq!(
            common::stored_names(&repo),
            [".repo-version", "CaseMix", "casemix"]
        );
        assert_eq!(
            std::fs::read_link(repo.join("CaseMix")).unwrap().to_str(),
            Some("casemix")
        );
        assert_eq!(
            std::fs::read_link(lib.join("1,0,0-RC,1")).unwrap().to_str(),
            Some("1,0,0-rc,1")
        );
    } else {
        assert_eq!(common::stored_names(&repo), [".repo-version", "casemix"]);
        assert!(common::stored_names(&lib).contains(&"1,0,0-rc,1".to_string()));
    }

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .success();
}

#[test]
fn install_moves_exact_case_git_clone_to_lowercase_dir() {
    // A clone left by hmm-rs's old exact-case layout, with a local file that
    // a fresh clone would not have. No `.current`, so `install` has work to do
    // on every filesystem.
    let (_repo, repo_path) = common::local_git_repo_with_lib_subdir("mylib");
    let url = common::file_url(&repo_path);
    let json = format!(
        r#"{{
        "dependencies": [
            {{"name": "GitMix", "type": "git", "ref": "main", "url": "{url}"}}
        ]
    }}"#
    );
    let temp = common::project_with_hmm_json(&json);
    let status = std::process::Command::new("git")
        .args(["clone", "-q", &url, ".haxelib/GitMix/git"])
        .current_dir(temp.path())
        .status()
        .unwrap();
    assert!(status.success());
    temp.child(".haxelib/GitMix/git/local-change")
        .write_str("x")
        .unwrap();

    Command::cargo_bin("hmm-rs")
        .unwrap()
        .current_dir(temp.path())
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("Repository exists"));

    let repo = temp.path().join(".haxelib");
    assert!(repo.join("gitmix/git/local-change").is_file());
    assert!(repo.join("GitMix/git/local-change").is_file());
    assert_eq!(
        std::fs::read_to_string(repo.join("gitmix/.name")).unwrap(),
        "GitMix"
    );
    assert!(common::stored_names(&repo).contains(&"gitmix".to_string()));
    if common::is_case_sensitive(&repo) {
        assert!(repo.join("GitMix").symlink_metadata().unwrap().is_symlink());
    }
}
