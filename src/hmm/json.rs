use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use super::dependencies::Dependancies;
use super::haxelib::Haxelib;
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::{ser::PrettyFormatter, Value};

/// Writes `deps` as a fresh hmm.json. Only used for creating a new file (`init`);
/// edits to an existing file go through `upsert_dependencies` /
/// `remove_dependencies` so untouched entries stay byte-identical.
pub fn save_json(deps: Dependancies, path: PathBuf) -> Result<()> {
    println!("{} saved/updated", path.display());
    let mut j = serde_json::to_string_pretty(&deps)?;
    j.push('\n');
    let mut file = File::create(path)?;
    file.write_all(j.as_bytes())?;
    Ok(())
}

pub fn create_empty_hmm_json() -> Result<()> {
    let empty_deps = Dependancies {
        dependencies: vec![],
    };

    save_json(empty_deps, PathBuf::from_str("hmm.json")?)
}

// Read the JSON, and return the Dependancies struct
pub fn read_json(path: &PathBuf) -> Result<Dependancies> {
    let file = File::open(path).context(format!("JSON {:?} not found", path))?;
    let deps: Dependancies = serde_json::from_reader(file)?;
    // Names from hmm.json feed directly into `.haxelib/` paths (including
    // `remove_dir_all`), so reject anything that could escape before use.
    for lib in deps.dependencies.iter() {
        crate::hmm::haxelib::validate_lib_name(&lib.name)
            .with_context(|| format!("in {}", path.display()))?;
    }
    Ok(deps)
}

/// Optional keys that `Haxelib` owns. When an entry is overlaid, any of these
/// missing from the new value are removed; every other key in the entry is
/// user data and is left alone.
const OPTIONAL_KEYS: [&str; 5] = ["dir", "ref", "path", "url", "version"];

/// Adds or updates `libs` in hmm.json by name, touching nothing else.
///
/// An existing entry keeps its position and key order: the fields `Haxelib`
/// serializes are written over it in place and stale optional keys are dropped.
/// A new entry is appended to the end of the array.
pub fn upsert_dependencies(path: &Path, libs: &[Haxelib]) -> Result<()> {
    edit_dependencies(path, |entries| {
        for lib in libs {
            let new = match serde_json::to_value(lib)? {
                Value::Object(map) => map,
                other => return Err(anyhow!("expected object for {}, got {}", lib.name, other)),
            };
            match entries
                .iter_mut()
                .find(|e| entry_name(e) == Some(&lib.name))
            {
                Some(Value::Object(existing)) => {
                    for key in OPTIONAL_KEYS {
                        if !new.contains_key(key) {
                            existing.shift_remove(key);
                        }
                    }
                    for (k, v) in new {
                        existing.insert(k, v);
                    }
                }
                _ => entries.push(Value::Object(new)),
            }
        }
        Ok(())
    })
}

/// Removes every entry whose name is in `names`, touching nothing else.
pub fn remove_dependencies(path: &Path, names: &[String]) -> Result<()> {
    edit_dependencies(path, |entries| {
        entries.retain(|e| !entry_name(e).is_some_and(|n| names.iter().any(|x| x == n)));
        Ok(())
    })
}

fn entry_name(entry: &Value) -> Option<&str> {
    entry.get("name").and_then(Value::as_str)
}

/// Read-modify-write of the `dependencies` array, preserving the file's own
/// indent unit and whether it ended with a newline.
fn edit_dependencies(path: &Path, edit: impl FnOnce(&mut Vec<Value>) -> Result<()>) -> Result<()> {
    let text = fs::read_to_string(path).with_context(|| format!("JSON {:?} not found", path))?;
    let mut root: Value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let entries = root
        .get_mut("dependencies")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("{}: 'dependencies' must be an array", path.display()))?;
    edit(entries)?;

    let indent = detect_indent(&text);
    let mut out = Vec::new();
    let mut ser =
        serde_json::Serializer::with_formatter(&mut out, PrettyFormatter::with_indent(indent));
    root.serialize(&mut ser)?;
    if text.ends_with('\n') {
        out.push(b'\n');
    }
    fs::write(path, out)?;
    println!("{} saved/updated", path.display());
    Ok(())
}

/// The indent unit of `text`: the leading whitespace of its first indented
/// line, or two spaces when there is none (minified file).
fn detect_indent(text: &str) -> &[u8] {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| &line[..line.len() - line.trim_start().len()])
        .find(|ws| !ws.is_empty())
        .unwrap_or("  ")
        .as_bytes()
}
