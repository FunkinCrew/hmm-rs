#![no_main]

use hmm_rs::commands::haxelib_command::parse_remoting_response;
use libfuzzer_sys::fuzz_target;

// The registry's Haxe remoting reply is untrusted text. Decoding it must never
// panic -- this target covers the char-boundary slice that used to blow up on
// long non-ASCII error pages.
fuzz_target!(|input: (String, String)| {
    let (resp, name) = input;
    if let Ok(version) = parse_remoting_response(&resp, &name) {
        // A successful decode is never the "no such project" sentinel.
        assert!(!version.starts_with("No such Project"));
    }
});
