#![no_main]

use hmm_rs::commands::haxelib_command::parse_spec;
use libfuzzer_sys::fuzz_target;

// A successful parse must be losslessly reconstructible from its parts, and
// must never hand back an empty name or version.
fuzz_target!(|spec: &str| {
    match parse_spec(spec) {
        Ok((name, Some(version))) => {
            assert!(!name.is_empty(), "empty name from {spec:?}");
            assert!(!version.is_empty(), "empty version from {spec:?}");
            assert_eq!(spec, format!("{name}@{version}"));
        }
        Ok((name, None)) => {
            assert!(!name.is_empty(), "empty name from {spec:?}");
            assert!(!name.contains('@'), "unsplit '@' in {name:?}");
            assert_eq!(spec, name);
        }
        Err(_) => {}
    }
});
