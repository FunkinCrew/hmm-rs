#![no_main]

use hmm_rs::commands::install_command::parse_git_progress;
use libfuzzer_sys::fuzz_target;

// git's stderr is parsed one segment at a time to drive the install progress
// bar. A parse must never panic, whatever it accepts must carry a title made
// of plain words (it is displayed as the bar's phase), and a line rendered
// from the parsed parts the way git prints them must parse back to the same
// thing.
fuzz_target!(|line: &str| {
    let Some(p) = parse_git_progress(line) else {
        return;
    };

    assert!(!p.title.is_empty(), "empty title from {line:?}");
    assert!(
        p.title.bytes().all(|b| b.is_ascii_alphabetic() || b == b' '),
        "odd title {:?} from {line:?}",
        p.title
    );

    let done = if p.done { ", done." } else { "" };
    let rendered = match p.total {
        Some(total) => {
            let percent = if total == 0 {
                0
            } else {
                p.current as u128 * 100 / total as u128
            };
            format!(
                "{}: {percent}% ({}/{total}){}{done}",
                p.title, p.current, p.detail
            )
        }
        None => format!("{}: {}{}{done}", p.title, p.current, p.detail),
    };
    assert_eq!(
        parse_git_progress(&rendered).as_ref(),
        Some(&p),
        "round trip of {line:?} via {rendered:?}"
    );
});
