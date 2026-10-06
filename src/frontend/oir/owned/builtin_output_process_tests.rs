//! Private actual-process proof driver. No public source or CLI route uses it.
//! The exact child test is inert without OXID_RAW_STDOUT_CASE. Its parent owns
//! a separate inherited output descriptor, so libtest text is never mistaken
//! for language stdout, nor removed from captured language output afterward.
use super::builtin_output_fixtures as fixture;
use super::consumer_fixtures as f;
use super::enum_consumer_fixtures as e;
use super::*;

// Enabled only after the coordinator's closed identity/layout/consumer review.
// This is not an alternate verifier, witness constructor or production route.
const POSITIVE_PROCESS_PROOF_ENABLED: bool = true;
const CHILD: &str = concat!(
    "frontend::oir::owned::builtin_output_process_tests::",
    "builtin_output_subprocess_child"
);
const DRIVER_DENIED: &[u8] = b"private output process proof is not enabled\n";
const DRIVER_OPTIONS: &[u8] = b"invalid private output process fixture options\n";

#[derive(Clone, Copy, Debug)]
enum Case {
    Abc,
    InvalidLast256,
    InvalidLastNegative,
    Empty,
    Projected,
    Forwarded,
    Status(i32),
}
impl Case {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "abc" => Self::Abc,
            "invalid-last-256" => Self::InvalidLast256,
            "invalid-last-negative" => Self::InvalidLastNegative,
            "empty" => Self::Empty,
            "projected" => Self::Projected,
            "forwarded" => Self::Forwarded,
            "status-0" => Self::Status(0),
            "status-37" => Self::Status(37),
            "status-negative" => Self::Status(-256),
            "status-256" => Self::Status(256),
            _ => return None,
        })
    }
}

/// These are untrusted raw rows. Only ordinary verify_owned establishes their
/// executable authority. Complete maps to 0, InvalidInput to 1, IoError(n) to
/// n + 2. No callback inspects or rewrites the operation's runtime result.
fn candidate(case: Case) -> (SourceMap, RawOwnedProgram, hir::DefId) {
    if let Case::Status(value) = case {
        let (sources, mut raw) = fixture::inventory(BuiltinOrigins::None);
        let statement = &mut raw.functions[0].blocks[0].statements[0];
        statement.kind = f::assign(0, Rvalue::I32(value), statement.span).kind;
        return (sources, raw, hir::DefId(0));
    }
    let (sources, mut raw, entry) = match case {
        Case::Projected => fixture::projected_program(),
        Case::Forwarded => fixture::forwarded_program(),
        Case::Empty => fixture::program(0),
        _ => fixture::program(3),
    };
    let main = &mut raw.functions[0];
    let span = main.span;
    let s = |index| e::at(span, 3600 + index);
    let base = main.locals.len();
    // A, B, last, Complete, InvalidInput, bias, payload, mapped IoError.
    main.locals.extend((0..8).map(|index| LocalDecl {
        ty: hir::Ty::I32,
        kind: if index == 6 {
            LocalKind::Binding
        } else {
            LocalKind::Temporary
        },
        span: s(index),
    }));
    let last = match case {
        Case::InvalidLast256 => 256,
        Case::InvalidLastNegative => -1,
        _ => 67,
    };
    main.blocks[0].statements.splice(
        0..0,
        [65, 66, last, 0, 1, 2]
            .into_iter()
            .enumerate()
            .map(|(index, value)| f::assign(base + index, Rvalue::I32(value), s(index))),
    );
    for statement in &mut main.blocks[0].statements {
        if let OwnedInstruction::ConstructArray { elements, .. } = &mut statement.kind {
            for (index, operand) in elements.iter_mut().enumerate() {
                *operand = f::operand(base + index, statement.span);
            }
        }
    }
    let aggregate = main.owners[1].aggregate;
    let AggregateTy::Enum(enumeration) = aggregate.aggregate() else {
        unreachable!("the canonical output fixture has an enum call result")
    };
    let subject = main.matches.first().map_or_else(
        || {
            let owner = OwnerPlaceId(main.owners.len());
            main.owners.push(OwnerDecl {
                kind: OwnerKind::Local { mutable: false },
                aggregate,
                span: s(8),
            });
            owner
        },
        |descriptor| descriptor.source,
    );
    main.matches = vec![MatchDecl {
        source: subject,
        arms: (0..3)
            .map(|arm| MatchArm {
                variant: VariantId {
                    enumeration,
                    index: arm,
                },
                dispatch: BlockId(1 + arm),
                entry: BlockId(4 + arm),
            })
            .collect(),
        span: s(11),
    }];
    main.blocks.truncate(1);
    if matches!(case, Case::Projected) {
        // The removed checksum loop alone initializes/uses these two scalar
        // places. The retained record constructor, shared projected loan and
        // status-consumption tail use only scalar locals and owned places.
        main.places.clear();
    }
    for arm in 0..3 {
        main.blocks.push(e::block(
            if arm == 0 {
                vec![
                    f::instruction(OwnedInstruction::StorageLive(subject), s(8)),
                    f::instruction(
                        OwnedInstruction::MoveInitialize {
                            destination: subject,
                            source: OwnerPlaceId(1),
                        },
                        s(9),
                    ),
                    f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(1)), s(10)),
                ]
            } else {
                vec![]
            },
            OwnedTerminatorKind::MatchDispatch {
                match_id: MatchId(0),
                arm,
            },
            s(11),
        ));
    }
    for arm in 0..3 {
        let mut statements = vec![
            f::instruction(
                OwnedInstruction::ConsumeVariant {
                    match_id: MatchId(0),
                    arm,
                    destination: (arm == 2).then_some(LocalId(base + 6)),
                },
                s(11),
            ),
            f::instruction(OwnedInstruction::StorageEnd(subject), s(13)),
            f::instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), s(14)),
            f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), s(15)),
        ];
        if arm == 2 {
            statements.push(f::assign(
                base + 7,
                Rvalue::CheckedI32 {
                    op: hir::ArithmeticOp::Add,
                    left: f::operand(base + 6, s(16)),
                    right: f::operand(base + 5, s(16)),
                    operator_span: s(16),
                },
                s(16),
            ));
        }
        main.blocks.push(e::block(
            statements,
            OwnedTerminatorKind::ReturnScalar(f::operand(base + [3, 4, 7][arm], s(17))),
            s(17),
        ));
    }
    (sources, raw, entry)
}

fn report_before_execution(bytes: &[u8]) -> i32 {
    if process::setup() {
        process::diagnostic(bytes)
    } else {
        74
    }
}

fn bounded_fuel() -> Option<usize> {
    match std::env::var("OXID_RAW_STDOUT_FUEL") {
        Ok(value) => value.parse().ok().filter(|fuel| *fuel <= plan::MAX_FUEL),
        Err(std::env::VarError::NotPresent) => Some(plan::MAX_FUEL),
        Err(_) => None,
    }
}

/// Rebind fd 1 only in the exact child, after libtest's opening text went to the
/// harness stream. The parent must provide a separate open fd, never 0/1/2.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn redirect_actual_stdout() -> bool {
    use std::io::Write;
    unsafe extern "C" {
        fn dup2(old: std::ffi::c_int, new: std::ffi::c_int) -> std::ffi::c_int;
        fn close(fd: std::ffi::c_int) -> std::ffi::c_int;
    }
    let Some(fd) = std::env::var("OXID_RAW_STDOUT_FD")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|fd| *fd >= 3)
    else {
        return false;
    };
    // libtest may have buffered its opening text without a newline. Drain that
    // buffer while fd 1 still names the parent-owned harness capture; otherwise
    // std::process::exit could flush harness text into the artifact at shutdown.
    if std::io::stdout().flush().is_err() {
        return false;
    }
    // SAFETY: the explicit parent contract supplies this inherited descriptor.
    // dup2 makes fd 1 a distinct reference to the same open description; close
    // releases only the private extra descriptor, not stdout or harness stderr.
    unsafe {
        let redirected = dup2(fd, 1) == 1;
        if redirected {
            close(fd);
        }
        redirected
    }
}
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn redirect_actual_stdout() -> bool {
    false
}

fn reference_status(case: Case, fuel: usize) -> i32 {
    let (sources, raw, entry) = candidate(case);
    let witness = match verified::verify_owned(raw, &sources) {
        Ok(witness) => witness,
        Err(error) => {
            // Verification is effect-free, so its failure has not performed
            // signal setup. Establish safety before rendering or reporting.
            if !process::setup() {
                return 74;
            }
            return process::diagnostic(
                source::raw_verification_diagnostic(&error, &sources)
                    .render_human(&sources)
                    .as_bytes(),
            );
        }
    };
    let status = match execute::run_process_limits(
        &witness,
        Some(entry),
        execute::Limits {
            fuel,
            ..execute::Limits::default()
        },
    ) {
        Ok(Scalar::I32(value @ 0..=255)) => value,
        Err(execute::OwnedRunFailure::ProcessSetup) => 74,
        // This host failure precedes setup, and the unsupported implementation
        // cannot safely report to an inherited possibly broken descriptor.
        Err(execute::OwnedRunFailure::ProcessHost) => 74,
        Err(error) => {
            process::diagnostic(error.diagnostic(&sources).render_human(&sources).as_bytes())
        }
        Ok(_) => process::diagnostic(b"invalid private process result\n"),
    };
    drop(witness);
    drop(sources);
    // No witness, owner, source map, diagnostic or allocation survives into
    // process::exit. The ordinary executor already destroyed runtime storage.
    status
}

/// Compilation has no process activation/setup. All text here is a build
/// artifact, independent of process stdout and the later native executable.
fn native_artifact_status(case: Case, fuel: usize, directory: &std::path::Path) -> i32 {
    if !directory.is_dir() {
        return 74;
    }
    let (sources, raw, entry) = candidate(case);
    let module = match verified::verify_owned(raw, &sources) {
        Ok(witness) => native::native_process_module_with_fuel(&witness, entry, &sources, fuel),
        Err(error) => Err(source::raw_verification_diagnostic(&error, &sources)),
    };
    let module = match module {
        Ok(module) => module,
        Err(error) => {
            return if save_new(
                directory.join("build-diagnostic.txt"),
                error.render_human(&sources).as_bytes(),
            )
            .is_ok()
            {
                1
            } else {
                74
            };
        }
    };
    drop(sources);
    if save_new(directory.join("program.ll"), module.as_bytes()).is_err() {
        return 74;
    }
    let output = directory.join("program");
    let Some(output) = output.to_str() else {
        return 74;
    };
    match crate::frontend::native::compile(&module, output) {
        Ok(()) => 0,
        Err(error) => {
            if save_new(
                directory.join("build-diagnostic.txt"),
                error.render_human(&SourceMap::new()).as_bytes(),
            )
            .is_ok()
            {
                1
            } else {
                74
            }
        }
    }
}

fn save_new(path: impl AsRef<std::path::Path>, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)
}

/// Run this exact test with --exact --nocapture --test-threads=1. Returning
/// without a selector is harmless in every ordinary test-suite invocation.
#[test]
fn builtin_output_subprocess_child() {
    let Some(selected) = std::env::var_os("OXID_RAW_STDOUT_CASE") else {
        return;
    };
    let native_directory = std::env::var_os("OXID_RAW_STDOUT_NATIVE_DIR");
    let status = if let Some(directory) = native_directory {
        // This branch never starts a language process or redirects fd 1.
        let directory = std::path::PathBuf::from(directory);
        match (selected.to_str().and_then(Case::parse), bounded_fuel()) {
            (Some(case), Some(fuel)) if POSITIVE_PROCESS_PROOF_ENABLED => {
                native_artifact_status(case, fuel, &directory)
            }
            _ => {
                let message = if POSITIVE_PROCESS_PROOF_ENABLED {
                    DRIVER_OPTIONS
                } else {
                    DRIVER_DENIED
                };
                if save_new(directory.join("build-diagnostic.txt"), message).is_ok() {
                    1
                } else {
                    74
                }
            }
        }
    } else if !redirect_actual_stdout() {
        74
    } else if !POSITIVE_PROCESS_PROOF_ENABLED {
        report_before_execution(DRIVER_DENIED)
    } else {
        match (selected.to_str().and_then(Case::parse), bounded_fuel()) {
            (Some(case), Some(fuel)) => reference_status(case, fuel),
            _ => report_before_execution(DRIVER_OPTIONS),
        }
    };
    // Do not return through libtest: its completion marker belongs to the
    // harness, and stdout now belongs exclusively to the language process.
    drop(selected);
    std::process::exit(status);
}

#[test]
fn builtin_output_process_candidates_require_the_ordinary_verifier() {
    if !POSITIVE_PROCESS_PROOF_ENABLED {
        return;
    }
    for case in [
        Case::Abc,
        Case::InvalidLast256,
        Case::InvalidLastNegative,
        Case::Empty,
        Case::Projected,
        Case::Forwarded,
        Case::Status(-256),
    ] {
        let (sources, raw, _) = candidate(case);
        verified::verify_owned(raw, &sources).unwrap_or_else(|error| panic!("{case:?}: {error:?}"));
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod subprocess {
    use super::*;
    use std::os::{fd::AsRawFd, unix::process::CommandExt};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    fn capture(directory: &std::path::Path, case: &str, readonly: bool) -> (i32, Vec<u8>, Vec<u8>) {
        std::fs::create_dir(directory).unwrap();
        let output_path = directory.join("actual.stdout");
        let output = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&output_path)
            .unwrap();
        let output = if readonly {
            drop(output);
            std::fs::File::open(&output_path).unwrap()
        } else {
            output
        };
        let fd = output.as_raw_fd();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", CHILD, "--nocapture", "--test-threads=1"])
            .env("OXID_RAW_STDOUT_CASE", case)
            .env("OXID_RAW_STDOUT_FD", fd.to_string())
            .env_remove("OXID_RAW_STDOUT_NATIVE_DIR")
            .env_remove("OXID_RAW_STDOUT_FUEL")
            .stdin(Stdio::null())
            .stdout(std::fs::File::create(directory.join("harness.stdout")).unwrap())
            .stderr(std::fs::File::create(directory.join("actual.stderr")).unwrap());
        // SAFETY: in the post-fork child this performs only fcntl, retaining the
        // parent-provided output fd across exec. It never changes parent flags.
        unsafe {
            command.pre_exec(move || {
                unsafe extern "C" {
                    fn fcntl(fd: std::ffi::c_int, command: std::ffi::c_int, ...)
                        -> std::ffi::c_int;
                }
                if fcntl(fd, 2, 0) < 0 {
                    // Linux F_SETFD, clear FD_CLOEXEC.
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        let mut child = command.spawn().unwrap();
        drop(output);
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("private process proof timed out: {}", directory.display());
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        save_new(
            directory.join("status.txt"),
            format!("{status}\n").as_bytes(),
        )
        .unwrap();
        (
            status
                .code()
                .expect("process must exit normally, never by SIGPIPE"),
            std::fs::read(output_path).unwrap(),
            std::fs::read(directory.join("actual.stderr")).unwrap(),
        )
    }

    #[test]
    #[ignore = "explicit private process proof; retains exact captures in OXID_RAW_STDOUT_EVIDENCE_DIR"]
    fn builtin_output_process_reference_bytes_and_statuses() {
        const { assert!(POSITIVE_PROCESS_PROOF_ENABLED) };
        let directory = std::path::PathBuf::from(
            std::env::var_os("OXID_RAW_STDOUT_EVIDENCE_DIR")
                .expect("fresh proof evidence directory"),
        );
        std::fs::create_dir(&directory).unwrap();
        for (case, bytes, status) in [
            ("abc", b"ABC".as_slice(), 0),
            ("invalid-last-256", b"".as_slice(), 1),
            ("invalid-last-negative", b"".as_slice(), 1),
            ("empty", b"".as_slice(), 0),
            ("projected", b"ABC".as_slice(), 0),
            ("forwarded", b"ABC".as_slice(), 0),
            ("status-0", b"".as_slice(), 0),
            ("status-37", b"".as_slice(), 37),
        ] {
            let actual = capture(&directory.join(case), case, false);
            assert_eq!(actual, (status, bytes.to_vec(), vec![]), "{case}");
        }
        let diagnostic = b"error[E0600] (oir-run): process main must return a status in 0..255\n  --> raw-owned-consumers.ox:1:1\n";
        for case in ["status-negative", "status-256"] {
            assert_eq!(
                capture(&directory.join(case), case, false),
                (1, vec![], diagnostic.to_vec()),
                "{case}",
            );
        }
        // A real EBADF output attempt becomes IoError(0), hence status 2.
        assert_eq!(
            capture(&directory.join("abc-readonly"), "abc", true),
            (2, vec![], vec![])
        );
        // Empty has no write attempt even when fd 1 is not writable.
        assert_eq!(
            capture(&directory.join("empty-readonly"), "empty", true),
            (0, vec![], vec![])
        );
    }
}
