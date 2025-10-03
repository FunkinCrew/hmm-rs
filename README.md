# hmm-rs

A Rust implementation of Haxe Module Manager ([`hmm`](https://github.com/andywhite37/hmm))

# Installation

`hmm-rs` can be installed as a binary from crates.io: https://crates.io/crates/hmm-rs

With [Rust installed](https://www.rust-lang.org/tools/install):
`cargo install hmm-rs`

or directly from git:

`cargo install --git https://github.com/ninjamuffin99/hmm-rs hmm-rs`

## TODO List

The below is a broad todo list / notes for myself.

- [ ] Github actions
  - [x] Windows Build
  - [x] Linux Build
  - [x] Mac Build
  - [ ] Github Releases
- [ ] Create tests against haxe `hmm`

- [ ] Implement something to slowly roll out from hmm -> hmm-rs
  - idea is to be able to integrate hmm with hmm-rs version, so certain commands get aliased. maybe need to make something on hmm haxe version perhaps to alias things?

### hmm command and notes

- [ ] install - installs libraries listed in hmm.json
  - [x] haxelib: installs from lib.haxe.org
  - [ ] git: installs from a git based source
    - allow writing / initalizing non-empty directories for clones?
    - install with `--no-tags` for quicker install?
    - check to see if repo is shallow or not, or maybe do a fetch before?
    - support git tags
  - [ ] check if version is already installed
- [~] check: shows info about the currently installed library, and what we want based on the hmm.json
  - git tags are sorta funky, try using hxcpp or something perhaps
  - improve speed, i think the git status thing slows it down.
    - need to dig into profiling code...
- [ ] from-hxml
- [ ] reinstall
  - this should function the way that `hmm reinstall -f` would, where it force reinstalls everything. `hmm-rs install` should be used for cases when you updated your hmm.json manually or something
- [x] haxelib
- [ ] git
- [ ] hg
  - probably not planned since I don't use mecurial personally or know any haxelib repos that do!
- [ ] dev
- [ ] update
- [ ] remove
  - Add the command simply
  - create the .rs file
  - Remove the library from `hmm.json`
  - Remove the directory from `.haxelib` folder
- [x] lock
  - how much depth should this go to for dependencies?
