use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Haxelib {
    pub name: String,
    #[serde(rename = "type")]
    pub haxelib_type: HaxelibType,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, deserialize_with = "de_blank_as_none")]
    pub dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "ref")]
    #[serde(default, deserialize_with = "de_blank_as_none")]
    pub vcs_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, deserialize_with = "de_blank_as_none")]
    pub version: Option<String>,
}

/// Normalizes empty or whitespace-only strings to `None` when reading hmm.json,
/// matching original hmm's `parseOptionalStringProperty` behavior for
/// `version`/`ref`/`dir`.
fn de_blank_as_none<'de, D>(deserializer: D) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Option::<String>::deserialize(deserializer)?;
    std::result::Result::Ok(v.filter(|s| !s.trim().is_empty()))
}

/// Base URL of the haxelib registry. Overridable via `HMM_HAXELIB_URL` (used by
/// tests to point at a local stub server).
pub fn registry_base_url() -> String {
    std::env::var("HMM_HAXELIB_URL").unwrap_or_else(|_| "https://lib.haxe.org".to_string())
}

impl Haxelib {
    pub fn version(&self) -> Result<&str> {
        self.version.as_deref().ok_or_else(|| {
            anyhow!(
                "{}: 'version' field is required for haxelib type",
                self.name
            )
        })
    }

    pub fn vcs_ref(&self) -> Result<&str> {
        self.vcs_ref
            .as_deref()
            .ok_or_else(|| anyhow!("{}: 'ref' field is required for git type", self.name))
    }

    pub fn url(&self) -> Result<&str> {
        self.url
            .as_deref()
            .ok_or_else(|| anyhow!("{}: 'url' field is required", self.name))
    }

    pub fn try_version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    pub fn try_vcs_ref(&self) -> Option<&str> {
        self.vcs_ref.as_deref()
    }

    pub fn try_url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub fn download_url(&self) -> Result<String> {
        match self.haxelib_type {
            HaxelibType::Haxelib => {
                let version = self.try_version().ok_or_else(|| {
                    anyhow!(
                        "{}: version required for
  Haxelib",
                        self.name
                    )
                })?;
                Ok(format!(
                    "{}/p/{}/{}/download",
                    registry_base_url(),
                    self.name,
                    version
                ))
            }
            HaxelibType::Git => {
                let url = self
                    .try_url()
                    .ok_or_else(|| anyhow!("{}: url required for Git", self.name))?;
                Ok(url.to_string())
            }
            _ => Err(anyhow!(
                "{}: cannot generate download URL for {:?}",
                self.name,
                self.haxelib_type
            )),
        }
    }

    pub fn version_or_ref(&self) -> Result<&str> {
        match self.haxelib_type {
            HaxelibType::Haxelib => self
                .try_version()
                .ok_or_else(|| anyhow!("{}: Haxelib requires version", self.name)),
            HaxelibType::Git => self
                .try_vcs_ref()
                .ok_or_else(|| anyhow!("{}: Git requires vcs_ref", self.name)),
            _ => Err(anyhow!(
                "{}: Unsupported type {:?}",
                self.name,
                self.haxelib_type
            )),
        }
    }

    pub fn version_as_commas(&self) -> Result<String> {
        Ok(self.version()?.replace(".", ","))
    }

    /// Returns the library directory path: .haxelib/{name_with_commas, lowercased}
    pub fn lib_dir_path(&self) -> PathBuf {
        lib_dir_path_for_name(&self.name)
    }

    /// Returns the git repo path: .haxelib/{name_with_commas, lowercased}/git
    pub fn git_repo_path(&self) -> PathBuf {
        self.lib_dir_path().join("git")
    }
}

/// Returns the library directory path given a library name: dots encoded as
/// commas and lowercased, the only form haxelib 4.2.0 looks up
/// (`Repository.addToRepoPath`). `ensure_lib_dir` makes the hmm.json-case
/// name resolve here too, for haxelib 4.1.1.
pub fn lib_dir_path_for_name(name: &str) -> PathBuf {
    exact_case_lib_dir_path_for_name(&name.to_ascii_lowercase())
}

/// `.haxelib/<name>` with dots encoded as commas, in hmm.json case: where
/// haxelib 4.1.1 looks (`rep + Data.safe(name)`).
pub fn exact_case_lib_dir_path_for_name(name: &str) -> PathBuf {
    PathBuf::from(".haxelib").join(name.replace(".", ","))
}

/// Creates the library directory for `name` so that both haxelib 4.1.1 and
/// 4.2.0 resolve it, writes `.name` the way 4.2.0's `setCapitalization` does
/// (only for a name with capitals, so `haxelib list` shows it in hmm.json
/// case), and returns `lib_dir_path_for_name(name)`.
pub fn ensure_lib_dir(name: &str) -> Result<PathBuf> {
    let dir = ensure_case_aliased_dir(Path::new(".haxelib"), &name.replace(".", ","))?;
    let name_file = dir.join(".name");
    if name != name.to_ascii_lowercase() {
        fs::write(&name_file, name)?;
    } else if name_file.exists() {
        fs::remove_file(&name_file)?;
    }
    Ok(dir)
}

/// Creates `parent/<dir_name lowercased>`, the only form haxelib 4.2.0 looks
/// up, and makes `parent/<dir_name>` resolve to it for haxelib 4.1.1, which
/// looks up the exact case. On a case-insensitive filesystem the two names
/// already alias; on a case-sensitive one that takes a relative symlink.
/// Returns the lowercased path.
pub fn ensure_case_aliased_dir(parent: &Path, dir_name: &str) -> Result<PathBuf> {
    let lower_name = dir_name.to_ascii_lowercase();
    let lower = parent.join(&lower_name);
    if lower_name == dir_name {
        fs::create_dir_all(&lower)?;
        return Ok(lower);
    }

    // A real exact-case dir from before hmm-rs lowercased is renamed rather
    // than installed beside, so a git clone and its local changes survive.
    // Only a listing shows the stored case on a case-insensitive filesystem,
    // where this is a case-only rename.
    let exact = parent.join(dir_name);
    let (mut legacy, mut has_lower) = (false, false);
    for entry in fs::read_dir(parent).into_iter().flatten().flatten() {
        if entry.file_name() == dir_name {
            legacy = entry.file_type()?.is_dir();
        } else if entry.file_name() == lower_name.as_str() {
            has_lower = true;
        }
    }
    if legacy && !has_lower {
        fs::rename(&exact, &lower)
            .with_context(|| format!("Failed to rename {} to lowercase", exact.display()))?;
    }
    fs::create_dir_all(&lower)?;

    if !exact.exists() {
        #[cfg(unix)]
        std::os::unix::fs::symlink(&lower_name, &exact)
            .with_context(|| format!("Failed to link {} to {lower_name}", exact.display()))?;
    }
    Ok(lower)
}

/// Enforces the haxelib name charset: `A-Z a-z 0-9 _ . -` (the allowlist
/// `Data.safe` in the real haxelib client validates before dot-to-comma
/// encoding). Anything outside it can never work with the real toolchain
/// (`haxelib path` throws "Invalid parameter"), so rejecting here loses
/// nothing and guarantees:
/// - `lib_dir_path_for_name` is injective up to ASCII case (commas are
///   rejected, so `a,b` can no longer alias `a.b`; case variants share a
///   directory, as haxelib treats them as one library) and always resolves
///   to a single directory inside `.haxelib/` (no separators, and `..`
///   encodes to `,,`),
/// - names are safe in the compiler's unquoted `haxelib path <names>`
///   shell-out and byte-length-correct in the remoting serialization.
///
/// Deliberately omitted from haxelib's `ProjectName` rules: min length 3,
/// reserved names (`haxe`, `all`) and `.zip`/`.hxml` suffixes — those are
/// registry-publishing rules, and short git/dev names work fine with the
/// bundled haxelib client.
pub fn validate_lib_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(anyhow!("invalid library name: empty"));
    }
    if let Some(c) = name
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')))
    {
        return Err(anyhow!(
            "invalid library name '{}': character {:?} is not allowed (haxelib names may only contain A-Z a-z 0-9 _ . -)",
            name.escape_debug(),
            c
        ));
    }
    Ok(())
}

/// Returns the git repo path given a library name
pub fn git_repo_path_for_name(name: &str) -> PathBuf {
    lib_dir_path_for_name(name).join("git")
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum HaxelibType {
    #[serde(rename = "git")]
    Git,
    #[serde(rename = "haxelib")]
    Haxelib,
    #[serde(rename = "dev")]
    Dev,
    #[serde(rename = "hg")]
    Mecurial,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_haxelib(
        name: &str,
        haxelib_type: HaxelibType,
        version: Option<&str>,
        vcs_ref: Option<&str>,
        url: Option<&str>,
    ) -> Haxelib {
        Haxelib {
            name: name.to_string(),
            haxelib_type,
            dir: None,
            vcs_ref: vcs_ref.map(|s| s.to_string()),
            path: None,
            url: url.map(|s| s.to_string()),
            version: version.map(|s| s.to_string()),
        }
    }

    // --- version_as_commas ---

    #[test]
    fn test_version_as_commas() {
        let h = make_haxelib("flixel", HaxelibType::Haxelib, Some("3.3.0"), None, None);
        assert_eq!(h.version_as_commas().unwrap(), "3,3,0");
    }

    #[test]
    fn test_version_as_commas_prerelease() {
        // Pre-release dots are encoded too: haxelib's Data.safe applies to the
        // whole version string (e.g. dir `1,0,0-alpha,1`).
        let h = make_haxelib(
            "flixel",
            HaxelibType::Haxelib,
            Some("1.0.0-alpha.1"),
            None,
            None,
        );
        assert_eq!(h.version_as_commas().unwrap(), "1,0,0-alpha,1");
    }

    // --- path construction ---

    #[test]
    fn test_lib_dir_path() {
        let h = make_haxelib("funkin.vis", HaxelibType::Git, None, None, None);
        assert_eq!(h.lib_dir_path(), PathBuf::from(".haxelib/funkin,vis"));
    }

    #[test]
    fn test_git_repo_path() {
        let h = make_haxelib("flixel", HaxelibType::Git, None, None, None);
        assert_eq!(h.git_repo_path(), PathBuf::from(".haxelib/flixel/git"));
    }

    #[test]
    fn test_lib_dir_path_for_name() {
        assert_eq!(
            lib_dir_path_for_name("funkin.vis"),
            PathBuf::from(".haxelib/funkin,vis")
        );
    }

    #[test]
    fn test_lib_dir_path_for_name_lowercases() {
        // haxelib 4.2.0 only looks up `Data.safe(name).toLowerCase()`.
        assert_eq!(
            lib_dir_path_for_name("FlxPartial.Sound"),
            PathBuf::from(".haxelib/flxpartial,sound")
        );
        assert_eq!(
            exact_case_lib_dir_path_for_name("FlxPartial.Sound"),
            PathBuf::from(".haxelib/FlxPartial,Sound")
        );
    }

    // --- ensure_case_aliased_dir ---

    /// Whether `dir` is case-sensitive, probed the way the code under test
    /// sees it: does a case variant of an existing name resolve?
    fn case_sensitive(dir: &Path) -> bool {
        fs::create_dir(dir.join("probe")).unwrap();
        let sensitive = !dir.join("PROBE").exists();
        fs::remove_dir(dir.join("probe")).unwrap();
        sensitive
    }

    fn stored_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn test_ensure_case_aliased_dir_lowercase_name_is_plain_dir() {
        let temp = tempfile::tempdir().unwrap();
        let dir = ensure_case_aliased_dir(temp.path(), "flixel").unwrap();
        assert_eq!(dir, temp.path().join("flixel"));
        assert!(dir.is_dir());
        assert_eq!(stored_names(temp.path()), ["flixel"]);
    }

    #[test]
    fn test_ensure_case_aliased_dir_mixed_case_resolves_both_ways() {
        let temp = tempfile::tempdir().unwrap();
        let dir = ensure_case_aliased_dir(temp.path(), "FlxFoo").unwrap();
        assert_eq!(dir, temp.path().join("flxfoo"));
        assert!(temp.path().join("flxfoo").is_dir());
        assert!(temp.path().join("FlxFoo").is_dir());

        if case_sensitive(temp.path()) {
            assert_eq!(stored_names(temp.path()), ["FlxFoo", "flxfoo"]);
            assert_eq!(
                fs::read_link(temp.path().join("FlxFoo")).unwrap(),
                PathBuf::from("flxfoo")
            );
        } else {
            assert_eq!(stored_names(temp.path()), ["flxfoo"]);
        }

        // Idempotent: a second call neither fails on the existing link nor
        // adds anything.
        let before = stored_names(temp.path());
        ensure_case_aliased_dir(temp.path(), "FlxFoo").unwrap();
        assert_eq!(stored_names(temp.path()), before);
    }

    #[test]
    fn test_ensure_case_aliased_dir_migrates_exact_case_dir() {
        // A dir left by the old exact-case layout keeps its contents (e.g. a
        // git clone) and ends up stored lowercase.
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("FlxFoo/git")).unwrap();
        fs::write(temp.path().join("FlxFoo/git/local-change"), "x").unwrap();

        let dir = ensure_case_aliased_dir(temp.path(), "FlxFoo").unwrap();

        assert!(dir.join("git/local-change").is_file());
        assert!(temp.path().join("FlxFoo/git/local-change").is_file());
        assert!(stored_names(temp.path()).contains(&"flxfoo".to_string()));
        assert!(fs::symlink_metadata(temp.path().join("flxfoo"))
            .unwrap()
            .is_dir());
    }

    #[test]
    fn test_git_repo_path_for_name() {
        assert_eq!(
            git_repo_path_for_name("flixel"),
            PathBuf::from(".haxelib/flixel/git")
        );
    }

    // --- download_url ---

    #[test]
    fn test_download_url_haxelib() {
        let h = make_haxelib("flixel-addons", HaxelibType::Haxelib, Some("3.3.0"), None, None);
        assert_eq!(
            h.download_url().unwrap(),
            "https://lib.haxe.org/p/flixel-addons/3.3.0/download"
        );
    }

    #[test]
    fn test_download_url_haxelib_dotted_name_stays_raw() {
        // The /p/<name>/<version>/download website route takes the RAW dotted
        // name and version (it redirects to the comma-encoded
        // files/3.0/<safe(name)>-<safe(ver)>.zip itself). Do not "fix" this to
        // the comma form.
        let h = make_haxelib("funkin.vis", HaxelibType::Haxelib, Some("1.0.0"), None, None);
        assert_eq!(
            h.download_url().unwrap(),
            "https://lib.haxe.org/p/funkin.vis/1.0.0/download"
        );
    }

    #[test]
    fn test_download_url_git() {
        let h = make_haxelib(
            "flixel",
            HaxelibType::Git,
            None,
            Some("master"),
            Some("https://github.com/haxeflixel/flixel"),
        );
        assert_eq!(
            h.download_url().unwrap(),
            "https://github.com/haxeflixel/flixel"
        );
    }

    #[test]
    fn test_download_url_dev_fails() {
        let h = make_haxelib("local-lib", HaxelibType::Dev, None, None, None);
        assert!(h.download_url().is_err());
    }

    // --- version_or_ref ---

    #[test]
    fn test_version_or_ref_haxelib() {
        let h = make_haxelib("flixel-addons", HaxelibType::Haxelib, Some("3.3.0"), None, None);
        assert_eq!(h.version_or_ref().unwrap(), "3.3.0");
    }

    #[test]
    fn test_version_or_ref_git() {
        let h = make_haxelib("flixel", HaxelibType::Git, None, Some("master"), None);
        assert_eq!(h.version_or_ref().unwrap(), "master");
    }

    // --- safe accessors (try_*) ---

    #[test]
    fn test_try_version_some() {
        let h = make_haxelib("x", HaxelibType::Haxelib, Some("1.0.0"), None, None);
        assert_eq!(h.try_version(), Some("1.0.0"));
    }

    #[test]
    fn test_try_version_none() {
        let h = make_haxelib("x", HaxelibType::Git, None, None, None);
        assert_eq!(h.try_version(), None);
    }

    #[test]
    fn test_try_vcs_ref_some() {
        let h = make_haxelib("x", HaxelibType::Git, None, Some("main"), None);
        assert_eq!(h.try_vcs_ref(), Some("main"));
    }

    #[test]
    fn test_try_vcs_ref_none() {
        let h = make_haxelib("x", HaxelibType::Haxelib, Some("1.0"), None, None);
        assert_eq!(h.try_vcs_ref(), None);
    }

    #[test]
    fn test_try_url_some() {
        let h = make_haxelib("x", HaxelibType::Git, None, None, Some("https://example.com"));
        assert_eq!(h.try_url(), Some("https://example.com"));
    }

    #[test]
    fn test_try_url_none() {
        let h = make_haxelib("x", HaxelibType::Haxelib, Some("1.0"), None, None);
        assert_eq!(h.try_url(), None);
    }

    // --- error-returning accessors ---

    #[test]
    fn test_version_errors_when_none() {
        let h = make_haxelib("x", HaxelibType::Haxelib, None, None, None);
        assert!(h.version().is_err());
    }

    #[test]
    fn test_vcs_ref_errors_when_none() {
        let h = make_haxelib("x", HaxelibType::Git, None, None, None);
        assert!(h.vcs_ref().is_err());
    }

    #[test]
    fn test_url_errors_when_none() {
        let h = make_haxelib("x", HaxelibType::Git, None, None, None);
        assert!(h.url().is_err());
    }

    // --- validate_lib_name ---

    #[test]
    fn test_validate_lib_name_accepts_real_names() {
        for name in ["flixel", "flixel-addons", "funkin.vis", "hxcpp", "a_b"] {
            assert!(validate_lib_name(name).is_ok(), "{name} should be valid");
        }
    }

    #[test]
    fn test_validate_lib_name_rejects_escaping_names() {
        // `/tmp/x` is the regression case: without validation,
        // `.haxelib`.join("/tmp/x") discards the base and yields `/tmp/x`.
        for name in ["/tmp/x", "a/b", "a\\b", "", "   ", "\u{0}", "a\nb"] {
            assert!(
                validate_lib_name(name).is_err(),
                "{name:?} should be rejected"
            );
        }
    }

    #[test]
    fn test_validate_lib_name_rejects_commas() {
        // Commas are the dot-encoding on disk: `a,b` would alias `a.b`
        // (both map to `.haxelib/a,b`), so they must never be accepted.
        for name in ["a,b", "funkin,vis", ",", "a,"] {
            assert!(
                validate_lib_name(name).is_err(),
                "{name:?} should be rejected"
            );
        }
    }

    #[test]
    fn test_validate_lib_name_rejects_non_haxelib_charset() {
        // Real haxelib's Data.safe allows only [A-Za-z0-9_.-]; anything else
        // throws "Invalid parameter" in `haxelib path`, so hmm-rs rejects it
        // up front.
        for name in ["a b", "a@b", "a:b", "a#b", "a%b", "a*b", "a?b", "\u{e9}clair"] {
            assert!(
                validate_lib_name(name).is_err(),
                "{name:?} should be rejected"
            );
        }
    }

    #[test]
    fn test_validated_names_stay_under_haxelib() {
        for name in ["flixel", "funkin.vis", "..", ".", "-x"] {
            if validate_lib_name(name).is_ok() {
                let path = lib_dir_path_for_name(name);
                assert!(
                    path.starts_with(".haxelib"),
                    "{name:?} escaped to {path:?}"
                );
                assert_eq!(path.components().count(), 2, "{name:?} -> {path:?}");
            }
        }
    }
}
