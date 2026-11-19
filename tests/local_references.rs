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
fn samples() -> [(&'static str, &'static [u8]); 18] {
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
