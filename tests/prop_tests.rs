use hmm_rs::commands::haxelib_command::parse_spec;
use hmm_rs::hmm::dependencies::Dependancies;
use hmm_rs::hmm::haxelib::{lib_dir_path_for_name, Haxelib, HaxelibType};
use hmm_rs::hmm::json;
use proptest::prelude::*;

proptest! {
    /// parse_spec must never panic, whatever the input.
    #[test]
    fn parse_spec_never_panics(s in ".{0,60}") {
        let _ = parse_spec(&s);
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
    /// names (which can never contain commas).
    #[test]
    fn comma_encoding_round_trips(name in "[A-Za-z0-9_.-]{1,24}") {
        let dir = lib_dir_path_for_name(&name);
        let encoded = dir.file_name().unwrap().to_str().unwrap().to_string();
        prop_assert!(!encoded.contains('.'));
        prop_assert_eq!(encoded.replace(',', "."), name);
    }
}

proptest! {
    // Filesystem-touching prop: keep the case count small.
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// save_json -> read_json round-trips every entry and always emits them
    /// sorted case-insensitively by name.
    #[test]
    fn save_json_round_trips_and_sorts(
        libs in proptest::collection::vec(
            ("[a-zA-Z][a-zA-Z0-9_-]{0,10}", "[0-9]\\.[0-9]\\.[0-9]"),
            1..8,
        )
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let json_path = tmp.path().join("hmm.json");

        let deps = Dependancies {
            dependencies: libs
                .iter()
                .map(|(name, version)| Haxelib {
                    name: name.clone(),
                    haxelib_type: HaxelibType::Haxelib,
                    dir: None,
                    vcs_ref: None,
                    path: None,
                    url: None,
                    version: Some(version.clone()),
                })
                .collect(),
        };
        json::save_json(deps, json_path.clone()).unwrap();

        let read_back = json::read_json(&json_path).unwrap();
        prop_assert_eq!(read_back.dependencies.len(), libs.len());

        let names: Vec<String> = read_back
            .dependencies
            .iter()
            .map(|d| d.name.clone())
            .collect();
        let mut sorted = names.clone();
        sorted.sort_by_key(|n| n.to_lowercase());
        prop_assert_eq!(&names, &sorted, "entries must be sorted case-insensitively");

        let mut expected: Vec<(String, String)> = libs.clone();
        let mut actual: Vec<(String, String)> = read_back
            .dependencies
            .iter()
            .map(|d| (d.name.clone(), d.version.clone().unwrap()))
            .collect();
        expected.sort();
        actual.sort();
        prop_assert_eq!(actual, expected);
    }
}
