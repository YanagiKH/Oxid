//! Owned enum vectors through ordinary typed project commands.
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
#[path = "../src/frontend/oir/owned/source/owned_enum_vector_cases.rs"]
mod cases;

fn output(command: &mut Command) -> Output {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let drain = |mut stream: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            bytes
        })
    };
    let stdout = drain(Box::new(child.stdout.take().unwrap()));
    let stderr = drain(Box::new(child.stderr.take().unwrap()));
    let deadline = Instant::now() + Duration::from_secs(90);
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
            panic!(
                "enum vector command timed out: {command:?}; stdout={:?}; stderr={:?}",
                stdout.join().unwrap(),
                stderr.join().unwrap()
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

struct Project(PathBuf);
impl Project {
    fn new(body: &str, policy: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-owned-enum-vector-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), format!("{API}\n{body}")).unwrap();
        fs::write(root.join("oxid.toml"), format!("[project]\nname = \"owned-enum-vector-contracts\"\nedition = \"typed-preview\"\nentry = \"main.ox\"\n[build]\nentry-mode = \"result\"\n[execution]\npolicy = \"{policy}\"\n")).unwrap();
        Self(root)
    }
    fn invoke(&self, args: &[&str]) -> Output {
        output(
            Command::new(env!("CARGO_BIN_EXE_oxid"))
                .current_dir(&self.0)
                .env_remove("OXID_PATH")
                .env("OXID_CACHE_DIR", self.0.join("cache"))
                .args(args),
        )
    }
    fn no_artifacts(&self) {
        for path in [".oxid", "oxid.lock", "cache", "main.oxb", "native"] {
            assert!(!self.0.join(path).exists(), "unexpected {path}");
        }
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("enum vector fixture retained: {}", self.0.display());
        } else {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

const API: &str = cases::API;

const MIXED: &str = r#"
enum Token<T> { Empty, Data(T), Wide(i128) }
struct Message { value: i32 }
fn identity<T>(value: T) -> T { return value; }
fn consume(token: Token<i32>) -> i32 {
    match token {
        Token::Empty => { return 0; },
        Token::Data(number) => { return number; },
        Token::Wide(wide) => {
            if wide != 170141183460469231731687303715884105727i128 { return -1; }
            return 40;
        },
    }
}
fn main() -> i32 {
    let mut values = new::<Token<i32>>();
    if len(&values) != 0 || capacity(&values) != 0 { return -2; }
    if !grew(try_reserve(&mut values, 3)) || capacity(&values) < 3 { return -3; }
    push_reserved(&mut values, identity(Token::<i32>::Data(2)));
    let first = consume(pop_last(&mut values));
    push_reserved(&mut values, Token::<i32>::Wide(170141183460469231731687303715884105727i128));
    let second = consume(pop_last(&mut values));
    push_reserved(&mut values, Token::<i32>::Empty);
    let mut detached = take(&mut values);
    if len(&values) != 0 || capacity(&values) != 0 || len(&detached) != 1 { return -4; }
    truncate(&mut detached, 0);
    let mut records = new::<Message>();
    if !grew(try_reserve(&mut records, 1)) { return -5; }
    push_reserved(&mut records, Message { value: first + second });
    let message = pop_last(&mut records);
    let mut scalars = new::<i32>();
    if len(&detached) != 0 || len(&records) != 0 || len(&scalars) != 0 { return -6; }
    return message.value;
}
"#;

const QUEUE: &str = r#"
mod text;
use crate::text::Text;
use crate::text::new_text;
use crate::text::append_utf8;
use crate::text::byte_len;
use crate::text::equals_utf8;
use std::fs::File;
use std::fs::OpenResult;
use std::fs::create;
enum Token { Empty, Text(Text), File(File), Wide(i128) }
struct Message { token: Token }
fn text_token() -> Token {
    let mut text = new_text();
    let expected = b"\xE9\x9B\xAA\xF0\x9F\xA6\x80";
    append_utf8(&mut text, &expected);
    if byte_len(&text) != 7 || !equals_utf8(&text, &expected) { return Token::Empty; }
    return Token::Text(text);
}
fn consume(token: Token) -> i32 {
    match token {
        Token::Empty => { return -1; },
        Token::Text(text) => { return byte_len(&text); },
        Token::File(file) => { return 1; },
        Token::Wide(number) => { if number != 170141183460469231731687303715884105727i128 { return -2; } return 34; },
    }
}
fn main() -> i32 {
    let mut values = new::<Token>();
    if !grew(try_reserve(&mut values, 4)) { return -3; }
    push_reserved(&mut values, text_token());
    let text_length = consume(pop_last(&mut values));
    push_reserved(&mut values, Token::Wide(170141183460469231731687303715884105727i128));
    let number = consume(pop_last(&mut values));
    let path = b"owned-enum-file.bin";
    let opened = create(&path);
    match opened {
        OpenResult::Opened(file) => {
            push_reserved(&mut values, Token::File(file));
            let file_result = consume(pop_last(&mut values));
            push_reserved(&mut values, text_token());
            let mut detached = take(&mut values);
            truncate(&mut detached, 0);
            let mut records = new::<Message>();
            if !grew(try_reserve(&mut records, 1)) { return -4; }
            push_reserved(&mut records, Message { token: text_token() });
            let Message { token } = pop_last(&mut records);
            if consume(token) != 7 || len(&values) != 0 { return -5; }
            return text_length + number + file_result;
        },
        OpenResult::Error(code) => { return -6; },
        OpenResult::AllocationFailed => { return -7; },
        OpenResult::MemoryLimit => { return -8; },
    }
}
"#;

#[test]
fn user_enum_vectors_support_all_eight_operations_and_mixed_scalar_record_roots() {
    for policy in ["bounded", "hosted"] {
        let project = Project::new(MIXED, policy);
        let checked = project.invoke(&["check"]);
        assert!(checked.status.success(), "{checked:?}");
        let ran = project.invoke(&["run"]);
        assert!(ran.status.success(), "{ran:?}");
        assert_eq!(ran.stdout, b"42\n");
        assert!(ran.stderr.is_empty(), "{ran:?}");
        project.no_artifacts();
    }
}

#[test]
fn builtin_enum_vectors_rebind_after_late_interleaved_user_enum_instantiations() {
    for policy in ["bounded", "hosted"] {
        let project = Project::new(cases::INTERLEAVED_BUILTINS, policy);
        let checked = project.invoke(&["check"]);
        assert!(checked.status.success(), "{checked:?}");
        let ran = project.invoke(&["run"]);
        assert!(ran.status.success(), "{ran:?}");
        assert_eq!(ran.stdout, b"42\n");
        assert!(ran.stderr.is_empty(), "{ran:?}");
        project.no_artifacts();
    }
}

#[test]
fn token_queue_moves_real_utf8_text_file_and_sixteen_byte_payloads() {
    for policy in ["bounded", "hosted"] {
        let project = Project::new(QUEUE, policy);
        fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("packages/utf8_text/src/lib.ox"),
            project.0.join("text.ox"),
        )
        .unwrap();
        let checked = project.invoke(&["check"]);
        assert!(checked.status.success(), "{checked:?}");
        let ran = project.invoke(&["run"]);
        if cfg!(all(
            any(target_os = "linux", target_os = "macos"),
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )) {
            assert!(ran.status.success(), "{ran:?}");
            assert_eq!(ran.stdout, b"42\n");
            assert!(ran.stderr.is_empty(), "{ran:?}");
            assert!(project.0.join("owned-enum-file.bin").is_file());
            fs::write(
                project.0.join("owned-enum-file.bin"),
                b"reopened after drop",
            )
            .unwrap();
        } else {
            assert_eq!(ran.status.code(), Some(1), "{ran:?}");
            assert!(
                String::from_utf8_lossy(&ran.stderr).contains("E0608"),
                "{ran:?}"
            );
            assert!(!project.0.join("owned-enum-file.bin").exists());
        }
        project.no_artifacts();
    }
}

#[test]
fn enum_vectors_preserve_move_loan_and_closed_element_operation_refusals() {
    for (body, code) in [
        ("enum E{N(i32)}fn main()->i32{let mut values=new::<E>();try_reserve(&mut values,2);let value=E::N(42);push_reserved(&mut values,value);push_reserved(&mut values,value);return 0;}", "E0310"),
        ("enum E{N(i32)}fn main()->i32{let mut values=new::<E>();let moved=values;let again=values;return 0;}", "E0310"),
        ("enum E{N(i32)}fn main()->i32{let mut values=new::<E>();push_reserved(&mut values,pop_last(&mut values));return 0;}", "E0311"),
        ("enum E{N(i32)}fn main()->i32{let mut values=new::<E>();let selected=values[0];return 0;}", "E0205"),
        ("enum E{N(i32)}fn inspect(values:&[E])->(){return;}fn main()->i32{return 0;}", "E0205"),
        ("enum E{N(i32)}fn main()->i32{let values=new::<Vec<E>>();return 0;}", "E0205"),
        ("struct R{value:i32}struct H{values:Vec<R>}fn main()->i32{return 0;}", "E0205"),
        ("enum E{N(i32)}struct H{values:Vec<E>}fn main()->i32{let values=new::<H>();return 0;}", "E0300"),
        ("enum E{N(i32)}struct H{values:Vec<E>}enum Outer{Value(H)}fn main()->i32{let values=new::<Outer>();return 0;}", "E0300"),
        ("enum E{Value(H)}struct H{values:Vec<E>}fn main()->i32{return 0;}", "E0205"),
        ("#[derive(Copy)]enum E{N(i32)}fn main()->i32{return 0;}", "E0202"),
        ("fn main()->i32{let values=new::<ByteBuf>();return 0;}", "E0205"),
    ] {
        let project = Project::new(body, "bounded");
        for command in ["check", "run"] {
            let result = project.invoke(&[command, "--message-format=json"]);
            assert_eq!(result.status.code(), Some(1), "{body}: {result:?}");
            assert!(result.stderr.is_empty(), "{result:?}");
            let json = String::from_utf8_lossy(&result.stdout);
            assert!(json.contains(&format!("\"code\":\"{code}\"")), "{body}: {json}");
            if code == "E0300" {
                assert!(json.contains("owned vector elements cannot contain owned vectors"), "{json}");
            }
            assert!(!json.contains("E0500"), "{json}");
        }
        project.no_artifacts();
    }
}

#[test]
#[ignore = "requires LLVM 19.1.7 on a supported Linux or macOS host"]
fn native_owned_enum_token_queue_runs_after_source_free_relocation() {
    for policy in ["bounded", "hosted"] {
        for profile in ["dev", "release"] {
            let project = Project::new(QUEUE, policy);
            fs::copy(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("packages/utf8_text/src/lib.ox"),
                project.0.join("text.ox"),
            )
            .unwrap();
            let built = project.invoke(&["build", "--profile", profile]);
            assert!(built.status.success(), "{built:?}");
            let binary = project.0.join(".oxid/bin/owned-enum-vector-contracts");
            assert!(binary.is_file(), "{}: {built:?}", binary.display());
            let relocated = project.0.join("native");
            fs::copy(&binary, &relocated).unwrap();
            fs::remove_file(project.0.join("main.ox")).unwrap();
            fs::remove_file(project.0.join("text.ox")).unwrap();
            fs::remove_file(project.0.join("oxid.toml")).unwrap();
            fs::remove_dir_all(project.0.join(".oxid")).unwrap();
            let ran = output(Command::new(&relocated).current_dir(&project.0));
            assert!(ran.status.success(), "{ran:?}");
            assert_eq!(ran.stdout, b"42\n");
            assert!(ran.stderr.is_empty(), "{ran:?}");
            fs::write(
                project.0.join("owned-enum-file.bin"),
                b"closed after native token drop",
            )
            .unwrap();
        }
    }
}

#[test]
#[ignore = "requires LLVM 19.1.7 on a supported Linux or macOS host"]
fn native_builtin_enum_vectors_keep_final_ids_after_source_free_relocation() {
    for policy in ["bounded", "hosted"] {
        for profile in ["dev", "release"] {
            let project = Project::new(cases::INTERLEAVED_BUILTINS, policy);
            let built = project.invoke(&["build", "--profile", profile]);
            assert!(built.status.success(), "{built:?}");
            let binary = project.0.join(".oxid/bin/owned-enum-vector-contracts");
            let relocated = project.0.join("native");
            fs::copy(binary, &relocated).unwrap();
            fs::remove_file(project.0.join("main.ox")).unwrap();
            fs::remove_file(project.0.join("oxid.toml")).unwrap();
            fs::remove_dir_all(project.0.join(".oxid")).unwrap();
            let ran = output(Command::new(&relocated).current_dir(&project.0));
            assert!(ran.status.success(), "{ran:?}");
            assert_eq!(ran.stdout, b"42\n");
            assert!(ran.stderr.is_empty(), "{ran:?}");
        }
    }
}

#[test]
fn enum_vector_record_fields_replace_and_destructure_with_live_children() {
    for policy in ["bounded", "hosted"] {
        for body in [
            cases::RECORD_FIELD_LIFETIME,
            cases::JOINT_RECORD_ENUM_FIELDS,
        ] {
            let project = Project::new(body, policy);
            let checked = project.invoke(&["check"]);
            assert!(checked.status.success(), "{checked:?}");
            let ran = project.invoke(&["run"]);
            assert!(ran.status.success(), "{ran:?}");
            assert_eq!(ran.stdout, b"42\n");
            assert!(ran.stderr.is_empty(), "{ran:?}");
            project.no_artifacts();
        }
    }
}

fn default_heap_memory_limit_case() -> String {
    let small = "try_reserve(&mut values, 127)";
    assert_eq!(cases::MEMORY_LIMIT.matches(small).count(), 1);
    // The public hosted runtime has a16 MiB heap ceiling. Request more than
    // that ceiling while retaining a vector child and an independent enum.
    cases::MEMORY_LIMIT.replace(small, "try_reserve(&mut values, 1048576)")
}

#[test]
fn hosted_enum_vector_growth_memory_limit_leaves_both_live_owners_usable() {
    let project = Project::new(&default_heap_memory_limit_case(), "hosted");
    let checked = project.invoke(&["check"]);
    assert!(checked.status.success(), "{checked:?}");
    let ran = project.invoke(&["run"]);
    assert!(ran.status.success(), "{ran:?}");
    assert_eq!(ran.stdout, b"42\n");
    assert!(ran.stderr.is_empty(), "{ran:?}");
    project.no_artifacts();
}

#[test]
fn sparse_builtin_enum_vectors_keep_active_ordinals_after_generic_worklist_growth() {
    for policy in ["bounded", "hosted"] {
        let project = Project::new(cases::SPARSE_ARGUMENT_STATUS, policy);
        let checked = project.invoke(&["check"]);
        assert!(checked.status.success(), "{checked:?}");
        let ran = project.invoke(&["run"]);
        assert!(ran.status.success(), "{ran:?}");
        assert_eq!(ran.stdout, b"42\n");
        assert!(ran.stderr.is_empty(), "{ran:?}");
        project.no_artifacts();
    }
}

#[test]
#[ignore = "requires LLVM 19.1.7 on a supported Linux or macOS host"]
fn native_enum_vector_record_fields_and_memory_limit_recover_after_source_free_relocation() {
    let memory_limit = default_heap_memory_limit_case();
    for (body, policy) in [
        (cases::RECORD_FIELD_LIFETIME, "bounded"),
        (cases::RECORD_FIELD_LIFETIME, "hosted"),
        (cases::JOINT_RECORD_ENUM_FIELDS, "bounded"),
        (cases::JOINT_RECORD_ENUM_FIELDS, "hosted"),
        (cases::SPARSE_ARGUMENT_STATUS, "bounded"),
        (cases::SPARSE_ARGUMENT_STATUS, "hosted"),
        (memory_limit.as_str(), "hosted"),
    ] {
        for profile in ["dev", "release"] {
            let project = Project::new(body, policy);
            let built = project.invoke(&["build", "--profile", profile]);
            assert!(built.status.success(), "{built:?}");
            let binary = project.0.join(".oxid/bin/owned-enum-vector-contracts");
            let relocated = project.0.join("native");
            fs::copy(binary, &relocated).unwrap();
            fs::remove_file(project.0.join("main.ox")).unwrap();
            fs::remove_file(project.0.join("oxid.toml")).unwrap();
            fs::remove_dir_all(project.0.join(".oxid")).unwrap();
            let ran = output(Command::new(&relocated).current_dir(&project.0));
            assert!(ran.status.success(), "{ran:?}");
            assert_eq!(ran.stdout, b"42\n");
            assert!(ran.stderr.is_empty(), "{ran:?}");
        }
    }
}
