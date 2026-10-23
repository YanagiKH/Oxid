//! Typed byte strings through public checking, execution and native builds.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new(source: &str, process: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-byte-strings-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), format!("[project]\nname=\"byte-strings\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"{}\"\n", if process { "process" } else { "result" })).unwrap();
        Self(root)
    }
    fn command(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(arguments)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(output: Output, expected: &[u8]) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, expected, "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}

const BASIC: &str = r#"
fn main()->i32{
    let empty=b"";
    let value=b"A\0\xFF\n\r\t\\\"";
    if empty.len()!=0 || value.len()!=8{return -1;}
    let a=value[0];let nul=value[1];let high=value[2];
    let newline=value[3];let carriage=value[4];let tab=value[5];
    let slash=value[6];let quote=value[7];
    if a.to_i32()!=65 || nul.to_i32()!=0 || high.to_i32()!=255{return -2;}
    if newline.to_i32()!=10 || carriage.to_i32()!=13 || tab.to_i32()!=9{return -3;}
    if slash.to_i32()!=92 || quote.to_i32()!=34{return -4;}
    return value.len();
}
"#;
const UNICODE: &str = r#"
fn same(left:&[u8],right:&[u8])->bool{
    if left.len()!=right.len(){return false;}
    let mut index=0;
    while index<left.len(){if left[index]!=right[index]{return false;}index=index+1;}
    return true;
}
fn main()->i32{
    let raw=b"A¢€👋";
    let escaped=b"\x41\xC2\xA2\xE2\x82\xAC\xF0\x9F\x91\x8B";
    let scalars=b"A\u{A2}\u{20AC}\u{1F44B}";
    let edge=b"\u{10FFFF}";
    let edge_bytes=b"\xF4\x8F\xBF\xBF";
    if !same(&raw,&escaped) || !same(&raw,&scalars) || !same(&edge,&edge_bytes){return -1;}
    return raw.len()+edge.len();
}
"#;
const OWNERS: &str = r#"
struct Packet{payload:[u8;4]}
enum Message{Data(Packet),Empty}
fn length(input:&[u8])->i32{return input.len();}
fn relay(input:[u8;4])->[u8;4]{return input;}
fn main()->i32{
    let mut value=relay(b"A\0\xFFZ");
    let changed=128;
    value[0]=changed.to_u8_checked();
    let packet=Packet{payload:value};
    let message=Message::Data(packet);
    match message{
        Message::Empty=>{return -1;},
        Message::Data(content)=>{return length(&content.payload);},
    }
}
"#;
const OUTPUT: &str = r#"
use std::io::WriteSomeStatus;
use std::io::write_some;
fn main()->i32{
    let message=b"Hello, Oxid 👋\n\0\xFF";
    let result=write_some(&message);
    match result{
        WriteSomeStatus::Written(count)=>{
            if count!=message.len(){return 74;}
            return 0;
        },
        WriteSomeStatus::Error(code)=>{return 74;},
    }
}
"#;

#[test]
fn empty_and_escaped_binary_literals_have_exact_u8_array_types_and_values() {
    let project = Project::new(BASIC, false);
    success(project.command(&["run"]), b"8\n");
}
#[test]
fn utf8_source_and_scalar_escapes_produce_the_same_exact_bytes() {
    let project = Project::new(UNICODE, false);
    success(project.command(&["run"]), b"14\n");
}
#[test]
fn literals_follow_existing_array_moves_mutation_and_nested_borrowing() {
    let project = Project::new(OWNERS, false);
    success(project.command(&["run"]), b"4\n");
}
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn literal_data_reaches_binary_output_without_encoding_or_nul_changes() {
    let project = Project::new(OUTPUT, true);
    let expected = ["Hello, Oxid 👋\n".as_bytes(), &[0, 255]].concat();
    success(project.command(&["run"]), &expected);
}
#[test]
fn every_octet_and_the_decoded_length_boundary_are_supported() {
    let literal: String = (0..=255).map(|value| format!("\\x{value:02X}")).collect();
    let source = format!(
        "fn main()->i32{{let bytes=b\"{literal}\";let mut index=0;while index<bytes.len(){{let value=bytes[index];if value.to_i32()!=index{{return -1;}}index=index+1;}}return bytes.len();}}"
    );
    let project = Project::new(&source, false);
    success(project.command(&["run"]), b"256\n");
    for literal in ["A".repeat(1024), "\\xFF".repeat(1024), "👋".repeat(256)] {
        let project = Project::new(
            &format!("fn main()->i32{{let bytes=b\"{literal}\";return bytes.len();}}"),
            false,
        );
        success(project.command(&["run"]), b"1024\n");
    }
}
#[test]
fn malformed_literals_and_type_or_move_errors_precede_tools_and_preserve_output() {
    for (literal, code) in [
        (r#"b"\q""#, "E0101"),
        (r#"b"\x0""#, "E0101"),
        (r#"b"\xGG""#, "E0101"),
        (r#"b"\u{}""#, "E0101"),
        (r#"b"\u{D800}""#, "E0101"),
        (r#"b"\u{110000}""#, "E0101"),
        (r#"b"\u{0000000}""#, "E0101"),
        (r#"b "A""#, "E0101"),
        (r#""A""#, "E0101"),
        ("b\"A\nB\"", "E0101"),
    ] {
        refusal(
            &format!("fn main()->i32{{let bytes={literal};return 0;}}"),
            code,
        );
    }
    refusal(
        &format!(
            "fn main()->i32{{let bytes=b\"{}\";return 0;}}",
            "A".repeat(1025)
        ),
        "E0400",
    );
    refusal(
        "fn main()->i32{let bytes:[i32;1]=b\"A\";return 0;}",
        "E0300",
    );
    refusal(
        "fn main()->i32{let bytes=b\"A\";let moved=bytes;return bytes.len();}",
        "E0310",
    );
}
fn refusal(source: &str, code: &str) {
    let project = Project::new(source, false);
    let artifact = project.0.join(".oxid/bin/byte-strings");
    let cache = project.0.join(".oxid/prior-cache");
    fs::create_dir_all(artifact.parent().unwrap()).unwrap();
    fs::write(&artifact, b"previous executable").unwrap();
    fs::write(&cache, b"previous cache").unwrap();
    for command in ["check", "run", "build"] {
        let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args([command, "--message-format=json"])
            .current_dir(&project.0)
            .env("OXID_LLVM_BIN", project.0.join("absent-tools"))
            .env("PATH", project.0.join("absent-path"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(&format!("\"code\":\"{code}\"")),
            "{output:?}"
        );
        assert_eq!(fs::read(&artifact).unwrap(), b"previous executable");
        assert_eq!(fs::read(&cache).unwrap(), b"previous cache");
    }
}
#[test]
fn byte_literal_formatting_preserves_values_and_reaches_a_fixed_point() {
    let project = Project::new(UNICODE, false);
    let formatted = project.command(&["fmt", "src/main.ox"]);
    assert!(formatted.status.success(), "{formatted:?}");
    assert!(formatted.stderr.is_empty(), "{formatted:?}");
    fs::write(project.0.join("src/main.ox"), &formatted.stdout).unwrap();
    success(project.command(&["fmt", "src/main.ox"]), &formatted.stdout);
    success(project.command(&["fmt", "src/main.ox", "--check"]), b"");
    success(project.command(&["run"]), b"14\n");
}
#[test]
#[ignore = "requires Linux x86_64 LLVM 19.1.7 in OXID_LLVM_BIN"]
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
fn native_byte_literals_preserve_binary_and_utf8_data_at_both_optimizations() {
    let stream = ["Hello, Oxid 👋\n".as_bytes(), &[0, 255]].concat();
    for (source, process, expected) in [
        (BASIC, false, b"8\n".as_slice()),
        (UNICODE, false, b"14\n".as_slice()),
        (OWNERS, false, b"4\n".as_slice()),
        (OUTPUT, true, stream.as_slice()),
    ] {
        let project = Project::new(source, process);
        for profile in ["dev", "release"] {
            let built = project.command(&["build", "--profile", profile]);
            assert!(built.status.success(), "{built:?}");
            let output = Command::new(project.0.join(".oxid/bin/byte-strings"))
                .current_dir(&project.0)
                .env_clear()
                .output()
                .unwrap();
            success(output, expected);
        }
    }
}
