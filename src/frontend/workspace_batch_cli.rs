//! Borrow-only selector for explicit sequential workspace CHECK and TEST.
//! Ordinary package requests and the established manifest scanner are unchanged.
use super::{
    manifest,
    options::{Route, PROFILE_APPLICABILITY},
};

// Keep this scanner separate from manifest::cli's admitted fixed-size carrier.
// The first eight options follow the existing global-option ownership rules.
const OPTIONS: [&str; 13] = [
    "--manifest-path",
    "--edition",
    "--message-format",
    "--profile",
    "--entry-mode",
    "--experimental-hir-import",
    "--experimental-hir-producers",
    "--experimental-lexical-provider",
    "--package",
    "--engine",
    "--filter",
    "--workspace",
    "--fail-fast",
];

#[derive(Clone, Copy)]
enum Error<'a> {
    Duplicate(&'static str),
    Missing(&'static str),
    Edition(&'a str),
    Format(&'a str),
    Profile(&'a str),
    Entry(&'a str),
    Unsupported(&'a str),
    FlagValue(&'static str),
    Engine(&'a str),
}

impl Error<'_> {
    fn message(self) -> String {
        match self {
            Self::Duplicate(name) => format!("{name} may only be specified once"),
            Self::Missing(name) => format!("{name} requires a value"),
            Self::Edition(value) => {
                format!("unknown edition `{value}`; expected legacy-0.9 or typed-preview")
            }
            Self::Format(value) => {
                format!("unknown --message-format `{value}`; expected text or json")
            }
            Self::Profile(value) => {
                format!("unknown --profile `{value}`; expected dev or release")
            }
            Self::Entry(value) => {
                format!("unknown --entry-mode `{value}`; expected result or process")
            }
            Self::Unsupported(value) => {
                format!("unsupported option for workspace check/test: {value}")
            }
            Self::FlagValue(name) => format!("{name} does not accept a value"),
            Self::Engine(value) => {
                format!("unknown --engine `{value}`; expected interpreter or native")
            }
        }
    }
}

#[derive(Default)]
struct Scan<'a> {
    path: Option<&'a str>,
    package: Option<&'a str>,
    native: bool,
    command: Option<&'a str>,
    edition: Option<&'a str>,
    global_error: Option<Error<'a>>,
    local_error: Option<Error<'a>>,
    seen: u16,
    json: bool,
    before: bool,
    operands: bool,
    script_name: bool,
    separated: bool,
}

pub(super) fn select(args: &[String]) -> Option<Route> {
    let mut scan = Scan::default();
    let mut index = 0;
    while index < args.len() {
        if scan.script_name {
            break;
        }
        let argument = args[index].as_str();
        index += 1;
        if scan.separated {
            scan.operands = true;
            continue;
        }
        if argument == "--" {
            scan.separated = true;
            continue;
        }
        let slot = OPTIONS.iter().enumerate().position(|(slot, name)| {
            (slot < 8 || scan.command.is_some_and(|command| command != "script"))
                && (argument == *name
                    || argument
                        .strip_prefix(name)
                        .is_some_and(|suffix| suffix.starts_with('=')))
        });
        if let Some(slot) = slot {
            let name = OPTIONS[slot];
            let error = if slot < 8 {
                &mut scan.global_error
            } else {
                &mut scan.local_error
            };
            if scan.seen & (1 << slot) != 0 {
                error.get_or_insert(Error::Duplicate(name));
            }
            scan.seen |= 1 << slot;
            if slot == 0 && scan.command.is_none() {
                scan.before = true;
            }
            if slot >= 11 {
                if argument.contains('=') {
                    error.get_or_insert(Error::FlagValue(name));
                }
                continue;
            }
            let value = if let Some((_, value)) = argument.split_once('=') {
                Some(value)
            } else if let Some(next) = args.get(index).filter(|next| !next.starts_with('-')) {
                index += 1;
                Some(next.as_str())
            } else {
                None
            };
            match (slot, value) {
                (_, None | Some("")) => {
                    error.get_or_insert(Error::Missing(name));
                }
                (0, value) => scan.path = value,
                (1, Some(value @ ("typed-preview" | "legacy-0.9"))) => {
                    scan.edition = Some(value);
                }
                (1, Some(value)) => {
                    error.get_or_insert(Error::Edition(value));
                }
                (2, Some("json")) => scan.json = true,
                (2, Some("text")) => {}
                (2, Some(value)) => {
                    error.get_or_insert(Error::Format(value));
                }
                (3, Some("dev" | "release")) => {}
                (3, Some(value)) => {
                    error.get_or_insert(Error::Profile(value));
                }
                (4, Some("result" | "process")) => {}
                (4, Some(value)) => {
                    error.get_or_insert(Error::Entry(value));
                }
                (8, value) => scan.package = value,
                (9, Some("interpreter")) => {}
                (9, Some("native")) => scan.native = true,
                (9, Some(value)) => {
                    error.get_or_insert(Error::Engine(value));
                }
                _ => {}
            }
        } else if scan.command.is_none() {
            scan.command = Some(argument);
        } else if scan.command == Some("script") {
            scan.script_name = true;
        } else if argument.starts_with('-') {
            scan.local_error.get_or_insert(Error::Unsupported(argument));
        } else {
            scan.operands = true;
        }
    }
    // Only an explicit local workspace flag on CHECK/TEST opts in. Script
    // arguments and everything after `--` retain their established owner.
    if !matches!(scan.command, Some("check" | "test")) || scan.seen & (1 << 11) == 0 {
        return None;
    }
    let test = scan.command == Some("test");
    let message = if let Some(error) = scan.global_error.or(scan.local_error) {
        Some(error.message())
    } else if scan.before {
        Some("--manifest-path must follow check or test".into())
    } else if scan.seen & 1 == 0 {
        Some("--workspace requires --manifest-path".into())
    } else if scan.edition == Some("legacy-0.9") {
        Some("--workspace conflicts with --edition legacy-0.9".into())
    } else if scan.operands {
        Some("workspace check/test does not accept a source operand".into())
    } else if scan.package.is_some() {
        Some("--workspace conflicts with --package".into())
    } else if scan.seen & (1 << 3) != 0 {
        Some(PROFILE_APPLICABILITY.into())
    } else if scan.seen & ((1 << 4) | (1 << 5) | (1 << 6) | (1 << 7)) != 0 {
        Some("workspace check/test does not support entry-mode or experimental selectors".into())
    } else if scan.seen & (1 << 10) != 0 {
        Some("workspace test does not support --filter; select one --package instead".into())
    } else if scan.seen & (1 << 9) != 0 && (!test || scan.native) {
        Some("--workspace permits --engine=interpreter for test only".into())
    } else if scan.json {
        Some("workspace check/test does not support --message-format=json".into())
    } else {
        None
    };
    if let Some(message) = message {
        return Some(Route::WorkspaceBatchError {
            failure: manifest::Failure::Cli(message),
        });
    }
    Some(
        match manifest::format_request(scan.path.unwrap_or(""), None) {
            Ok(request) => Route::WorkspaceBatch {
                request,
                test,
                fail_fast: scan.seen & (1 << 12) != 0,
            },
            Err(failure) => Route::WorkspaceBatchError { failure },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(args: &[&str]) -> Option<Route> {
        select(&args.iter().map(|value| (*value).into()).collect::<Vec<_>>())
    }

    fn error(args: &[&str]) -> String {
        match route(args) {
            Some(Route::WorkspaceBatchError { failure }) => failure.message(),
            other => panic!("expected workspace error, got {other:?}"),
        }
    }

    #[test]
    fn explicit_check_and_interpreter_test_use_ordinary_requests() {
        for (args, test, fail_fast) in [
            (
                vec!["check", "--manifest-path", "missing", "--workspace"],
                false,
                false,
            ),
            (
                vec![
                    "--edition=typed-preview",
                    "test",
                    "--workspace",
                    "--manifest-path=missing",
                    "--engine=interpreter",
                    "--fail-fast",
                    "--message-format=text",
                ],
                true,
                true,
            ),
            (
                vec![
                    "check",
                    "--workspace",
                    "--fail-fast",
                    "--manifest-path=missing",
                ],
                false,
                true,
            ),
        ] {
            assert!(matches!(
                route(&args),
                Some(Route::WorkspaceBatch { test: actual_test, fail_fast: actual_fast, .. })
                    if actual_test == test && actual_fast == fail_fast
            ));
        }
    }

    #[test]
    fn incompatible_options_fail_without_manifest_io() {
        for (tail, expected) in [
            (
                vec!["--workspace"],
                "--workspace may only be specified once",
            ),
            (
                vec!["--workspace=true"],
                "--workspace may only be specified once",
            ),
            (vec!["--package=app"], "conflicts with --package"),
            (vec!["--engine=native"], "permits --engine=interpreter"),
            (vec!["--engine=other"], "unknown --engine"),
            (vec!["--profile=dev"], "--profile"),
            (vec!["--filter=test_"], "does not support --filter"),
            (
                vec!["--message-format=json"],
                "does not support --message-format=json",
            ),
            (vec!["--entry-mode=result"], "does not support entry-mode"),
            (
                vec!["--experimental-hir-import=x"],
                "experimental selectors",
            ),
            (
                vec!["--experimental-hir-producers=x"],
                "experimental selectors",
            ),
            (
                vec!["--experimental-lexical-provider=x"],
                "experimental selectors",
            ),
            (vec!["--edition=legacy-0.9"], "conflicts with --edition"),
            (vec!["--output=x"], "unsupported option"),
            (vec!["source.ox"], "does not accept a source operand"),
            (vec!["--fail-fast=true"], "does not accept a value"),
            (
                vec!["--fail-fast", "--fail-fast"],
                "may only be specified once",
            ),
        ] {
            let mut args = vec!["test", "--manifest-path=missing", "--workspace"];
            args.extend(tail);
            assert!(error(&args).contains(expected), "{args:?}");
        }
        assert!(error(&["check", "--workspace"]).contains("requires --manifest-path"));
        assert!(
            error(&["test", "--manifest-path=missing", "--workspace=false"])
                .contains("does not accept a value")
        );
        assert!(error(&[
            "check",
            "--workspace",
            "--manifest-path=missing",
            "--engine=interpreter"
        ])
        .contains("for test only"));
        assert!(error(&[
            "test",
            "--package=app",
            "--manifest-path=missing",
            "--workspace"
        ])
        .contains("conflicts with --package"));
        assert!(error(&["--manifest-path=missing", "test", "--workspace"]).contains("must follow"));
    }

    #[test]
    fn global_errors_keep_priority_and_options_keep_their_argument_owner() {
        assert!(error(&[
            "test",
            "--manifest-path=missing",
            "--workspace",
            "--package=app",
            "--edition=bad",
        ])
        .starts_with("unknown edition"));
        assert!(error(&["test", "--manifest-path", "--workspace"])
            .contains("--manifest-path requires a value"));
        for args in [
            vec!["test", "--manifest-path=missing"],
            vec!["test", "--manifest-path=missing", "--package=app"],
            vec!["new", "w", "--edition=typed-preview", "--workspace"],
            vec!["run", "--manifest-path=missing", "--workspace"],
            vec!["script", "test", "--manifest-path=missing", "--workspace"],
            vec!["test", "--manifest-path=missing", "--", "--workspace"],
            vec!["test", "--manifest-path=--workspace"],
            vec!["test", "--manifest-path=missing", "--filter=--workspace"],
        ] {
            assert!(route(&args).is_none(), "{args:?}");
        }
    }
}
