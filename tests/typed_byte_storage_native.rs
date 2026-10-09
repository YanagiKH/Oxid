//! RFC0031 public native admission and artifact boundaries. Missing tools and
//! persistent output sentinels are controls, never alternate native executors.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

const SOURCE: &str = "fn byte(x:i32)->u8{return x.to_u8_checked();}fn read(a:&[u8])->i32{let b=a[0];return b.to_i32();}fn main()->i32{let a=[byte(255)];return read(&a);}";
const SENTINEL: &[u8] = b"byte storage output sentinel\x00\xff\n";
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Case(PathBuf);
impl Case {
    fn new(source: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "oxid-byte-storage-native-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("main.ox"), source).unwrap();
        fs::write(dir.join("existing"), SENTINEL).unwrap();
        Self(dir)
    }
    fn compile(&self, output: &str, extra: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .env_remove("OXID_PATH")
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env("OXID_LLVM_BIN", self.0.join("missing-tools"))
            .args([
                "compile",
                "main.ox",
                "--edition=typed-preview",
                "--backend=llvm",
                "--message-format=json",
                "--output",
                output,
            ])
            .args(extra)
            .output()
            .unwrap()
    }
    fn unchanged(&self) {
        assert_eq!(fs::read(self.0.join("existing")).unwrap(), SENTINEL);
        let mut names = fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, ["existing", "main.ox"]);
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn diagnostic(output: Output, code: &str, stage: &str) -> String {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains(&format!("\"code\":\"{code}\"")), "{text}");
    assert!(text.contains(&format!("\"stage\":\"{stage}\"")), "{text}");
    assert!(text.contains("\"output\":null"), "{text}");
    text
}

#[test]
fn byte_storage_native_public_no_clobber_missing_tools_and_host_gate() {
    let case = Case::new(SOURCE);
    for output in ["existing", "fresh"] {
        let text = diagnostic(case.compile(output, &[]), "E0701", "native-toolchain");
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            if output == "existing" {
                assert!(text.contains("native output already exists"), "{text}");
            } else {
                assert!(text.contains("cannot launch "), "{text}");
                assert!(text.contains("missing-tools/clang"), "{text}");
            }
        } else {
            assert!(
                text.contains("native preview requires a Linux x86_64 build host"),
                "{text}"
            );
        }
        case.unchanged();
    }
}

#[test]
fn byte_storage_native_public_entry_and_capacity_refuse_before_tool_and_output() {
    let elements = vec!["b"; 1024].join(",");
    for (source, marker) in [
        ("fn main()->[u8;0]{let a:[u8;0]=[];return a;}".to_string(), "native main must return a scalar"),
        (format!("fn main()->i32{{let n=255;let b=n.to_u8_checked();let a=[{elements}];return a.len();}}"), "scalar slots per function"),
    ] {
        let case = Case::new(&source);
        for output in ["existing", "fresh"] {
            let text = diagnostic(case.compile(output, &[]), "E0700", "native-admission");
            assert!(text.contains(marker), "{text}");
            assert!(!text.contains("E0701"), "{text}");
            case.unchanged();
        }
    }
}

#[test]
fn byte_storage_native_public_unsupported_target_has_no_artifact_effects() {
    let case = Case::new(SOURCE);
    let output = case.compile("fresh", &["--target=aarch64-unknown-linux-gnu"]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("E0001"), "{text}");
    case.unchanged();
}

#[test]
fn byte_storage_public_excluded_spellings_fail_before_native_tools() {
    // Exact baseline b5455ad diagnostics were recorded before these successor
    // cases ran. Array element acceptance does not widen any of these grammars.
    for (source, code, message, selected) in [
        (
            "enum E{V([u8;0])}fn main()->i32{return 0;}",
            "E0100",
            "only bool, i32 and () enum payloads are supported",
            "[",
        ),
        (
            "fn main()->i32{let a=[1u8];return 0;}",
            "E0101",
            "unsupported numeric spelling; expected ASCII decimal literal digits",
            "1u8",
        ),
        (
            "fn f(b:u8)->i32{let a=[b;2];return 0;}",
            "E0101",
            "unsupported typed-preview construct `;`",
            ";",
        ),
    ] {
        let case = Case::new(source);
        let start = source.find(selected).unwrap();
        let end = start + selected.len();
        let expected = format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"{code}\",\"stage\":\"parse\",\"message\":\"{message}\",\"primary\":{{\"file_id\":0,\"path\":\"main.ox\",\"start\":{start},\"end\":{end},\"line\":1,\"column\":{},\"end_line\":1,\"end_column\":{}}},\"secondary\":[],\"notes\":[]}}", start+1, end+1);
        for output in ["existing", "fresh"] {
            let text = diagnostic(case.compile(output, &[]), code, "parse");
            assert_eq!(text.lines().count(), 2);
            assert_eq!(text.lines().next(), Some(expected.as_str()));
            case.unchanged();
        }
    }
}

#[test]
fn byte_storage_public_i32_stdout_signature_stays_closed() {
    // b5455ad's &[bool] analogue fixes the diagnostic template, argument
    // selection and imported declaration secondary. Only the array leaf type
    // changes here; the production builtin remains the existing &[i32] API.
    let source = "use std::io::write_stdout;fn f(a:&[u8])->i32{write_stdout(&*a);return 0;}";
    let case = Case::new(source);
    let start = source.find("&*a").unwrap();
    let end = start + 3;
    let expected = format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0300\",\"stage\":\"type\",\"message\":\"call argument type or borrow mode does not match parameter\",\"primary\":{{\"file_id\":0,\"path\":\"main.ox\",\"start\":{start},\"end\":{end},\"line\":1,\"column\":{},\"end_line\":1,\"end_column\":{}}},\"secondary\":[{{\"span\":{{\"file_id\":0,\"path\":\"main.ox\",\"start\":13,\"end\":25,\"line\":1,\"column\":14,\"end_line\":1,\"end_column\":26}},\"message\":\"function declared here\"}}],\"notes\":[]}}", start+1, end+1);
    for output in ["existing", "fresh"] {
        let text = diagnostic(case.compile(output, &[]), "E0300", "type");
        assert_eq!(text.lines().count(), 2);
        assert_eq!(text.lines().next(), Some(expected.as_str()));
        case.unchanged();
    }
}

fn source_command(case: &Case, operation: &str, json: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
    command
        .current_dir(&case.0)
        .env_remove("OXID_PATH")
        .env("OXID_CACHE_DIR", case.0.join("cache"))
        .env("OXID_LLVM_BIN", case.0.join("missing-tools"))
        .args([operation, "main.ox", "--edition=typed-preview"]);
    if json {
        command.arg("--message-format=json");
    }
    command.output().unwrap()
}

#[test]
fn byte_storage_public_array_main_checks_but_is_not_a_run_entry() {
    // The immutable [i32;0] analogue checks successfully but refuses execution
    // with this exact E0600 template and main-name span.
    let case = Case::new("fn main()->[u8;0]{let a:[u8;0]=[];return a;}");
    let checked = source_command(&case, "check", true);
    assert_eq!(checked.status.code(), Some(0));
    assert!(checked.stderr.is_empty());
    assert_eq!(checked.stdout, b"{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":true,\"errors\":0,\"functions\":1}\n");
    let ran = source_command(&case, "run", true);
    assert_eq!(ran.status.code(), Some(1));
    assert!(ran.stderr.is_empty());
    let text = String::from_utf8(ran.stdout).unwrap();
    assert_eq!(text.lines().count(), 2);
    assert_eq!(text.lines().next(), Some("{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0600\",\"stage\":\"oir-run\",\"message\":\"typed-preview main must return bool, i32 or ()\",\"primary\":{\"file_id\":0,\"path\":\"main.ox\",\"start\":3,\"end\":7,\"line\":1,\"column\":4,\"end_line\":1,\"end_column\":8},\"secondary\":[],\"notes\":[]}"));
    assert!(
        text.contains("\"kind\":\"run-summary\",\"success\":false,\"errors\":1,\"result\":null")
    );
    case.unchanged();
}

#[test]
fn byte_storage_public_identical_module_text_keeps_runtime_file_identity() {
    // Identical bytes and function names in two retained files cannot exchange
    // runtime origins. The predecessor slice bounds template fixes rendering.
    let child = "pub fn get(p:&[u8])->i32{let b=p[1];return b.to_i32();}";
    let column = child.find("p[1]").unwrap() + 1;
    for chosen in ["left", "right"] {
        let root = format!("mod left;mod right;fn main()->i32{{let n=128;let b=n.to_u8_checked();let a=[b];return crate::{chosen}::get(&a);}}");
        let case = Case::new(&root);
        for name in ["left.ox", "right.ox"] {
            fs::write(case.0.join(name), child).unwrap();
        }
        let checked = source_command(&case, "check", true);
        assert_eq!(checked.status.code(), Some(0), "{checked:?}");
        assert!(checked.stderr.is_empty());
        let ran = source_command(&case, "run", false);
        assert_eq!(ran.status.code(), Some(1), "{ran:?}");
        assert!(ran.stdout.is_empty());
        assert_eq!(ran.stderr, format!("error[E0606] (oir-owned-run): array index out of bounds\n  --> {chosen}.ox:1:{column}\n").as_bytes());
        assert_eq!(fs::read(case.0.join("existing")).unwrap(), SENTINEL);
        assert_eq!(fs::read_to_string(case.0.join("main.ox")).unwrap(), root);
        for name in ["left.ox", "right.ox"] {
            assert_eq!(fs::read_to_string(case.0.join(name)).unwrap(), child);
        }
    }
}

#[test]
fn byte_storage_public_slice_value_and_element_reference_grammar_stays_closed() {
    // All five exact diagnostic templates and selected token ranges were
    // independently replayed on b5455ad bool-slice analogues before execution.
    for (source, marker, offset, length, message) in [
        (
            "fn f(p:&[u8])->&[u8]{return p;}",
            ")->&",
            3,
            1,
            "unsupported typed-preview construct `&`",
        ),
        (
            "fn f(p:&[u8])->i32{let q:&[u8]=p;return 0;}",
            "q:&",
            2,
            1,
            "unsupported typed-preview construct `&`",
        ),
        (
            "struct R{p:&[u8]}fn main()->i32{return 0;}",
            "p:&",
            2,
            1,
            "unsupported typed-preview construct `&`",
        ),
        (
            "fn f(p:&[u8])->i32{let q=p[0..1];return 0;}",
            "0..1",
            0,
            4,
            "unsupported numeric spelling; expected ASCII decimal literal digits",
        ),
        (
            "fn f(p:&[u8])->i32{return g(&p[0]);}fn g(p:&[u8])->i32{return 0;}",
            "p[0]",
            1,
            1,
            "unsupported typed-preview construct `[`",
        ),
    ] {
        let case = Case::new(source);
        let start = source.find(marker).unwrap() + offset;
        let end = start + length;
        let expected = format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0101\",\"stage\":\"parse\",\"message\":\"{message}\",\"primary\":{{\"file_id\":0,\"path\":\"main.ox\",\"start\":{start},\"end\":{end},\"line\":1,\"column\":{},\"end_line\":1,\"end_column\":{}}},\"secondary\":[],\"notes\":[]}}", start+1, end+1);
        for output in ["existing", "fresh"] {
            let text = diagnostic(case.compile(output, &[]), "E0101", "parse");
            assert_eq!(text.lines().count(), 2);
            assert_eq!(text.lines().next(), Some(expected.as_str()));
            case.unchanged();
        }
    }
}

#[test]
fn byte_storage_public_record_reference_projection_stops_at_declaration() {
    // The sealed direct/unused-record oracle fixes E0202 at the full field type.
    // A whole-record reference and unused projection cannot get past that gate.
    for n in [0, 1, 1024] {
        let ty = format!("[u8;{n}]");
        let source = format!(
            "struct R{{a:{ty}}}fn f(r:&R)->i32{{return r.a.len();}}fn main()->i32{{return 0;}}"
        );
        let case = Case::new(&source);
        let start = source.find(&ty).unwrap();
        let end = start + ty.len();
        let expected = format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0202\",\"stage\":\"resolve\",\"message\":\"u8 array record fields are not supported\",\"primary\":{{\"file_id\":0,\"path\":\"main.ox\",\"start\":{start},\"end\":{end},\"line\":1,\"column\":{},\"end_line\":1,\"end_column\":{}}},\"secondary\":[],\"notes\":[]}}", start+1, end+1);
        for output in ["existing", "fresh"] {
            let text = diagnostic(case.compile(output, &[]), "E0202", "resolve");
            assert_eq!(text.lines().count(), 2);
            assert_eq!(text.lines().next(), Some(expected.as_str()));
            assert!(!text.contains("E0701"));
            case.unchanged();
        }
    }
}

#[test]
fn byte_storage_public_unused_nested_enum_array_payload_stops_in_inner_parser() {
    // enum_payload_type is byte-identical to b5455ad: '[' is refused before
    // reading any leaf/length. DFS retains root0, outer1, inner2; no name is used.
    let root = "mod outer;fn main()->i32{return 0;}";
    let outer = "mod inner;";
    for n in [0, 1, 1024] {
        let inner = format!("enum E{{V([u8;{n}])}}");
        let case = Case::new(root);
        fs::write(case.0.join("outer.ox"), outer).unwrap();
        fs::create_dir(case.0.join("outer")).unwrap();
        fs::write(case.0.join("outer/inner.ox"), &inner).unwrap();
        let start = inner.find('[').unwrap();
        let end = start + 1;
        let expected = format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0100\",\"stage\":\"parse\",\"message\":\"only bool, i32 and () enum payloads are supported\",\"primary\":{{\"file_id\":2,\"path\":\"outer/inner.ox\",\"start\":{start},\"end\":{end},\"line\":1,\"column\":{},\"end_line\":1,\"end_column\":{}}},\"secondary\":[],\"notes\":[]}}", start+1, end+1);
        for output in ["existing", "fresh"] {
            let text = diagnostic(case.compile(output, &[]), "E0100", "parse");
            assert_eq!(text.lines().count(), 2);
            assert_eq!(text.lines().next(), Some(expected.as_str()));
            assert!(!text.contains("E0701"));
            assert_eq!(fs::read(case.0.join("existing")).unwrap(), SENTINEL);
            for (path, expected) in [
                ("main.ox", root),
                ("outer.ox", outer),
                ("outer/inner.ox", &inner),
            ] {
                assert_eq!(fs::read_to_string(case.0.join(path)).unwrap(), expected);
            }
            let mut paths = fs::read_dir(&case.0)
                .unwrap()
                .map(|p| p.unwrap().file_name())
                .collect::<Vec<_>>();
            paths.sort();
            assert_eq!(paths, ["existing", "main.ox", "outer", "outer.ox"]);
            assert_eq!(fs::read_dir(case.0.join("outer")).unwrap().count(), 1);
        }
    }
}
