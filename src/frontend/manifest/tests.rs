use super::*;
#[test]
fn manifest_layout_inventory() {
    println!(
        "MANIFEST_LAYOUT request={} route={} baseline={} alignment={} fixed={} ceiling={}",
        size_of::<Request>(),
        size_of::<Route>(),
        size_of::<BaselineRoute>(),
        align_of::<Route>(),
        inventory().unwrap(),
        H_FIXED
    );
    admit_layout().unwrap();
    assert_eq!(
        P_MANIFEST + M_RAW + D_TOTAL + P_JOINED + S_READ + 4 + H_FIXED,
        34_820
    );
}

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|s| (*s).into()).collect()
}
fn error(words: &[&str]) -> (Failure, bool) {
    match select(&args(words)).unwrap() {
        Route::ManifestError { failure, json, .. } => (failure, json),
        route => panic!("expected refusal, got {route:?}"),
    }
}
fn prepared(text: &str) -> Result<String, Failure> {
    prepare(
        "oxid.toml",
        text.as_bytes(),
        &mut Allocator::default(),
        Operation::Check,
    )
}
const VALID: &str = "[project]\nedition=\"typed-preview\"\nentry=\"src/main.ox\"";

#[test]
fn manifest_cli_competition_and_rendering() {
    for (words, expected, json) in [
        (
            vec!["--manifest-path", "p.toml", "check"],
            "--manifest-path must follow check",
            false,
        ),
        (
            vec!["build", "--manifest-path="],
            "--manifest-path requires a value",
            false,
        ),
        (
            vec!["build", "--manifest-path", "p.toml"],
            "--manifest-path requires check or run",
            false,
        ),
        (
            vec![
                "check",
                "--manifest-path",
                "absent.toml",
                "--edition",
                "typed-preview",
                "--edition",
                "typed-preview",
            ],
            "--edition may only be specified once",
            false,
        ),
        (
            vec!["check", "--manifest-path=", "--message-format=json"],
            "--manifest-path requires a value",
            true,
        ),
        (
            vec!["check", "--manifest-path=p", "--message-format=bad"],
            "unknown --message-format `bad`; expected text or json",
            false,
        ),
        (
            vec![
                "check",
                "--manifest-path=p",
                "--message-format=json",
                "--message-format=text",
            ],
            "--message-format may only be specified once",
            true,
        ),
        (
            vec![
                "check",
                "--manifest-path=p",
                "--message-format=bad",
                "--message-format=json",
            ],
            "unknown --message-format `bad`; expected text or json",
            true,
        ),
        (
            vec!["check", "--manifest-path=p", "--message-format"],
            "--message-format requires a value",
            false,
        ),
        (
            vec![
                "check",
                "--manifest-path=p",
                "--message-format=json",
                "--message-format=",
            ],
            "--message-format may only be specified once",
            true,
        ),
        (
            vec![
                "check",
                "--manifest-path=p",
                "--edition=legacy-0.9",
                "main.ox",
            ],
            "manifest check conflicts with --edition legacy-0.9",
            false,
        ),
        (
            vec!["check", "--manifest-path=p", "main.ox", "--output=x"],
            "manifest check does not accept a source operand",
            false,
        ),
    ] {
        let (failure, actual_json) = error(&words);
        assert_eq!(failure.classification(), ("E0001", "cli"));
        assert_eq!(failure.message(), expected, "{words:?}");
        assert_eq!(actual_json, json);
    }
    for words in [
        vec![
            "--edition=typed-preview",
            "check",
            "--",
            "--manifest-path=absent.toml",
        ],
        vec![
            "script",
            "probe",
            "--manifest-path=absent.toml",
            "--message-format=json",
        ],
        vec!["check", "main.ox"],
        vec![
            "--edition=typed-preview",
            "run",
            "main.ox",
            "--entry-mode=process",
        ],
    ] {
        assert!(select(&args(&words)).is_none(), "{words:?}");
    }
    for flag in [
        "--entry-mode=process",
        "--backend=llvm",
        "--target=x",
        "--output=x",
        "--experimental-hir-import=x",
        "--experimental-hir-producers=x",
        "--experimental-lexical-provider=x",
        "--locked",
        "--offline",
    ] {
        assert_eq!(
            error(&["check", "--manifest-path=p", flag])
                .0
                .classification()
                .0,
            "E0001"
        );
    }
    assert!(matches!(
        select(&args(&["check", "--manifest-path=p", "--"])),
        Some(Route::ManifestCheck { .. })
    ));
}

#[test]
fn manifest_grammar_line_and_trailing_contract() {
    for text in [
        VALID.to_string(),
        format!("{VALID}\n"),
        format!("{}\r\n", VALID.replace('\n', "\r\n")),
        format!(" # 日本語\n{VALID} # tail"),
        format!("{VALID}\nname=\"a\\n\\t\\u0000é#\"\nversion=\"1\""),
    ] {
        assert_eq!(prepared(&text).unwrap(), "src/main.ox");
    }
    for (text, reason) in [
        (format!("\u{feff}{VALID}"), "BOM"),
        (VALID.replace('\n', "\r"), "line end"),
        (format!("[project] entry=\"x.ox\"\n{VALID}"), "line end"),
        (format!("{VALID} xyz"), "line end"),
        (format!("{VALID};"), "line end"),
        (format!("{VALID}\u{a0}"), "line end"),
        (format!("{VALID}\nname=\"\\uD800\""), "scalar"),
        (format!("{VALID}\nname=\"a\t\""), "control"),
        (format!("{VALID}\n# bad\u{85}"), "control"),
        (
            format!("{VALID}\nentry=not-a-string"),
            "duplicate manifest key: project.entry",
        ),
        (
            format!("{VALID}\nmystery=bad\nentry=\"again.ox\""),
            "unsupported manifest key",
        ),
        (
            format!("{VALID}\nentry=bad\nmystery=bad"),
            "duplicate manifest key",
        ),
        (
            format!("{VALID}\n[dependencies]"),
            "unsupported manifest section",
        ),
    ] {
        let failure = prepared(&text).unwrap_err();
        assert_eq!(failure.classification().0, "E0001");
        assert!(
            failure.message().contains(reason),
            "{text:?}: expected {reason}"
        );
    }
    assert!(prepared("[scripts]\n")
        .unwrap_err()
        .message()
        .contains("line 1:"));
    for bad in [
        "",
        "/x.ox",
        "../x.ox",
        "./x.ox",
        "a//x.ox",
        "a/../x.ox",
        "C:/x.ox",
        "a\\\\b.ox",
        "a\\n.ox",
        "a.txt",
    ] {
        assert!(
            prepared(&format!(
                "[project]\nedition=\"typed-preview\"\nentry=\"{bad}\""
            ))
            .is_err(),
            "{bad}"
        );
    }
}

#[test]
fn manifest_decoded_limits_and_required_order() {
    let input = |edition: &str, entry: &str, name: &str, version: &str| {
        format!("[project]\nedition=\"{edition}\"\nentry=\"{entry}\"\nname=\"{name}\"\nversion=\"{version}\"")
    };
    let entry = format!("{}.ox", "a".repeat(2045));
    let name = "n".repeat(512);
    let version = "v".repeat(499);
    assert!(prepared(&input("typed-preview", &entry, &name, &version)).is_ok());
    for (text, reason) in [
        (
            input("typed-preview", &format!("a{entry}"), "", ""),
            LENGTH_ERRORS[1],
        ),
        (
            input("typed-preview", "x.ox", &"é".repeat(257), ""),
            LENGTH_ERRORS[2],
        ),
        (
            input("typed-preview", "x.ox", "", &"v".repeat(513)),
            LENGTH_ERRORS[3],
        ),
        (
            input("typed-preview", &entry, &name, &"v".repeat(500)),
            "manifest decoded total exceeds 3072 bytes",
        ),
        (
            format!("[project]\nedition=\"{}\"", "e".repeat(33)),
            LENGTH_ERRORS[0],
        ),
    ] {
        assert_eq!(prepared(&text).unwrap_err().message(), reason);
    }
    assert_eq!(
        prepared(&format!("[project]\nedition=\"{}\"", "e".repeat(32)))
            .unwrap_err()
            .classification()
            .0,
        "E0001"
    );
    assert!(prepared("[project]\nentry=\"x.ox\"")
        .unwrap_err()
        .message()
        .contains("requires [project].edition"));
    assert!(prepared("[project]\nedition=\"typed-preview\"")
        .unwrap_err()
        .message()
        .contains("requires [project].entry"));
    assert_eq!(
        prepared("[project]\nedition=\"typed\\u002dpreview\"\nentry=\"x.ox\"").unwrap(),
        "x.ox"
    );
    assert!(prepared(&format!("{VALID}\nname=\"{}\"", "\\u0061".repeat(512))).is_ok());
}

struct Split<'a> {
    data: &'a [u8],
    max: usize,
    interrupt: bool,
    calls: usize,
    fail_end: bool,
}
impl Read for Split<'_> {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        self.calls += 1;
        if self.interrupt {
            self.interrupt = false;
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self.data.is_empty() && self.fail_end {
            return Err(io::ErrorKind::Other.into());
        }
        let n = dst.len().min(self.max).min(self.data.len());
        dst[..n].copy_from_slice(&self.data[..n]);
        self.data = &self.data[n..];
        Ok(n)
    }
}
fn read_message(result: Result<Vec<u8>, ReadFailure>) -> String {
    match result {
        Err(ReadFailure::Limit(f)) => f.message(),
        Err(ReadFailure::Io(_)) => "io".into(),
        Ok(_) => panic!("expected read refusal"),
    }
}
#[test]
fn manifest_read_exact_extra_split_interrupted_precedence() {
    let mut raw = VALID.as_bytes().to_vec();
    raw.extend_from_slice(b"\n#");
    raw.resize(M_RAW, b' ');
    for max in [1, 7, 1024] {
        let mut reader = Split {
            data: &raw,
            max,
            interrupt: true,
            calls: 0,
            fail_end: false,
        };
        let bytes = read(&mut reader, &mut Allocator::default()).unwrap_or_else(|_| panic!("read"));
        assert_eq!(bytes, raw);
        assert_eq!(
            prepared(std::str::from_utf8(&bytes).unwrap()).unwrap(),
            "src/main.ox"
        );
        assert!(reader.calls > 2);
    }
    raw[0] = 0xff;
    raw.push(b'x');
    assert_eq!(
        read_message(read(&mut &raw[..], &mut Allocator::default())),
        "manifest input exceeds 16384 bytes"
    );
    raw.pop();
    let bytes = read(&mut &raw[..], &mut Allocator::default()).unwrap_or_else(|_| panic!("read"));
    assert_eq!(
        prepare("p", &bytes, &mut Allocator::default(), Operation::Check)
            .unwrap_err()
            .classification()
            .0,
        "E0003"
    );
    let mut reader = Split {
        data: &raw,
        max: 7,
        interrupt: false,
        calls: 0,
        fail_end: true,
    };
    assert_eq!(
        read_message(read(&mut reader, &mut Allocator::default())),
        "io"
    );
}

#[test]
fn manifest_allocation_sequence_and_invariants() {
    let text = format!("{VALID}\nname=\"n\"\nversion=\"v\"");
    for fail in 1..=8 {
        let allocator = Allocator {
            fail_at: Some(fail),
            ..Allocator::default()
        };
        let result = request("oxid.toml", false, allocator).and_then(|mut request| {
            let r = &mut request[0];
            let mut reader = Split {
                data: text.as_bytes(),
                max: 7,
                interrupt: true,
                calls: 0,
                fail_end: false,
            };
            let bytes = read(&mut reader, &mut r.allocator).map_err(|e| match e {
                ReadFailure::Limit(f) => f,
                _ => panic!("unexpected I/O"),
            });
            if fail == 3 {
                assert_eq!(reader.calls, 0);
            }
            prepare(&r.path, &bytes?, &mut r.allocator, Operation::Check)
        });
        let failure = result.unwrap_err();
        assert_eq!(failure.classification(), ("E0400", "manifest-input"));
        assert_eq!(
            failure.message(),
            "manifest transport allocation failed",
            "attempt {fail}"
        );
    }
    let allocator = Allocator {
        fail_at: Some(1),
        ..Allocator::default()
    };
    match cli::select_with(
        &args(&["check", "--manifest-path=", "--message-format=json"]),
        allocator,
    )
    .unwrap()
    {
        Route::ManifestError {
            failure,
            json: true,
            operation: Operation::Check,
        } => assert_eq!(failure.classification().0, "E0001"),
        _ => panic!(),
    }
    assert!(request(&"x".repeat(P_MANIFEST), false, Allocator::default()).is_ok());
    assert_eq!(
        request(
            &"x".repeat(P_MANIFEST + 1),
            false,
            Allocator {
                fail_at: Some(1),
                ..Allocator::default()
            }
        )
        .unwrap_err()
        .message(),
        "manifest path exceeds 4096 bytes"
    );
    assert_eq!(add(usize::MAX, 1).unwrap_err().classification().0, "E0400");
    let mut a = Allocator::default();
    assert_eq!(
        a.attempt(usize::MAX, 2).unwrap_err().classification().0,
        "E0400"
    );
    assert_eq!(
        visit(
            "a",
            Field {
                start: 0,
                end: 1,
                decoded: 0
            },
            |_| Ok(())
        )
        .unwrap_err(),
        Failure::Invariant
    );
    assert_eq!(Failure::Invariant.classification().0, "E0500");
    assert_eq!(capacity_failure().classification().0, "E0400");
    assert_eq!(layout_failure().classification().0, "E0400");
}

#[test]
fn manifest_lexical_parent_spelling() {
    for (path, expected) in [
        ("oxid.toml", "src/main.ox"),
        ("./oxid.toml", "./src/main.ox"),
        ("./cfg/oxid.toml", "./cfg/src/main.ox"),
        ("cfg//oxid.toml", "cfg//src/main.ox"),
        ("/oxid.toml", "/src/main.ox"),
    ] {
        assert_eq!(
            prepare(
                path,
                VALID.as_bytes(),
                &mut Allocator::default(),
                Operation::Check
            )
            .unwrap(),
            expected
        );
    }
    // Public selector/entry ceilings make the joined ceiling unreachable today;
    // still validate the defensive private admission without filesystem probing.
    assert_eq!(
        prepare(
            &format!("{}/p", "a".repeat(P_JOINED)),
            VALID.as_bytes(),
            &mut Allocator::default(),
            Operation::Check
        )
        .unwrap_err()
        .message(),
        "manifest entry path exceeds 8192 bytes"
    );
}

// Bounded, thread-local test instrumentation only. There is no production
// environment flag, tracing allocation, alternate CLI, or alternate reporter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Event {
    Allocation,
    PathMetadata,
    Open,
    HandleMetadata,
    Read,
    SourceLoad,
}
#[derive(Clone, Copy)]
struct Probe {
    enabled: bool,
    fail_at: Option<usize>,
    layout_slot: Option<usize>,
    capacity_fault: bool,
    invariant_fault: bool,
    events: [Event; 32],
    count: usize,
}
impl Default for Probe {
    fn default() -> Self {
        Self {
            enabled: false,
            fail_at: None,
            layout_slot: None,
            capacity_fault: false,
            invariant_fault: false,
            events: [Event::Allocation; 32],
            count: 0,
        }
    }
}
thread_local! { static PROBE: std::cell::RefCell<Probe> = std::cell::RefCell::new(Probe::default()); }
struct ProbeGuard;
impl ProbeGuard {
    fn new(probe: Probe) -> Self {
        PROBE.with(|p| *p.borrow_mut() = probe);
        Self
    }
}
impl Drop for ProbeGuard {
    fn drop(&mut self) {
        PROBE.with(|p| *p.borrow_mut() = Probe::default());
    }
}
pub(super) fn record(event: Event) {
    PROBE.with(|p| {
        let mut p = p.borrow_mut();
        if p.enabled {
            assert!(p.count < p.events.len(), "bounded probe overflow");
            let index = p.count;
            p.events[index] = event;
            p.count += 1;
        }
    });
}
pub(super) fn fail_at() -> Option<usize> {
    PROBE.with(|p| p.borrow().fail_at)
}
pub(super) fn capacity_fault() -> bool {
    PROBE.with(|p| p.borrow().capacity_fault)
}
pub(super) fn invariant_fault() -> bool {
    PROBE.with(|p| p.borrow().invariant_fault)
}
pub(super) fn layout_values(mut values: [usize; 4]) -> [usize; 4] {
    PROBE.with(|p| {
        if let Some(slot) = p.borrow().layout_slot {
            values[slot] = match slot {
                0 => H_CARRIER + 1,
                1 => H_FIXED + 1,
                2 => values[2] + 1,
                3 => values[3] * 2,
                _ => panic!(),
            };
        }
    });
    values
}
fn events() -> Vec<Event> {
    PROBE.with(|p| {
        let p = p.borrow();
        p.events[..p.count].to_vec()
    })
}

#[test]
fn manifest_script_names_and_global_error_tier() {
    for (words, reason, json) in [
        (
            vec!["script", "--manifest-path", "./pkg/oxid.toml", "echo"],
            "--manifest-path requires check or run",
            false,
        ),
        (
            vec![
                "script",
                "--manifest-path=./pkg/oxid.toml",
                "--message-format=json",
                "echo",
            ],
            "--manifest-path requires check or run",
            true,
        ),
        (
            vec![
                "script",
                "--manifest-path",
                "./pkg/oxid.toml",
                "--message-format=json",
                "echo",
            ],
            "--manifest-path requires check or run",
            true,
        ),
        (
            vec![
                "--manifest-path=p",
                "script",
                "--output",
                "x",
                "--message-format=json",
            ],
            "--manifest-path requires check or run",
            false,
        ),
        (
            vec![
                "--manifest-path=p",
                "script",
                "--output",
                "--message-format=json",
            ],
            "--manifest-path requires check or run",
            false,
        ),
        (
            vec![
                "check",
                "--wat",
                "--edition=bad",
                "--manifest-path=p",
                "--message-format=json",
            ],
            "unknown edition `bad`; expected legacy-0.9 or typed-preview",
            true,
        ),
        (
            vec![
                "check",
                "--output",
                "--edition=bad",
                "--manifest-path=p",
                "--message-format=json",
            ],
            "unknown edition `bad`; expected legacy-0.9 or typed-preview",
            true,
        ),
        (
            vec![
                "check",
                "--backend=",
                "--message-format=bad",
                "--manifest-path=p",
                "--message-format=json",
            ],
            "unknown --message-format `bad`; expected text or json",
            true,
        ),
        (
            vec![
                "check",
                "a.ox",
                "b.ox",
                "--edition=bad",
                "--manifest-path=p",
                "--message-format=json",
            ],
            "unknown edition `bad`; expected legacy-0.9 or typed-preview",
            true,
        ),
        (
            vec![
                "check",
                "--target",
                "--manifest-path=",
                "--message-format=json",
            ],
            "--manifest-path requires a value",
            true,
        ),
    ] {
        let (failure, actual_json) = error(&words);
        assert_eq!(failure.message(), reason, "{words:?}");
        assert_eq!(actual_json, json);
    }
    for local in ["--output", "--target", "--backend"] {
        assert!(select(&args(&[
            "script",
            local,
            "x",
            "--manifest-path=p",
            "--message-format=json"
        ]))
        .is_none());
    }
    assert!(select(&args(&[
        "script",
        "echo",
        "--manifest-path=p",
        "--message-format=json"
    ]))
    .is_none());
}

#[test]
fn manifest_local_options_do_not_hide_first_forwarded_word() {
    for local in [
        vec!["--output=unused"],
        vec!["--output", "unused"],
        vec!["--backend=unused"],
        vec!["--backend", "unused"],
        vec!["--target=unused"],
        vec!["--target", "unused"],
    ] {
        let mut words = vec!["--manifest-path=p"];
        words.extend_from_slice(&local);
        words.extend(["script", "echo", "--edition=bad", "--message-format=json"]);
        let (failure, json) = error(&words);
        assert_eq!(
            failure.message(),
            "unknown edition `bad`; expected legacy-0.9 or typed-preview",
            "{words:?}"
        );
        assert!(json, "{words:?}");

        // Reverse command/local order: the local-looking token is now the
        // actual script name, so later globals are script payload.
        let mut words = vec!["--manifest-path=p", "script"];
        words.extend_from_slice(&local);
        words.extend(["echo", "--edition=bad", "--message-format=json"]);
        let (failure, json) = error(&words);
        assert_eq!(failure.message(), "--manifest-path requires check or run");
        assert!(!json, "{words:?}");

        // Without a selector, the opt-in scan must still defer unchanged.
        assert!(select(&args(&words[1..])).is_none());
    }
}

#[test]
fn manifest_windows_lexical_prefixes_simulated() {
    // Pure spelling checks on this host, not Windows filesystem execution.
    for (path, joined) in [
        (r"C:oxid.toml", r"C:src/main.ox"),
        (r"C:cfg\oxid.toml", r"C:cfg\src/main.ox"),
        (r"\oxid.toml", r"\src/main.ox"),
        (r"C:\oxid.toml", r"C:\src/main.ox"),
        (r"C:\cfg\\oxid.toml", r"C:\cfg\\src/main.ox"),
        ("C:/cfg/oxid.toml", "C:/cfg/src/main.ox"),
        (r"\\server\share\oxid.toml", r"\\server\share\src/main.ox"),
        (
            r"\\server\share\cfg\oxid.toml",
            r"\\server\share\cfg\src/main.ox",
        ),
        (r"\\?\C:\cfg\oxid.toml", r"\\?\C:\cfg\src/main.ox"),
        ("oxid.toml", "src/main.ox"),
        ("./oxid.toml", "./src/main.ox"),
    ] {
        assert_eq!(format!("{}src/main.ox", parent(path, true)), joined);
    }
    assert_eq!(parent("C:oxid.toml", false), "");
}

#[test]
fn manifest_actual_layout_and_carrier_admission_refusals() {
    for slot in 0..4 {
        let _guard = ProbeGuard::new(Probe {
            enabled: true,
            layout_slot: Some(slot),
            fail_at: Some(1),
            ..Probe::default()
        });
        assert_eq!(
            request("missing.toml", false, Allocator::default())
                .unwrap_err()
                .message(),
            "manifest transport layout exceeds admitted bounds"
        );
        assert!(
            events().is_empty(),
            "layout rejection must precede allocation and I/O"
        );
    }
    let _guard = ProbeGuard::new(Probe {
        enabled: true,
        capacity_fault: true,
        ..Probe::default()
    });
    assert_eq!(
        request("missing.toml", false, Allocator::default())
            .unwrap_err()
            .message(),
        "manifest transport capacity differs from admitted carrier"
    );
    assert_eq!(events(), [Event::Allocation, Event::Allocation]);
}

#[test]
fn manifest_missing_grammar_and_precedence_controls() {
    let oversized = "n".repeat(513);
    for text in [
        format!("{VALID}\nmystery=\"{oversized}\""),
        format!("{VALID}\nmystery=bad\nname=\"{oversized}\""),
    ] {
        assert!(prepared(&text)
            .unwrap_err()
            .message()
            .contains("unsupported manifest key: project.mystery"));
    }
    assert!(prepared(&format!("{VALID}\n[project]\nname=bad"))
        .unwrap_err()
        .message()
        .contains("duplicate manifest table: project"));
    for field in ["name", "version"] {
        assert!(prepared(&format!("{VALID}\n{field}=\"{}\"", "n".repeat(512))).is_ok());
        let failure = prepared(&format!("{VALID}\n{field}=\"{}\"", "n".repeat(513))).unwrap_err();
        assert_eq!(
            failure.message(),
            format!("manifest decoded {field} exceeds 512 bytes")
        );
    }
}

// Spawn this test alone in the existing test binary, then enter the actual public
// frontend dispatch/report path. Fault configuration is absent from production.
#[test]
fn manifest_public_probe_child() {
    let Ok(case) = std::env::var("OXID_MANIFEST_TEST_PROBE") else {
        return;
    };
    let path = std::env::var("OXID_MANIFEST_TEST_PATH").unwrap();
    let json = std::env::var("OXID_MANIFEST_TEST_JSON").unwrap() == "1";
    let mut probe = Probe {
        enabled: true,
        ..Probe::default()
    };
    if case == "invariant" {
        probe.invariant_fault = true;
    } else if case != "success" {
        probe.fail_at = Some(case.parse().unwrap());
    }
    let _guard = ProbeGuard::new(probe);
    let mut argv = args(&["check", "--manifest-path", &path]);
    if json {
        argv.push("--message-format=json".into());
    }
    let status = crate::frontend::dispatch(&mut argv).expect("selected CHECK");
    let seen = events();
    if matches!(case.as_str(), "1" | "2") {
        assert!(seen.iter().all(|e| *e == Event::Allocation));
    }
    if case == "3" {
        assert!(seen.contains(&Event::PathMetadata));
        assert!(seen.contains(&Event::Open));
        assert!(seen.contains(&Event::HandleMetadata));
        assert!(!seen.contains(&Event::Read));
    }
    if matches!(case.as_str(), "4" | "5" | "6" | "7" | "8" | "invariant") {
        assert!(seen.contains(&Event::Read));
    }
    assert_eq!(seen.contains(&Event::SourceLoad), case == "success");
    assert_eq!(
        status,
        if case == "success" {
            0
        } else if case == "invariant" {
            2
        } else {
            1
        }
    );
    std::process::exit(status);
}

#[test]
fn manifest_public_fault_reporting_and_suppression() {
    use std::{
        fs,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let dir = std::env::temp_dir().join(format!("oxid-manifest-observer-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(dir.clone());
    fs::write(
        dir.join("oxid.toml"),
        "[project]\nedition=\"typed-preview\"\nentry=\"main.ox\"\nname=\"n\"\nversion=\"v\"",
    )
    .unwrap();
    fs::write(dir.join("main.ox"), "fn helper()->i32{return 7;}").unwrap();
    for case in [
        "1",
        "2",
        "3",
        "4",
        "5",
        "6",
        "7",
        "8",
        "invariant",
        "success",
    ] {
        for json in [false, true] {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "frontend::manifest::tests::manifest_public_probe_child",
                    "--nocapture",
                ])
                .env("OXID_MANIFEST_TEST_PROBE", case)
                .env("OXID_MANIFEST_TEST_PATH", dir.join("oxid.toml"))
                .env("OXID_MANIFEST_TEST_JSON", if json { "1" } else { "0" })
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while child.try_wait().unwrap().is_none() {
                if Instant::now() >= deadline {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("probe timeout");
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let out = child.wait_with_output().unwrap();
            let status = if case == "success" {
                0
            } else if case == "invariant" {
                2
            } else {
                1
            };
            assert_eq!(out.status.code(), Some(status), "{case} {out:?}");
            let stdout = String::from_utf8(out.stdout).unwrap();
            let stderr = String::from_utf8(out.stderr).unwrap();
            // Remove only libtest's known preamble; compiler output is unchanged.
            let stdout = stdout.strip_prefix("\nrunning 1 test\n").unwrap_or(&stdout);
            if json {
                assert!(stderr.is_empty(), "{stderr}");
                let lines: Vec<_> = stdout.lines().collect();
                if case == "success" {
                    assert_eq!(lines.len(), 1);
                    assert!(lines[0].contains("\"functions\":1"));
                } else {
                    assert_eq!(lines.len(), 2, "{stdout}");
                    let code = if case == "invariant" {
                        "E0500"
                    } else {
                        "E0400"
                    };
                    assert!(lines[0].contains(code));
                    assert!(lines[0].contains("\"primary\":null"));
                    assert!(!lines[0].contains("\"success\":"));
                    assert_eq!(lines[1],"{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":1,\"functions\":null}");
                }
            } else if case == "success" {
                assert!(stderr.is_empty());
                assert!(stdout.contains("check ok"));
            } else {
                assert!(stdout.is_empty(), "{stdout}");
                assert!(stderr.contains(if case == "invariant" {
                    "E0500"
                } else {
                    "E0400"
                }));
            }
        }
    }
}

fn refusal_with_operation(words: &[&str]) -> (String, bool, Operation) {
    match select(&args(words)).unwrap() {
        Route::ManifestError {
            failure,
            json,
            operation,
        } => {
            assert_eq!(failure.classification(), ("E0001", "cli"), "{words:?}");
            (failure.message(), json, operation)
        }
        route => panic!("expected refusal, got {route:?}"),
    }
}

#[test]
fn manifest_run_selection_wording_and_summary_operation() {
    for words in [
        vec!["run", "--manifest-path=p"],
        vec!["run", "--manifest-path", "p", "--message-format=json"],
        vec!["--edition=typed-preview", "run", "--manifest-path=p"],
        vec!["--message-format=json", "run", "--manifest-path=p"],
        vec!["run", "--manifest-path=p", "--edition", "typed-preview"],
        vec!["run", "--manifest-path=p", "--"],
    ] {
        assert!(
            matches!(select(&args(&words)), Some(Route::ManifestRun { .. })),
            "{words:?}"
        );
    }
    assert!(matches!(
        select(&args(&["check", "--manifest-path=p"])),
        Some(Route::ManifestCheck { .. })
    ));
    for (words, expected, json, operation) in [
        // Global-tier errors keep the CHECK summary even after `run`.
        (
            vec![
                "run",
                "--wat",
                "--manifest-path=missing.toml",
                "--edition=bad",
                "--message-format=json",
            ],
            "unknown edition `bad`; expected legacy-0.9 or typed-preview",
            true,
            Operation::Check,
        ),
        (
            vec!["run", "--manifest-path=", "--message-format=json"],
            "--manifest-path requires a value",
            true,
            Operation::Check,
        ),
        (
            vec!["run", "--manifest-path=p", "--manifest-path=q"],
            "--manifest-path may only be specified once",
            false,
            Operation::Check,
        ),
        // Valid globals: command/local errors after an actual `run` word.
        (
            vec!["run", "--manifest-path=p", "--wat", "--message-format=json"],
            "unsupported option for manifest run: --wat",
            true,
            Operation::Run,
        ),
        (
            vec!["--manifest-path=p", "run"],
            "--manifest-path must follow run",
            false,
            Operation::Run,
        ),
        (
            vec!["run", "--manifest-path=p", "--edition=legacy-0.9"],
            "manifest run conflicts with --edition legacy-0.9",
            false,
            Operation::Run,
        ),
        (
            vec![
                "run",
                "source.ox",
                "--manifest-path=missing.toml",
                "--output=out",
                "--message-format=json",
            ],
            "manifest run does not accept a source operand",
            true,
            Operation::Run,
        ),
        (
            vec!["run", "--manifest-path=p", "--", "argument"],
            "manifest run does not accept a source operand",
            false,
            Operation::Run,
        ),
        (
            vec![
                "run",
                "--manifest-path=missing.toml",
                "--output=out",
                "--message-format=json",
            ],
            "manifest run does not support entry-mode, backend, target, output or experimental selectors",
            true,
            Operation::Run,
        ),
        // Unsupported or missing command words keep the CHECK fallback.
        (
            vec![
                "compile",
                "--manifest-path=missing.toml",
                "--message-format=json",
            ],
            "--manifest-path requires check or run",
            true,
            Operation::Check,
        ),
        (
            vec!["--manifest-path=p"],
            "--manifest-path requires check or run",
            false,
            Operation::Check,
        ),
        (
            vec!["--wat", "run", "--manifest-path=p"],
            "unsupported option for manifest check: --wat",
            false,
            Operation::Check,
        ),
        (
            vec!["check", "--manifest-path=p", "--wat"],
            "unsupported option for manifest check: --wat",
            false,
            Operation::Check,
        ),
    ] {
        let (message, actual_json, actual_operation) = refusal_with_operation(&words);
        assert_eq!(message, expected, "{words:?}");
        assert_eq!(actual_json, json, "{words:?}");
        assert_eq!(actual_operation, operation, "{words:?}");
    }
    for command in ["build", "test", "package", "fmt", "new"] {
        let (message, _, operation) = refusal_with_operation(&[command, "--manifest-path=p"]);
        assert_eq!(message, "--manifest-path requires check or run");
        assert_eq!(operation, Operation::Check);
    }
    for flag in [
        "--entry-mode=result",
        "--entry-mode=process",
        "--backend=llvm",
        "--target=x",
        "--output=x",
        "--experimental-hir-import=x",
        "--experimental-hir-producers=x",
        "--experimental-lexical-provider=x",
    ] {
        let (message, _, operation) = refusal_with_operation(&["run", "--manifest-path=p", flag]);
        assert_eq!(message, "manifest run does not support entry-mode, backend, target, output or experimental selectors", "{flag}");
        assert_eq!(operation, Operation::Run);
    }
    for flag in ["--profile=x", "--release", "--offline", "--help"] {
        let (message, _, operation) = refusal_with_operation(&["run", "--manifest-path=p", flag]);
        assert_eq!(
            message,
            format!("unsupported option for manifest run: {flag}")
        );
        assert_eq!(operation, Operation::Run);
    }
    // No eligible selector: the gate defers without allocation or I/O.
    for words in [
        vec!["run", "main.ox"],
        vec![
            "--edition=typed-preview",
            "run",
            "--",
            "--manifest-path=missing.toml",
        ],
        vec!["script", "probe", "--manifest-path=absent.toml"],
        vec!["run"],
    ] {
        let _guard = ProbeGuard::new(Probe {
            enabled: true,
            ..Probe::default()
        });
        assert!(select(&args(&words)).is_none(), "{words:?}");
        assert!(events().is_empty(), "{words:?}");
    }
}

#[test]
fn manifest_run_decoder_wording_only() {
    let run = |text: &str| {
        prepare(
            "oxid.toml",
            text.as_bytes(),
            &mut Allocator::default(),
            Operation::Run,
        )
    };
    assert_eq!(run(VALID).unwrap(), "src/main.ox");
    for (text, expected) in [
        (
            "[project]\nentry=\"x.ox\"".to_string(),
            "manifest run requires [project].edition = \"typed-preview\"",
        ),
        (
            "[project]\nedition=\"legacy-0.9\"\nentry=\"x.ox\"".to_string(),
            "manifest run requires [project].edition = \"typed-preview\"",
        ),
        (
            "[project]\nedition=\"typed-preview\"".to_string(),
            "manifest run requires [project].entry",
        ),
        (
            format!("{VALID}\n[dependencies]"),
            "manifest `oxid.toml` line 4: unsupported manifest section for typed run: dependencies",
        ),
    ] {
        let failure = run(&text).unwrap_err();
        assert_eq!(failure.classification(), ("E0001", "cli"));
        assert_eq!(failure.message(), expected);
    }
    // Grammar, limits and resource classes are shared with CHECK unchanged.
    assert_eq!(
        run(&format!("{VALID}\nname=\"{}\"", "n".repeat(513)))
            .unwrap_err()
            .message(),
        "manifest decoded name exceeds 512 bytes"
    );
    assert!(run(&format!("{VALID}\nentry=\"again.ox\""))
        .unwrap_err()
        .message()
        .contains("duplicate manifest key: project.entry"));
}
