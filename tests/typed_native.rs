use std::process::Command;

#[test]
fn native_compile_requires_explicit_backend_and_output() {
    let out = Command::new(env!("CARGO_BIN_EXE_oxid"))
        .args([
            "compile",
            "missing.ox",
            "--edition=typed-preview",
            "--backend=llvm",
            "--message-format=json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("--output"), "{text}");
    assert!(text.contains("compile-summary"), "{text}");
}

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-native-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("input.ox"), source).unwrap();
        Self(path)
    }
    fn compile(&self, extra: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .env("OXID_LLVM_BIN", self.0.join("absent-toolchain"))
            .args([
                "compile",
                "input.ox",
                "--edition=typed-preview",
                "--backend=llvm",
                "--output=program",
                "--message-format=json",
            ])
            .args(extra)
            .output()
            .unwrap()
    }
    fn assert_untouched(&self) {
        let mut files: Vec<_> = fs::read_dir(&self.0)
            .unwrap()
            .map(|p| p.unwrap().file_name())
            .collect();
        files.sort();
        assert_eq!(files, [std::ffi::OsString::from("input.ox")]);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn rejected(f: &Fixture, code: &str) -> String {
    let out = f.compile(&[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stderr.is_empty());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains(&format!("\"code\":\"{code}\"")), "{text}");
    assert!(
        text.contains("\"kind\":\"compile-summary\",\"success\":false"),
        "{text}"
    );
    f.assert_untouched();
    text
}
#[test]
fn malformed_or_unsupported_source_is_rejected_before_tools_or_files() {
    for (source, code) in [
        (&b"\xff"[..], "E0003"),
        (&b"OXBC\0"[..], "E0004"),
        (&b"fn main() -> i32 { return true; }"[..], "E0300"),
        (&b"fn main() -> i32 { return 2147483648; }"[..], "E0203"),
        (&b"fn main() -> bool { return missing(); }"[..], "E0200"),
        (
            &b"fn main() -> bool { while true {} return true; }"[..],
            "E0101",
        ),
    ] {
        rejected(&Fixture::new(source), code);
    }
}
#[test]
fn entry_and_whole_program_recursion_are_rejected_before_tools() {
    for source in [
        "fn no_main() -> () { return; }",
        "fn main(x: bool) -> bool { return x; }",
        "fn main() -> bool { return main(); }",
        "fn a() -> () { b(); return; } fn b() -> () { a(); return; } fn main() -> bool { return true; }",
        "fn main() -> bool { if false { return main(); } else { return true; } }",
    ] { rejected(&Fixture::new(source.as_bytes()), "E0700"); }
}
#[test]
fn native_limits_preflight_dead_declarations_and_exponential_calls() {
    let mut params = String::from("fn many(");
    params.push_str(
        &(0..65)
            .map(|i| format!("p{i}: bool"))
            .collect::<Vec<_>>()
            .join(","),
    );
    params.push_str(") -> () { return; } fn main() -> () { return; }");
    assert!(rejected(&Fixture::new(params.as_bytes()), "E0700").contains("parameter count"));
    let mut locals = String::from("fn main() -> () {");
    locals.push_str(&"true;".repeat(256));
    locals.push_str("return; }");
    assert!(rejected(&Fixture::new(locals.as_bytes()), "E0700").contains("locals per function"));
    let mut deep = String::from("fn main() -> () { f0(); return; }");
    for i in 0..32 {
        deep.push_str(&format!(
            "fn f{i}() -> () {{ {} return; }}",
            if i == 31 {
                String::new()
            } else {
                format!("f{}();", i + 1)
            }
        ));
    }
    assert!(rejected(&Fixture::new(deep.as_bytes()), "E0700").contains("call depth"));
    let mut exponential = String::from("fn main() -> () { f0(); return; }");
    for i in 0..18 {
        exponential.push_str(&format!(
            "fn f{i}() -> () {{ {} return; }}",
            if i == 17 {
                String::new()
            } else {
                format!("f{0}(); f{0}();", i + 1)
            }
        ));
    }
    assert!(rejected(&Fixture::new(exponential.as_bytes()), "E0700").contains("fuel upper bound"));
}
#[test]
fn unsupported_cli_combinations_have_no_effects() {
    for option in [
        "--target=aarch64-unknown-linux-gnu",
        "--profile=release",
        "--features=avx",
        "--backend=llvm",
        "--output=again",
    ] {
        let f = Fixture::new(b"fn main() -> bool { return true; }");
        let out = f.compile(&[option]);
        assert_eq!(out.status.code(), Some(1));
        assert!(String::from_utf8(out.stdout).unwrap().contains("E0001"));
        f.assert_untouched();
    }
}
#[test]
fn toolchain_errors_leave_no_output_or_scratch() {
    rejected(
        &Fixture::new(b"fn main() -> bool { return true; }"),
        "E0701",
    );
}
#[test]
fn existing_output_is_never_overwritten_even_before_missing_toolchain() {
    let f = Fixture::new(b"fn main() -> bool { return true; }");
    fs::write(f.0.join("program"), "keep me").unwrap();
    let out = f.compile(&[]);
    assert_eq!(out.status.code(), Some(1));
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        assert!(String::from_utf8(out.stdout)
            .unwrap()
            .contains("already exists"));
    }
    assert_eq!(fs::read_to_string(f.0.join("program")).unwrap(), "keep me");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn fake_tools(f: &Fixture, script: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let dir = f.0.join("tools");
    fs::create_dir(&dir).unwrap();
    for tool in ["clang", "opt", "ld.lld"] {
        let path = dir.join(tool);
        fs::write(&path, format!("#!/bin/sh\n{script}")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn compile_with_tools(f: &Fixture, dir: &std::path::Path, mode: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_oxid"))
        .current_dir(&f.0)
        .env("OXID_LLVM_BIN", dir)
        .env("TEST_MODE", mode)
        .env("TEST_ROOT", &f.0)
        .args([
            "compile",
            "input.ox",
            "--edition=typed-preview",
            "--backend=llvm",
            "--output=program",
            "--message-format=json",
        ])
        .output()
        .unwrap()
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn versions_failures_cleanup_and_atomic_no_clobber_are_enforced() {
    let script = r#"
name=${0##*/}
if [ "$1" = --version ]; then
  if [ "$TEST_MODE" = wrong-version ]; then echo 'version 99.0.0'; exit 0; fi
  case "$name" in
    clang) echo 'Debian clang version 19.1.7 (test)' ;;
    opt) echo 'Debian LLVM version 19.1.7' ;;
    ld.lld) echo 'LLD 19.1.7' ;;
  esac
  exit 0
fi
printf '%s\n' "$name" >> "$TEST_ROOT/invoked"
if [ "$TEST_MODE" = verifier-failure ]; then exit 9; fi
while [ "$#" -gt 0 ]; do
  if [ "$1" = -o ]; then shift; output=$1; break; fi
  shift
done
if [ "$output" = executable ]; then
  if [ "$TEST_MODE" = linker-failure ]; then exit 8; fi
  printf '#!/bin/sh\ntouch "%s/executed"\n' "$TEST_ROOT" > "$output"
  chmod 700 "$output"
  if [ "$TEST_MODE" = raced-output ]; then printf 'winner' > "$TEST_ROOT/program"; fi
elif [ -n "$output" ]; then
  touch "$output"
fi
"#;
    for mode in [
        "wrong-version",
        "verifier-failure",
        "linker-failure",
        "raced-output",
        "success",
    ] {
        let f = Fixture::new(b"fn main() -> bool { return true; }");
        let dir = fake_tools(&f, script);
        let result = compile_with_tools(&f, &dir, mode);
        assert_eq!(
            result.status.code(),
            Some(if mode == "success" { 0 } else { 1 }),
            "{mode}: {result:?}"
        );
        assert!(
            !f.0.join("executed").exists(),
            "compile executed its output"
        );
        assert!(!fs::read_dir(&f.0).unwrap().any(|p| p
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".oxid-native-")));
        if mode == "raced-output" {
            assert_eq!(fs::read_to_string(f.0.join("program")).unwrap(), "winner");
        } else if mode != "success" {
            assert!(!f.0.join("program").exists());
        }
        if mode == "wrong-version" {
            assert!(!f.0.join("invoked").exists());
        }
    }
}
#[cfg(unix)]
#[test]
fn existing_symlink_is_never_followed_or_replaced() {
    let f = Fixture::new(b"fn main() -> () { return; }");
    std::os::unix::fs::symlink("missing-target", f.0.join("program")).unwrap();
    let result = f.compile(&[]);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        fs::read_link(f.0.join("program")).unwrap(),
        PathBuf::from("missing-target")
    );
    assert!(!f.0.join("missing-target").exists());
}

#[test]
fn whole_file_native_count_limits_are_inclusive_before_any_tools() {
    for (count, code) in [(256, "E0701"), (257, "E0700")] {
        let mut source = String::from("fn main() -> () { return; }");
        for i in 1..count {
            source.push_str(&format!("fn f{i}() -> () {{ return; }}"));
        }
        rejected(&Fixture::new(source.as_bytes()), code);
    }
    // Exactly 256 local slots per function; 32 functions consume 8192 slots.
    for (count, code) in [(32, "E0701"), (33, "E0700")] {
        let mut source = String::new();
        for i in 0..count {
            let name = if i == 0 {
                "main".into()
            } else {
                format!("f{i}")
            };
            source.push_str(&format!(
                "fn {name}() -> () {{ {} return; }}",
                "true;".repeat(255)
            ));
        }
        rejected(&Fixture::new(source.as_bytes()), code);
    }
    // Each full if/else contributes three blocks; (85*3+1)*16 = 4096.
    for (extra, code) in [(false, "E0701"), (true, "E0700")] {
        let mut source = String::new();
        for i in 0..16 {
            let name = if i == 0 {
                "main".into()
            } else {
                format!("f{i}")
            };
            source.push_str(&format!(
                "fn {name}() -> () {{ {} return; }}",
                "if true {} else {}".repeat(85)
            ));
        }
        if extra {
            source.push_str("fn extra() -> () { return; }");
        }
        rejected(&Fixture::new(source.as_bytes()), code);
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn syntactic_directory_outputs_are_rejected_before_any_tool_or_artifact() {
    for output in [
        "newname/",
        "newname/.",
        "./newname//",
        "newname/./",
        "newname/..",
        "./",
        ".",
        "..",
        "/",
        "/.",
    ] {
        let f = Fixture::new(b"fn main() -> i32 { return 42; }");
        let dir = fake_tools(&f, "touch \"$TEST_ROOT/invoked\"; exit 1\n");
        let result = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&f.0)
            .env("OXID_LLVM_BIN", dir)
            .env("TEST_ROOT", &f.0)
            .args([
                "compile",
                "input.ox",
                "--edition=typed-preview",
                "--backend=llvm",
                "--output",
                output,
                "--message-format=json",
            ])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1), "{output}: {result:?}");
        let text = String::from_utf8(result.stdout).unwrap();
        assert!(text.contains("E0701"), "{output}: {text}");
        assert!(text.contains("must name a new file"), "{output}: {text}");
        assert!(!f.0.join("invoked").exists(), "tools ran for {output}");
        let mut names: Vec<_> = fs::read_dir(&f.0)
            .unwrap()
            .map(|p| p.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                std::ffi::OsString::from("input.ox"),
                std::ffi::OsString::from("tools")
            ],
            "filesystem changed for {output}"
        );
    }
}

#[test]
fn checked_arithmetic_is_admitted_including_dead_code_before_missing_tools() {
    for source in [
        "fn main() -> i32 { return 1 + 2; }",
        "fn main() -> i32 { return 1 - 2; }",
        "fn main() -> i32 { return 2 * 3; }",
        "fn unused() -> i32 { return 1 + 2; } fn main() -> i32 { return 42; }",
        "fn main() -> i32 { if false { return 2147483647 + 1; } return 42; }",
        "fn main() -> i32 { return -2147483648 - 1; }",
    ] {
        let f = Fixture::new(source.as_bytes());
        let checked = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&f.0)
            .args(["check", "input.ox", "--edition=typed-preview"])
            .output()
            .unwrap();
        assert_eq!(
            checked.status.code(),
            Some(0),
            "frontend rejected arithmetic: {checked:?}"
        );
        let native = rejected(&f, "E0701");
        assert!(native.contains("native-toolchain"));
    }
}

#[test]
fn scalar_comparisons_are_admitted_before_missing_tools() {
    for source in [
        "fn main() -> bool { return -2147483648 < 2147483647; }",
        "fn main() -> bool { return -1 <= 0; }",
        "fn main() -> bool { return 1 > -1; }",
        "fn main() -> bool { return 0 >= 0; }",
        "fn main() -> bool { return 1 == 1; }",
        "fn main() -> bool { return true != false; }",
        "fn main() -> bool { return (1 < 2) == true; }",
        "fn unused() -> bool { return 2147483647 + 1 > 0; } fn main() -> bool { return false; }",
    ] {
        rejected(&Fixture::new(source.as_bytes()), "E0701");
    }
}

#[test]
fn invalid_comparison_types_and_chains_fail_before_native_tools() {
    for (expression, code) in [
        ("true == 1", "E0300"),
        ("1 != false", "E0300"),
        ("true < false", "E0300"),
        ("() == ()", "E0300"),
        ("1 < 2 < 3", "E0100"),
        ("1 == 2 < 3", "E0100"),
    ] {
        rejected(
            &Fixture::new(format!("fn main() -> bool {{ return {expression}; }}").as_bytes()),
            code,
        );
    }
}
