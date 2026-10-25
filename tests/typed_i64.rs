//! Full-width values, strict typing and diagnostics through the public CLI.
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new(source: &str, policy: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-i64-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("oxid.toml"), format!("[project]\nname=\"integer\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        Self(root)
    }
    fn invoke(&self, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command.args(args).current_dir(&self.0);
        bounded_output(&mut command)
    }
    fn result(&self, expected: &str) {
        let output = self.invoke(&["run"]);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        assert!(output.stderr.is_empty(), "{output:?}");
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn bounded_output(command: &mut Command) -> Output {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let drain = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).unwrap();
            bytes
        })
    };
    let stdout = drain(Box::new(child.stdout.take().unwrap()));
    let stderr = drain(Box::new(child.stderr.take().unwrap()));
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return Output {
                status,
                stdout: stdout.join().unwrap(),
                stderr: stderr.join().unwrap(),
            };
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("i64 command exceeded 30 seconds");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn records(output: Output, status: i32) -> Vec<String> {
    assert_eq!(output.status.code(), Some(status), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| String::from_utf8(line.to_vec()).unwrap())
        .collect()
}

#[test]
fn full_width_literals_copies_and_json_are_exact_decimal_strings() {
    for (literal, value) in [
        ("9223372036854775807i64", "9223372036854775807"),
        ("-9223372036854775808i64", "-9223372036854775808"),
        ("9007199254740993i64", "9007199254740993"),
        ("-9007199254740993i64", "-9007199254740993"),
        ("000000000000000000000001i64", "1"),
        ("- /* 雪 */ 000000000000000000000000i64", "0"),
    ] {
        let project = Project::new(&format!("fn copy(value:i64)->i64{{return value;}}fn main()->i64{{let value={literal};let copied:i64=value;return copy(copied);}}"), "bounded");
        project.result(value);
        let rows = records(project.invoke(&["run", "--message-format=json"]), 0);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0], format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":true,\"errors\":0,\"result\":{{\"type\":\"i64\",\"value\":\"{value}\"}}}}"));
    }
}

#[test]
fn type_inference_stays_strict_and_literal_ranges_are_checked_before_execution() {
    for (source, code) in [
        ("fn main()->i64{return 1;}", "E0300"),
        ("fn main()->i32{return 1i64;}", "E0300"),
        ("fn main()->i64{return 1i64+1;}", "E0300"),
        ("fn main()->bool{return 1i64==1;}", "E0300"),
        ("fn main()->i64{let values=[1i64,2];return 0i64;}", "E0300"),
        ("fn main()->i64{return 9223372036854775808i64;}", "E0203"),
        ("fn main()->i64{return -9223372036854775809i64;}", "E0203"),
        (
            "fn main()->i64{if true{return 0i64;}else{return 9223372036854775808i64;}}",
            "E0203",
        ),
        ("fn main()->i64{return 1_000i64;}", "E0101"),
        ("fn main()->i64{return 1.0i64;}", "E0101"),
        ("struct i64{value:i32}fn main()->i32{return 0;}", "E0208"),
    ] {
        let project = Project::new(source, "bounded");
        for action in ["check", "run"] {
            let rows = records(project.invoke(&[action, "--message-format=json"]), 1);
            assert!(
                rows[0].contains(&format!("\"code\":\"{code}\"")),
                "{source}: {rows:?}"
            );
            assert!(!rows[0].contains("E0500"));
        }
    }
}

#[test]
fn overflow_division_and_narrowing_report_the_actual_operator() {
    for (body, ty, code, operator) in [
        (
            "let a=9223372036854775807i64;return a+1i64;",
            "i64",
            "E0604",
            "+",
        ),
        (
            "let a=-9223372036854775808i64;return a-1i64;",
            "i64",
            "E0604",
            "a-",
        ),
        (
            "let a=9223372036854775807i64;return a*2i64;",
            "i64",
            "E0604",
            "*",
        ),
        (
            "let a=-9223372036854775808i64;return -a;",
            "i64",
            "E0604",
            "-a",
        ),
        (
            "let a=-9223372036854775808i64;return a/-1i64;",
            "i64",
            "E0604",
            "/",
        ),
        (
            "let a=-9223372036854775808i64;return a%-1i64;",
            "i64",
            "E0604",
            "%",
        ),
        ("let a=1i64;return a/0i64;", "i64", "E0607", "/"),
        ("let a=1i64;return a%0i64;", "i64", "E0607", "%"),
        (
            "let a=2147483648i64;return a.to_i32_checked();",
            "i32",
            "E0610",
            "to_i32_checked",
        ),
        (
            "let a=-1i64;let b=a.to_u8_checked();return b.to_i32();",
            "i32",
            "E0610",
            "to_u8_checked",
        ),
    ] {
        let prefix = format!("// 雪\r\nfn main()->{ty}{{");
        let source = format!("{prefix}{body}}}");
        let marker = body.find(operator).unwrap();
        let offset = prefix.len() + marker + usize::from(operator == "a-");
        let width = if operator.ends_with("checked") {
            operator.len()
        } else {
            1
        };
        for policy in ["bounded", "hosted"] {
            let project = Project::new(&source, policy);
            let rows = records(project.invoke(&["run", "--message-format=json"]), 1);
            assert!(
                rows[0].contains(&format!("\"code\":\"{code}\"")),
                "{source}: {rows:?}"
            );
            assert!(
                rows[0].contains(&format!("\"start\":{offset},\"end\":{}", offset + width)),
                "{source}: {rows:?}"
            );
            assert!(rows[1].contains("\"result\":null"));
        }
    }
}

const COMPOSITE: &str = "enum Value{Large(i64),Empty}
struct Packet{tag:u8,value:i64,tail:i32,values:[i64;2]}
enum Envelope{Packet(Packet),Error(i64),Empty}
fn pass(value:Value)->Value{return value;}
fn forward(value:Envelope)->Envelope{return value;}
fn bump(values:&mut[i64])->(){values[0]=values[0]+1i64;return;}
fn select(value:Value)->i64{match value{Value::Large(number)=>{return number;},Value::Empty=>{return 0i64;},}}
fn total(value:Envelope)->i64{match value{Envelope::Packet(packet)=>{return packet.values[0]+packet.values[1];},Envelope::Error(number)=>{return number;},Envelope::Empty=>{return 0i64;},}}
fn main()->i64{let narrow=255;let byte=narrow.to_u8_checked();let mut packet=Packet{tag:byte,value:-9223372036854775808i64,tail:17,values:[9007199254740993i64,7i64]};bump(&mut packet.values);let tag=packet.tag;if tag.to_i32()!=255||packet.tail!=17{return 0i64;}let empty=select(pass(Value::Empty));if empty!=0i64{return 0i64;}let selected=select(pass(Value::Large(packet.value)));if selected!=-9223372036854775808i64{return 0i64;}return total(forward(Envelope::Packet(packet)));}";

#[test]
fn aligned_fields_array_loans_and_enum_moves_preserve_all_bits() {
    for policy in ["bounded", "hosted"] {
        Project::new(COMPOSITE, policy).result("9007199254741001");
        Project::new("struct Count{value:i64}fn next(value:&mut Count)->i64{value.value=value.value+1i64;return value.value;}fn pair(a:i64,b:i64)->i64{return a*10i64+b;}fn main()->i64{let mut value=Count{value:7000000000i64};return pair(value.value,next(&mut value));}", policy).result("77000000001");
    }
}

#[test]
fn formatter_preserves_suffixes_and_checked_conversion_semantics() {
    let project = Project::new(COMPOSITE, "bounded");
    let formatted = project.invoke(&["fmt", "src/main.ox"]);
    assert_eq!(formatted.status.code(), Some(0), "{formatted:?}");
    assert!(formatted.stderr.is_empty());
    let text = String::from_utf8(formatted.stdout).unwrap();
    assert!(text.contains("9007199254740993i64"));
    assert!(text.contains("-9223372036854775808i64"));
    fs::write(project.0.join("src/main.ox"), text).unwrap();
    let check = project.invoke(&["fmt", "src/main.ox", "--check"]);
    assert_eq!(check.status.code(), Some(0), "{check:?}");
    project.result("9007199254741001");
}

#[test]
#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7 in OXID_LLVM_BIN"]
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn native_full_width_layouts_run_without_sources_in_both_policies_and_profiles() {
    for policy in ["bounded", "hosted"] {
        for profile in ["dev", "release"] {
            let project = Project::new(COMPOSITE, policy);
            let built = project.invoke(&["build", "--profile", profile]);
            assert!(built.status.success(), "{built:?}");
            assert!(built.stderr.is_empty(), "{built:?}");
            let executable = project.0.join("integer");
            fs::copy(project.0.join(".oxid/bin/integer"), &executable).unwrap();
            fs::remove_dir_all(project.0.join("src")).unwrap();
            fs::remove_dir_all(project.0.join(".oxid")).unwrap();
            fs::remove_file(project.0.join("oxid.toml")).unwrap();
            assert_eq!(fs::read_dir(&project.0).unwrap().count(), 1);
            let mut command = Command::new(executable);
            command.current_dir(&project.0).env_clear();
            let output = bounded_output(&mut command);
            assert_eq!(output.status.code(), Some(0), "{output:?}");
            assert_eq!(output.stdout, b"9007199254741001\n");
            assert!(output.stderr.is_empty(), "{output:?}");
        }
    }
}
