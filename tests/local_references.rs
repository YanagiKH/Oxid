use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Input(PathBuf);
impl Input {
    fn new(source: &str) -> Self {
        let p = Self(std::env::temp_dir().join(format!(
            "oxid-local-references-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&p.0).unwrap();
        fs::write(p.0.join("main.ox"), source).unwrap();
        p
    }
    fn invoke(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .args(args)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .output()
            .unwrap()
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
const FILE_FIELD_SAMPLE: &str = r#"use std::fs::File;use std::fs::create;use std::fs::OpenResult;use std::fs::write_some;use std::io::WriteSomeStatus;
struct Output{file:File,extra:i32}
fn main()->i32{let path=b"local-field-output.bin";let opened_file=create(&path);match opened_file{
OpenResult::Opened(opened)=>{let mut result=Output{file:opened,extra:40};let writer=&mut result.file;let bytes=b"ok";
let written=write_some(&mut *writer,&bytes);match written{WriteSomeStatus::Written(count)=>{return result.extra+count;},WriteSomeStatus::Error(code)=>{return -1;},}},
OpenResult::Error(code)=>{return -2;},OpenResult::AllocationFailed=>{return -3;},OpenResult::MemoryLimit=>{return -4;},}}"#;

fn samples() -> [(&'static str, &'static [u8]); 29] {
    [
        ("struct Cell<T>{value:T} fn main()->i32{let value=Cell{value:42};let r=&value;return r.value;}",b"42\n"),
        ("struct Value{x:i32} fn main()->i32{let value=Value{x:42};let r=&value;let copy=r.x;return copy;}",b"42\n"),
        ("struct Value{x:i32} fn read(v:&Value)->i32{return v.x;} fn main()->i32{let value=Value{x:21};let a=&value;let b=&value;return read(&*a)+b.x;}",b"42\n"),
        ("struct Value{x:i32} fn bump(v:&mut Value)->(){v.x=v.x+1;return;} fn main()->i32{let mut value=Value{x:40};let r=&mut value;bump(&mut *r);r.x=r.x+1;return r.x;}",b"42\n"),
        ("struct Value{x:i32} fn main()->i32{let mut value=Value{x:1};let parent=&mut value;if true{let child=&mut *parent;child.x=42;}return parent.x;}",b"42\n"),
        ("struct Value{x:i32} fn main()->i32{let mut value=Value{x:39};let mut i=0;while i<3{let r=&mut value;r.x=r.x+1;i=i+1;}return value.x;}",b"42\n"),
        ("fn main()->i32{let mut values:[i32;3]=[10,20,30];if true{let r=&mut values;r[1]=42;return r[1];}return 0;}",b"42\n"),
        ("struct Value{x:i32} fn relay(v:&Value)->i32{let r=&*v;return r.x;} fn main()->i32{let value=Value{x:42};let r:&Value=&value;return relay(&*r);}",b"42\n"),
        ("use alloc::bytes::new;use alloc::bytes::try_push;use alloc::bytes::len;fn main()->i32{let mut bytes=new();if true{let r=&mut bytes;try_push(&mut *r,42u8);}return len(&bytes);}",b"1\n"),
        ("struct Value{x:i32} fn main()->i32{let mut value=Value{x:40};while true{let r=&mut value;r.x=42;break;}return value.x;}",b"42\n"),
        ("struct Value{x:i32} fn main()->i32{let mut value=Value{x:39};let mut i=0;while i<3{let r=&mut value;r.x=r.x+1;i=i+1;continue;}return value.x;}",b"42\n"),
        ("struct Value{x:i32} fn main()->i32{let mut value=Value{x:0};if false{let r=&mut value;r.x=7;}else{let r=&mut value;r.x=42;}return value.x;}",b"42\n"),
        ("struct Value{x:i32} fn read(v:&mut Value)->i32{let a=&mut *v;if true{let b=&*a;return b.x;}return 0;}fn main()->i32{let mut value=Value{x:42};return read(&mut value);}",b"42\n"),
        ("struct Value{x:i32} fn consume(v:Value)->i32{return v.x;}fn main()->i32{let value=Value{x:42};if true{let r=&value;let x=r.x;}return consume(value);}",b"42\n"),
        ("struct Value{x:i32} fn main()->i32{let value=Value{x:21};let a=&value;let b=&*a;return b.x+value.x;}",b"42\n"),
        ("struct Cell<T>{value:T}fn read<T:Copy>(cell:&Cell<T>)->T{let r=&*cell;return r.value;}fn main()->i32{let cell=Cell{value:42};return read(&cell);}",b"42\n"),
        ("enum Choice{No,Yes}struct Value{x:i32}fn main()->i32{let mut value=Value{x:0};let choice=Choice::Yes;match choice{Choice::No=>{let r=&mut value;r.x=1;},Choice::Yes=>{let r=&mut value;r.x=42;},}return value.x;}",b"42\n"),
        ("struct Cell<T>{value:T,n:i32}fn update<T>(cell:&mut Cell<T>)->i32{let current=&mut *cell;current.n=42;return current.n;}fn main()->i32{let mut cell=Cell{value:true,n:0};return update(&mut cell);}",b"42\n"),
        ("struct Part{n:i32}struct Pair{left:Part,right:Part}fn bump(part:&mut Part)->(){part.n=part.n+1;return;}fn main()->i32{let mut pair=Pair{left:Part{n:19},right:Part{n:21}};if true{let left=&mut pair.left;let right=&mut pair.right;bump(&mut *left);right.n=22;}return pair.left.n+pair.right.n;}",b"42\n"),
        ("struct Part{n:i32}struct Middle{part:Part}struct Root{middle:Middle}fn main()->i32{let mut root=Root{middle:Middle{part:Part{n:40}}};let whole=&mut root;if true{let selected=&mut *whole.middle.part;selected.n=42;}return whole.middle.part.n;}",b"42\n"),
        ("struct Part{n:i32}struct Pair{left:Part,right:Part}fn main()->i32{let mut pair=Pair{left:Part{n:20},right:Part{n:0}};let left:&Part=&pair.left;pair.right.n=22;return left.n+pair.right.n;}",b"42\n"),
        ("struct Part{n:i32}struct Wrap{part:Part}fn main()->i32{let mut root=Wrap{part:Part{n:39}};let mut i=0;while i<3{let child=&mut root.part;child.n=child.n+1;i=i+1;continue;}let last=&root.part;return last.n;}",b"42\n"),
        ("struct Part{n:i32}struct Root{part:Part}fn read<T>(root:&Root)->i32{let selected=&*root.part;return selected.n;}fn main()->i32{let root=Root{part:Part{n:42}};return read::<bool>(&root);}",b"42\n"),
        ("struct Part{n:i32}struct Middle{part:Part}struct Pair{left:Middle,right:Middle}fn main()->i32{let mut pair=Pair{left:Middle{part:Part{n:1}},right:Middle{part:Part{n:2}}};let left=&mut pair.left;let right=&mut pair.right;let a=&mut *left.part;let b=&mut *right.part;a.n=20;b.n=22;return a.n+b.n;}",b"42\n"),
        ("struct Samples{first:[i64;2],tail:i32}fn main()->i32{let mut data=Samples{first:[0i64,40i64],tail:1};let view=&mut data.first;let tail=data.tail;view[1]=view[1]+tail.to_i64();let result=view[1];return result.to_i32_checked()+data.tail;}",b"42\n"),
        (r#"use alloc::bytes::ByteBuf;use alloc::bytes::GrowStatus;use alloc::bytes::new;use alloc::bytes::try_push;use alloc::bytes::len;
struct Buffers{left:ByteBuf,right:ByteBuf}
fn grow(buf:&mut ByteBuf,value:u8)->bool{let result=try_push(&mut *buf,value);match result{GrowStatus::Ok=>{return true;},GrowStatus::CapacityOverflow=>{return false;},GrowStatus::AllocationFailed=>{return false;},GrowStatus::MemoryLimit=>{return false;},}}
fn first(buf:&[u8])->i32{let value=buf[0];return value.to_i32();}
fn main()->i32{let mut pair=Buffers{left:new(),right:new()};let left=&mut pair.left;let right=&mut pair.right;for i in 0..128{if !grow(&mut *left,20u8)||!grow(&mut *right,22u8){return -1;}}if len(&*left)!=128||len(&*right)!=128{return -2;}return first(&*left)+first(&*right);}"#,b"42\n"),
        (r#"use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::try_push;use alloc::vec::GrowStatus;
struct Samples{values:Vec<i64>}
fn main()->i32{let mut data=Samples{values:new::<i64>()};let view=&mut data.values;
let pushed=try_push(&mut *view,41i64);match pushed{GrowStatus::Ok=>{},GrowStatus::CapacityOverflow=>{return -1;},GrowStatus::AllocationFailed=>{return -2;},GrowStatus::MemoryLimit=>{return -3;},}
view[0]=view[0]+1i64;let long=view[0];return long.to_i32_checked();}"#,b"42\n"),
        (r#"use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::try_push;use alloc::vec::GrowStatus;
#[derive(Copy)]struct Point{x:i32,y:i32}struct Points{values:Vec<Point>}
fn main()->i32{let mut points=Points{values:new::<Point>()};let view=&mut points.values;
let pushed=try_push(&mut *view,Point{x:19,y:21});match pushed{GrowStatus::Ok=>{},GrowStatus::CapacityOverflow=>{return -1;},GrowStatus::AllocationFailed=>{return -2;},GrowStatus::MemoryLimit=>{return -3;},}
let before=view[0];view[0]=Point{x:99,y:23};let after=view[0];return before.x+after.y;}"#,b"42\n"),
        (FILE_FIELD_SAMPLE,b"42\n"),
    ]
}
#[test]
fn local_references_keep_their_values_across_statements_calls_and_scopes() {
    for (source, expected) in samples() {
        let input = Input::new(source);
        let checked = input.invoke(&["check", "main.ox", "--edition=typed-preview"]);
        assert!(
            checked.status.success(),
            "{}",
            String::from_utf8_lossy(&checked.stderr)
        );
        let actual = input.invoke(&["run", "main.ox", "--edition=typed-preview"]);
        assert!(
            actual.status.success(),
            "{}",
            String::from_utf8_lossy(&actual.stderr)
        );
        assert_eq!(actual.stdout, expected);
        assert!(actual.stderr.is_empty());
        if source == FILE_FIELD_SAMPLE {
            assert_eq!(
                fs::read(input.0.join("local-field-output.bin")).unwrap(),
                b"ok"
            );
        }
    }
}
#[test]
fn local_references_reject_unavailable_owners_conflicts_and_invalid_grants() {
    for source in [
        "struct Value{x:i32} fn main()->i32{let value=Value{x:42};let r=&mut value;return r.x;}",
        "struct Value{x:i32} fn main()->i32{let mut value=Value{x:42};let r=&value;r.x=0;return r.x;}",
        "struct Value{x:i32} fn main()->i32{let mut value=Value{x:42};let a=&mut value;let b=&mut value;return a.x+b.x;}",
        "struct Value{x:i32} fn main()->i32{let value=Value{x:42};let r=&value;let moved=value;return r.x;}",
        "struct Value{x:i32} fn main()->i32{let mut value=Value{x:42};let p=&mut value;let child=&mut *p;return p.x+child.x;}",
        "struct Value{x:i32} fn main()->i32{let value=Value{x:42};if true{let r=&value;}return r.x;}",
        "struct Value{x:i32} fn main()->i32{let value=Value{x:42};let r:&mut Value=&value;return r.x;}",
        "struct Value{x:i32} fn main()->i32{let value=Value{x:42};let mut r=&value;return r.x;}",
        "struct Value{x:i32} fn main()->i32{let mut value=Value{x:42};let r=&mut value;value=Value{x:0};return r.x;}",
        "struct Value{x:i32} fn main()->i32{let value=Value{x:42};let r=&value;let b=&r;return b.x;}",
        "struct Value{x:i32} fn main()->i32{let mut value=Value{x:42};let a=&value;let b=&mut *a;return b.x;}",
        "struct A{x:i32}struct B{x:i32}fn main()->i32{let value=A{x:42};let r:&B=&value;return r.x;}",
        "struct V{x:i32}fn bad<T>(v:&mut V)->i32{let r=&*v;v.x=0;return r.x;}fn main()->i32{return 0;}",
        "struct V{x:i32}fn bad<T>()->i32{let mut v=V{x:0};let r=&v;v=V{x:1};return r.x;}fn main()->i32{return 0;}",
        "fn bad<T>(value:i32)->i32{let r=&value;return 0;}fn main()->i32{return 0;}",
        "fn bad<T>(value:&[i32])->i32{let r=&*value;return 0;}fn main()->i32{return 0;}",
    ] {
        let actual=Input::new(source).invoke(&["check","main.ox","--edition=typed-preview"]);
        assert!(!actual.status.success(),"{source}");
        let error=String::from_utf8(actual.stderr).unwrap();
        assert!(error.contains("main.ox"),"{error}");
        assert!(!error.contains("E0500"),"{error}");
    }
}

#[test]
fn local_field_references_reject_overlap_and_wrong_field_types() {
    let declarations = "struct Part{n:i32}struct Pair{left:Part,right:Part}";
    for (source, diagnostic) in [
        ("fn main()->i32{let mut pair=Pair{left:Part{n:1},right:Part{n:2}};let a=&pair.left;let b=&mut pair.left;return a.n+b.n;}", "E0311"),
        ("fn main()->i32{let pair=Pair{left:Part{n:1},right:Part{n:2}};let a=&pair.left;let moved=pair;return a.n;}", "E0311"),
        ("fn reader(pair:&Pair)->i32{let a=&mut *pair.left;return a.n;}", "E0313"),
        ("fn main()->i32{let pair=Pair{left:Part{n:1},right:Part{n:2}};let field=&pair.left.n;return 0;}", "E0300"),
        ("fn main()->i32{let pair=Pair{left:Part{n:1},right:Part{n:2}};let field=&pair.unknown;return 0;}", "E0305"),
        ("fn main()->i32{let pair=Pair{left:Part{n:1},right:Part{n:2}};let field:&Pair=&pair.left;return 0;}", "E0300"),
    ] {
        let input = Input::new(&format!("{declarations}{source}"));
        let result = input.invoke(&["check", "main.ox", "--edition=typed-preview"]);
        assert!(!result.status.success(), "{source}");
        let message = String::from_utf8(result.stderr).unwrap();
        assert!(message.contains(diagnostic) && !message.contains("E0500"), "{message}");
    }
    let nested = "struct Part{n:i32}struct Middle{part:Part}struct Root{middle:Middle}fn main()->i32{let mut root=Root{middle:Middle{part:Part{n:42}}};let parent=&mut root.middle;let child=&mut *parent.part;parent.part.n=0;return child.n;}";
    let result = Input::new(nested).invoke(&["check", "main.ox", "--edition=typed-preview"]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("E0311"));
}

#[test]
fn formatting_local_references_preserves_program_results() {
    for (source, expected) in samples() {
        let input = Input::new(source);
        let formatted = input.invoke(&["fmt", "main.ox", "--edition=typed-preview"]);
        assert!(
            formatted.status.success(),
            "{}",
            String::from_utf8_lossy(&formatted.stderr)
        );
        fs::write(input.0.join("main.ox"), &formatted.stdout).unwrap();
        let checked = input.invoke(&["fmt", "main.ox", "--edition=typed-preview", "--check"]);
        assert!(
            checked.status.success(),
            "{}",
            String::from_utf8_lossy(&checked.stderr)
        );
        let result = input.invoke(&["run", "main.ox", "--edition=typed-preview"]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, expected);
        assert!(result.stderr.is_empty());
    }
}

#[test]
fn local_reference_parent_chains_admit_the_bound_and_report_exhaustion() {
    for depth in [1, 4, 64, 65] {
        let mut source = String::from(
            "struct V{x:i32}fn read(v:&V)->i32{return v.x;}fn main()->i32{let v=V{x:42};let r0=&v;",
        );
        for id in 1..depth {
            source.push_str(&format!("let r{id}=&*r{};", id - 1));
        }
        source.push_str(&format!("return read(&*r{});}}", depth - 1));
        let actual = Input::new(&source).invoke(&["run", "main.ox", "--edition=typed-preview"]);
        if depth <= 64 {
            assert!(
                actual.status.success(),
                "{}",
                String::from_utf8_lossy(&actual.stderr)
            );
            assert_eq!(actual.stdout, b"42\n");
            assert!(actual.stderr.is_empty());
        } else {
            assert!(!actual.status.success());
            assert!(actual.stdout.is_empty());
            assert!(String::from_utf8_lossy(&actual.stderr).contains("E0400"));
        }
    }
}

fn capability_cases() -> Vec<(String, bool)> {
    let mut cases = Vec::new();
    for first_exclusive in [false, true] {
        for second_exclusive in [false, true] {
            for child in [false, true] {
                let acquisition = if child {
                    first_exclusive || !second_exclusive
                } else {
                    !first_exclusive && !second_exclusive
                };
                for target in ["root", "first", "second"] {
                    for write in [false, true] {
                        // Whole-value capability rules: shared siblings may read;
                        // a child suspends conflicting parent access; only an
                        // exclusive leaf may write.
                        let permission = match target {
                            "root" => !first_exclusive && !second_exclusive && !write,
                            "first" => (!child || !second_exclusive) && !write,
                            "second" => !write || second_exclusive,
                            _ => unreachable!(),
                        };
                        let first = if first_exclusive { "mut " } else { "" };
                        let second = if second_exclusive { "mut " } else { "" };
                        let source = if child { "*first" } else { "root" };
                        let action = if write {
                            format!("{target}.x=42;")
                        } else {
                            String::new()
                        };
                        cases.push((format!("struct V{{x:i32}}fn main()->i32{{let mut root=V{{x:42}};let first=&{first}root;if true{{let second=&{second}{source};{action}return {target}.x;}}return 0;}}"), acquisition && permission));
                    }
                }
            }
        }
    }
    cases
}

#[test]
fn local_reference_programs_match_independent_whole_value_capabilities() {
    let cases = capability_cases();
    assert_eq!(cases.len(), 48);
    assert_eq!(cases.iter().filter(|(_, allowed)| *allowed).count(), 10);
    for (source, allowed) in cases {
        for unused_generic in [false, true] {
            let source = if unused_generic {
                source.replace("fn main()->i32", "fn unused<T>()->i32")
                    + "fn main()->i32{return 0;}"
            } else {
                source.clone()
            };
            let input = Input::new(&source);
            for operation in ["check", "run"] {
                let result = input.invoke(&[operation, "main.ox", "--edition=typed-preview"]);
                assert_eq!(
                    result.status.success(),
                    allowed,
                    "{source}: {}",
                    String::from_utf8_lossy(&result.stderr)
                );
                if allowed {
                    assert!(result.stderr.is_empty());
                    if operation == "run" {
                        assert_eq!(
                            result.stdout,
                            if unused_generic {
                                b"0\n".as_slice()
                            } else {
                                b"42\n".as_slice()
                            }
                        );
                    }
                } else {
                    assert!(!String::from_utf8_lossy(&result.stderr).contains("E0500"));
                }
            }
        }
    }
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7"]
fn local_reference_native_programs_run_after_removing_sources_and_build_storage() {
    let evidence = std::env::var_os("OXID_LOCAL_REFERENCES_NATIVE_EVIDENCE").map(|path| {
        let path = PathBuf::from(path).join(if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        });
        assert!(
            !path.exists(),
            "evidence directory must be new: {}",
            path.display()
        );
        fs::create_dir_all(&path).unwrap();
        path
    });
    let cases = samples()
        .into_iter()
        .map(|(source, expected)| (source.to_owned(), expected))
        .chain(
            capability_cases()
                .into_iter()
                .filter(|(_, allowed)| *allowed)
                .map(|(source, _)| (source, b"42\n".as_slice())),
        );
    for (case, (source, expected)) in cases.enumerate() {
        for policy in ["bounded", "hosted"] {
            let retained = evidence.as_ref().map(|root| {
                let path = root.join(format!("case-{case}-{policy}"));
                fs::create_dir(&path).unwrap();
                fs::write(path.join("source.ox"), &source).unwrap();
                path
            });
            let input = Input::new(&source);
            fs::write(input.0.join("oxid.toml"),format!("[project]\nname=\"local-references\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
            let deployment = input.0.join("deployment");
            fs::create_dir(&deployment).unwrap();
            for profile in ["dev", "release"] {
                let built = input.invoke(&["build", "--profile", profile]);
                if let Some(path) = &retained {
                    fs::write(path.join(format!("build-{profile}.stdout")), &built.stdout).unwrap();
                    fs::write(path.join(format!("build-{profile}.stderr")), &built.stderr).unwrap();
                }
                assert!(
                    built.status.success(),
                    "{}",
                    String::from_utf8_lossy(&built.stderr)
                );
                fs::copy(
                    input.0.join(".oxid/bin/local-references"),
                    deployment.join(profile),
                )
                .unwrap();
            }
            fs::remove_file(input.0.join("main.ox")).unwrap();
            fs::remove_file(input.0.join("oxid.toml")).unwrap();
            for dir in ["cache", ".oxid"] {
                let p = input.0.join(dir);
                if p.exists() {
                    fs::remove_dir_all(p).unwrap();
                }
            }
            for profile in ["dev", "release"] {
                let actual = Command::new(deployment.join(profile))
                    .env_clear()
                    .current_dir(&deployment)
                    .output()
                    .unwrap();
                assert!(
                    actual.status.success(),
                    "{}",
                    String::from_utf8_lossy(&actual.stderr)
                );
                assert_eq!(actual.stdout, expected);
                assert!(actual.stderr.is_empty());
                if source == FILE_FIELD_SAMPLE {
                    let written = fs::read(deployment.join("local-field-output.bin")).unwrap();
                    assert_eq!(written, b"ok");
                    if let Some(path) = &retained {
                        fs::write(path.join(format!("{profile}-file-output.bin")), written)
                            .unwrap();
                    }
                }
                if let Some(path) = &retained {
                    let executable = path.join(profile);
                    fs::copy(deployment.join(profile), &executable).unwrap();
                    assert_eq!(&fs::read(&executable).unwrap()[..4], b"\x7fELF");
                    fs::write(path.join(format!("{profile}.stdout")), &actual.stdout).unwrap();
                    fs::write(path.join(format!("{profile}.stderr")), &actual.stderr).unwrap();
                    fs::write(
                        path.join(format!("{profile}.status")),
                        actual.status.code().unwrap().to_string(),
                    )
                    .unwrap();
                }
            }
        }
    }
}
