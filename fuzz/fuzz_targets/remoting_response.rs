#![no_main]

use hmm_rs::commands::haxelib_command::{parse_infos_versions, pick_latest_version};
use libfuzzer_sys::fuzz_target;

// The registry's Haxe remoting reply is untrusted text. Decoding it must never
// panic or overflow the stack (it nests and back-references strings), the
// versions it yields must come back unchanged from a reply re-serialized from
// them, and the pick must be one of them, a release whenever one is listed.
fuzz_target!(|resp: &str| {
    let Ok(versions) = parse_infos_versions(resp, "lib") else {
        return;
    };

    // Every byte percent-encoded is a valid (if verbose) Haxe string payload.
    let mut reply = String::from("hxroy8:versionsa");
    for v in &versions {
        let encoded: String = v.bytes().map(|b| format!("%{b:02X}")).collect();
        reply += &format!("oy4:namey{}:{}g", encoded.len(), encoded);
    }
    reply += "hg";
    assert_eq!(parse_infos_versions(&reply, "lib").unwrap(), versions);

    if let Ok(Some(latest)) = pick_latest_version(&versions) {
        assert!(versions.iter().any(|v| v == latest));
        if versions.iter().any(|v| !v.contains('-')) {
            assert!(!latest.contains('-'), "{latest} picked over a release");
        }
    }
});
