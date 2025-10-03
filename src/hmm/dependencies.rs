use super::haxelib::{Haxelib, HaxelibType};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, Clone)]
pub struct Dependancies {
    pub dependencies: Vec<Haxelib>,
}

impl fmt::Display for Dependancies {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", serde_json::to_string_pretty(self).unwrap())
    }
}

impl Dependancies {
    pub fn print_string_list(&self, libs: &Option<Vec<String>>) -> Result<()> {
        if let Some(libs) = libs {
            for lib in libs {
                let haxelib = Self::get_haxelib(self, lib)?;
                Self::print_haxelib(haxelib);
            }

            return Ok(());
        }

        for haxelib in self.dependencies.iter() {
            Self::print_haxelib(haxelib);
        }
        Ok(())
    }

    pub fn get_haxelib(&self, lib: &str) -> Result<&Haxelib> {
        for haxelib in self.dependencies.iter() {
            if haxelib.name == lib {
                return Ok(haxelib);
            }
        }
        Err(anyhow::anyhow!("Haxelib not found"))
    }

    pub fn print_haxelib(lib: &Haxelib) {
        let version_or_ref = match &lib.version {
            Some(v) => format!("version: {}", v),
            None => match &lib.vcs_ref {
                Some(r) => format!("ref: {}", r),
                None => "No version or ref".to_string(),
            },
        };

        let mut haxelib_output = format!(
            "{} [{haxelib_type:?}] \n{} \n",
            lib.name,
            version_or_ref,
            haxelib_type = lib.haxelib_type
        );

        match lib.haxelib_type {
            HaxelibType::Git => {
                if let Some(u) = &lib.url {
                    haxelib_output.push_str(&format!("url: {}\n", u))
                }
            }
            HaxelibType::Haxelib => {
                let haxelib_url = format!("https://lib.haxe.org/p/{}", lib.name);
                haxelib_output.push_str(&format!("url: {}\n", haxelib_url))
            }
            _ => {}
        }

        println!("{}", haxelib_output);
    }
}
