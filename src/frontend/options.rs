//! Edition selection is a pure gate, before legacy dispatch or any host effects.
//!
//! The two new options are recognized anywhere before `--`. For `script`,
//! recognition stops immediately after the script name: the remaining arguments
//! belong to the launched process. With neither option, legacy argv is unchanged.
//! Typed checking accepts `check [options] [--] <source>` and options before the
//! command or after the source. The separator is optional and makes all later
//! words literal operands; exactly one source is required. It must follow `check`.

#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    Legacy(Vec<String>),
    TypedCheck { path: String, json: bool },
    Error { message: String, json: bool },
}

/// Classify arguments excluding the executable name, without reading any files.
pub fn route(args: &[String]) -> Route {
    let mut forwarded = Vec::with_capacity(args.len());
    let mut edition = None;
    let mut edition_seen = false;
    let mut format_seen = false;
    let mut json = false;
    let mut error = None;
    let mut index = 0;

    while index < args.len() {
        // A manifest script owns every argument after its name, including words
        // that happen to look like the new Oxid options.
        if forwarded.first().map(String::as_str) == Some("script") && forwarded.len() == 2 {
            forwarded.extend_from_slice(&args[index..]);
            break;
        }
        let argument = args[index].as_str();
        if argument == "--" {
            forwarded.extend_from_slice(&args[index..]);
            break;
        }
        let option = ["--edition", "--message-format"].into_iter().find(|name| {
            argument == *name
                || argument
                    .strip_prefix(name)
                    .is_some_and(|suffix| suffix.starts_with('='))
        });
        let Some(name) = option else {
            forwarded.push(args[index].clone());
            index += 1;
            continue;
        };

        let seen = if name == "--edition" {
            &mut edition_seen
        } else {
            &mut format_seen
        };
        if *seen {
            error.get_or_insert_with(|| format!("{name} may only be specified once"));
        }
        *seen = true;
        let value = if let Some((_, value)) = argument.split_once('=') {
            Some(value)
        } else if let Some(next) = args.get(index + 1).filter(|next| !next.starts_with('-')) {
            index += 1;
            Some(next.as_str())
        } else {
            None
        };
        index += 1;

        match (name, value) {
            (_, None | Some("")) => {
                error.get_or_insert_with(|| format!("{name} requires a value"));
            }
            ("--edition", Some(value @ ("legacy-0.9" | "typed-preview"))) => {
                edition = Some(value);
            }
            ("--edition", Some(value)) => {
                error.get_or_insert_with(|| {
                    format!("unknown edition `{value}`; expected legacy-0.9 or typed-preview")
                });
            }
            ("--message-format", Some("json")) => json = true,
            ("--message-format", Some("text")) => {}
            ("--message-format", Some(value)) => {
                error.get_or_insert_with(|| {
                    format!("unknown --message-format `{value}`; expected text or json")
                });
            }
            _ => unreachable!("the option name is selected from a closed list"),
        }
    }

    if format_seen && edition != Some("typed-preview") {
        error
            .get_or_insert_with(|| "--message-format requires --edition typed-preview".to_string());
    }
    if let Some(message) = error {
        return Route::Error { message, json };
    }
    if edition != Some("typed-preview") {
        return Route::Legacy(forwarded);
    }
    if forwarded.first().map(String::as_str) != Some("check") {
        return Route::Error {
            message: format!(
                "edition `typed-preview` supports only `check`; command `{}` is unavailable",
                forwarded.first().map(String::as_str).unwrap_or("<missing>")
            ),
            json,
        };
    }

    let mut path = None;
    let mut separated = false;
    for argument in forwarded.into_iter().skip(1) {
        if !separated && argument == "--" {
            separated = true;
            continue;
        }
        if !separated && argument.starts_with('-') {
            return Route::Error {
                message: format!("unsupported option for typed-preview check: {argument}"),
                json,
            };
        }
        if path.replace(argument).is_some() {
            return Route::Error {
                message: "typed-preview check requires exactly one source path".to_string(),
                json,
            };
        }
    }
    match path {
        Some(path) => Route::TypedCheck { path, json },
        None => Route::Error {
            message: "typed-preview check requires exactly one source path".to_string(),
            json,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{route, Route};

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn error(values: &[&str], expected: &str, json: bool) {
        match route(&arguments(values)) {
            Route::Error {
                message,
                json: actual,
            } => {
                assert!(message.contains(expected), "{message}");
                assert_eq!(actual, json);
            }
            other => panic!("expected {expected} error for {values:?}, got {other:?}"),
        }
    }

    #[test]
    fn old_arguments_are_forwarded_identically() {
        for values in [
            &[][..],
            &["run", "file.ox", "--unrelated"],
            &["--help"],
            &["script", "name", "--edition", "typed-preview"],
            &["run", "file.ox", "--", "--edition=typed-preview"],
            &["script", "name", "--message-format=json", "--edition"],
        ] {
            let args = arguments(values);
            assert_eq!(route(&args), Route::Legacy(args));
        }
    }

    #[test]
    fn explicit_legacy_only_removes_its_selector() {
        for values in [
            &["--edition=legacy-0.9", "run", "file.ox"][..],
            &["run", "--edition", "legacy-0.9", "file.ox"],
            &["run", "file.ox", "--edition=legacy-0.9"],
        ] {
            assert_eq!(
                route(&arguments(values)),
                Route::Legacy(arguments(&["run", "file.ox"]))
            );
        }
        assert_eq!(
            route(&arguments(&[
                "script",
                "--edition=legacy-0.9",
                "name",
                "--edition=typed-preview"
            ])),
            Route::Legacy(arguments(&["script", "name", "--edition=typed-preview"]))
        );
    }

    #[test]
    fn typed_check_accepts_global_options_and_a_source_separator() {
        for values in [
            &["--edition=typed-preview", "check", "file.ox"][..],
            &["check", "--edition", "typed-preview", "file.ox"],
            &["check", "file.ox", "--edition=typed-preview"],
            &[
                "check",
                "--edition=typed-preview",
                "--message-format=text",
                "--",
                "file.ox",
            ],
            &["--edition=typed-preview", "check", "--", "--flag-named.ox"],
        ] {
            let path = if values.last() == Some(&"--flag-named.ox") {
                "--flag-named.ox"
            } else {
                "file.ox"
            };
            assert_eq!(
                route(&arguments(values)),
                Route::TypedCheck {
                    path: path.into(),
                    json: false
                }
            );
        }
        assert_eq!(
            route(&arguments(&[
                "--message-format",
                "json",
                "check",
                "file.ox",
                "--edition=typed-preview"
            ])),
            Route::TypedCheck {
                path: "file.ox".into(),
                json: true
            }
        );
    }

    #[test]
    fn typed_preview_rejects_every_other_command() {
        for command in [
            "run",
            "file.ox",
            "compile",
            "ast",
            "build",
            "test",
            "watch",
            "new",
            "clean",
            "script",
            "help",
            "--version",
            "unknown",
        ] {
            error(
                &[command, "--edition=typed-preview"],
                "typed-preview",
                false,
            );
        }
        error(&["--edition=typed-preview"], "typed-preview", false);
        error(
            &["script", "--edition=typed-preview", "name"],
            "typed-preview",
            false,
        );
    }

    #[test]
    fn edition_values_must_be_known_present_and_unique() {
        for selection in [
            &["--edition"][..],
            &["--edition="],
            &["--edition=nope"],
            &["--edition", "nope"],
            &["--edition", "--other"],
            &["--edition=legacy-0.9", "--edition=legacy-0.9"],
            &["--edition=typed-preview", "--edition=typed-preview"],
            &["--edition=typed-preview", "--edition=legacy-0.9"],
        ] {
            let mut values = vec!["run", "file.ox"];
            values.extend_from_slice(selection);
            error(&values, "edition", false);
        }
    }

    #[test]
    fn message_format_is_typed_only_present_known_and_unique() {
        for selection in [
            &["--message-format"][..],
            &["--message-format="],
            &["--message-format=xml"],
            &["--message-format=text", "--message-format=text"],
        ] {
            let mut values = vec!["check", "file.ox", "--edition=typed-preview"];
            values.extend_from_slice(selection);
            error(&values, "message-format", false);
        }
        error(
            &["check", "file.ox", "--message-format=text"],
            "message-format",
            false,
        );
        error(
            &[
                "check",
                "file.ox",
                "--edition=legacy-0.9",
                "--message-format=json",
            ],
            "message-format",
            true,
        );
    }

    #[test]
    fn json_errors_are_selected_even_when_an_earlier_option_is_invalid() {
        error(
            &[
                "check",
                "file.ox",
                "--edition=nope",
                "--message-format=json",
            ],
            "edition",
            true,
        );
        error(
            &["check", "file.ox", "--edition", "--message-format", "json"],
            "edition",
            true,
        );
        error(
            &[
                "check",
                "file.ox",
                "--edition=typed-preview",
                "--message-format=json",
                "--message-format=text",
            ],
            "message-format",
            true,
        );
        error(
            &[
                "run",
                "file.ox",
                "--message-format=json",
                "--edition=typed-preview",
            ],
            "typed-preview",
            true,
        );
    }

    #[test]
    fn typed_check_uses_one_literal_source_after_separator() {
        for values in [
            &["check", "--edition=typed-preview"][..],
            &["check", "a.ox", "b.ox", "--edition=typed-preview"],
            &["check", "--unknown", "a.ox", "--edition=typed-preview"],
            &[
                "check",
                "--edition=typed-preview",
                "--",
                "a.ox",
                "--message-format=json",
            ],
        ] {
            error(values, "check", false);
        }
        error(
            &["--edition=typed-preview", "--", "check", "file.ox"],
            "typed-preview",
            false,
        );
    }
}
