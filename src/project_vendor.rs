//! Detached, locked source checkouts. This command never substitutes sources
//! during an ordinary project operation or modifies the original project.
use std::{
    io::{self, Write},
    path::Path,
};

const HELP: &str = "Offline typed projects (experimental):\n  oxid vendor --frozen --output NEW_DIRECTORY [--workspace]\n\nCreate a self-contained project or complete workspace from its matching lock\nand already cached dependencies. No dependencies are fetched and no user code\nis executed. Git and registry dependencies become local dependencies in the\nexported copy; original manifests, locks and sources stay unchanged. Run project\ncommands from the exported root printed on success.\n\n--locked and --offline are also accepted; every export requires an existing\nmatching lock and never fetches. --edition=typed-preview and\n--message-format=text|json are accepted.\n";
const MAX_ARGUMENT_BYTES: usize = 4096;
const MAX_ARGUMENTS: usize = 16;

struct Options<'a> {
    output: &'a Path,
    workspace: bool,
    json: bool,
}

type OptionsResult<'a> = Result<Options<'a>, String>;
type SummaryResult = Result<crate::project_package::VendorSummary, String>;
type JsonSerializer<'a> = serde_json::Serializer<&'a mut io::StdoutLock<'static>>;
type JsonFields<'a> = <&'a mut JsonSerializer<'a> as serde::Serializer>::SerializeStruct;
type JsonFieldsResult<'a> = Result<JsonFields<'a>, serde_json::Error>;

// Fixed command/report carriers are paid by the bridge before it allocates or
// creates output. Argument strings stay borrowed from the invocation owner;
// serde writes directly to the output stream instead of owning a JSON buffer.
#[repr(C)]
struct CommandRoles {
    arguments: &'static [String],
    options_construction: OptionsResult<'static>,
    options_return: OptionsResult<'static>,
    options_caller: OptionsResult<'static>,
    options: Options<'static>,
    cwd_return: io::Result<std::path::PathBuf>,
    cwd: std::path::PathBuf,
    summary_input: SummaryResult,
    summary_caller: SummaryResult,
    summary_borrow: Option<&'static crate::project_package::VendorSummary>,
    report: Report<'static>,
    serializer: JsonSerializer<'static>,
    fields_construction: JsonFieldsResult<'static>,
    fields_return: JsonFieldsResult<'static>,
    fields_caller: JsonFieldsResult<'static>,
    fields: JsonFields<'static>,
    serialized: Result<(), serde_json::Error>,
    stdout: io::StdoutLock<'static>,
    stderr: io::StderrLock<'static>,
    written: io::Result<()>,
    format_arguments: std::fmt::Arguments<'static>,
    output: Option<&'static Path>,
    word: &'static String,
    value: &'static str,
    command: usize,
    index: usize,
    seen: u8,
    bit: u8,
    workspace: bool,
    json: bool,
}

pub(crate) fn control_bytes() -> usize {
    std::mem::size_of::<CommandRoles>()
}

/// Match only the command position, never a filename, script argument or value
/// belonging to another command. Borrowing options allocates no path copy.
pub(crate) fn dispatch(args: &[String]) -> Option<i32> {
    let mut command = 0;
    loop {
        let argument = args.get(command)?;
        if matches!(argument.as_str(), "--edition" | "--message-format") {
            command += 2;
        } else if argument.starts_with("--edition=") || argument.starts_with("--message-format=") {
            command += 1;
        } else {
            break;
        }
    }
    if args.get(command)?.as_str() != "vendor" {
        return None;
    }
    if command == 0 && args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        return Some(if io::stdout().lock().write_all(HELP.as_bytes()).is_ok() {
            0
        } else {
            74
        });
    }
    let json = args.iter().any(|value| value == "--message-format=json")
        || args
            .windows(2)
            .any(|pair| pair == ["--message-format", "json"]);
    let options = match parse(args, command) {
        Ok(options) => options,
        Err(error) => return Some(report(json, Err(error))),
    };
    let start = match std::env::current_dir() {
        Ok(start) => start,
        Err(error) => {
            return Some(report(
                options.json,
                Err(format!("cannot locate the current project: {error}")),
            ))
        }
    };
    let result =
        crate::project_package::vendor_directory(&start, options.output, options.workspace);
    Some(report(options.json, result))
}

fn parse(args: &[String], command: usize) -> OptionsResult<'_> {
    if args.len() > MAX_ARGUMENTS || args.iter().any(|word| word.len() > MAX_ARGUMENT_BYTES) {
        return Err("vendor accepts at most 16 arguments of at most 4096 bytes each".into());
    }
    let mut output = None;
    let mut workspace = false;
    let mut json = false;
    let mut seen = 0u8;
    let mut index = 0;
    while index < args.len() {
        if index == command {
            index += 1;
            continue;
        }
        let (flag, inline) = args[index]
            .split_once('=')
            .map_or((args[index].as_str(), None), |(flag, value)| {
                (flag, Some(value))
            });
        let bit = match flag {
            "--output" => 1,
            "--workspace" => 2,
            "--frozen" => 4,
            "--locked" => 8,
            "--offline" => 16,
            "--edition" => 32,
            "--message-format" => 64,
            _ => return Err(format!("unsupported vendor option: {flag}")),
        };
        if seen & bit != 0 {
            return Err(format!("{flag} may only be specified once"));
        }
        seen |= bit;
        if matches!(flag, "--workspace" | "--frozen" | "--locked" | "--offline") {
            if inline.is_some() {
                return Err(format!("{flag} is a flag and takes no value"));
            }
            workspace |= flag == "--workspace";
            index += 1;
            continue;
        }
        let value = if let Some(value) = inline {
            value
        } else {
            index += 1;
            args.get(index)
                .filter(|value| !value.starts_with('-'))
                .map(String::as_str)
                .ok_or_else(|| format!("{flag} requires a value"))?
        };
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(format!(
                "{flag} requires a nonempty value without control characters"
            ));
        }
        match flag {
            "--output" => output = Some(Path::new(value)),
            "--edition" if value == "typed-preview" => {}
            "--message-format" if matches!(value, "text" | "json") => json = value == "json",
            _ => return Err(format!("unsupported value for {flag}")),
        }
        index += 1;
    }
    Ok(Options {
        output: output.ok_or("vendor requires --output NEW_DIRECTORY")?,
        workspace,
        json,
    })
}

struct Report<'a> {
    schema: &'static str,
    schema_version: u8,
    edition: &'static str,
    kind: &'static str,
    success: bool,
    dependency_mode: &'static str,
    output: Option<&'a str>,
    root: Option<&'a str>,
    source_package_sha256: Option<&'a str>,
    source_package_bytes: Option<u64>,
    files: Option<usize>,
    error: Option<&'a str>,
}

impl serde::Serialize for Report<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut fields = serializer.serialize_struct("VendorReport", 12)?;
        fields.serialize_field("schema", self.schema)?;
        fields.serialize_field("schema_version", &self.schema_version)?;
        fields.serialize_field("edition", self.edition)?;
        fields.serialize_field("kind", self.kind)?;
        fields.serialize_field("success", &self.success)?;
        fields.serialize_field("dependency_mode", self.dependency_mode)?;
        fields.serialize_field("output", &self.output)?;
        fields.serialize_field("root", &self.root)?;
        fields.serialize_field("source_package_sha256", &self.source_package_sha256)?;
        fields.serialize_field("source_package_bytes", &self.source_package_bytes)?;
        fields.serialize_field("files", &self.files)?;
        fields.serialize_field("error", &self.error)?;
        fields.end()
    }
}

fn report(json: bool, result: SummaryResult) -> i32 {
    let success = result.is_ok();
    let written = if json {
        let summary = result.as_ref().ok();
        let report = Report {
            schema: "oxid-typed-vendor-summary-v1",
            schema_version: 1,
            edition: "typed-preview",
            kind: "vendor-summary",
            success,
            dependency_mode: "self-contained-local",
            output: summary.and_then(|summary| summary.output.to_str()),
            root: summary.and_then(|summary| summary.root.to_str()),
            source_package_sha256: summary.map(|summary| summary.sha256.as_str()),
            source_package_bytes: summary.map(|summary| summary.bytes),
            files: summary.map(|summary| summary.files),
            error: result.as_ref().err().map(String::as_str),
        };
        let mut output = io::stdout().lock();
        let serialized = {
            let mut serializer: JsonSerializer<'_> = serde_json::Serializer::new(&mut output);
            serde::Serialize::serialize(&report, &mut serializer)
        };
        match serialized {
            Ok(()) => output.write_all(b"\n"),
            Err(error) => Err(io::Error::other(error)),
        }
    } else {
        match result {
            Ok(summary) => writeln!(
                io::stdout().lock(),
                "Offline project: {}\nDependencies use a self-contained local lock.\nRun check, test or build --frozen from this directory.\nSource package SHA-256: {}",
                summary.root.display(), summary.sha256
            ),
            Err(error) => writeln!(io::stderr().lock(), "error: {error}"),
        }
    };
    if written.is_err() {
        74
    } else if success {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }

    #[test]
    fn command_roles_include_actual_borrowed_options_and_json_serializer_transports() {
        use std::mem::{align_of, size_of};
        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(size_of::<Options<'static>>(), 24);
            assert_eq!(size_of::<OptionsResult<'static>>(), 32);
            assert_eq!(size_of::<Option<&Path>>(), 16);
        }
        let mut offset = 0usize;
        let mut alignment = 1usize;
        macro_rules! role {
            ($field:ident, $ty:ty) => {{
                let align = align_of::<$ty>();
                alignment = alignment.max(align);
                offset = offset.div_ceil(align) * align;
                assert_eq!(std::mem::offset_of!(CommandRoles, $field), offset);
                offset += size_of::<$ty>();
            }};
        }
        role!(arguments, &'static [String]);
        role!(options_construction, OptionsResult<'static>);
        role!(options_return, OptionsResult<'static>);
        role!(options_caller, OptionsResult<'static>);
        role!(options, Options<'static>);
        role!(cwd_return, io::Result<std::path::PathBuf>);
        role!(cwd, std::path::PathBuf);
        role!(summary_input, SummaryResult);
        role!(summary_caller, SummaryResult);
        role!(
            summary_borrow,
            Option<&'static crate::project_package::VendorSummary>
        );
        role!(report, Report<'static>);
        role!(serializer, JsonSerializer<'static>);
        role!(fields_construction, JsonFieldsResult<'static>);
        role!(fields_return, JsonFieldsResult<'static>);
        role!(fields_caller, JsonFieldsResult<'static>);
        role!(fields, JsonFields<'static>);
        role!(serialized, Result<(), serde_json::Error>);
        role!(stdout, io::StdoutLock<'static>);
        role!(stderr, io::StderrLock<'static>);
        role!(written, io::Result<()>);
        role!(format_arguments, std::fmt::Arguments<'static>);
        role!(output, Option<&'static Path>);
        role!(word, &'static String);
        role!(value, &'static str);
        role!(command, usize);
        role!(index, usize);
        role!(seen, u8);
        role!(bit, u8);
        role!(workspace, bool);
        role!(json, bool);
        assert_eq!(control_bytes(), offset.div_ceil(alignment) * alignment);
        assert_eq!(align_of::<CommandRoles>(), alignment);
    }

    #[test]
    fn options_borrow_output_and_all_exports_keep_the_same_strict_policy() {
        for flag in ["--frozen", "--locked", "--offline"] {
            let input = words(&["vendor", flag, "--output", "offline 雪", "--workspace"]);
            let options = parse(&input, 0).unwrap();
            assert_eq!(options.output, Path::new("offline 雪"));
            assert!(options.workspace);
            assert!(std::ptr::eq(
                options.output.as_os_str().as_encoded_bytes().as_ptr(),
                input[3].as_ptr()
            ));
        }
    }

    #[test]
    fn unknown_ambiguous_and_effectful_options_are_refused() {
        for input in [
            vec!["vendor"],
            vec!["vendor", "--output"],
            vec!["vendor", "--output="],
            vec!["vendor", "--output=a", "--output=b"],
            vec!["vendor", "--output=a", "--frozen", "--frozen"],
            vec!["vendor", "--output=a", "--offline=true"],
            vec!["vendor", "--output=a", "--update"],
            vec!["vendor", "--output=a", "--package=app"],
            vec!["vendor", "--output=a", "--profile=release"],
            vec!["vendor", "--output=a", "--entry-mode=result"],
            vec!["vendor", "--output=a", "--edition=legacy-0.9"],
            vec!["vendor", "--output=a", "--", "other"],
        ] {
            assert!(parse(&words(&input), 0).is_err(), "{input:?}");
        }
        let oversized = vec!["vendor".into(), format!("--output={}", "a".repeat(4097))];
        assert!(parse(&oversized, 0).is_err());
    }

    #[test]
    fn other_commands_and_their_vendor_operands_do_not_enter_this_dispatcher() {
        for input in [
            vec!["check", "vendor"],
            vec!["run", "vendor", "--output=sentinel"],
            vec!["script", "vendor"],
            vec!["--", "vendor"],
            vec!["--edition=typed-preview", "run", "vendor"],
        ] {
            assert_eq!(dispatch(&words(&input)), None);
        }
    }
}
