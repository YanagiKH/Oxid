//! Edition selection is a pure gate, before legacy dispatch or any host effects.
//!
//! Global edition, format and entry options are recognized anywhere before `--`. For `script`,
//! recognition stops immediately after the script name: the remaining arguments
//! belong to the launched process. Without global options, legacy argv is unchanged.
//! Typed commands accept `check|run|compile [options] [--] <source>` and options before the
//! command or after the source. The separator is optional and makes all later
//! words literal operands; exactly one source is required. It follows the command.
//! Typed `fmt` is isolated from semantic operations and accepts a local `--check` flag.

#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    Legacy(Vec<String>),
    TypedImport {
        request: Box<[ImportOptions; 1]>,
    },
    TypedProducer {
        request: Box<[ProducerOptions; 1]>,
    },
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
    // Process-intent errors use only the safe, fallible stderr reporter.
    ProcessError {
        message: String,
    },
}

/// Explicit bridge-only CLI ownership, never stored in a checked source program.
/// The one-element box is built with a fallible exact reservation; default Route
/// remains unchanged in size. String payloads retain ordinary argv ownership.
#[derive(Debug, PartialEq, Eq)]
pub struct ImportOptions {
    pub path: String,
    pub observation: String,
    pub output: Option<String>,
    pub json: bool,
    pub operation: Operation,
}

/// Explicit producer-only CLI ownership, separate from the stable import route.
/// The fallible one-element carrier owns already classified argv strings.
#[derive(Debug, PartialEq, Eq)]
pub struct ProducerOptions {
    pub path: String,
    pub bundle: String,
    pub output: Option<String>,
    pub json: bool,
    pub operation: Operation,
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

/// Compare the active carriers with the unchanged input-only route.
#[cfg(test)]
#[allow(dead_code)]
mod output_layout_feasibility {
    use super::*;
    use std::mem::{align_of, size_of};

    // Exact input-only variants, field types and declaration order. In particular,
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
    fn bounded_stdout_actual_public_route_layout() {
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
            Option<i32>,
            BaselineRoute,
            EntryOptionScan,
            Option<EntryOptionScan>,
            Result<EntryOptionScan, Route>,
            &mut EntryOptionScan,
        );
        #[cfg(target_pointer_width = "64")]
        assert_eq!(size_of::<Route>(), 56);
        println!(
            "OUTPUT_ROUTE_PUBLIC policy_and_text_only_process_error \
             baseline_route={} actual_route={} delta={} \
             public_behavior=EXPLICIT_PROCESS process_admission=QUALIFIED_HOST",
            size_of::<BaselineRoute>(),
            size_of::<Route>(),
            size_of::<Route>() as i128 - size_of::<BaselineRoute>() as i128,
        );
        println!(
            "OUTPUT_ROUTE_SCOPE measured_enclosing_enum_padding; \
             String_and_Vec_heap_capacities_not_measured; \
             public_classifier_returns_Route; \
             no_source_owner_or_effects_during_classification"
        );
    }
}

/// Classify arguments excluding the executable name, without reading any files.
pub fn route(args: &[String]) -> Route {
    let mut entry = EntryOptionScan::new();
    let classified = classify(args, &mut entry);
    match classified {
        Route::Error { message, .. } | Route::FormatError { message } if entry.process_errors => {
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

struct EntryOptionScan {
    policy: EntryPolicy,
    seen: bool,
    process_requested: bool,
    malformed: bool,
    process_errors: bool,
}

impl EntryOptionScan {
    fn new() -> Self {
        Self {
            policy: EntryPolicy::Result,
            seen: false,
            process_requested: false,
            malformed: false,
            process_errors: false,
        }
    }
}

fn classify(args: &[String], entry: &mut EntryOptionScan) -> Route {
    let mut forwarded = Vec::with_capacity(args.len());
    let mut edition = None;
    let mut edition_seen = false;
    let mut format_seen = false;
    let mut import_seen = false;
    let mut import = None;
    let mut producer_seen = false;
    let mut producer = None;
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
        let option = [
            "--edition",
            "--message-format",
            "--entry-mode",
            "--experimental-hir-import",
            "--experimental-hir-producers",
        ]
        .into_iter()
        .find(|name| {
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
            "--experimental-hir-import" => &mut import_seen,
            "--experimental-hir-producers" => &mut producer_seen,
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
                entry.malformed |= name == "--entry-mode";
                error.get_or_insert_with(|| format!("{name} requires a value"));
            }
            ("--experimental-hir-import", Some(value)) => import = Some(value.to_string()),
            ("--experimental-hir-producers", Some(value)) => producer = Some(value.to_string()),
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
                entry.malformed = true;
                error.get_or_insert_with(|| {
                    format!("unknown --entry-mode `{value}`; expected result or process")
                });
            }
            _ => unreachable!("the option name is selected from a closed list"),
        }
    }

    // Decide channels only after the complete applicable argv is classified.
    // A later conflicting entry value cannot erase a valid Process request.
    let command = forwarded.first().map(String::as_str);
    entry.process_errors = match command {
        Some("compile" | "check" | "fmt") => false,
        Some("run") => {
            entry.process_requested || (entry.malformed && edition == Some("typed-preview"))
        }
        _ => entry.process_requested,
    };
    if entry.seen
        && (edition != Some("typed-preview")
            || !matches!(
                forwarded.first().map(String::as_str),
                Some("run" | "compile")
            ))
    {
        error.get_or_insert_with(|| {
            "--entry-mode requires typed-preview run or compile".to_string()
        });
    }
    if import_seen
        && (edition != Some("typed-preview")
            || !matches!(command, Some("check" | "run" | "compile")))
    {
        error.get_or_insert_with(|| {
            "--experimental-hir-import requires explicit typed-preview check, run or compile".into()
        });
    }
    if import_seen && entry.policy == EntryPolicy::Process {
        error.get_or_insert_with(|| {
            "--experimental-hir-import supports only Result entry mode".into()
        });
    }
    if producer_seen
        && (edition != Some("typed-preview")
            || !matches!(command, Some("check" | "run" | "compile")))
    {
        error.get_or_insert_with(|| {
            "--experimental-hir-producers requires explicit typed-preview check, run or compile"
                .into()
        });
    }
    if producer_seen && entry.policy == EntryPolicy::Process {
        error.get_or_insert_with(|| {
            "--experimental-hir-producers supports only Result entry mode".into()
        });
    }
    if import_seen && producer_seen {
        error.get_or_insert_with(|| {
            "--experimental-hir-import and --experimental-hir-producers are mutually exclusive"
                .into()
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
    if let Some(bundle) = producer {
        let Some(path) = path else {
            return Route::Error {
                message: format!("typed-preview {command} requires exactly one source path"),
                json,
                operation,
            };
        };
        let options = ProducerOptions {
            path,
            bundle,
            output,
            json,
            operation,
        };
        return producer_route(options, &mut super::project::budget::Allocator::default());
    }
    if let Some(observation) = import {
        let Some(path) = path else {
            return Route::Error {
                message: format!("typed-preview {command} requires exactly one source path"),
                json,
                operation,
            };
        };
        let options = ImportOptions {
            path,
            observation,
            output,
            json,
            operation,
        };
        return import_route(options, &mut super::project::budget::Allocator::default());
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

fn import_route(
    options: ImportOptions,
    allocator: &mut super::project::budget::Allocator,
) -> Route {
    let json = options.json;
    let operation = options.operation;
    // Stable Rust has no fallible Box::new. This bridge-specific allocation is
    // a single actual carrier, admitted before reserve under the unchanged cap.
    // String ownership is moved from argv; no source/artifact bytes are loaded.
    let mut storage = Vec::new();
    if std::mem::size_of::<ImportOptions>() as u64
        > super::declaration_index::IndexLimits::default().retained
        || allocator
            .vector_exact(&mut storage, 1, "experimental import options")
            .is_err()
        || storage.capacity() != 1
    {
        return Route::Error {
            message: "cannot allocate experimental import options".into(),
            json,
            operation,
        };
    }
    storage.push(options);
    let request = storage
        .into_boxed_slice()
        .try_into()
        .expect("one admitted import option");
    Route::TypedImport { request }
}

fn producer_route(
    options: ProducerOptions,
    allocator: &mut super::project::budget::Allocator,
) -> Route {
    let json = options.json;
    let operation = options.operation;
    // One exact, fallible allocation owns the new opt-in carrier. Conversion
    // cannot need a shrink/reallocation because actual capacity is exactly one.
    let mut storage = Vec::new();
    if std::mem::size_of::<ProducerOptions>() as u64
        > super::declaration_index::IndexLimits::default().retained
        || allocator
            .vector_exact(&mut storage, 1, "experimental producer options")
            .is_err()
        || storage.capacity() != 1
    {
        return Route::Error {
            message: "cannot allocate experimental producer options".into(),
            json,
            operation,
        };
    }
    storage.push(options);
    let request = storage
        .into_boxed_slice()
        .try_into()
        .expect("one admitted producer option");
    Route::TypedProducer { request }
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
    use super::{route, EntryPolicy, Route};

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
    fn nonexecuting_commands_keep_ordinary_errors_with_entry_options() {
        for command in ["check", "fmt", "compile"] {
            for value in ["result", "process", "unknown", ""] {
                for json in [false, true] {
                    let mut values = vec![command, "--edition=typed-preview", "file.ox"];
                    if json {
                        values.push("--message-format=json");
                    }
                    values.extend(["--entry-mode", value]);
                    let classified = route(&arguments(&values));
                    assert!(
                        matches!(classified, Route::Error { json: actual, .. } if actual == json),
                        "{values:?}: {classified:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn public_entry_options_select_only_run_and_compile() {
        for (value, policy) in [
            ("result", EntryPolicy::Result),
            ("process", EntryPolicy::Process),
        ] {
            for values in [
                vec![
                    "--entry-mode",
                    value,
                    "run",
                    "file.ox",
                    "--edition=typed-preview",
                ],
                vec![
                    "run",
                    "--entry-mode",
                    value,
                    "--edition=typed-preview",
                    "file.ox",
                ],
                vec![
                    "run",
                    "file.ox",
                    "--edition=typed-preview",
                    "--entry-mode",
                    value,
                ],
            ] {
                assert_eq!(
                    route(&arguments(&values)),
                    Route::TypedRun {
                        path: "file.ox".into(),
                        json: false,
                        entry_policy: policy,
                    }
                );
            }
            assert_eq!(
                route(&arguments(&[
                    "--edition=typed-preview",
                    "compile",
                    "file.ox",
                    "--backend=llvm",
                    "--output=program",
                    "--message-format=json",
                    "--entry-mode",
                    value,
                ])),
                Route::TypedCompile {
                    path: "file.ox".into(),
                    json: true,
                    output: "program".into(),
                    entry_policy: policy,
                }
            );
        }
        assert_eq!(
            route(&arguments(&[
                "run",
                "file.ox",
                "--edition=typed-preview",
                "--entry-mode=result",
                "--message-format=json",
            ])),
            Route::TypedRun {
                path: "file.ox".into(),
                json: true,
                entry_policy: EntryPolicy::Result,
            }
        );
        assert!(matches!(
            route(&arguments(&[
                "run",
                "file.ox",
                "--edition=typed-preview",
                "--entry-mode=process"
            ])),
            Route::TypedRun {
                entry_policy: EntryPolicy::Process,
                ..
            }
        ));
        for command in ["script", "unknown"] {
            assert!(matches!(
                route(&arguments(&[
                    "--entry-mode=process",
                    "--edition=typed-preview",
                    command,
                    "file.ox"
                ])),
                Route::ProcessError { .. }
            ));
        }
        assert!(matches!(
            route(&arguments(&["run", "file.ox", "--entry-mode=process"])),
            Route::ProcessError { .. }
        ));
    }

    #[test]
    fn process_run_json_and_early_errors_are_text_only() {
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
            let mut values = vec![
                "run",
                "file.ox",
                "--edition=typed-preview",
                "--entry-mode=process",
            ];
            values.extend_from_slice(suffix);
            assert!(
                matches!(route(&arguments(&values)), Route::ProcessError { .. }),
                "{values:?}"
            );
        }
        for values in [
            &["run", "--edition=typed-preview", "--entry-mode=process"][..],
            &[
                "run",
                "file.ox",
                "--edition=unknown",
                "--entry-mode=process",
                "--message-format=json",
            ],
            &[
                "run",
                "file.ox",
                "--edition=typed-preview",
                "--entry-mode=unknown",
                "--entry-mode=process",
            ],
        ] {
            assert!(matches!(
                route(&arguments(values)),
                Route::ProcessError { .. }
            ));
        }
        for selection in [
            &["--entry-mode"][..],
            &["--entry-mode="],
            &["--entry-mode=unknown"],
        ] {
            let mut values = vec![
                "run",
                "file.ox",
                "--edition=typed-preview",
                "--message-format=json",
            ];
            values.extend_from_slice(selection);
            match route(&arguments(&values)) {
                Route::ProcessError { message } => {
                    assert!(message.contains("--entry-mode"));
                }
                other => panic!("expected entry option error for {values:?}, got {other:?}"),
            }
        }
        for suffix in [&["--unknown"][..], &["--entry-mode=result"]] {
            let mut values = vec![
                "run",
                "file.ox",
                "--edition=typed-preview",
                "--entry-mode=result",
                "--message-format=json",
            ];
            values.extend_from_slice(suffix);
            assert!(matches!(
                route(&arguments(&values)),
                Route::Error { json: true, .. }
            ));
        }
        // Compilation never enters process execution and keeps JSON reporting.
        assert!(matches!(
            route(&arguments(&[
                "compile",
                "file.ox",
                "--edition=typed-preview",
                "--entry-mode=process",
                "--message-format=json",
                "--backend=unknown",
                "--output=program"
            ])),
            Route::Error { json: true, .. }
        ));
    }

    #[test]
    fn public_routes_preserve_script_and_separator_boundaries() {
        for values in [
            &[
                "script",
                "task",
                "--entry-mode",
                "process",
                "--edition=typed-preview",
            ][..],
            &[
                "run",
                "file.ox",
                "--",
                "--entry-mode=process",
                "--edition=typed-preview",
            ],
        ] {
            let args = arguments(values);
            assert_eq!(route(&args), Route::Legacy(args));
        }
        {
            assert_eq!(
                route(&arguments(&[
                    "run",
                    "--edition=typed-preview",
                    "--",
                    "--entry-mode=process"
                ])),
                Route::TypedRun {
                    path: "--entry-mode=process".into(),
                    json: false,
                    entry_policy: EntryPolicy::Result,
                }
            );
            assert_eq!(
                route(&arguments(&[
                    "script",
                    "--edition=legacy-0.9",
                    "task",
                    "--entry-mode=process"
                ])),
                Route::Legacy(arguments(&["script", "task", "--entry-mode=process"]))
            );
        }
        assert_eq!(
            route(&arguments(&[
                "run",
                "--edition=typed-preview",
                "--entry-mode=process",
                "--",
                "--entry-mode=result",
            ])),
            Route::TypedRun {
                path: "--entry-mode=result".into(),
                json: false,
                entry_policy: EntryPolicy::Process,
            }
        );
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

#[cfg(test)]
mod import_options_tests {
    use super::*;
    fn parse(args: &[&str]) -> Route {
        route(
            &args
                .iter()
                .map(|arg| (*arg).to_string())
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn experimental_import_requires_typed_result_semantic_operation() {
        for operation in ["check", "run", "compile"] {
            let mut args = vec![
                operation,
                "source.ox",
                "--edition=typed-preview",
                "--experimental-hir-import=wire.bin",
            ];
            if operation == "compile" {
                args.extend(["--backend=llvm", "--output=new.elf"]);
            }
            assert!(
                matches!(parse(&args), Route::TypedImport { request } if request[0].path == "source.ox" && request[0].observation == "wire.bin")
            );
        }
        for args in [
            vec!["run", "x", "--experimental-hir-import=wire"],
            vec![
                "run",
                "x",
                "--edition=legacy-0.9",
                "--experimental-hir-import=wire",
            ],
            vec![
                "fmt",
                "x",
                "--edition=typed-preview",
                "--experimental-hir-import=wire",
            ],
            vec![
                "run",
                "x",
                "--edition=typed-preview",
                "--entry-mode=process",
                "--experimental-hir-import=wire",
            ],
            vec![
                "check",
                "x",
                "--edition=typed-preview",
                "--experimental-hir-import=",
            ],
            vec![
                "check",
                "x",
                "--edition=typed-preview",
                "--experimental-hir-import=a",
                "--experimental-hir-import=b",
            ],
        ] {
            assert!(matches!(
                parse(&args),
                Route::Error { .. } | Route::ProcessError { .. }
            ));
        }
        assert!(matches!(
            parse(&["script", "test", "--experimental-hir-import=wire"]),
            Route::Legacy(_)
        ));
        assert!(
            matches!(parse(&["run", "--edition=typed-preview", "--", "--experimental-hir-import=wire"]), Route::TypedRun { path, .. } if path == "--experimental-hir-import=wire")
        );
    }

    #[test]
    fn experimental_import_box_is_fallible_and_default_route_layout_stable() {
        let options = || ImportOptions {
            path: "source.ox".into(),
            observation: "wire.bin".into(),
            output: None,
            json: false,
            operation: Operation::Check,
        };
        let mut allocator = super::super::project::budget::Allocator {
            fail_at: Some(1),
            ..Default::default()
        };
        assert!(matches!(
            import_route(options(), &mut allocator),
            Route::Error { .. }
        ));
        assert_eq!(allocator.attempts, 1);
        let mut allocator = super::super::project::budget::Allocator::default();
        allocator.observer_trace_bound(1).unwrap();
        assert!(matches!(
            import_route(options(), &mut allocator),
            Route::TypedImport { .. }
        ));
        assert_eq!(allocator.attempts, 1);
        assert_eq!(allocator.trace[0].kind, "experimental import options");
        assert_eq!(allocator.trace[0].length, 1);
        #[cfg(target_pointer_width = "64")]
        assert_eq!(std::mem::size_of::<Route>(), 56);
    }
}

#[cfg(test)]
mod producer_options_tests {
    use super::*;

    fn parse(values: &[&str]) -> Route {
        route(
            &values
                .iter()
                .map(|value| (*value).into())
                .collect::<Vec<_>>(),
        )
    }

    fn expect_error(values: &[&str], expected: &str) {
        match parse(values) {
            Route::Error { message, .. } | Route::ProcessError { message } => {
                assert!(message.contains(expected), "{values:?}: {message}");
            }
            other => panic!("expected {expected} error for {values:?}, got {other:?}"),
        }
    }

    #[test]
    fn producers_accept_all_result_operations_and_global_option_positions() {
        for (command, operation) in [
            ("check", Operation::Check),
            ("run", Operation::Run),
            ("compile", Operation::Compile),
        ] {
            for prefix in [true, false] {
                for json in [false, true] {
                    let mut values = vec!["--edition=typed-preview"];
                    if prefix {
                        values.extend(["--experimental-hir-producers", "bundle dir"]);
                    }
                    values.extend([command, "source.ox"]);
                    if !prefix {
                        values.push("--experimental-hir-producers=bundle dir");
                    }
                    if json {
                        values.push("--message-format=json");
                    }
                    if command != "check" {
                        values.push("--entry-mode=result");
                    }
                    if command == "compile" {
                        values.extend([
                            "--backend=llvm",
                            "--target=x86_64-unknown-linux-gnu",
                            "--output=new.elf",
                        ]);
                    }
                    let Route::TypedProducer { request } = parse(&values) else {
                        panic!("producer route was not selected for {values:?}");
                    };
                    assert_eq!(request[0].path, "source.ox");
                    assert_eq!(request[0].bundle, "bundle dir");
                    assert_eq!(request[0].operation, operation);
                    assert_eq!(request[0].json, json);
                    assert_eq!(
                        request[0].output.as_deref(),
                        (command == "compile").then_some("new.elf")
                    );
                }
            }
        }
    }

    #[test]
    fn producers_require_explicit_typed_edition_and_semantic_command() {
        for edition in [None, Some("--edition=legacy-0.9")] {
            let mut values = vec!["run", "missing.ox", "--experimental-hir-producers=bundle"];
            values.extend(edition);
            expect_error(&values, "requires explicit typed-preview");
        }
        for command in ["fmt", "build", "check-other", "script", "help", "unknown"] {
            // Place the recognized option before script's command-owned tail.
            expect_error(
                &[
                    "--edition=typed-preview",
                    "--experimental-hir-producers=bundle",
                    command,
                    "missing.ox",
                ],
                "requires explicit typed-preview",
            );
        }
        expect_error(
            &[
                "--edition=typed-preview",
                "--experimental-hir-producers=bundle",
            ],
            "requires explicit typed-preview",
        );
    }

    #[test]
    fn producers_reject_empty_duplicate_conflicting_and_process_selections() {
        for selection in [
            vec!["--experimental-hir-producers"],
            vec!["--experimental-hir-producers="],
            vec!["--experimental-hir-producers", ""],
            vec!["--experimental-hir-producers", "--message-format=text"],
        ] {
            let mut values = vec!["run", "missing.ox", "--edition=typed-preview"];
            values.extend(selection);
            expect_error(&values, "requires a value");
        }
        for selection in [
            vec![
                "--experimental-hir-producers=a",
                "--experimental-hir-producers=b",
            ],
            vec![
                "--experimental-hir-producers",
                "a",
                "--experimental-hir-producers",
                "a",
            ],
        ] {
            let mut values = vec!["run", "missing.ox", "--edition=typed-preview"];
            values.extend(selection);
            expect_error(&values, "may only be specified once");
        }
        for selection in [
            [
                "--experimental-hir-import=wire",
                "--experimental-hir-producers=bundle",
            ],
            [
                "--experimental-hir-producers=bundle",
                "--experimental-hir-import=wire",
            ],
        ] {
            let mut values = vec!["run", "missing.ox", "--edition=typed-preview"];
            values.extend(selection);
            expect_error(&values, "mutually exclusive");
        }
        for command in ["run", "compile"] {
            for json in [false, true] {
                let mut values = vec![
                    command,
                    "missing.ox",
                    "--edition=typed-preview",
                    "--experimental-hir-producers=bundle",
                    "--entry-mode=process",
                ];
                if json {
                    values.push("--message-format=json");
                }
                expect_error(&values, "only Result entry mode");
                match parse(&values) {
                    Route::ProcessError { .. } => assert_eq!(command, "run"),
                    Route::Error { json: actual, .. } => {
                        assert_eq!(command, "compile");
                        assert_eq!(actual, json);
                    }
                    other => panic!("wrong process error channel: {other:?}"),
                }
            }
        }
    }

    #[test]
    fn producers_keep_script_and_separator_ownership() {
        let values = [
            "script",
            "test",
            "--experimental-hir-producers=bundle",
            "--experimental-hir-import=wire",
            "--edition=typed-preview",
        ];
        assert_eq!(
            parse(&values),
            Route::Legacy(values.iter().map(|value| (*value).into()).collect())
        );
        assert_eq!(
            parse(&[
                "run",
                "--edition=typed-preview",
                "--",
                "--experimental-hir-producers=bundle"
            ]),
            Route::TypedRun {
                path: "--experimental-hir-producers=bundle".into(),
                json: false,
                entry_policy: EntryPolicy::Result,
            }
        );
        let Route::TypedProducer { request } = parse(&[
            "run",
            "--edition=typed-preview",
            "--experimental-hir-producers=bundle",
            "--",
            "--experimental-hir-import=literal-source",
        ]) else {
            panic!("separator did not preserve a literal producer source");
        };
        assert_eq!(request[0].path, "--experimental-hir-import=literal-source");
        expect_error(
            &[
                "run",
                "--edition=typed-preview",
                "--experimental-hir-producers=bundle",
                "--",
                "source.ox",
                "--entry-mode=process",
            ],
            "exactly one source path",
        );
    }

    #[test]
    fn producers_preserve_compile_backend_output_and_source_validation() {
        for (tail, expected) in [
            (vec!["source.ox"], "requires --backend llvm"),
            (vec!["source.ox", "--backend=llvm"], "requires --output"),
            (
                vec!["source.ox", "--backend=other", "--output=new.elf"],
                "requires --backend llvm",
            ),
            (
                vec![
                    "source.ox",
                    "--backend=llvm",
                    "--output=new.elf",
                    "--target=other",
                ],
                "only --target",
            ),
            (
                vec!["source.ox", "--backend=llvm", "--output="],
                "requires one nonempty value",
            ),
            (
                vec!["source.ox", "--backend=llvm", "--output=a", "--output=b"],
                "may occur only once",
            ),
            (
                vec!["--backend=llvm", "--output=new.elf"],
                "exactly one source path",
            ),
            (
                vec![
                    "source.ox",
                    "other.ox",
                    "--backend=llvm",
                    "--output=new.elf",
                ],
                "exactly one source path",
            ),
        ] {
            let mut values = vec![
                "compile",
                "--edition=typed-preview",
                "--experimental-hir-producers=bundle",
            ];
            values.extend(tail);
            expect_error(&values, expected);
        }
        expect_error(
            &[
                "check",
                "source.ox",
                "--edition=typed-preview",
                "--experimental-hir-producers=bundle",
                "--output=new.elf",
            ],
            "unsupported option",
        );
    }

    #[test]
    fn producer_box_uses_one_fallible_exact_allocation_and_moves_strings() {
        let options = || ProducerOptions {
            path: "source.ox".into(),
            bundle: "bundle".into(),
            output: Some("new.elf".into()),
            json: true,
            operation: Operation::Compile,
        };
        let mut allocator = super::super::project::budget::Allocator {
            fail_at: Some(1),
            ..Default::default()
        };
        assert!(matches!(
            producer_route(options(), &mut allocator),
            Route::Error {
                json: true,
                operation: Operation::Compile,
                ..
            }
        ));
        assert_eq!(allocator.attempts, 1);
        assert_eq!(allocator.trace[0].kind, "experimental producer options");
        assert!(!allocator.trace[0].success);

        let mut allocator = super::super::project::budget::Allocator::default();
        allocator.observer_trace_bound(1).unwrap();
        let options = options();
        let original = (
            options.path.as_ptr(),
            options.bundle.as_ptr(),
            options.output.as_ref().unwrap().as_ptr(),
        );
        let Route::TypedProducer { request } = producer_route(options, &mut allocator) else {
            panic!("producer allocation failed");
        };
        assert_eq!(allocator.attempts, 1);
        assert_eq!(allocator.trace.len(), 1);
        assert_eq!(allocator.trace[0].kind, "experimental producer options");
        assert_eq!(allocator.trace[0].length, 1);
        assert_eq!(
            allocator.trace[0].element_bytes,
            std::mem::size_of::<ProducerOptions>()
        );
        assert!(allocator.trace[0].success);
        assert_eq!(
            original,
            (
                request[0].path.as_ptr(),
                request[0].bundle.as_ptr(),
                request[0].output.as_ref().unwrap().as_ptr(),
            )
        );
        #[cfg(target_pointer_width = "64")]
        assert_eq!(std::mem::size_of::<Route>(), 56);
    }
}
