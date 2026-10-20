//! Borrow-only selector for explicit manifest doctor. The established manifest
//! scanner, legacy doctor and standalone typed toolchain doctor stay unchanged.
use super::{
    manifest,
    options::{Route, PROFILE_APPLICABILITY},
};

// Keep this scanner separate from manifest::cli's admitted fixed-size carrier.
// The first eight options follow the existing global-option ownership rules.
const OPTIONS: [&str; 10] = [
    "--manifest-path",
    "--edition",
    "--message-format",
    "--profile",
    "--entry-mode",
    "--experimental-hir-import",
    "--experimental-hir-producers",
    "--experimental-lexical-provider",
    "--package",
    "--native",
];

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
            Self::Unsupported(value) => format!("unsupported option for manifest doctor: {value}"),
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
            if slot == 9 {
                if argument.contains('=') {
                    error.get_or_insert(Error::UnexpectedValue(name));
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
    // Only the literal doctor command with an in-scope manifest selector opts
    // in. Script arguments and `--` operands cannot enter project doctor.
    if scan.command != Some("doctor") || scan.seen & 1 == 0 {
        return None;
    }
    let message = if let Some(error) = scan.global_error.or(scan.local_error) {
        Some(error.message())
    } else if scan.before {
        Some("--manifest-path must follow doctor".into())
    } else if scan.edition == Some("legacy-0.9") {
        Some("manifest doctor conflicts with --edition legacy-0.9".into())
    } else if scan.operands {
        Some("manifest doctor does not accept a source operand".into())
    } else if scan.seen & (1 << 3) != 0 {
        Some(PROFILE_APPLICABILITY.into())
    } else if scan.seen & ((1 << 4) | (1 << 5) | (1 << 6) | (1 << 7)) != 0 {
        Some("manifest doctor does not support entry-mode or experimental selectors".into())
    } else if scan.json {
        Some("manifest doctor does not support --message-format=json".into())
    } else {
        None
    };
    if let Some(message) = message {
        return Some(Route::ManifestDoctorError {
            failure: manifest::Failure::Cli(message),
        });
    }
    Some(
        match manifest::doctor_request(scan.path.unwrap_or(""), scan.package) {
            Ok(request) => Route::ManifestDoctor {
                request,
                native: scan.seen & (1 << 9) != 0,
            },
            Err(failure) => Route::ManifestDoctorError { failure },
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
        let Route::ManifestDoctorError { failure } = options::route(&args(words)) else {
            panic!("expected project doctor refusal: {words:?}");
        };
        assert_eq!(failure.classification(), ("E0001", "cli"), "{words:?}");
        assert_eq!(failure.message(), expected, "{words:?}");
    }

    #[test]
    fn explicit_manifest_selection_is_text_only_and_native_is_opt_in() {
        for (words, package, native) in [
            (
                vec!["doctor", "--manifest-path", "workspace/oxid.toml"],
                None,
                false,
            ),
            (
                vec!["doctor", "--native", "--manifest-path=workspace/oxid.toml"],
                None,
                true,
            ),
            (
                vec![
                    "--edition=typed-preview",
                    "doctor",
                    "--package=app",
                    "--manifest-path=workspace/oxid.toml",
                    "--message-format=text",
                ],
                Some("app"),
                false,
            ),
            (
                vec![
                    "--message-format",
                    "text",
                    "doctor",
                    "--native",
                    "--manifest-path",
                    "workspace/oxid.toml",
                    "--package",
                    "app",
                    "--edition",
                    "typed-preview",
                ],
                Some("app"),
                true,
            ),
            (
                vec![
                    "doctor",
                    "--manifest-path=workspace/oxid.toml",
                    "--message-format=text",
                    "--",
                ],
                None,
                false,
            ),
        ] {
            let Route::ManifestDoctor {
                request,
                native: actual_native,
            } = options::route(&args(&words))
            else {
                panic!("expected project doctor route: {words:?}");
            };
            assert_eq!(
                request,
                manifest::doctor_request("workspace/oxid.toml", package).unwrap()
            );
            assert!(!request[0].json);
            assert_eq!(actual_native, native, "{words:?}");
        }
    }

    #[test]
    fn native_is_valueless_and_not_repeatable() {
        for (tail, expected) in [
            (
                vec!["--native", "--native"],
                "--native may only be specified once",
            ),
            (vec!["--native=true"], "--native does not accept a value"),
            (vec!["--native="], "--native does not accept a value"),
            (vec!["--native=false"], "--native does not accept a value"),
            (
                vec!["--native", "--native="],
                "--native may only be specified once",
            ),
            (
                vec!["--native=", "--native"],
                "--native does not accept a value",
            ),
            (
                vec!["--native", "true"],
                "manifest doctor does not accept a source operand",
            ),
        ] {
            let mut words = vec!["doctor", "--manifest-path=missing"];
            words.extend(tail);
            refusal(&words, expected);
        }
    }

    #[test]
    fn selectors_and_global_error_priority_are_deterministic() {
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
                "manifest doctor conflicts with --edition legacy-0.9",
            ),
            (
                vec!["--unknown", "--edition=wrong"],
                "unknown edition `wrong`; expected legacy-0.9 or typed-preview",
            ),
            (
                vec!["--package=", "--profile=wrong"],
                "unknown --profile `wrong`; expected dev or release",
            ),
            (
                vec!["--native=true", "--message-format=xml"],
                "unknown --message-format `xml`; expected text or json",
            ),
            (
                vec!["--unknown", "--entry-mode=wrong"],
                "unknown --entry-mode `wrong`; expected result or process",
            ),
            (
                vec!["--edition=typed-preview", "--edition=typed-preview"],
                "--edition may only be specified once",
            ),
            (
                vec!["--message-format=text", "--message-format=json"],
                "--message-format may only be specified once",
            ),
            (vec!["--profile"], "--profile requires a value"),
            (
                vec!["--profile=dev", "--profile=release"],
                "--profile may only be specified once",
            ),
        ] {
            let mut words = vec!["doctor", "--manifest-path=missing"];
            words.extend(tail);
            refusal(&words, expected);
        }
        for words in [
            vec!["doctor", "--manifest-path"],
            vec!["doctor", "--manifest-path="],
            vec!["doctor", "--manifest-path", "--native"],
        ] {
            refusal(&words, "--manifest-path requires a value");
        }
        refusal(
            &["--manifest-path=missing", "doctor"],
            "--manifest-path must follow doctor",
        );
        refusal(
            &["--manifest-path", "missing", "doctor", "--native"],
            "--manifest-path must follow doctor",
        );
    }

    #[test]
    fn inapplicable_selectors_and_operands_fail_before_manifest_io() {
        for (tail, expected) in [
            (vec!["--profile=dev"], PROFILE_APPLICABILITY),
            (vec!["--profile", "release"], PROFILE_APPLICABILITY),
            (
                vec!["--message-format=json"],
                "manifest doctor does not support --message-format=json",
            ),
            (
                vec!["--entry-mode=result"],
                "manifest doctor does not support entry-mode or experimental selectors",
            ),
            (
                vec!["--entry-mode", "process"],
                "manifest doctor does not support entry-mode or experimental selectors",
            ),
            (
                vec!["--experimental-hir-import=observation"],
                "manifest doctor does not support entry-mode or experimental selectors",
            ),
            (
                vec!["--experimental-hir-producers", "bundle"],
                "manifest doctor does not support entry-mode or experimental selectors",
            ),
            (
                vec!["--experimental-lexical-provider=bundle"],
                "manifest doctor does not support entry-mode or experimental selectors",
            ),
            (
                vec!["--backend=llvm"],
                "unsupported option for manifest doctor: --backend=llvm",
            ),
            (
                vec!["--engine", "native"],
                "unsupported option for manifest doctor: --engine",
            ),
            (
                vec!["--target=x86_64-unknown-linux-gnu"],
                "unsupported option for manifest doctor: --target=x86_64-unknown-linux-gnu",
            ),
            (
                vec!["--output=result"],
                "unsupported option for manifest doctor: --output=result",
            ),
            (
                vec!["-o", "result"],
                "unsupported option for manifest doctor: -o",
            ),
            (
                vec!["--json"],
                "unsupported option for manifest doctor: --json",
            ),
            (
                vec!["source.ox"],
                "manifest doctor does not accept a source operand",
            ),
            (
                vec!["--", "--native"],
                "manifest doctor does not accept a source operand",
            ),
        ] {
            let mut words = vec!["doctor", "--manifest-path=missing"];
            words.extend(tail);
            refusal(&words, expected);
        }
    }

    #[test]
    fn scripts_separators_and_existing_doctor_routes_keep_ownership() {
        for words in [
            vec!["doctor"],
            vec!["doctor", "--native"],
            vec!["doctor", "--edition=typed-preview"],
            vec!["doctor", "--", "--manifest-path=p"],
            vec!["script", "doctor", "--manifest-path=p", "--native"],
            vec!["script", "--native", "--manifest-path=p"],
            vec!["script", "task", "--profile=wrong", "--manifest-path=p"],
            vec!["check", "--manifest-path=p"],
            vec!["fmt", "--manifest-path=p", "--check"],
        ] {
            assert!(select(&args(&words)).is_none(), "{words:?}");
        }
        for words in [
            vec!["doctor"],
            vec!["doctor", "--native"],
            vec!["doctor", "--", "--manifest-path=p"],
            vec!["script", "doctor", "--manifest-path=p", "--native"],
            vec!["script", "task", "--profile=wrong", "--manifest-path=p"],
        ] {
            let arguments = args(&words);
            assert_eq!(options::route(&arguments), Route::Legacy(arguments));
        }
        for words in [
            vec!["doctor", "--edition=typed-preview"],
            vec![
                "--edition",
                "typed-preview",
                "doctor",
                "--message-format=text",
            ],
        ] {
            assert_eq!(options::route(&args(&words)), Route::TypedDoctor);
        }
        for words in [
            vec!["doctor", "--edition=typed-preview", "--native"],
            vec![
                "doctor",
                "--edition=typed-preview",
                "--",
                "--manifest-path=p",
            ],
        ] {
            assert!(matches!(options::route(&args(&words)), Route::Error { .. }));
        }
        assert!(matches!(
            options::route(&args(&["fmt", "--manifest-path=p", "--check"])),
            Route::ManifestFormat { write: false, .. }
        ));
    }

    #[test]
    fn manifest_request_limits_and_route_layout_remain_in_force() {
        let long_path = format!("--manifest-path={}", "p".repeat(4097));
        let Route::ManifestDoctorError { failure } = options::route(&args(&["doctor", &long_path]))
        else {
            panic!("existing manifest path bound must apply");
        };
        assert_eq!(failure.classification(), ("E0400", "manifest-input"));
        assert_eq!(failure.message(), "manifest path exceeds 4096 bytes");
        let package = format!("--package={}", "p".repeat(65));
        refusal(
            &["doctor", "--manifest-path=p", "--native", &package],
            "--package name exceeds 64 bytes",
        );
        #[cfg(target_pointer_width = "64")]
        assert_eq!(std::mem::size_of::<Route>(), 56);
    }
}
