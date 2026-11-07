//! Explicit scalar library options. No ordinary native setting or source owner grows.
use super::{
    diagnostic::Diagnostic,
    options::{Operation, Route},
    project::budget::Allocator,
};
use std::fmt::Write;
use std::mem::size_of;

pub(super) const MAX_FUEL: usize = super::oir::native_library_max_fuel();

#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub path: String,
    pub output: String,
    pub export: String,
    pub release: bool,
    pub fuel: usize,
    pub json: bool,
}

#[allow(dead_code)]
struct DriverBank {
    options_owner: Box<[Options; 1]>,
    options_transports: [&'static Options; 3],
    checked_owners: [super::oir::CheckedScalarLibrary<'static>; 3],
    meter: super::declaration_index::WorkMeter,
    allocator: Allocator,
    outputs: [String; 3],
    results: [Result<String, Box<Diagnostic>>; 3],
    inputs: [(&'static Options, &'static super::project::ProjectSources); 2],
    limits: [super::declaration_index::IndexLimits; 3],
    raw_option_inputs: [&'static [String]; 2],
    raw_option_iterators: [std::slice::Iter<'static, String>; 2],
    raw_option_words: [Option<&'static String>; 4],
}
#[allow(dead_code)]
struct HeaderBank {
    outputs: [String; 2],
    returns: [Result<String, Box<Diagnostic>>; 2],
    input: [(usize, &'static mut Allocator); 2],
    formatting: [std::fmt::Arguments<'static>; 3],
    // One decimal argument: two-pointer formatter return/backing and the four
    // borrowed call/constructor transports of the existing scalar model.
    formatter_arguments: [[*const (); 2]; 2],
    borrows: [*const (); 4],
    numbers: [usize; 5],
    writes: [std::fmt::Result; 2],
}

pub(super) fn limits(
    options: &Options,
) -> Result<super::declaration_index::IndexLimits, Box<Diagnostic>> {
    let mut limits = super::declaration_index::IndexLimits::default();
    // Sum disjoint phases conservatively: IR remains live through header and
    // publication. No legacy envelope or unexplained overlay pays these roles.
    let paid = size_of::<DriverBank>()
        .checked_add(size_of::<Options>())
        .and_then(|n| n.checked_add(options.path.capacity()))
        .and_then(|n| n.checked_add(options.output.capacity()))
        .and_then(|n| n.checked_add(options.output.capacity())) // returned compile summary
        .and_then(|n| n.checked_add(options.export.capacity()))
        .and_then(|n| n.checked_add(size_of::<HeaderBank>()))
        .and_then(|n| n.checked_add(header_bytes(super::oir::native_library_max_params())?))
        .and_then(|n| n.checked_add(super::native::library::carrier_bytes()))
        .ok_or_else(|| {
            Diagnostic::new(
                "E0700",
                "native-library",
                "library transport budget overflow",
                None,
            )
        })? as u64;
    limits.retained = limits.retained.checked_sub(paid).ok_or_else(|| {
        Diagnostic::new(
            "E0700",
            "native-library",
            "library transport budget exceeded",
            None,
        )
    })?;
    limits.scratch = limits.scratch.checked_sub(paid).ok_or_else(|| {
        Diagnostic::new(
            "E0700",
            "native-library",
            "library transport budget exceeded",
            None,
        )
    })?;
    Ok(limits)
}

pub(super) fn require_host() -> Result<(), Box<Diagnostic>> {
    if cfg!(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
        Ok(())
    } else {
        Err(Diagnostic::new(
            "E0700",
            "native-library",
            "scalar native libraries currently require Linux x86_64 or ARM64",
            None,
        ))
    }
}

fn invalid(message: impl Into<String>, json: bool) -> Route {
    Route::Error {
        message: message.into(),
        json,
        operation: Operation::Compile,
    }
}

pub(super) fn export_name(value: &str) -> Option<&str> {
    let name = value.strip_prefix("crate::")?;
    let mut bytes = name.bytes();
    if name.len() > 64
        || !matches!(bytes.next(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'_'))
        || !bytes.all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return None;
    }
    Some(name)
}

pub(super) fn route(arguments: Vec<String>, json: bool, raw_arguments: &[String]) -> Route {
    // Validate original adjacency before using globally normalized arguments.
    // A global option cannot fill or erase another option's missing value.
    // This preflight is reached only for an explicit typed library command.
    let mut raw = raw_arguments.iter();
    while let Some(key) = raw.next() {
        if key == "--" {
            break;
        }
        if matches!(
            key.as_str(),
            "--output" | "--export" | "--backend" | "--profile" | "--fuel"
        ) && raw
            .next()
            .is_none_or(|value| value.is_empty() || value.starts_with('-'))
        {
            return invalid(format!("{key} requires an adjacent nonempty value"), json);
        }
    }
    let mut path = None;
    let mut output = None;
    let mut export = None;
    let mut backend = None;
    let mut profile = None;
    let mut fuel = None;
    let mut separated = false;
    let mut words = arguments.into_iter().skip(1);
    while let Some(word) = words.next() {
        if word == "--" && !separated {
            separated = true;
            continue;
        }
        if !separated && word.starts_with('-') {
            let key = word.split('=').next().unwrap_or("");
            let slot = match key {
                "--output" => &mut output,
                "--export" => &mut export,
                "--backend" => &mut backend,
                "--profile" => &mut profile,
                "--fuel" => &mut fuel,
                _ => {
                    return invalid(
                        format!("unsupported option for typed-preview library: {word}"),
                        json,
                    )
                }
            };
            let value = word
                .split_once('=')
                .map(|(_, v)| v.to_string())
                .or_else(|| words.next());
            if slot.is_some()
                || value
                    .as_ref()
                    .is_none_or(|v| v.is_empty() || v.starts_with('-'))
            {
                return invalid(
                    format!("{key} requires one nonempty value and may occur only once"),
                    json,
                );
            }
            *slot = value;
        } else if path.replace(word).is_some() {
            return invalid(
                "typed-preview library requires exactly one source path",
                json,
            );
        }
    }
    let Some(path) = path else {
        return invalid(
            "typed-preview library requires exactly one source path",
            json,
        );
    };
    let Some(output) = output else {
        return invalid(
            "typed-preview library requires --output <new-version-directory>",
            json,
        );
    };
    let Some(export) = export else {
        return invalid(
            "typed-preview library requires --export crate::<identifier>",
            json,
        );
    };
    let path_limit = super::project::ProjectLimits::default().relative_bytes;
    if path.len() > path_limit || output.len() > path_limit {
        return invalid(
            "library source and output paths exceed the existing project path limit",
            json,
        );
    }
    if export_name(&export).is_none() {
        return invalid("library export requires exactly crate::<ASCII identifier>, at most 64 identifier bytes", json);
    }
    if backend.as_deref().is_some_and(|v| v != "llvm") {
        return invalid("library supports only --backend llvm", json);
    }
    let release = match profile.as_deref() {
        None | Some("dev" | "bounded") => false,
        Some("release") => true,
        Some(_) => {
            return invalid(
                "library profile must be dev, bounded (dev alias), or release",
                json,
            )
        }
    };
    let fuel = match fuel {
        None => MAX_FUEL,
        Some(value) => match value.parse::<usize>() {
            Ok(n) if n <= MAX_FUEL && value.bytes().all(|c| c.is_ascii_digit()) => n,
            _ => {
                return invalid(
                    format!("library fuel must be an integer from 0 through {MAX_FUEL}"),
                    json,
                )
            }
        },
    };
    let value = Options {
        path,
        output,
        export,
        release,
        fuel,
        json,
    };
    let mut allocator = Allocator::default();
    match allocator.boxed_carrier(value, "scalar library options") {
        Ok(request) => Route::TypedLibrary { request },
        Err(_) => invalid("cannot allocate scalar library options", json),
    }
}

fn header_bytes(parameters: usize) -> Option<usize> {
    let mut capacity = HEADER_PREFIX.len() + HEADER_SUFFIX.len();
    for i in 0..parameters {
        let digits = if i == 0 { 1 } else { i.ilog10() as usize + 1 };
        capacity = capacity.checked_add("int32_t arg, ".len() + digits)?;
    }
    Some(capacity)
}

pub(super) fn header(
    parameters: usize,
    allocator: &mut Allocator,
) -> Result<String, Box<Diagnostic>> {
    let maximum =
        header_bytes(super::oir::native_library_max_params()).expect("bounded header size");
    let capacity = header_bytes(parameters)
        .filter(|&n| n <= maximum)
        .ok_or_else(|| {
            Diagnostic::new(
                "E0700",
                "native-library",
                "library header size overflow",
                None,
            )
        })?;
    let mut output = String::new();
    allocator
        .string(&mut output, capacity, "scalar library header")
        .map_err(|_| {
            Diagnostic::new(
                "E0700",
                "native-library",
                "cannot allocate library header",
                None,
            )
        })?;
    if output.capacity() > maximum {
        return Err(Diagnostic::new(
            "E0700",
            "native-library",
            "library header capacity exceeds paid payload",
            None,
        ));
    }
    output.push_str(HEADER_PREFIX);
    for i in 0..parameters {
        write!(output, "int32_t arg{i}, ").expect("admitted header format");
    }
    output.push_str(HEADER_SUFFIX);
    if output.len() != capacity {
        return Err(Diagnostic::new(
            "E0500",
            "native-library",
            "internal compiler error: library header count mismatch",
            None,
        ));
    }
    Ok(output)
}

const HEADER_PREFIX: &str = "#ifndef OXID_SCALAR_LIBRARY_V1_H\n#define OXID_SCALAR_LIBRARY_V1_H\n#include <stdint.h>\n#ifdef __cplusplus\nextern \"C\" {\n#endif\nenum oxid_library_status { OXID_LIBRARY_OK=0, OXID_LIBRARY_OVERFLOW=1, OXID_LIBRARY_DIVISION_BY_ZERO=2, OXID_LIBRARY_FUEL=3, OXID_LIBRARY_INVALID_OUTPUT=4 };\nuint32_t oxid_library_abi_version(void);\n/* out must point to caller-owned, writable, naturally aligned int32_t storage.\n * Every nonzero status preserves *out. No source pointers or resources escape. */\nint32_t oxid_library_call_v1(";
const HEADER_SUFFIX: &str = "int32_t *out);\n#ifdef __cplusplus\n}\n#endif\n#endif\n";

#[cfg(test)]
mod tests {
    use super::*;
    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|s| s.to_string()).collect()
    }
    #[test]
    fn explicit_options_do_not_expand_the_default_route() {
        let Route::TypedLibrary { request } = super::super::options::route(&args(&[
            "--edition",
            "typed-preview",
            "library",
            "--export",
            "crate::price",
            "--output",
            "new",
            "entry.ox",
        ])) else {
            panic!("library route");
        };
        assert_eq!(request[0].fuel, MAX_FUEL);
        assert!(!request[0].release);
        #[cfg(target_pointer_width = "64")]
        assert_eq!(std::mem::size_of::<Route>(), 56);
        for words in [
            args(&[
                "--edition",
                "typed-preview",
                "library",
                "--export",
                "crate::a::b",
                "--output",
                "new",
                "entry.ox",
            ]),
            args(&[
                "--edition",
                "typed-preview",
                "library",
                "--export",
                "crate::a",
                "--export",
                "crate::b",
                "--output",
                "new",
                "entry.ox",
            ]),
            args(&[
                "--edition",
                "typed-preview",
                "library",
                "--export",
                "crate::a",
                "--output",
                "new",
                "--fuel",
                "1000001",
                "entry.ox",
            ]),
        ] {
            assert!(matches!(
                super::super::options::route(&words),
                Route::Error { .. }
            ));
        }
        assert!(matches!(
            super::super::options::route(&args(&["library", "entry.ox"])),
            Route::Legacy(_)
        ));
        let Route::Legacy(forwarded) = super::super::options::route(&args(&[
            "library",
            "--output",
            "--edition=legacy-0.9",
            "old",
            "entry.ox",
        ])) else {
            panic!("legacy route");
        };
        assert_eq!(forwarded, args(&["library", "--output", "old", "entry.ox"]));
    }

    #[test]
    fn max_arity_header_has_independent_decimal_extent_and_transport_payment() {
        let parameters = super::super::oir::native_library_max_params();
        assert_eq!(parameters, 64);
        // 0..9: ten one-digit indices; 10..63: fifty-four two-digit indices.
        let bytes =
            HEADER_PREFIX.len() + HEADER_SUFFIX.len() + 64 * "int32_t arg, ".len() + 10 + 54 * 2;
        let mut allocator = Allocator::default();
        let output = header(64, &mut allocator).unwrap();
        assert_eq!(output.len(), bytes);
        assert_eq!(allocator.attempts, 1);
        assert!(output.contains("int32_t arg0, int32_t arg1, "));
        assert!(output.ends_with(HEADER_SUFFIX));
        let options = Options {
            path: "in.ox".into(),
            output: "version-v1".into(),
            export: "crate::answer".into(),
            release: false,
            fuel: MAX_FUEL,
            json: false,
        };
        let paid = limits(&options).unwrap();
        let original = super::super::declaration_index::IndexLimits::default();
        assert_eq!(
            original.retained - paid.retained,
            original.scratch - paid.scratch
        );
        assert!(
            original.retained - paid.retained
                >= (bytes
                    + size_of::<DriverBank>()
                    + size_of::<HeaderBank>()
                    + super::super::native::library::carrier_bytes()) as u64
        );
    }
}
