use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Haxelib {
    pub name: String,
    #[serde(rename = "type")]
    pub haxelib_type: HaxelibType,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "ref")]
    pub vcs_ref: Option<String>,
    pub dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl Haxelib {
    pub fn version(&self) -> &str {
        self.version.as_deref().expect(&format!(
            "{}: version field is required for Haxelib
  type",
            self.name
        ))
    }

    pub fn vcs_ref(&self) -> &str {
        self.vcs_ref.as_deref().expect(&format!(
            "{}: vcs_ref field is required for Git type",
            self.name
        ))
    }

    pub fn url(&self) -> &str {
        self.url
            .as_deref()
            .expect(&format!("{}: url field is required", self.name))
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

    pub fn version_as_commas(&self) -> String {
        self.version().replace(".", ",")
    }
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
