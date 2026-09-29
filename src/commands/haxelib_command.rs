use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Ok, Result};
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

/// A value in Haxe's serialization format, limited to what a registry reply
/// holds.
#[derive(Debug)]
enum HaxeValue {
    String(String),
    /// An array or a list.
    Array(Vec<HaxeValue>),
    /// An anonymous object's fields, in order.
    Object(Vec<(String, HaxeValue)>),
    /// Null, a bool or a number. Never read, only skipped.
    Scalar,
}

impl HaxeValue {
    fn get(&self, key: &str) -> Option<&HaxeValue> {
        match self {
            HaxeValue::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

/// An `infos` reply nests three deep; this only bounds hostile input.
const MAX_DEPTH: usize = 32;

/// Decodes `haxe.Serializer` output for scalars, strings, arrays, lists and
/// anonymous objects. Every string decoded is cached, and `R<n>` refers back to
/// the n-th one: lib.haxe.org writes each repeated key and value that way.
struct Unserializer<'a> {
    buf: &'a [u8],
    pos: usize,
    strings: Vec<String>,
}

impl Unserializer<'_> {
    fn peek(&self) -> Option<u8> {
        self.buf.get(self.pos).copied()
    }

    fn skip_while(&mut self, accept: impl Fn(u8) -> bool) -> &[u8] {
        let start = self.pos;
        while self.peek().is_some_and(&accept) {
            self.pos += 1;
        }
        &self.buf[start..self.pos]
    }

    fn number(&mut self) -> Result<usize> {
        let digits = self.skip_while(|b| b.is_ascii_digit());
        Ok(std::str::from_utf8(digits)?.parse()?)
    }

    fn value(&mut self, depth: usize) -> Result<HaxeValue> {
        if depth > MAX_DEPTH {
            bail!("nested deeper than {}", MAX_DEPTH);
        }
        let tag = self.peek().context("unexpected end")?;
        self.pos += 1;
        Ok(match tag {
            b'n' | b't' | b'f' | b'z' | b'k' | b'm' | b'p' => HaxeValue::Scalar,
            b'i' => {
                if self.peek() == Some(b'-') {
                    self.pos += 1;
                }
                self.number()?;
                HaxeValue::Scalar
            }
            b'd' => {
                self.skip_while(|b| b.is_ascii_digit() || b"+-.eE".contains(&b));
                HaxeValue::Scalar
            }
            b'y' => {
                let len = self.number()?;
                if self.peek() != Some(b':') {
                    bail!("missing ':' after string length");
                }
                self.pos += 1;
                let raw = self
                    .pos
                    .checked_add(len)
                    .and_then(|end| self.buf.get(self.pos..end))
                    .context("string runs past the end")?;
                self.pos += len;
                let s = urlencoding::decode(std::str::from_utf8(raw)?)?.into_owned();
                self.strings.push(s.clone());
                HaxeValue::String(s)
            }
            b'R' => {
                let i = self.number()?;
                HaxeValue::String(self.strings.get(i).context("bad string reference")?.clone())
            }
            b'a' | b'l' => {
                let mut items = Vec::new();
                while self.peek() != Some(b'h') {
                    items.push(self.value(depth + 1)?);
                }
                self.pos += 1;
                HaxeValue::Array(items)
            }
            b'o' => {
                let mut fields = Vec::new();
                while self.peek() != Some(b'g') {
                    let HaxeValue::String(key) = self.value(depth + 1)? else {
                        bail!("object key is not a string");
                    };
                    fields.push((key, self.value(depth + 1)?));
                }
                self.pos += 1;
                HaxeValue::Object(fields)
            }
            _ => bail!("unsupported tag {:?}", tag as char),
        })
    }
}

fn unserialize(s: &str) -> Result<HaxeValue> {
    Unserializer {
        buf: s.as_bytes(),
        pos: 0,
        strings: Vec::new(),
    }
    .value(0)
}

/// Decodes the registry's Haxe remoting reply to `api.infos(name)` into the
/// library's released version names, in the order listed.
pub fn parse_infos_versions(resp: &str, name: &str) -> Result<Vec<String>> {
    let unexpected = || {
        format!(
            "Unexpected response from lib.haxe.org for '{}': {}",
            name,
            truncate_chars(resp, 200)
        )
    };
    let body = resp.strip_prefix("hxr").with_context(unexpected)?;
    // `x` marks an exception thrown by the server, such as "No such Project : <name>".
    if let Some(exception) = body.strip_prefix('x') {
        return match unserialize(exception) {
            Result::Ok(HaxeValue::String(msg)) => Err(anyhow!(msg)),
            _ => Err(anyhow!(unexpected())),
        };
    }
    let infos = unserialize(body).with_context(unexpected)?;
    let Some(HaxeValue::Array(versions)) = infos.get("versions") else {
        bail!(unexpected());
    };
    versions
        .iter()
        .map(|v| match v.get("name") {
            Some(HaxeValue::String(version)) => Ok(version.clone()),
            _ => Err(anyhow!(unexpected())),
        })
        .collect()
}

/// Rank of a release in [`semver_key`]: above every prerelease tag.
const RELEASE_RANK: u8 = u8::MAX;

/// Sort key matching haxelib's `SemVer.compare`, or `None` for a version
/// haxelib rejects. A release ranks above its prereleases, tags rank
/// `alpha < beta < rc < preview`, and `-rc` ranks below `-rc.0`.
fn semver_key(version: &str) -> Option<(u64, u64, u64, u8, Option<u64>)> {
    // haxelib's `\d|[1-9]\d*`: digits with no leading zero.
    fn num(s: &str) -> Option<u64> {
        let digits = !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        if digits && (s == "0" || !s.starts_with('0')) {
            s.parse().ok()
        } else {
            None
        }
    }
    let (release, preview) = match version.split_once('-') {
        Some((release, preview)) => (release, Some(preview)),
        None => (version, None),
    };
    let mut parts = release.split('.');
    let (major, minor, patch) = (
        num(parts.next()?)?,
        num(parts.next()?)?,
        num(parts.next()?)?,
    );
    if parts.next().is_some() {
        return None;
    }
    let Some(preview) = preview else {
        return Some((major, minor, patch, RELEASE_RANK, None));
    };
    let (tag, preview_num) = match preview.split_once('.') {
        Some((tag, n)) => (tag, Some(num(n)?)),
        None => (preview, None),
    };
    let rank = match tag {
        "alpha" => 0,
        "beta" => 1,
        "rc" => 2,
        "preview" => 3,
        _ => return None,
    };
    Some((major, minor, patch, rank, preview_num))
}

/// The version `haxelib install <name>` picks from the registry's list: the
/// newest release, else the newest prerelease. `None` for an empty list.
pub fn pick_latest_version(versions: &[String]) -> Result<Option<&str>> {
    let keyed = versions
        .iter()
        .map(|v| {
            let key = semver_key(v)
                .with_context(|| format!("lib.haxe.org listed an invalid version '{}'", v))?;
            Ok((key, v.as_str()))
        })
        .collect::<Result<Vec<_>>>()?;
    let newest_release = keyed.iter().filter(|(key, _)| key.3 == RELEASE_RANK).max();
    Ok(newest_release.or(keyed.iter().max()).map(|(_, v)| *v))
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
    let version = match version {
        Some(v) => v.to_string(),
        None => resolve_latest_version(name)?,
    };
    Ok(Haxelib {
        name: name.to_string(),
        haxelib_type: HaxelibType::Haxelib,
        vcs_ref: None,
        dir: None,
        path: None,
        url: None,
        version: Some(version),
    })
}

/// Asks the registry which version `haxelib install <name>` would install.
///
/// This fetches the full version list (`infos`) and picks client-side like
/// haxelib does. The server's `getLatestVersion` is not used: it ranks a
/// prerelease of a higher x.y.z above the newest release.
pub fn resolve_latest_version(name: &str) -> Result<String> {
    // haxelib url: lib.haxe.org/api/3.0/index.n/
    // needs X-Haxe-Remoting header
    // and __x param with the serialized call `api.infos(name)`, e.g. for lime:
    // ay3:apiy5:infoshay4:limeh
    let serialized = format!("ay3:apiy5:infoshay{}:{}h", name.len(), name);
    let client = Client::new();

    let url = format!(
        "{}/api/3.0/index.n/?__x={}",
        crate::hmm::haxelib::registry_base_url(),
        urlencoding::encode(&serialized)
    );
    let resp = client.get(&url).header("X-Haxe-Remoting", "1").send()?;

    let resp = resp.text()?;
    let versions = parse_infos_versions(&resp, name)?;
    let latest = pick_latest_version(&versions)?
        .with_context(|| format!("The library {} has not yet released a version", name))?
        .to_string();

    println!("Latest version of {} is {}", name, latest);

    Ok(latest)
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

    // --- parse_infos_versions ---

    /// A live lib.haxe.org `infos` reply for flixel, cut to its first two
    /// versions. Keys and values repeat as `R<n>` back-references.
    const FLIXEL_INFOS: &str = "hxroy4:descy106:HaxeFlixel%20is%20a%202D%20game%20engine%20based%20on%20OpenFL%20that%20delivers%20cross-platform%20games.y4:namey6:flixely7:licensey3:MITy4:tagsly7:androidy3:cppy5:crossy5:flashy4:gamey5:html5y3:iosy4:nekoy6:openflhy8:versionsaoy8:commentsy43:Alpha%20release%20with%20Haxe%203%20supporty4:datey25:2013-05-28%2013%3A03%3A19R2y11:2.0.0-alphay9:downloadsi63goR17y56:Second%20alpha%20release%20with%20openfl%20compatibilityR19y25:2013-06-02%2008%3A37%3A30R2y13:2.0.0-alpha.2R22i162ghy12:contributorsaoR2y10:haxeflixely8:fullnamey18:Alexander%20Hohlovghy7:websitey46:https%3A%2F%2Fgithub.com%2FHaxeFlixel%2FflixelR22i1448389y5:ownerR27y10:curversionR25g";

    #[test]
    fn parse_infos_versions_reads_live_reply() {
        assert_eq!(
            parse_infos_versions(FLIXEL_INFOS, "flixel").unwrap(),
            ["2.0.0-alpha", "2.0.0-alpha.2"]
        );
        let infos = unserialize(FLIXEL_INFOS.strip_prefix("hxr").unwrap()).unwrap();
        assert!(matches!(
            infos.get("curversion"),
            Some(HaxeValue::String(v)) if v == "2.0.0-alpha.2"
        ));
    }

    /// A version name that equals an earlier string (here a release comment)
    /// arrives as a back-reference, not as a string literal.
    #[test]
    fn parse_infos_versions_resolves_back_referenced_version() {
        let resp = "hxroy4:namey4:demoy8:versionsaoy8:commentsy5:2.0.0R0y5:1.0.0goR3y4:oopsR0R4ghg";
        assert_eq!(
            parse_infos_versions(resp, "demo").unwrap(),
            ["1.0.0", "2.0.0"]
        );
    }

    #[test]
    fn parse_infos_versions_reports_server_exception() {
        let resp = "hxrxy40:No%20such%20Project%20%3A%20nonexistentx";
        let err = parse_infos_versions(resp, "nonexistentx").unwrap_err();
        assert_eq!(err.to_string(), "No such Project : nonexistentx");
    }

    #[test]
    fn parse_infos_versions_rejects_malformed_replies() {
        for resp in [
            "<html>error</html>",
            "hxs5:5.0.0",
            "hxroy4:namey4:dem",
            "hxroR5y1:ag",
            "hxroy8:versionsy3:abcg",
            "hxroy8:versionsaoy4:namei1ghg",
        ] {
            assert!(parse_infos_versions(resp, "demo").is_err(), "{resp}");
        }
    }

    #[test]
    fn parse_infos_versions_bounds_nesting() {
        let resp = format!("hxr{}", "a".repeat(100_000));
        assert!(parse_infos_versions(&resp, "demo").is_err());
    }

    /// Regression: the error path used to slice at *byte* 200, which panics
    /// when byte 200 lands inside a multi-byte character.
    #[test]
    fn parse_infos_versions_long_non_ascii_does_not_panic() {
        // `€` is 3 bytes, so byte 200 lands mid-character.
        let resp = "€".repeat(300);
        assert!(!resp.is_char_boundary(200), "test input must straddle byte 200");
        assert!(parse_infos_versions(&resp, "lime").is_err());
    }

    // --- pick_latest_version ---

    fn latest(versions: &[&str]) -> Result<Option<String>> {
        let versions: Vec<String> = versions.iter().map(|v| v.to_string()).collect();
        Ok(pick_latest_version(&versions)?.map(str::to_string))
    }

    #[test]
    fn pick_latest_version_prefers_release_over_newer_prerelease() {
        let got = latest(&["1.0.0", "2.0.0", "3.0.0-rc.1"]).unwrap();
        assert_eq!(got.as_deref(), Some("2.0.0"));
    }

    #[test]
    fn pick_latest_version_compares_numerically() {
        let got = latest(&["10.0.0", "9.0.0", "1.10.0-rc.1"]).unwrap();
        assert_eq!(got.as_deref(), Some("10.0.0"));
    }

    #[test]
    fn pick_latest_version_falls_back_to_newest_prerelease() {
        let got = latest(&["1.0.0-rc.2", "1.0.0-beta.1", "1.0.0-rc.10", "1.0.0-alpha"]).unwrap();
        assert_eq!(got.as_deref(), Some("1.0.0-rc.10"));
        let got = latest(&["1.0.0-rc.0", "1.0.0-rc"]).unwrap();
        assert_eq!(got.as_deref(), Some("1.0.0-rc.0"));
    }

    #[test]
    fn pick_latest_version_empty_is_none() {
        assert_eq!(latest(&[]).unwrap(), None);
    }

    #[test]
    fn pick_latest_version_rejects_versions_haxelib_rejects() {
        for bad in [
            "1.0",
            "01.0.0",
            "1.0.0-RC.1",
            "1.0.0-",
            "1.0.0-rc.01",
            "1.0.0+b",
            "+1.0.0",
        ] {
            assert!(latest(&["1.0.0", bad]).is_err(), "{bad}");
        }
    }

    #[test]
    fn truncate_chars_respects_char_boundaries() {
        assert_eq!(truncate_chars("ébc", 2), "éb");
        assert_eq!(truncate_chars("abc", 10), "abc");
        assert_eq!(truncate_chars("", 5), "");
    }
}
