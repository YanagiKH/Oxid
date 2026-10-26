//! Generic records and enums through the public project interface.
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
            "oxid-nominal-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"),"[project]\nname=\"nominal\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n").unwrap();
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
        if cfg!(any(target_os = "linux", target_os = "macos")) {
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
fn generic_records_substitute_fields_and_nested_nominal_arguments() {
    for source in [
        "struct Cell<T>{value:T}fn main()->i32{let value=Cell::<i32>{value:42};return value.value;}",
        "struct Cell<T>{value:T}fn main()->i32{let value=Cell::<Cell<i32>>{value:Cell::<i32>{value:42}};return value.value.value;}",
        "struct Pair<A,B>{first:A,second:B}fn main()->i32{let pair=Pair::<f64,u128>{first:0.25,second:340282366920938463463374607431768211455u128};if pair.first==0.25&&pair.second==340282366920938463463374607431768211455u128{return 42;}return 1;}",
    ] {returns(source,b"42\n");}
}

#[test]
fn function_inference_recurses_through_generic_records_and_results() {
    returns("struct Cell<T>{value:T}fn make<T>(value:T)->Cell<T>{return Cell::<T>{value:value};}fn echo<T>(value:Cell<T>)->Cell<T>{return value;}fn main()->i32{let value=echo(make(42));return value.value;}",b"42\n");
}

#[test]
fn scalar_copy_bounds_permit_reads_through_generic_record_references() {
    returns("struct Cell<T>{value:T}fn read<T:Copy>(value:&Cell<T>)->T{return value.value;}fn main()->u128{let value=Cell::<u128>{value:340282366920938463463374607431768211455u128};return read(&value);}",b"340282366920938463463374607431768211455\n");
    returns("struct Cell<T>{value:T}fn read<T:Copy>(value:&Cell<T>)->T{return value.value;}fn main()->char{let value=Cell::<char>{value:'雪'};return read(&value);}","雪\n".as_bytes());
}

#[test]
fn generic_option_and_result_match_their_substituted_payload_types() {
    returns("enum Option<T>{None,Some(T)}fn wrap<T>(value:T)->Option<T>{return Option::<T>::Some(value);}fn unwrap<T>(value:Option<T>,fallback:T)->T{match value{Option::None=>{return fallback;},Option::Some(found)=>{return found;},}}fn main()->i32{return unwrap(wrap(42),0);}",b"42\n");
    returns("enum Result<T,E>{Ok(T),Err(E)}fn fail<T,E>(error:E)->Result<T,E>{return Result::<T,E>::Err(error);}fn main()->char{let value=fail::<i32,char>('雪');match value{Result::Ok(n)=>{return '?';},Result::Err(error)=>{return error;},}}","雪\n".as_bytes());
}

#[test]
fn generic_enum_payloads_transfer_owned_resources() {
    returns("use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::len;enum Option<T>{None,Some(T)}fn wrap<T>(value:T)->Option<T>{return Option::<T>::Some(value);}fn main()->i32{let value=wrap(new());match value{Option::None=>{return 1;},Option::Some(bytes)=>{return len(&bytes)+42;},}}",b"42\n");
}

#[test]
fn newly_discovered_enum_instances_preserve_builtin_enum_identity() {
    returns("use std::io::read_stdin;use std::io::ReadStatus;enum Choice<T>{Empty,Value(T)}fn choose<T>(value:T)->Choice<T>{return Choice::<T>::Value(value);}fn identity<T>(value:T)->T{return value;}fn main()->i32{let first=choose(42);let mut codes=[0];let status=identity(read_stdin(&mut codes));let second=choose(true);match status{ReadStatus::Eof(count)=>{match first{Choice::Empty=>{return 1;},Choice::Value(value)=>{return value;},}},ReadStatus::Full=>{return 2;},ReadStatus::IoError=>{return 3;},}}",b"42\n");
}

#[test]
fn equal_layouts_do_not_erase_type_argument_identity_or_order() {
    for source in [
        "struct Left{value:i32}struct Right{value:i32}struct Cell<T>{value:T}fn need(value:Cell<Left>)->i32{return value.value.value;}fn main()->i32{return need(Cell::<Right>{value:Right{value:42}});}",
        "struct Marker<A,B>{value:i32}fn need(value:Marker<i32,bool>)->i32{return value.value;}fn main()->i32{return need(Marker::<bool,i32>{value:42});}",
    ] {
        let output=Project::new(source).invoke(&["check"]);assert_eq!(output.status.code(),Some(1),"{output:?}");
        let message=String::from_utf8_lossy(&output.stderr);assert!(message.contains("mismatch")&&!message.contains("E0500"),"{output:?}");
    }
}

#[test]
fn unused_nominal_templates_still_check_names_arity_and_bounds() {
    for (source,needle) in [
        ("struct Broken<T>{value:Unknown}fn main()->i32{return 0;}","unknown"),
        ("struct Cell<T>{value:T}struct Broken<U>{value:Cell<U,U>}fn main()->i32{return 0;}","argument"),
        ("struct Broken<T:Unknown>{value:T}fn main()->i32{return 0;}","bound"),
        ("struct Cell<T:Copy>{value:T}fn unused<U>(value:Cell<U>)->(){return;}fn main()->i32{return 0;}","copy"),
    ] {
        let output=Project::new(source).invoke(&["check"]);assert_eq!(output.status.code(),Some(1),"{output:?}");
        let message=String::from_utf8_lossy(&output.stderr).to_lowercase();assert!(message.contains(needle)&&!message.contains("e0500"),"{output:?}");
    }
}

#[test]
fn generic_declarations_resolve_names_in_their_defining_module() {
    let project=Project::new("mod helper;struct Named{flag:bool}fn main()->i32{let value=crate::helper::make(42);return value.value;}");
    fs::write(project.0.join("helper.ox"),"pub struct Holder<T>{pub value:T}struct Named{value:i32}pub fn make<T>(value:T)->Holder<T>{let local=Named{value:1};return Holder::<T>{value:value};}").unwrap();
    if project.unavailable_multifile() {
        return;
    }
    let output = project.invoke(&["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
}

#[test]
fn nominal_applications_preserve_nested_owned_payloads_and_mutation() {
    returns("struct Cell<T>{value:T}enum Option<T>{None,Some(T)}fn main()->i32{let value=Option::<Cell<i32>>::Some(Cell::<i32>{value:42});match value{Option::None=>{return 1;},Option::Some(cell)=>{return cell.value;},}}",b"42\n");
    returns("enum Option<T>{None,Some(T)}fn main()->i32{let value=Option::<Option<i32>>::Some(Option::<i32>::Some(42));match value{Option::None=>{return 1;},Option::Some(inner)=>{match inner{Option::None=>{return 2;},Option::Some(found)=>{return found;},}},}}",b"42\n");
    returns("struct Cell<T>{value:T}fn set<T:Copy>(cell:&mut Cell<T>,value:T)->(){cell.value=value;return;}fn main()->i32{let mut cell=Cell::<i32>{value:1};set(&mut cell,42);return cell.value;}",b"42\n");
}

#[test]
fn public_generic_containers_can_be_used_with_private_arguments_internally() {
    let project = Project::new("mod helper;fn main()->i32{return crate::helper::answer();}");
    fs::write(project.0.join("helper.ox"),"pub enum Option<T>{None,Some(T)}pub struct Cell<T>{pub value:T}struct Hidden{value:i32}pub fn answer()->i32{let direct=Option::<Hidden>::Some(Hidden{value:1});let value=Option::<Cell<Hidden>>::Some(Cell::<Hidden>{value:Hidden{value:42}});match value{Option::None=>{return 1;},Option::Some(cell)=>{return cell.value.value;},}}").unwrap();
    if project.unavailable_multifile() {
        return;
    }
    let output = project.invoke(&["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
}

#[test]
fn public_signatures_check_private_arguments_even_when_phantom_or_unused() {
    for helper in [
        "pub struct Marker<T>{pub value:i32}struct Hidden{value:i32}pub fn leak<T>(value:Marker<Hidden>)->(){return;}",
        "pub enum Marker<T>{None,Some(T)}struct Hidden{value:i32}pub fn leak(value:Marker<Hidden>)->(){return;}",
        "pub struct Marker<T>{pub value:i32}struct Hidden{value:i32}pub enum Leak<T>{None,Some(Marker<Hidden>)}",
    ] {
        let project=Project::new("mod helper;fn main()->i32{return 0;}");
        fs::write(project.0.join("helper.ox"),helper).unwrap();
        if project.unavailable_multifile(){return;}
        let output=project.invoke(&["check"]);
        assert_eq!(output.status.code(),Some(1),"{helper}: {output:?}");
        let message=String::from_utf8_lossy(&output.stderr);
        assert!(message.contains("private")||message.contains("visible"),"{output:?}");
        assert!(!message.contains("E0500"),"{output:?}");
    }
}

#[test]
fn recursive_nominal_layouts_are_rejected_without_instantiating_a_function() {
    for source in [
        "struct Loop<T>{value:Loop<T>}fn main()->i32{return 0;}",
        "struct Cell<T>{value:T}struct Loop<T>{value:Loop<Cell<T>>}fn main()->i32{return 0;}",
        "enum Loop<T>{Next(Loop<T>)}fn main()->i32{return 0;}",
        "enum Left<T>{Next(Right<T>)}enum Right<T>{Next(Left<T>)}fn main()->i32{return 0;}",
    ] {
        let output = Project::new(source).invoke(&["check"]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let message = String::from_utf8_lossy(&output.stderr);
        assert!(
            message.contains("recursive") && !message.contains("E0500"),
            "{output:?}"
        );
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
fn native_nominal_instances_survive_source_removal_in_both_profiles() {
    for (source, policy) in [
        ("struct Cell<T>{value:T}enum Option<T>{None,Some(T)}fn wrap<T>(value:T)->Option<Cell<T>>{return Option::<Cell<T>>::Some(Cell::<T>{value:value});}fn read<T:Copy>(value:&Cell<T>)->T{return value.value;}fn main()->i32{let value=wrap(42);match value{Option::None=>{return 1;},Option::Some(cell)=>{return read(&cell);},}}", "hosted"),
        ("use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::len;enum Result<T,E>{Ok(T),Err(E)}fn keep<T>(value:T)->Result<T,i32>{return Result::<T,i32>::Ok(value);}fn main()->i32{let value=keep(new());match value{Result::Ok(bytes)=>{return len(&bytes)+42;},Result::Err(code)=>{return code;},}}", "bounded"),
    ] {
        let project=Project::new(source);
        fs::write(project.0.join("oxid.toml"),format!("[project]\nname=\"nominal\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
        let deployment=project.0.join("deployed");
        fs::create_dir(&deployment).unwrap();
        for profile in ["dev", "release"] {
            let built=project.invoke(&["build", "--profile", profile]);
            assert!(built.status.success(),"{built:?}");
            fs::copy(project.0.join(".oxid/bin/nominal"),deployment.join(profile)).unwrap();
        }
        for name in ["main.ox", "oxid.toml"] { fs::remove_file(project.0.join(name)).unwrap(); }
        fs::remove_dir_all(project.0.join(".oxid")).unwrap();
        for profile in ["dev", "release"] {
            let output=Command::new(deployment.join(profile)).env_clear().current_dir(&deployment).output().unwrap();
            assert_eq!((output.status.code(), output.stdout, output.stderr),(Some(0), b"42\n".to_vec(), Vec::new()));
        }
    }
}

#[test]
fn shared_type_argument_graphs_are_bounded_without_recursive_expansion() {
    let mut source=String::from("struct Pair<A,B>{value:i32}fn grow<T>(value:T)->Pair<T,T>{return Pair::<T,T>{value:42};}fn main()->i32{let v0=42;");
    for step in 1..=70 {
        source.push_str(&format!("let v{step}=grow(v{});", step - 1));
    }
    source.push_str("return 42;}");
    let output = Project::new(&source).invoke(&["check"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("error[E0400]"),
        "{output:?}"
    );
}

#[test]
fn type_parameter_bases_shadow_nominal_constructors_and_applications() {
    for source in [
        "struct T<U>{value:U}fn unused<T>(value:T<i32>)->(){return;}fn main()->i32{return 0;}",
        "struct T<U>{value:U}fn unused<T>()->(){let value=T::<i32>{value:1};return;}fn main()->i32{return 0;}",
        "enum T<U>{None,Some(U)}fn unused<T>()->(){let value=T::<i32>::Some(1);return;}fn main()->i32{return 0;}",
        "enum T{None}fn unused<T>()->(){let value=T::None;return;}fn main()->i32{return 0;}",
    ] {
        let output=Project::new(source).invoke(&["check"]);
        assert_eq!(output.status.code(),Some(1),"{output:?}");
        let message=String::from_utf8_lossy(&output.stderr);
        assert!(message.contains("type parameter")&&!message.contains("E0500"),"{output:?}");
    }
}
