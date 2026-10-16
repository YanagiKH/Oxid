//! Stdlib `strings.trim` contract regressions.
//!
//! These tests exercise the public CLI source and OXBC routes the same way as
//! `tests/legacy_modules.rs`, with expected values fixed independently of the
//! trim implementation under test.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

/// ASCII whitespace characters that `trim` must strip at both ends.
const ASCII_TRIM_WS: &[char] = &[
    '\u{0009}', // HT
    '\u{000A}', // LF
    '\u{000B}', // VT
    '\u{000C}', // FF
    '\u{000D}', // CR
    '\u{0020}', // SPACE
];

struct Project(PathBuf);

impl Project {
    fn new(main_source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-stdlib-strings-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create isolated stdlib strings project");
        let stdlib = root.join("stdlib");
        fs::create_dir(&stdlib).expect("create project stdlib");
        // Always copy the tree's current stdlib/strings.ox so the under-test
        // trim implementation is whatever the checkout provides.
        let strings_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("stdlib/strings.ox");
        fs::copy(&strings_src, stdlib.join("strings.ox")).unwrap_or_else(|error| {
            panic!("copy {} into test project: {error}", strings_src.display())
        });
        fs::write(root.join("main.ox"), main_source).expect("write main.ox");
        Self(root)
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.path("cache"))
            .env_remove("OXID_PATH");
        command
    }

    fn run(&self, path: &Path) -> Output {
        self.command()
            .arg("run")
            .arg(path)
            .output()
            .expect("run fixture through the public CLI")
    }

    fn compile(&self) -> Output {
        self.command()
            .arg("compile")
            .arg(self.path("main.ox"))
            .arg("-o")
            .arg(self.path("main.oxb"))
            .output()
            .expect("compile fixture through the production writer")
    }

    fn routes(&self) -> [Output; 2] {
        let source = self.run(&self.path("main.ox"));
        let compile = self.compile();
        assert!(compile.status.success(), "{compile:?}");
        assert!(compile.stderr.is_empty(), "{compile:?}");
        assert!(fs::read(self.path("main.oxb"))
            .expect("read artifact")
            .starts_with(b"OXBC"));
        [source, self.run(&self.path("main.oxb"))]
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_success(output: &Output, stdout: &str) {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        stdout,
        "stdout mismatch\nactual bytes: {:?}\nexpected bytes: {:?}",
        output.stdout,
        stdout.as_bytes()
    );
}

/// Encode `text` as an Oxid double-quoted string literal.
///
/// Uses `\n` / `\r` / `\t` escapes so preprocess `lines()` cannot split a
/// literal. Vertical tab and form feed have no dedicated escapes in the
/// legacy lexer, so they are embedded as raw characters.
fn ox_string_literal(text: &str) -> String {
    let mut out = String::from('"');
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn program_printing_trims(inputs: &[&str]) -> String {
    let mut source = String::from("use \"strings.ox\";\n");
    for input in inputs {
        source.push_str("print trim(");
        source.push_str(&ox_string_literal(input));
        source.push_str(");\n");
    }
    source
}

fn program_printing_double_trims(inputs: &[&str]) -> String {
    let mut source = String::from("use \"strings.ox\";\n");
    for input in inputs {
        source.push_str("print trim(trim(");
        source.push_str(&ox_string_literal(input));
        source.push_str("));\n");
    }
    source
}

fn expected_prints(values: &[&str]) -> String {
    let mut out = String::new();
    for value in values {
        out.push_str(value);
        out.push('\n');
    }
    out
}

fn assert_trim_cases(inputs: &[&str], expected: &[&str]) {
    assert_eq!(inputs.len(), expected.len());
    let project = Project::new(&program_printing_trims(inputs));
    let want = expected_prints(expected);
    for output in project.routes() {
        assert_success(&output, &want);
    }
}

fn assert_double_trim_cases(inputs: &[&str], expected: &[&str]) {
    assert_eq!(inputs.len(), expected.len());
    let project = Project::new(&program_printing_double_trims(inputs));
    let want = expected_prints(expected);
    for output in project.routes() {
        assert_success(&output, &want);
    }
}

#[test]
fn trim_empty_and_all_ascii_whitespace() {
    // Fixed expected values: empty stays empty; runs of only the six ASCII
    // whitespace code points collapse to empty.
    assert_trim_cases(
        &[
            "",
            " ",
            "   ",
            "\t\t",
            "\n\n",
            "\u{000B}\u{000B}",
            "\u{000C}\u{000C}",
            "\r\r",
            " \t\n\u{000B}\u{000C}\r ",
        ],
        &["", "", "", "", "", "", "", "", ""],
    );
}

#[test]
fn trim_left_right_and_both_sides() {
    assert_trim_cases(
        &[
            "  hello",
            "hello  ",
            "  hello  ",
            "\thello",
            "hello\n",
            " \thello\n ",
        ],
        &["hello", "hello", "hello", "hello", "hello", "hello"],
    );
}

#[test]
fn trim_each_ascii_whitespace_character_on_both_ends() {
    // One case per whitespace character, plus a mixed-ends case.
    assert_trim_cases(
        &[
            "\thello\t",
            "\nhello\n",
            "\u{000B}hello\u{000B}",
            "\u{000C}hello\u{000C}",
            "\rhello\r",
            " hello ",
            "\t\n\u{000B}\u{000C}\r hello \t\n\u{000B}\u{000C}\r",
        ],
        &[
            "hello", "hello", "hello", "hello", "hello", "hello", "hello",
        ],
    );
}

#[test]
fn trim_keeps_internal_ascii_whitespace_and_newlines() {
    assert_trim_cases(
        &[
            "  hel lo  ",
            "  hel\tlo  ",
            "  hel\nlo  ",
            "\nhel\nlo\n",
            "  a b\tc\rd\u{000B}e\u{000C}f  ",
        ],
        &[
            "hel lo",
            "hel\tlo",
            "hel\nlo",
            "hel\nlo",
            "a b\tc\rd\u{000B}e\u{000C}f",
        ],
    );
}

#[test]
fn trim_preserves_unicode_content() {
    assert_trim_cases(
        &[
            "  你好  ",
            "\t世界\n",
            "  Oxid語言  ",
            "  café  ",
            "  𝄞 music  ",
        ],
        &["你好", "世界", "Oxid語言", "café", "𝄞 music"],
    );
}

#[test]
fn trim_retains_non_ascii_whitespace() {
    // NBSP U+00A0, Em Space U+2003, Ideographic Space U+3000 must not be
    // stripped even when they sit at the ends.
    assert_trim_cases(
        &[
            "\u{00A0}hello\u{00A0}",
            "\u{2003}hello\u{2003}",
            "\u{3000}hello\u{3000}",
            "  \u{00A0}kept\u{00A0}  ",
            "\t\u{3000}edge\u{2003}\n",
            "\u{00A0}",
            "\u{00A0}\u{2003}\u{3000}",
        ],
        &[
            "\u{00A0}hello\u{00A0}",
            "\u{2003}hello\u{2003}",
            "\u{3000}hello\u{3000}",
            "\u{00A0}kept\u{00A0}",
            "\u{3000}edge\u{2003}",
            "\u{00A0}",
            "\u{00A0}\u{2003}\u{3000}",
        ],
    );
}

#[test]
fn trim_is_idempotent() {
    // Expected values are the single-trim results of the contract, not a
    // second call into the implementation under test.
    assert_double_trim_cases(
        &[
            "",
            "hello",
            "  hello  ",
            "\t\n hello \r",
            "  hel\nlo  ",
            "\u{00A0}x\u{00A0}",
            "  \u{00A0}y\u{00A0}  ",
            " \t\n\u{000B}\u{000C}\r ",
        ],
        &[
            "",
            "hello",
            "hello",
            "hello",
            "hel\nlo",
            "\u{00A0}x\u{00A0}",
            "\u{00A0}y\u{00A0}",
            "",
        ],
    );
}

#[test]
fn ascii_trim_whitespace_table_matches_contract() {
    // Guard the local documentation of the six code points; this does not
    // call Oxid trim. Failures here mean the test file drifted from the
    // stated contract, not that the stub changed.
    let mut encoded = Vec::new();
    for ch in ASCII_TRIM_WS {
        encoded.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
    }
    assert_eq!(
        encoded,
        [0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x20],
        "ASCII trim whitespace table drifted"
    );
}
