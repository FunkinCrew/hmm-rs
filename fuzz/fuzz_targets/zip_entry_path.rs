#![no_main]

use hmm_rs::commands::install_command::sanitize_zip_entry;
use libfuzzer_sys::fuzz_target;
use std::path::{Component, Path};

// Zip archives come straight off lib.haxe.org, so a hostile entry name must
// never resolve to a path outside the destination directory (zip-slip).
fuzz_target!(|input: (String, String)| {
    let (base_path, entry_name) = input;
    let dest = Path::new(".haxelib/lib/1,0,0");

    let Some(out) = sanitize_zip_entry(&base_path, &entry_name, dest) else {
        return;
    };

    assert!(
        out.starts_with(dest),
        "{entry_name:?} (base {base_path:?}) escaped to {out:?}"
    );

    let extra = out
        .strip_prefix(dest)
        .expect("starts_with already checked the prefix");
    assert!(
        extra.components().count() > 0,
        "{entry_name:?} produced the destination itself"
    );
    assert!(
        extra.components().all(|c| matches!(c, Component::Normal(_))),
        "{entry_name:?} produced a traversal component: {out:?}"
    );
});
