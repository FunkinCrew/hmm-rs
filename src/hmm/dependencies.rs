use super::haxelib::{Haxelib, HaxelibType};
use anyhow::Result;
use owo_colors::OwoColorize;
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
    pub fn print_string_list(&self, names: &[String]) -> Result<()> {
        for haxelib in self.filter_by_names(names) {
            Self::print_haxelib(haxelib);
        }
        Ok(())
    }

    /// Returns refs to deps named in `names`, or all deps if `names` is empty.
    /// Prints a warning for any name not in `hmm.json` and skips it.
    pub fn filter_by_names(&self, names: &[String]) -> Vec<&Haxelib> {
        if names.is_empty() {
            return self.dependencies.iter().collect();
        }
        let mut out = Vec::with_capacity(names.len());
        for n in names {
            match self.dependencies.iter().find(|d| &d.name == n) {
                Some(h) => out.push(h),
                None => println!("{}: not found in hmm.json, skipping", n.red().bold()),
            }
        }
        out
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_deps(names: &[&str]) -> Dependancies {
        Dependancies {
            dependencies: names
                .iter()
                .map(|name| Haxelib {
                    name: name.to_string(),
                    haxelib_type: HaxelibType::Haxelib,
                    dir: None,
                    vcs_ref: None,
                    path: None,
                    url: None,
                    version: Some("1.0.0".to_string()),
                })
                .collect(),
        }
    }

    #[test]
    fn test_get_haxelib_found() {
        let deps = make_deps(&["flixel", "lime"]);
        let result = deps.get_haxelib("lime");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "lime");
    }

    #[test]
    fn test_get_haxelib_not_found() {
        let deps = make_deps(&["flixel", "lime"]);
        let result = deps.get_haxelib("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_haxelib_returns_first_match() {
        // If duplicates exist, get_haxelib returns the first one (linear search)
        let mut deps = make_deps(&["flixel", "flixel"]);
        deps.dependencies[0].version = Some("1.0.0".to_string());
        deps.dependencies[1].version = Some("2.0.0".to_string());
        let result = deps.get_haxelib("flixel").unwrap();
        assert_eq!(result.try_version(), Some("1.0.0"));
    }

    #[test]
    fn test_filter_by_names_empty_returns_all() {
        let deps = make_deps(&["flixel", "lime", "openfl"]);
        let filtered = deps.filter_by_names(&[]);
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0].name, "flixel");
        assert_eq!(filtered[1].name, "lime");
        assert_eq!(filtered[2].name, "openfl");
    }

    #[test]
    fn test_filter_by_names_subset() {
        let deps = make_deps(&["flixel", "lime", "openfl"]);
        let names = vec!["flixel".to_string(), "openfl".to_string()];
        let filtered = deps.filter_by_names(&names);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "flixel");
        assert_eq!(filtered[1].name, "openfl");
    }

    #[test]
    fn test_filter_by_names_unknown_skips() {
        let deps = make_deps(&["flixel", "lime"]);
        let names = vec!["flixel".to_string(), "nope".to_string()];
        let filtered = deps.filter_by_names(&names);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "flixel");
    }

    #[test]
    fn test_filter_by_names_all_unknown_returns_empty() {
        let deps = make_deps(&["flixel", "lime"]);
        let names = vec!["nope".to_string(), "also-nope".to_string()];
        let filtered = deps.filter_by_names(&names);
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_filter_by_names_preserves_input_order() {
        let deps = make_deps(&["a", "b", "c"]);
        let names = vec!["c".to_string(), "a".to_string()];
        let filtered = deps.filter_by_names(&names);
        assert_eq!(filtered[0].name, "c");
        assert_eq!(filtered[1].name, "a");
    }
}
