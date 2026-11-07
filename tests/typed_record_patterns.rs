//! Owned record destructuring through the public project interface.
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
            "oxid-record-pattern-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"),"[project]\nname=\"patterns\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n").unwrap();
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
fn record_patterns_move_generic_fields_into_new_bindings() {
    returns("struct Cell<T>{value:T}fn take<T>(cell:Cell<T>)->T{let Cell{value}=cell;return value;}fn main()->i32{return take(Cell::<i32>{value:42});}",b"42\n");
    returns("use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::len;struct Cell<T>{value:T}fn take<T>(cell:Cell<T>)->T{let Cell{value}=cell;return value;}fn main()->i32{let bytes=take(Cell::<ByteBuf>{value:new()});return len(&bytes)+42;}",b"42\n");
}

#[test]
fn record_patterns_accept_grouped_initializers_and_owned_call_results() {
    returns("struct Cell<T>{value:T}fn make<T>(value:T)->Cell<T>{return Cell::<T>{value:value};}fn main()->i32{let Cell{value}=((make(42)));return value;}",b"42\n");
    returns("struct Cell{value:i32}fn main()->i32{let cell=Cell{value:42};let Cell{value}=((cell));return value;}",b"42\n");
}

#[test]
fn record_patterns_bind_names_independently_of_field_order() {
    returns("struct Pair<A,B>{first:A,second:B}fn main()->i32{let pair=Pair::<i32,bool>{first:42,second:true};let Pair{second:flag,first:value}=pair;if flag{return value;}return 0;}",b"42\n");
}

#[test]
fn record_patterns_preserve_nested_records_arrays_and_wide_scalar_bits() {
    returns("struct Cell<T>{value:T}struct Pair<A,B>{first:A,second:B}fn main()->i32{let pair=Pair::<Cell<i32>,[i32;2]>{first:Cell::<i32>{value:40},second:[1,2]};let Pair{second:array,first:cell}=pair;let Cell{value}=cell;return value+array[1];}",b"42\n");
    returns("struct Pair<A,B>{first:A,second:B}fn main()->i32{let pair=Pair::<u128,char>{first:340282366920938463463374607431768211455u128,second:'雪'};let Pair{second:letter,first:bits}=pair;if letter=='雪'&&bits==340282366920938463463374607431768211455u128{return 42;}return 1;}",b"42\n");
}

#[test]
fn destructuring_consumes_the_record_and_does_not_copy_owned_fields() {
    for source in [
        "struct Cell<T>{value:T}fn bad<T>(cell:Cell<T>)->T{let Cell{value}=cell;let again=cell;return value;}fn main()->i32{return 0;}",
        "struct Cell<T>{value:T}fn bad<T>(cell:Cell<T>)->T{let Cell{value}=cell;let again=value;return value;}fn main()->i32{return 0;}",
    ] {
        let output=Project::new(source).invoke(&["check"]);
        assert_eq!(output.status.code(),Some(1),"{output:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("moved"),"{output:?}");
    }
}

#[test]
fn fieldless_patterns_consume_empty_records_without_allocating_fields() {
    returns(
        "struct Empty{}fn main()->i32{let value=Empty{};let Empty{}=value;return 42;}",
        b"42\n",
    );
}

#[test]
fn destructuring_transfers_one_resource_and_cleans_up_the_other() {
    returns("use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::len;struct Pair<A,B>{first:A,second:B}fn first<A,B>(pair:Pair<A,B>)->A{let Pair{second:rest,first:value}=pair;return value;}fn main()->i32{let pair=Pair::<ByteBuf,ByteBuf>{first:new(),second:new()};let value=first(pair);return len(&value)+42;}",b"42\n");
}

#[test]
fn record_patterns_respect_field_privacy_in_the_requesting_module() {
    let project=Project::new("mod model;fn main()->i32{let cell=crate::model::make();let crate::model::Cell{value}=cell;return value;}");
    fs::write(
        project.0.join("model.ox"),
        "pub struct Cell<T>{value:T}pub fn make()->Cell<i32>{return Cell::<i32>{value:42};}",
    )
    .unwrap();
    if project.unavailable_multifile() {
        return;
    }
    let output = project.invoke(&["check"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("private"),
        "{output:?}"
    );
}

#[test]
fn incomplete_duplicate_and_borrowed_patterns_report_source_errors() {
    for (source, message) in [
        ("struct Pair{first:i32,second:i32}fn main()->i32{let pair=Pair{first:1,second:2};let Pair{first}=pair;return first;}","every field"),
        ("struct Pair{first:i32,second:i32}fn main()->i32{let pair=Pair{first:1,second:2};let Pair{first:a,first:b}=pair;return a;}","duplicate"),
        ("struct Pair{first:i32,second:i32}fn main()->i32{let pair=Pair{first:1,second:2};let Pair{first:a,second:a}=pair;return a;}","duplicate"),
        ("struct Cell<T>{value:T}fn bad<T>(cell:&Cell<T>)->(){let Cell{value}=cell;return;}fn main()->i32{return 0;}","reference"),
    ] {
        let output=Project::new(source).invoke(&["check"]);
        assert_eq!(output.status.code(),Some(1),"{output:?}");
        let stderr=String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(message)&&!stderr.contains("E0500"),"{output:?}");
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
fn native_record_patterns_transfer_resources_after_source_removal() {
    for source in [
        "struct Empty{}fn main()->i32{let empty=Empty{};let Empty{}=empty;return 42;}",
        "struct Pair<A,B>{first:A,second:B}fn main()->i32{let pair=Pair::<u128,char>{first:340282366920938463463374607431768211455u128,second:'雪'};let Pair{second:letter,first:bits}=pair;if letter=='雪'&&bits==340282366920938463463374607431768211455u128{return 42;}return 1;}",
        "struct Cell<T>{value:T}struct Pair<A,B>{first:A,second:B}fn main()->i32{let pair=Pair::<Cell<i32>,[i32;2]>{first:Cell::<i32>{value:40},second:[1,2]};let Pair{second:array,first:cell}=pair;let Cell{value}=cell;return value+array[1];}",
        "use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::try_push;use alloc::bytes::len;struct Pair<A,B>{first:A,second:B}fn choose<A,B>(pair:Pair<A,B>,flag:bool)->A{let Pair{second:rest,first:value}=pair;if flag{return value;}return value;}fn main()->i32{let mut first=new();let mut second=new();try_push(&mut first,40u8);try_push(&mut second,2u8);let value=choose(Pair::<ByteBuf,ByteBuf>{first:first,second:second},true);return len(&value)+41;}",
    ] {
        for policy in ["bounded", "hosted"] {
            let project = Project::new(source);
            fs::write(project.0.join("oxid.toml"),format!("[project]\nname=\"patterns\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
            let deployment = project.0.join("deployed");
            fs::create_dir(&deployment).unwrap();
            for profile in ["dev", "release"] {
                let built = project.invoke(&["build", "--profile", profile]);
                assert!(built.status.success(), "{source}: {built:?}");
                fs::copy(project.0.join(".oxid/bin/patterns"), deployment.join(profile)).unwrap();
            }
            for name in ["main.ox", "oxid.toml"] {
                fs::remove_file(project.0.join(name)).unwrap();
            }
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
fn native_record_pattern_traps_keep_clean_output_after_source_removal() {
    let source = "use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::try_push;struct Pair{first:ByteBuf,second:ByteBuf}fn make(n:u8)->ByteBuf{let mut value=new();try_push(&mut value,n);return value;}fn main()->i32{let pair=Pair{first:make(40u8),second:make(2u8)};let Pair{first:a,second:b}=pair;let zero=0;return 1/zero;}";
    for policy in ["bounded", "hosted"] {
        let project = Project::new(source);
        fs::write(project.0.join("oxid.toml"), format!("[project]\nname=\"patterns\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
        let deployment = project.0.join("deployed");
        fs::create_dir(&deployment).unwrap();
        for profile in ["dev", "release"] {
            let built = project.invoke(&["build", "--profile", profile]);
            assert!(built.status.success(), "{built:?}");
            fs::copy(
                project.0.join(".oxid/bin/patterns"),
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
