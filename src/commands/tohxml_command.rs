use std::path::PathBuf;

use crate::hmm::dependencies::Dependancies;
use crate::hmm::haxelib::HaxelibType;
use anyhow::Result;

/// Renders the dependency list as hxml: one `-lib` line per dependency.
pub fn render_hxml(deps: &Dependancies) -> Result<String> {
    let mut hxml = String::new();
    for haxelib in deps.dependencies.iter() {
        let mut lib_string = String::from("-lib ");
        lib_string.push_str(haxelib.name.as_str());

        match haxelib.haxelib_type {
            HaxelibType::Git => {
                lib_string.push_str(format!(":git:{}", &haxelib.url()?).as_str());
                if let Some(r) = &haxelib.vcs_ref {
                    lib_string.push_str(format!("#{}", r).as_str())
                }
            }
            HaxelibType::Haxelib => lib_string.push_str(format!(":{}", haxelib.version()?).as_str()),
            _ => {}
        }
        hxml.push_str(&lib_string);
        hxml.push('\n');
    }

    Ok(hxml)
}

pub fn dump_to_hxml(deps: &Dependancies, hxml_out: Option<PathBuf>) -> Result<()> {
    let hxml = render_hxml(deps)?;

    if let Some(hxml_out) = hxml_out {
        std::fs::write(hxml_out, hxml)?;
    } else {
        println!("{}", hxml);
    }

    Ok(())
}
