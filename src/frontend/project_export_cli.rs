//! Borrow-only selector for explicit manifest source export. Reuse the existing
//! manifest request without changing the established scanner or legacy routes.
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
    "--output",
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
            Self::Unsupported(value) => format!("unsupported option for manifest export: {value}"),
        }
    }
}

#[derive(Default)]
struct Scan<'a> {
    path: Option<&'a str>,
    package: Option<&'a str>,
    output: Option<&'a str>,
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
                (9, value) => scan.output = value,
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
    // Only the literal export command with an in-scope manifest selector opts
    // in. Script arguments and `--` operands cannot enter project export.
    if scan.command != Some("export") || scan.seen & 1 == 0 {
        return None;
    }
    let message = if let Some(error) = scan.global_error.or(scan.local_error) {
        Some(error.message())
    } else if scan.before {
        Some("--manifest-path must follow export".into())
    } else if scan.edition == Some("legacy-0.9") {
        Some("manifest export conflicts with --edition legacy-0.9".into())
    } else if scan.operands {
        Some("manifest export does not accept a source operand".into())
    } else if scan.seen & (1 << 3) != 0 {
        Some(PROFILE_APPLICABILITY.into())
    } else if scan.seen & ((1 << 4) | (1 << 5) | (1 << 6) | (1 << 7)) != 0 {
        Some("manifest export does not support entry-mode or experimental selectors".into())
    } else if scan.output.is_none() {
        Some("manifest export requires --output".into())
    } else if scan.json {
        Some("manifest export does not support --message-format=json".into())
    } else {
        None
    };
    if let Some(message) = message {
        return Some(Route::ManifestExportError {
            failure: manifest::Failure::Cli(message),
        });
    }
    let output = scan.output.unwrap_or("");
    if output.len() > 4096 {
        return Some(Route::ManifestExportError {
            failure: manifest::Failure::Resource("export output path exceeds 4096 bytes"),
        });
    }
    let mut owned_output = String::new();
    if owned_output.try_reserve_exact(output.len()).is_err() {
        return Some(Route::ManifestExportError {
            failure: manifest::Failure::Resource("export output path allocation failed"),
        });
    }
    owned_output.push_str(output);
    Some(
        match manifest::format_request(scan.path.unwrap_or(""), scan.package) {
            Ok(request) => Route::ManifestExport {
                request,
                output: owned_output,
            },
            Err(failure) => Route::ManifestExportError { failure },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(args: &[&str]) -> Option<Route> {
        let args = args
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        select(&args)
    }
    fn error(args: &[&str]) -> String {
        match route(args) {
            Some(Route::ManifestExportError { failure }) => failure.message(),
            other => panic!("expected export error, got {other:?}"),
        }
    }

    #[test]
    fn split_and_equal_selectors_create_an_export_route() {
        for args in [
            vec![
                "export",
                "--manifest-path",
                "project/oxid.toml",
                "--output",
                "copy",
            ],
            vec![
                "--edition=typed-preview",
                "export",
                "--manifest-path=project/oxid.toml",
                "--package=app",
                "--output=copy",
                "--message-format=text",
            ],
        ] {
            assert!(
                matches!(route(&args), Some(Route::ManifestExport { output, .. }) if output == "copy")
            );
        }
    }

    #[test]
    fn invalid_options_fail_before_any_manifest_io() {
        for (tail, expected) in [
            (vec![], "requires --output"),
            (vec!["--output"], "--output requires a value"),
            (
                vec!["--output=x", "--output=y"],
                "--output may only be specified once",
            ),
            (vec!["--output=x", "--profile=dev"], "--profile"),
            (
                vec!["--output=x", "--entry-mode=process"],
                "does not support entry-mode",
            ),
            (vec!["--output=x", "--engine=native"], "unsupported option"),
            (
                vec!["--output=x", "--message-format=json"],
                "does not support --message-format=json",
            ),
            (
                vec!["--output=x", "--edition=legacy-0.9"],
                "conflicts with --edition legacy-0.9",
            ),
            (
                vec!["--output=x", "source.ox"],
                "does not accept a source operand",
            ),
        ] {
            let mut args = vec!["export", "--manifest-path=missing-file"];
            args.extend(tail);
            assert!(error(&args).contains(expected), "{args:?}");
        }
        assert!(
            error(&["--manifest-path=missing", "export", "--output=copy"])
                .contains("must follow export")
        );
        let long = format!("--output={}", "x".repeat(4097));
        assert!(error(&["export", "--manifest-path=missing", &long]).contains("exceeds 4096 bytes"));
    }

    #[test]
    fn unrelated_commands_script_arguments_and_separator_keep_their_owner() {
        for args in [
            vec!["doc", "--manifest-path=missing"],
            vec![
                "script",
                "export",
                "--manifest-path=missing",
                "--output=copy",
            ],
            vec!["export", "--", "--manifest-path=missing", "--output=copy"],
            vec!["export", "--output=copy"],
            vec!["check", "--manifest-path=missing", "--output=copy"],
        ] {
            assert!(route(&args).is_none(), "{args:?}");
        }
    }
}
