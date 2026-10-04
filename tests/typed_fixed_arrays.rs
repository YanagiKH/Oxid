//! Fixed scalar arrays through the real explicit typed-preview CLI.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Project(PathBuf);
impl Project {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-public-arrays-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("main.ox"), source).unwrap();
        Self(path)
    }
    fn typed(&self, operation: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .env_remove("OXID_PATH")
            .args([operation, "main.ox", "--edition=typed-preview"])
            .output()
            .unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn public_array_source_checks_and_runs() {
    let project = Project::new(
        "fn relay(a: [i32; 2]) -> [i32; 2] { return a; } \
         fn bump(p: &mut [i32; 2]) -> () { p[0] = p[0] + 1; return; } \
         fn main() -> i32 { let mut a = relay([5, 7]); bump(&mut a); \
         return a[0] * 10 + a[1] + a.len(); }",
    );
    let check = project.typed("check");
    assert!(check.status.success(), "{check:?}");
    let run = project.typed("run");
    assert!(run.status.success(), "{run:?}");
    assert_eq!(run.stdout, b"69\n");
    assert!(run.stderr.is_empty());
}

impl Project {
    fn json(&self, operation: &str) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command.current_dir(&self.0).env_remove("OXID_PATH").args([
            operation,
            "main.ox",
            "--edition=typed-preview",
            "--message-format=json",
        ]);
        if operation == "compile" {
            command
                .env("OXID_LLVM_BIN", self.0.join("missing-tools"))
                .args(["--backend=llvm", "--output=program"]);
        }
        command.output().unwrap()
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn compile(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .env_remove("OXID_PATH")
            .args([
                "compile",
                "main.ox",
                "--edition=typed-preview",
                "--backend=llvm",
                "--output=program",
            ])
            .output()
            .unwrap()
    }
}

fn success_cases() -> Vec<(&'static str, &'static str)> {
    vec![
        ("fn relay(a:[i32;2])->[i32;2]{return a;} fn main()->i32{let a=[4,9];let mut b=relay(a);b[1]=b[0]+b[1];return b[1]+b.len();}", "15\n"),
        ("fn main()->bool{let mut a=[false,true];a[0]=a[1];return a[0];}", "true\n"),
        ("fn main()->(){let a=[(),()];return a[1];}", "()\n"),
        ("fn main()->i32{let a:[i32;0000]=(([]));let b=a;return b.len();}", "0\n"),
        ("fn main()->i32{let a:[bool;0]=[];let b:[();0]=[];return a.len()+b.len();}", "0\n"),
        ("fn main()->i32{let mut a=[1,2];a=[7,8];a=a;return a[0]+a[1];}", "15\n"),
        ("fn index(a:&mut [i32;1])->i32{a[0]=99;return 0;} fn main()->i32{let mut a=[7];a[index(&mut a)]=a[0];return a[0];}", "7\n"),
        ("fn next(a:&mut [i32;1])->i32{a[0]=a[0]+1;return a[0];} fn main()->i32{let mut state=[0];let a=[next(&mut state),next(&mut state)];return a[0]*10+a[1];}", "12\n"),
        ("fn tick(a:&mut [i32;1])->(){a[0]=a[0]+1;return;} fn main()->i32{let mut state=[0];let a=[tick(&mut state),tick(&mut state)];a[1];return state[0]+a.len();}", "4\n"),
        ("fn inner(a:&mut [i32;1])->(){a[0]=a[0]+3;return;} fn outer(a:&mut [i32;1])->(){inner(&mut *a);return;} fn main()->i32{let mut a=[5];outer(&mut a);return a[0];}", "8\n"),
        ("fn main()->i32{let a=[-1,2,3];let mut i=0;let mut total=0;while i<a.len(){let n=a[i];i=i+1;if n<0{continue;}total=total+n;}return total;}", "5\n"),
        ("fn main()->bool{let a:[bool;0]=[];return false&&a[0]||true;}", "true\n"),
        ("struct Record{len:i32} fn main()->i32{let r=Record{len:8};let a=[1,2];return r.len+a.len();}", "10\n"),
    ]
}

#[test]
fn public_array_scalar_kinds_moves_borrows_and_effect_order() {
    for (source, expected) in success_cases() {
        let project = Project::new(source);
        let check = project.typed("check");
        assert!(check.status.success(), "{source}: {check:?}");
        let run = project.typed("run");
        assert!(run.status.success(), "{source}: {run:?}");
        assert_eq!(run.stdout, expected.as_bytes(), "{source}");
        assert!(run.stderr.is_empty(), "{source}");
    }
}

#[test]
fn public_array_rejections_keep_stage_and_source_origin() {
    for (source, code, stage, origin) in [
        ("fn main()->(){let a=[];return;}", "E0300", "type", "[]"),
        ("fn main()->[i32;0]{return [];}", "E0300", "type", "[]"),
        ("fn f(a:[i32;0])->(){return;} fn main()->(){f([]);return;}", "E0300", "type", "[]"),
        ("fn main()->(){let mut a:[i32;0]=[];a=[];return;}", "E0300", "type", "[]"),
        ("fn main()->(){let a:[i32;1]=[];return;}", "E0300", "type", "[]"),
        ("fn main()->(){let a=[1,true];return;}", "E0300", "type", "true"),
        ("fn main()->(){let a=[[1]];return;}", "E0300", "type", "[1]"),
        ("fn main()->(){let a=[1];a[0]=2;return;}", "E0304", "type", "a[0]"),
        ("fn main()->i32{let a=[1];return a[false];}", "E0300", "type", "false"),
        ("fn main()->i32{let a=1;return a[0];}", "E0305", "type", "a[0]"),
        ("fn main()->i32{let a=1;return a.len();}", "E0305", "type", "a.len()"),
        ("fn main()->i32{let a=[1];let b=a;return a.len();}", "E0310", "ownership", "a.len()"),
        ("fn main()->i32{let a:[i32;0]=[];let b=a;return a.len();}", "E0310", "ownership", "a.len()"),
        ("fn main()->(){let a=[()];let b=a;return a[0];}", "E0310", "ownership", "a[0]"),
        ("fn consume(a:[i32;1])->i32{return 0;} fn main()->i32{let a=[1];return a[consume(a)];}", "E0310", "ownership", "a[consume(a)]"),
        ("fn f(a:&mut [i32;1],n:i32)->(){return;} fn main()->(){let mut a=[1];f(&mut a,a.len());return;}", "E0311", "ownership", "a.len()"),
        ("fn f(a:&[i32;1])->(){a[0]=2;return;}", "E0313", "ownership", "a[0]"),
        ("fn f(a:[i32;2])->(){return;} fn main()->(){f([1]);return;}", "E0300", "type", "[1]"),
        ("fn f(a:[bool;0])->(){return;} fn main()->(){let a:[i32;0]=[];f(a);return;}", "E0300", "type", "a"),
    ] {
        let project = Project::new(source);
        for operation in ["check", "run", "compile"] {
            let output = project.json(operation);
            assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
            assert!(output.stderr.is_empty(), "{source}: {output:?}");
            let json = String::from_utf8(output.stdout).unwrap();
            assert!(json.contains(&format!("\"code\":\"{code}\"")), "{source}: {json}");
            assert!(json.contains(&format!("\"stage\":\"{stage}\"")), "{source}: {json}");
            let start = source.rfind(origin).unwrap();
            assert!(json.contains(&format!("\"start\":{start},\"end\":{}", start + origin.len())), "{source}: {json}");
        }
    }
}

#[test]
fn public_array_excluded_grammar_and_length_caps() {
    for (source, code) in [
        ("fn f(a:[[i32;1];1])->(){}", "E0101"),
        ("fn f(a:[&i32;1])->(){}", "E0101"),
        ("fn f(a:[i32;-1])->(){}", "E0101"),
        ("fn f(a:[i32;1+1])->(){}", "E0101"),
        ("fn f(a:[i32;1_0])->(){}", "E0101"),
        ("fn f(a:[i32;1025])->(){}", "E0400"),
        (
            "fn f(a:[i32;999999999999999999999999999999])->(){}",
            "E0400",
        ),
        ("fn main()->(){let a=[1;2];return;}", "E0101"),
        ("fn main()->(){let mut a=[1];(a[0])=2;return;}", "E0101"),
        ("fn main()->i32{let a=[1];return (a)[0];}", "E0101"),
        ("fn main()->i32{return [1][0];}", "E0101"),
        ("fn main()->i32{let a=[1];return a.len(1);}", "E0101"),
        ("fn main()->i32{let a=[1];return a.other();}", "E0101"),
        ("fn main()->(){let a=[1 2];return;}", "E0100"),
    ] {
        let output = Project::new(source).json("check");
        let json = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output.status.code(), Some(1), "{source}: {json}");
        assert!(
            json.contains(&format!("\"code\":\"{code}\"")),
            "{source}: {json}"
        );
        assert!(json.contains("\"stage\":\"parse\""), "{source}: {json}");
    }
    for length in [257, 1024, 1025] {
        let source = format!(
            "fn main()->i32{{let a=[{}];return a.len();}}",
            "1,".repeat(length)
        );
        let project = Project::new(&source);
        let output = project.typed("run");
        if length <= 1024 {
            assert!(output.status.success(), "{length}: {output:?}");
            assert_eq!(output.stdout, format!("{length}\n").as_bytes());
        } else {
            assert_eq!(output.status.code(), Some(1));
            assert!(String::from_utf8(output.stderr).unwrap().contains("E0400"));
        }
    }
}

#[test]
fn public_array_main_may_check_without_being_executable() {
    for source in [
        "fn main()->[i32;1]{return [1];}",
        "fn main(a:[i32;1])->i32{return a[0];}",
        "fn helper()->[i32;1]{return [1];}",
    ] {
        let project = Project::new(source);
        assert!(project.typed("check").status.success());
        for (operation, code) in [("run", "E0600"), ("compile", "E0700")] {
            let output = project.json(operation);
            let text = String::from_utf8(output.stdout).unwrap();
            assert_eq!(output.status.code(), Some(1), "{text}");
            assert!(text.contains(code), "{text}");
            assert!(!text.contains("E0701"), "{text}");
        }
    }
}

fn failures() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    vec![
        (
            "fn main()->i32{let a=[7];return a[-1];}",
            "E0606",
            "oir-owned-run",
            "a[-1]",
        ),
        (
            "fn main()->i32{let a=[7];return a[1];}",
            "E0606",
            "oir-owned-run",
            "a[1]",
        ),
        (
            "fn main()->(){let a:[();0]=[];return a[0];}",
            "E0606",
            "oir-owned-run",
            "a[0]",
        ),
        (
            "fn main()->(){let mut a=[7];a[2]=1;return;}",
            "E0606",
            "oir-owned-run",
            "a[2]",
        ),
        (
            "fn main()->i32{let a=[7];return a[2147483647+1];}",
            "E0604",
            "oir-run",
            "+",
        ),
        (
            "fn main()->(){let mut a=[7];a[2]=2147483647+1;return;}",
            "E0604",
            "oir-run",
            "+",
        ),
        (
            "fn main()->(){let mut a=[7];a[2147483647+1]=a[2];return;}",
            "E0606",
            "oir-owned-run",
            "a[2]",
        ),
        (
            "// 雪\nfn main()->i32{let a=[7];return a[1];}",
            "E0606",
            "oir-owned-run",
            "a[1]",
        ),
    ]
}
fn runtime_message(source: &str, code: &str, stage: &str, origin: &str) -> String {
    let start = source.rfind(origin).unwrap();
    let before = &source[..start];
    let line = before.bytes().filter(|&c| c == b'\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap().chars().count() + 1;
    let message = if code == "E0606" {
        "array index out of bounds"
    } else {
        "checked i32 arithmetic overflow"
    };
    format!("error[{code}] ({stage}): {message}\n  --> main.ox:{line}:{column}\n")
}
#[test]
fn public_array_runtime_errors_keep_exact_messages_and_rhs_precedence() {
    for (source, code, stage, origin) in failures() {
        let project = Project::new(source);
        assert!(project.typed("check").status.success());
        let output = project.typed("run");
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            runtime_message(source, code, stage, origin).as_bytes()
        );
    }
}

#[test]
fn legacy_dynamic_arrays_keep_default_and_explicit_behavior() {
    let project = Project::new("let a=[1,2]; a[0]=7; print a[0];");
    for options in [vec![], vec!["--edition=legacy-0.9"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&project.0)
            .args(["run", "main.ox"])
            .args(options)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stdout, b"7\n");
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires LLVM/Clang/LLD 19.1.7; run the public fixed-array native gate explicitly"]
fn public_array_cli_compiles_source_free_native_parity() {
    for (source, expected) in success_cases() {
        let project = Project::new(source);
        let reference = project.typed("run");
        assert!(reference.status.success(), "{source}: {reference:?}");
        assert_eq!(reference.stdout, expected.as_bytes());
        let compile = project.compile();
        assert!(compile.status.success(), "{source}: {compile:?}");
        assert_eq!(
            &fs::read(project.0.join("program")).unwrap()[..4],
            b"\x7fELF"
        );
        fs::remove_file(project.0.join("main.ox")).unwrap();
        let native = Command::new(project.0.join("program"))
            .current_dir(&project.0)
            .env_clear()
            .output()
            .unwrap();
        assert!(native.status.success(), "{source}: {native:?}");
        assert_eq!(native.stdout, expected.as_bytes());
        assert!(native.stderr.is_empty());
    }
    for (source, code, stage, origin) in failures() {
        let project = Project::new(source);
        let compile = project.compile();
        assert!(compile.status.success(), "{source}: {compile:?}");
        fs::remove_file(project.0.join("main.ox")).unwrap();
        let native = Command::new(project.0.join("program"))
            .current_dir(&project.0)
            .env_clear()
            .output()
            .unwrap();
        assert_eq!(native.status.code(), Some(1));
        assert!(native.stdout.is_empty());
        assert_eq!(
            native.stderr,
            runtime_message(source, code, stage, origin).as_bytes()
        );
    }
}

#[cfg(target_os = "linux")]
fn samples_project() -> Project {
    let main = include_str!("../fixtures/typed-array-samples/main.ox");
    // Check the full logical sequence independently as well as the checksum,
    // count and sum; a colliding weighted checksum cannot hide a wrong store.
    let main = main.replace("    return crate::stats::count", "    if !(values[0] == 5 && values[1] == 7 && values[2] == 0 && values[3] == 9 && values[4] == 4 && values[5] == 0 && values[6] == 0 && values[7] == 0 && crate::stats::count(&result) == 5 && crate::stats::sum(&result) == 25) { return -1; }\n    return crate::stats::count");
    let project = Project::new(&main);
    fs::write(
        project.0.join("stats.ox"),
        include_str!("../fixtures/typed-array-samples/stats.ox"),
    )
    .unwrap();
    fs::write(
        project.0.join("samples.ox"),
        include_str!("../fixtures/typed-array-samples/samples.ox"),
    )
    .unwrap();
    project
}

#[cfg(target_os = "linux")]
#[test]
fn public_array_three_module_pilot_preserves_complete_sequence() {
    let project = samples_project();
    let check = project.typed("check");
    assert!(check.status.success(), "{check:?}");
    let output = project.typed("run");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"5325\n");
    assert!(output.stderr.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn public_array_unused_child_and_child_origins_select_one_checked_route() {
    let project = Project::new("mod child; fn main()->i32{return 3;}");
    fs::write(
        project.0.join("child.ox"),
        "fn unused(a:[i32;0])->i32{return a.len();}",
    )
    .unwrap();
    assert!(project.typed("check").status.success());
    assert_eq!(project.typed("run").stdout, b"3\n");
    let child = "// 雪\nfn unused()->i32{let a=[1];let b=a;return a[0];}";
    fs::write(project.0.join("child.ox"), child).unwrap();
    for operation in ["check", "run", "compile"] {
        let output = project.json(operation);
        let json = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output.status.code(), Some(1), "{json}");
        assert!(json.contains("\"code\":\"E0310\""), "{json}");
        assert!(json.contains("child.ox"), "{json}");
        let start = child.find("a[0]").unwrap();
        assert!(
            json.contains(&format!("\"start\":{start},\"end\":{}", start + 4)),
            "{json}"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn public_array_cross_module_identity_and_entry_rules() {
    let project =
        Project::new("mod child; fn main()->i32{let a:[i32;2]=crate::child::make();return a[1];}");
    fs::write(
        project.0.join("child.ox"),
        "pub fn make()->[i32;0002]{return [3,7];} pub fn main()->i32{return 99;}",
    )
    .unwrap();
    assert_eq!(project.typed("run").stdout, b"7\n");
    fs::write(
        project.0.join("main.ox"),
        "mod child; fn main()->i32{let a:[i32;1]=crate::child::make();return a[0];}",
    )
    .unwrap();
    let output = project.json("check");
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stdout).unwrap().contains("E0300"));
    fs::write(
        project.0.join("main.ox"),
        "mod child; use crate::child::main;",
    )
    .unwrap();
    assert!(project.typed("check").status.success());
    for (operation, code) in [("run", "E0600"), ("compile", "E0700")] {
        let output = project.json(operation);
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8(output.stdout).unwrap().contains(code));
    }
}

#[test]
fn public_array_loop_remains_fuel_bounded() {
    let project = Project::new("fn main()->i32{let a=[1];while true{a[0];}return 0;}");
    let output = project.json("run");
    assert_eq!(output.status.code(), Some(1));
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(json.contains("\"code\":\"E0601\""), "{json}");
    assert!(json.contains("\"stage\":\"oir-run\""), "{json}");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires LLVM/Clang/LLD 19.1.7; run the public fixed-array native gate explicitly"]
fn public_array_cli_native_module_and_fuel_parity() {
    for (project, expected) in [
        (samples_project(), Some(b"5325\n".as_slice())),
        (
            Project::new("fn main()->i32{let a=[1];while true{a[0];}return 0;}"),
            None,
        ),
    ] {
        let reference = project.typed("run");
        match expected {
            Some(value) => {
                assert!(reference.status.success(), "{reference:?}");
                assert_eq!(reference.stdout, value);
            }
            None => {
                assert_eq!(reference.status.code(), Some(1));
                assert!(String::from_utf8_lossy(&reference.stderr).contains("E0601"));
            }
        }
        let compile = project.compile();
        assert!(compile.status.success(), "{compile:?}");
        for name in ["main.ox", "stats.ox", "samples.ox"] {
            let path = project.0.join(name);
            if path.exists() {
                fs::remove_file(path).unwrap();
            }
        }
        let native = Command::new(project.0.join("program"))
            .current_dir(&project.0)
            .env_clear()
            .output()
            .unwrap();
        assert_eq!(native.status.code(), reference.status.code());
        assert_eq!(native.stdout, reference.stdout);
        assert_eq!(native.stderr, reference.stderr);
    }
}

#[test]
fn array_activation_preserves_non_array_assignment_diagnostics() {
    for source in [
        "fn main()->(){let mut x=1;(x)=2;return;}",
        "fn f()->i32{return 1;} fn main()->(){f()=2;return;}",
        "fn main()->(){1=2;return;}",
        "struct T{x:i32} fn main()->(){let mut r=T{x:1};(r.x)=2;return;}",
    ] {
        let project = Project::new(source);
        for operation in ["check", "run", "compile"] {
            let output = project.json(operation);
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            let json = String::from_utf8(output.stdout).unwrap();
            assert!(json.contains("\"code\":\"E0100\""), "{json}");
            assert!(json.contains("statement requires `;`"), "{json}");
            let start = source.rfind('=').unwrap();
            assert!(
                json.contains(&format!("\"start\":{start},\"end\":{}", start + 1)),
                "{json}"
            );
        }
    }
}
