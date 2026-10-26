//! Public finite owned enum payload behavior through current project commands.
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
            "oxid-owned-enum-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("oxid.toml"), "[project]\nname = \"enum-app\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n\n[build]\nentry-mode = \"result\"\n").unwrap();
        let fixture = Self(root);
        fixture.source(source);
        fixture
    }
    fn source(&self, source: &str) {
        fs::write(self.0.join("src/main.ox"), source).unwrap();
    }
    fn command(&self, action: &str, extra: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .arg(action)
            .current_dir(&self.0)
            .args(extra)
            .output()
            .unwrap()
    }
    fn run(&self, engine: &str, expected: i32) {
        let output = if engine == "interpreter" {
            self.command("run", &["--entry-mode=result"])
        } else {
            let built = self.command("build", &["--profile", engine, "--entry-mode=result"]);
            assert!(built.status.success(), "{built:?}");
            Command::new(self.0.join(".oxid/bin/enum-app"))
                .current_dir(std::env::temp_dir())
                .env_clear()
                .output()
                .unwrap()
        };
        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        assert!(output.stderr.is_empty(), "{output:?}");
    }
    fn reject(&self, code: &str) {
        let output = self.command("check", &["--message-format=json"]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains(code), "expected {code}: {text}");
        assert!(!text.contains("E0500"), "{text}");
    }
}
impl Drop for Package {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const LIBRARY: &str = "pub struct Packet { left:i32,right:i32 }
pub enum Outcome { Value(Packet), Error(i32) }
pub fn good()->Outcome { let packet=Packet{left:19,right:23}; return Outcome::Value(packet); }
pub fn bad()->Outcome { return Outcome::Error(-7); }
pub fn relay(value:Outcome)->Outcome { let next=value; return next; }
pub fn packet_value(packet:Packet)->i32 { return packet.left+packet.right; }";
const CONSUMER: &str = "use crate::lib::model::Outcome; use crate::lib::model::packet_value;
pub fn score(value:Outcome)->i32 { match value {
Outcome::Value(packet)=>{let moved=packet;return packet_value(moved);},
Outcome::Error(code)=>{return code;},} }";
fn opaque() -> Package {
    let p = Package::new("");
    fs::create_dir_all(p.0.join("library/src")).unwrap();
    fs::write(p.0.join("library/oxid.toml"), "[project]\nname=\"enum-library\"\nversion=\"0.1.0\"\nedition=\"typed-preview\"\nentry=\"src/lib.ox\"\n").unwrap();
    fs::write(p.0.join("library/src/lib.ox"), "pub mod model;").unwrap();
    fs::write(p.0.join("library/src/model.ox"), LIBRARY).unwrap();
    fs::write(p.0.join("src/consumer.ox"), CONSUMER).unwrap();
    let manifest = p.0.join("oxid.toml");
    let contents = fs::read_to_string(&manifest).unwrap();
    fs::write(
        manifest,
        format!("{contents}\n[dependencies]\nlib=\"library\"\n"),
    )
    .unwrap();
    p
}
fn opaque_acceptance(engine: &str) {
    let p = opaque();
    if !cfg!(target_os = "linux") {
        p.reject("E0006");
        return;
    }
    for (producer, expected) in [("good", 42), ("bad", -7)] {
        p.source(&format!("mod consumer; use crate::consumer::score; use crate::lib::model::{producer}; use crate::lib::model::relay; fn main()->i32{{return score(relay({producer}()));}}"));
        let checked = p.command("check", &[]);
        assert!(checked.status.success(), "{checked:?}");
        p.run(engine, expected);
    }
}
#[test]
fn owned_enum_private_record_across_modules_and_path_dependency() {
    opaque_acceptance("interpreter");
}

const ARRAY: &str = "enum Values{Some([i32;2]),Error(i32)}
fn relay(value:Values)->Values{return value;}
fn score(value:Values)->i32{match value{Values::Some(items)=>{let moved=items;return moved[0]+moved[1];},Values::Error(code)=>{return code;},}}
fn main()->i32{let items=[19,23];return score(relay(Values::Some(items)));}";
const NESTED: &str = "struct Packet{left:i32,right:i32} enum Inner{Hit(Packet),Empty} enum Outer{Wrapped(Inner),Error(i32)}
fn relay(value:Outer)->Outer{return value;}
fn score(value:Outer)->i32{match value{Outer::Wrapped(inner)=>{match inner{Inner::Hit(packet)=>{return packet.left+packet.right;},Inner::Empty=>{return 0;},}},Outer::Error(code)=>{return code;},}}
fn main()->i32{return score(relay(Outer::Wrapped(Inner::Hit(Packet{left:19,right:23}))));}";
const ZERO: &str = "struct Empty{} enum Zero{Record(Empty),Array([i32;0]),Unit(()),None}
fn relay(value:Zero)->Zero{return value;}
fn score(value:Zero)->i32{match value{Zero::Record(empty)=>{empty;return 7;},Zero::Array(items)=>{items;return 11;},Zero::Unit(unit)=>{unit;return 24;},Zero::None=>{return 0;},}}
fn main()->i32{let empty=Empty{};let items:[i32;0]=[];return score(relay(Zero::Record(empty)))+score(relay(Zero::Array(items)))+score(relay(Zero::Unit(())))+score(relay(Zero::None));}";
const REPLACE: &str = "enum E{V([i32;2]),Error(i32)}
fn score(e:E)->i32{match e{E::V(a)=>{return a[0]+a[1];},E::Error(code)=>{return code;},}}
fn main()->i32{let mut e=E::Error(-7);e=E::V([19,23]);let first=score(e);e=E::V([3,4]);e=E::Error(-7);let second=score(e);let discarded=E::V([9,11]);discarded;return first+second+7;}";
const BYTE_ARRAY: &str = "enum E{V([u8;2])} fn byte(x:i32)->u8{return x.to_u8_checked();}
fn score(e:E)->i32{match e{E::V(a)=>{let left=a[0];let right=a[1];return left.to_i32()+right.to_i32();},}}
fn main()->i32{return score(E::V([byte(19),byte(23)]));}";
fn value_acceptance(engine: &str) {
    for source in [ARRAY, NESTED, ZERO, REPLACE, BYTE_ARRAY] {
        Package::new(source).run(engine, 42);
    }
    let hit = "Outer::Wrapped(Inner::Hit(Packet{left:19,right:23}))";
    Package::new(&NESTED.replace(hit, "Outer::Wrapped(Inner::Empty)")).run(engine, 0);
    Package::new(&NESTED.replace(hit, "Outer::Error(-7)")).run(engine, -7);
}
#[test]
fn owned_enum_array_nested_and_empty_payloads_move_and_return() {
    value_acceptance("interpreter");
}

const RECORD_CALL: &str = "struct Packet{left:i32,right:i32} enum Value{Some(Packet)}
fn make()->Packet{return Packet{left:19,right:23};}
fn score(value:Value)->i32{match value{Value::Some(packet)=>{return packet.left+packet.right;},}}";
const ARRAY_CALL: &str = "enum Value{Some([i32;2])}
fn make()->[i32;2]{return [19,23];}
fn score(value:Value)->i32{match value{Value::Some(items)=>{return items[0]+items[1];},}}";
const ENUM_CALL: &str = "struct Packet{left:i32,right:i32} enum Inner{Hit(Packet),Empty} enum Value{Some(Inner)}
fn make()->Inner{return Inner::Hit(Packet{left:19,right:23});}
fn score(value:Value)->i32{match value{Value::Some(inner)=>{match inner{Inner::Hit(packet)=>{return packet.left+packet.right;},Inner::Empty=>{return 0;},}},}}";

fn call_payload_acceptance(engine: &str, declarations: &str, grouped: bool) {
    let payload = if grouped { "((make()))" } else { "make()" };
    let p = Package::new(&format!(
        "{declarations}\nfn main()->i32{{return score(Value::Some({payload}));}}"
    ));
    let checked = p.command("check", &[]);
    assert!(
        checked.status.success(),
        "check failed: {}{}",
        String::from_utf8_lossy(&checked.stdout),
        String::from_utf8_lossy(&checked.stderr)
    );
    p.run(engine, 42);
}

#[test]
fn owned_enum_call_payload_record_direct() {
    call_payload_acceptance("interpreter", RECORD_CALL, false);
}
#[test]
fn owned_enum_call_payload_record_grouped() {
    call_payload_acceptance("interpreter", RECORD_CALL, true);
}
#[test]
fn owned_enum_call_payload_array_direct() {
    call_payload_acceptance("interpreter", ARRAY_CALL, false);
}
#[test]
fn owned_enum_call_payload_array_grouped() {
    call_payload_acceptance("interpreter", ARRAY_CALL, true);
}
#[test]
fn owned_enum_call_payload_enum_direct() {
    call_payload_acceptance("interpreter", ENUM_CALL, false);
}
#[test]
fn owned_enum_call_payload_enum_grouped() {
    call_payload_acceptance("interpreter", ENUM_CALL, true);
}

#[test]
fn owned_enum_type_ownership_coverage_and_cycle_rejections() {
    for (source, code) in [
        ("struct P{n:i32} struct Q{n:i32} enum E{V(P)} fn main()->i32{let x=E::V(Q{n:42});return 0;}","E0300"),
        ("struct P{n:i32} enum E{V(P)} fn main()->i32{let p=P{n:42};let e=E::V(p);return p.n;}","E0310"),
        ("struct P{n:i32} enum E{V(P)} fn main()->i32{let e=E::V(P{n:42});match e{E::V(p)=>{let next=p;return p.n;},}}","E0310"),
        ("struct P{n:i32} enum E{V(P),N} fn main()->i32{let e=E::N;match e{E::V(p)=>{return p.n;},}}","E0300"),
        ("struct P{n:i32} enum E{V(P),N} fn main()->i32{let p=P{n:42};let e=E::N;match e{E::V(q)=>{let moved=p;},E::N=>{},}return p.n;}","E0310"),
        ("enum E{N,V(E)} fn main()->i32{return 0;}","E0202"),
        ("enum A{N,V(B)} enum B{V(A)} fn main()->i32{return 0;}","E0202"),
        ("struct P{n:i32} pub enum E{V(P)} fn main()->i32{return 0;}","E0207"),
    ] { Package::new(source).reject(code); }
    Package::new("enum E{N}struct P{e:E}fn main()->i32{let record=P{e:E::N};let P{e}=record;match e{E::N=>{return 0;},}}")
        .run("interpreter", 0);
    let p = opaque();
    p.source("use crate::lib::model::good;use crate::lib::model::Outcome;fn main()->i32{let e=good();match e{Outcome::Value(p)=>{return p.left;},Outcome::Error(code)=>{return code;},}}");
    p.reject(if cfg!(target_os = "linux") {
        "E0206"
    } else {
        "E0006"
    });
}

fn identity_graph(separate_roots: bool) -> Package {
    let package = Package::new("use crate::left::produce; use crate::right::consume; fn main()->i32{return consume(produce());}");
    let manifest = package.0.join("oxid.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    fs::write(
        manifest,
        format!("{text}\n[dependencies]\nleft=\"left\"\nright=\"right\"\n"),
    )
    .unwrap();
    let shared = "pub struct Packet{value:i32} pub enum Outcome{Value(Packet),Empty} pub fn make()->Outcome{return Outcome::Value(Packet{value:42});} pub fn score(value:Outcome)->i32{match value{Outcome::Value(packet)=>{return packet.value;},Outcome::Empty=>{return 0;},}}";
    for (directory, source, dependency) in [
        ("shared", shared, None),
        ("copy", shared, None),
        ("left", "use crate::token::Outcome; use crate::token::make; pub fn produce()->Outcome{return make();}", Some("../shared")),
        ("right", "use crate::token::Outcome; use crate::token::score; pub fn consume(value:Outcome)->i32{return score(value);}", Some(if separate_roots { "../copy" } else { "../shared" })),
    ] {
        let root = package.0.join(directory);
        fs::create_dir_all(root.join("src")).unwrap();
        let mut manifest = "[project]\nname=\"same-name\"\nversion=\"0.1.0\"\nedition=\"typed-preview\"\nkind=\"lib\"\nentry=\"src/lib.ox\"\n".to_string();
        if let Some(path) = dependency {
            manifest.push_str(&format!("\n[dependencies]\ntoken=\"{path}\"\n"));
        }
        fs::write(root.join("oxid.toml"), manifest).unwrap();
        fs::write(root.join("src/lib.ox"), source).unwrap();
    }
    package
}

#[test]
fn owned_enum_package_diamond_shares_nominal_payload_identity() {
    let package = identity_graph(false);
    if cfg!(target_os = "linux") {
        package.run("interpreter", 42);
    } else {
        package.reject("E0006");
    }
}

#[test]
fn owned_enum_same_name_and_source_at_distinct_roots_do_not_share_identity() {
    let package = identity_graph(true);
    package.reject(if cfg!(target_os = "linux") {
        "E0300"
    } else {
        "E0006"
    });
    assert!(!package.0.join(".oxid").exists());
    assert!(!package.0.join("oxid.lock").exists());
}

#[test]
fn owned_enum_qualified_dependency_payload_uses_its_declaring_package() {
    let package = opaque();
    package.source("enum Envelope{Nested(crate::lib::model::Outcome)} fn main()->i32{let outer=Envelope::Nested(crate::lib::model::good());match outer{Envelope::Nested(inner)=>{match inner{crate::lib::model::Outcome::Value(packet)=>{return crate::lib::model::packet_value(packet);},crate::lib::model::Outcome::Error(code)=>{return code;},}}}}");
    if cfg!(target_os = "linux") {
        package.run("interpreter", 42);
    } else {
        package.reject("E0006");
    }
}

#[test]
fn owned_enum_unused_source_containment_still_enforces_the_depth_limit() {
    for depth in [64, 65] {
        let mut source = String::new();
        for id in 0..depth {
            if id + 1 == depth {
                source.push_str(&format!("enum E{id}{{None}}\n"));
            } else {
                source.push_str(&format!("enum E{id}{{Next(E{}),None}}\n", id + 1));
            }
        }
        source.push_str("fn main()->i32{return 0;}\n");
        let package = Package::new(&source);
        if depth == 64 {
            let checked = package.command("check", &[]);
            assert!(checked.status.success(), "{checked:?}");
        } else {
            package.reject("E0400");
        }
        assert!(!package.0.join(".oxid").exists());
    }
}

#[test]
#[ignore = "requires an already installed pinned LLVM toolchain; no installation or fallback"]
fn owned_enum_native_acceptance() {
    for profile in ["dev", "release"] {
        opaque_acceptance(profile);
        identity_graph(false).run(profile, 42);
        value_acceptance(profile);
        for declarations in [RECORD_CALL, ARRAY_CALL, ENUM_CALL] {
            for grouped in [false, true] {
                call_payload_acceptance(profile, declarations, grouped);
            }
        }
    }
}
