//! Bounded single-file formatter I/O, deliberately separate from project loading.
use super::{
    diagnostic::Diagnostic,
    owned_diagnostic,
    project::budget::{Allocator, ReserveFailure},
    source::{SourceFileId, SourceMap, MAX_SOURCE_BYTES},
};
use std::fs::{self, File, Metadata};
use std::io::{self, Read, Write};

pub(super) fn cli_error(message: &str) -> i32 {
    let error = owned_diagnostic::diagnostic("E0001", "cli", format_args!("{message}"), None);
    report(&SourceMap::new(), &error, &mut io::stderr().lock());
    2
}

pub(super) fn process_file(path: &str, check: bool) -> i32 {
    process_with_io(
        path,
        check,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
        &mut Allocator::default(),
    )
}

fn process_with_io(
    path: &str,
    check: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    allocator: &mut Allocator,
) -> i32 {
    let (sources, id) = match load_source(path, allocator) {
        Ok(source) => source,
        Err(error) => {
            report(&SourceMap::new(), &error, stderr);
            return 2;
        }
    };
    let source = sources.get(id);
    let candidate = match super::format::format_source(source) {
        Ok(candidate) => candidate,
        Err(errors) => {
            for error in errors {
                report(&sources, &error, stderr);
            }
            return 2;
        }
    };
    if check {
        if candidate.as_bytes() == source.text().as_bytes() {
            return 0;
        }
        if let Err(error) = stderr
            .write_all(b"typed-preview fmt: formatting required\n")
            .and_then(|()| stderr.flush())
        {
            report(&sources, &io_error("write formatter stderr", error), stderr);
            return 2;
        }
        return 1;
    }
    if let Err(error) = stdout
        .write_all(candidate.as_bytes())
        .and_then(|()| stdout.flush())
    {
        report(&sources, &io_error("write formatter stdout", error), stderr);
        return 2;
    }
    0
}

fn load_source(
    path: &str,
    allocator: &mut Allocator,
) -> Result<(SourceMap, SourceFileId), Box<Diagnostic>> {
    // This is a stable-filesystem policy, not an OS sandbox against races or
    // blocking host I/O. A symlink resolving to a regular file is permitted.
    let metadata =
        fs::metadata(path).map_err(|error| io_error("inspect formatter source", error))?;
    require_regular(&metadata)?;
    let mut file = File::open(path).map_err(|error| io_error("open formatter source", error))?;
    let metadata = file
        .metadata()
        .map_err(|error| io_error("inspect opened formatter source", error))?;
    require_regular(&metadata)?;
    let text = read_bounded(&mut file, allocator)?;
    let mut display_path = String::new();
    allocator
        .string(&mut display_path, path.len(), "formatter source path")
        .map_err(reserve_error)?;
    display_path.push_str(path);
    let mut sources = SourceMap::new();
    let id = sources
        .try_add(display_path, text, allocator)
        .map_err(reserve_error)?;
    Ok((sources, id))
}

fn require_regular(metadata: &Metadata) -> Result<(), Box<Diagnostic>> {
    if !metadata.is_file() {
        return Err(owned_diagnostic::diagnostic(
            "E0002",
            "io",
            format_args!("typed-preview fmt requires a regular source file"),
            None,
        ));
    }
    Ok(())
}

/// Read no more than the ceiling plus one sentinel byte, even if the file grows
/// after metadata inspection. No read_to_end or source-sized transient copy.
fn read_bounded(
    reader: &mut impl Read,
    allocator: &mut Allocator,
) -> Result<String, Box<Diagnostic>> {
    let maximum = MAX_SOURCE_BYTES
        .checked_add(1)
        .ok_or_else(|| reserve_error(ReserveFailure::Overflow))?;
    let mut bytes = Vec::new();
    allocator
        .vector(&mut bytes, maximum, "formatter source bytes")
        .map_err(reserve_error)?;
    let mut chunk = [0_u8; 8192];
    while bytes.len() < maximum {
        let count = chunk.len().min(maximum - bytes.len());
        match reader.read(&mut chunk[..count]) {
            Ok(0) => break,
            Ok(count) => {
                let next = bytes
                    .len()
                    .checked_add(count)
                    .ok_or_else(|| reserve_error(ReserveFailure::Overflow))?;
                if next > MAX_SOURCE_BYTES {
                    return Err(owned_diagnostic::diagnostic(
                        "E0400",
                        "format",
                        format_args!("formatter source exceeds 1048576 bytes"),
                        None,
                    ));
                }
                bytes.extend_from_slice(&chunk[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(io_error("read formatter source", error)),
        }
    }
    String::from_utf8(bytes).map_err(|_| {
        owned_diagnostic::diagnostic(
            "E0003",
            "io",
            format_args!("formatter source is not valid UTF-8"),
            None,
        )
    })
}

fn reserve_error(error: ReserveFailure) -> Box<Diagnostic> {
    let message = match error {
        ReserveFailure::Overflow => "formatter reader storage arithmetic overflow",
        ReserveFailure::Allocation => "formatter reader storage allocation failed",
    };
    owned_diagnostic::diagnostic("E0400", "format", format_args!("{message}"), None)
}

fn io_error(action: &str, error: io::Error) -> Box<Diagnostic> {
    owned_diagnostic::diagnostic(
        "E0002",
        "io",
        format_args!("cannot {action}: {error}"),
        None,
    )
}

/// Reuse the existing escaping without allocating a complete rendered message.
fn report(sources: &SourceMap, error: &Diagnostic, stderr: &mut impl Write) {
    struct TextWriter<'a, W>(&'a mut W);
    impl<W: Write> std::fmt::Write for TextWriter<'_, W> {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            self.0
                .write_all(text.as_bytes())
                .map_err(|_| std::fmt::Error)
        }
    }
    let _ = error.write_human(sources, &mut TextWriter(stderr));
    let _ = stderr.flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn reader_admits_exact_limit_and_stops_at_one_over() {
        let exact = vec![b' '; MAX_SOURCE_BYTES];
        assert_eq!(
            read_bounded(&mut Cursor::new(&exact), &mut Allocator::default())
                .unwrap()
                .len(),
            MAX_SOURCE_BYTES
        );
        let oversized = vec![b' '; MAX_SOURCE_BYTES + 8192];
        let mut reader = Cursor::new(&oversized);
        let error = read_bounded(&mut reader, &mut Allocator::default()).unwrap_err();
        assert_eq!((error.code, error.stage), ("E0400", "format"));
        assert_eq!(reader.position(), (MAX_SOURCE_BYTES + 1) as u64);
    }

    #[test]
    fn reader_rejects_encoding_and_read_failures_without_partial_source() {
        let error = read_bounded(&mut Cursor::new(b"\xff"), &mut Allocator::default()).unwrap_err();
        assert_eq!(error.code, "E0003");
        struct FailsAfterPrefix(bool);
        impl Read for FailsAfterPrefix {
            fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
                if self.0 {
                    Err(io::Error::other("read failure\x1b\n"))
                } else {
                    self.0 = true;
                    output[0] = b'x';
                    Ok(1)
                }
            }
        }
        let error = read_bounded(&mut FailsAfterPrefix(false), &mut Allocator::default()).unwrap_err();
        assert_eq!(error.code, "E0002");
        let mut stderr = Vec::new();
        report(&SourceMap::new(), &error, &mut stderr);
        let text = String::from_utf8(stderr).unwrap();
        assert!(text.contains("read failure\\u{1b}\\n"), "{text}");
        assert_eq!(text.matches('\n').count(), 1);
    }

    #[test]
    fn reader_retries_interrupted_reads_and_preserves_crlf() {
        struct InterruptOnce(bool);
        impl Read for InterruptOnce {
            fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
                if !self.0 {
                    self.0 = true;
                    return Err(io::ErrorKind::Interrupted.into());
                }
                output[..2].copy_from_slice(b"\r\n");
                Ok(2)
            }
        }
        let mut reader = InterruptOnce(false).take(2);
        assert_eq!(read_bounded(&mut reader, &mut Allocator::default()).unwrap(), "\r\n");
    }

    struct Source(std::path::PathBuf);
    impl Source {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "oxid-format-reader-{}-{}.ox", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::write(&path, "// reader\n").unwrap();
            Self(path)
        }
        fn path(&self) -> &str { self.0.to_str().unwrap() }
    }
    impl Drop for Source {
        fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
    }

    #[test]
    fn every_reader_source_owner_reservation_fails_closed() {
        let source = Source::new();
        let mut successful = Allocator::default();
        let (sources, id) = load_source(source.path(), &mut successful).unwrap();
        assert_eq!(sources.get(id).text(), "// reader\n");
        assert_eq!(successful.attempts, 4);
        for fail_at in 1..=successful.attempts {
            let mut allocator = Allocator { fail_at: Some(fail_at), ..Allocator::default() };
            let error = load_source(source.path(), &mut allocator).unwrap_err();
            assert_eq!((error.code, error.stage), ("E0400", "format"));
            assert_eq!(allocator.attempts, fail_at);
        }
    }

    struct FailingWriter { fail_flush: bool, bytes: Vec<u8> }
    impl Write for FailingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail_flush {
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            } else if self.bytes.is_empty() && !bytes.is_empty() {
                self.bytes.push(bytes[0]);
                Ok(1)
            } else {
                Err(io::ErrorKind::BrokenPipe.into())
            }
        }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }

    #[test]
    fn stdout_partial_write_and_flush_failures_are_code_two_without_panics() {
        let source = Source::new();
        for fail_flush in [false, true] {
            let mut stdout = FailingWriter { fail_flush, bytes: Vec::new() };
            let mut stderr = Vec::new();
            assert_eq!(process_with_io(source.path(), false, &mut stdout, &mut stderr, &mut Allocator::default()), 2);
            assert!(!stdout.bytes.is_empty());
            assert!(String::from_utf8(stderr).unwrap().contains("cannot write formatter stdout"));
        }
    }

    #[test]
    fn check_never_touches_stdout_and_stderr_failure_is_not_drift() {
        let source = Source::new();
        let mut stdout = FailingWriter { fail_flush: false, bytes: Vec::new() };
        let mut stderr = Vec::new();
        assert_eq!(process_with_io(source.path(), true, &mut stdout, &mut stderr, &mut Allocator::default()), 0);
        assert!(stdout.bytes.is_empty());
        assert!(stderr.is_empty());
        fs::write(&source.0, "// reader").unwrap();
        let mut stderr = FailingWriter { fail_flush: false, bytes: Vec::new() };
        assert_eq!(process_with_io(source.path(), true, &mut stdout, &mut stderr, &mut Allocator::default()), 2);
        assert!(stdout.bytes.is_empty());
    }
}
