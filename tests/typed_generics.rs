//! Generic functions and nominal types through the ordinary project interface.
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
            "oxid-generics-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"),"[project]\nname=\"generics\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n").unwrap();
        Self(root)
    }
    fn unavailable_multifile(&self) -> bool {
        if cfg!(target_os = "linux") {
            return false;
        }
        let output = self.invoke(&["check"]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        let message = String::from_utf8_lossy(&output.stderr);
        assert!(
            message.contains("error[E0005] (source):") && message.contains("qualified"),
            "{output:?}"
        );
        true
    }
    fn invoke(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn generic_identity_infers_scalar_and_nominal_owned_arguments() {
    for (source,expected) in [
        ("fn identity<T>(value:T)->T{return value;}fn main()->i32{return identity(7);}",b"7\n".as_slice()),
        ("fn identity<T>(value:T)->T{return value;}fn main()->u128{let n=340282366920938463463374607431768211455u128;return identity(n);}",b"340282366920938463463374607431768211455\n"),
        ("fn identity<T>(value:T)->T{return value;}fn main()->char{return identity('雪');}","雪\n".as_bytes()),
        ("fn identity<T>(value:T)->T{return value;}fn main()->f32{return identity(42.5f32);}",b"42.5\n"),
        ("fn identity<T>(value:T)->T{return value;}fn main()->i32{identity(());if identity(true){return 42;}return 0;}",b"42\n"),
        ("struct Number{value:i32}fn identity<T>(value:T)->T{return value;}fn main()->i32{let n=identity(Number{value:42});return n.value;}",b"42\n"),
    ] {
        let result=Project::new(source).invoke(&["run"]);assert!(result.status.success(),"{result:?}");assert_eq!(result.stdout,expected);assert!(result.stderr.is_empty(),"{result:?}");
    }
}

#[test]
fn generic_copy_bounds_and_recursive_forwarding() {
    for (source,expected) in [
        ("fn keep<T:Copy>(value:T)->T{let first=value;return value;}fn main()->i32{return keep(42);}",b"42\n".as_slice()),
        ("struct N{value:i32}fn relay<T>(value:T,n:i32)->T{if n==0{return value;}return relay(value,n-1);}fn main()->i32{let n=relay(N{value:42},12);return n.value;}",b"42\n"),
        ("fn first<T,U>(left:T,right:U)->T{return left;}fn relay<V>(value:V)->V{return first(value,true);}fn main()->char{return relay('中');}","中\n".as_bytes()),
        ("enum E{N(i32),Empty}fn identity<T>(value:T)->T{return value;}fn main()->i32{let e=identity(E::N(42));match e{E::N(n)=>{return n;},E::Empty=>{return 0;},}}",b"42\n"),
        ("use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::len;fn identity<T>(value:T)->T{return value;}fn main()->i32{let bytes=identity(new());return len(&bytes);}",b"0\n"),
    ] {
        let output=Project::new(source).invoke(&["run"]);assert!(output.status.success(),"{output:?}");assert_eq!(output.stdout,expected);assert!(output.stderr.is_empty(),"{output:?}");
    }
}

#[test]
fn generic_definitions_are_checked_even_when_unused() {
    for (source, message) in [
        (
            "fn invalid<T>(value:T)->T{let moved=value;return value;}fn main()->i32{return 0;}",
            "moved value",
        ),
        (
            "fn invalid<T:Copy>(value:T)->T{return value+value;}fn main()->i32{return 0;}",
            "type bound",
        ),
        (
            "fn invalid<T>(value:T)->T{return true;}fn main()->i32{return 0;}",
            "type mismatch",
        ),
        (
            "fn invalid<T>(value:T)->T{let x=value;}fn main()->i32{return 0;}",
            "not all paths",
        ),
        (
            "fn invalid<T,T>(value:T)->T{return value;}fn main()->i32{return 0;}",
            "duplicate",
        ),
        (
            "fn invalid<T:Unknown>(value:T)->T{return value;}fn main()->i32{return 0;}",
            "trait bound",
        ),
        (
            "fn invalid<T>(value:T)->T{let n=2147483648;return value;}fn main()->i32{return 0;}",
            "literal",
        ),
    ] {
        let output = Project::new(source).invoke(&["check"]);
        assert!(
            !output.status.success(),
            "accepted invalid definition: {source}: {output:?}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{source}: {output:?}"
        );
    }
}

#[test]
fn generic_inference_rejects_conflicting_types_and_unsatisfied_bounds() {
    for (source,message) in [
        ("fn first<T>(left:T,right:T)->T{return left;}fn main()->i32{return first(1,true);}","incompatible types"),
        ("struct N{value:i32}fn keep<T:Copy>(value:T)->T{let first=value;return value;}fn main()->i32{let n=keep(N{value:42});return n.value;}","satisfy Copy"),
        ("fn make<T>()->i32{return 0;}fn main()->i32{return make();}","cannot infer"),
    ] {
        let output=Project::new(source).invoke(&["check"]);assert!(!output.status.success(),"{output:?}");assert!(String::from_utf8_lossy(&output.stderr).contains(message),"{output:?}");
    }
}

#[test]
fn generic_names_resolve_in_the_defining_module() {
    let project = Project::new("mod helper;fn main()->i32{return crate::helper::forward(42);}");
    fs::write(project.0.join("helper.ox"),"fn identity<T>(value:T)->T{return value;}pub fn forward<T>(value:T)->T{return identity(value);}").unwrap();
    if project.unavailable_multifile() {
        return;
    }
    let output = project.invoke(&["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
}

#[test]
fn generic_calls_accept_private_types_without_exposing_them() {
    let project=Project::new("mod helper;struct Private{value:i32}fn main()->i32{let n=crate::helper::identity(Private{value:42});return n.value;}");
    fs::write(
        project.0.join("helper.ox"),
        "pub fn identity<T>(value:T)->T{return value;}",
    )
    .unwrap();
    if project.unavailable_multifile() {
        return;
    }
    let output = project.invoke(&["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires Linux x86_64 and LLVM 19.1.7"]
fn native_generic_instances_run_without_source_files() {
    for (source,policy) in [
        ("struct N{value:i32}fn relay<T>(value:T,n:i32)->T{if n==0{return value;}return relay(value,n-1);}fn main()->i32{let n=relay(N{value:42},12);let wide=relay(340282366920938463463374607431768211455u128,3);let text=relay('雪',2);let fraction=relay(0.25,4);relay((),1);if fraction!=0.25||wide!=340282366920938463463374607431768211455u128||text!='雪'{return 1;}return n.value;}","hosted"),
        ("use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::len;enum E{Empty,Bytes(ByteBuf)}fn identity<T>(value:T)->T{return value;}fn main()->i32{let bytes=identity(new());let value=identity(E::Bytes(bytes));match value{E::Empty=>{return 1;},E::Bytes(contents)=>{return len(&contents)+42;},}}","bounded"),
    ] {
        let project=Project::new(source);
        fs::write(project.0.join("oxid.toml"),format!("[project]\nname=\"generics\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
        let deployment=project.0.join("deployed");fs::create_dir(&deployment).unwrap();
        for profile in ["dev","release"] {
            let output=project.invoke(&["build","--profile",profile]);assert!(output.status.success(),"{output:?}");
            fs::copy(project.0.join(".oxid/bin/generics"),deployment.join(profile)).unwrap();
        }
        fs::remove_file(project.0.join("main.ox")).unwrap();fs::remove_file(project.0.join("oxid.toml")).unwrap();fs::remove_dir_all(project.0.join(".oxid")).unwrap();
        for profile in ["dev","release"] {
            let output=Command::new(deployment.join(profile)).env_clear().current_dir(&deployment).output().unwrap();assert_eq!((output.status.code(),output.stdout,output.stderr),(Some(0),b"42\n".to_vec(),Vec::new()));
        }
    }
}

#[test]
fn generic_definitions_reject_invalid_mutation_and_call_loans() {
    for (source,message) in [
        ("struct N{x:i32}fn unused<T>(value:T)->T{let n=N{x:1};n.x=2;return value;}fn main()->i32{return 0;}","mutable access"),
        ("struct N{x:i32}fn unused<T>(value:T,n:&N)->T{n.x=2;return value;}fn main()->i32{return 0;}","mutable access"),
        ("struct N{x:i32,y:i32}fn unused<T>(value:T)->T{let n=N{x:1,x:2};return value;}fn main()->i32{return 0;}","duplicate"),
        ("fn sink<T>(view:&T,value:T)->(){return;}fn unused<U>(value:U)->(){sink(&value,value);return;}fn main()->i32{return 0;}","borrowed"),
        ("fn sink<T>(a:&mut T,b:&mut T)->(){return;}fn unused<U>(value:&mut U)->(){sink(&mut *value,&mut *value);return;}fn main()->i32{return 0;}","conflicting borrows"),
        ("fn sink<T>(value:&mut T)->(){return;}fn unused<U>(value:&U)->(){sink(&*value);return;}fn main()->i32{return 0;}","type mismatch"),
    ] {
        let output=Project::new(source).invoke(&["check"]);assert!(!output.status.success(),"{source}: {output:?}");assert!(String::from_utf8_lossy(&output.stderr).contains(message),"{source}: {output:?}");
    }
}

#[test]
fn generic_references_forward_known_record_fields_and_slices() {
    let source="struct Inner{values:[i32;2]}struct Outer{inner:Inner}fn ignore<T>(value:&T)->(){return;}fn forward<T>(value:&T)->(){ignore(&*value);return;}fn first(value:&[i32])->i32{return value[0];}fn inspect<T>(unused:&T,outer:&Outer)->i32{forward(&*outer.inner);return first(&*outer.inner.values);}fn main()->i32{let marker=Inner{values:[1,2]};let outer=Outer{inner:Inner{values:[42,9]}};return inspect(&marker,&outer);}";
    let output = Project::new(source).invoke(&["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
}

#[test]
fn equal_type_arguments_share_instances_but_nominal_types_remain_distinct() {
    let project=Project::new("struct A{value:i32}struct B{value:i32}fn identity<T>(value:T)->T{return value;}fn main()->i32{let a=identity(A{value:10});let b=identity(B{value:20});let x=identity(5);let y=identity(7);return a.value+b.value+x+y;}");
    let output = project.invoke(&["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
    let checked = project.invoke(&["check"]);
    assert!(checked.status.success(), "{checked:?}");
    assert!(
        String::from_utf8_lossy(&checked.stdout).contains("4 functions"),
        "{checked:?}"
    );
}

#[test]
fn unused_generic_templates_do_not_change_ordinary_function_targets() {
    let project=Project::new("fn unused<T>()->i32{return 99;}fn answer()->i32{return 42;}fn main()->i32{return answer();}");
    let output = project.invoke(&["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
}

#[test]
fn generic_parameters_shadow_nominal_names_in_the_type_namespace() {
    let output=Project::new("struct T{flag:bool}fn identity<T>(value:T)->T{return value;}fn main()->i32{return identity(42);}").invoke(&["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
}

#[test]
fn generic_function_formatting_preserves_types_and_comments() {
    let project=Project::new("fn keep < T : Copy , > (value:T)->T{/* retained */let first=value;return value;}fn main()->i32{return keep(42);}");
    let formatted = project.invoke(&["fmt", "main.ox"]);
    assert!(formatted.status.success(), "{formatted:?}");
    assert!(
        String::from_utf8_lossy(&formatted.stdout).contains("fn keep<T: Copy,>(value: T)"),
        "{formatted:?}"
    );
    assert!(String::from_utf8_lossy(&formatted.stdout).contains("/* retained */"));
    fs::write(project.0.join("main.ox"), formatted.stdout).unwrap();
    let checked = project.invoke(&["fmt", "main.ox", "--check"]);
    assert!(checked.status.success(), "{checked:?}");
    let output = project.invoke(&["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
}

#[test]
fn unreachable_reinitialization_cannot_hide_a_generic_loop_move() {
    let project=Project::new("struct N{x:i32}fn sink(value:N)->(){return;}fn invalid<T>(value:T,repeat:bool)->(){let mut n=N{x:1};while repeat{sink(n);continue;n=N{x:2};}return;}fn main()->i32{return 0;}");
    let output = project.invoke(&["check"]);
    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("previous loop iteration"),
        "{output:?}"
    );
}

#[test]
fn generic_functions_keep_dependency_type_and_definition_identity() {
    let project=Project::new("use crate::left::Item as Left;use crate::right::Item as Right;use crate::left::identity as choose_left;use crate::right::identity as choose_right;fn main()->i32{let a=choose_left(Left{value:20});let b=choose_right(Right{value:22});return a.value+b.value;}");
    fs::write(project.0.join("oxid.toml"),"[project]\nname=\"generics\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[dependencies]\nleft=\"left\"\nright=\"right\"\n").unwrap();
    for package in ["left", "right"] {
        let root = project.0.join(package);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("oxid.toml"),
            format!(
                "[project]\nname=\"{package}\"\nedition=\"typed-preview\"\nentry=\"src/lib.ox\"\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join("src/lib.ox"),
            "pub struct Item{pub value:i32}pub fn identity<T>(value:T)->T{return value;}",
        )
        .unwrap();
    }
    if project.unavailable_multifile() {
        return;
    }
    let output = project.invoke(&["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
    fs::write(project.0.join("main.ox"),"use crate::left::Item as Left;use crate::right::Item as Right;use crate::left::identity;fn need(value:Left)->i32{return value.value;}fn main()->i32{return need(identity(Right{value:42}));}").unwrap();
    let output = project.invoke(&["check"]);
    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("type mismatch"),
        "{output:?}"
    );
}

#[test]
fn generic_projects_preserve_typed_shift_and_empty_array_rules() {
    let project=Project::new("fn identity<T>(value:T)->T{return value;}fn main()->u64{let empty:[u8;0]=[];let bytes=identity(empty);let n=identity(21u64);let shift=1u8;return n<<shift;}");
    let output = project.invoke(&["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
    for source in [
        "fn unused<T>(value:T)->(){let empty=[];return;}fn main()->i32{return 0;}",
        "fn sink<T>(value:&T)->(){return;}fn unused<T>(value:&T)->(){sink(value);return;}fn main()->i32{return 0;}",
    ] {
        let output=Project::new(source).invoke(&["check"]);assert!(!output.status.success(),"{source}: {output:?}");
    }
}

#[test]
fn generic_field_loans_follow_existing_owner_boundaries() {
    let source="struct N{value:i32}struct Pair{a:N,b:N}fn read(a:&mut N,b:&N)->i32{return a.value+b.value;}fn combined<T>(value:T,pair:&mut Pair,other:&N)->i32{return read(&mut *pair.a,&*other);}fn main()->i32{let mut pair=Pair{a:N{value:20},b:N{value:22}};let other=N{value:22};return combined((),&mut pair,&other);}";
    let output = Project::new(source).invoke(&["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
    let source="fn take(values:&mut[i32],number:i32)->(){return;}fn invalid<T>(value:T)->(){let mut values=[1,2];take(&mut values,values[0]);return;}fn main()->i32{return 0;}";
    let output = Project::new(source).invoke(&["check"]);
    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("exclusively borrowed"),
        "{output:?}"
    );
}

#[test]
fn generic_loops_join_continue_break_and_condition_moves() {
    for (source,message) in [
        ("fn predicate<T>(value:T)->bool{return true;}fn invalid<T>(value:T)->(){while predicate(value){}return;}fn main()->i32{return 0;}","previous loop iteration"),
        ("struct N{x:i32}fn sink(value:N)->(){return;}fn invalid<T>(value:T,repeat:bool,skip:bool)->(){let mut n=N{x:1};while repeat{if skip{sink(n);continue;}n=N{x:2};}return;}fn main()->i32{return 0;}","previous loop iteration"),
        ("struct N{x:i32}fn sink(value:N)->(){return;}fn invalid<T>(value:T,repeat:bool,skip:bool)->i32{let mut n=N{x:1};while repeat{if skip{sink(n);break;}n=N{x:2};}return n.x;}fn main()->i32{return 0;}","moved value"),
        ("fn invalid<T>(value:T)->T{return value;let n=true+1;return value;}fn main()->i32{return 0;}","type mismatch"),
    ] {
        let output=Project::new(source).invoke(&["check"]);assert!(!output.status.success(),"{source}: {output:?}");assert!(String::from_utf8_lossy(&output.stderr).contains(message),"{source}: {output:?}");
    }
    for source in [
        "struct N{x:i32}fn sink(value:N)->(){return;}fn valid<T>(value:T,repeat:bool)->(){let mut n=N{x:1};while repeat{n=N{x:2};sink(n);}return;}fn main()->i32{return 42;}",
        "fn valid<T>(value:T)->T{return value;let again=value;return again;}fn main()->i32{return 42;}",
    ] {
        let output=Project::new(source).invoke(&["check"]);assert!(output.status.success(),"{source}: {output:?}");
    }
}

#[test]
fn unsupported_generic_referents_report_source_errors() {
    for (declaration, value, message) in [
        ("enum E{Empty}", "E::Empty", "enum references"),
        ("", "42", "concrete owned type"),
    ] {
        let project=Project::new(&format!("{declaration}fn ignore<T>(value:&T)->(){{return;}}fn main()->i32{{let value={value};ignore(&value);return 0;}}"));
        let output = project.invoke(&["check"]);
        assert!(!output.status.success(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{output:?}"
        );
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("internal compiler error"),
            "{output:?}"
        );
    }
}
