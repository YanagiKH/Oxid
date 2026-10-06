//! Edition selection is a pure gate, before legacy dispatch or any host effects.
//!
//! Global edition and format options are recognized anywhere before `--`. For `script`,
//! recognition stops immediately after the script name: the remaining arguments
//! belong to the launched process. With neither option, legacy argv is unchanged.
//! Typed commands accept `check|run|compile [options] [--] <source>` and options before the
//! command or after the source. The separator is optional and makes all later
//! words literal operands; exactly one source is required. It follows the command.
//! Typed `fmt` is isolated from semantic operations and accepts a local `--check` flag.

#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    Legacy(Vec<String>),
    TypedFormat {
        path: String,
        check: bool,
    },
    FormatError {
        message: String,
    },
    TypedCheck {
        path: String,
        json: bool,
    },
    TypedRun {
        path: String,
        json: bool,
        entry_policy: EntryPolicy,
    },
    TypedCompile {
        path: String,
        json: bool,
        output: String,
        entry_policy: EntryPolicy,
    },
    Error {
        message: String,
        json: bool,
        operation: Operation,
    },
    // Closed data carrier. The public classifier cannot produce this variant.
    ProcessError {
        message: String,
    },
}

/// Transient entry selection, never retained in a checked source program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryPolicy {
    Result,
    Process,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Check,
    Run,
    Compile,
}
impl Operation {
    fn command(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Run => "run",
            Self::Compile => "compile",
        }
    }
}

/// Compare the actual closed carriers with the unchanged input-only route.
#[cfg(test)]
#[allow(dead_code)]
mod output_layout_feasibility {
    use super::*;
    use std::mem::{align_of, size_of};

    // Exact current variants, field types and declaration order. In particular,
    // use the real Operation/String/Vec carriers instead of byte stand-ins.
    enum BaselineRoute {
        Legacy(Vec<String>),
        TypedFormat {
            path: String,
            check: bool,
        },
        FormatError {
            message: String,
        },
        TypedCheck {
            path: String,
            json: bool,
        },
        TypedRun {
            path: String,
            json: bool,
        },
        TypedCompile {
            path: String,
            json: bool,
            output: String,
        },
        Error {
            message: String,
            json: bool,
            operation: Operation,
        },
    }

    fn report<T>(name: &str) {
        println!(
            "OUTPUT_ROUTE_LAYOUT {name} bytes={} align={}",
            size_of::<T>(),
            align_of::<T>()
        );
    }

    #[test]
    fn bounded_stdout_actual_closed_route_layout() {
        assert_eq!(size_of::<BaselineRoute>(), size_of::<Route>());
        assert_eq!(align_of::<BaselineRoute>(), align_of::<Route>());
        macro_rules! layouts {
            ($($ty:ty),+ $(,)?) => {$(report::<$ty>(stringify!($ty));)+};
        }
        layouts!(
            Operation,
            bool,
            String,
            Vec<String>,
            EntryPolicy,
            Option<EntryPolicy>,
            Result<EntryPolicy, EntryPolicy>,
            Route,
            Option<Route>,
            Result<Route, Route>,
            Result<Route, String>,
            BaselineRoute,
            EntryOptionMode,
            EntryOptionScan,
            Option<EntryOptionScan>,
            Result<EntryOptionScan, Route>,
            &mut EntryOptionScan,
        );
        #[cfg(target_pointer_width = "64")]
        assert_eq!(size_of::<Route>(), 56);
        println!(
            "OUTPUT_ROUTE_CLOSED policy_and_text_only_process_error \
             baseline_route={} actual_route={} delta={} \
             public_behavior=UNCHANGED process_admission=DENIED",
            size_of::<BaselineRoute>(),
            size_of::<Route>(),
            size_of::<Route>() as i128 - size_of::<BaselineRoute>() as i128,
        );
        println!(
            "OUTPUT_ROUTE_SCOPE measured_enclosing_enum_padding; \
             String_and_Vec_heap_capacities_not_measured; \
             candidate_classifier_is_test_only_and_returns_Route; \
             no_source_owner_no_process_reporting_no_policy_activation"
        );
    }
}

/// Classify arguments excluding the executable name, without reading any files.
pub fn route(args: &[String]) -> Route {
    classify(args, &mut EntryOptionScan::new(EntryOptionMode::Closed))
}

// Only a test can select proposed entry-option parsing. The production mode
// recognizes exactly the existing global options and always selects Result.
#[derive(Clone, Copy)]
enum EntryOptionMode {
    Closed,
    #[cfg(test)]
    Candidate,
}

struct EntryOptionScan {
    mode: EntryOptionMode,
    policy: EntryPolicy,
    seen: bool,
    process_requested: bool,
    compile: bool,
}

impl EntryOptionScan {
    fn new(mode: EntryOptionMode) -> Self {
        Self {
            mode,
            policy: EntryPolicy::Result,
            seen: false,
            process_requested: false,
            compile: false,
        }
    }

    fn names(&self) -> &'static [&'static str] {
        match self.mode {
            EntryOptionMode::Closed => &["--edition", "--message-format"],
            #[cfg(test)]
            EntryOptionMode::Candidate => &["--edition", "--message-format", "--entry-mode"],
        }
    }
}

/// Pure proposed argv classification only: no driver, source owner or effects.
#[cfg(test)]
fn candidate_route(args: &[String]) -> Route {
    let mut entry = EntryOptionScan::new(EntryOptionMode::Candidate);
    let classified = classify(args, &mut entry);
    match classified {
        Route::Error { message, .. } | Route::FormatError { message }
            if entry.process_requested && !entry.compile =>
        {
            Route::ProcessError { message }
        }
        Route::TypedRun {
            json: true,
            entry_policy: EntryPolicy::Process,
            ..
        } => Route::ProcessError {
            message: "process-mode run does not support --message-format=json".into(),
        },
        other => other,
    }
}

fn classify(args: &[String], entry: &mut EntryOptionScan) -> Route {
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
        let option = entry.names().iter().copied().find(|name| {
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

        let seen = match name {
            "--edition" => &mut edition_seen,
            "--message-format" => &mut format_seen,
            "--entry-mode" => &mut entry.seen,
            _ => unreachable!("the option name is selected from a closed list"),
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
            ("--entry-mode", Some("result")) => entry.policy = EntryPolicy::Result,
            ("--entry-mode", Some("process")) => {
                entry.policy = EntryPolicy::Process;
                entry.process_requested = true;
            }
            ("--entry-mode", Some(value)) => {
                error.get_or_insert_with(|| {
                    format!("unknown --entry-mode `{value}`; expected result or process")
                });
            }
            _ => unreachable!("the option name is selected from a closed list"),
        }
    }

    entry.compile = forwarded.first().map(String::as_str) == Some("compile");
    if entry.seen
        && (edition != Some("typed-preview")
            || !matches!(forwarded.first().map(String::as_str), Some("run" | "compile")))
    {
        error.get_or_insert_with(|| {
            "--entry-mode requires typed-preview run or compile".to_string()
        });
    }
    if format_seen && edition != Some("typed-preview") {
        error
            .get_or_insert_with(|| "--message-format requires --edition typed-preview".to_string());
    }
    if let Some(message) = error {
        return Route::Error {
            message,
            json,
            operation: Operation::Check,
        };
    }
    if edition != Some("typed-preview") {
        return Route::Legacy(forwarded);
    }
    if forwarded.first().map(String::as_str) == Some("fmt") {
        return route_format(forwarded, json);
    }
    let operation = match forwarded.first().map(String::as_str) {
        Some("check") => Operation::Check,
        Some("run") => Operation::Run,
        Some("compile") => Operation::Compile,
        _ => return Route::Error {
            message: format!("edition `typed-preview` supports only `check`, explicit `run`, explicit native `compile`, and explicit `fmt`; command `{}` is unavailable",
                forwarded.first().map(String::as_str).unwrap_or("<missing>")),
            json, operation: Operation::Check,
        },
    };
    let command = operation.command();

    let mut path = None;
    let mut output = None;
    let mut backend = None;
    let mut target = None;
    let mut separated = false;
    let mut words = forwarded.into_iter().skip(1);
    while let Some(argument) = words.next() {
        if operation == Operation::Compile && !separated {
            let name = argument.split('=').next().unwrap_or("");
            if matches!(name, "--backend" | "--target" | "--output") {
                let value = argument
                    .split_once('=')
                    .map(|(_, v)| v.to_string())
                    .or_else(|| words.next());
                let slot = match name {
                    "--backend" => &mut backend,
                    "--target" => &mut target,
                    _ => &mut output,
                };
                if slot.is_some()
                    || value
                        .as_ref()
                        .is_none_or(|v| v.is_empty() || v.starts_with('-'))
                {
                    return Route::Error {
                        message: format!(
                            "{name} requires one nonempty value and may occur only once"
                        ),
                        json,
                        operation,
                    };
                }
                *slot = value;
                continue;
            }
        }
        if !separated && argument == "--" {
            separated = true;
            continue;
        }
        if !separated && argument.starts_with('-') {
            return Route::Error {
                message: format!("unsupported option for typed-preview {command}: {argument}"),
                json,
                operation,
            };
        }
        if path.replace(argument).is_some() {
            return Route::Error {
                message: format!("typed-preview {command} requires exactly one source path"),
                json,
                operation,
            };
        }
    }
    if operation == Operation::Compile {
        let message = if backend.as_deref() != Some("llvm") {
            Some("typed-preview compile requires --backend llvm")
        } else if target
            .as_deref()
            .is_some_and(|v| v != "x86_64-unknown-linux-gnu")
        {
            Some("native preview supports only --target x86_64-unknown-linux-gnu")
        } else if output.is_none() {
            Some("typed-preview compile requires --output <new-path>")
        } else {
            None
        };
        if let Some(message) = message {
            return Route::Error {
                message: message.into(),
                json,
                operation,
            };
        }
    }
    match path {
        Some(path) => match operation {
            Operation::Check => Route::TypedCheck { path, json },
            Operation::Run => Route::TypedRun {
                path,
                json,
                entry_policy: entry.policy,
            },
            Operation::Compile => Route::TypedCompile {
                path,
                json,
                output: output.expect("validated output"),
                entry_policy: entry.policy,
            },
        },
        None => Route::Error {
            message: format!("typed-preview {command} requires exactly one source path"),
            json,
            operation,
        },
    }
}

/// A selected formatter has its own text-only, exit-2 error contract.
/// Keep it outside Operation so it cannot enter semantic loading or summaries.
fn route_format(forwarded: Vec<String>, json: bool) -> Route {
    let fail = |message: String| Route::FormatError { message };
    if json {
        return fail("typed-preview fmt does not support --message-format=json".into());
    }
    let mut path = None;
    let mut check = false;
    let mut separated = false;
    for argument in forwarded.into_iter().skip(1) {
        if !separated && argument == "--" {
            separated = true;
            continue;
        }
        if !separated && argument == "--check" {
            if check {
                return fail("typed-preview fmt --check may only be specified once".into());
            }
            check = true;
            continue;
        }
        if argument == "-" {
            return fail(
                "typed-preview fmt does not support stdin; use a named source path".into(),
            );
        }
        if !separated && argument.starts_with('-') {
            return fail(format!(
                "unsupported option for typed-preview fmt: {argument}"
            ));
        }
        if path.replace(argument).is_some() {
            return fail("typed-preview fmt requires exactly one source path".into());
        }
    }
    match path {
        Some(path) => Route::TypedFormat { path, check },
        None => fail("typed-preview fmt requires exactly one source path".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::{candidate_route, route, EntryPolicy, Route};

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn error(values: &[&str], expected: &str, json: bool) {
        match route(&arguments(values)) {
            Route::Error {
                message,
                json: actual,
                ..
            } => {
                assert!(message.contains(expected), "{message}");
                assert_eq!(actual, json);
            }
            other => panic!("expected {expected} error for {values:?}, got {other:?}"),
        }
    }

    #[test]
    fn public_run_and_compile_keep_result_policy_and_existing_options() {
        for json in [false, true] {
            let format = if json {
                "--message-format=json"
            } else {
                "--message-format=text"
            };
            assert_eq!(
                route(&arguments(&[
                    "run",
                    "file.ox",
                    "--edition=typed-preview",
                    format,
                ])),
                Route::TypedRun {
                    path: "file.ox".into(),
                    json,
                    entry_policy: EntryPolicy::Result,
                }
            );
            assert_eq!(
                route(&arguments(&[
                    "--edition=typed-preview",
                    format,
                    "compile",
                    "--backend",
                    "llvm",
                    "--target=x86_64-unknown-linux-gnu",
                    "file.ox",
                    "--output=program",
                ])),
                Route::TypedCompile {
                    path: "file.ox".into(),
                    json,
                    output: "program".into(),
                    entry_policy: EntryPolicy::Result,
                }
            );
        }
    }

    #[test]
    fn public_routes_cannot_select_process_or_process_errors() {
        for command in ["check", "run", "compile", "fmt"] {
            for value in ["result", "process", "unknown"] {
                for json in [false, true] {
                    let mut values = vec![command, "--edition=typed-preview", "file.ox"];
                    if command == "compile" {
                        values.extend(["--backend=llvm", "--output=program"]);
                    }
                    if json {
                        values.push("--message-format=json");
                    }
                    values.extend(["--entry-mode", value]);
                    let classified = route(&arguments(&values));
                    assert!(
                        matches!(classified, Route::Error { .. } | Route::FormatError { .. }),
                        "{values:?}: {classified:?}"
                    );
                    if let Route::Error { json: actual, .. } = classified {
                        assert_eq!(actual, json);
                    }
                }
            }
        }
        for values in [
            &["--entry-mode=process", "run", "file.ox", "--edition=typed-preview"][..],
            &["run", "file.ox", "--edition=typed-preview", "--entry-mode=process"],
            &["run", "file.ox", "--edition=typed-preview", "--entry-mode"],
        ] {
            assert!(matches!(route(&arguments(values)), Route::Error { .. }));
        }
    }

    #[test]
    fn candidate_entry_options_select_only_run_and_compile_data() {
        for (value, policy) in [
            ("result", EntryPolicy::Result),
            ("process", EntryPolicy::Process),
        ] {
            for values in [
                vec!["--entry-mode", value, "run", "file.ox", "--edition=typed-preview"],
                vec!["run", "--entry-mode", value, "--edition=typed-preview", "file.ox"],
                vec!["run", "file.ox", "--edition=typed-preview", "--entry-mode", value],
            ] {
                assert_eq!(
                    candidate_route(&arguments(&values)),
                    Route::TypedRun {
                        path: "file.ox".into(),
                        json: false,
                        entry_policy: policy,
                    }
                );
            }
            assert_eq!(
                candidate_route(&arguments(&[
                    "--edition=typed-preview", "compile", "file.ox", "--backend=llvm",
                    "--output=program", "--message-format=json", "--entry-mode", value,
                ])),
                Route::TypedCompile {
                    path: "file.ox".into(),
                    json: true,
                    output: "program".into(),
                    entry_policy: policy,
                }
            );
        }
        assert!(matches!(
            candidate_route(&arguments(&[
                "run", "file.ox", "--edition=typed-preview", "--entry-mode=process"
            ])),
            Route::TypedRun { entry_policy: EntryPolicy::Process, .. }
        ));
        for command in ["check", "fmt", "script", "unknown"] {
            assert!(matches!(
                candidate_route(&arguments(&[
                    "--entry-mode=process", "--edition=typed-preview", command, "file.ox"
                ])),
                Route::ProcessError { .. }
            ));
        }
        assert!(matches!(
            candidate_route(&arguments(&["run", "file.ox", "--entry-mode=process"])),
            Route::ProcessError { .. }
        ));
    }

    #[test]
    fn candidate_process_run_json_and_early_errors_are_text_only_data() {
        for suffix in [
            &["--message-format=json"][..],
            &["--entry-mode=process"],
            &["--entry-mode=result"],
            &["--entry-mode"],
            &["--entry-mode="],
            &["--entry-mode=unknown"],
            &["--edition=unknown"],
            &["--message-format=unknown"],
            &["--unknown"],
            &["second.ox"],
        ] {
            let mut values = vec!["run", "file.ox", "--edition=typed-preview", "--entry-mode=process"];
            values.extend_from_slice(suffix);
            assert!(
                matches!(candidate_route(&arguments(&values)), Route::ProcessError { .. }),
                "{values:?}"
            );
        }
        for values in [
            &["run", "--edition=typed-preview", "--entry-mode=process"][..],
            &["run", "file.ox", "--edition=unknown", "--entry-mode=process", "--message-format=json"],
            &["run", "file.ox", "--edition=typed-preview", "--entry-mode=unknown", "--entry-mode=process"],
        ] {
            assert!(matches!(candidate_route(&arguments(values)), Route::ProcessError { .. }));
        }
        for selection in [
            &["--entry-mode"][..],
            &["--entry-mode="],
            &["--entry-mode=unknown"],
            &["--entry-mode=result", "--entry-mode=result"],
        ] {
            let mut values = vec!["run", "file.ox", "--edition=typed-preview", "--message-format=json"];
            values.extend_from_slice(selection);
            match candidate_route(&arguments(&values)) {
                Route::Error { message, json, .. } => {
                    assert!(message.contains("--entry-mode"));
                    assert!(json);
                }
                other => panic!("expected entry option error for {values:?}, got {other:?}"),
            }
        }
        // Compilation never enters process execution; its ordinary JSON error
        // carrier is also preserved when a candidate process policy was selected.
        assert!(matches!(
            candidate_route(&arguments(&[
                "compile", "file.ox", "--edition=typed-preview", "--entry-mode=process",
                "--message-format=json", "--backend=unknown", "--output=program"
            ])),
            Route::Error { json: true, .. }
        ));
    }

    #[test]
    fn candidate_and_public_routes_preserve_script_and_separator_boundaries() {
        for values in [
            &["script", "task", "--entry-mode", "process", "--edition=typed-preview"][..],
            &["run", "file.ox", "--", "--entry-mode=process", "--edition=typed-preview"],
        ] {
            let args = arguments(values);
            assert_eq!(route(&args), Route::Legacy(args.clone()));
            assert_eq!(candidate_route(&args), Route::Legacy(args));
        }
        for classify in [route, candidate_route] {
            assert_eq!(
                classify(&arguments(&[
                    "run", "--edition=typed-preview", "--", "--entry-mode=process"
                ])),
                Route::TypedRun {
                    path: "--entry-mode=process".into(),
                    json: false,
                    entry_policy: EntryPolicy::Result,
                }
            );
            assert_eq!(
                classify(&arguments(&[
                    "script", "--edition=legacy-0.9", "task", "--entry-mode=process"
                ])),
                Route::Legacy(arguments(&["script", "task", "--entry-mode=process"]))
            );
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
    fn typed_preview_rejects_unavailable_or_incomplete_commands() {
        for command in [
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
                "compile",
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

    #[test]
    fn typed_format_accepts_only_its_scoped_flag_and_single_literal_path() {
        for values in [
            &["fmt", "--edition", "typed-preview", "file.ox"][..],
            &[
                "--edition=typed-preview",
                "fmt",
                "--message-format=text",
                "file.ox",
            ],
            &["fmt", "file.ox", "--edition=typed-preview"],
        ] {
            assert_eq!(
                route(&arguments(values)),
                Route::TypedFormat {
                    path: "file.ox".into(),
                    check: false
                }
            );
        }
        for values in [
            &["--edition=typed-preview", "fmt", "--check", "file.ox"][..],
            &["fmt", "file.ox", "--edition=typed-preview", "--check"],
        ] {
            assert_eq!(
                route(&arguments(values)),
                Route::TypedFormat {
                    path: "file.ox".into(),
                    check: true
                }
            );
        }
        for name in ["--check", "--flag-named.ox", "./-"] {
            assert_eq!(
                route(&arguments(&["fmt", "--edition=typed-preview", "--", name])),
                Route::TypedFormat {
                    path: name.into(),
                    check: false
                }
            );
        }
    }

    #[test]
    fn typed_format_owns_selected_errors_but_preserves_global_errors() {
        for trailing in [
            &[][..],
            &["a.ox", "b.ox"],
            &["--check", "--check", "a.ox"],
            &["--check=true", "a.ox"],
            &["--write", "a.ox"],
            &["--output", "a.ox"],
            &["-"],
            &["--", "-"],
            &["a.ox", "--message-format=json"],
        ] {
            let mut values = vec!["fmt", "--edition=typed-preview"];
            values.extend_from_slice(trailing);
            assert!(
                matches!(route(&arguments(&values)), Route::FormatError { .. }),
                "{values:?}"
            );
        }
        for values in [
            &["--check", "fmt", "file.ox", "--edition=typed-preview"][..],
            &["fmt", "file.ox", "--edition=nope", "--message-format=json"],
            &[
                "fmt",
                "file.ox",
                "--edition=typed-preview",
                "--message-format=json",
                "--message-format=text",
            ],
        ] {
            assert!(
                matches!(route(&arguments(values)), Route::Error { .. }),
                "{values:?}"
            );
        }
        assert_eq!(
            route(&arguments(&["fmt", "file.ox"])),
            Route::Legacy(arguments(&["fmt", "file.ox"]))
        );
        assert_eq!(
            route(&arguments(&["fmt", "file.ox", "--edition=legacy-0.9"])),
            Route::Legacy(arguments(&["fmt", "file.ox"]))
        );
    }
}
