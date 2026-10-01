//! Typed-preview fixtures stay embedded so legacy .ox discovery is unchanged.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Project(PathBuf);
impl Project {
    fn new(source: &[u8]) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "oxid-typed-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("input.ox"), source).unwrap();
        Self(dir)
    }
    fn check(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args([
                "check",
                "input.ox",
                "--edition",
                "typed-preview",
                "--message-format",
                "json",
            ])
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env_remove("OXID_PATH")
            .output()
            .unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn failure(source: &str, code: &str, marked: &str) {
    let project = Project::new(source.as_bytes());
    let output = project.check();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(json.contains(&format!("\"code\":\"{code}\"")), "{json}");
    let start = source.rfind(marked).unwrap();
    let end = start + marked.len();
    assert!(
        json.contains(&format!(
            "\"primary\":{{\"file_id\":0,\"path\":\"input.ox\",\"start\":{start},\"end\":{end}"
        )),
        "{json}"
    );
    assert!(json.contains("\"success\":false"), "{json}");
    assert!(!project.0.join("cache").exists());
}
#[test]
fn bool_unit_forward_calls_and_inferred_locals_are_checked_without_execution() {
    let project = Project::new(b"fn main() -> () { let answer: bool = identity(true); identity(answer); return (); }\nfn identity(value: bool) -> bool { let result = (value); return result; }");
    let output = project.check();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(json.contains("\"kind\":\"check-summary\""), "{json}");
    assert!(json.contains("\"success\":true"), "{json}");
    assert!(!json.contains("diagnostic"), "{json}");
    assert!(!project.0.join("cache").exists());
    assert!(!project.0.join("input.oxb").exists());
}
#[test]
fn names_arity_types_and_returns_report_exact_ranges() {
    failure("fn f() -> bool { return missing; }", "E0200", "missing");
    failure("fn f() -> () { missing(); return; }", "E0200", "missing");
    failure("fn f() -> strange { return; }", "E0202", "strange");
    failure("fn f() -> bool { return (); }", "E0300", "()");
}
#[test]
fn type_failures_do_not_accept_legacy_dynamic_programs() {
    failure("fn f(x: bool) -> () { return x; }", "E0300", "x");
    failure(
        "fn f() -> () { let bad: bool = (); return; }",
        "E0300",
        "()",
    );
}

#[test]
fn duplicate_declarations_and_call_contracts_fail() {
    failure(
        "fn f() -> () { return; } fn f() -> () { return; }",
        "E0201",
        "f",
    );
    failure("fn f(x: bool, x: bool) -> () { return; }", "E0201", "x");
    failure(
        "fn f() -> () { let x = true; let x = false; return; }",
        "E0201",
        "x",
    );
    failure(
        "fn f(x: bool) -> () { let x = true; return; }",
        "E0201",
        "x",
    );
    failure(
        "fn f(x: bool) -> () { return; } fn g() -> () { f(); return; }",
        "E0301",
        "f()",
    );
    failure(
        "fn f(x: bool) -> () { return; } fn g() -> () { f(()); return; }",
        "E0300",
        "()",
    );
    failure("fn f() -> bool { return; }", "E0300", "return;");
    failure("fn f() -> () { }", "E0302", "}");
    failure("fn f() -> () { return; true; }", "E0303", "true;");
}
#[test]
fn unsupported_constructs_never_enter_legacy_frontend() {
    for (source, mark) in [
        (
            "fn f() -> () { 9007199254740993; return; }",
            "9007199254740993",
        ),
        ("use \"side.ox\";", "use"),
        ("macro hi { }", "macro"),
        ("fn f(x: &bool) -> () { return; }", "&"),
        ("fn f() -> () { let mut x = true; return; }", "mut"),
        ("fn f() -> () { let x = true; x = false; return; }", "="),
        ("fn f() -> () { while true { return; } return; }", "while"),
        ("async fn f() -> () { return; }", "async"),
        ("fn f() -> () { \"text\"; return; }", "\"text\""),
    ] {
        failure(source, "E0101", mark);
    }
}
#[test]
fn unicode_crlf_and_eof_diagnostics_are_source_based() {
    let source = "// 雪\r\nfn f() -> bool { return absent; }";
    failure(source, "E0200", "absent");
    let output = Project::new(source.as_bytes()).check();
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(json.contains("\"line\":2,\"column\":25"), "{json}");
    failure("fn f() -> () { return;", "E0100", "");
}
#[test]
fn malformed_bytes_and_legacy_artifacts_are_rejected() {
    for (bytes, code) in [(&b"\xff"[..], "E0003"), (&b"OXBC\x01\0\0\0"[..], "E0004")] {
        let out = Project::new(bytes).check();
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stderr.is_empty());
        assert!(String::from_utf8(out.stdout).unwrap().contains(code));
    }
}
#[test]
fn source_token_and_nesting_budgets_fail_as_diagnostics() {
    for source in [
        " ".repeat(1_048_577),
        format!("//{}", "a".repeat(65_536)),
        format!(
            "fn f() -> bool {{ return {}true{}; }}",
            "(".repeat(65),
            ")".repeat(65)
        ),
        format!(
            "fn f() -> bool {{ return true{}; }}",
            " + true".repeat(1000)
        ),
    ] {
        let out = Project::new(source.as_bytes()).check();
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(out.stderr.is_empty(), "{out:?}");
        let json = String::from_utf8(out.stdout).unwrap();
        assert!(json.contains("E0400") || json.contains("E0101"), "{json}");
    }
}

#[test]
fn diagnostic_budget_is_never_exceeded_by_duplicate_bad_signatures() {
    let source = "fn f() -> unknown { return; }\n".repeat(100);
    let out = Project::new(source.as_bytes()).check();
    assert_eq!(out.status.code(), Some(1));
    let json = String::from_utf8(out.stdout).unwrap();
    let errors = json.matches("\"kind\":\"diagnostic\"").count();
    assert!(errors <= 100, "emitted {errors} diagnostics");
}

#[test]
fn unit_bindings_empty_files_and_check_only_recursion_are_supported() {
    for source in [
        "",
        "// no main function required\nfn unit(value: ()) -> () { let local: () = value; return local; }",
        "fn cycle(x: bool) -> bool { return cycle(x); }",
        "fn f() -> ( /* unit */ ) { let local = (()); return local; }",
    ] {
        let project = Project::new(source.as_bytes());
        let output = project.check();
        assert!(output.status.success(), "{output:?}");
        assert!(!project.0.join("cache").exists());
    }
}

#[test]
fn bindings_require_prior_initialization_and_cannot_shadow_functions() {
    failure(
        "fn f() -> () { let local = local; return; }",
        "E0200",
        "local",
    );
    failure("fn f() -> () { let f = true; return; }", "E0201", "f");
    failure("fn f(f: bool) -> () { return; }", "E0201", "f");
}

#[test]
fn source_io_and_lexical_errors_use_the_json_protocol() {
    let project = Project::new(b"/* never closed");
    failure("/* never closed", "E0100", "/* never closed");
    failure("fn f() -> () { \"never closed", "E0100", "\"never closed");
    fs::remove_file(project.0.join("input.ox")).unwrap();
    let output = project.check();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(json.contains("\"code\":\"E0002\""), "{json}");
    assert!(json.contains("\"primary\":null"), "{json}");
}

#[test]
fn nesting_and_parameter_boundaries_are_explicit() {
    let params = (0..256)
        .map(|n| format!("p{n}: bool"))
        .collect::<Vec<_>>()
        .join(",");
    for source in [
        format!(
            "fn f() -> bool {{ return {}true{}; }}",
            "(".repeat(63),
            ")".repeat(63)
        ),
        format!("fn f({params}) -> bool {{ return p0; }}"),
    ] {
        assert!(Project::new(source.as_bytes()).check().status.success());
    }
    for source in [
        format!(
            "fn f() -> bool {{ return {}true{}; }}",
            "(".repeat(64),
            ")".repeat(64)
        ),
        format!("fn f({params}, extra: bool) -> bool {{ return p0; }}"),
        "true ".repeat(50_001),
    ] {
        let output = Project::new(source.as_bytes()).check();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8(output.stdout).unwrap().contains("E0400"));
    }
}

#[test]
fn syntax_recovery_is_deterministic_and_bounded() {
    let project = Project::new("fn f() -> () { let ; }\n".repeat(200).as_bytes());
    let one = project.check();
    let two = project.check();
    assert_eq!(one.status.code(), Some(1));
    assert_eq!(one.stdout, two.stdout);
    let json = String::from_utf8(one.stdout).unwrap();
    assert_eq!(json.matches("\"kind\":\"diagnostic\"").count(), 100);
}

#[test]
fn every_duplicate_function_labels_the_original_declaration() {
    let project = Project::new(b"fn f()->(){return;} fn f()->(){return;} fn f()->(){return;}");
    let output = project.check();
    assert_eq!(output.status.code(), Some(1));
    let json = String::from_utf8(output.stdout).unwrap();
    let errors = json
        .lines()
        .filter(|line| line.contains("\"kind\":\"diagnostic\""))
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 2, "{json}");
    for (line, start) in errors.iter().zip([23, 43]) {
        assert!(line.contains("\"code\":\"E0201\""), "{line}");
        assert!(
            line.contains(&format!(
                "\"primary\":{{\"file_id\":0,\"path\":\"input.ox\",\"start\":{start},\"end\":{}",
                start + 1
            )),
            "{line}"
        );
        assert!(line.contains("\"secondary\":[{\"span\":{\"file_id\":0,\"path\":\"input.ox\",\"start\":3,\"end\":4"),"{line}");
        assert!(line.contains("first declared here"), "{line}");
    }
}

#[test]
fn boolean_branches_scopes_and_all_path_returns_are_check_only() {
    for source in [
        "fn choose(flag: bool, left: bool, right: bool) -> bool { if flag { return left; } else { return right; } }",
        "fn early(flag: bool) -> bool { if flag { return true; } return false; }",
        "fn scopes(flag: bool) -> () { let outer = flag; if outer { let value = true; value; } else { let value = (); value; } let value = (); outer; return value; }",
        "fn nested(flag: bool) -> bool { if flag { if false { return true; } else { return flag; } } else { if true {} else {} } return false; }",
        "fn empty(flag: bool) -> () { if flag {} if flag {} else {} return; }",
        "fn calls(flag: bool) -> bool { if left(flag) { right(flag); } else { left(flag); } return right(flag); } fn left(x: bool) -> bool { return right(x); } fn right(x: bool) -> bool { return left(x); }",
    ] {
        let project = Project::new(source.as_bytes());
        let one = project.check();
        let two = project.check();
        assert!(one.status.success(), "{source}: {one:?}");
        assert!(one.stderr.is_empty(), "{one:?}");
        assert_eq!(one.stdout, two.stdout);
        let json = String::from_utf8(one.stdout).unwrap();
        assert!(json.contains("\"success\":true"), "{json}");
        assert_eq!(fs::read_dir(&project.0).unwrap().count(), 1);
    }
}

#[test]
fn branch_type_scope_and_return_failures_have_exact_ranges() {
    for (source, code, mark) in [
        (
            "fn f() -> () { if () { return; } else { return; } }",
            "E0300",
            "() {",
        ),
        (
            "fn f(flag: bool) -> bool { if flag { return true; } }",
            "E0302",
            "}",
        ),
        (
            "fn f(flag: bool) -> () { if flag { return; } else { if flag { return; } } }",
            "E0302",
            "}",
        ),
        (
            "fn f(flag: bool) -> () { if flag { return; } else { return; } true; }",
            "E0303",
            "true;",
        ),
        (
            "fn f(flag: bool) -> () { if flag { return; false; } return; }",
            "E0303",
            "false;",
        ),
        (
            "fn f(flag: bool) -> () { if flag { let local = true; } local; return; }",
            "E0200",
            "local",
        ),
        (
            "fn f(flag: bool) -> () { if flag { let local = true; } else { local; } return; }",
            "E0200",
            "local",
        ),
        (
            "fn f(flag: bool) -> () { if flag { let flag = true; } return; }",
            "E0201",
            "flag",
        ),
        (
            "fn f(flag: bool) -> () { let outer = true; if flag { let outer = false; } return; }",
            "E0201",
            "outer",
        ),
        (
            "fn f(flag: bool) -> () { if flag { let x = true; let x = false; } return; }",
            "E0201",
            "x",
        ),
        (
            "fn f(flag: bool) -> () { if flag { let f = true; } return; }",
            "E0201",
            "f",
        ),
        (
            "fn f(flag: bool) -> () { if flag { let x = x; } return; }",
            "E0200",
            "x",
        ),
        (
            "fn f() -> () { if true { return; } else { return false; } }",
            "E0300",
            "false",
        ),
    ] {
        // The condition's unit occurrence is followed by a brace; distinguish it
        // from the result type while retaining the exact expression-only range.
        if mark == "() {" {
            let out = Project::new(source.as_bytes()).check();
            assert_eq!(out.status.code(), Some(1));
            let json = String::from_utf8(out.stdout).unwrap();
            let start = source.find("if ()").unwrap() + 3;
            assert!(json.contains("\"code\":\"E0300\""), "{json}");
            assert!(
                json.contains(&format!("\"start\":{start},\"end\":{}", start + 2)),
                "{json}"
            );
        } else {
            failure(source, code, mark);
        }
    }
}

#[test]
fn statement_block_nesting_has_its_own_exact_boundary() {
    // The function body is active frame 1; each then arm adds one frame.
    for (depth, accepted) in [(63, true), (64, false)] {
        let source = format!(
            "fn f() -> () {{ {}{}return; }}",
            "if true {".repeat(depth),
            "}".repeat(depth)
        );
        let out = Project::new(source.as_bytes()).check();
        assert_eq!(out.status.success(), accepted, "{out:?}");
        if !accepted {
            assert!(String::from_utf8(out.stdout).unwrap().contains("E0400"));
        }
    }
}

#[test]
fn malformed_branch_syntax_is_recovered_deterministically() {
    for source in [
        "fn f()->(){ if true return; } fn later()->(){return;}",
        "fn f()->(){ if true {} else return; } fn later()->(){return;}",
        "fn f()->(){ if true {} else if true {} return; } fn later()->(){return;}",
        "fn f()->(){ let x = if true {}; return; } fn later()->(){return;}",
        "fn f()->(){ { return; } } fn later()->(){return;}",
        "fn f()->(){ if true { return;",
        "fn f()->(){ if true {} ; return; }",
    ] {
        let project = Project::new(source.as_bytes());
        let one = project.check();
        let two = project.check();
        assert_eq!(one.status.code(), Some(1), "{one:?}");
        assert!(one.stderr.is_empty());
        assert_eq!(one.stdout, two.stdout);
        let json = String::from_utf8(one.stdout).unwrap();
        assert!(json.contains("E0100") || json.contains("E0101"), "{json}");
        assert!(json.matches("\"kind\":\"diagnostic\"").count() <= 100);
    }
}
