use std::path::PathBuf;

use anyhow::{anyhow, Ok, Result};
use reqwest::blocking::Client;

use crate::{
    commands,
    hmm::{
        self,
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

/// Truncates to at most `max` *characters*, never splitting a UTF-8 sequence.
fn truncate_chars(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// Decodes a Haxe remoting `getLatestVersion` response into the version string.
///
/// The wire format is `<tag>:<url-encoded payload>`; anything without a `:` is
/// an error page or a truncated reply.
pub fn parse_remoting_response(resp: &str, name: &str) -> Result<String> {
    let resp_splits = resp.split(':').collect::<Vec<&str>>();
    let encoded = resp_splits.get(1).ok_or_else(|| {
        anyhow!(
            "Unexpected response from lib.haxe.org for '{}': {}",
            name,
            truncate_chars(resp, 200)
        )
    })?;
    let decoded_resp = urlencoding::decode(encoded)?;

    if decoded_resp.starts_with("No such Project") {
        return Err(anyhow!("{}", decoded_resp));
    }

    Ok(decoded_resp.to_string())
}

pub fn install_haxelibs(specs: &[String], json_path: PathBuf) -> Result<()> {
    for spec in specs {
        let (name, version) = parse_spec(spec)?;
        hmm::haxelib::validate_lib_name(name)?;
        let haxelib_install = build_haxelib_install(name, version)?;
        commands::install_command::install_from_haxelib(&haxelib_install, None)?;
        hmm::json::upsert_dependencies(&json_path, &[haxelib_install])?;
    }
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
                "{}/api/3.0/index.n/?__x={}",
                crate::hmm::haxelib::registry_base_url(),
                urlencoding::encode(&serialized)
            );
            let resp = client.get(&url).header("X-Haxe-Remoting", "1").send()?;

            let resp = resp.text()?;
            let decoded_resp = parse_remoting_response(&resp, name)?;

            println!("Latest version of {} is {}", name, decoded_resp);

            haxelib_install.version = Some(decoded_resp);
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

    // --- parse_remoting_response ---

    #[test]
    fn parse_remoting_response_decodes_version() {
        assert_eq!(parse_remoting_response("hxs5:5.0.0", "lime").unwrap(), "5.0.0");
    }

    #[test]
    fn parse_remoting_response_url_decodes() {
        assert_eq!(
            parse_remoting_response("hxs:1.0.0%2Bbuild", "lime").unwrap(),
            "1.0.0+build"
        );
    }

    #[test]
    fn parse_remoting_response_no_such_project_errors() {
        assert!(parse_remoting_response("hxs:No such Project : nope", "nope").is_err());
    }

    #[test]
    fn parse_remoting_response_without_colon_errors() {
        assert!(parse_remoting_response("<html>error</html>", "lime").is_err());
    }

    /// Regression: the error path used to slice at *byte* 200, which panics
    /// when byte 200 lands inside a multi-byte character.
    #[test]
    fn parse_remoting_response_long_non_ascii_does_not_panic() {
        // `€` is 3 bytes, so byte 200 lands mid-character.
        let resp = "€".repeat(300);
        assert!(!resp.is_char_boundary(200), "test input must straddle byte 200");
        assert!(parse_remoting_response(&resp, "lime").is_err());
    }

    #[test]
    fn truncate_chars_respects_char_boundaries() {
        assert_eq!(truncate_chars("ébc", 2), "éb");
        assert_eq!(truncate_chars("abc", 10), "abc");
        assert_eq!(truncate_chars("", 5), "");
    }
}
