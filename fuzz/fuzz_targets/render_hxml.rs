#![no_main]

use arbitrary::Arbitrary;
use hmm_rs::commands::tohxml_command::render_hxml;
use hmm_rs::hmm::dependencies::Dependancies;
use hmm_rs::hmm::haxelib::{Haxelib, HaxelibType};
use libfuzzer_sys::fuzz_target;

// `Haxelib` lives in the main crate, which deliberately does not depend on
// `arbitrary`. Generate a local mirror and map it over instead.
#[derive(Arbitrary, Debug)]
enum FuzzType {
    Git,
    Haxelib,
    Dev,
    Mercurial,
}

#[derive(Arbitrary, Debug)]
struct FuzzLib {
    name: String,
    haxelib_type: FuzzType,
    dir: Option<String>,
    vcs_ref: Option<String>,
    path: Option<String>,
    url: Option<String>,
    version: Option<String>,
}

fn to_haxelib(lib: &FuzzLib) -> Haxelib {
    Haxelib {
        name: lib.name.clone(),
        haxelib_type: match lib.haxelib_type {
            FuzzType::Git => HaxelibType::Git,
            FuzzType::Haxelib => HaxelibType::Haxelib,
            FuzzType::Dev => HaxelibType::Dev,
            FuzzType::Mercurial => HaxelibType::Mecurial,
        },
        dir: lib.dir.clone(),
        vcs_ref: lib.vcs_ref.clone(),
        path: lib.path.clone(),
        url: lib.url.clone(),
        version: lib.version.clone(),
    }
}

fn has_line_break(s: &str) -> bool {
    s.contains('\n') || s.contains('\r')
}

fuzz_target!(|libs: Vec<FuzzLib>| {
    let deps = Dependancies {
        dependencies: libs.iter().map(to_haxelib).collect(),
    };

    let Ok(hxml) = render_hxml(&deps) else {
        return;
    };

    // hxml is line-oriented: one `-lib` directive per dependency. That only
    // holds if no field smuggles a line break into the output, which is what
    // `validate_lib_name` guarantees for names reaching this code for real.
    let no_breaks = libs.iter().all(|l| {
        !has_line_break(&l.name)
            && l.url.as_deref().is_none_or(|s| !has_line_break(s))
            && l.vcs_ref.as_deref().is_none_or(|s| !has_line_break(s))
            && l.version.as_deref().is_none_or(|s| !has_line_break(s))
    });

    if no_breaks {
        assert_eq!(
            hxml.lines().count(),
            deps.dependencies.len(),
            "line count diverged from dependency count: {hxml:?}"
        );
        for line in hxml.lines() {
            assert!(line.starts_with("-lib "), "unexpected hxml line {line:?}");
        }
    }
});
