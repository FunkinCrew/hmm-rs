use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct Haxelib {
    pub name: String,
    #[serde(rename = "type")]
    pub haxelib_type: HaxelibType,
    pub dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "ref")]
    pub vcs_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
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
                    "https://lib.haxe.org/p/{}/{}/download",
                    self.name, version
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

    pub fn name_as_commas(&self) -> String {
        self.name.replace(".", ",")
    }

    /// Returns the library directory path: .haxelib/{name_with_commas}
    pub fn lib_dir_path(&self) -> PathBuf {
        lib_dir_path_for_name(&self.name)
    }

    /// Returns the git repo path: .haxelib/{name_with_commas}/git
    pub fn git_repo_path(&self) -> PathBuf {
        self.lib_dir_path().join("git")
    }
}

/// Returns the library directory path given a library name
pub fn lib_dir_path_for_name(name: &str) -> PathBuf {
    PathBuf::from(".haxelib").join(name.replace(".", ","))
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

    // --- name_as_commas ---

    #[test]
    fn test_name_as_commas_with_dots() {
        let h = make_haxelib("funkin.vis", HaxelibType::Git, None, None, None);
        assert_eq!(h.name_as_commas(), "funkin,vis");
    }

    #[test]
    fn test_name_as_commas_without_dots() {
        let h = make_haxelib("flixel", HaxelibType::Git, None, None, None);
        assert_eq!(h.name_as_commas(), "flixel");
    }

    #[test]
    fn test_name_as_commas_multiple_dots() {
        let h = make_haxelib("a.b.c", HaxelibType::Git, None, None, None);
        assert_eq!(h.name_as_commas(), "a,b,c");
    }

    // --- version_as_commas ---

    #[test]
    fn test_version_as_commas() {
        let h = make_haxelib("flixel", HaxelibType::Haxelib, Some("3.3.0"), None, None);
        assert_eq!(h.version_as_commas().unwrap(), "3,3,0");
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
}
