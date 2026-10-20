//! Borrow-only selector for explicit manifest formatting. It does not widen the
//! established manifest scanner and leaves source/legacy formatter routes alone.
use super::{
    manifest,
    options::{Route, PROFILE_APPLICABILITY},
};

// Keep this scanner separate from manifest::cli's admitted fixed-size carrier.
// The first eight options follow the existing global-option ownership rules.
const OPTIONS: [&str; 11] = [
    "--manifest-path",
    "--edition",
    "--message-format",
    "--profile",
    "--entry-mode",
    "--experimental-hir-import",
    "--experimental-hir-producers",
    "--experimental-lexical-provider",
    "--package",
    "--check",
    "--write",
];
const MODE_REQUIRED: &str = "manifest fmt requires exactly one of --check or --write";

#[derive(Clone, Copy)]
enum Error<'a> {
    Duplicate(&'static str),
    Missing(&'static str),
    UnexpectedValue(&'static str),
    Edition(&'a str),
    Format(&'a str),
    Profile(&'a str),
    Entry(&'a str),
    Unsupported(&'a str),
    Mode,
}

impl Error<'_> {
    fn message(self) -> String {
        match self {
            Self::Duplicate(name) => format!("{name} may only be specified once"),
            Self::Missing(name) => format!("{name} requires a value"),
            Self::UnexpectedValue(name) => format!("{name} does not accept a value"),
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
            Self::Unsupported(value) => format!("unsupported option for manifest fmt: {value}"),
            Self::Mode => MODE_REQUIRED.into(),
        }
    }
}

#[derive(Default)]
struct Scan<'a> {
    path: Option<&'a str>,
    package: Option<&'a str>,
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
            if slot == 9 || slot == 10 {
                if argument.contains('=') {
                    error.get_or_insert(Error::UnexpectedValue(name));
                }
                if scan.seen & (1 << 9) != 0 && scan.seen & (1 << 10) != 0 {
                    error.get_or_insert(Error::Mode);
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
    // Claim only a literal fmt command with an explicit, in-scope selector.
    // In particular, no single-file command, script argument or `--` operand
    // can enter the project formatter, even if it looks like an option.
    if scan.command != Some("fmt") || scan.seen & 1 == 0 {
        return None;
    }
    let message = if let Some(error) = scan.global_error.or(scan.local_error) {
        Some(error.message())
    } else if scan.before {
        Some("--manifest-path must follow fmt".into())
    } else if scan.edition == Some("legacy-0.9") {
        Some("manifest fmt conflicts with --edition legacy-0.9".into())
    } else if scan.operands {
        Some("manifest fmt does not accept a source operand".into())
    } else if scan.seen & (1 << 3) != 0 {
        Some(PROFILE_APPLICABILITY.into())
    } else if scan.seen & ((1 << 4) | (1 << 5) | (1 << 6) | (1 << 7)) != 0 {
        Some("manifest fmt does not support entry-mode or experimental selectors".into())
    } else if scan.json {
        Some("manifest fmt does not support --message-format=json".into())
    } else if scan.seen & ((1 << 9) | (1 << 10)) == 0 {
        Some(MODE_REQUIRED.into())
    } else {
        None
    };
    if let Some(message) = message {
        return Some(Route::FormatError { message });
    }
    Some(
        match manifest::format_request(scan.path.unwrap_or(""), scan.package) {
            Ok(request) => Route::ManifestFormat {
                request,
                write: scan.seen & (1 << 10) != 0,
            },
            Err(failure) => Route::ManifestFormatError { failure },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::options;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).into()).collect()
    }

    fn refusal(words: &[&str], expected: &str) {
        let Route::FormatError { message } = options::route(&args(words)) else {
            panic!("expected formatter refusal: {words:?}");
        };
        assert_eq!(message, expected, "{words:?}");
    }

    #[test]
    fn explicit_modes_reuse_manifest_selection_without_an_edition_flag() {
        for (mode, write) in [("--check", false), ("--write", true)] {
            for words in [
                vec!["fmt", "--manifest-path", "workspace/oxid.toml", mode],
                vec!["fmt", mode, "--manifest-path=workspace/oxid.toml"],
                vec![
                    "--edition=typed-preview",
                    "fmt",
                    "--package=app",
                    "--manifest-path=workspace/oxid.toml",
                    mode,
                    "--message-format=text",
                ],
                vec![
                    "--message-format",
                    "text",
                    "fmt",
                    mode,
                    "--manifest-path",
                    "workspace/oxid.toml",
                    "--package",
                    "app",
                    "--edition",
                    "typed-preview",
                ],
            ] {
                let Route::ManifestFormat {
                    request,
                    write: actual_write,
                } = options::route(&args(&words))
                else {
                    panic!("expected project formatter route: {words:?}");
                };
                let package = words
                    .iter()
                    .any(|word| word.starts_with("--package"))
                    .then_some("app");
                assert_eq!(
                    request,
                    manifest::format_request("workspace/oxid.toml", package).unwrap()
                );
                assert!(!request[0].json);
                assert_eq!(actual_write, write);
            }
        }
    }

    #[test]
    fn modes_are_explicit_exclusive_valueless_and_not_repeatable() {
        for (tail, expected) in [
            (vec![], MODE_REQUIRED),
            (vec!["--check", "--write"], MODE_REQUIRED),
            (vec!["--write", "--check"], MODE_REQUIRED),
            (
                vec!["--check", "--check"],
                "--check may only be specified once",
            ),
            (
                vec!["--write", "--write"],
                "--write may only be specified once",
            ),
            (vec!["--check=true"], "--check does not accept a value"),
            (vec!["--write="], "--write does not accept a value"),
            (vec!["--write=false"], "--write does not accept a value"),
            (
                vec!["--write", "--write="],
                "--write may only be specified once",
            ),
            (
                vec!["--check=", "--check"],
                "--check does not accept a value",
            ),
        ] {
            let mut words = vec!["fmt", "--manifest-path=missing"];
            words.extend(tail);
            refusal(&words, expected);
        }
    }

    #[test]
    fn selector_syntax_global_priority_and_unsupported_options_are_deterministic() {
        for (tail, expected) in [
            (vec!["--package"], "--package requires a value"),
            (vec!["--package="], "--package requires a value"),
            (
                vec!["--package=a", "--package=b"],
                "--package may only be specified once",
            ),
            (
                vec!["--manifest-path=x"],
                "--manifest-path may only be specified once",
            ),
            (
                vec!["--edition=legacy-0.9"],
                "manifest fmt conflicts with --edition legacy-0.9",
            ),
            (
                vec!["--unknown", "--edition=wrong"],
                "unknown edition `wrong`; expected legacy-0.9 or typed-preview",
            ),
            (
                vec!["--package=", "--profile=wrong"],
                "unknown --profile `wrong`; expected dev or release",
            ),
            (vec!["--profile=dev"], PROFILE_APPLICABILITY),
            (vec!["--profile=release"], PROFILE_APPLICABILITY),
            (vec!["--profile"], "--profile requires a value"),
            (
                vec!["--profile=dev", "--profile=release"],
                "--profile may only be specified once",
            ),
            (
                vec!["--edition=typed-preview", "--edition=typed-preview"],
                "--edition may only be specified once",
            ),
            (
                vec!["--message-format=json"],
                "manifest fmt does not support --message-format=json",
            ),
            (
                vec!["--message-format=xml"],
                "unknown --message-format `xml`; expected text or json",
            ),
            (
                vec!["--message-format=text", "--message-format=json"],
                "--message-format may only be specified once",
            ),
            (
                vec!["--entry-mode=process"],
                "manifest fmt does not support entry-mode or experimental selectors",
            ),
            (
                vec!["--experimental-hir-import=observation"],
                "manifest fmt does not support entry-mode or experimental selectors",
            ),
            (
                vec!["--backend=llvm"],
                "unsupported option for manifest fmt: --backend=llvm",
            ),
            (
                vec!["--engine=interpreter"],
                "unsupported option for manifest fmt: --engine=interpreter",
            ),
            (
                vec!["--target=x86_64-unknown-linux-gnu"],
                "unsupported option for manifest fmt: --target=x86_64-unknown-linux-gnu",
            ),
            (
                vec!["--output=result"],
                "unsupported option for manifest fmt: --output=result",
            ),
            (
                vec!["--json"],
                "unsupported option for manifest fmt: --json",
            ),
            (
                vec!["source.ox"],
                "manifest fmt does not accept a source operand",
            ),
            (
                vec!["--", "--write"],
                "manifest fmt does not accept a source operand",
            ),
        ] {
            let mut words = vec!["fmt", "--manifest-path=missing", "--check"];
            words.extend(tail);
            refusal(&words, expected);
        }
        refusal(
            &["fmt", "--manifest-path", "--check"],
            "--manifest-path requires a value",
        );
        refusal(
            &["--manifest-path=missing", "fmt", "--check"],
            "--manifest-path must follow fmt",
        );
    }

    #[test]
    fn scripts_separators_and_existing_source_formatting_keep_ownership() {
        for words in [
            vec!["fmt"],
            vec!["fmt", "--write", "legacy.ox"],
            vec!["fmt", "--", "--manifest-path=p", "--check"],
            vec!["script", "fmt", "--manifest-path=p", "--write"],
            vec!["script", "--write", "--manifest-path=p"],
            vec!["script", "task", "--profile=wrong", "--manifest-path=p"],
            vec!["check", "--manifest-path=p"],
        ] {
            assert!(select(&args(&words)).is_none(), "{words:?}");
        }
        for words in [
            vec!["fmt"],
            vec!["fmt", "--write", "legacy.ox"],
            vec!["fmt", "--", "--manifest-path=p", "--check"],
            vec!["script", "fmt", "--manifest-path=p", "--write"],
        ] {
            let arguments = args(&words);
            assert_eq!(options::route(&arguments), Route::Legacy(arguments));
        }
        assert_eq!(
            options::route(&args(&[
                "fmt",
                "--edition=typed-preview",
                "--check",
                "input.ox",
            ])),
            Route::TypedFormat {
                path: "input.ox".into(),
                check: true,
            }
        );
        assert_eq!(
            options::route(&args(&[
                "fmt",
                "--edition=typed-preview",
                "--",
                "--manifest-path=p",
            ])),
            Route::TypedFormat {
                path: "--manifest-path=p".into(),
                check: false,
            }
        );
        refusal(
            &["fmt", "--edition=typed-preview", "--write", "input.ox"],
            "unsupported option for typed-preview fmt: --write",
        );
    }

    #[test]
    fn manifest_request_limits_and_route_layout_remain_in_force() {
        let long_path = format!("--manifest-path={}", "p".repeat(4097));
        let Route::ManifestFormatError { failure } =
            options::route(&args(&["fmt", &long_path, "--check"]))
        else {
            panic!("existing manifest path bound must apply");
        };
        assert_eq!(failure.classification(), ("E0400", "manifest-input"));
        assert_eq!(failure.message(), "manifest path exceeds 4096 bytes");
        let package = format!("--package={}", "p".repeat(65));
        let Route::ManifestFormatError { failure } =
            options::route(&args(&["fmt", "--manifest-path=p", "--write", &package]))
        else {
            panic!("existing package name bound must apply");
        };
        assert_eq!(failure.message(), "--package name exceeds 64 bytes");
        #[cfg(target_pointer_width = "64")]
        assert_eq!(std::mem::size_of::<Route>(), 56);
    }
}
