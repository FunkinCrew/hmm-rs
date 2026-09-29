#![no_main]

use hmm_rs::hmm::haxelib::{
    exact_case_lib_dir_path_for_name, lib_dir_path_for_name, validate_lib_name,
};
use libfuzzer_sys::fuzz_target;
use std::path::Component;

// Any name that passes validation must map to exactly `.haxelib/<one dir>`,
// both lowercased and in exact case. Consumers call `remove_dir_all` on these
// paths, so escaping them is destructive.
fuzz_target!(|name: &str| {
    if validate_lib_name(name).is_err() {
        return;
    }

    for (path, expected) in [
        (lib_dir_path_for_name(name), name.to_ascii_lowercase()),
        (exact_case_lib_dir_path_for_name(name), name.to_string()),
    ] {
        assert!(path.starts_with(".haxelib"), "{name:?} escaped to {path:?}");
        assert!(
            path.components().all(|c| matches!(c, Component::Normal(_))),
            "{name:?} produced a non-normal component: {path:?}"
        );
        assert_eq!(
            path.components().count(),
            2,
            "{name:?} did not yield a single directory under .haxelib: {path:?}"
        );

        // The encoding must be invertible for accepted names (validation
        // rejects commas), which makes the mapping injective up to ASCII case:
        // `a,b` can never alias `a.b`.
        let encoded = path.file_name().unwrap().to_str().unwrap();
        assert!(!encoded.contains('.'), "{name:?} left a dot in {encoded:?}");
        assert_eq!(
            encoded.replace(',', "."),
            expected,
            "{name:?} does not round-trip through the comma encoding"
        );
    }
});
