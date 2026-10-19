//! Root help documents the typed project route without invoking the toolchain.
use std::process::{Command, Stdio};

#[test]
fn root_help_lists_typed_projects_and_preserves_legacy_usage() {
    let mut previous = None;
    for args in [&[][..], &["help"][..], &["--help"][..], &["-h"][..]] {
        let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .stdin(Stdio::null())
            .output()
            .expect("run root help");
        assert_eq!(output.status.code(), Some(0), "{args:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{args:?}: {output:?}");
        let help = String::from_utf8(output.stdout).unwrap();
        for expected in [
            "oxid run <file.ox|file.oxb>",
            "oxid compile <file.ox> [-o app.oxb]",
            "oxid build [--offline|--locked|--frozen]",
            "--edition legacy-0.9 keeps the default legacy route",
            "oxid new <project-path> --edition typed-preview",
            "oxid check --manifest-path <oxid.toml>",
            "oxid build --manifest-path <oxid.toml>",
            "oxid run --manifest-path <oxid.toml> [--engine interpreter|native]",
            "oxid test --manifest-path <oxid.toml> [--engine interpreter|native]",
            "oxid package --manifest-path <oxid.toml> --output <archive.tar>",
            "oxid doctor --edition typed-preview",
            "--package <name>",
            "[--filter <text>]",
            "--entry-mode result|process",
            "Entry mode defaults to result",
            "run/test default to the interpreter",
            "x86_64-unknown-linux-gnu",
            "LLVM 19.1.7",
            "OXID_LLVM_BIN",
            "clang-19, opt-19, ld.lld-19",
        ] {
            assert!(help.contains(expected), "missing {expected:?}: {help}");
        }
        assert!(!help.contains("typed-preview is check-only"));
        if let Some(previous) = &previous {
            assert_eq!(&help, previous, "root help aliases differ");
        }
        previous = Some(help);
    }
}
