//! Golden test: `--version` prints exactly one stable line.
//!
//! The line is pinned because the estate's theodolite bridge matches
//! `expect_version` against a probe's complete output (bridge config
//! law: an exact full-string match). Any change to the version output
//! is a deliberate, reviewed change, never an accident.

use std::process::Command;

/// The version flag succeeds and prints the pinned one-liner.
#[test]
fn version_flag_prints_exactly_one_line() {
    let output = Command::new(env!("CARGO_BIN_EXE_verse-ls"))
        .arg("--version")
        .output()
        .expect("the verse-ls binary runs");
    assert!(output.status.success(), "--version exits successfully");
    let stdout = String::from_utf8(output.stdout).expect("--version output is UTF-8");
    assert_eq!(
        stdout,
        concat!("verse-ls ", env!("CARGO_PKG_VERSION"), "\n")
    );
}

/// An unknown flag is a usage error: exit code 2, nothing on stdout.
#[test]
fn unknown_flag_is_a_usage_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_verse-ls"))
        .arg("--frobnicate")
        .output()
        .expect("the verse-ls binary runs");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "usage errors print nothing to stdout"
    );
}
