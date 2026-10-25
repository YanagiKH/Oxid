//! Byte record ownership, field bounds and native profiles through project commands.
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
        fs::write(root.join("oxid.toml"), "[project]\nname=\"byte-record\"\nversion=\"0.1.0\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n").unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        Self(root)
    }
    fn command(&self, action: &str, extra: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .arg(action)
            .current_dir(&self.0)
            .args(extra)
            .output()
            .unwrap()
    }
    fn execute(&self, profile: Option<&str>) -> Output {
        if let Some(profile) = profile {
            let built = self.command("build", &["--profile", profile, "--entry-mode=result"]);
            assert!(built.status.success(), "{built:?}");
            Command::new(self.0.join(".oxid/bin/byte-record"))
                .current_dir(std::env::temp_dir())
                .env_clear()
                .output()
                .unwrap()
        } else {
            self.command("run", &["--entry-mode=result"])
        }
    }
    fn run(&self, profile: Option<&str>, expected: i32) {
        let output = self.execute(profile);
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

fn acceptance(profile: Option<&str>) {
    for (source, expected) in [(SCALAR, 255), (ZERO, 170)] {
        Package::new(&format!("{BYTE}{source}")).run(profile, expected);
    }
    for payload in ["make()", "((make()))"] {
        let source = format!(
            "{BYTE}{COMPOSITE}fn main()->i32{{return score(forward(Outcome::Value({payload})));}}"
        );
        Package::new(&source).run(profile, 893);
    }
    let source = format!("{BYTE}{COMPOSITE}fn main()->i32{{let first=make();let moved=relay(first);return score(forward(Outcome::Value(moved)));}}");
    Package::new(&source).run(profile, 893);
}

#[test]
fn byte_records_move_mutate_project_and_return_in_interpreter() {
    acceptance(None);
}

#[test]
fn byte_record_restrictions_and_whole_root_loans_are_preserved() {
    for (source, code) in [
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
    Package::new("enum E{None}struct R{value:E}fn main()->i32{let record=R{value:E::None};let R{value}=record;match value{E::None=>{return 0;},}}")
        .run(None, 0);
}

fn bounds(profile: Option<&str>) {
    for index in [-1, 1] {
        let source = format!("{BYTE}struct R{{before:i32,bytes:[u8;1],after:i32}}fn get(bytes:&[u8])->u8{{return bytes[{index}];}}fn main()->i32{{let r=R{{before:17,bytes:[byte(255)],after:23}};let b=get(&r.bytes);return b.to_i32();}}");
        let output = Package::new(&source).execute(profile);
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
    bounds(None);
}

#[test]
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[ignore = "requires LLVM 19.1.7; executes dev and release native profiles"]
fn byte_records_native_matches_fixed_reference_oracles() {
    for profile in ["dev", "release"] {
        acceptance(Some(profile));
        bounds(Some(profile));
    }
}

const ALL_BYTES: &str = "struct R{first:i32,byte:u8,bytes:[u8;1],last:i32}fn main()->i32{let z=0;let b=z.to_u8_checked();let mut r=R{first:16909060,byte:b,bytes:[b],last:84281096};let mut n=0;let mut sum=0;while n<256{r.byte=n.to_u8_checked();let copied=r.byte;r.bytes[0]=copied;let stored=r.bytes[0];if stored.to_i32()!=n||r.first!=16909060||r.last!=84281096{return -1;}sum=sum+stored.to_i32();n=n+1;}return sum;}";

#[test]
fn byte_record_all_octets_and_maximum_field_declaration() {
    Package::new(ALL_BYTES).run(None, 32640);
    for length in [0, 1, 1024] {
        let source = format!("struct Inner{{bytes:[u8;{length}]}}struct Outer{{before:u8,inner:Inner,after:i32}}enum Item{{Value(Outer),Empty}}fn len(p:&Outer)->i32{{return p.inner.bytes.len();}}fn main()->i32{{return 0;}}");
        let package = Package::new(&source);
        let checked = package.command("check", &[]);
        assert!(checked.status.success(), "{length}: {checked:?}");
        package.run(None, 0);
    }
}

fn narrow_failures(profile: Option<&str>) {
    for invalid in [-1, 256] {
        for body in [
            format!("let b=byte(128);let mut r=R{{first:b,last:b}};r.last=byte({invalid});"),
            format!("let r=R{{first:byte(128),last:byte({invalid})}};"),
        ] {
            let source =
                format!("{BYTE}struct R{{first:u8,last:u8}}fn main()->i32{{{body}return 0;}}");
            let output = Package::new(&source).execute(profile);
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            let message = String::from_utf8_lossy(&output.stderr);
            assert!(message.contains("E0610"), "{message}");
            assert!(!message.contains("E0500"), "{message}");
        }
    }
}

#[test]
fn byte_record_checked_narrowing_rejects_outside_octet_range() {
    narrow_failures(None);
}

#[test]
fn byte_record_bad_field_type_preserves_existing_managed_artifact() {
    let package =
        Package::new("struct R{bytes:[u8;1]}fn main()->i32{let r=R{bytes:[256]};return 0;}");
    let output = package.0.join(".oxid/bin/byte-record");
    fs::create_dir_all(output.parent().unwrap()).unwrap();
    fs::write(&output, b"previous native artifact\0\xff").unwrap();
    for profile in ["dev", "release"] {
        let rejected = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&package.0)
            .env("OXID_LLVM_BIN", package.0.join("missing-tools"))
            .args(["build", "--profile", profile, "--message-format=json"])
            .output()
            .unwrap();
        assert_eq!(rejected.status.code(), Some(1), "{rejected:?}");
        let text = String::from_utf8_lossy(&rejected.stdout);
        assert!(text.contains("E0300"), "{text}");
        assert!(!text.contains("E0701"), "{text}");
        assert!(rejected.stderr.is_empty(), "{rejected:?}");
        assert_eq!(
            fs::read(&output).unwrap(),
            b"previous native artifact\0\xff"
        );
        assert_eq!(fs::read_dir(package.0.join(".oxid")).unwrap().count(), 1);
    }
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires LLVM 19.1.7; executes all octets and narrowing failures at O0/O2"]
fn byte_record_native_octets_and_checked_narrowing_in_both_profiles() {
    for profile in ["dev", "release"] {
        Package::new(ALL_BYTES).run(Some(profile), 32640);
        narrow_failures(Some(profile));
    }
}
