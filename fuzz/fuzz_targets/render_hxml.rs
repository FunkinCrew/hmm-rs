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

    let hxml = render_hxml(&deps);

    // hxml is line-oriented: one `-lib` directive per dependency. Only the
    // fields a line is built from may break it (`validate_lib_name` keeps line
    // breaks out of real names): a haxelib dep's version, and the url and ref
    // of a git dep without `dir`. A git dep with `dir` is a bare name, so a
    // line break in its url or ref must not change the line count.
    let clean = |s: &Option<String>| s.as_deref().is_none_or(|s| !has_line_break(s));
    let no_breaks = libs.iter().all(|l| {
        !has_line_break(&l.name)
            && match l.haxelib_type {
                FuzzType::Haxelib => clean(&l.version),
                FuzzType::Git => l.dir.is_some() || (clean(&l.url) && clean(&l.vcs_ref)),
                FuzzType::Dev | FuzzType::Mercurial => true,
            }
    });

    if no_breaks {
        assert_eq!(
            hxml.lines().count(),
            deps.dependencies.len(),
            "line count diverged from dependency count: {hxml:?}"
        );
        for (lib, line) in libs.iter().zip(hxml.lines()) {
            assert!(line.starts_with("-lib "), "unexpected hxml line {line:?}");
            // The bare name is the only form `haxelib path` resolves through
            // `.dev`, which a git dep with `dir` relies on.
            if matches!(lib.haxelib_type, FuzzType::Git) && lib.dir.is_some() {
                assert_eq!(line, format!("-lib {}", lib.name), "git dep with dir must stay bare");
            }
        }
    }
});
