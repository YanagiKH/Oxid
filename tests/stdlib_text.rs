//! Stdlib `text.upper` / `text.lower` contract regressions.
//!
//! Contract: only the 26 ASCII letters are case-mapped (`a`-`z` <-> `A`-`Z`).
//! Every other character, including non-ASCII letters that have Unicode case
//! mappings, punctuation, digits, whitespace and control characters, is kept
//! byte-for-byte. These tests run the checkout's actual `stdlib/text.ox`
//! through the public CLI source route and the compile -> OXBC -> run route,
//! the same way as `tests/stdlib_strings.rs`. Every expected value is a fixed
//! literal written independently of the implementation under test.
//!
//! `text.ox` may import `strings.ox`, so each temp project copies both
//! modules from the checkout, and setup fails loudly if a copied module
//! imports another stdlib module that was not copied.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

/// Checkout stdlib modules copied into every temp project: the `text.ox`
/// entry point plus its `strings.ox` module dependency.
const STDLIB_MODULES: &[&str] = &["text.ox", "strings.ox"];

/// Paths named by top-level `use "...";` lines in an Oxid source file.
fn use_imports(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| line.trim().strip_prefix("use \""))
        .filter_map(|rest| rest.split_once('"'))
        .map(|(path, _)| path.to_string())
        .collect()
}

struct Project(PathBuf);

impl Project {
    fn new(main_source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-stdlib-text-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create isolated stdlib text project");
        let stdlib = root.join("stdlib");
        fs::create_dir(&stdlib).expect("create project stdlib");
        // Always copy the tree's current stdlib/text.ox and its strings.ox
        // dependency so the under-test upper/lower implementation is whatever
        // the checkout provides, imports included.
        let checkout_stdlib = Path::new(env!("CARGO_MANIFEST_DIR")).join("stdlib");
        for module in STDLIB_MODULES {
            let src = checkout_stdlib.join(module);
            fs::copy(&src, stdlib.join(module)).unwrap_or_else(|error| {
                panic!("copy {} into test project: {error}", src.display())
            });
        }
        // Fixture dependency guard: every import of a copied module must
        // resolve inside the temp project, so a new dependency surfaces here
        // instead of as an opaque import failure in every case.
        for module in STDLIB_MODULES {
            let source = fs::read_to_string(stdlib.join(module))
                .unwrap_or_else(|error| panic!("read copied {module}: {error}"));
            for import in use_imports(&source) {
                assert!(
                    STDLIB_MODULES.contains(&import.as_str()),
                    "stdlib/{module} imports {import:?}, which the test project does not copy"
                );
            }
        }
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
/// literal. Other control characters have no dedicated escapes in the legacy
/// lexer, so they are embedded as raw characters.
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

/// Build a program that prints `calls(input)` for every case, where `calls`
/// is applied innermost-last: `&["lower", "upper"]` prints
/// `lower(upper(input))`.
fn program(calls: &[&str], cases: &[(&str, &str)]) -> String {
    let mut source = String::from("use \"text.ox\";\n");
    for (input, _) in cases {
        source.push_str("print ");
        for call in calls {
            source.push_str(call);
            source.push('(');
        }
        source.push_str(&ox_string_literal(input));
        for _ in calls {
            source.push(')');
        }
        source.push_str(";\n");
    }
    source
}

fn expected_prints(cases: &[(&str, &str)]) -> String {
    let mut out = String::new();
    for (_, expected) in cases {
        out.push_str(expected);
        out.push('\n');
    }
    out
}

fn assert_cases(calls: &[&str], cases: &[(&str, &str)]) {
    let project = Project::new(&program(calls, cases));
    let want = expected_prints(cases);
    for output in project.routes() {
        assert_success(&output, &want);
    }
}

// ---------------------------------------------------------------------------
// Fixed literal oracle tables: (input, expected output).
// ---------------------------------------------------------------------------

const UPPER_EACH_LETTER: &[(&str, &str)] = &[
    ("a", "A"),
    ("b", "B"),
    ("c", "C"),
    ("d", "D"),
    ("e", "E"),
    ("f", "F"),
    ("g", "G"),
    ("h", "H"),
    ("i", "I"),
    ("j", "J"),
    ("k", "K"),
    ("l", "L"),
    ("m", "M"),
    ("n", "N"),
    ("o", "O"),
    ("p", "P"),
    ("q", "Q"),
    ("r", "R"),
    ("s", "S"),
    ("t", "T"),
    ("u", "U"),
    ("v", "V"),
    ("w", "W"),
    ("x", "X"),
    ("y", "Y"),
    ("z", "Z"),
];
const LOWER_EACH_LETTER: &[(&str, &str)] = &[
    ("A", "a"),
    ("B", "b"),
    ("C", "c"),
    ("D", "d"),
    ("E", "e"),
    ("F", "f"),
    ("G", "g"),
    ("H", "h"),
    ("I", "i"),
    ("J", "j"),
    ("K", "k"),
    ("L", "l"),
    ("M", "m"),
    ("N", "n"),
    ("O", "o"),
    ("P", "p"),
    ("Q", "q"),
    ("R", "r"),
    ("S", "s"),
    ("T", "t"),
    ("U", "u"),
    ("V", "v"),
    ("W", "w"),
    ("X", "x"),
    ("Y", "y"),
    ("Z", "z"),
];

const UPPER_ALPHABET: &[(&str, &str)] = &[
    ("abcdefghijklmnopqrstuvwxyz", "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
    ("zyxwvutsrqponmlkjihgfedcba", "ZYXWVUTSRQPONMLKJIHGFEDCBA"),
    ("ABCDEFGHIJKLMNOPQRSTUVWXYZ", "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
];

const LOWER_ALPHABET: &[(&str, &str)] = &[
    ("ABCDEFGHIJKLMNOPQRSTUVWXYZ", "abcdefghijklmnopqrstuvwxyz"),
    ("ZYXWVUTSRQPONMLKJIHGFEDCBA", "zyxwvutsrqponmlkjihgfedcba"),
    ("abcdefghijklmnopqrstuvwxyz", "abcdefghijklmnopqrstuvwxyz"),
];

/// Empty input stays empty.
const EMPTY: &[(&str, &str)] = &[("", "")];

const UPPER_MIXED: &[(&str, &str)] = &[
    ("Hello, World!", "HELLO, WORLD!"),
    ("oXiD lang 2026", "OXID LANG 2026"),
    ("snake_case-and.dot", "SNAKE_CASE-AND.DOT"),
    ("a1b2c3", "A1B2C3"),
    ("MiXeD\tCaSe\nLine", "MIXED\tCASE\nLINE"),
];

const LOWER_MIXED: &[(&str, &str)] = &[
    ("Hello, World!", "hello, world!"),
    ("oXiD LANG 2026", "oxid lang 2026"),
    ("SNAKE_CASE-AND.DOT", "snake_case-and.dot"),
    ("A1B2C3", "a1b2c3"),
    ("MiXeD\tCaSe\nLine", "mixed\tcase\nline"),
];

/// Already in the target case: output equals input.
const UPPER_ALREADY: &[(&str, &str)] = &[
    ("HELLO", "HELLO"),
    ("HELLO, WORLD! 123", "HELLO, WORLD! 123"),
    ("Q", "Q"),
];

const LOWER_ALREADY: &[(&str, &str)] = &[
    ("hello", "hello"),
    ("hello, world! 123", "hello, world! 123"),
    ("q", "q"),
];

/// Printable ASCII with letters and the code points that sit right next to
/// the letter ranges (`@` 0x40, `[` 0x5B, `` ` `` 0x60, `{` 0x7B).
const UPPER_ASCII_NON_LETTERS: &[(&str, &str)] = &[
    (
        " !\"#$%&'()*+,-./0123456789:;<=>?@[\\]^_`{|}~",
        " !\"#$%&'()*+,-./0123456789:;<=>?@[\\]^_`{|}~",
    ),
    ("@AZ[`az{", "@AZ[`AZ{"),
    (
        " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~",
        " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`ABCDEFGHIJKLMNOPQRSTUVWXYZ{|}~",
    ),
];

const LOWER_ASCII_NON_LETTERS: &[(&str, &str)] = &[
    (
        " !\"#$%&'()*+,-./0123456789:;<=>?@[\\]^_`{|}~",
        " !\"#$%&'()*+,-./0123456789:;<=>?@[\\]^_`{|}~",
    ),
    ("@AZ[`az{", "@az[`az{"),
    (
        " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~",
        " !\"#$%&'()*+,-./0123456789:;<=>?@abcdefghijklmnopqrstuvwxyz[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~",
    ),
];

/// ASCII whitespace and control characters (C0, DEL, and C1 NEL) are kept.
const UPPER_WHITESPACE_CONTROLS: &[(&str, &str)] = &[
    (" \t\n\u{000B}\u{000C}\r", " \t\n\u{000B}\u{000C}\r"),
    (
        "\u{0001}\u{0007}\u{0008}\u{001B}\u{001F}\u{007F}\u{0085}",
        "\u{0001}\u{0007}\u{0008}\u{001B}\u{001F}\u{007F}\u{0085}",
    ),
    ("\u{001B}[1ma\u{001B}[0m", "\u{001B}[1MA\u{001B}[0M"),
    (
        "\ta\u{000B}b\u{000C}c\rd\u{007F}e",
        "\tA\u{000B}B\u{000C}C\rD\u{007F}E",
    ),
];

const LOWER_WHITESPACE_CONTROLS: &[(&str, &str)] = &[
    (" \t\n\u{000B}\u{000C}\r", " \t\n\u{000B}\u{000C}\r"),
    (
        "\u{0001}\u{0007}\u{0008}\u{001B}\u{001F}\u{007F}\u{0085}",
        "\u{0001}\u{0007}\u{0008}\u{001B}\u{001F}\u{007F}\u{0085}",
    ),
    ("\u{001B}[1MA\u{001B}[0M", "\u{001B}[1ma\u{001B}[0m"),
    (
        "\tA\u{000B}B\u{000C}C\rD\u{007F}E",
        "\ta\u{000B}b\u{000C}c\rd\u{007F}e",
    ),
];

/// Non-ASCII characters are never case-mapped, including ones whose Unicode
/// mappings land on ASCII letters (U+017F long s -> `S`, U+212A Kelvin sign
/// -> `k`, U+0130 dotted capital I -> `i`, U+0131 dotless i -> `I`) or expand
/// (U+00DF sharp s, U+FB00 ligature ff, U+0149).
const UNICODE_PRESERVED: &[&str] = &[
    "é",
    "É",
    "ß",
    "\u{1E9E}",
    "ǅ",
    "Σσς",
    "Жж",
    "Ωω",
    "\u{00B5}",
    "\u{0149}",
    "\u{0130}",
    "\u{0131}",
    "\u{017F}",
    "\u{212A}",
    "\u{FB00}",
    "\u{FF21}\u{FF41}",
    "𝐀𝐚",
    "你好",
    "\u{00A0}\u{2003}\u{3000}",
];

const UPPER_UNICODE_MIXED: &[(&str, &str)] = &[
    ("café", "CAFé"),
    ("straße", "STRAßE"),
    ("\u{212A}elvin", "\u{212A}ELVIN"),
    ("\u{017F}tar", "\u{017F}TAR"),
    ("ı i I \u{0130}", "ı I I \u{0130}"),
    ("Oxid語言 ok", "OXID語言 OK"),
];

const LOWER_UNICODE_MIXED: &[(&str, &str)] = &[
    ("CAFÉ", "cafÉ"),
    ("STRAẞE", "straẞe"),
    ("\u{212A}ELVIN", "\u{212A}elvin"),
    ("\u{0130}STANBUL", "\u{0130}stanbul"),
    ("ı i I \u{0130}", "ı i i \u{0130}"),
    ("OXID語言 OK", "oxid語言 ok"),
];

/// Single-application results; applied twice they must not change.
const UPPER_IDEMPOTENT: &[(&str, &str)] = &[
    ("", ""),
    ("hello", "HELLO"),
    ("Hello, World!", "HELLO, WORLD!"),
    ("abcdefghijklmnopqrstuvwxyz", "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
    ("straße \u{212A} \u{017F}", "STRAßE \u{212A} \u{017F}"),
    ("\ta\u{001B}b\u{007F}", "\tA\u{001B}B\u{007F}"),
];

const LOWER_IDEMPOTENT: &[(&str, &str)] = &[
    ("", ""),
    ("HELLO", "hello"),
    ("Hello, World!", "hello, world!"),
    ("ABCDEFGHIJKLMNOPQRSTUVWXYZ", "abcdefghijklmnopqrstuvwxyz"),
    ("STRAẞE \u{212A} \u{0130}", "straẞe \u{212A} \u{0130}"),
    ("\tA\u{001B}B\u{007F}", "\ta\u{001B}b\u{007F}"),
];

/// `lower(upper(x))`: restores `x` only when `x` was already lowercase ASCII;
/// mixed input comes back fully lowercased, and non-ASCII survives unchanged.
const LOWER_OF_UPPER: &[(&str, &str)] = &[
    ("", ""),
    ("hello world", "hello world"),
    ("abcdefghijklmnopqrstuvwxyz", "abcdefghijklmnopqrstuvwxyz"),
    ("Hello World", "hello world"),
    ("straße ǅ Σσς \u{212A}", "straße ǅ Σσς \u{212A}"),
    ("Oxid語言 \t1", "oxid語言 \t1"),
];

/// `upper(lower(x))`: restores `x` only when `x` was already uppercase ASCII.
const UPPER_OF_LOWER: &[(&str, &str)] = &[
    ("", ""),
    ("HELLO WORLD", "HELLO WORLD"),
    ("ABCDEFGHIJKLMNOPQRSTUVWXYZ", "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
    ("Hello World", "HELLO WORLD"),
    ("STRAẞE ǅ Σσς \u{0130}", "STRAẞE ǅ Σσς \u{0130}"),
    ("Oxid語言 \t1", "OXID語言 \t1"),
];

fn unchanged(values: &[&'static str]) -> Vec<(&'static str, &'static str)> {
    values.iter().map(|value| (*value, *value)).collect()
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

#[test]
fn upper_maps_each_ascii_lowercase_letter() {
    assert_cases(&["upper"], UPPER_EACH_LETTER);
}

#[test]
fn lower_maps_each_ascii_uppercase_letter() {
    assert_cases(&["lower"], LOWER_EACH_LETTER);
}

#[test]
fn upper_maps_full_alphabet() {
    assert_cases(&["upper"], UPPER_ALPHABET);
}

#[test]
fn lower_maps_full_alphabet() {
    assert_cases(&["lower"], LOWER_ALPHABET);
}

#[test]
fn upper_and_lower_keep_empty_string() {
    assert_cases(&["upper"], EMPTY);
    assert_cases(&["lower"], EMPTY);
}

#[test]
fn upper_maps_mixed_text() {
    assert_cases(&["upper"], UPPER_MIXED);
}

#[test]
fn lower_maps_mixed_text() {
    assert_cases(&["lower"], LOWER_MIXED);
}

#[test]
fn already_converted_text_is_unchanged() {
    assert_cases(&["upper"], UPPER_ALREADY);
    assert_cases(&["lower"], LOWER_ALREADY);
}

#[test]
fn ascii_punctuation_digits_and_letter_range_neighbours_are_kept() {
    assert_cases(&["upper"], UPPER_ASCII_NON_LETTERS);
    assert_cases(&["lower"], LOWER_ASCII_NON_LETTERS);
}

#[test]
fn whitespace_and_control_characters_are_kept() {
    assert_cases(&["upper"], UPPER_WHITESPACE_CONTROLS);
    assert_cases(&["lower"], LOWER_WHITESPACE_CONTROLS);
}

#[test]
fn non_ascii_characters_are_never_case_mapped() {
    let cases = unchanged(UNICODE_PRESERVED);
    assert_cases(&["upper"], &cases);
    assert_cases(&["lower"], &cases);
}

#[test]
fn upper_maps_only_ascii_inside_unicode_text() {
    assert_cases(&["upper"], UPPER_UNICODE_MIXED);
}

#[test]
fn lower_maps_only_ascii_inside_unicode_text() {
    assert_cases(&["lower"], LOWER_UNICODE_MIXED);
}

#[test]
fn upper_and_lower_are_idempotent() {
    // Expected values are the single-application results of the contract,
    // not a second call into the implementation under test.
    assert_cases(&["upper", "upper"], UPPER_IDEMPOTENT);
    assert_cases(&["lower", "lower"], LOWER_IDEMPOTENT);
}

#[test]
fn round_trips_restore_only_text_already_in_the_target_case() {
    assert_cases(&["lower", "upper"], LOWER_OF_UPPER);
    assert_cases(&["upper", "lower"], UPPER_OF_LOWER);
}

#[test]
fn literal_oracle_tables_match_ascii_only_contract() {
    // Guard the fixed literals against drift from the stated contract; this
    // does not call Oxid. Failures here mean the test file is wrong, not that
    // the implementation changed. `str::to_ascii_*` maps exactly `a`-`z` /
    // `A`-`Z` and leaves every other character untouched.
    let upper_tables: &[&[(&str, &str)]] = &[
        UPPER_EACH_LETTER,
        UPPER_ALPHABET,
        EMPTY,
        UPPER_MIXED,
        UPPER_ALREADY,
        UPPER_ASCII_NON_LETTERS,
        UPPER_WHITESPACE_CONTROLS,
        UPPER_UNICODE_MIXED,
        UPPER_IDEMPOTENT,
    ];
    let lower_tables: &[&[(&str, &str)]] = &[
        LOWER_EACH_LETTER,
        LOWER_ALPHABET,
        EMPTY,
        LOWER_MIXED,
        LOWER_ALREADY,
        LOWER_ASCII_NON_LETTERS,
        LOWER_WHITESPACE_CONTROLS,
        LOWER_UNICODE_MIXED,
        LOWER_IDEMPOTENT,
    ];
    for (input, expected) in upper_tables.iter().flat_map(|table| table.iter()) {
        assert_eq!(
            *expected,
            input.to_ascii_uppercase(),
            "upper oracle for {input:?}"
        );
    }
    for (input, expected) in lower_tables.iter().flat_map(|table| table.iter()) {
        assert_eq!(
            *expected,
            input.to_ascii_lowercase(),
            "lower oracle for {input:?}"
        );
    }
    for (input, expected) in LOWER_OF_UPPER {
        assert_eq!(*expected, input.to_ascii_uppercase().to_ascii_lowercase());
    }
    for (input, expected) in UPPER_OF_LOWER {
        assert_eq!(*expected, input.to_ascii_lowercase().to_ascii_uppercase());
    }
    for value in UNICODE_PRESERVED {
        assert!(!value.is_ascii(), "{value:?} must be non-ASCII");
    }
    let lower: Vec<u8> = UPPER_EACH_LETTER
        .iter()
        .flat_map(|(i, _)| i.bytes())
        .collect();
    let upper: Vec<u8> = UPPER_EACH_LETTER
        .iter()
        .flat_map(|(_, e)| e.bytes())
        .collect();
    assert_eq!(lower, (b'a'..=b'z').collect::<Vec<u8>>());
    assert_eq!(upper, (b'A'..=b'Z').collect::<Vec<u8>>());
    assert_eq!(LOWER_EACH_LETTER.len(), 26);
}
