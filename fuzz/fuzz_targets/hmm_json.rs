#![no_main]

use hmm_rs::hmm::dependencies::Dependancies;
use libfuzzer_sys::fuzz_target;

// Arbitrary bytes must never panic the hmm.json parser, and anything that does
// parse must survive a serialize/re-parse cycle unchanged. The second part
// guards the blank-string-to-None normalization in `de_blank_as_none` against
// producing output it cannot itself read back.
fuzz_target!(|data: &[u8]| {
    let Ok(deps) = serde_json::from_slice::<Dependancies>(data) else {
        return;
    };

    let text = serde_json::to_string(&deps).expect("serializing a parsed value must not fail");
    let reparsed: Dependancies =
        serde_json::from_str(&text).expect("re-parsing our own output must not fail");

    assert_eq!(deps.dependencies.len(), reparsed.dependencies.len());

    // Serialization is a fixpoint after the first pass.
    let text2 = serde_json::to_string(&reparsed).unwrap();
    assert_eq!(text, text2);
});
