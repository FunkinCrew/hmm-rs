use std::path::PathBuf;

use crate::hmm::dependencies::Dependancies;
use crate::hmm::haxelib::HaxelibType;
use anyhow::Result;

/// Renders the dependency list as hxml: one `-lib` line per dependency.
///
/// A haxelib dep with a version renders `-lib name:version`. A git dep without
/// `dir` renders `-lib name:git:<url>[#<ref>]`, which `haxelib install
/// <file>.hxml` (4.1.1 and 4.2.0) clones. Everything else is a bare
/// `-lib name`, as in hmm, which `haxelib path` resolves through `.dev` first
/// and `.current` second. That matters for a git dep with `dir`: it is
/// installed as `.current = git` plus `.dev -> git/<dir>`, and `haxelib path`
/// reads any `name:<x>` as the explicit version `x`, which skips `.dev` and
/// would compile against the repository root. No hxml syntax carries a
/// subdirectory, so the install side loses nothing by keeping it bare.
pub fn render_hxml(deps: &Dependancies) -> String {
    let mut hxml = String::new();
    for haxelib in deps.dependencies.iter() {
        hxml.push_str("-lib ");
        hxml.push_str(&haxelib.name);
        match haxelib.haxelib_type {
            HaxelibType::Haxelib => {
                if let Some(version) = &haxelib.version {
                    hxml.push(':');
                    hxml.push_str(version);
                }
            }
            HaxelibType::Git => {
                if let (None, Some(url)) = (&haxelib.dir, &haxelib.url) {
                    hxml.push_str(":git:");
                    hxml.push_str(url);
                    if let Some(vcs_ref) = &haxelib.vcs_ref {
                        hxml.push('#');
                        hxml.push_str(vcs_ref);
                    }
                }
            }
            _ => {}
        }
        hxml.push('\n');
    }
    hxml
}

pub fn dump_to_hxml(deps: &Dependancies, hxml_out: Option<PathBuf>) -> Result<()> {
    let hxml = render_hxml(deps);

    if let Some(hxml_out) = hxml_out {
        std::fs::write(hxml_out, hxml)?;
    } else {
        print!("{}", hxml);
    }

    Ok(())
}
