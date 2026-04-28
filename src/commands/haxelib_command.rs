use std::path::PathBuf;

use anyhow::{anyhow, Ok, Result};
use reqwest::blocking::Client;

use crate::{
    commands,
    hmm::{
        self,
        dependencies::Dependancies,
        haxelib::{Haxelib, HaxelibType},
    },
};

/// Parse a library spec into (name, optional version).
/// Accepts `name` or `name@version`.
pub fn parse_spec(spec: &str) -> Result<(&str, Option<&str>)> {
    match spec.split_once('@') {
        Some((name, version)) => {
            if name.is_empty() {
                return Err(anyhow!("invalid spec '{}': missing library name", spec));
            }
            if version.is_empty() {
                return Err(anyhow!("invalid spec '{}': missing version after '@'", spec));
            }
            Ok((name, Some(version)))
        }
        None => {
            if spec.is_empty() {
                return Err(anyhow!("invalid spec: empty"));
            }
            Ok((spec, None))
        }
    }
}

pub fn install_haxelibs(
    specs: &[String],
    mut deps: Dependancies,
    json_path: PathBuf,
) -> Result<()> {
    for spec in specs {
        let (name, version) = parse_spec(spec)?;
        let haxelib_install = build_haxelib_install(name, version)?;
        commands::install_command::install_from_haxelib(&haxelib_install)?;
        deps.dependencies.push(haxelib_install);
    }
    hmm::json::save_json(deps, json_path)?;
    Ok(())
}

fn build_haxelib_install(name: &str, version: Option<&str>) -> Result<Haxelib> {
    let mut haxelib_install = Haxelib {
        name: name.to_string(),
        haxelib_type: HaxelibType::Haxelib,
        vcs_ref: None,
        dir: None,
        path: None,
        url: None,
        version: None,
    };
    match version {
        Some(v) => haxelib_install.version = Some(v.to_string()),
        None => {
            // we need to query the latest version from haxelib
            // haxelib url: lib.haxe.org/api/3.0/index.n/
            // needs X-Haxe-Remoting header
            // and __x param with the query
            // in __x param, we can query with something like
            // ay3:apiy16:getLatestVersionhay4:limeh
            let serialized = format!("ay3:apiy16:getLatestVersionhay{}:{}h", name.len(), name);
            let client = Client::new();

            let url = format!(
                "https://lib.haxe.org/api/3.0/index.n/?__x={}",
                urlencoding::encode(&serialized)
            );
            let resp = client.get(&url).header("X-Haxe-Remoting", "1").send()?;

            let resp = resp.text()?;
            let resp_splits = resp.split(':').collect::<Vec<&str>>();
            let encoded = resp_splits.get(1).ok_or_else(|| {
                anyhow!(
                    "Unexpected response from lib.haxe.org for '{}': {}",
                    name,
                    &resp[..resp.len().min(200)]
                )
            })?;
            let decoded_resp = urlencoding::decode(encoded)?;

            println!("Latest version of {} is {}", name, decoded_resp);

            if decoded_resp.starts_with("No such Project") {
                return Err(anyhow!("{}", decoded_resp));
            }

            haxelib_install.version = Some(decoded_resp.to_string());
        }
    };
    Ok(haxelib_install)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_spec_name_only() {
        let (name, version) = parse_spec("lime").unwrap();
        assert_eq!(name, "lime");
        assert_eq!(version, None);
    }

    #[test]
    fn parse_spec_name_at_version() {
        let (name, version) = parse_spec("lime@5.0.0").unwrap();
        assert_eq!(name, "lime");
        assert_eq!(version, Some("5.0.0"));
    }

    #[test]
    fn parse_spec_dotted_name() {
        let (name, version) = parse_spec("funkin.vis@1.2.3").unwrap();
        assert_eq!(name, "funkin.vis");
        assert_eq!(version, Some("1.2.3"));
    }

    #[test]
    fn parse_spec_empty_errors() {
        assert!(parse_spec("").is_err());
    }

    #[test]
    fn parse_spec_missing_name_errors() {
        assert!(parse_spec("@5.0.0").is_err());
    }

    #[test]
    fn parse_spec_missing_version_errors() {
        assert!(parse_spec("lime@").is_err());
    }
}
