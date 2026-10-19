//! Client acceptance for adjacent lowercase `b"..."` byte literals whose value
//! is the existing owned `[u8; N]` (`b""` is `[u8; 0]`). The published project
//! is copied to a fresh directory, never built in-tree. Expected results are
//! fixed in advance: check succeeds; result-mode execution prints exactly
//! `42\n`, exits 0, and leaves stderr empty. Every example byte is pinned by an
//! existing `[byte(n), ...]` array in the project source, so no host decoder is
//! involved. `b"\xFF"` is a legal byte array that the ordinary utf8_text
//! package must refuse; byte literals are not UTF-8 strings.
//! Run the ignored native test explicitly with:
//! `OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --test typed_byte_literals -- --include-ignored`.
//! No compiler changes, capacity-edge probes, fault injection, or fuel probes.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const RESULT: &[u8] = b"42\n";
const CHECK_FAILED: &str = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":1,\"functions\":null}";
const PRELUDE: &str = "fn byte(value:i32)->u8{return value.to_u8_checked();} ";

struct Fixture {
    root: PathBuf,
    project: PathBuf,
    llvm: PathBuf,
}
impl Fixture {
    fn new(llvm: Option<PathBuf>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-typed-byte-literals-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
        let project = root.join("examples/projects/typed_byte_literals");
        // Preserve manifest-relative dependency paths; copy only source inputs.
        for path in [
            "examples/projects/typed_byte_literals/oxid.toml",
            "examples/projects/typed_byte_literals/src/main.ox",
            "packages/utf8_text/oxid.toml",
            "packages/utf8_text/src/lib.ox",
        ] {
            let destination = root.join(path);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(repository.join(path), destination).unwrap();
        }
        Self {
            llvm: llvm.unwrap_or_else(|| root.join("absent-llvm")),
            root,
            project,
        }
    }

    fn oxid(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.project)
            .env("OXID_LLVM_BIN", &self.llvm)
            .env("OXID_CACHE_DIR", self.root.join("cache"))
            .env("PATH", self.root.join("absent-path"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn reference(&self) -> Output {
        self.oxid(&[
            "run",
            "--manifest-path=oxid.toml",
            "--engine=interpreter",
            "--entry-mode=result",
        ])
    }

    fn check_json(&self, source: &str) -> Vec<String> {
        fs::write(self.project.join("src/main.ox"), source).unwrap();
        let output = self.oxid(&[
            "check",
            "--manifest-path=oxid.toml",
            "--message-format=json",
        ]);
        assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
        assert!(output.stderr.is_empty(), "{source}: {output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        let lines: Vec<String> = text.lines().map(str::to_owned).collect();
        assert_eq!(
            lines.len(),
            2,
            "exactly one diagnostic for {source}: {text}"
        );
        assert_eq!(lines[1], CHECK_FAILED, "{source}: {text}");
        lines
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn assert_success(output: &Output, stdout: &[u8]) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, stdout, "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}

fn diagnostic_prefix(code: &str, stage: &str, message: Option<&str>) -> String {
    let head = format!(
        "{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"{code}\",\"stage\":\"{stage}\",\"message\":\""
    );
    match message {
        Some(message) => format!("{head}{message}\",\"primary\":{{"),
        None => head,
    }
}

#[test]
fn byte_literal_project_checks_and_interprets_fixed_result() {
    let f = Fixture::new(None);
    let checked = f.oxid(&["check", "--manifest-path=oxid.toml"]);
    assert_eq!(checked.status.code(), Some(0), "{checked:?}");
    assert!(checked.stderr.is_empty(), "{checked:?}");
    let text = std::str::from_utf8(&checked.stdout).unwrap();
    // Validate the exact summary shape without guessing a function count.
    let count = text
        .strip_prefix("typed-preview check ok (")
        .and_then(|s| s.strip_suffix(" functions; check only)\n"))
        .expect("one successful check summary");
    assert!(count.parse::<usize>().unwrap() > 0, "{text}");
    assert_success(&f.reference(), RESULT);
}

#[test]
fn byte_literal_values_keep_existing_array_type_and_ownership_rules() {
    // Byte literals are ordinary owned [u8; N] values, so these rejections are
    // the existing array type and ownership checks; the code/stage/message
    // identities are those the same programs produce with `[byte(n), ...]`
    // arrays. Primary spans of the literal token are not guessed here.
    for (source, code, stage, message) in [
        (
            "fn main()->i32{let a:[u8;2]=b\"abc\";return a.len();}",
            "E0300",
            "type",
            Some("type mismatch: expected [u8; 2], found [u8; 3]"),
        ),
        (
            "fn main()->i32{let a:[u8;1]=b\"\";return a.len();}",
            "E0300",
            "type",
            Some("type mismatch: expected [u8; 1], found [u8; 0]"),
        ),
        (
            "fn want(a:[u8;3])->i32{return a.len();} fn main()->i32{return want(b\"ab\");}",
            "E0300",
            "type",
            Some("call argument type or borrow mode does not match parameter"),
        ),
        (
            "fn want(a:[i32;2])->i32{return a.len();} fn main()->i32{let a=b\"ab\";return want(a);}",
            "E0300",
            "type",
            Some("call argument type or borrow mode does not match parameter"),
        ),
        (
            "fn main()->i32{let a=b\"ab\";let b=a;return a.len();}",
            "E0310",
            "ownership",
            Some("owned value is not available on every path"),
        ),
        (
            "fn take(a:[u8;2])->i32{return 0;} fn main()->i32{let a=b\"ab\";let n=take(a);return a.len()+n;}",
            "E0310",
            "ownership",
            Some("owned value is not available on every path"),
        ),
    ] {
        let f = Fixture::new(None);
        let lines = f.check_json(&format!("{PRELUDE}{source}"));
        let prefix = diagnostic_prefix(code, stage, message);
        assert!(lines[0].starts_with(&prefix), "{source}: {lines:?}");
    }
}

/// Lexical/grammar expectations are fixed from the source-derived diagnostic
/// contract, never recorded from candidate output. Offsets are zero-based,
/// half-open UTF-8 bytes; line/column locations count Unicode scalars from one.
struct RejectedSpelling {
    spelling: &'static str,
    code: &'static str,
    stage: &'static str,
    message: &'static str,
    // start, end, line, column, end_line, end_column
    primary: [usize; 6],
}

const REJECTED_SPELLINGS: &[RejectedSpelling] = &[
    RejectedSpelling {
        spelling: "b\"\\q\"",
        code: "E0101",
        stage: "parse",
        message: "unsupported or incomplete byte literal escape",
        primary: [23, 25, 1, 24, 1, 26],
    },
    RejectedSpelling {
        spelling: "b\"\\x4\"",
        code: "E0101",
        stage: "parse",
        message: "byte literal hex escapes require exactly two ASCII hex digits",
        primary: [23, 26, 1, 24, 1, 27],
    },
    RejectedSpelling {
        spelling: "b\"\\xG0\"",
        code: "E0101",
        stage: "parse",
        message: "byte literal hex escapes require exactly two ASCII hex digits",
        primary: [23, 27, 1, 24, 1, 28],
    },
    RejectedSpelling {
        spelling: "b\"\\x0g\"",
        code: "E0101",
        stage: "parse",
        message: "byte literal hex escapes require exactly two ASCII hex digits",
        primary: [23, 27, 1, 24, 1, 28],
    },
    RejectedSpelling {
        spelling: "b\"\\u{41}\"",
        code: "E0101",
        stage: "parse",
        message: "unsupported or incomplete byte literal escape",
        primary: [23, 25, 1, 24, 1, 26],
    },
    RejectedSpelling {
        spelling: "b\"\\'\"",
        code: "E0101",
        stage: "parse",
        message: "unsupported or incomplete byte literal escape",
        primary: [23, 25, 1, 24, 1, 26],
    },
    RejectedSpelling {
        spelling: "b\"\u{e9}\"",
        code: "E0101",
        stage: "parse",
        message: "byte literals accept raw printable ASCII only",
        primary: [23, 25, 1, 24, 1, 25],
    },
    RejectedSpelling {
        spelling: "b\"\u{7f}\"",
        code: "E0101",
        stage: "parse",
        message: "byte literals require escapes for control bytes and newlines",
        primary: [23, 24, 1, 24, 1, 25],
    },
    RejectedSpelling {
        spelling: "b\"\t\"",
        code: "E0101",
        stage: "parse",
        message: "byte literals require escapes for control bytes and newlines",
        primary: [23, 24, 1, 24, 1, 25],
    },
    RejectedSpelling {
        spelling: "b\"a\nb\"",
        code: "E0101",
        stage: "parse",
        message: "byte literals require escapes for control bytes and newlines",
        primary: [24, 25, 1, 25, 2, 1],
    },
    RejectedSpelling {
        spelling: "b\"a\\\nb\"",
        code: "E0101",
        stage: "parse",
        message: "unsupported or incomplete byte literal escape",
        primary: [24, 26, 1, 25, 2, 1],
    },
    RejectedSpelling {
        spelling: "br\"a\"",
        code: "E0101",
        stage: "parse",
        message: "unsupported typed-preview construct `\"a\"`",
        primary: [23, 26, 1, 24, 1, 27],
    },
    RejectedSpelling {
        spelling: "b\"a\"u8",
        code: "E0101",
        stage: "parse",
        message: "byte literal suffixes are unsupported",
        primary: [25, 26, 1, 26, 1, 27],
    },
    RejectedSpelling {
        spelling: "b \"a\"",
        code: "E0101",
        stage: "parse",
        message: "unsupported typed-preview construct `\"a\"`",
        primary: [23, 26, 1, 24, 1, 27],
    },
    RejectedSpelling {
        spelling: "b/**/\"a\"",
        code: "E0101",
        stage: "parse",
        message: "unsupported typed-preview construct `\"a\"`",
        primary: [26, 29, 1, 27, 1, 30],
    },
    RejectedSpelling {
        spelling: "B\"a\"",
        code: "E0101",
        stage: "parse",
        message: "unsupported typed-preview construct `\"a\"`",
        primary: [22, 25, 1, 23, 1, 26],
    },
    RejectedSpelling {
        spelling: "b\"a",
        code: "E0100",
        stage: "lex",
        message: "unterminated string literal",
        primary: [21, 41, 1, 22, 1, 42],
    },
];

// Independent JSON string quoting: do not call the compiler's formatter.
fn expected_json_string(text: &str) -> String {
    use std::fmt::Write;

    let mut quoted = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\u{8}' => quoted.push_str("\\b"),
            '\t' => quoted.push_str("\\t"),
            '\n' => quoted.push_str("\\n"),
            '\u{c}' => quoted.push_str("\\f"),
            '\r' => quoted.push_str("\\r"),
            '\u{0}'..='\u{1f}' => write!(quoted, "\\u{:04x}", ch as u32).unwrap(),
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}

#[test]
fn expected_json_string_quotes_controls_and_preserves_unicode() {
    assert_eq!(
        expected_json_string("\"\\\u{8}\t\n\u{c}\r\0\u{1f}é\u{7f}"),
        "\"\\\"\\\\\\b\\t\\n\\f\\r\\u0000\\u001fé\u{7f}\""
    );
}

#[test]
fn byte_literal_rejections_report_one_fixed_diagnostic() {
    for expected in REJECTED_SPELLINGS {
        let f = Fixture::new(None);
        let spelling = expected.spelling;
        let source = format!("fn main()->i32{{let a={spelling};return a.len();}}");
        let lines = f.check_json(&source);
        let [start, end, line, column, end_line, end_column] = expected.primary;
        // This fixture passes literal `oxid.toml`: its entry display path stays
        // `src/main.ox`. The root is parsed first into a fresh SourceMap, before
        // dependency sources, so its file_id is 0 even with a path dependency.
        let diagnostic = format!(
            concat!(
                "{{\"schema_version\":1,\"edition\":\"typed-preview\",",
                "\"kind\":\"diagnostic\",\"severity\":\"error\",",
                "\"code\":{},\"stage\":{},\"message\":{},",
                "\"primary\":{{\"file_id\":0,\"path\":\"src/main.ox\",",
                "\"start\":{},\"end\":{},\"line\":{},\"column\":{},",
                "\"end_line\":{},\"end_column\":{}}},",
                "\"secondary\":[],\"notes\":[]}}"
            ),
            expected_json_string(expected.code),
            expected_json_string(expected.stage),
            expected_json_string(expected.message),
            start,
            end,
            line,
            column,
            end_line,
            end_column,
        );
        assert_eq!(lines[0], diagnostic, "{source:?}: {lines:?}");
    }
}

fn llvm_bin() -> PathBuf {
    let dir = PathBuf::from(std::env::var_os("OXID_TEST_LLVM_BIN").expect(
        "native byte-literal regression requires OXID_TEST_LLVM_BIN (LLVM 19.1.7 bin directory)",
    ));
    // Resolve relative opt-in paths before changing to the fixture directory.
    let dir = fs::canonicalize(dir).expect("LLVM bin directory must exist");
    for (tool, marker) in [
        ("clang", "clang version 19.1.7"),
        ("opt", "LLVM version 19.1.7"),
        ("ld.lld", "LLD 19.1.7"),
    ] {
        let version = Command::new(dir.join(tool))
            .arg("--version")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(version.status.success(), "{tool}: {version:?}");
        assert!(
            String::from_utf8_lossy(&version.stdout).contains(marker),
            "{tool}: {version:?}"
        );
    }
    dir
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN; no installation or fallback"]
fn standalone_native_byte_literals_match_fixed_reference_result() {
    let f = Fixture::new(Some(llvm_bin()));
    assert_success(&f.reference(), RESULT);
    let artifact = f
        .project
        .join(".oxid/typed-preview/result/typed-byte-literals");
    assert!(!artifact.exists());
    let compiled = f.oxid(&["build", "--manifest-path=oxid.toml", "--entry-mode=result"]);
    assert_success(
        &compiled,
        b"typed-preview native build ok: \".oxid/typed-preview/result/typed-byte-literals\"\n",
    );
    assert!(artifact.is_file(), "compiler must produce the artifact");
    let executed = Command::new(&artifact)
        .current_dir("/")
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_success(&executed, RESULT);
}
