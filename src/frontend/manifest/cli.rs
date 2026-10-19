//! Borrow-only opt-in gate. Calls without an eligible selector never allocate
//! here and go unchanged to the old classifier, including Process error routing.
//! Both RUN engines and BUILD admit explicit process entry mode.
use super::{word, Allocator, Failure, Operation, Route};
use crate::frontend::options::EntryPolicy;
const OPTIONS: [&str; 12] = [
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
];
#[derive(Clone, Copy)]
enum Error<'a> {
    Duplicate(&'static str),
    Missing(&'static str),
    Edition(&'a str),
    Format(&'a str),
    Entry(&'a str),
    Engine(&'a str),
    Unsupported(&'a str),
}
impl Error<'_> {
    fn message(self, operation: &str) -> String {
        match self {
            Self::Duplicate(n) => format!("{n} may only be specified once"),
            Self::Missing(n) => format!("{n} requires a value"),
            Self::Edition(v) => {
                format!("unknown edition `{v}`; expected legacy-0.9 or typed-preview")
            }
            Self::Format(v) => format!("unknown --message-format `{v}`; expected text or json"),
            Self::Entry(v) => format!("unknown --entry-mode `{v}`; expected result or process"),
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
}
pub(super) fn select(args: &[String]) -> Option<Route> {
    select_with(args, Allocator::default())
}
pub(super) fn select_with(args: &[String], allocator: Allocator) -> Option<Route> {
    let mut scan = Scan::default();
    let mut index = 0;
    while index < args.len() {
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
            (slot < 7 || scan.command.is_some_and(|command| command != "script"))
                && (arg == *name || arg.strip_prefix(name).is_some_and(|s| s.starts_with('=')))
        });
        if let Some(slot) = option {
            let name = OPTIONS[slot];
            // Preserve the existing global scan tier before local validation.
            let error = if slot < 7 {
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
                (9, value) => scan.output = value,
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
    } else if !run && !test && scan.seen & (1 << 10) != 0 {
        Some("--engine is only supported by manifest run or test".into())
    } else if scan.seen
        & !((if build || run || package { 15 } else { 7 })
            | (if run || test { 1 << 10 } else { 0 })
            | (if package { 1 << 9 } else { 0 })
            | (1 << 11))
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
            },
            Ok(request) if test => Route::ManifestTest {
                request,
                native: scan.native,
            },
            Ok(request) if build => Route::ManifestBuild {
                request,
                entry_policy,
            },
            Ok(request) if native_run => Route::ManifestNativeRun {
                request,
                entry_policy,
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
        }) = select(&words)
        else {
            panic!("package route");
        };
        assert_eq!(output, "out.tar");
        assert_eq!(entry_policy, EntryPolicy::Process);
        assert_eq!(request[0].package.as_deref(), Some("app"));
    }
}
