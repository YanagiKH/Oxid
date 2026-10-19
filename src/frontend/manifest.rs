//! Explicit manifest selection. Independent limits/allocator; no legacy
//! manifest/dependency machinery and no change to the source loader's ledger.
use super::options::{Operation, Route};
use std::{
    fs::{File, Metadata},
    io::{self, Read},
    mem::{align_of, size_of},
};
mod cli;
pub(super) mod graph;
#[cfg(test)]
mod tests;

pub(super) const M_RAW: usize = 16_384;
const D_LIMITS: [usize; 4] = [32, 2_048, 512, 512];
const D_TOTAL: usize = 3_072;
const P_MANIFEST: usize = 4_096;
const P_JOINED: usize = 8_192;
const S_READ: usize = 1_024;
const H_CARRIER: usize = 256;
const H_FIXED: usize = 2_048;
const KEYS: [&str; 4] = ["edition", "entry", "name", "version"];
const LENGTH_ERRORS: [&str; 4] = [
    "manifest decoded edition exceeds 32 bytes",
    "manifest decoded entry exceeds 2048 bytes",
    "manifest decoded name exceeds 512 bytes",
    "manifest decoded version exceeds 512 bytes",
];

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Failure {
    Cli(String),
    Io(String),
    Utf8(String),
    Resource(&'static str),
    Invariant,
}
impl Failure {
    pub(super) fn classification(&self) -> (&'static str, &'static str) {
        match self {
            Self::Cli(_) => ("E0001", "cli"),
            Self::Io(_) => ("E0002", "source"),
            Self::Utf8(_) => ("E0003", "source"),
            Self::Resource(_) => ("E0400", "manifest-input"),
            Self::Invariant => ("E0500", "manifest-input"),
        }
    }
    pub(super) fn message(self) -> String {
        match self {
            Self::Cli(s) | Self::Io(s) | Self::Utf8(s) => s,
            Self::Resource(s) => s.into(),
            Self::Invariant => "manifest decoder invariant failed".into(),
        }
    }
}
fn overflow() -> Failure {
    Failure::Resource("manifest transport arithmetic overflow")
}
fn allocation() -> Failure {
    Failure::Resource("manifest transport allocation failed")
}
fn layout_failure() -> Failure {
    Failure::Resource("manifest transport layout exceeds admitted bounds")
}
fn capacity_failure() -> Failure {
    Failure::Resource("manifest transport capacity differs from admitted carrier")
}
fn add(a: usize, b: usize) -> Result<usize, Failure> {
    a.checked_add(b).ok_or_else(overflow)
}
/// Diagnostic wording only; Compile labels the explicit managed BUILD selection.
fn word(operation: Operation) -> &'static str {
    Command::from(operation).word()
}
/// Keep TEST local to manifest commands; source compiler operations stay closed.
#[derive(Clone, Copy)]
pub(super) enum Command {
    Check,
    Run,
    Build,
    Test,
}
impl From<Operation> for Command {
    fn from(operation: Operation) -> Self {
        match operation {
            Operation::Check => Self::Check,
            Operation::Run => Self::Run,
            Operation::Compile => Self::Build,
        }
    }
}
impl Command {
    fn word(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Run => "run",
            Self::Build => "build",
            Self::Test => "test",
        }
    }
}

/// Separate attempt sequence. Test injection is fixed state, never a trace Vec.
#[derive(Debug, Default, PartialEq, Eq)]
struct Allocator {
    attempts: usize,
    #[cfg(test)]
    fail_at: Option<usize>,
}
impl Allocator {
    fn attempt(&mut self, len: usize, width: usize) -> Result<(), Failure> {
        len.checked_mul(width).ok_or_else(overflow)?;
        self.attempts = add(self.attempts, 1)?;
        #[cfg(test)]
        {
            tests::record(tests::Event::Allocation);
            if self.fail_at == Some(self.attempts) || tests::fail_at() == Some(self.attempts) {
                return Err(allocation());
            }
        }
        Ok(())
    }
    fn string(&mut self, length: usize) -> Result<String, Failure> {
        self.attempt(length, 1)?;
        let mut result = String::new();
        result.try_reserve_exact(length).map_err(|_| allocation())?;
        Ok(result)
    }
    fn vector<T>(&mut self, length: usize) -> Result<Vec<T>, Failure> {
        self.attempt(length, size_of::<T>())?;
        let mut result = Vec::new();
        result.try_reserve_exact(length).map_err(|_| allocation())?;
        Ok(result)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Request {
    path: String,
    pub(super) json: bool,
    allocator: Allocator,
}

// Exact pre-change Route declaration, including existing boxed option carriers.
// Keep this prototype baseline immutable; no guessed byte stand-ins.
#[allow(dead_code)]
enum BaselineRoute {
    Legacy(Vec<String>),
    TypedLexical {
        request: Box<[super::options::LexicalOptions; 1]>,
    },
    TypedImport {
        request: Box<[super::options::ImportOptions; 1]>,
    },
    TypedProducer {
        request: Box<[super::options::ProducerOptions; 1]>,
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
        entry_policy: super::options::EntryPolicy,
    },
    TypedCompile {
        path: String,
        json: bool,
        output: String,
        entry_policy: super::options::EntryPolicy,
    },
    Error {
        message: String,
        json: bool,
        operation: super::options::Operation,
    },
    ProcessError {
        message: String,
    },
}
#[derive(Clone, Copy, Debug)]
struct Field {
    start: usize,
    end: usize,
    decoded: usize,
}
#[derive(Default)]
struct Parsed {
    fields: [Option<Field>; 4],
}
struct Scanner<'a> {
    text: &'a str,
    cursor: usize,
    line: usize,
}

/// Conservative named owners, including moved/result roles; not physical frames.
/// Counts and products are checked before either path or carrier allocation.
fn inventory() -> Result<usize, Failure> {
    let mut total = 0usize;
    // No retained inventory array: admission itself needs only the sum and
    // scalar arithmetic temporaries included in the counter row below.
    macro_rules! row {
        ($count:expr, $ty:ty) => {
            total = add(
                total,
                ($count as usize)
                    .checked_mul(size_of::<$ty>())
                    .ok_or_else(overflow)?,
            )?;
        };
    }
    row!(2, Route);
    row!(2, Request);
    row!(1, Vec<Request>);
    row!(2, Box<[Request; 1]>);
    row!(2, Vec<u8>);
    row!(2, [String; 4]);
    row!(3, String);
    row!(1, Parsed);
    row!(2, Field);
    row!(2, Scanner<'_>);
    row!(1, cli::Scan<'_>);
    row!(2, File);
    row!(2, io::Result<Metadata>);
    row!(2, io::Result<usize>);
    row!(2, Failure);
    row!(2, Result<String, Failure>);
    row!(24, usize);
    row!(1, [usize; 4]); // layout admission values
    row!(8, &str);
    row!(2, std::str::Chars<'_>);
    row!(5, char);
    row!(12, bool);
    Ok(total)
}
fn admit_layout() -> Result<(), Failure> {
    let values = [
        size_of::<Request>(),
        inventory()?,
        size_of::<Route>(),
        align_of::<Route>(),
    ];
    #[cfg(test)]
    let values = tests::layout_values(values);
    if values[0] > H_CARRIER
        || values[1] > H_FIXED
        || values[2] != size_of::<BaselineRoute>()
        || values[3] != align_of::<BaselineRoute>()
    {
        return Err(layout_failure());
    }
    Ok(())
}
fn request(path: &str, json: bool, mut allocator: Allocator) -> Result<Box<[Request; 1]>, Failure> {
    if path.len() > P_MANIFEST {
        return Err(Failure::Resource("manifest path exceeds 4096 bytes"));
    }
    admit_layout()?;
    let mut owned_path = allocator.string(path.len())?;
    owned_path.push_str(path);
    let count = 1;
    #[cfg(test)]
    let count = if tests::capacity_fault() { 2 } else { count };
    let mut carrier = allocator.vector::<Request>(count)?;
    if carrier.capacity() != 1 {
        return Err(capacity_failure());
    }
    carrier.push(Request {
        path: owned_path,
        json,
        allocator,
    });
    carrier
        .into_boxed_slice()
        .try_into()
        .map_err(|_| Failure::Invariant)
}
pub(super) fn select(args: &[String]) -> Option<Route> {
    cli::select(args)
}

/// Publication metadata of the selected application; graph entries remain in
/// the session's immutable resolved manifests.
pub(super) struct BuildSelection {
    pub parent: String,
    pub name: String,
}

// The route transfers its fallibly allocated carrier; retain that ownership
// through manifest I/O rather than moving the payload onto the call frame.
#[expect(
    clippy::boxed_local,
    reason = "retain the owned manifest route carrier through I/O"
)]
fn with_manifest<T>(
    mut request: Box<[Request; 1]>,
    consume: impl FnOnce(&str, &[u8], &mut Allocator) -> Result<T, Failure>,
) -> Result<T, Failure> {
    let Request {
        path, allocator, ..
    } = &mut request[0];
    let raw = read_path(path, allocator, M_RAW)?;
    consume(path, &raw, allocator)
}
fn read_path(path: &str, allocator: &mut Allocator, limit: usize) -> Result<Vec<u8>, Failure> {
    let io_error = |reason| Failure::Io(format!("cannot read manifest `{path}`: {reason}"));
    #[cfg(test)]
    tests::record(tests::Event::PathMetadata);
    if !std::fs::metadata(path).map_err(io_error)?.is_file() {
        return Err(Failure::Io(format!(
            "manifest `{path}` must be a regular file"
        )));
    }
    #[cfg(test)]
    tests::record(tests::Event::Open);
    let mut file = File::open(path).map_err(io_error)?;
    #[cfg(test)]
    tests::record(tests::Event::HandleMetadata);
    if !file.metadata().map_err(io_error)?.is_file() {
        return Err(Failure::Io(format!(
            "manifest `{path}` must be a regular file"
        )));
    }
    read_limit(&mut file, allocator, limit).map_err(|e| match e {
        ReadFailure::Io(e) => io_error(e),
        ReadFailure::Limit(e) => e,
    })
}
enum ReadFailure {
    Io(io::Error),
    Limit(Failure),
}
#[cfg(test)]
fn read(reader: &mut impl Read, allocator: &mut Allocator) -> Result<Vec<u8>, ReadFailure> {
    read_limit(reader, allocator, M_RAW)
}
fn read_limit(
    reader: &mut impl Read,
    allocator: &mut Allocator,
    limit: usize,
) -> Result<Vec<u8>, ReadFailure> {
    let mut raw = allocator.vector(limit).map_err(ReadFailure::Limit)?;
    let mut scratch = [0u8; S_READ];
    loop {
        let remaining = limit
            .checked_sub(raw.len())
            .ok_or_else(|| ReadFailure::Limit(overflow()))?;
        let count = add(remaining, 1).map_err(ReadFailure::Limit)?.min(S_READ);
        #[cfg(test)]
        tests::record(tests::Event::Read);
        match reader.read(&mut scratch[..count]) {
            Ok(0) => return Ok(raw),
            Ok(n) if n > count => return Err(ReadFailure::Limit(Failure::Invariant)),
            Ok(n) if n > remaining => {
                return Err(ReadFailure::Limit(Failure::Resource(if limit == M_RAW {
                    "manifest input exceeds 16384 bytes"
                } else {
                    "aggregate manifest input exceeds 16384 bytes"
                })))
            }
            Ok(n) => raw.extend_from_slice(&scratch[..n]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(ReadFailure::Io(e)),
        }
    }
}

impl<'a> Scanner<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            cursor: 0,
            line: 1,
        }
    }
    fn rest(&self) -> &'a str {
        &self.text[self.cursor..]
    }
    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }
    fn next(&mut self) -> Result<Option<char>, Failure> {
        let Some(c) = self.peek() else {
            return Ok(None);
        };
        self.cursor = add(self.cursor, c.len_utf8())?;
        Ok(Some(c))
    }
    fn horizontal(&mut self) -> Result<(), Failure> {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.next()?;
        }
        Ok(())
    }
    fn syntax(&self, path: &str, reason: &str) -> Failure {
        Failure::Cli(format!("manifest `{path}` line {}: {reason}", self.line))
    }
    fn end_line(&mut self, path: &str) -> Result<(), Failure> {
        self.horizontal()?;
        if self.peek() == Some('#') {
            while let Some(c) = self.peek() {
                if c == '\n' || c == '\r' {
                    break;
                }
                if c.is_control() && c != '\t' {
                    return Err(self.syntax(path, "control character in comment"));
                }
                self.next()?;
            }
        }
        match self.next()? {
            None => Ok(()),
            Some('\n') => {
                self.line = add(self.line, 1)?;
                Ok(())
            }
            Some('\r') if self.next()? == Some('\n') => {
                self.line = add(self.line, 1)?;
                Ok(())
            }
            _ => Err(self.syntax(path, "expected line end after statement")),
        }
    }
    fn identifier(&mut self) -> Result<&'a str, Failure> {
        let start = self.cursor;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            self.next()?;
        }
        Ok(&self.text[start..self.cursor])
    }
    fn scalar(&mut self, path: &str) -> Result<char, Failure> {
        match self.next()? {
            Some('"') => Ok('"'),
            Some('\\') => Ok('\\'),
            Some('n') => Ok('\n'),
            Some('r') => Ok('\r'),
            Some('t') => Ok('\t'),
            Some('u') => {
                let mut value = 0u32;
                for _ in 0..4 {
                    let digit = self
                        .next()?
                        .and_then(|c| c.to_digit(16))
                        .ok_or_else(|| self.syntax(path, "invalid Unicode escape"))?;
                    value = value
                        .checked_mul(16)
                        .and_then(|v| v.checked_add(digit))
                        .ok_or_else(overflow)?;
                }
                char::from_u32(value).ok_or_else(|| self.syntax(path, "invalid Unicode scalar"))
            }
            _ => Err(self.syntax(path, "unsupported string escape")),
        }
    }
    fn string(&mut self, path: &str) -> Result<Field, Failure> {
        if self.next()? != Some('"') {
            return Err(self.syntax(path, "expected double-quoted value"));
        }
        let start = self.cursor;
        let mut decoded = 0;
        loop {
            let end = self.cursor;
            let c = match self.next()? {
                Some('"') => {
                    return Ok(Field {
                        start,
                        end,
                        decoded,
                    })
                }
                Some('\\') => self.scalar(path)?,
                Some(c) if !c.is_control() => c,
                _ => return Err(self.syntax(path, "unterminated string or raw control character")),
            };
            decoded = add(decoded, c.len_utf8())?;
        }
    }
}
#[cfg(test)]
fn parse(path: &str, text: &str, operation: Command) -> Result<Parsed, Failure> {
    parse_with_dependencies(path, text, operation, None)
}
type DependencyVisitor<'a> = dyn FnMut(&str, Field, usize) -> Result<(), Failure> + 'a;
fn parse_with_dependencies(
    path: &str,
    text: &str,
    operation: Command,
    mut dependency: Option<&mut DependencyVisitor<'_>>,
) -> Result<Parsed, Failure> {
    let mut scan = Scanner::new(text);
    if text.starts_with('\u{feff}') {
        return Err(scan.syntax(path, "manifest BOM is not supported"));
    }
    let mut result = Parsed::default();
    let mut project = false;
    let mut dependencies = false;
    let mut in_dependencies = false;
    while !scan.rest().is_empty() {
        scan.horizontal()?;
        match scan.peek() {
            None | Some('#' | '\n' | '\r') => {
                scan.end_line(path)?;
                continue;
            }
            Some('[') => {
                let header_line = scan.line;
                scan.next()?;
                scan.horizontal()?;
                let section = scan.identifier()?;
                scan.horizontal()?;
                if section.is_empty() || scan.next()? != Some(']') {
                    return Err(scan.syntax(path, "invalid table header"));
                }
                scan.end_line(path)?;
                if section == "dependencies" && dependency.is_some() {
                    if dependencies {
                        return Err(Failure::Cli(format!(
                            "manifest `{path}` line {header_line}: duplicate manifest table: dependencies"
                        )));
                    }
                    dependencies = true;
                    in_dependencies = true;
                    continue;
                }
                if section != "project" {
                    return Err(Failure::Cli(format!(
                        "manifest `{path}` line {header_line}: unsupported manifest section for typed {}: {section}",
                        operation.word()
                    )));
                }
                if project {
                    return Err(Failure::Cli(format!(
                        "manifest `{path}` line {header_line}: duplicate manifest table: project"
                    )));
                }
                project = true;
                in_dependencies = false;
            }
            _ => {
                let key = scan.identifier()?;
                scan.horizontal()?;
                if key.is_empty() || scan.next()? != Some('=') {
                    return Err(scan.syntax(path, "expected bare key and ="));
                }
                if in_dependencies {
                    scan.horizontal()?;
                    let line = scan.line;
                    let value = scan.string(path)?;
                    dependency.as_mut().ok_or(Failure::Invariant)?(key, value, line)?;
                    scan.end_line(path)?;
                    continue;
                }
                if !project {
                    return Err(scan.syntax(path, "manifest keys require [project]"));
                }
                let index = KEYS
                    .iter()
                    .position(|candidate| *candidate == key)
                    .ok_or_else(|| {
                        scan.syntax(path, &format!("unsupported manifest key: project.{key}"))
                    })?;
                if result.fields[index].is_some() {
                    return Err(
                        scan.syntax(path, &format!("duplicate manifest key: project.{key}"))
                    );
                }
                scan.horizontal()?;
                result.fields[index] = Some(scan.string(path)?);
                scan.end_line(path)?;
            }
        }
    }
    Ok(result)
}
/// Visit a validated field without allocating or storing a decoded copy.
fn visit(
    text: &str,
    field: Field,
    mut emit: impl FnMut(char) -> Result<(), Failure>,
) -> Result<(), Failure> {
    let slice = text.get(field.start..field.end).ok_or(Failure::Invariant)?;
    let mut scan = Scanner::new(slice);
    let mut length = 0;
    while let Some(c) = scan.next()? {
        let c = if c == '\\' {
            scan.scalar("").map_err(|_| Failure::Invariant)?
        } else {
            c
        };
        length = add(length, c.len_utf8())?;
        if length > field.decoded {
            return Err(Failure::Invariant);
        }
        emit(c)?;
    }
    if length != field.decoded {
        return Err(Failure::Invariant);
    }
    Ok(())
}
fn admitted_field(parsed: &Parsed, index: usize, operation: Command) -> Result<Field, Failure> {
    let field = parsed.fields[index].ok_or_else(|| {
        let op = operation.word();
        Failure::Cli(match index {
            0 => format!("manifest {op} requires [project].edition = \"typed-preview\""),
            _ => format!("manifest {op} requires [project].entry"),
        })
    })?;
    if field.decoded > D_LIMITS[index] {
        return Err(Failure::Resource(LENGTH_ERRORS[index]));
    }
    Ok(field)
}
fn entry_policy(text: &str, field: Field) -> Result<(), Failure> {
    let invalid = || {
        Failure::Cli("manifest entry must be a nonempty relative .ox path without controls, backslashes, drive prefixes or empty/dot components".into())
    };
    let mut component = 0usize;
    let mut dots_only = true;
    let mut position = 0usize;
    let mut drive_letter = false;
    let mut tail = ['\0'; 3];
    visit(text, field, |c| {
        if c.is_control() || c == '\\' || (position == 1 && drive_letter && c == ':') {
            return Err(invalid());
        }
        if position == 0 {
            drive_letter = c.is_ascii_alphabetic();
        }
        position = add(position, 1)?;
        if c == '/' {
            if component == 0 || (dots_only && component <= 2) {
                return Err(invalid());
            }
            component = 0;
            dots_only = true;
        } else {
            component = add(component, 1)?;
            dots_only &= c == '.';
        }
        tail.rotate_left(1);
        tail[2] = c;
        Ok(())
    })?;
    if component == 0 || (dots_only && component <= 2) || tail != ['.', 'o', 'x'] {
        return Err(invalid());
    }
    Ok(())
}
fn parent(path: &str, windows: bool) -> &str {
    // Return a literal prefix, never a normalized Path/PathBuf. Drive-relative
    // C:file has parent C:, so concatenation must not insert a root separator.
    if let Some(index) = path.rfind(|c| c == '/' || (windows && c == '\\')) {
        return &path[..=index];
    }
    let bytes = path.as_bytes();
    if windows && bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        &path[..2]
    } else {
        ""
    }
}
#[cfg(test)]
fn prepare(
    path: &str,
    raw: &[u8],
    allocator: &mut Allocator,
    operation: Operation,
) -> Result<String, Failure> {
    prepare_inner(path, raw, allocator, operation.into(), None)
}
#[cfg(test)]
fn prepare_inner(
    path: &str,
    raw: &[u8],
    allocator: &mut Allocator,
    operation: Command,
    build_name: Option<&mut String>,
) -> Result<String, Failure> {
    let text = std::str::from_utf8(raw)
        .map_err(|_| Failure::Utf8(format!("manifest `{path}` is not valid UTF-8")))?;
    let parsed = parse(path, text, operation)?;
    prepare_parsed(path, text, allocator, operation, build_name, &parsed)
}
fn prepare_parsed(
    path: &str,
    text: &str,
    allocator: &mut Allocator,
    operation: Command,
    build_name: Option<&mut String>,
    parsed: &Parsed,
) -> Result<String, Failure> {
    let edition = admitted_field(parsed, 0, operation)?;
    let mut expected = "typed-preview".chars();
    let mut equal = true;
    visit(text, edition, |c| {
        equal &= expected.next() == Some(c);
        Ok(())
    })?;
    if !equal || expected.next().is_some() {
        return Err(Failure::Cli(format!(
            "manifest {} requires [project].edition = \"typed-preview\"",
            operation.word()
        )));
    }
    let entry = admitted_field(parsed, 1, operation)?;
    entry_policy(text, entry)?;
    #[cfg(test)]
    if tests::invariant_fault() {
        // Exercise the real decoder invariant path before source dispatch.
        visit(
            text,
            Field {
                decoded: 0,
                ..entry
            },
            |_| Ok(()),
        )?;
    }
    for index in 2..4 {
        if parsed.fields[index].is_some() {
            admitted_field(parsed, index, operation)?;
        }
    }
    let total = parsed
        .fields
        .iter()
        .flatten()
        .try_fold(0, |sum, f| add(sum, f.decoded))?;
    if total > D_TOTAL {
        return Err(Failure::Resource(
            "manifest decoded total exceeds 3072 bytes",
        ));
    }
    if build_name.is_some() {
        let field = parsed.fields[2].ok_or_else(|| {
            Failure::Cli(format!(
                "manifest {} requires [project].name",
                operation.word()
            ))
        })?;
        let mut position = 0;
        let mut valid = (1..=64).contains(&field.decoded);
        visit(text, field, |c| {
            valid &= c.is_ascii_alphanumeric() || (position > 0 && matches!(c, '_' | '-'));
            position += 1;
            Ok(())
        })?;
        if !valid {
            return Err(Failure::Cli(format!(
                "manifest {} name must match [A-Za-z0-9][A-Za-z0-9_-]{{0,63}}",
                operation.word()
            )));
        }
    }
    let parent = parent(path, cfg!(windows));
    // parent already ends at a supplied separator or at a drive-relative colon.
    let joined_len = add(parent.len(), entry.decoded)?;
    if joined_len > P_JOINED {
        return Err(Failure::Resource("manifest entry path exceeds 8192 bytes"));
    }
    let mut decoded = [String::new(), String::new(), String::new(), String::new()];
    for (index, slot) in decoded.iter_mut().enumerate() {
        if let Some(field) = parsed.fields[index] {
            *slot = allocator.string(field.decoded)?;
            visit(text, field, |c| {
                if add(slot.len(), c.len_utf8())? > field.decoded {
                    return Err(Failure::Invariant);
                }
                let mut scratch = [0u8; 4];
                slot.push_str(c.encode_utf8(&mut scratch));
                Ok(())
            })?;
        }
    }
    let mut joined = allocator.string(joined_len)?;
    joined.push_str(parent);
    joined.push_str(&decoded[1]);
    if joined.len() != joined_len {
        return Err(Failure::Invariant);
    }
    if let Some(name) = build_name {
        *name = std::mem::take(&mut decoded[2]);
    }
    Ok(joined)
}

#[cfg(test)]
pub(super) fn observe_source_load() {
    tests::record(tests::Event::SourceLoad);
}
