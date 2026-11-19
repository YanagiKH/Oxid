//! Enum fields in owned records through the public project interface.
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
            "oxid-record-enum-field-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"),"[project]\nname=\"enum_fields\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n").unwrap();
        Self(root)
    }
    fn invoke(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
    fn unavailable_multifile(&self) -> bool {
        if cfg!(any(
            target_os = "linux",
            target_os = "macos",
            all(
                target_os = "windows",
                target_arch = "x86_64",
                target_pointer_width = "64",
                target_endian = "little"
            )
        )) {
            return false;
        }
        let output = self.invoke(&["check"]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("error[E0005] (source):"),
            "{output:?}"
        );
        true
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn returns(source: &str, expected: &[u8]) {
    let output = Project::new(source).invoke(&["run"]);
    assert!(output.status.success(), "{source}: {output:?}");
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty(), "{output:?}");
}

#[test]
fn record_enum_fields_move_into_match_bindings() {
    returns("enum Choice{Empty,Value(i32)}struct State{choice:Choice}fn main()->i32{let state=State{choice:Choice::Value(42)};let State{choice}=state;match choice{Choice::Empty=>{return 1;},Choice::Value(n)=>{return n;},}}",b"42\n");
    returns("enum Choice{Empty,Value(i32)}struct State{choice:Choice}fn main()->i32{let state=State{choice:Choice::Empty};let State{choice}=state;match choice{Choice::Empty=>{return 42;},Choice::Value(n)=>{return n;},}}",b"42\n");
}

#[test]
fn generic_records_can_store_and_return_generic_enum_values() {
    returns("enum Option<T>{None,Some(T)}struct Cell<T>{value:T}fn take<T>(cell:Cell<T>)->T{let Cell{value}=cell;return value;}fn main()->i32{let value=take(Cell::<Option<i32>>{value:Option::<i32>::Some(42)});match value{Option::None=>{return 1;},Option::Some(n)=>{return n;},}}",b"42\n");
    returns("enum Option<T>{None,Some(T)}struct Cell<T>{value:Option<T>}fn main()->i32{let cell=Cell::<u128>{value:Option::<u128>::Some(340282366920938463463374607431768211455u128)};let Cell{value}=cell;match value{Option::None=>{return 1;},Option::Some(n)=>{if n==340282366920938463463374607431768211455u128{return 42;}return 2;},}}",b"42\n");
}

#[test]
fn alternating_records_and_enums_preserve_tagged_layouts_and_scalars() {
    returns("struct Inner{letter:char}enum Leaf{Empty,Full(Inner)}struct Shell{state:Leaf,number:u128}enum Outer{Empty,Full(Shell)}fn main()->i32{let wrapped=Outer::Full(Shell{state:Leaf::Full(Inner{letter:'雪'}),number:340282366920938463463374607431768211455u128});match wrapped{Outer::Empty=>{return 1;},Outer::Full(shell)=>{let Shell{number:n,state}=shell;match state{Leaf::Empty=>{return 2;},Leaf::Full(inner)=>{if inner.letter=='雪'&&n==340282366920938463463374607431768211455u128{return 42;}return 3;},}},}}",b"42\n");
}

#[test]
fn enum_fields_transfer_owned_payloads_without_copying_resources() {
    returns("use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::len;use alloc::bytes::try_push;enum Option<T>{None,Some(T)}struct Cell<T>{value:T}fn take<T>(cell:Cell<T>)->T{let Cell{value}=cell;return value;}fn main()->i32{let mut bytes=new();try_push(&mut bytes,255u8);let option=take(Cell::<Option<ByteBuf>>{value:Option::<ByteBuf>::Some(bytes)});match option{Option::None=>{return 1;},Option::Some(value)=>{return len(&value)+41;},}}",b"42\n");
}

#[test]
fn enum_record_containment_cycles_are_source_errors_even_when_unused() {
    for source in [
        "enum Choice{Empty,Full(State)}struct State{choice:Choice}fn main()->i32{return 0;}",
        "struct First{value:Choice}enum Choice{Empty,Full(Second)}struct Second{value:First}fn main()->i32{return 0;}",
        "enum Option<T>{None,Some(T)}struct State{next:Option<State>}fn main()->i32{return 0;}",
    ] {
        let output = Project::new(source).invoke(&["check"]);
        assert_eq!(output.status.code(),Some(1),"{output:?}");
        let error=String::from_utf8_lossy(&output.stderr);
        assert!(!error.contains("E0500")&&(error.contains("cyclic")||error.contains("recursive")),"{output:?}");
    }
}

#[test]
fn enum_field_types_preserve_nominal_identity() {
    let source="enum Left{Value(i32)}enum Right{Value(i32)}struct Cell<T>{value:T}fn accept(cell:Cell<Left>)->i32{return 42;}fn main()->i32{return accept(Cell::<Right>{value:Right::Value(42)});}";
    let output = Project::new(source).invoke(&["check"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("mismatch") && !error.contains("E0500"),
        "{output:?}"
    );
}

#[test]
fn enum_fields_and_nested_arguments_respect_public_api_visibility() {
    for source in [
        "enum Hidden{Value(i32)}pub struct Public{pub value:Hidden}fn main()->i32{return 0;}",
        "enum Hidden<T>{Value(T)}pub struct Public<T>{pub value:Hidden<T>}fn main()->i32{return 0;}",
        "struct Hidden{}pub enum Option<T>{None,Some(T)}pub struct Public{pub value:Option<Hidden>}fn main()->i32{return 0;}",
        "pub enum Choice{None,Some(i32)}struct Hidden{value:Choice}pub struct Public{pub value:Hidden}fn main()->i32{return 0;}",
    ] {
        let output = Project::new(source).invoke(&["check"]);
        assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("E0207") && !error.contains("E0500"), "{source}: {output:?}");
    }
    for source in [
        "enum Hidden{Value(i32)}pub struct Public{value:Hidden}fn main()->i32{return 42;}",
        "pub enum Option<T>{None,Some(T)}pub struct Cell<T>{pub value:T}enum Hidden{Value(i32)}fn main()->i32{let cell=Cell::<Option<Hidden>>{value:Option::<Hidden>::Some(Hidden::Value(7))};return 42;}",
    ] {
        returns(source, b"42\n");
    }
}

#[test]
fn enum_fields_keep_their_defining_module_and_field_privacy() {
    let project=Project::new("mod model;fn main()->i32{let state=crate::model::make();let crate::model::State{choice}=state;match choice{crate::model::Choice::Value(n)=>{return n;},}}");
    let model = project.0.join("model.ox");
    fs::write(&model,"pub enum Choice{Value(i32)}pub struct State{pub choice:Choice}pub fn make()->State{return State{choice:Choice::Value(42)};}").unwrap();
    if project.unavailable_multifile() {
        return;
    }
    let output = project.invoke(&["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
    fs::write(&model,"pub enum Choice{Value(i32)}pub struct State{choice:Choice}pub fn make()->State{return State{choice:Choice::Value(42)};}").unwrap();
    let output = project.invoke(&["check"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("private"),
        "{output:?}"
    );
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]
fn native_enum_fields_keep_active_payloads_after_source_removal() {
    for source in [
        "enum Choice{Empty,Value(i32)}struct State{choice:Choice}fn main()->i32{let state=State{choice:Choice::Empty};let State{choice}=state;match choice{Choice::Empty=>{return 42;},Choice::Value(n)=>{return n;},}}",
        "enum Option<T>{None,Some(T)}struct Cell<T>{value:T}fn take<T>(cell:Cell<T>)->T{let Cell{value}=cell;return value;}fn main()->i32{let value=take(Cell::<Option<i32>>{value:Option::<i32>::Some(42)});match value{Option::None=>{return 1;},Option::Some(n)=>{return n;},}}",
        "struct Inner{letter:char}enum Leaf{Empty,Full(Inner)}struct Shell{state:Leaf,number:u128}enum Outer{Empty,Full(Shell)}fn main()->i32{let wrapped=Outer::Full(Shell{state:Leaf::Full(Inner{letter:'雪'}),number:340282366920938463463374607431768211455u128});match wrapped{Outer::Empty=>{return 1;},Outer::Full(shell)=>{let Shell{number:n,state}=shell;match state{Leaf::Empty=>{return 2;},Leaf::Full(inner)=>{if inner.letter=='雪'&&n==340282366920938463463374607431768211455u128{return 42;}return 3;},}},}}",
        "use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::len;use alloc::bytes::try_push;enum Option<T>{None,Some(T)}struct Cell<T>{value:T}fn take<T>(cell:Cell<T>)->T{let Cell{value}=cell;return value;}fn main()->i32{let mut bytes=new();try_push(&mut bytes,255u8);let option=take(Cell::<Option<ByteBuf>>{value:Option::<ByteBuf>::Some(bytes)});match option{Option::None=>{return 1;},Option::Some(value)=>{return len(&value)+41;},}}",
        "enum State{Empty,Full([char;2])}struct Cell{state:State}fn main()->i32{let mut cell=Cell{state:State::Full(['雪','人'])};cell=Cell{state:State::Full(['人','雪'])};let Cell{state}=cell;match state{State::Empty=>{return 1;},State::Full(letters)=>{if letters[1]=='雪'{return 42;}return 2;},}}",
        "use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::len;enum Option<T>{None,Some(T)}struct Pair{first:Option<ByteBuf>,second:Option<ByteBuf>}fn main()->i32{let mut pair=Pair{first:Option::<ByteBuf>::Some(new()),second:Option::<ByteBuf>::Some(new())};pair=Pair{first:Option::<ByteBuf>::Some(new()),second:Option::<ByteBuf>::None};let Pair{first,second:rest}=pair;match first{Option::None=>{return 1;},Option::Some(value)=>{return len(&value)+42;},}}",
    ] {
        for policy in ["bounded", "hosted"] {
            let project = Project::new(source);
            fs::write(project.0.join("oxid.toml"), format!("[project]\nname=\"enum_fields\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
            let deployment = project.0.join("deployed");
            fs::create_dir(&deployment).unwrap();
            for profile in ["dev", "release"] {
                let built = project.invoke(&["build", "--profile", profile]);
                assert!(built.status.success(), "{source} {policy} {profile}: {built:?}");
                fs::copy(project.0.join(".oxid/bin/enum_fields"), deployment.join(profile)).unwrap();
            }
            for name in ["main.ox", "oxid.toml"] { fs::remove_file(project.0.join(name)).unwrap(); }
            fs::remove_dir_all(project.0.join(".oxid")).unwrap();
            for profile in ["dev", "release"] {
                let output = Command::new(deployment.join(profile)).env_clear().current_dir(&deployment).output().unwrap();
                assert_eq!((output.status.code(), output.stdout, output.stderr), (Some(0), b"42\n".to_vec(), Vec::new()), "{source} {policy} {profile}");
            }
        }
    }
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]
fn native_enum_field_resources_are_cleaned_up_after_a_later_trap() {
    let source = "use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::try_push;enum Option<T>{None,Some(T)}struct Pair{first:Option<ByteBuf>,second:Option<ByteBuf>}fn make(n:u8)->ByteBuf{let mut bytes=new();try_push(&mut bytes,n);return bytes;}fn main()->i32{let pair=Pair{first:Option::<ByteBuf>::Some(make(7u8)),second:Option::<ByteBuf>::Some(make(9u8))};let Pair{first:a,second:b}=pair;let zero=0;return 1/zero;}";
    for policy in ["bounded", "hosted"] {
        let project = Project::new(source);
        fs::write(project.0.join("oxid.toml"), format!("[project]\nname=\"enum_fields\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
        let deployment = project.0.join("deployed");
        fs::create_dir(&deployment).unwrap();
        for profile in ["dev", "release"] {
            let built = project.invoke(&["build", "--profile", profile]);
            assert!(built.status.success(), "{built:?}");
            fs::copy(
                project.0.join(".oxid/bin/enum_fields"),
                deployment.join(profile),
            )
            .unwrap();
        }
        for name in ["main.ox", "oxid.toml"] {
            fs::remove_file(project.0.join(name)).unwrap();
        }
        fs::remove_dir_all(project.0.join(".oxid")).unwrap();
        for profile in ["dev", "release"] {
            let output = Command::new(deployment.join(profile))
                .env_clear()
                .current_dir(&deployment)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("E0607"),
                "{output:?}"
            );
        }
    }
}
