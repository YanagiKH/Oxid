//! Borrow-only opt-in gate. Calls without an eligible selector never allocate
//! here and go unchanged to the old classifier, including Process error routing.
//! Both RUN engines and BUILD admit explicit process entry mode.
use super::{word, Allocator, Failure, Operation, Route};
use crate::frontend::options::{BuildProfile, EntryPolicy, PROFILE_APPLICABILITY};
const OPTIONS: [&str; 15] = [
    "--manifest-path",
    "--edition",
    "--message-format",
    "--entry-mode",
    "--experimental-hir-import",
    "--experimental-hir-producers",
    "--experimental-lexical-provider",
    "--backend",
    "--target",
    "--output",
    "--engine",
    "--package",
    "--filter",
    "--fail-fast",
    // Append a global selector without renumbering established local option bits.
    "--profile",
];
#[derive(Clone, Copy)]
enum Error<'a> {
    Duplicate(&'static str),
    Missing(&'static str),
    UnexpectedValue(&'static str),
    Edition(&'a str),
    Format(&'a str),
    Entry(&'a str),
    Profile(&'a str),
    Engine(&'a str),
    Unsupported(&'a str),
}
impl Error<'_> {
    fn message(self, operation: &str) -> String {
        match self {
            Self::Duplicate(n) => format!("{n} may only be specified once"),
            Self::Missing(n) => format!("{n} requires a value"),
            Self::UnexpectedValue(n) => format!("{n} does not accept a value"),
            Self::Edition(v) => {
                format!("unknown edition `{v}`; expected legacy-0.9 or typed-preview")
            }
            Self::Format(v) => format!("unknown --message-format `{v}`; expected text or json"),
            Self::Entry(v) => format!("unknown --entry-mode `{v}`; expected result or process"),
            Self::Profile(v) => format!("unknown --profile `{v}`; expected dev or release"),
            Self::Engine(v) => format!("unknown --engine `{v}`; expected interpreter or native"),
            Self::Unsupported(v) => {
                format!("unsupported option for manifest {operation}: {v}")
            }
        }
    }
}
#[derive(Default)]
pub(super) struct Scan<'a> {
    path: Option<&'a str>,
    package: Option<&'a str>,
    filter: Option<usize>,
    output: Option<&'a str>,
    command: Option<&'a str>,
    edition: Option<&'a str>,
    error: Option<Error<'a>>,
    local_error: Option<Error<'a>>,
    seen: u16,
    json: bool,
    before: bool,
    operands: bool,
    script_name: bool,
    separated: bool,
    process: bool,
    native: bool,
    fail_fast: bool,
    profile: BuildProfile,
}
pub(super) fn select(args: &[String]) -> Option<Route> {
    select_with(args, Allocator::default())
}
pub(super) fn select_with(args: &[String], allocator: Allocator) -> Option<Route> {
    let mut scan = Scan::default();
    let mut index = 0;
    while index < args.len() {
        let argument_index = index;
        let arg = args[index].as_str();
        index += 1;
        if scan.script_name {
            break;
        }
        if scan.separated {
            scan.operands = true;
            continue;
        }
        if arg == "--" {
            scan.separated = true;
            continue;
        }
        // Only true globals can precede the first forwarded command word.
        // Compile-local options are also ordinary names after `script`.
        let option = OPTIONS.iter().enumerate().position(|(slot, name)| {
            (slot < 7 || slot == 14 || scan.command.is_some_and(|command| command != "script"))
                && (arg == *name || arg.strip_prefix(name).is_some_and(|s| s.starts_with('=')))
        });
        if let Some(slot) = option {
            let name = OPTIONS[slot];
            // Preserve the existing global scan tier before local validation.
            let error = if slot < 7 || slot == 14 {
                &mut scan.error
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
            // This local flag never consumes the following argument. Preserve the
            // first scanner error in this tier, including a prior duplicate.
            if slot == 13 {
                if arg.contains('=') {
                    error.get_or_insert(Error::UnexpectedValue(name));
                }
                scan.fail_fast = true;
                continue;
            }
            let value = if let Some((_, value)) = arg.split_once('=') {
                Some(value)
            } else if let Some(next) = args.get(index).filter(|s| !s.starts_with('-')) {
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
                (1, Some(value @ ("typed-preview" | "legacy-0.9"))) => scan.edition = Some(value),
                (1, Some(value)) => {
                    error.get_or_insert(Error::Edition(value));
                }
                (2, Some("json")) => scan.json = true,
                (2, Some("text")) => {}
                (2, Some(value)) => {
                    error.get_or_insert(Error::Format(value));
                }
                (3, Some(value @ ("result" | "process"))) => scan.process = value == "process",
                (3, Some(value)) => {
                    error.get_or_insert(Error::Entry(value));
                }
                (10, Some(value @ ("interpreter" | "native"))) => scan.native = value == "native",
                (10, Some(value)) => {
                    error.get_or_insert(Error::Engine(value));
                }
                (11, value) => scan.package = value,
                (12, Some(_)) => {
                    scan.filter = Some(if arg.contains('=') {
                        argument_index
                    } else {
                        argument_index + 1
                    })
                }
                (9, value) => scan.output = value,
                (14, Some("dev")) => scan.profile = BuildProfile::Dev,
                (14, Some("release")) => scan.profile = BuildProfile::Release,
                (14, Some(value)) => {
                    error.get_or_insert(Error::Profile(value));
                }
                _ => {}
            }
        } else if scan.command.is_none() {
            scan.command = Some(arg);
            if arg.starts_with('-') {
                scan.local_error.get_or_insert(Error::Unsupported(arg));
            }
        } else if scan.command == Some("script") {
            scan.script_name = true;
            scan.operands = true;
        } else {
            if arg.starts_with('-') {
                scan.local_error.get_or_insert(Error::Unsupported(arg));
            } else {
                scan.operands = true;
            }
        }
    }
    if scan.seen & 1 == 0 {
        return None;
    }
    // Global-tier errors keep the CHECK summary even after a `run` word. Otherwise
    // an actual first command word of `run` reports with the RUN summary; any
    // other word, including an unrecognized one, keeps the CHECK fallback.
    let operation = if scan.error.is_none() && scan.command == Some("run") {
        Operation::Run
    } else {
        Operation::Check
    };
    let build = scan.command == Some("build");
    let package = scan.command == Some("package");
    let test = scan.command == Some("test");
    let run = scan.command == Some("run");
    let native_run = run && scan.native;
    let entry_policy = if scan.process {
        EntryPolicy::Process
    } else {
        EntryPolicy::Result
    };
    let op = if package {
        "package"
    } else if build {
        "build"
    } else if test {
        "test"
    } else {
        word(operation)
    };
    let message = if let Some(error) = scan.error.or(scan.local_error) {
        Some(error.message(op))
    } else if !matches!(
        scan.command,
        Some("check" | "run" | "build" | "test" | "package")
    ) {
        // Preserve the existing fallback diagnostic for unrelated commands.
        Some("--manifest-path requires check, run or build, or test".into())
    } else if scan.before {
        Some(format!("--manifest-path must follow {op}"))
    } else if scan.edition == Some("legacy-0.9") {
        Some(format!("manifest {op} conflicts with --edition legacy-0.9"))
    } else if scan.operands {
        Some(format!("manifest {op} does not accept a source operand"))
    } else if !test && scan.seen & (1 << 12) != 0 {
        Some("--filter is only supported by explicit manifest test".into())
    } else if !test && scan.seen & (1 << 13) != 0 {
        Some("--fail-fast is only supported by explicit manifest test".into())
    } else if !run && !test && scan.seen & (1 << 10) != 0 {
        Some("--engine is only supported by manifest run or test".into())
    } else if scan.seen & (1 << 14) != 0
        && !(build || package || native_run || (test && scan.native))
    {
        Some(PROFILE_APPLICABILITY.into())
    } else if scan.seen
        & !((if build || run || package { 15 } else { 7 })
            | (if run || test { 1 << 10 } else { 0 })
            | (if package { 1 << 9 } else { 0 })
            | (1 << 11)
            | (1 << 14)
            | (if test { (1 << 12) | (1 << 13) } else { 0 }))
        != 0
    {
        Some(if package {
            "manifest package does not support backend, target or experimental selectors".into()
        } else if build || run {
            format!(
                "manifest {op} does not support backend, target, output or experimental selectors"
            )
        } else {
            format!("manifest {op} does not support entry-mode, backend, target, output or experimental selectors")
        })
    } else if native_run && scan.json {
        Some("native manifest run does not support --message-format=json".into())
    } else if run && scan.process && scan.json {
        Some("process-mode run does not support --message-format=json".into())
    } else if test && scan.json {
        Some("manifest test does not support --message-format=json".into())
    } else if package && scan.output.is_none() {
        Some("manifest package requires --output ARCHIVE".into())
    } else {
        None
    };
    if let Some(message) = message {
        if run && scan.process && !native_run {
            return Some(Route::ProcessError { message });
        }
        if package {
            return Some(Route::ManifestPackageError {
                failure: Failure::Cli(message),
                json: scan.json,
                entry_policy,
            });
        }
        if test {
            return Some(Route::ManifestTestError {
                failure: Failure::Cli(message),
                json: scan.json,
            });
        }
        if build {
            return Some(Route::ManifestBuildError {
                failure: Failure::Cli(message),
                json: scan.json,
                entry_policy,
            });
        }
        return Some(Route::ManifestError {
            failure: Failure::Cli(message),
            json: scan.json,
            operation,
        });
    }
    // The package-only owned path is fallibly copied after CLI validation. It
    // does not enlarge Request or change the manifest allocation sequence.
    let output = if package {
        let text = scan.output.expect("validated package output");
        let mut output = String::new();
        if output.try_reserve_exact(text.len()).is_err() {
            return Some(Route::ManifestPackageError {
                failure: Failure::Resource("package output path allocation failed"),
                json: scan.json,
                entry_policy,
            });
        }
        output.push_str(text);
        output
    } else {
        String::new()
    };
    Some(
        match super::request_selected(scan.path.unwrap_or(""), scan.package, scan.json, allocator) {
            Ok(request) if package => Route::ManifestPackage {
                request,
                output,
                entry_policy,
                profile: scan.profile,
            },
            Ok(request) if test => Route::ManifestTest {
                request,
                native: scan.native,
                filter: scan.filter,
                fail_fast: scan.fail_fast,
                profile: scan.profile,
            },
            Ok(request) if build => Route::ManifestBuild {
                request,
                entry_policy,
                profile: scan.profile,
            },
            Ok(request) if native_run => Route::ManifestNativeRun {
                request,
                entry_policy,
                profile: scan.profile,
            },
            Ok(request) if run && scan.process => Route::ManifestProcessRun { request },
            Ok(request) if operation == Operation::Run => Route::ManifestRun { request },
            Ok(request) => Route::ManifestCheck { request },
            Err(failure) if package => Route::ManifestPackageError {
                failure,
                json: scan.json,
                entry_policy,
            },
            Err(failure) if build => Route::ManifestBuildError {
                failure,
                json: scan.json,
                entry_policy,
            },
            Err(failure) if test => Route::ManifestTestError {
                failure,
                json: scan.json,
            },
            Err(failure) if run && scan.process && !native_run => {
                Route::ManifestProcessError { failure }
            }
            Err(failure) => Route::ManifestError {
                failure,
                json: scan.json,
                operation,
            },
        },
    )
}

#[cfg(test)]
mod package_tests {
    use super::*;
    #[test]
    fn package_cli_carriers_and_legacy_boundaries() {
        use std::mem::{align_of, size_of};
        println!(
            "PACKAGE_CARRIERS route={} request={} scan={} inventory={} route_alignment={}",
            size_of::<Route>(),
            size_of::<super::super::Request>(),
            size_of::<Scan<'_>>(),
            super::super::inventory().unwrap(),
            align_of::<Route>()
        );
        let args = |words: &[&str]| words.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        for words in [
            vec!["package", "--output=x.tar"],
            vec!["script", "name", "--manifest-path=x", "--output=y"],
            vec!["package", "--", "--manifest-path=x"],
        ] {
            assert!(select(&args(&words)).is_none());
        }
        let words = args(&[
            "package",
            "--manifest-path=x",
            "--output=out.tar",
            "--package=app",
            "--entry-mode=process",
        ]);
        let Some(Route::ManifestPackage {
            output,
            entry_policy,
            request,
            profile,
        }) = select(&words)
        else {
            panic!("package route");
        };
        assert_eq!(output, "out.tar");
        assert_eq!(profile, BuildProfile::Dev);
        assert_eq!(entry_policy, EntryPolicy::Process);
        assert_eq!(request[0].package.as_deref(), Some("app"));
    }
}

#[cfg(test)]
mod filter_tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).into()).collect()
    }

    #[test]
    fn filter_retains_original_argv_index_without_text_allocation() {
        for (words, expected_index, expected) in [
            (
                vec!["test", "--filter", "keep", "--manifest-path=p"],
                2,
                "keep",
            ),
            (
                vec!["test", "--manifest-path=p", "--filter=keep"],
                2,
                "keep",
            ),
            (
                vec![
                    "--edition=typed-preview",
                    "test",
                    "--filter=日本",
                    "--manifest-path=p",
                ],
                2,
                "日本",
            ),
            (
                vec!["test", "--manifest-path=p", "--filter=--filter=x"],
                2,
                "--filter=x",
            ),
        ] {
            let arguments = args(&words);
            let Some(Route::ManifestTest {
                filter: Some(index),
                ..
            }) = select(&arguments)
            else {
                panic!("expected filter route: {words:?}");
            };
            assert_eq!(index, expected_index);
            let text = arguments[index]
                .strip_prefix("--filter=")
                .unwrap_or(&arguments[index]);
            assert_eq!(text, expected);
        }
        assert!(matches!(
            select(&args(&["test", "--manifest-path=p"])),
            Some(Route::ManifestTest { filter: None, .. })
        ));
        println!(
            "FILTER_CARRIERS route={} scan={} request={} filter_index={}",
            std::mem::size_of::<Route>(),
            std::mem::size_of::<Scan<'_>>(),
            std::mem::size_of::<super::super::Request>(),
            std::mem::size_of::<Option<usize>>()
        );
    }

    #[test]
    fn filter_cli_preserves_global_error_priority_and_json_refusal() {
        for (tail, expected, json) in [
            (
                vec!["--filter", "--engine=native"],
                "--filter requires a value",
                false,
            ),
            (vec!["--filter="], "--filter requires a value", false),
            (
                vec!["--filter=a", "--filter", "a"],
                "--filter may only be specified once",
                false,
            ),
            (
                vec!["--filter=", "--edition=wrong"],
                "unknown edition",
                false,
            ),
            (
                vec!["--filter=a", "--message-format=json"],
                "manifest test does not support --message-format=json",
                true,
            ),
            (
                vec!["--filter=", "--message-format=json"],
                "--filter requires a value",
                true,
            ),
        ] {
            let mut words = vec!["test", "--manifest-path=missing"];
            words.extend(tail);
            let Some(Route::ManifestTestError {
                failure,
                json: actual_json,
            }) = select(&args(&words))
            else {
                panic!("expected filter refusal: {words:?}");
            };
            let message = failure.message();
            assert!(message.contains(expected), "{message}");
            assert_eq!(actual_json, json);
        }
    }

    #[test]
    fn filter_preserves_script_arguments_and_separator_ownership() {
        use crate::frontend::options;
        for words in [
            vec!["script", "task", "--filter", "keep", "--manifest-path=p"],
            vec!["script", "task", "--filter=keep", "--edition=typed-preview"],
            vec!["test", "--", "--filter=keep", "--manifest-path=p"],
        ] {
            let arguments = args(&words);
            assert!(select(&arguments).is_none());
            assert_eq!(options::route(&arguments), Route::Legacy(arguments));
        }
        assert_eq!(
            options::route(&args(&[
                "check",
                "--edition=typed-preview",
                "--",
                "--filter=literal.ox"
            ])),
            Route::TypedCheck {
                path: "--filter=literal.ox".into(),
                json: false
            }
        );
        let Some(Route::ManifestTestError { failure, .. }) =
            select(&args(&["test", "--manifest-path=p", "--", "--filter=keep"]))
        else {
            panic!("manifest test must reject a literal source operand");
        };
        assert!(failure
            .message()
            .contains("does not accept a source operand"));
    }
}

#[cfg(test)]
mod fail_fast_tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).into()).collect()
    }

    #[test]
    fn fail_fast_is_valueless_and_composes_with_filter_and_both_engines() {
        for engine in ["--engine=interpreter", "--engine=native"] {
            for words in [
                vec!["test", "--fail-fast", "--manifest-path=p", engine],
                vec![
                    "test",
                    "--manifest-path=p",
                    "--filter=keep",
                    engine,
                    "--fail-fast",
                ],
                vec![
                    "test",
                    "--fail-fast",
                    "--filter",
                    "keep",
                    "--manifest-path=p",
                    engine,
                ],
            ] {
                let Some(Route::ManifestTest {
                    native, fail_fast, ..
                }) = select(&args(&words))
                else {
                    panic!("expected fail-fast route: {words:?}");
                };
                assert!(fail_fast);
                assert_eq!(native, engine == "--engine=native");
            }
        }
        assert!(matches!(
            select(&args(&["test", "--manifest-path=p"])),
            Some(Route::ManifestTest {
                fail_fast: false,
                ..
            })
        ));
        println!(
            "FAIL_FAST_CARRIERS route={} scan={} request={}",
            std::mem::size_of::<Route>(),
            std::mem::size_of::<Scan<'_>>(),
            std::mem::size_of::<super::super::Request>()
        );
    }

    #[test]
    fn fail_fast_errors_keep_first_error_per_tier_and_json_refusal() {
        for (tail, expected, json) in [
            (
                vec!["--fail-fast="],
                "--fail-fast does not accept a value",
                false,
            ),
            (
                vec!["--fail-fast=true"],
                "--fail-fast does not accept a value",
                false,
            ),
            (
                vec!["--fail-fast", "--fail-fast="],
                "--fail-fast may only be specified once",
                false,
            ),
            (
                vec!["--fail-fast=", "--fail-fast"],
                "--fail-fast does not accept a value",
                false,
            ),
            (
                vec!["--filter=", "--fail-fast="],
                "--filter requires a value",
                false,
            ),
            (
                vec!["--fail-fast=", "--filter="],
                "--fail-fast does not accept a value",
                false,
            ),
            (
                vec!["--fail-fast=", "--edition=wrong"],
                "unknown edition `wrong`; expected legacy-0.9 or typed-preview",
                false,
            ),
            (
                vec!["--fail-fast", "--message-format=json"],
                "manifest test does not support --message-format=json",
                true,
            ),
            (
                vec!["--fail-fast=", "--message-format=json"],
                "--fail-fast does not accept a value",
                true,
            ),
            (
                vec!["--fail-fast", "operand"],
                "manifest test does not accept a source operand",
                false,
            ),
        ] {
            let mut words = vec!["test", "--manifest-path=missing"];
            words.extend(tail);
            let Some(Route::ManifestTestError {
                failure,
                json: actual_json,
            }) = select(&args(&words))
            else {
                panic!("expected fail-fast refusal: {words:?}");
            };
            assert_eq!(failure.message(), expected, "{words:?}");
            assert_eq!(actual_json, json);
        }
    }

    #[test]
    fn fail_fast_applicability_follows_filter_and_keeps_package_route() {
        for command in ["check", "run", "build", "package"] {
            for tail in [
                vec!["--fail-fast"],
                vec!["--fail-fast", "--filter=keep"],
                vec!["--filter=keep", "--fail-fast"],
            ] {
                let expected = if tail.len() == 1 {
                    "--fail-fast is only supported by explicit manifest test"
                } else {
                    "--filter is only supported by explicit manifest test"
                };
                let mut words = vec![command, "--manifest-path=missing"];
                words.extend(tail);
                let route = select(&args(&words)).expect("manifest route");
                if command == "package" {
                    assert!(matches!(&route, Route::ManifestPackageError { .. }));
                }
                let failure = match route {
                    Route::ManifestError { failure, .. }
                    | Route::ManifestBuildError { failure, .. }
                    | Route::ManifestPackageError { failure, .. } => failure,
                    other => panic!("expected applicability error: {other:?}"),
                };
                assert_eq!(failure.message(), expected, "{words:?}");
            }
        }
    }

    #[test]
    fn fail_fast_does_not_claim_legacy_script_or_separator_arguments() {
        use crate::frontend::options;
        for words in [
            vec!["test", "--fail-fast"],
            vec!["test", "--fail-fast=true"],
            vec!["script", "task", "--fail-fast", "--manifest-path=p"],
            vec!["script", "--fail-fast", "--manifest-path=p"],
            vec!["test", "--", "--fail-fast", "--manifest-path=p"],
        ] {
            let arguments = args(&words);
            assert!(select(&arguments).is_none());
            assert_eq!(options::route(&arguments), Route::Legacy(arguments));
        }
        assert_eq!(
            options::route(&args(&[
                "check",
                "--edition=typed-preview",
                "--",
                "--fail-fast.ox",
            ])),
            Route::TypedCheck {
                path: "--fail-fast.ox".into(),
                json: false,
            }
        );
        let Some(Route::ManifestTestError { failure, .. }) =
            select(&args(&["test", "--manifest-path=p", "--", "--fail-fast"]))
        else {
            panic!("manifest separator must preserve literal operand ownership");
        };
        assert_eq!(
            failure.message(),
            "manifest test does not accept a source operand"
        );
    }
}

#[cfg(test)]
mod profile_tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).into()).collect()
    }

    fn refusal(words: &[&str], expected: &str) {
        let route = select(&args(words)).expect("manifest selector");
        let failure = match route {
            Route::ManifestError { failure, .. }
            | Route::ManifestBuildError { failure, .. }
            | Route::ManifestPackageError { failure, .. }
            | Route::ManifestTestError { failure, .. } => failure,
            Route::ProcessError { message } => {
                assert_eq!(message, expected, "{words:?}");
                return;
            }
            other => panic!("expected profile refusal for {words:?}, got {other:?}"),
        };
        assert_eq!(failure.classification(), ("E0001", "cli"));
        assert_eq!(failure.message(), expected, "{words:?}");
    }

    #[test]
    fn native_profile_accepts_both_forms_at_every_global_position_without_edition() {
        for command in ["build", "package", "run", "test"] {
            let mut base = vec![
                command,
                "--manifest-path=workspace/oxid.toml",
                "--package=app",
            ];
            if command == "package" {
                base.push("--output=app.tar");
            }
            if command == "run" || command == "test" {
                base.push("--engine=native");
            }
            if command == "test" {
                base.extend(["--filter", "keep", "--fail-fast"]);
            } else {
                base.push("--entry-mode=process");
            }
            for (selection, expected) in [
                (vec![], BuildProfile::Dev),
                (vec!["--profile", "dev"], BuildProfile::Dev),
                (vec!["--profile=dev"], BuildProfile::Dev),
                (vec!["--profile", "release"], BuildProfile::Release),
                (vec!["--profile=release"], BuildProfile::Release),
            ] {
                for position in 0..=base.len() {
                    // An existing local option continues to own its value.
                    if position > 0 && base[position - 1] == "--filter" {
                        continue;
                    }
                    let mut words = base.clone();
                    words.splice(position..position, selection.iter().copied());
                    let arguments = args(&words);
                    let route = select(&arguments).expect("manifest selector");
                    let (request, profile) = match route {
                        Route::ManifestPackage {
                            request,
                            profile,
                            output,
                            entry_policy,
                        } => {
                            assert_eq!(command, "package");
                            assert_eq!(output, "app.tar");
                            assert_eq!(entry_policy, EntryPolicy::Process);
                            (request, profile)
                        }
                        Route::ManifestBuild {
                            request,
                            profile,
                            entry_policy,
                        }
                        | Route::ManifestNativeRun {
                            request,
                            profile,
                            entry_policy,
                        } => {
                            assert!(command == "build" || command == "run");
                            assert_eq!(entry_policy, EntryPolicy::Process);
                            (request, profile)
                        }
                        Route::ManifestTest {
                            request,
                            profile,
                            native,
                            filter,
                            fail_fast,
                        } => {
                            assert_eq!(command, "test");
                            assert!(native && fail_fast);
                            assert_eq!(arguments[filter.expect("filter index")], "keep");
                            (request, profile)
                        }
                        other => {
                            panic!("expected native profile route for {words:?}, got {other:?}")
                        }
                    };
                    assert_eq!(profile, expected, "{words:?}");
                    assert_eq!(request[0].path, "workspace/oxid.toml");
                    assert_eq!(request[0].package.as_deref(), Some("app"));
                    assert_eq!(request[0].allocator.attempts, 3);
                }
            }
        }
        assert!(matches!(
            select(&args(&[
                "test",
                "--manifest-path=p",
                "--engine=interpreter"
            ])),
            Some(Route::ManifestTest {
                native: false,
                profile: BuildProfile::Dev,
                ..
            })
        ));
    }

    #[test]
    fn explicit_native_profile_preserves_the_manifest_allocation_sequence() {
        for profile in ["--profile=dev", "--profile=release"] {
            for fail_at in 1..=3 {
                let route = select_with(
                    &args(&["build", "--manifest-path=p", "--package=app", profile]),
                    Allocator {
                        attempts: 0,
                        fail_at: Some(fail_at),
                    },
                );
                assert!(matches!(
                    route,
                    Some(Route::ManifestBuildError {
                        failure: Failure::Resource("manifest transport allocation failed"),
                        ..
                    })
                ));
            }
            let Some(Route::ManifestBuild { request, .. }) = select_with(
                &args(&["build", "--manifest-path=p", profile]),
                Allocator {
                    attempts: 0,
                    fail_at: Some(3),
                },
            ) else {
                panic!("profile added an allocation without a package selector");
            };
            assert_eq!(request[0].allocator.attempts, 2);
        }
    }

    #[test]
    fn profile_syntax_is_global_first_error_before_filter_fail_fast_and_local_errors() {
        for (tail, expected) in [
            (vec!["--profile"], "--profile requires a value"),
            (vec!["--profile="], "--profile requires a value"),
            (
                vec!["--profile", "--engine=native"],
                "--profile requires a value",
            ),
            (
                vec!["--profile=debug"],
                "unknown --profile `debug`; expected dev or release",
            ),
            (
                vec!["--profile=dev", "--profile", "release"],
                "--profile may only be specified once",
            ),
            (
                vec!["--profile=release", "--profile="],
                "--profile may only be specified once",
            ),
            (
                vec!["--profile=wrong", "--profile=dev"],
                "unknown --profile `wrong`; expected dev or release",
            ),
            (
                vec!["--profile=", "--edition=wrong"],
                "--profile requires a value",
            ),
            (
                vec!["--edition=wrong", "--profile="],
                "unknown edition `wrong`; expected legacy-0.9 or typed-preview",
            ),
            (
                vec!["--filter=", "--profile=wrong"],
                "unknown --profile `wrong`; expected dev or release",
            ),
            (
                vec!["--fail-fast=", "--profile=wrong"],
                "unknown --profile `wrong`; expected dev or release",
            ),
            (
                vec!["--unknown", "--profile=wrong"],
                "unknown --profile `wrong`; expected dev or release",
            ),
            (
                vec!["--profile=dev", "--filter=", "--fail-fast="],
                "--filter requires a value",
            ),
            (
                vec!["--profile=dev", "--fail-fast=", "--filter="],
                "--fail-fast does not accept a value",
            ),
        ] {
            let mut words = vec!["test", "--manifest-path=missing", "--message-format=json"];
            words.extend(tail);
            let Some(Route::ManifestTestError { failure, json }) = select(&args(&words)) else {
                panic!("expected test refusal: {words:?}");
            };
            assert!(json);
            assert_eq!(failure.message(), expected, "{words:?}");
        }
        let Some(Route::ManifestError {
            failure, operation, ..
        }) = select(&args(&["run", "--manifest-path=p", "--profile=x"]))
        else {
            panic!("global profile syntax error");
        };
        assert_eq!(operation, Operation::Check);
        assert_eq!(
            failure.message(),
            "unknown --profile `x`; expected dev or release"
        );
    }

    #[test]
    fn explicit_profile_even_dev_rejects_check_and_interpreter_engines() {
        for profile in ["--profile=dev", "--profile=release"] {
            refusal(
                &["check", "--manifest-path=p", profile],
                PROFILE_APPLICABILITY,
            );
            for command in ["run", "test"] {
                for engine in [None, Some("--engine=interpreter")] {
                    let mut words = vec![command, "--manifest-path=p", profile];
                    words.extend(engine);
                    refusal(&words, PROFILE_APPLICABILITY);
                }
            }
            refusal(
                &["run", "--manifest-path=p", "--entry-mode=process", profile],
                PROFILE_APPLICABILITY,
            );
            for local in ["--filter=keep", "--fail-fast"] {
                for command in ["check", "run", "build", "package"] {
                    let expected = if local == "--filter=keep" {
                        "--filter is only supported by explicit manifest test"
                    } else {
                        "--fail-fast is only supported by explicit manifest test"
                    };
                    refusal(&[command, "--manifest-path=p", profile, local], expected);
                    refusal(&[command, "--manifest-path=p", local, profile], expected);
                }
            }
        }
        refusal(
            &[
                "test",
                "--manifest-path=p",
                "--engine=native",
                "--profile=release",
                "--message-format=json",
            ],
            "manifest test does not support --message-format=json",
        );
        refusal(
            &[
                "run",
                "--manifest-path=p",
                "--engine=native",
                "--profile=release",
                "--message-format=json",
            ],
            "native manifest run does not support --message-format=json",
        );
    }

    #[test]
    fn profile_preserves_script_and_separator_ownership_and_other_selector_refusals() {
        use crate::frontend::options;
        for words in [
            vec![
                "script",
                "task",
                "--profile",
                "release",
                "--manifest-path=p",
            ],
            vec!["script", "task", "--profile=", "--manifest-path=p"],
            vec!["build", "--", "--profile=release", "--manifest-path=p"],
        ] {
            let arguments = args(&words);
            assert!(select(&arguments).is_none());
            assert_eq!(options::route(&arguments), Route::Legacy(arguments));
        }
        refusal(
            &["build", "--manifest-path=p", "--", "--profile=release"],
            "manifest build does not accept a source operand",
        );
        refusal(
            &["--manifest-path=p", "--profile=release", "build"],
            "--manifest-path must follow build",
        );
        refusal(
            &[
                "build",
                "--manifest-path=p",
                "--edition=legacy-0.9",
                "--profile=release",
            ],
            "manifest build conflicts with --edition legacy-0.9",
        );
        for command in ["fmt", "doctor", "new", "compile", "unknown"] {
            refusal(
                &[command, "--manifest-path=p", "--profile=release"],
                "--manifest-path requires check, run or build, or test",
            );
        }
        for experimental in [
            "--experimental-hir-import=observation",
            "--experimental-hir-producers=bundle",
            "--experimental-lexical-provider=bundle",
        ] {
            refusal(
                &[
                    "build",
                    "--manifest-path=p",
                    "--profile=release",
                    experimental,
                ],
                "manifest build does not support backend, target, output or experimental selectors",
            );
        }
    }
}
