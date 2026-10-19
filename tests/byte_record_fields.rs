//! Focused byte-bearing record behavior through the public CLI. No qualification,
//! capacity or fuel probes. Native cases require an existing pinned LLVM toolchain.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Package(PathBuf);
impl Package {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-byte-record-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("oxid.toml"), "[project]\nname=\"byte-record\"\nversion=\"0.1.0\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n").unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        Self(root)
    }
    fn command(&self, action: &str, extra: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .arg(action)
            .arg("--manifest-path")
            .arg(self.0.join("oxid.toml"))
            .args(extra);
        if extra.contains(&"native") {
            command.env(
                "OXID_LLVM_BIN",
                std::env::var_os("OXID_TEST_LLVM_BIN")
                    .expect("native byte record tests require OXID_TEST_LLVM_BIN (LLVM 19.1.7)"),
            );
        }
        command.output().unwrap()
    }
    fn run(&self, engine: &str, expected: i32) {
        let output = self.command("run", &["--engine", engine]);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        assert!(output.stderr.is_empty(), "{output:?}");
    }
    fn reject(&self, code: &str) {
        let output = self.command("check", &["--message-format=json"]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains(&format!("\"code\":\"{code}\"")), "{text}");
        assert!(!text.contains("E0500"), "{text}");
    }
}
impl Drop for Package {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const BYTE: &str = "fn byte(x:i32)->u8{return x.to_u8_checked();}";
const SCALAR: &str = "struct Byte{value:u8}
fn make()->Byte{return Byte{value:byte(128)};}
fn relay(value:Byte)->Byte{let moved=value;return moved;}
fn edit(value:&mut Byte)->(){value.value=byte(255);return;}
fn main()->i32{let mut value=relay(make());edit(&mut value);let copied=value.value;value.value=byte(0);return copied.to_i32();}";
const COMPOSITE: &str = "struct Meta{kind:u8,empty:[u8;0],bytes:[u8;4],guard:i32}
struct Packet{left:i32,meta:Meta,tail:u8,right:i32}
enum Outcome{Value(Packet),Error(i32)}
fn make()->Packet{let empty:[u8;0]=[];return Packet{right:1432778632,meta:Meta{bytes:[byte(0),byte(127),byte(128),byte(255)],guard:16909060,kind:byte(0),empty:empty},left:287454020,tail:byte(0)};}
fn relay(packet:Packet)->Packet{let moved=packet;return moved;}
fn forward(value:Outcome)->Outcome{return value;}
fn sum(bytes:&[u8])->i32{let mut total=0;let mut i=0;while i<bytes.len(){let b=bytes[i];total=total+b.to_i32();i=i+1;}return total;}
fn edit(bytes:&mut[u8])->(){bytes[0]=byte(255);bytes[3]=byte(0);return;}
fn change(packet:&mut Packet)->(){packet.meta.kind=byte(128);packet.tail=byte(127);packet.meta.bytes[1]=byte(255);edit(&mut *packet.meta.bytes);return;}
fn inspect(packet:&Packet)->i32{if packet.left!=287454020||packet.right!=1432778632||packet.meta.guard!=16909060||packet.meta.empty.len()!=0{return -1;}let kind=packet.meta.kind;let tail=packet.tail;return sum(&*packet.meta.bytes)+sum(&*packet.meta.empty)+kind.to_i32()+tail.to_i32();}
fn score(value:Outcome)->i32{match value{Outcome::Value(packet)=>{let mut moved=relay(packet);change(&mut moved);let before=sum(&moved.meta.bytes);edit(&mut moved.meta.bytes);if before!=638{return -2;}let last=moved.meta.bytes[3];if last!=byte(0){return -3;}return inspect(&moved);},Outcome::Error(code)=>{return code;},}}";
const ZERO: &str = "struct Empty{before:u8,bytes:[u8;0],after:i32}
fn relay(value:Empty)->Empty{return value;}
fn len(bytes:&[u8])->i32{return bytes.len();}
fn main()->i32{let bytes:[u8;0]=[];let mut value=relay(Empty{before:byte(255),bytes:bytes,after:42});value.before=byte(128);let b=value.before;return len(&value.bytes)+value.after+b.to_i32();}";

fn acceptance(engine: &str) {
    for (source, expected) in [(SCALAR, 255), (ZERO, 170)] {
        Package::new(&format!("{BYTE}{source}")).run(engine, expected);
    }
    for payload in ["make()", "((make()))"] {
        let source = format!(
            "{BYTE}{COMPOSITE}fn main()->i32{{return score(forward(Outcome::Value({payload})));}}"
        );
        Package::new(&source).run(engine, 893);
    }
    let source = format!("{BYTE}{COMPOSITE}fn main()->i32{{let first=make();let moved=relay(first);return score(forward(Outcome::Value(moved)));}}");
    Package::new(&source).run(engine, 893);
}

#[test]
fn byte_records_move_mutate_project_and_return_in_interpreter() {
    acceptance("interpreter");
}

#[test]
fn byte_record_restrictions_and_whole_root_loans_are_preserved() {
    for (source, code) in [
        ("enum E{None}struct R{value:E}fn main()->i32{return 0;}", "E0300"),
        ("enum E{Value(u8)}fn main()->i32{return 0;}", "E0100"),
        ("struct R{bytes:&[u8]}fn main()->i32{return 0;}", "E0101"),
        ("struct R{bytes:[u8;1025]}fn main()->i32{return 0;}", "E0400"),
        ("struct R{value:u8}fn main()->i32{let r=R{value:1};return 0;}", "E0300"),
        ("struct R{bytes:[u8;1]}fn main()->i32{let r=R{bytes:[1]};return 0;}", "E0300"),
        ("struct R{bytes:[u8;0]}fn main()->i32{let r=R{bytes:[]};return 0;}", "E0300"),
        ("struct R{value:u8}fn main()->i32{let r=R{value:byte(1)};let next=r;let b=r.value;return b.to_i32();}", "E0310"),
        ("struct R{bytes:[u8;1]}fn main()->i32{let r=R{bytes:[byte(1)]};let extracted=r.bytes;return 0;}", "E0305"),
        ("struct R{bytes:[u8;1]}fn main()->i32{let r=R{bytes:[byte(1)]};let i=byte(0);let b=r.bytes[i];return b.to_i32();}", "E0300"),
        ("struct R{a:[u8;1],b:[u8;1]}fn use_both(a:&mut[u8],b:&[u8])->i32{return b.len();}fn main()->i32{let mut r=R{a:[byte(0)],b:[byte(1)]};return use_both(&mut r.a,&r.b);}", "E0311"),
        ("struct R{bytes:[u8;1]}fn exact(bytes:&[u8;1])->i32{return bytes.len();}fn main()->i32{let r=R{bytes:[byte(1)]};return exact(&r.bytes);}", "E0300"),
        ("struct R{bytes:[u8;1]}fn wrong(bytes:&[i32])->i32{return bytes.len();}fn main()->i32{let r=R{bytes:[byte(1)]};return wrong(&r.bytes);}", "E0300"),
        ("struct R{bytes:[u8;1]}fn len(bytes:&[u8])->i32{return bytes.len();}fn wrong(r:&R)->i32{return len(&r.bytes);}fn main()->i32{return 0;}", "E0312"),
    ] {
        Package::new(&format!("{BYTE}{source}")).reject(code);
    }
}

fn bounds(engine: &str) {
    for index in [-1, 1] {
        let source = format!("{BYTE}struct R{{before:i32,bytes:[u8;1],after:i32}}fn get(bytes:&[u8])->u8{{return bytes[{index}];}}fn main()->i32{{let r=R{{before:17,bytes:[byte(255)],after:23}};let b=get(&r.bytes);return b.to_i32();}}");
        let output = Package::new(&source).command("run", &["--engine", engine]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("E0606"),
            "{output:?}"
        );
    }
}

#[test]
fn byte_record_projected_bounds_are_signed_and_use_field_length() {
    bounds("interpreter");
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN; no installation or fallback"]
fn byte_records_native_matches_fixed_reference_oracles() {
    acceptance("native");
    bounds("native");
}
