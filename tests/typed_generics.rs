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
fn explicit_type_arguments_support_parameters_absent_from_value_arguments() {
    for (source, expected) in [
        ("fn answer<T>()->i32{return 42;}fn main()->i32{return answer::<i64>();}", b"42\n".as_slice()),
        ("fn first<T,U>(left:T,right:U)->T{return left;}fn main()->i32{return first::<i32,()>(42,());}", b"42\n"),
        ("fn identity<T>(value:T)->T{return value;}fn main()->u128{return identity::<u128>(340282366920938463463374607431768211455u128);}", b"340282366920938463463374607431768211455\n"),
        ("fn identity<T>(value:T)->T{return value;}fn relay<U>(value:U)->U{return identity::<U>(value);}struct N{value:i32}fn main()->i32{let n=relay::<N>(N{value:42});return n.value;}", b"42\n"),
        ("fn identity<T>(value:T)->T{return value;}fn main()->i32{let values=identity::<[i32;2]>([20,22]);return values[0]+values[1];}", b"42\n"),
        ("struct N{value:i32}fn read<T>(value:&T)->i32{return 42;}fn main()->i32{let n=N{value:1};return read::<N>(&n);}", b"42\n"),
    ] {
        let output = Project::new(source).invoke(&["run"]);
        assert!(output.status.success(), "{source}: {output:?}");
        assert_eq!(output.stdout, expected);
        assert!(output.stderr.is_empty(), "{output:?}");
    }
}

#[test]
fn explicit_type_arguments_preserve_bounds_and_value_argument_types() {
    for (source, message) in [
        ("fn identity<T>(value:T)->T{return value;}fn main()->i32{return identity::<bool>(42);}", "incompatible types"),
        ("fn identity<T>(value:T)->T{return value;}fn main()->i32{return identity::<i32,bool>(42);}", "type argument count"),
        ("fn ordinary()->i32{return 42;}fn main()->i32{return ordinary::<i32>();}", "type argument count"),
        ("fn answer<T>()->i32{return 42;}fn main()->i32{return answer::<Unknown>();}", "Unknown"),
        ("struct N{value:i32}fn answer<T:Copy>()->i32{return 42;}fn main()->i32{return answer::<N>();}", "satisfy Copy"),
        ("fn answer<T:Copy>()->i32{return 42;}fn main()->i32{return answer::<[i32;0]>();}", "satisfy Copy"),
        ("fn answer<T:Copy>()->i32{return 42;}fn relay<U>(value:U)->i32{return answer::<U>();}fn main()->i32{return 0;}", "satisfy Copy"),
        ("enum E{N(i32)}fn main()->i32{let e=E::N::<i32>(42);return 0;}", "enum variants"),
        ("use alloc::bytes::ByteBuf;use alloc::bytes::new;fn main()->i32{new::<i32>();return 0;}", "does not have type parameters"),
    ] {
        let output = Project::new(source).invoke(&["check"]);
        assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(message) && !stderr.contains("E0500"), "{source}: {output:?}");
    }
}

#[test]
fn malformed_explicit_arguments_and_noncall_uses_are_rejected() {
    for expression in [
        "answer::<>()",
        "answer::<i32,bool>()",
        "answer::<i32",
        "answer::<i32>",
        "answer::<i32>{}",
    ] {
        let source =
            format!("fn answer<T>()->i32{{return 42;}}fn main()->i32{{return {expression};}}");
        let output = Project::new(&source).invoke(&["check"]);
        assert_eq!(output.status.code(), Some(1), "{expression}: {output:?}");
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("E0500"),
            "{output:?}"
        );
    }
}

#[test]
fn explicit_type_arguments_resolve_in_the_callers_module() {
    let project=Project::new("mod helper;struct Private{value:i32}fn main()->i32{let n=crate::helper::identity::<Private>(Private{value:42});return n.value;}");
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
    assert!(output.stderr.is_empty(), "{output:?}");
}

#[test]
fn explicit_arguments_format_with_comments_and_trailing_commas() {
    let project = Project::new(
        "fn answer<T>()->i32{return 42;}fn main()->i32{return answer :: < /* type */ i32, > ();}",
    );
    let formatted = project.invoke(&["fmt", "main.ox"]);
    assert!(formatted.status.success(), "{formatted:?}");
    let once = formatted.stdout;
    fs::write(project.0.join("main.ox"), &once).unwrap();
    assert!(
        String::from_utf8_lossy(&once).contains("answer::<"),
        "{once:?}"
    );
    assert!(
        String::from_utf8_lossy(&once).contains("/* type */"),
        "{once:?}"
    );
    let repeated = project.invoke(&["fmt", "main.ox"]);
    assert!(repeated.status.success(), "{repeated:?}");
    assert_eq!(repeated.stdout, once);
    assert!(project
        .invoke(&["fmt", "main.ox", "--check"])
        .status
        .success());
    let output = project.invoke(&["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
}

#[test]
fn explicit_and_inferred_calls_share_equal_function_instances() {
    let project=Project::new("fn identity<T>(value:T)->T{return value;}fn main()->i32{return identity(20)+identity::<i32>(22);}");
    let checked = project.invoke(&["check"]);
    assert!(checked.status.success(), "{checked:?}");
    assert!(
        String::from_utf8_lossy(&checked.stdout).contains("2 functions"),
        "{checked:?}"
    );
    let output = project.invoke(&["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]
fn native_explicit_type_arguments_survive_source_removal() {
    let project=Project::new("use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::len;struct N{value:i32}fn answer<T>()->i32{return 1;}fn identity<T>(value:T)->T{return value;}fn relay<U>(value:U)->U{return identity::<U>(value);}fn main()->i32{let n=relay::<N>(N{value:41});let bytes=identity::<ByteBuf>(new());if len(&bytes)!=0||answer::<i32>()!=answer::<u128>(){return 1;}return n.value+answer::<()>();}");
    let deployment = project.0.join("deployed");
    fs::create_dir(&deployment).unwrap();
    for profile in ["dev", "release"] {
        let built = project.invoke(&["build", "--profile", profile]);
        assert!(built.status.success(), "{built:?}");
        fs::copy(
            project.0.join(".oxid/bin/generics"),
            deployment.join(profile),
        )
        .unwrap();
    }
    for file in ["main.ox", "oxid.toml"] {
        fs::remove_file(project.0.join(file)).unwrap();
    }
    fs::remove_dir_all(project.0.join(".oxid")).unwrap();
    for profile in ["dev", "release"] {
        let output = Command::new(deployment.join(profile))
            .env_clear()
            .current_dir(&deployment)
            .output()
            .unwrap();
        assert_eq!(
            (output.status.code(), output.stdout, output.stderr),
            (Some(0), b"42\n".to_vec(), Vec::new())
        );
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

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]
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

fn disjoint_generic_samples() -> [&'static str; 5] {
    [
        "struct Cell{n:i32}struct Pair{left:Cell,right:Cell}fn change(a:&mut Cell,b:&mut Cell)->(){a.n=20;b.n=22;return;}fn forward<T>(p:&mut Pair)->i32{change(&mut *p.left,&mut *p.right);return p.left.n+p.right.n;}fn main()->i32{let mut pair=Pair{left:Cell{n:0},right:Cell{n:0}};return forward::<i32>(&mut pair);}",
        "struct Cell{n:i32}struct Pair{left:Cell,right:Cell}struct Outer{pair:Pair}fn add(a:&mut Cell,b:&Cell)->i32{a.n=a.n+b.n;return a.n;}fn forward<T>(p:&mut Outer)->i32{let r=&mut *p;return add(&mut *r.pair /* left */ .left,&*r.pair /* right */ .right);}fn main()->i32{let mut p=Outer{pair:Pair{left:Cell{n:20},right:Cell{n:22}}};return forward::<()>(&mut p);}",
        "struct Cell{n:i32}struct Pair{left:Cell,right:Cell}fn add(a:&mut Cell,n:i32)->i32{a.n=a.n+n;return a.n;}fn bump(a:&mut Cell)->i32{a.n=a.n+1;return a.n;}fn forward<T>(p:&mut Pair)->i32{let first=add(&mut *p.left,p.right.n);return add(&mut *p.left,bump(&mut *p.right))-first+19;}fn main()->i32{let mut p=Pair{left:Cell{n:20},right:Cell{n:22}};return forward::<bool>(&mut p);}",
        "struct Pair<T>{left:T,right:T}fn change(a:&mut [u8],b:&mut [u8])->i32{a[0]=20u8;b[0]=22u8;let left=a[0];let right=b[0];return left.to_i32()+right.to_i32();}fn forward<T>(p:&mut Pair<[u8;2]>)->i32{return change(&mut *p.left[0..1],&mut *p.right[1..2]);}fn main()->i32{let mut p=Pair{left:[0u8,0u8],right:[0u8,0u8]};return forward::<i128>(&mut p);}",
        "struct Empty{}struct Pair{left:Empty,right:Empty}fn hold(a:&mut Empty,b:&mut Empty)->i32{return 42;}fn forward<T>(p:&mut Pair)->i32{return hold(&mut *p.left,&mut *p.right);}fn main()->i32{let mut p=Pair{left:Empty{},right:Empty{}};return forward::<u128>(&mut p);}",
    ]
}

#[test]
fn generic_field_loans_accept_disjoint_forwarded_nested_and_slice_places() {
    for source in disjoint_generic_samples() {
        let project = Project::new(source);
        for operation in ["check", "run"] {
            let output = project.invoke(&[operation]);
            assert!(output.status.success(), "{source}: {output:?}");
            assert!(output.stderr.is_empty(), "{output:?}");
            if operation == "run" {
                assert_eq!(output.stdout, b"42\n", "{source}");
            }
        }
    }
}

#[test]
fn generic_field_loans_match_independent_prefix_and_permission_rules() {
    let mut accepted = 0;
    let mut cases = 0;
    for left in [
        "root",
        "root.branch",
        "root.branch.left",
        "root.branch.right",
        "root.other",
    ] {
        for right in [
            "root",
            "root.branch",
            "root.branch.left",
            "root.branch.right",
            "root.other",
        ] {
            for left_mutable in [false, true] {
                for right_mutable in [false, true] {
                    let overlap = left == right
                        || left.starts_with(&(right.to_owned() + "."))
                        || right.starts_with(&(left.to_owned() + "."));
                    let allowed = !overlap || (!left_mutable && !right_mutable);
                    let a = if left_mutable { "mut " } else { "" };
                    let b = if right_mutable { "mut " } else { "" };
                    let left_ty = match left {
                        "root" => "Outer",
                        "root.branch" => "Pair",
                        _ => "Cell",
                    };
                    let right_ty = match right {
                        "root" => "Outer",
                        "root.branch" => "Pair",
                        _ => "Cell",
                    };
                    let source = format!("struct Cell{{n:i32}}struct Pair{{left:Cell,right:Cell}}struct Outer{{branch:Pair,other:Cell}}fn hold(a:&{a}{left_ty},b:&{b}{right_ty})->i32{{return 42;}}fn forward<T>(root:&mut Outer)->i32{{return hold(&{a}*{left},&{b}*{right});}}fn main()->i32{{let mut p=Outer{{branch:Pair{{left:Cell{{n:20}},right:Cell{{n:22}}}},other:Cell{{n:0}}}};return forward::<i32>(&mut p);}}");
                    for instantiated in [false, true] {
                        let source = if instantiated {
                            source.clone()
                        } else {
                            source.replace("return forward::<i32>(&mut p);", "return 42;")
                        };
                        let output = Project::new(&source).invoke(&["run"]);
                        assert_eq!(output.status.success(), allowed, "{source}: {output:?}");
                        if allowed {
                            assert_eq!(output.stdout, b"42\n");
                            assert!(output.stderr.is_empty());
                        } else {
                            let message = String::from_utf8_lossy(&output.stderr);
                            assert!(
                                message.contains("E0300")
                                    && message.contains("conflicting borrows"),
                                "{source}: {output:?}"
                            );
                        }
                        cases += 1;
                        accepted += usize::from(allowed);
                    }
                }
            }
        }
    }
    assert_eq!((cases, accepted), (200, 98));
}

#[test]
fn generic_field_loans_preserve_overlapping_reads_moves_and_mutability_rejections() {
    let declarations = "struct Cell{n:i32}struct Pair{left:Cell,right:Cell}fn hold(a:&mut Cell,n:i32)->i32{return n;}fn whole(a:&mut Pair,n:i32)->i32{return n;}fn own(p:Pair)->i32{return p.left.n;}fn read(p:&Cell)->i32{return p.n;}";
    for body in [
        "return hold(&mut *p.left,p.left.n);",
        "return whole(&mut *p,p.right.n);",
        "return hold(&mut *p.left,own(p));",
        "let r=&*p;return hold(&mut *p.right,42);",
        "let r=&mut *p;return read(&*p.left);",
    ] {
        let source = format!(
            "{declarations}fn unused<T>(p:&mut Pair)->i32{{{body}}}fn main()->i32{{return 0;}}"
        );
        let output = Project::new(&source).invoke(&["check"]);
        assert!(!output.status.success(), "{source}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("E0300"),
            "{output:?}"
        );
    }
    let source = format!("{declarations}fn unused<T>(p:&Pair)->i32{{return hold(&mut *p.left,42);}}fn main()->i32{{return 0;}}");
    let output = Project::new(&source).invoke(&["check"]);
    assert!(!output.status.success(), "{output:?}");
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]
fn native_generic_field_loans_run_without_sources_and_build_storage() {
    let evidence = std::env::var_os("OXID_GENERIC_FIELD_BORROWS_NATIVE_EVIDENCE").map(|root| {
        let root = PathBuf::from(root).join(if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        });
        assert!(!root.exists(), "evidence directory must be new");
        fs::create_dir_all(&root).unwrap();
        root
    });
    for (case, source) in disjoint_generic_samples().into_iter().enumerate() {
        for policy in ["bounded", "hosted"] {
            let project = Project::new(source);
            fs::write(project.0.join("oxid.toml"), format!("[project]\nname=\"generics\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
            let deployment = project.0.join("deployed");
            fs::create_dir(&deployment).unwrap();
            for profile in ["dev", "release"] {
                let built = project.invoke(&["build", "--profile", profile]);
                if let Some(root) = &evidence {
                    let retained = root.join(format!("{case}-{policy}-{profile}"));
                    fs::create_dir(&retained).unwrap();
                    fs::write(retained.join("main.ox"), source).unwrap();
                    fs::copy(project.0.join("oxid.toml"), retained.join("oxid.toml")).unwrap();
                    fs::write(retained.join("build.stdout"), &built.stdout).unwrap();
                    fs::write(retained.join("build.stderr"), &built.stderr).unwrap();
                    fs::write(
                        retained.join("build.status"),
                        format!("{:?}\n", built.status.code()),
                    )
                    .unwrap();
                    if built.status.success() {
                        fs::copy(
                            project.0.join(".oxid/bin/generics"),
                            retained.join("program"),
                        )
                        .unwrap();
                    }
                }
                assert!(built.status.success(), "{source}: {built:?}");
                fs::copy(
                    project.0.join(".oxid/bin/generics"),
                    deployment.join(profile),
                )
                .unwrap();
            }
            for file in ["main.ox", "oxid.toml"] {
                fs::remove_file(project.0.join(file)).unwrap();
            }
            fs::remove_dir_all(project.0.join(".oxid")).unwrap();
            for profile in ["dev", "release"] {
                let output = Command::new(deployment.join(profile))
                    .env_clear()
                    .current_dir(&deployment)
                    .output()
                    .unwrap();
                if let Some(root) = &evidence {
                    let retained = root.join(format!("{case}-{policy}-{profile}"));
                    fs::write(retained.join("stdout"), &output.stdout).unwrap();
                    fs::write(retained.join("stderr"), &output.stderr).unwrap();
                    fs::write(
                        retained.join("status"),
                        format!("{:?}\n", output.status.code()),
                    )
                    .unwrap();
                }
                assert_eq!(
                    (output.status.code(), output.stdout, output.stderr),
                    (Some(0), b"42\n".to_vec(), Vec::new()),
                    "{source}"
                );
            }
        }
    }
}
