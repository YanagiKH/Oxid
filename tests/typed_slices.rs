//! Call-only scalar slices through the public typed-preview CLI.
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn bounded_output(command: &mut Command) -> Output {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let drain = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).unwrap();
            bytes
        })
    };
    let stdout = drain(Box::new(child.stdout.take().unwrap()));
    let stderr = drain(Box::new(child.stderr.take().unwrap()));
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return Output {
                status,
                stdout: stdout.join().unwrap(),
                stderr: stderr.join().unwrap(),
            };
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!(
                "slice test command timed out: {command:?} ({} stdout bytes, {} stderr bytes)",
                stdout.join().unwrap().len(),
                stderr.join().unwrap().len()
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

struct Project(PathBuf);
impl Project {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-public-slices-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        Self(root)
    }
    fn typed(&self, operation: &str) -> Output {
        bounded_output(
            Command::new(env!("CARGO_BIN_EXE_oxid"))
                .current_dir(&self.0)
                .env_remove("OXID_PATH")
                .args([operation, "main.ox", "--edition=typed-preview"]),
        )
    }
    fn json(&self, operation: &str) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command.current_dir(&self.0).env_remove("OXID_PATH").args([
            operation,
            "main.ox",
            "--edition=typed-preview",
            "--message-format=json",
        ]);
        if operation == "compile" {
            command
                .env("OXID_LLVM_BIN", self.0.join("absent-tools"))
                .args(["--backend=llvm", "--output=program"]);
        }
        bounded_output(&mut command)
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn native_without_sources(&self) -> Output {
        let compile = bounded_output(
            Command::new(env!("CARGO_BIN_EXE_oxid"))
                .current_dir(&self.0)
                .env_remove("OXID_PATH")
                .args([
                    "compile",
                    "main.ox",
                    "--edition=typed-preview",
                    "--backend=llvm",
                    "--output=program",
                ]),
        );
        assert!(compile.status.success(), "{compile:?}");
        assert_eq!(&fs::read(self.0.join("program")).unwrap()[..4], b"\x7fELF");
        for entry in fs::read_dir(&self.0).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|extension| extension == "ox") {
                fs::remove_file(path).unwrap();
            }
        }
        bounded_output(
            Command::new(self.0.join("program"))
                .current_dir(&self.0)
                .env_clear(),
        )
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const SUM: &str = "fn sum(p:&[i32])->i32{let mut i=0;let mut total=0;while i<p.len(){total=total+p[i];i=i+1;}return total;}";
const BUMP: &str =
    "fn bump(p:&mut [i32])->(){let mut i=0;while i<p.len(){p[i]=p[i]+1;i=i+1;}return;}";
const RELAY: &str = "fn relay(p:&mut [i32])->i32{bump(&mut *p);return sum(&*p);}";

fn sum_source() -> String {
    format!("{SUM} fn main()->i32{{let a=[1,2];let b=[3,4,5];let empty:[i32;0]=[];return sum(&a)*100+sum(&b)+sum(&empty);}}")
}
fn relay_source() -> String {
    format!("{SUM} {BUMP} {RELAY} fn main()->i32{{let mut a=[1,2];let mut b=[3,4,5];let mut empty:[i32;0]=[];let first=relay(&mut a);let second=relay(&mut b);let zero=relay(&mut empty);return first*100+second+zero;}}")
}

fn assert_success(project: &Project, expected: &str) {
    let check = project.typed("check");
    assert!(check.status.success(), "{check:?}");
    let run = project.typed("run");
    assert!(run.status.success(), "{run:?}");
    assert_eq!(run.stdout, expected.as_bytes());
    assert!(run.stderr.is_empty());
}

#[test]
fn public_slice_one_sum_body_accepts_lengths_two_three_and_zero() {
    assert_success(&Project::new(&sum_source()), "312\n");
}

#[test]
fn public_slice_mutation_and_explicit_relay_produce_515() {
    let project = Project::new(&relay_source());
    assert_success(&project, "515\n");
    let output = project.json("run");
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":true,\"errors\":0,\"result\":{\"type\":\"i32\",\"value\":515}}\n");
}

fn success_cases() -> Vec<(String, &'static str)> {
    vec![
        (format!("{SUM} fn forward(p:&[i32])->i32{{return sum(&*p);}} fn main()->i32{{let a=[7,8];return forward(&a);}}"), "15\n"),
        (format!("{SUM} {BUMP} fn fixed(p:&mut [i32;2])->i32{{bump(&mut *p);return sum(&*p);}} fn main()->i32{{let mut a=[1,2];return fixed(&mut a);}}"), "5\n"),
        ("fn count(p:&[bool])->i32{let mut i=0;let mut n=0;while i<p.len(){if p[i]{n=n+1;}i=i+1;}return n;} fn flip(p:&mut [bool])->(){let mut i=0;while i<p.len(){p[i]=!p[i];i=i+1;}return;} fn main()->i32{let mut a=[true,false,true];let empty:[bool;0]=[];flip(&mut a);return count(&a)*10+count(&empty);}".into(), "10\n"),
        ("fn touch(p:&mut [()])->i32{let mut i=0;while i<p.len(){p[i]=p[i];i=i+1;}return p.len();} fn main()->i32{let mut a=[(),(),()];let mut empty:[();0]=[];return touch(&mut a)+touch(&mut empty);}".into(), "3\n"),
        ("fn first(p:&[()])->(){return p[0];} fn main()->(){let a=[()];return first(&a);}".into(), "()\n"),
        ("fn pair(a:&[i32],b:&[i32])->i32{return a.len()+b.len();} fn main()->i32{let a=[1,2,3];return pair(&a,&a);}".into(), "6\n"),
        ("fn pair(a:&[i32],b:&[i32;2])->i32{return a[0]+b[1];} fn main()->i32{let a=[7,9];return pair(&a,&a);}".into(), "16\n"),
        ("fn index(p:&mut [i32])->i32{p[0]=99;return 0;} fn assign(p:&mut [i32])->i32{p[index(&mut *p)]=p[0];return p[0];} fn main()->i32{let mut a=[7];return assign(&mut a);}".into(), "7\n"),
        ("fn read(p:&[i32])->i32{if false{return p[9];}return p.len();} fn main()->i32{let a:[i32;0]=[];return read(&a);}".into(), "0\n"),
    ]
}

#[test]
fn public_slice_scalar_elements_reborrows_aliases_and_rhs_order() {
    for (source, expected) in success_cases() {
        assert_success(&Project::new(&source), expected);
    }
}

#[test]
fn public_slice_formatter_preserves_explicit_borrows_and_idempotence() {
    let source = "fn sum(p:&[i32])->i32{return p.len();}fn relay(p:&mut [i32])->i32{return sum(&*p);}fn main()->i32{let mut a=[1,2];return relay(&mut a);}";
    let project = Project::new(source);
    let first = project.typed("fmt");
    assert!(first.status.success(), "{first:?}");
    assert_eq!(first.stdout, b"fn sum(p: &[i32]) -> i32 { return p.len(); } fn relay(p: &mut [i32]) -> i32 { return sum(&*p); } fn main() -> i32 { let mut a = [1, 2]; return relay(&mut a); }\n");
    assert!(first.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(project.0.join("main.ox")).unwrap(),
        source
    );
    fs::write(project.0.join("main.ox"), &first.stdout).unwrap();
    let second = project.typed("fmt");
    assert!(second.status.success(), "{second:?}");
    assert_eq!(first.stdout, second.stdout);
    assert_success(&project, "2\n");
}

#[cfg(target_os = "linux")]
fn module_project() -> Project {
    let project = Project::new("mod helpers; fn main()->i32{let mut a=[1,2];let mut b=[3,4,5];let mut empty:[i32;0]=[];let first=crate::helpers::relay(&mut a);let second=crate::helpers::relay(&mut b);let zero=crate::helpers::relay(&mut empty);return first*100+second+zero;}");
    fs::write(
        project.0.join("helpers.ox"),
        format!("{SUM} {BUMP} pub {RELAY}"),
    )
    .unwrap();
    project
}

#[cfg(target_os = "linux")]
#[test]
fn public_slice_linked_module_relay_produces_515() {
    assert_success(&module_project(), "515\n");
}

fn bounds_cases() -> Vec<(&'static str, &'static str)> {
    vec![
        ("fn read(p:&[i32])->i32{return p[-1];} fn main()->i32{let a=[7];return read(&a);}", "p[-1]"),
        ("fn read(p:&[i32])->i32{return p[p.len()];} fn main()->i32{let a=[7,8];return read(&a);}", "p[p.len()]"),
        ("fn read(p:&[i32])->i32{return p[0];} fn main()->i32{let a:[i32;0]=[];return read(&a);}", "p[0]"),
        ("fn write(p:&mut [i32])->(){p[-1]=9;return;} fn main()->(){let mut a=[7];write(&mut a);return;}", "p[-1]"),
        ("fn write(p:&mut [i32])->(){p[p.len()]=9;return;} fn main()->(){let mut a=[7,8];write(&mut a);return;}", "p[p.len()]"),
        ("fn write(p:&mut [()])->(){p[0]=();return;} fn main()->(){let mut a:[();0]=[];write(&mut a);return;}", "p[0]"),
        ("fn read(p:&[bool])->bool{return p[-2147483648];} fn main()->bool{let a=[true];return read(&a);}", "p[-2147483648]"),
        ("fn read(p:&[()])->(){return p[2147483647];} fn main()->(){let a=[()];return read(&a);}", "p[2147483647]"),
    ]
}
fn bounds_message(source: &str, path: &str, origin: &str) -> String {
    let start = source.find(origin).unwrap();
    let before = &source[..start];
    let line = before.bytes().filter(|&byte| byte == b'\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap().chars().count() + 1;
    format!(
        "error[E0606] (oir-owned-run): array index out of bounds\n  --> {path}:{line}:{column}\n"
    )
}

#[test]
fn public_slice_signed_bounds_fail_at_the_full_access_after_check_succeeds() {
    for (source, origin) in bounds_cases() {
        let project = Project::new(source);
        let check = project.typed("check");
        assert!(check.status.success(), "{source}: {check:?}");
        let run = project.typed("run");
        assert_eq!(run.status.code(), Some(1), "{source}: {run:?}");
        assert!(run.stdout.is_empty());
        assert_eq!(
            run.stderr,
            bounds_message(source, "main.ox", origin).as_bytes()
        );
        let json = project.json("run");
        assert_eq!(json.status.code(), Some(1), "{source}: {json:?}");
        assert!(json.stderr.is_empty());
        let text = String::from_utf8(json.stdout).unwrap();
        assert_eq!(text.matches("\"kind\":\"diagnostic\"").count(), 1, "{text}");
        assert!(
            text.contains("\"code\":\"E0606\",\"stage\":\"oir-owned-run\""),
            "{text}"
        );
        let start = source.find(origin).unwrap();
        assert!(
            text.contains(&format!(
                "\"start\":{start},\"end\":{}",
                start + origin.len()
            )),
            "{text}"
        );
        assert!(text.ends_with("{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":false,\"errors\":1,\"result\":null}\n"), "{text}");
    }
}

fn rejection_cases() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("fn pair(a:&mut [i32],b:&mut [i32])->(){return;} fn main()->(){let mut a=[1,2];pair(&mut a,&mut a);return;}", "E0311", "ownership"),
        ("fn pair(a:&[i32],b:&mut [i32])->(){return;} fn main()->(){let mut a=[1,2];pair(&a,&mut a);return;}", "E0311", "ownership"),
        ("fn pair(a:&mut [i32],b:&[i32])->(){return;} fn main()->(){let mut a=[1,2];pair(&mut a,&a);return;}", "E0311", "ownership"),
        ("fn pair(a:&mut [i32],b:&[i32;2])->(){return;} fn main()->(){let mut a=[1,2];pair(&mut a,&a);return;}", "E0311", "ownership"),
        ("fn pair(a:&[i32],b:&mut [i32;2])->(){return;} fn main()->(){let mut a=[1,2];pair(&a,&mut a);return;}", "E0311", "ownership"),
        ("fn use_slice(p:&[i32])->(){return;} fn forward(p:&[i32])->(){use_slice(p);return;}", "E0312", "type"),
        ("fn fixed(p:&[i32;2])->(){return;} fn erased(p:&[i32])->(){fixed(&*p);return;}", "E0300", "type"),
        ("fn fixed(p:&mut [i32;2])->(){return;} fn erased(p:&mut [i32])->(){fixed(&mut *p);return;}", "E0300", "type"),
        ("fn read(p:&[i32])->i32{return p[0];} fn main()->i32{let a=[true];return read(&a);}", "E0300", "type"),
        ("fn read(p:&[i32])->i32{return p.len();} fn main()->i32{let a:[bool;0]=[];return read(&a);}", "E0300", "type"),
        ("fn read(p:&[i32])->i32{return p.len();} fn main()->i32{let mut a=[1];return read(&mut a);}", "E0300", "type"),
        ("fn write(p:&mut [i32])->(){p[0]=9;return;} fn main()->(){let a=[1];write(&mut a);return;}", "E0304", "type"),
        ("fn write(p:&[i32])->(){p[0]=9;return;}", "E0313", "ownership"),
        ("fn take(p:&mut [i32])->(){return;} fn forward(p:&[i32])->(){take(&mut *p);return;}", "E0313", "ownership"),
        ("fn take(p:&mut [i32],n:i32)->(){return;} fn main()->(){let mut a=[1];take(&mut a,a.len());return;}", "E0311", "ownership"),
        ("fn read(p:&[i32])->i32{return p[false];}", "E0300", "type"),
        ("fn write(p:&mut [i32])->(){p[0]=true;return;}", "E0300", "type"),
        ("fn main()->(){let a:[i32]=[1];return;}", "E0100", "parse"),
        ("fn take(p:[i32])->(){return;}", "E0100", "parse"),
        ("fn make()->[i32]{return [1];}", "E0100", "parse"),
        ("fn escape(p:&[i32])->&[i32]{return p;}", "E0101", "parse"),
        ("fn local(p:&[i32])->(){let q=p;return;}", "E0312", "type"),
        ("struct Saved{p:&[i32]} fn main()->(){return;}", "E0101", "parse"),
        ("fn read(p:&[[i32;2]])->(){return;}", "E0101", "parse"),
        ("struct R{x:i32} fn read(p:&[R])->(){return;}", "E0101", "parse"),
        ("fn read(p:&[&i32])->(){return;}", "E0101", "parse"),
        ("fn read(p:&[i32])->i32{return p[0..1];}", "E0101", "parse"),
        ("fn read(p:&[i32])->i32{return p[..];}", "E0101", "parse"),
        ("fn read(p:&[i32])->i32{return (*p)[0];}", "E0100", "parse"),
        ("fn pair(a:&mut [i32],b:&mut [i32])->(){return;} fn relay(p:&mut [i32])->(){pair(&mut *p,&mut *p);return;}", "E0311", "ownership"),
        ("fn use_slice(p:&mut [i32],n:i32)->(){return;} fn relay(p:&mut [i32])->(){use_slice(&mut *p,p.len());return;}", "E0311", "ownership"),
        ("fn pair(a:&mut [i32],b:&[i32])->(){return;} fn main()->(){let mut a:[i32;0]=[];pair(&mut a,&a);return;}", "E0311", "ownership"),
        ("fn read(p:&[i32])->i32{return p.len();} fn main()->i32{let a=[1];let b=a;return read(&a);}", "E0310", "ownership"),
        ("fn consume(a:[i32;1])->i32{return a[0];} fn use_slice(p:&[i32],n:i32)->i32{return n;} fn main()->i32{let a=[1];return use_slice(&a,consume(a));}", "E0311", "ownership"),
        ("fn use_slice(p:&[i32])->(){return;} fn relay(p:&[i32])->(){use_slice(&p);return;}", "E0312", "type"),
    ]
}

#[test]
fn public_slice_rejects_aliases_reverse_coercion_escaping_and_unowned_unsized_values() {
    for (source, code, stage) in rejection_cases() {
        let project = Project::new(source);
        for operation in ["check", "run", "compile"] {
            let output = project.json(operation);
            assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
            assert!(output.stderr.is_empty());
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(
                text.contains(&format!("\"code\":\"{code}\",\"stage\":\"{stage}\"")),
                "{source}: {text}"
            );
            assert!(!text.contains("E0701"), "{source}: {text}");
            assert!(!project.0.join("program").exists());
        }
    }
}

#[test]
fn public_slice_backing_arrays_keep_the_inclusive_1024_element_limit() {
    for length in [1024, 1025] {
        let source = format!("fn length(p:&[i32])->i32{{return p.len();}} fn main()->i32{{let a=[{}];return length(&a);}}", "1,".repeat(length));
        let project = Project::new(&source);
        if length == 1024 {
            assert_success(&project, "1024\n");
        } else {
            let output = project.json("check");
            assert_eq!(output.status.code(), Some(1));
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(
                text.contains("\"code\":\"E0400\",\"stage\":\"parse\""),
                "{text}"
            );
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn public_slice_module_bounds_retain_child_file_and_access_span() {
    let child = "// 雪\n pub fn read(p:&[i32])->i32{return p[p.len()];}";
    let project =
        Project::new("mod child; fn main()->i32{let a=[7,8];return crate::child::read(&a);}");
    fs::write(project.0.join("child.ox"), child).unwrap();
    assert!(project.typed("check").status.success());
    let run = project.typed("run");
    assert_eq!(run.status.code(), Some(1), "{run:?}");
    assert!(run.stdout.is_empty());
    assert_eq!(
        run.stderr,
        bounds_message(child, "child.ox", "p[p.len()]").as_bytes()
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires LLVM/Clang/LLD 19.1.7; run the public borrowed-slice native gate explicitly"]
fn public_slice_source_free_native_parity() {
    let mut cases = success_cases();
    cases.extend([(sum_source(), "312\n"), (relay_source(), "515\n")]);
    for (source, expected) in cases {
        let native = Project::new(&source).native_without_sources();
        assert!(native.status.success(), "{source}: {native:?}");
        assert_eq!(native.stdout, expected.as_bytes(), "{source}");
        assert!(native.stderr.is_empty());
    }
    for (source, origin) in bounds_cases() {
        let native = Project::new(source).native_without_sources();
        assert_eq!(native.status.code(), Some(1), "{source}: {native:?}");
        assert!(native.stdout.is_empty());
        assert_eq!(
            native.stderr,
            bounds_message(source, "main.ox", origin).as_bytes()
        );
    }
    let native = module_project().native_without_sources();
    assert!(native.status.success(), "{native:?}");
    assert_eq!(native.stdout, b"515\n");
    assert!(native.stderr.is_empty());
}
