//! Acceptance for the pure typed `packages/bytes_hex` path package through two
//! independent consumers (dependency alias `hex`): the contract project
//! `examples/projects/bytes_hex_contracts` and the bounded Process workspace
//! `examples/projects/oxhex` (members `oxhex_encode` and `oxhex_decode`).
//! Sources are copied into a fresh repo-shaped temp tree with relative
//! dependency paths preserved; the in-tree projects are never built. Expected
//! bytes are fixed here or produced by the small independent `lower_hex` table
//! helper, never by the package. Every Process run asserts the exact status,
//! stdout and (empty) stderr. Regular-file stdin gives deterministic 1,024-byte
//! reads; controlled pipe injection waits for each step's non-empty output
//! before the next write to prove split reads (an empty-output step does not
//! wait, so it may merge with the next write). Interpreter only: no native/LLVM run,
//! capacity or fuel pressure, allocation fault, EINTR or short-write injection.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const RESULT: &[u8] = b"42\n";
const CONTRACTS: &str = "examples/projects/bytes_hex_contracts";
const WORKSPACE: &str = "examples/projects/oxhex";
const ENCODE: &str = "oxhex_encode";
const DECODE: &str = "oxhex_decode";

/// Source inputs copied into each fixture, relative to the repository root.
const SOURCES: [&str; 9] = [
    "packages/bytes_hex/oxid.toml",
    "packages/bytes_hex/src/lib.ox",
    "examples/projects/bytes_hex_contracts/oxid.toml",
    "examples/projects/bytes_hex_contracts/src/main.ox",
    "examples/projects/oxhex/oxid.toml",
    "examples/projects/oxhex/encode/oxid.toml",
    "examples/projects/oxhex/encode/src/main.ox",
    "examples/projects/oxhex/decode/oxid.toml",
    "examples/projects/oxhex/decode/src/main.ox",
];

/// Root `test_` declarations of the contracts project, in source order.
const CASES: &[&str] = &[
    "test_encode_all_bytes_fixed_oracle",
    "test_decode_all_bytes_every_case",
    "test_encode_empty_input",
    "test_encode_destination_sizes",
    "test_encode_nibble_order_lowercase",
    "test_decode_empty_input",
    "test_decode_destination_sizes",
    "test_decode_odd_length_first",
    "test_decode_invalid_each_nibble_position",
    "test_decode_invalid_first_middle_last",
    "test_decode_rejects_byte_classes",
    "test_decode_classifies_every_byte",
    "test_decode_invalid_beats_short_destination",
    "test_decode_too_small_only_for_valid_even",
    "test_inputs_unchanged",
    "test_round_trip_supplement",
];

/// Bytes 0..=255 as fixed lowercase, uppercase and mixed-case hex.
const ALL_LOWER: &[u8; 512] = b"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9fa0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebfc0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7e8e9eaebecedeeeff0f1f2f3f4f5f6f7f8f9fafbfcfdfeff";
const ALL_UPPER: &[u8; 512] = b"000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F202122232425262728292A2B2C2D2E2F303132333435363738393A3B3C3D3E3F404142434445464748494A4B4C4D4E4F505152535455565758595A5B5C5D5E5F606162636465666768696A6B6C6D6E6F707172737475767778797A7B7C7D7E7F808182838485868788898A8B8C8D8E8F909192939495969798999A9B9C9D9E9FA0A1A2A3A4A5A6A7A8A9AAABACADAEAFB0B1B2B3B4B5B6B7B8B9BABBBCBDBEBFC0C1C2C3C4C5C6C7C8C9CACBCCCDCECFD0D1D2D3D4D5D6D7D8D9DADBDCDDDEDFE0E1E2E3E4E5E6E7E8E9EAEBECEDEEEFF0F1F2F3F4F5F6F7F8F9FAFBFCFDFEFF";
const ALL_MIXED: &[u8; 512] = b"000102030405060708090a0b0C0d0e0F101112131415161718191a1B1c1d1E1f202122232425262728292A2b2c2D2e2f303132333435363738393a3b3C3d3e3F404142434445464748494a4B4c4d4E4f505152535455565758595A5b5c5D5e5f606162636465666768696a6b6C6d6e6F707172737475767778797a7B7c7d7E7f808182838485868788898A8b8c8D8e8f909192939495969798999a9b9C9d9e9FA0a1A2a3A4a5A6a7A8a9AaaBAcadAEafB0b1B2b3B4b5B6b7B8b9BAbbBcbDBebfC0c1C2c3C4c5C6c7C8c9CacbCCcdCecFD0d1D2d3D4d5D6d7D8d9DadBDcddDEdfE0e1E2e3E4e5E6e7E8e9EAebEceDEeefF0f1F2f3F4f5F6f7F8f9FafbFCfdFefF";

fn all_bytes() -> Vec<u8> {
    (0..=255).collect()
}

/// Independent lowercase oracle from a fixed digit table.
fn lower_hex(bytes: &[u8]) -> Vec<u8> {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = Vec::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(DIGITS[usize::from(byte >> 4)]);
        text.push(DIGITS[usize::from(byte & 15)]);
    }
    text
}

/// Deterministic nonuniform payload.
fn payload(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| ((index * 89 + 7) % 256) as u8)
        .collect()
}

fn joined(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-bytes-hex-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
        for path in SOURCES {
            let destination = root.join(path);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(repository.join(path), destination).unwrap();
        }
        Self { root }
    }

    fn command(&self, project: &str, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .args(args)
            .current_dir(self.root.join(project))
            .env("OXID_LLVM_BIN", self.root.join("absent-llvm"))
            .env("OXID_CACHE_DIR", self.root.join("cache"))
            .env("PATH", self.root.join("absent-path"))
            .env_remove("OXID_PATH");
        command
    }

    fn oxid(&self, project: &str, args: &[&str]) -> Output {
        self.command(project, args)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn client(&self, package: &str) -> Command {
        let selected = format!("--package={package}");
        self.command(
            WORKSPACE,
            &[
                "run",
                "--manifest-path=oxid.toml",
                &selected,
                "--engine=interpreter",
                "--entry-mode=process",
            ],
        )
    }

    /// Regular-file stdin: every read returns min(request, remaining) bytes.
    fn stdin_file(&self, input: &[u8]) -> File {
        let path = self.root.join(format!(
            "stdin-{}.bin",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, input).unwrap();
        File::open(path).unwrap()
    }

    fn stream(&self, package: &str, input: &[u8]) -> Output {
        self.client(package)
            .stdin(self.stdin_file(input))
            .output()
            .unwrap()
    }

    /// Piped stdin: the parent closes stdout's only read end first, and only
    /// then supplies the input and closes stdin, so the client cannot read
    /// (and therefore cannot write) before the reader is gone.
    fn closed_stdout(&self, package: &str, input: &[u8]) -> Output {
        let mut child = self
            .client(package)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        drop(child.stdout.take());
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(input).unwrap();
        drop(stdin);
        child.wait_with_output().unwrap()
    }

    /// Controlled chunk injection: each step is one pipe write, and its
    /// expected output must arrive before the next step is written, so the
    /// client consumed that step in an earlier read. A step with empty
    /// expected output does not wait and may be merged with the next write
    /// into one read; it is not a read-boundary guarantee.
    fn lockstep(&self, package: &str, steps: &[(&[u8], &[u8])]) -> Output {
        let mut child = self
            .client(package)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let (sender, receiver) = mpsc::channel::<Vec<u8>>();
        let reader = thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            while let Ok(count @ 1..) = stdout.read(&mut chunk) {
                if sender.send(chunk[..count].to_vec()).is_err() {
                    break;
                }
            }
        });
        let errors = thread::spawn(move || {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).unwrap();
            bytes
        });
        let mut seen = Vec::new();
        for (input, output) in steps {
            stdin.write_all(input).unwrap();
            stdin.flush().unwrap();
            let wanted = seen.len() + output.len();
            while seen.len() < wanted {
                let chunk = receiver
                    .recv_timeout(Duration::from_secs(120))
                    .expect("step output before the next write");
                seen.extend_from_slice(&chunk);
            }
        }
        drop(stdin);
        let status = child.wait().unwrap();
        reader.join().unwrap();
        seen.extend(receiver.try_iter().flatten());
        Output {
            status,
            stdout: seen,
            stderr: errors.join().unwrap(),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn assert_exact(label: &str, output: &Output, status: i32, stdout: &[u8]) {
    assert_eq!(output.status.code(), Some(status), "{label}: {output:?}");
    assert_eq!(output.stdout, stdout, "{label}: {output:?}");
    assert!(output.stderr.is_empty(), "{label}: {output:?}");
}

fn assert_check(label: &str, output: &Output) {
    assert_eq!(output.status.code(), Some(0), "{label}: {output:?}");
    assert!(output.stderr.is_empty(), "{label}: {output:?}");
    let text = std::str::from_utf8(&output.stdout).unwrap();
    // The inventory includes package functions, so validate the exact summary
    // shape without guessing the count.
    let count = text
        .strip_prefix("typed-preview check ok (")
        .and_then(|rest| rest.strip_suffix(" functions; check only)\n"))
        .expect("one successful check summary");
    assert!(count.parse::<usize>().unwrap() > 0, "{label}: {text}");
}

#[test]
fn bytes_hex_contracts_check_and_interpret_fixed_result() {
    let f = Fixture::new();
    let checked = f.oxid(CONTRACTS, &["check", "--manifest-path=oxid.toml"]);
    assert_check("contracts check", &checked);
    let ran = f.oxid(
        CONTRACTS,
        &[
            "run",
            "--manifest-path=oxid.toml",
            "--engine=interpreter",
            "--entry-mode=result",
        ],
    );
    assert_exact("contracts result", &ran, 0, RESULT);
}

#[test]
fn bytes_hex_contracts_pass_every_named_case() {
    let f = Fixture::new();
    let tested = f.oxid(
        CONTRACTS,
        &["test", "--manifest-path=oxid.toml", "--engine=interpreter"],
    );
    let mut expected = String::new();
    for case in CASES {
        expected.push_str(&format!("PASS {case}\n"));
    }
    let total = CASES.len();
    expected.push_str(&format!(
        "test result: {total} passed; 0 failed; {total} total\n"
    ));
    assert_exact("contracts test", &tested, 0, expected.as_bytes());
}

#[test]
fn oxhex_members_check() {
    let f = Fixture::new();
    for package in [ENCODE, DECODE] {
        let selected = format!("--package={package}");
        let checked = f.oxid(
            WORKSPACE,
            &["check", "--manifest-path=oxid.toml", &selected],
        );
        assert_check(package, &checked);
    }
}

#[test]
fn oxhex_encode_streams_exact_lowercase() {
    assert_eq!(lower_hex(&all_bytes()), ALL_LOWER);
    let f = Fixture::new();
    let all = f.stream(ENCODE, &all_bytes());
    assert_exact("all-256", &all, 0, ALL_LOWER);
    for (label, input) in [
        ("empty", Vec::new()),
        ("single-zero", vec![0]),
        ("length-1023", payload(1023)),
        ("length-1024", payload(1024)),
        ("length-1025", payload(1025)),
        ("length-2049", payload(2049)),
    ] {
        let output = f.stream(ENCODE, &input);
        assert_exact(label, &output, 0, &lower_hex(&input));
    }
}

#[test]
fn oxhex_decode_streams_exact_bytes() {
    let f = Fixture::new();
    let all = all_bytes();
    for (label, input) in [
        ("all-lower", &ALL_LOWER[..]),
        ("all-upper", &ALL_UPPER[..]),
        ("all-mixed", &ALL_MIXED[..]),
    ] {
        let output = f.stream(DECODE, input);
        assert_exact(label, &output, 0, &all);
    }
    let control = f.stream(DECODE, b"00Ff80");
    assert_exact("mixed-control", &control, 0, b"\x00\xff\x80");
    let empty = f.stream(DECODE, b"");
    assert_exact("empty", &empty, 0, b"");
    for (label, length) in [
        ("chars-1024", 512),
        ("chars-1026", 513),
        ("chars-2048", 1024),
        ("chars-2050", 1025),
    ] {
        let bytes = payload(length);
        let output = f.stream(DECODE, &lower_hex(&bytes));
        assert_exact(label, &output, 0, &bytes);
    }
    let upper_bytes = payload(700);
    let upper = lower_hex(&upper_bytes).to_ascii_uppercase();
    let output = f.stream(DECODE, &upper);
    assert_exact("upper-chars-1400", &output, 0, &upper_bytes);
}

#[test]
fn oxhex_decode_malformed_streams_keep_earlier_chunks_only() {
    let f = Fixture::new();
    let first = payload(512);
    let first_hex = lower_hex(&first);
    let both = payload(1024);
    let both_hex = lower_hex(&both);
    let cases: [(&str, Vec<u8>, &[u8]); 12] = [
        ("odd-single", b"0".to_vec(), b""),
        ("odd-tail-not-hex", b"00ffz".to_vec(), b"\x00\xff"),
        ("odd-trailing-lf", b"00ff\n".to_vec(), b"\x00\xff"),
        ("odd-valid-abc", b"abc".to_vec(), b"\xab"),
        ("invalid-in-first-chunk", b"00ff0g".to_vec(), b""),
        ("trailing-crlf", b"00ff\r\n".to_vec(), b""),
        ("prefix-0x", b"0x41".to_vec(), b""),
        (
            "first-chunk-early-invalid",
            joined(&[b"0g".as_slice(), first_hex.as_slice()]),
            b"",
        ),
        (
            "odd-after-chunk",
            joined(&[first_hex.as_slice(), b"g".as_slice()]),
            first.as_slice(),
        ),
        (
            "late-invalid-pair",
            joined(&[first_hex.as_slice(), b"0g".as_slice()]),
            first.as_slice(),
        ),
        (
            "late-invalid-after-valid",
            joined(&[first_hex.as_slice(), b"0011zz".as_slice()]),
            first.as_slice(),
        ),
        (
            "late-invalid-after-two",
            joined(&[both_hex.as_slice(), b"x1".as_slice()]),
            both.as_slice(),
        ),
    ];
    for (label, input, stdout) in cases {
        let output = f.stream(DECODE, &input);
        assert_exact(label, &output, 65, stdout);
    }
}

#[test]
fn oxhex_controlled_split_reads() {
    let f = Fixture::new();
    let encoded = f.lockstep(
        ENCODE,
        &[(b"\x00", b"00"), (b"\xff\x10", b"ff10"), (b"\xab", b"ab")],
    );
    assert_exact("encode-steps", &encoded, 0, b"00ff10ab");
    let split = f.lockstep(
        DECODE,
        &[
            (b"41a", b"\x41"),
            (b"b42", b"\xab\x42"),
            (b"c", b""),
            (b"D", b"\xcd"),
        ],
    );
    assert_exact("decode-split-pairs", &split, 0, b"\x41\xab\x42\xcd");
    let late = f.lockstep(DECODE, &[(b"00ff", b"\x00\xff"), (b"1z", b"")]);
    assert_exact("decode-late-invalid", &late, 65, b"\x00\xff");
    let odd = f.lockstep(DECODE, &[(b"4", b"")]);
    assert_exact("decode-odd-eof", &odd, 65, b"");
}

#[test]
fn oxhex_closed_stdout_is_io_failure() {
    let f = Fixture::new();
    let encoded = f.closed_stdout(ENCODE, b"ab");
    assert_exact("encode-closed-stdout", &encoded, 74, b"");
    let decoded = f.closed_stdout(DECODE, b"6162");
    assert_exact("decode-closed-stdout", &decoded, 74, b"");
}
