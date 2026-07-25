#![no_main]

use hmm_rs::commands::install_command::parse_remote_name_from_url;
use libfuzzer_sys::fuzz_target;

// Git remote names are derived from URLs that come out of hmm.json. A parsed
// name is used as a `git remote` name, so it must be non-empty and must not
// pick up path separators from the URL.
fuzz_target!(|input: (String, String)| {
    let (url, separator) = input;
    let Ok(remote) = parse_remote_name_from_url(&url, &separator) else {
        return;
    };

    assert!(!remote.is_empty(), "empty remote name from {url:?}");
    if !separator.contains('/') {
        assert!(
            !remote.contains('/'),
            "{url:?} leaked a path separator into {remote:?}"
        );
    }
});
