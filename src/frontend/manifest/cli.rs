//! Borrow-only opt-in gate. Calls without an eligible selector never allocate
//! here and go unchanged to the old classifier, including Process error routing.
//! Only `check` and `run` are eligible; RUN reuses the CHECK scan unchanged.
use super::{word, Allocator, Failure, Operation, Route};
const OPTIONS: [&str; 10] = [
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
];
#[derive(Clone, Copy)]
enum Error<'a> {
    Duplicate(&'static str),
    Missing(&'static str),
    Edition(&'a str),
    Format(&'a str),
    Entry(&'a str),
    Unsupported(&'a str),
}
impl Error<'_> {
    fn message(self, operation: Operation) -> String {
        match self {
            Self::Duplicate(n) => format!("{n} may only be specified once"),
            Self::Missing(n) => format!("{n} requires a value"),
            Self::Edition(v) => {
                format!("unknown edition `{v}`; expected legacy-0.9 or typed-preview")
            }
            Self::Format(v) => format!("unknown --message-format `{v}`; expected text or json"),
            Self::Entry(v) => format!("unknown --entry-mode `{v}`; expected result or process"),
            Self::Unsupported(v) => {
                format!("unsupported option for manifest {}: {v}", word(operation))
            }
        }
    }
}
#[derive(Default)]
pub(super) struct Scan<'a> {
    path: Option<&'a str>,
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
                (3, Some("result" | "process")) => {}
                (3, Some(value)) => {
                    error.get_or_insert(Error::Entry(value));
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
    let op = word(operation);
    let message = if let Some(error) = scan.error.or(scan.local_error) {
        Some(error.message(operation))
    } else if !matches!(scan.command, Some("check" | "run")) {
        Some("--manifest-path requires check or run".into())
    } else if scan.before {
        Some(format!("--manifest-path must follow {op}"))
    } else if scan.edition == Some("legacy-0.9") {
        Some(format!("manifest {op} conflicts with --edition legacy-0.9"))
    } else if scan.operands {
        Some(format!("manifest {op} does not accept a source operand"))
    } else if scan.seen & !7 != 0 {
        Some(format!("manifest {op} does not support entry-mode, backend, target, output or experimental selectors"))
    } else {
        None
    };
    if let Some(message) = message {
        return Some(Route::ManifestError {
            failure: Failure::Cli(message),
            json: scan.json,
            operation,
        });
    }
    Some(
        match super::request(scan.path.unwrap_or(""), scan.json, allocator) {
            Ok(request) if operation == Operation::Run => Route::ManifestRun { request },
            Ok(request) => Route::ManifestCheck { request },
            Err(failure) => Route::ManifestError {
                failure,
                json: scan.json,
                operation,
            },
        },
    )
}
