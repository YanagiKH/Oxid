//! Bounded record composition through the public source, project and formatter paths.
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
        let dir = std::env::temp_dir().join(format!(
            "oxid-record-composition-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("main.ox"), source).unwrap();
        Self(dir)
    }
    fn command(&self, operation: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command.current_dir(&self.0).env_remove("OXID_PATH").args([
            operation,
            "main.ox",
            "--edition=typed-preview",
        ]);
        command
    }
    fn run(&self, expected: &str) {
        let check = self.command("check").output().unwrap();
        assert!(check.status.success(), "{check:?}");
        let output = self.command("run").output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stdout, expected.as_bytes());
        assert!(output.stderr.is_empty(), "{output:?}");
    }
    fn reject(&self, code: &str, stage: &str) -> String {
        let output = self
            .command("check")
            .arg("--message-format=json")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let message = String::from_utf8(output.stdout).unwrap();
        assert!(
            message.contains(&format!("\"code\":\"{code}\"")),
            "{message}"
        );
        assert!(
            message.contains(&format!("\"stage\":\"{stage}\"")),
            "{message}"
        );
        message
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const PILOT: &str = "struct Batch{meta:Meta,samples:[i32;3]} struct Meta{completed:i32} \
fn relay(b:Batch)->Batch{return b;} \
fn update(b:&mut Batch)->(){let mut i=0;while i<b.samples.len(){b.samples[i]=b.samples[i]+1;b.meta.completed=b.meta.completed+1;i=i+1;}return;} \
fn inspect(b:&Batch)->i32{return b.meta.completed*100+b.samples[0]*10+b.samples[2];} \
fn read(b:&Batch)->i32{return inspect(&*b);} \
fn forward(b:&mut Batch)->(){update(&mut *b);return;} \
fn main()->i32{let m=Meta{completed:0};let samples=[1,2,3];let mut b=relay(Batch{samples:samples,meta:m});forward(&mut b);return read(&b);}";

#[test]
fn record_composition_pilot_moves_whole_roots_and_reborrows() {
    Project::new(PILOT).run("324\n");
}

#[test]
fn record_composition_constructor_order_is_written_order_and_once() {
    Project::new("struct Meta{n:i32} struct Wrap{first:Meta,second:[i32;1],last:i32} \
fn tick(p:&mut Meta)->i32{p.n=p.n+1;return p.n;} \
fn main()->i32{let mut state=Meta{n:0};let w=Wrap{last:tick(&mut state),second:[tick(&mut state)],first:Meta{n:tick(&mut state)}};return w.first.n*100+w.second[0]*10+w.last;}").run("321\n");
}

#[test]
fn record_composition_index_store_evaluates_rhs_before_index() {
    Project::new(
        "struct R{a:[i32;1]} fn index(p:&mut R)->i32{p.a[0]=99;return 0;} \
fn main()->i32{let mut r=R{a:[7]};r.a[index(&mut r)]=r.a[0];return r.a[0];}",
    )
    .run("7\n");
}

#[test]
fn record_composition_zero_arrays_empty_records_scalars_and_replacement() {
    Project::new("struct Empty{} struct Child{flag:bool,unit:()} struct R{e:Empty,child:Child,a:[bool;0],b:[();1]} \
fn main()->i32{let a:[bool;0]=[];let mut r=R{e:Empty{},child:Child{flag:false,unit:()},a:a,b:[()]};r.child.flag=true;r.b[0]=r.child.unit;let a2:[bool;0]=[];r=R{e:Empty{},child:Child{flag:r.child.flag,unit:()},a:a2,b:[()]};if r.child.flag{return r.a.len()+r.b.len();}return 0;}").run("1\n");
}

#[test]
fn record_composition_moves_are_observable_before_later_fields() {
    for source in [
        "struct C{n:i32} struct R{child:C,n:i32} fn main()->(){let c=C{n:7};let r=R{child:c,n:c.n};return;}",
        "struct R{a:[i32;1],n:i32} fn main()->(){let a=[7];let r=R{a:a,n:a[0]};return;}",
        "struct C{} struct R{a:C,b:C} fn main()->(){let c=C{};let r=R{a:c,b:c};return;}",
    ] { Project::new(source).reject("E0310", "ownership"); }
}

#[test]
fn record_composition_whole_root_availability_and_permissions() {
    for (source, code, stage) in [
        ("struct R{a:[i32;1]} fn f(p:&R)->(){p.a[0]=1;return;}", "E0313", "ownership"),
        ("struct C{n:i32} struct R{c:C} fn f(p:&R)->(){p.c.n=1;return;}", "E0313", "ownership"),
        ("struct R{a:[i32;1]} fn f()->(){let r=R{a:[1]};r.a[0]=2;return;}", "E0304", "type"),
        ("struct R{a:[i32;1]} fn f()->i32{let r=R{a:[1]};let b=r;return r.a.len();}", "E0310", "ownership"),
        ("struct R{a:[i32;1]} fn use_(p:&mut R,n:i32)->(){return;} fn f()->(){let mut r=R{a:[1]};use_(&mut r,r.a[0]);return;}", "E0311", "ownership"),
    ] { Project::new(source).reject(code, stage); }
}

#[test]
fn record_composition_exclusions_remain_explicit() {
    for (source, code, stage) in [
        (
            "struct C{} struct R{c:C} fn f(r:R)->C{return r.c;}",
            "E0305",
            "type",
        ),
        (
            "struct R{a:[i32;1]} fn f(r:R)->[i32;1]{return r.a;}",
            "E0305",
            "type",
        ),
        (
            "struct C{} struct R{c:C} fn f()->(){let mut r=R{c:C{}};r.c=C{};return;}",
            "E0305",
            "type",
        ),
        (
            "struct R{a:[i32;1]} fn f()->(){let mut r=R{a:[1]};r.a=[2];return;}",
            "E0305",
            "type",
        ),
        (
            "struct R{a:[i32;0]} fn f()->R{return R{a:[]};}",
            "E0300",
            "type",
        ),
        ("struct C{} struct R{a:[C;1]}", "E0101", "parse"),
        ("struct R{a:[[i32;1];1]}", "E0101", "parse"),
        ("struct C{} struct R{c:&C}", "E0101", "parse"),
        (
            "struct R{a:[i32;1]} fn g(a:&[i32])->(){return;} fn f(r:R)->(){g(&r.a);return;}",
            "E0101",
            "parse",
        ),
        (
            "struct C{n:i32} struct R{c:C} fn f(r:R)->i32{return (r).c.n;}",
            "E0101",
            "parse",
        ),
        (
            "struct C{n:i32} struct R{c:C} fn f()->i32{return R{c:C{n:1}}.c.n;}",
            "E0101",
            "parse",
        ),
        (
            "struct C{n:i32} struct D{n:i32} struct R{c:C} fn f()->R{return R{c:D{n:1}};}",
            "E0300",
            "type",
        ),
    ] {
        Project::new(source).reject(code, stage);
    }
}

#[test]
fn record_composition_cycles_are_rejected_even_when_unused() {
    for source in ["struct R{r:R}", "struct A{b:B} struct B{a:A}"] {
        Project::new(source).reject("E0202", "resolve");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn record_composition_project_paths_and_private_intermediate_fields() {
    let project = Project::new(
        "mod state; fn main()->i32{let b=crate::state::make();return b.meta.n+b.samples[0];}",
    );
    fs::write(project.0.join("state.ox"), "pub struct Meta{pub n:i32} pub struct Batch{pub meta:crate::state::Meta,pub samples:[i32;1]} pub fn make()->Batch{return Batch{meta:Meta{n:4},samples:[5]};}").unwrap();
    project.run("9\n");
    fs::write(project.0.join("state.ox"), "pub struct Meta{pub n:i32} pub struct Batch{meta:Meta,pub samples:[i32;1]} pub fn make()->Batch{return Batch{meta:Meta{n:4},samples:[5]};}").unwrap();
    let denied = project.reject("E0206", "type");
    assert!(denied.contains("private"));
}

#[test]
fn record_composition_formatter_round_trip_preserves_comments_and_tokens() {
    let source = PILOT
        .replace("b.samples[i]", "b /* root */ . samples /* array */ [ i ]")
        .replace("b.meta.completed", "b . meta /* hop */ . completed");
    let project = Project::new(&source);
    project.run("324\n");
    let formatted: Output = project.command("fmt").output().unwrap();
    assert!(formatted.status.success(), "{formatted:?}");
    let formatted = String::from_utf8(formatted.stdout).unwrap();
    assert!(formatted.contains("/* hop */"));
    assert!(formatted.contains("samples["));
    fs::write(project.0.join("main.ox"), &formatted).unwrap();
    project.run("324\n");
    let again = project.command("fmt").output().unwrap();
    assert!(again.status.success(), "{again:?}");
    assert_eq!(again.stdout, formatted.as_bytes());
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires the pinned LLVM 19.1.7 native toolchain"]
fn record_composition_source_free_native_pilot() {
    let project = Project::new(PILOT);
    project.run("324\n");
    let compile = project
        .command("compile")
        .args(["--backend=llvm", "--output=program"])
        .output()
        .unwrap();
    assert!(compile.status.success(), "{compile:?}");
    fs::remove_file(project.0.join("main.ox")).unwrap();
    let output = Command::new(project.0.join("program"))
        .current_dir(&project.0)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"324\n");
}

#[test]
fn record_composition_bounds_paths_and_containment_before_execution() {
    fn chain(depth: usize, leaf: &str) -> String {
        let mut source = String::new();
        for index in 0..depth - 1 {
            source.push_str(&format!("struct R{index}{{next:R{}}}", index + 1));
        }
        source.push_str(&format!("struct R{}{{leaf:{leaf}}}", depth - 1));
        source
    }
    for (leaf, suffix) in [("i32", ""), ("[i32;1]", "[0]"), ("[i32;1]", ".len()")] {
        let mut source = chain(64, leaf);
        source.push_str(&format!(
            "fn read(p:&R0)->i32{{return p.{}leaf{suffix};}}",
            "next.".repeat(63)
        ));
        let project = Project::new(&source);
        let check = project.command("check").output().unwrap();
        assert!(check.status.success(), "{check:?}");
    }
    Project::new(&chain(65, "i32")).reject("E0400", "resolve");
    for ending in [";", "=0;", "[0];", ".len();"] {
        let source = format!("fn f()->(){{p.{}leaf{ending}return;}}", "next.".repeat(64));
        Project::new(&source).reject("E0400", "parse");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn record_composition_intermediate_privacy_covers_index_length_and_write() {
    for body in [
        "return b.meta.a[0];",
        "return b.meta.a.len();",
        "b.meta.a[0]=3;return 0;",
    ] {
        let source = format!("mod state; fn main()->i32{{let mut b=crate::state::make();{body}}}");
        let project = Project::new(&source);
        fs::write(project.0.join("state.ox"), "pub struct Meta{pub a:[i32;1]} pub struct Batch{meta:Meta} pub fn make()->Batch{return Batch{meta:Meta{a:[4]}};}").unwrap();
        let denied = project.reject("E0206", "type");
        let start = source.find("b.meta").unwrap() + 2;
        assert!(
            denied.contains(&format!("\"start\":{start},\"end\":{}", start + 4)),
            "{denied}"
        );
    }
}

#[test]
fn record_composition_moves_stay_unavailable_at_control_flow_joins() {
    let declarations = "struct C{n:i32} struct R{c:C,a:[i32;1]} ";
    for body in [
        "fn f(r:R,flag:bool)->i32{if flag{let moved=r;}return r.c.n;}",
        "fn f(r:R,flag:bool)->i32{while flag{let moved=r;break;}return r.a[0];}",
        "fn f(r:R,flag:bool)->i32{while flag{let moved=r;continue;}return r.c.n;}",
        "fn f(flag:bool)->(){let mut r=R{c:C{n:0},a:[1]};if flag{let moved=r;}r.a[0]=2;return;}",
    ] {
        Project::new(&format!("{declarations}{body}")).reject("E0310", "ownership");
    }
}

#[test]
fn record_composition_reborrows_preserve_whole_root_borrow_modes() {
    for read in ["p.c.n", "p.a[0]", "p.a.len()"] {
        let source = format!("struct C{{n:i32}} struct R{{c:C,a:[i32;1]}} fn use_(r:&mut R,n:i32)->(){{return;}} fn f(p:&mut R)->(){{use_(&mut *p,{read});return;}}");
        Project::new(&source).reject("E0311", "ownership");
    }
    Project::new("struct R{a:[i32;1]} fn use_(p:&R,n:i32)->(){return;} fn mutate(p:&mut R)->i32{p.a[0]=9;return 0;} fn f(p:&mut R)->(){use_(&*p,mutate(&mut *p));return;}").reject("E0311", "ownership");
    for parameter in ["&R", "&mut R"] {
        let borrow = if parameter == "&R" { "&r" } else { "&mut r" };
        let source = format!("struct R{{a:[i32;1]}} fn sum(p:&R,n:i32)->i32{{return p.a[0]+n;}} fn f(p:{parameter})->i32{{return sum(&*p,p.a[0]);}} fn main()->i32{{let mut r=R{{a:[4]}};return f({borrow});}}");
        Project::new(&source).run("8\n");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn record_composition_documented_three_module_sample_produces_324() {
    let project = Project::new(include_str!(
        "../fixtures/typed-record-composition-samples/main.ox"
    ));
    fs::write(
        project.0.join("model.ox"),
        include_str!("../fixtures/typed-record-composition-samples/model.ox"),
    )
    .unwrap();
    fs::write(
        project.0.join("ops.ox"),
        include_str!("../fixtures/typed-record-composition-samples/ops.ox"),
    )
    .unwrap();
    project.run("324\n");
}
