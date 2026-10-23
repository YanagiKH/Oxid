//! Resource transfer through the public project routes, including a full heap.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-transfer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), "[project]\nname=\"transfer\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n").unwrap();
        Self(root)
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
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
const HELPERS: &str = r#"
use alloc::bytes::ByteBuf;
use alloc::bytes::GrowStatus;
use alloc::bytes::new;
use alloc::bytes::len;
use alloc::bytes::capacity;
use alloc::bytes::try_extend;
use alloc::bytes::try_push;
use alloc::bytes::try_reserve;
use alloc::bytes::take;
fn byte(n:i32)->u8{return n.to_u8_checked();}
fn grew(status:GrowStatus)->bool{
    match status{
        GrowStatus::Ok=>{return true;},
        GrowStatus::CapacityOverflow=>{return false;},
        GrowStatus::AllocationFailed=>{return false;},
        GrowStatus::MemoryLimit=>{return false;},
    }
}
fn equal(actual:&[u8],expected:&[u8])->bool{
    if actual.len()!=expected.len(){return false;}
    let mut i=0;
    while i<actual.len(){if actual[i]!=expected[i]{return false;}i=i+1;}
    return true;
}
"#;
const BASIC: &str = r#"
fn forward(input:&mut ByteBuf)->ByteBuf{return take(&mut *input);}
fn main()->i32{
    let expected=[byte(0),byte(255),byte(195),byte(169)];
    let mut original=new();
    if !grew(try_extend(&mut original,&expected)){return -1;}
    let previous_capacity=capacity(&original);
    let detached=forward(&mut original);
    if len(&original)!=0 || capacity(&original)!=0{return -2;}
    if capacity(&detached)!=previous_capacity || !equal(&detached,&expected){return -3;}
    let empty=take(&mut original);
    if len(&empty)!=0 || capacity(&empty)!=0{return -4;}
    if !grew(try_push(&mut original,byte(77))){return -5;}
    if len(&original)!=1 || !equal(&detached,&expected){return -6;}
    take(&mut original);
    if len(&original)!=0 || capacity(&original)!=0{return -7;}
    return len(&detached);
}
"#;
const NESTED: &str = r#"
struct Pair{left:ByteBuf,right:ByteBuf}
enum Envelope{Data(ByteBuf),Empty}
fn main()->i32{
    let expected=[byte(0),byte(255),byte(65)];
    let other=[byte(66),byte(67)];
    let mut pair=Pair{left:new(),right:new()};
    if !grew(try_extend(&mut pair.left,&expected)) || !grew(try_extend(&mut pair.right,&other)){return -1;}
    let detached=take(&mut pair.left);
    if len(&pair.left)!=0 || !equal(&pair.right,&other){return -2;}
    if !grew(try_push(&mut pair.left,byte(88))){return -3;}
    let wrapped=Envelope::Data(detached);
    match wrapped{
        Envelope::Empty=>{return -4;},
        Envelope::Data(payload)=>{
            if !equal(&payload,&expected) || !equal(&pair.right,&other){return -5;}
            return len(&payload)+len(&pair.left)+len(&pair.right);
        },
    }
}
"#;
const FULL_HEAP: &str = r#"
enum Transfer{Data(ByteBuf),Refused(i32)}
fn full_heap_transfer(input:&mut ByteBuf)->Transfer{
    let mut filler=new();
    if !grew(try_reserve(&mut filler,16777144)){return Transfer::Refused(-1);}
    let detached=take(&mut *input);
    if len(&*input)!=0 || capacity(&*input)!=0{return Transfer::Refused(-2);}
    return Transfer::Data(detached);
}
fn main()->i32{
    let expected=[byte(65),byte(66),byte(67),byte(68),byte(69),byte(70),byte(71),byte(72)];
    let mut original=new();
    if !grew(try_extend(&mut original,&expected)){return -3;}
    let moved=full_heap_transfer(&mut original);
    match moved{
        Transfer::Refused(code)=>{return code;},
        Transfer::Data(detached)=>{
            if !equal(&detached,&expected) || capacity(&detached)!=8{return -4;}
            if !grew(try_extend(&mut original,&expected)){return -5;}
            if !equal(&original,&expected) || !equal(&detached,&expected){return -6;}
            return len(&detached)+len(&original);
        },
    }
}
"#;
#[test]
fn moving_empty_and_populated_buffers_preserves_contents_capacity_and_source_reuse() {
    let project = Project::new(&format!("{HELPERS}{BASIC}"));
    success(project.command(&["run"]), b"4\n");
}
#[test]
fn transfers_through_record_fields_and_owned_enum_payloads_keep_other_owners_live() {
    let project = Project::new(&format!("{HELPERS}{NESTED}"));
    success(project.command(&["run"]), b"6\n");
}
#[test]
fn transfer_at_the_exact_heap_limit_needs_no_new_allocation() {
    let project = Project::new(&format!("{HELPERS}{FULL_HEAP}"));
    success(project.command(&["run"]), b"16\n");
}

#[test]
fn transfers_require_exclusive_live_owners_before_tools_or_output_replacement() {
    for (source, code) in [
        ("fn main()->i32{let original=new();let moved=take(&mut original);return len(&moved);}", "E0304"),
        ("fn main()->i32{let mut original=new();let previous=original;let moved=take(&mut original);return len(&moved);}", "E0310"),
        ("fn main()->i32{let mut original=1;let moved=take(&mut original);return len(&moved);}", "E0300"),
        ("fn shared(input:&ByteBuf)->ByteBuf{return take(&*input);}fn main()->i32{let original=new();let moved=shared(&original);return len(&moved);}", "E0300"),
        ("fn shared(input:&ByteBuf)->ByteBuf{return take(&mut *input);}fn main()->i32{let original=new();let moved=shared(&original);return len(&moved);}", "E0313"),
        ("fn both(shared:&ByteBuf,moved:ByteBuf)->i32{return len(&*shared)+len(&moved);}fn main()->i32{let mut original=new();return both(&original,take(&mut original));}", "E0311"),
    ] {
        let project = Project::new(&format!("{HELPERS}{source}"));
        let artifact = project.0.join(".oxid/bin/transfer");
        fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        fs::write(&artifact, b"previous executable").unwrap();
        for command in ["check", "run", "build"] {
            let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
                .args([command, "--message-format=json"])
                .current_dir(&project.0)
                .env("OXID_LLVM_BIN", project.0.join("absent-tools"))
                .env("PATH", project.0.join("absent-path"))
                .output().unwrap();
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            assert!(output.stderr.is_empty(), "{output:?}");
            assert!(String::from_utf8_lossy(&output.stdout).contains(&format!("\"code\":\"{code}\"")), "{output:?}");
            assert_eq!(fs::read(&artifact).unwrap(), b"previous executable");
        }
    }
}
#[test]
#[ignore = "requires Linux x86_64 LLVM 19.1.7 in OXID_LLVM_BIN"]
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
fn native_transfers_preserve_owners_without_allocating_at_both_optimizations() {
    for (source, expected) in [
        (BASIC, b"4\n".as_slice()),
        (NESTED, b"6\n".as_slice()),
        (FULL_HEAP, b"16\n".as_slice()),
    ] {
        let project = Project::new(&format!("{HELPERS}{source}"));
        for profile in ["dev", "release"] {
            let built = project.command(&["build", "--profile", profile]);
            assert!(built.status.success(), "{built:?}");
            let output = Command::new(project.0.join(".oxid/bin/transfer"))
                .current_dir(&project.0)
                .env_clear()
                .output()
                .unwrap();
            success(output, expected);
        }
    }
}
