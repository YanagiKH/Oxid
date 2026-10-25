//! Hosted recursive calls retain checked arguments and call-only ownership.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Project(PathBuf);
impl Project {
    fn new(source: &str, policy: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-hosted-recursion-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), format!("[project]\nname=\"recursion\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
        Self(root)
    }

    fn invoke(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .output()
            .unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

const FACTORIAL: &str =
    "fn fact(n:i32)->i32{if n<=1{return 1;}return n*fact(n-1);}fn main()->i32{return fact(6);}";
const MUTUAL: &str = "fn even(n:i32)->bool{if n==0{return true;}return odd(n-1);}fn odd(n:i32)->bool{if n==0{return false;}return even(n-1);}fn main()->bool{return even(100);}";
const BORROWED: &str = "struct Counter{value:i32}fn count(p:&mut Counter,n:i32)->(){if n==0{return;}p.value=p.value+1;count(&mut *p,n-1);return;}fn main()->i32{let mut value=Counter{value:0};count(&mut value,100);return value.value;}";
const OWNED_BUFFER: &str = "use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::len;use alloc::bytes::try_push;use alloc::bytes::GrowStatus;fn make(n:i32)->ByteBuf{if n==0{return new();}let mut value=make(n-1);let status=try_push(&mut value,42u8);match status{GrowStatus::Ok=>{},GrowStatus::CapacityOverflow=>{},GrowStatus::AllocationFailed=>{},GrowStatus::MemoryLimit=>{},}return value;}fn main()->i32{let value=make(40);return len(&value);}";
const WIDE: &str = "fn descend(n:i32,value:u128)->u128{if n==0{return value;}return descend(n-1,value)+1u128;}fn main()->u128{return descend(40,340282366920938463463374607431768211415u128);}";
const FLOATING: &str = "fn descend(n:i32)->f64{if n==0{return 0.5;}return descend(n-1)+1.0;}fn main()->f64{return descend(40);}";
const UNUSED: &str = "fn forever(n:i32)->i32{return forever(n);}fn main()->i32{return 7;}";

fn depth(level: usize, owned: bool) -> String {
    if owned {
        format!("struct Value{{number:i32}}fn descend(p:&Value,n:i32)->i32{{if n==0{{return p.number;}}return descend(&*p,n-1);}}fn main()->i32{{let value=Value{{number:1}};return descend(&value,{level});}}")
    } else {
        format!("fn descend(n:i32)->i32{{if n==0{{return 1;}}return descend(n-1);}}fn main()->i32{{return descend({level});}}")
    }
}

fn dense_slots() -> String {
    let mut source = String::from("fn descend(n:i32)->i32{");
    for index in 0..110 {
        source.push_str(&format!("let pad{index}=n;"));
    }
    source.push_str("if n==0{return 1;}return descend(n-1);}fn main()->i32{return descend(1000);}");
    source
}

fn chain() -> String {
    let mut source = String::from("fn main()->i32{return step0();}");
    for index in 0..63 {
        source.push_str(&format!(
            "fn step{index}()->i32{{return step{}();}}",
            index + 1
        ));
    }
    source.push_str("fn step63()->i32{return 42;}");
    source
}

#[test]
fn reference_recursion_preserves_values_borrows_and_owned_returns() {
    for (source, expected) in [
        (FACTORIAL, b"720\n".as_slice()),
        (MUTUAL, b"true\n"),
        (BORROWED, b"100\n"),
        (OWNED_BUFFER, b"40\n"),
        (UNUSED, b"7\n"),
        (WIDE, b"340282366920938463463374607431768211455\n"),
        (FLOATING, b"40.5\n"),
    ] {
        let result = Project::new(source, "hosted").invoke(&["run"]);
        assert_eq!(
            (result.status.code(), result.stdout, result.stderr),
            (Some(0), expected.to_vec(), Vec::new())
        );
    }
}

#[test]
fn reference_call_limits_have_errors_at_the_attempted_call() {
    for owned in [false, true] {
        let exact = Project::new(&depth(1022, owned), "hosted").invoke(&["run"]);
        assert_eq!(
            (exact.status.code(), exact.stdout, exact.stderr),
            (Some(0), b"1\n".to_vec(), Vec::new())
        );
        let over = Project::new(&depth(1023, owned), "hosted").invoke(&["run"]);
        assert!(!over.status.success());
        assert!(String::from_utf8(over.stderr)
            .unwrap()
            .contains("error[E0602]"));
    }
    let result = Project::new(&dense_slots(), "hosted").invoke(&["run"]);
    assert!(!result.status.success());
    assert!(String::from_utf8(result.stderr)
        .unwrap()
        .contains("error[E0603]"));
}

#[test]
fn recursive_calls_cannot_alias_exclusive_arguments() {
    let source="struct Value{number:i32}fn descend(p:&mut Value,q:&mut Value,n:i32)->(){if n==0{return;}descend(&mut *p,&mut *q,n-1);return;}fn main()->i32{let mut value=Value{number:0};descend(&mut value,&mut value,2);return 0;}";
    for policy in ["bounded", "hosted"] {
        let result = Project::new(source, policy).invoke(&["check"]);
        assert!(!result.status.success());
        assert!(String::from_utf8(result.stderr)
            .unwrap()
            .contains("error[E0311]"));
    }
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn native_limited(path: &std::path::Path, cwd: &std::path::Path) -> Output {
    use std::os::unix::process::CommandExt;
    let mut command = Command::new(path);
    command.current_dir(cwd).env_clear();
    unsafe {
        command.pre_exec(|| {
            let limit = libc::rlimit {
                rlim_cur: 32 * 1024 * 1024,
                rlim_max: 32 * 1024 * 1024,
            };
            if libc::setrlimit(libc::RLIMIT_AS, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.output().unwrap()
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn deployed_outputs(project: &Project) -> Vec<Output> {
    let deployment = project.0.join("deployment");
    fs::create_dir(&deployment).unwrap();
    for profile in ["dev", "release"] {
        let result = project.invoke(&["build", "--profile", profile]);
        assert!(result.status.success(), "{result:?}");
        fs::copy(
            project.0.join(".oxid/bin/recursion"),
            deployment.join(profile),
        )
        .unwrap();
    }
    fs::remove_file(project.0.join("main.ox")).unwrap();
    fs::remove_file(project.0.join("oxid.toml")).unwrap();
    fs::remove_dir_all(project.0.join(".oxid")).unwrap();
    let cache = project.0.join("cache");
    if cache.exists() {
        fs::remove_dir_all(cache).unwrap();
    }
    ["dev", "release"]
        .map(|name| native_limited(&deployment.join(name), &deployment))
        .into_iter()
        .collect()
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]
fn native_hosted_recursion_works_without_sources() {
    for (source, expected) in [
        (FACTORIAL, b"720\n".as_slice()),
        (MUTUAL, b"true\n"),
        (BORROWED, b"100\n"),
        (OWNED_BUFFER, b"40\n"),
        (UNUSED, b"7\n"),
        (WIDE, b"340282366920938463463374607431768211455\n"),
        (FLOATING, b"40.5\n"),
    ] {
        for result in deployed_outputs(&Project::new(source, "hosted")) {
            assert_eq!(
                (result.status.code(), result.stdout, result.stderr),
                (Some(0), expected.to_vec(), Vec::new())
            );
        }
        let result = Project::new(source, "bounded").invoke(&["build"]);
        assert!(!result.status.success());
        assert!(String::from_utf8(result.stderr)
            .unwrap()
            .contains("recursive call graphs"));
    }
    let project = Project::new(&chain(), "hosted");
    for result in deployed_outputs(&project) {
        assert_eq!(
            (result.status.code(), result.stdout, result.stderr),
            (Some(0), b"42\n".to_vec(), Vec::new())
        );
    }
    let rejected = Project::new(&chain(), "bounded").invoke(&["build"]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8(rejected.stderr)
        .unwrap()
        .contains("call depth"));
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]
fn native_call_limits_match_reference_error_positions() {
    for source in [
        depth(1022, false),
        depth(1023, false),
        depth(1022, true),
        depth(1023, true),
        dense_slots(),
    ] {
        let project = Project::new(&source, "hosted");
        let reference = project.invoke(&["run"]);
        for actual in deployed_outputs(&project) {
            assert_eq!(actual.stdout, reference.stdout);
            if reference.status.success() {
                assert!(actual.status.success(), "{actual:?}");
                assert!(actual.stderr.is_empty());
            } else {
                assert!(
                    actual.status.code().is_some_and(|status| status != 0),
                    "{actual:?}"
                );
                let expected = String::from_utf8(reference.stderr.clone()).unwrap();
                let found = String::from_utf8(actual.stderr).unwrap();
                assert_eq!(found.lines().next(), expected.lines().next());
                let location = |message: &str| {
                    message
                        .lines()
                        .find(|line| line.contains("--> "))
                        .unwrap()
                        .rsplit(':')
                        .take(2)
                        .collect::<Vec<_>>()
                        .join(":")
                };
                assert_eq!(location(&found), location(&expected));
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
fn native_recursive_storage_is_reserved_before_the_call() {
    let values = vec!["0u8"; 64].join(",");
    let fields = (0..16)
        .map(|i| format!("part{i}:Block"))
        .collect::<Vec<_>>()
        .join(",");
    let initialization = (0..16)
        .map(|i| format!("part{i}:block()"))
        .collect::<Vec<_>>()
        .join(",");
    let source=format!("struct Block{{data:[u8;64]}}struct Frame{{{fields}}}fn block()->Block{{return Block{{data:[{values}]}};}}fn descend(n:i32)->i32{{let padding=Frame{{{initialization}}};if n==0{{let value=padding.part0.data[0];return value.to_i32();}}let child=descend(n-1);let value=padding.part15.data[0];return child+value.to_i32();}}fn main()->i32{{return descend(600);}}");
    for actual in deployed_outputs(&Project::new(&source, "hosted")) {
        assert!(
            actual.status.code().is_some_and(|status| status != 0),
            "{actual:?}"
        );
        assert!(actual.stdout.is_empty());
        let error = String::from_utf8(actual.stderr).unwrap();
        assert!(error.contains("error[E0605]"), "{error}");
        assert!(error.contains("live native frame bytes"), "{error}");
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
fn native_process_recursion_preserves_status_and_error_contracts() {
    for (source, status) in [(depth(15, false), 1), (OWNED_BUFFER.to_owned(), 40)] {
        let project = Project::new(&source, "hosted");
        let manifest = project.0.join("oxid.toml");
        fs::write(
            &manifest,
            fs::read_to_string(&manifest)
                .unwrap()
                .replace("entry-mode=\"result\"", "entry-mode=\"process\""),
        )
        .unwrap();
        let reference = project.invoke(&["run"]);
        assert_eq!(
            (reference.status.code(), reference.stdout, reference.stderr),
            (Some(status), Vec::new(), Vec::new())
        );
        for actual in deployed_outputs(&project) {
            assert_eq!(
                (actual.status.code(), actual.stdout, actual.stderr),
                (Some(status), Vec::new(), Vec::new())
            );
        }
    }
    for owned in [false, true] {
        let project = Project::new(&depth(1023, owned), "hosted");
        let manifest = project.0.join("oxid.toml");
        fs::write(
            &manifest,
            fs::read_to_string(&manifest)
                .unwrap()
                .replace("entry-mode=\"result\"", "entry-mode=\"process\""),
        )
        .unwrap();
        let reference = project.invoke(&["run"]);
        assert!(!reference.status.success());
        let reference_error = String::from_utf8(reference.stderr).unwrap();
        assert!(reference_error.contains("error[E0602]"));
        for actual in deployed_outputs(&project) {
            assert!(actual.status.code().is_some_and(|status| status != 0));
            assert!(actual.stdout.is_empty());
            assert_eq!(
                String::from_utf8(actual.stderr).unwrap().lines().next(),
                reference_error.lines().next()
            );
        }
    }
}
