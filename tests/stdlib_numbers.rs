//! Ordinary integer parity regressions for the checkout's actual stdlib.
//!
//! Every case runs through the public CLI source and compile -> OXBC -> run
//! routes. Inputs are exactly representable integer literals; expected results
//! are fixed literals, not computed with either parity implementation. These
//! tests do not specify fractional, non-finite, or rounded integer inputs.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);
const STDLIB_MODULES: &[&str] = &["numbers.ox", "core.ox"];

struct Project(PathBuf);

impl Project {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-stdlib-numbers-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create isolated stdlib numbers project");
        let stdlib = root.join("stdlib");
        fs::create_dir(&stdlib).expect("create project stdlib");
        let checkout_stdlib = Path::new(env!("CARGO_MANIFEST_DIR")).join("stdlib");
        for module in STDLIB_MODULES {
            let src = checkout_stdlib.join(module);
            fs::copy(&src, stdlib.join(module)).unwrap_or_else(|error| {
                panic!("copy {} into test project: {error}", src.display())
            });
        }
        // Match the text fixture's dependency guard: a new import must be
        // copied explicitly rather than resolved from an ambient checkout.
        for module in STDLIB_MODULES {
            let source = fs::read_to_string(stdlib.join(module))
                .unwrap_or_else(|error| panic!("read copied {module}: {error}"));
            for import in source
                .lines()
                .filter_map(|line| line.trim().strip_prefix("use \""))
                .filter_map(|rest| rest.split_once('"'))
                .map(|(path, _)| path)
            {
                assert!(
                    STDLIB_MODULES.contains(&import),
                    "stdlib/{module} imports {import:?}, which the test project does not copy"
                );
            }
        }
        fs::write(root.join("main.ox"), source).expect("write main.ox");
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

    fn routes(&self) -> [(&'static str, Output); 2] {
        let source = self.run(&self.path("main.ox"));
        let compile = self
            .command()
            .arg("compile")
            .arg(self.path("main.ox"))
            .arg("-o")
            .arg(self.path("main.oxb"))
            .output()
            .expect("compile fixture through the production writer");
        assert_eq!(compile.status.code(), Some(0), "{compile:?}");
        assert!(compile.stderr.is_empty(), "{compile:?}");
        assert!(fs::read(self.path("main.oxb"))
            .expect("read artifact")
            .starts_with(b"OXBC"));
        [
            ("source", source),
            ("OXBC", self.run(&self.path("main.oxb"))),
        ]
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_program(source: &str, expected: &str) {
    let project = Project::new(source);
    let mut failures = Vec::new();
    // Inspect both routes before reporting, so a source failure cannot hide
    // the corresponding artifact result in a regression log.
    for (route, output) in project.routes() {
        if output.status.code() != Some(0)
            || !output.stderr.is_empty()
            || output.stdout != expected.as_bytes()
        {
            failures.push(format!(
                "{route}: expected exit 0, empty stderr, stdout {expected:?}; actual {output:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn assert_parity_cases(cases: &[(&str, &str)]) {
    let mut source = String::from("use \"numbers.ox\";\n");
    let mut expected = String::new();
    for (input, result) in cases {
        source.push_str(&format!(
            "print json_stringify([is_even({input}), is_odd({input})]);\n"
        ));
        expected.push_str(result);
        expected.push('\n');
    }
    assert_program(&source, &expected);
}

#[test]
fn zero_is_even_and_not_odd() {
    assert_parity_cases(&[("0", "[true,false]")]);
}

#[test]
fn positive_integers_have_expected_parity() {
    assert_parity_cases(&[
        ("1", "[false,true]"),
        ("2", "[true,false]"),
        ("3", "[false,true]"),
        ("4", "[true,false]"),
        ("17", "[false,true]"),
        ("42", "[true,false]"),
    ]);
}

#[test]
fn negative_integers_have_expected_parity() {
    assert_parity_cases(&[
        ("-1", "[false,true]"),
        ("-2", "[true,false]"),
        ("-3", "[false,true]"),
        ("-4", "[true,false]"),
        ("-17", "[false,true]"),
        ("-42", "[true,false]"),
    ]);
}

#[test]
fn larger_exactly_representable_integers_have_expected_parity() {
    assert_parity_cases(&[
        ("1048575", "[false,true]"),
        ("1048576", "[true,false]"),
        ("2147483647", "[false,true]"),
        ("2147483648", "[true,false]"),
        ("-2147483647", "[false,true]"),
        ("-2147483648", "[true,false]"),
        ("9007199254740991", "[false,true]"),
        ("9007199254740992", "[true,false]"),
        ("-9007199254740991", "[false,true]"),
        ("-9007199254740992", "[true,false]"),
    ]);
}

#[test]
fn integer_parity_helpers_are_complements() {
    let mut source = String::from("use \"numbers.ox\";\n");
    for input in ["0", "1", "-1", "2", "-2", "9007199254740991"] {
        source.push_str(&format!(
            "print json_stringify([is_even({input}) != is_odd({input}), \
             is_odd({input}) == !is_even({input})]);\n"
        ));
    }
    assert_program(
        &source,
        "[true,true]\n[true,true]\n[true,true]\n[true,true]\n[true,true]\n[true,true]\n",
    );
}
