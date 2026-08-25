//! Property tests for the pure parsing/encoding surfaces.
//!
//! These deliberately mirror the `fuzz/fuzz_targets/` invariants. The fuzz
//! targets go deeper but need nightly and an explicit run; these run on stable
//! in every `cargo test`, so the invariants are enforced on every PR.

use hmm_rs::commands::haxelib_command::{parse_remoting_response, parse_spec};
use hmm_rs::commands::install_command::sanitize_zip_entry;
use hmm_rs::commands::tohxml_command::render_hxml;
use hmm_rs::hmm::dependencies::Dependancies;
use hmm_rs::hmm::haxelib::{lib_dir_path_for_name, validate_lib_name, Haxelib, HaxelibType};
use hmm_rs::hmm::json;
use proptest::prelude::*;
use std::path::{Component, Path};

/// Names built from the characters that actually break things: separators,
/// commas (which collide with the dot encoding), dots, and leading slashes.
fn adversarial_name() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-zA-Z0-9_.-]{1,24}",
        "[a-zA-Z0-9_.,/\\\\-]{0,24}",
        r"[./\\]{0,6}[a-z]{0,8}[./\\]{0,6}",
        "\\PC{0,24}",
    ]
}

/// hmm.json documents carrying the null/blank/whitespace optional-field shapes
/// that `de_blank_as_none` is responsible for normalizing.
fn hmm_json_document() -> impl Strategy<Value = String> {
    let optional = prop_oneof![
        Just("null".to_string()),
        Just("\"\"".to_string()),
        Just("\"   \"".to_string()),
        Just("\"\\t\\n\"".to_string()),
        Just("\"1.2.3\"".to_string()),
    ];
    let type_name = prop_oneof![
        Just("haxelib".to_string()),
        Just("git".to_string()),
        Just("dev".to_string()),
    ];

    proptest::collection::vec(
        (
            "[a-z][a-z0-9.]{0,8}",
            type_name,
            optional.clone(),
            optional.clone(),
            optional,
        ),
        0..5,
    )
    .prop_map(|entries| {
        let deps: Vec<String> = entries
            .iter()
            .map(|(name, ty, version, vcs_ref, dir)| {
                format!(
                    r#"{{"name":"{name}","type":"{ty}","version":{version},"ref":{vcs_ref},"dir":{dir}}}"#
                )
            })
            .collect();
        format!(r#"{{"dependencies":[{}]}}"#, deps.join(","))
    })
}

fn haxelib_type() -> impl Strategy<Value = HaxelibType> {
    prop_oneof![
        Just(HaxelibType::Haxelib),
        Just(HaxelibType::Git),
        Just(HaxelibType::Dev),
    ]
}

/// A `Haxelib` whose required fields are always populated, so `render_hxml`
/// and `save_json` both succeed.
fn well_formed_haxelib() -> impl Strategy<Value = Haxelib> {
    (
        "[a-zA-Z][a-zA-Z0-9_.-]{0,12}",
        haxelib_type(),
        "[0-9]{1,2}\\.[0-9]{1,2}\\.[0-9]{1,2}",
        "https://example\\.com/[a-z]{1,8}/[a-z]{1,8}",
        "[a-z0-9]{1,12}",
    )
        .prop_map(|(name, haxelib_type, version, url, vcs_ref)| Haxelib {
            name,
            dir: None,
            path: Some("some/path".to_string()),
            version: Some(version),
            url: Some(url),
            vcs_ref: Some(vcs_ref),
            haxelib_type,
        })
}

proptest! {
    /// parse_spec must never panic, whatever the input.
    #[test]
    fn parse_spec_never_panics(s in "\\PC{0,60}") {
        let _ = parse_spec(&s);
    }

    /// A successful parse is losslessly reconstructible from its parts, and
    /// never yields an empty name or version.
    #[test]
    fn parse_spec_result_reconstructs_input(s in "[a-zA-Z0-9_.@-]{0,24}") {
        match parse_spec(&s) {
            Ok((name, Some(version))) => {
                prop_assert!(!name.is_empty());
                prop_assert!(!version.is_empty());
                prop_assert_eq!(&s, &format!("{name}@{version}"));
            }
            Ok((name, None)) => {
                prop_assert!(!name.is_empty());
                prop_assert!(!name.contains('@'));
                prop_assert_eq!(&s, name);
            }
            Err(_) => {}
        }
    }

    /// A well-formed `name@version` spec splits into its two parts.
    #[test]
    fn parse_spec_splits_name_at_version(
        name in "[a-zA-Z0-9_.-]{1,16}",
        version in "[a-zA-Z0-9_.-]{1,16}",
    ) {
        let spec = format!("{name}@{version}");
        let (parsed_name, parsed_version) = parse_spec(&spec).unwrap();
        prop_assert_eq!(parsed_name, name.as_str());
        prop_assert_eq!(parsed_version, Some(version.as_str()));
    }

    /// The dots-to-commas filesystem encoding is invertible for valid library
    /// names. `validate_lib_name` enforces the haxelib charset `[A-Za-z0-9_.-]`
    /// (no commas), so this generator IS the valid-name set and the round-trip
    /// makes `lib_dir_path_for_name` injective over it: two distinct valid
    /// names can never share a `.haxelib/` directory.
    #[test]
    fn comma_encoding_round_trips(name in "[A-Za-z0-9_.-]{1,24}") {
        prop_assert!(validate_lib_name(&name).is_ok(), "charset name rejected: {:?}", name);
        let dir = lib_dir_path_for_name(&name);
        let encoded = dir.file_name().unwrap().to_str().unwrap().to_string();
        prop_assert!(!encoded.contains('.'));
        prop_assert_eq!(encoded.replace(',', "."), name);
    }

    /// Any name that passes validation maps to exactly `.haxelib/<one dir>`.
    /// Callers `remove_dir_all` this path, so escaping it is destructive.
    #[test]
    fn validated_names_stay_under_haxelib(name in adversarial_name()) {
        prop_assume!(validate_lib_name(&name).is_ok());

        let path = lib_dir_path_for_name(&name);
        prop_assert!(path.starts_with(".haxelib"), "{:?} escaped to {:?}", name, path);
        prop_assert!(
            path.components().all(|c| matches!(c, Component::Normal(_))),
            "{:?} produced a non-normal component: {:?}", name, path
        );
        prop_assert_eq!(path.components().count(), 2, "{:?} -> {:?}", name, path);
    }

    /// Validation never accepts a name carrying a separator, control char, or
    /// comma (commas would alias the dot encoding: `a,b` == `a.b` on disk).
    #[test]
    fn validation_rejects_separators_and_controls(name in adversarial_name()) {
        if name.contains('/') || name.contains('\\') || name.contains(char::is_control) || name.contains(',') {
            prop_assert!(validate_lib_name(&name).is_err(), "accepted {:?}", name);
        }
    }

    /// Validation is exactly the haxelib charset: non-empty and every char in
    /// `[A-Za-z0-9_.-]` (what haxelib's `Data.safe` requires). Names outside it
    /// can never work with `haxelib path`, and names inside it are shell-safe
    /// and encoding-safe.
    #[test]
    fn validation_is_exactly_the_haxelib_charset(name in adversarial_name()) {
        let in_charset = !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
        prop_assert_eq!(validate_lib_name(&name).is_ok(), in_charset, "name: {:?}", name);
    }

    /// Arbitrary bytes must never panic the hmm.json parser.
    #[test]
    fn hmm_json_parsing_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
        let _ = serde_json::from_slice::<Dependancies>(&bytes);
    }

    /// Anything that parses survives a serialize/re-parse cycle unchanged.
    /// Guards the blank-string-to-None normalization against emitting output
    /// it cannot read back.
    #[test]
    fn hmm_json_serialization_is_a_fixpoint(text in hmm_json_document()) {
        let deps = serde_json::from_str::<Dependancies>(&text)
            .expect("generated documents are well-formed");

        let out = serde_json::to_string(&deps).unwrap();
        let reparsed = serde_json::from_str::<Dependancies>(&out).unwrap();
        prop_assert_eq!(deps.dependencies.len(), reparsed.dependencies.len());
        prop_assert_eq!(&out, &serde_json::to_string(&reparsed).unwrap());
    }

    /// Blank and whitespace-only optional fields always normalize to `None`,
    /// matching original hmm's `parseOptionalStringProperty`.
    #[test]
    fn blank_optional_fields_normalize_to_none(text in hmm_json_document()) {
        let deps = serde_json::from_str::<Dependancies>(&text).unwrap();
        for lib in deps.dependencies.iter() {
            for field in [&lib.version, &lib.vcs_ref, &lib.dir] {
                if let Some(value) = field {
                    prop_assert!(!value.trim().is_empty(), "blank field survived: {:?}", lib);
                }
            }
        }
    }

    /// A sanitized zip entry is always strictly inside the destination dir.
    #[test]
    fn zip_entries_never_escape_destination(
        base_path in "[a-z/]{0,8}",
        entry_name in prop_oneof![
            "[a-zA-Z0-9_./\\\\-]{0,32}",
            r"[./\\]{0,8}[a-z]{0,8}[./\\]{0,8}[a-z]{0,8}",
            "\\PC{0,32}",
        ],
    ) {
        let dest = Path::new(".haxelib/lib/1,0,0");
        let Some(out) = sanitize_zip_entry(&base_path, &entry_name, dest) else { return Ok(()) };

        prop_assert!(out.starts_with(dest), "{:?} escaped to {:?}", entry_name, out);
        let extra = out.strip_prefix(dest).unwrap();
        prop_assert!(extra.components().count() > 0);
        prop_assert!(
            extra.components().all(|c| matches!(c, Component::Normal(_))),
            "{:?} produced a traversal component: {:?}", entry_name, out
        );
    }

    /// Decoding an untrusted registry reply must never panic. The `€` case
    /// covers the char-boundary slice that used to blow up on long non-ASCII
    /// error pages.
    #[test]
    fn remoting_response_never_panics(
        resp in prop_oneof!["\\PC{0,300}", "€{0,300}", "[a-z:%0-9]{0,64}"],
        name in "[a-z]{0,12}",
    ) {
        let _ = parse_remoting_response(&resp, &name);
    }

    /// hxml is line-oriented: exactly one `-lib` directive per dependency.
    #[test]
    fn render_hxml_emits_one_line_per_dependency(
        libs in proptest::collection::vec(well_formed_haxelib(), 0..8)
    ) {
        let count = libs.len();
        let deps = Dependancies { dependencies: libs };
        let hxml = render_hxml(&deps).unwrap();

        prop_assert_eq!(hxml.lines().count(), count);
        for line in hxml.lines() {
            prop_assert!(line.starts_with("-lib "), "unexpected hxml line {:?}", line);
        }
    }
}

proptest! {
    // Filesystem-touching prop: keep the case count small.
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// save_json -> read_json round-trips every entry in the order given.
    /// Covers git and dev entries and dotted names, not just plain haxelib deps.
    #[test]
    fn save_json_round_trips_in_order(
        libs in proptest::collection::vec(well_formed_haxelib(), 1..8)
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let json_path = tmp.path().join("hmm.json");

        let expected: Vec<(String, Option<String>)> = libs
            .iter()
            .map(|l| (l.name.clone(), l.version.clone()))
            .collect();

        json::save_json(Dependancies { dependencies: libs }, json_path.clone()).unwrap();

        let read_back = json::read_json(&json_path).unwrap();
        let actual: Vec<(String, Option<String>)> = read_back
            .dependencies
            .iter()
            .map(|d| (d.name.clone(), d.version.clone()))
            .collect();
        prop_assert_eq!(actual, expected);
    }

    /// Upserting a sequence of libs (with repeated names) one at a time reads
    /// back as: each name once, in first-occurrence order, with the last
    /// upserted value winning; and `dir: None` never appears in the file.
    #[test]
    fn upsert_dependencies_dedups_by_name_in_first_seen_order(
        libs in proptest::collection::vec(well_formed_haxelib(), 1..8)
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let json_path = tmp.path().join("hmm.json");
        json::save_json(Dependancies { dependencies: vec![] }, json_path.clone()).unwrap();

        let mut expected: Vec<(String, Option<String>)> = Vec::new();
        for lib in &libs {
            json::upsert_dependencies(&json_path, std::slice::from_ref(lib)).unwrap();
            match expected.iter_mut().find(|(n, _)| n == &lib.name) {
                Some(slot) => slot.1 = lib.version.clone(),
                None => expected.push((lib.name.clone(), lib.version.clone())),
            }
        }

        let read_back = json::read_json(&json_path).unwrap();
        let actual: Vec<(String, Option<String>)> = read_back
            .dependencies
            .iter()
            .map(|d| (d.name.clone(), d.version.clone()))
            .collect();
        prop_assert_eq!(actual, expected);

        let text = std::fs::read_to_string(&json_path).unwrap();
        prop_assert!(!text.contains("\"dir\""), "dir: None must be omitted, got: {}", text);
    }
}
