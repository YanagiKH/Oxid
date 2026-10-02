//! Public owned-source entry, early failure and artifact boundaries.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
const SENTINEL: &[u8] = b"existing output remains intact\n";

struct Scratch(PathBuf);
impl Scratch {
    fn new(text: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "oxid-owned-boundary-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("main.ox"), text).unwrap();
        fs::write(dir.join("output"), SENTINEL).unwrap();
        Self(dir)
    }

    fn typed(&self, operation: &str) -> Output {
        self.typed_with_output(operation, "output")
    }

    fn typed_with_output(&self, operation: &str, output: &str) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .current_dir(&self.0)
            .env("OXID_LLVM_BIN", self.0.join("missing-tools"))
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env_remove("OXID_PATH")
            .args([
                operation,
                "main.ox",
                "--edition=typed-preview",
                "--message-format=json",
            ]);
        if operation == "compile" {
            command.args(["--backend=llvm", "--output", output]);
        }
        command.output().unwrap()
    }

    fn unchanged_artifacts(&self) {
        assert_eq!(fs::read(self.0.join("output")).unwrap(), SENTINEL);
        assert!(!self.0.join("cache").exists());
        assert!(!self.0.join(".oxid").exists());
        assert!(!self.0.join("main.oxb").exists());
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn text(output: &Output) -> &str {
    assert!(output.stderr.is_empty(), "{output:?}");
    std::str::from_utf8(&output.stdout).unwrap()
}

fn diagnostic(output: &Output, code: &str, stage: &str) {
    let text = text(output);
    assert_eq!(output.status.code(), Some(1), "{text}");
    assert!(text.contains(&format!("\"code\":\"{code}\"")), "{text}");
    assert!(text.contains(&format!("\"stage\":\"{stage}\"")), "{text}");
    assert!(text.contains("\"success\":false"), "{text}");
    assert!(!text.contains("E0500"), "{text}");
}

#[test]
fn owned_source_reaches_check_reference_and_native_admission() {
    let scratch = Scratch::new(
        "struct State { n: i32, enabled: bool, marker: () } \
         fn read(p: &State) -> i32 { return p.n; } \
         fn update(p: &mut State) -> () { p.n = p.n + 2; return; } \
         fn relay(x: State) -> State { return x; } \
         fn main() -> i32 { \
             let mut x = State { marker: (), n: 3, enabled: true }; \
             update(&mut x); let y = relay(x); return read(&y); \
         }",
    );
    let check = scratch.typed("check");
    assert!(check.status.success(), "{check:?}");
    assert!(text(&check).contains("\"functions\":4"));
    let run = scratch.typed("run");
    assert!(run.status.success(), "{run:?}");
    assert!(
        text(&run).contains("\"result\":{\"type\":\"i32\",\"value\":5}"),
        "{}",
        text(&run)
    );
    // Valid owned source reaches the native toolchain on every host. Linux
    // rejects the existing output before tools, then the missing tool path for
    // a fresh output; other hosts reject the unsupported build host first.
    for output in ["output", "new-output"] {
        let compile = scratch.typed_with_output("compile", output);
        diagnostic(&compile, "E0701", "native-toolchain");
        let json = text(&compile);
        assert_eq!(json.lines().count(), 2, "{json}");
        assert!(json.contains("\"errors\":1,\"output\":null"), "{json}");
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            if output == "output" {
                assert!(json.contains("native output already exists"), "{json}");
            } else {
                assert!(json.contains("cannot launch "), "{json}");
                assert!(json.contains("missing-tools/clang"), "{json}");
            }
        } else {
            assert!(
                json.contains("native preview requires a Linux x86_64 build host"),
                "{json}"
            );
        }
        assert!(!json.contains("E0101"), "{json}");
        assert!(!scratch.0.join("new-output").exists());
        scratch.unchanged_artifacts();
    }
}

#[test]
fn source_errors_precede_native_tools_and_preserve_existing_outputs() {
    let cases = [
        ("struct T {} fn main() -> () { let x = T {}; let p = &x; return; }", "E0101", "parse"),
        ("struct T {} fn main() -> Missing { return T {}; }", "E0202", "resolve"),
        ("struct T { n: i32 } fn main() -> () { let x = T { n: true }; return; }", "E0300", "type"),
        ("struct T {} fn unused(x: T) -> () { x; x; return; } fn main() -> () { return; }", "E0310", "ownership"),
        ("struct T {} fn pair(a: &mut T, b: &T) -> () { return; } fn main() -> () { let mut x = T {}; pair(&mut x, &x); return; }", "E0311", "ownership"),
        ("struct T { n: i32 } fn write(p: &T) -> () { p.n = 7; return; } fn main() -> () { return; }", "E0313", "ownership"),
    ];
    for (source, code, stage) in cases {
        let scratch = Scratch::new(source);
        for operation in ["check", "run", "compile"] {
            let output = scratch.typed(operation);
            diagnostic(&output, code, stage);
            assert!(!text(&output).contains("E0701"));
            scratch.unchanged_artifacts();
        }
    }
}

#[test]
fn owned_check_does_not_imply_an_executable_scalar_entry() {
    for source in [
        "struct T {} fn helper() -> T { return T {}; }",
        "struct T {} fn main(x: T) -> () { return; }",
        "struct T {} fn main() -> T { return T {}; }",
    ] {
        let scratch = Scratch::new(source);
        let check = scratch.typed("check");
        assert!(check.status.success(), "{check:?}");
        assert!(text(&check).contains("\"success\":true"));
        let run = scratch.typed("run");
        assert_eq!(run.status.code(), Some(1));
        assert!(text(&run).contains("E0600"), "{}", text(&run));
        let compile = scratch.typed("compile");
        assert_eq!(compile.status.code(), Some(1));
        assert!(text(&compile).contains("E0700"), "{}", text(&compile));
        assert!(!text(&compile).contains("E0701"));
        scratch.unchanged_artifacts();
    }
}

#[test]
fn borrow_modes_are_exact_and_scalar_referents_remain_unavailable() {
    for source in [
        "struct T {} fn f(p: &T) -> () { return; } fn main() -> () { let mut x = T {}; f(&mut x); return; }",
        "struct T {} fn f(p: &mut T) -> () { return; } fn main() -> () { let mut x = T {}; f(&x); return; }",
    ] {
        let scratch = Scratch::new(source);
        diagnostic(&scratch.typed("check"), "E0300", "type");
        scratch.unchanged_artifacts();
    }
    let scratch = Scratch::new("fn f(p: &bool) -> () { return; }");
    diagnostic(&scratch.typed("check"), "E0202", "resolve");
    scratch.unchanged_artifacts();
}

#[test]
fn real_file_loader_accepts_one_mebibyte_and_rejects_one_byte_over() {
    const CAP: usize = 1_048_576;
    let mut source = "struct Marker {} fn main() -> () { return; }\n".to_owned();
    let comment = format!("/*{}*/", "x".repeat(4_092));
    while source.len() + comment.len() <= CAP {
        source.push_str(&comment);
    }
    source.push_str(&" ".repeat(CAP - source.len()));
    assert_eq!(source.len(), CAP);
    let scratch = Scratch::new(&source);
    for operation in ["check", "run"] {
        let output = scratch.typed(operation);
        assert!(output.status.success(), "{output:?}");
        assert!(text(&output).contains("\"success\":true"));
    }
    source.push(' ');
    fs::write(scratch.0.join("main.ox"), source).unwrap();
    for operation in ["check", "run", "compile"] {
        diagnostic(&scratch.typed(operation), "E0400", "source");
        scratch.unchanged_artifacts();
    }
}
