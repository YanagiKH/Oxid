//! Fixed-array field slices through the public typed-preview CLI.
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
                "projected slice test command timed out: {command:?} ({} stdout bytes, {} stderr bytes)",
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
            "oxid-public-projected-slices-{}-{}",
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
        let result = bounded_output(
            Command::new(self.0.join("program"))
                .current_dir(&self.0)
                .env_clear(),
        );
        if let Some(root) = std::env::var_os("OXID_PROJECTED_NATIVE_EVIDENCE") {
            let evidence = PathBuf::from(root).join(self.0.file_name().unwrap());
            fs::create_dir_all(&evidence).unwrap();
            fs::copy(self.0.join("program"), evidence.join("program")).unwrap();
            fs::write(evidence.join("stdout"), &result.stdout).unwrap();
            fs::write(evidence.join("stderr"), &result.stderr).unwrap();
            fs::write(evidence.join("status.txt"), format!("{}\n", result.status)).unwrap();
        }
        result
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

fn assert_success(project: &Project, expected: &str) {
    let check = project.typed("check");
    assert!(check.status.success(), "{check:?}");
    let run = project.typed("run");
    assert!(run.status.success(), "{run:?}");
    assert_eq!(run.stdout, expected.as_bytes());
    assert!(run.stderr.is_empty());
}
fn success_cases() -> Vec<(String, &'static str)> {
    vec![
        (format!("{SUM} {BUMP} struct Batch{{samples:[i32;3],tag:i32}} fn main()->i32{{let mut b=Batch{{samples:[1,2,3],tag:17}};bump(&mut b.samples);return sum(&b.samples)*100+b.tag;}}"), "917\n"),
        (format!("{SUM} struct Batch{{samples:[i32;3],tag:i32}} fn main()->i32{{let b=Batch{{samples:[2,3,7],tag:41}};return sum(&b.samples)+b.tag;}}"), "53\n"),
        (format!("{SUM} {BUMP} struct Batch{{samples:[i32;2],tag:i32}} fn main()->i32{{let mut b=Batch{{samples:[2,3],tag:17}};bump(&mut b.samples);return sum(&b.samples)*100+b.tag;}}"), "717\n"),
        (format!("{SUM} {BUMP} struct Inner{{samples:[i32;2],tag:i32}} struct Batch{{head:i32,inner:Inner,tail:i32}} fn relay(p:&mut Batch)->i32{{bump(&mut *p.inner.samples);return sum(&*p.inner.samples);}} fn main()->i32{{let mut b=Batch{{head:11,inner:Inner{{samples:[3,8],tag:17}},tail:29}};let n=relay(&mut b);b.inner.samples[0]=b.inner.samples[0]+1;return n*100+b.inner.samples[0]+b.head+b.inner.tag+b.tail;}}"), "1362\n"),
        (format!("{SUM} struct Batch{{samples:[i32;2]}} fn relay(p:&Batch)->i32{{return sum(&*p.samples);}} fn main()->i32{{let b=Batch{{samples:[4,9]}};return relay(&b);}}"), "13\n"),
        ("struct B{flags:[bool;2],tag:i32} fn flip(p:&mut [bool])->(){p[1]=!p[1];return;} fn yes(p:&[bool])->bool{return p[1];} fn main()->bool{let mut b=B{flags:[true,false],tag:7};flip(&mut b.flags);return yes(&b.flags);}".into(), "true\n"),
        ("struct B{units:[();2],empty:[i32;0],tag:i32} fn touch(p:&mut [()])->i32{p[0]=();p[1]=p[0];return p.len();} fn length(p:&[i32])->i32{return p.len();} fn main()->i32{let z:[i32;0]=[];let mut b=B{units:[(),()],empty:z,tag:9};return touch(&mut b.units)+length(&b.empty)+b.tag;}".into(), "11\n"),
        ("struct B{samples:[i32;2]} fn pair(a:&[i32],b:&[i32])->i32{return a[0]+b[1];} fn main()->i32{let b=B{samples:[4,9]};return pair(&b.samples,&b.samples);}".into(), "13\n"),
        ("struct B{samples:[i32;1]} fn pair(a:&mut [i32],b:&mut [i32])->(){a[0]=a[0]+1;b[0]=b[0]+2;return;} fn main()->i32{let mut a=B{samples:[3]};let mut b=B{samples:[5]};pair(&mut a.samples,&mut b.samples);return a.samples[0]*10+b.samples[0];}".into(), "47\n"),
        ("struct B{samples:[i32;1],tag:i32} fn use_slice(n:i32,p:&mut [i32])->i32{p[0]=p[0]+n;return p[0];} fn main()->i32{let mut b=B{samples:[3],tag:5};return use_slice(b.tag,&mut b.samples);}".into(), "8\n"),
        (format!("{SUM} struct B{{samples:[i32;2]}} fn main()->i32{{let b=B{{samples:[3,5]}};return sum(&b /* path */ . samples);}}"), "8\n"),
        ("struct B{samples:[i32;1]} fn read(p:&[i32])->i32{return p[0];} fn select(p:&[i32],n:i32)->i32{return p[0]+n;} fn main()->i32{let a=B{samples:[3]};let b=B{samples:[7]};return select(&a.samples,read(&b.samples));}".into(), "10\n"),
        ("struct B{samples:[i32;1]} fn use_slice(p:&mut [i32])->(){p[0]=9;return;} fn main()->i32{let mut b=B{samples:[1]};let mut i=0;while i<3{use_slice(&mut b.samples);i=i+1;}return b.samples[0];}".into(), "9\n"),
    ]
}

#[test]
fn projected_slice_public_scalar_kinds_nested_reborrows_and_order() {
    for (source, expected) in success_cases() {
        assert_success(&Project::new(&source), expected);
    }
}

#[test]
fn projected_slice_formatter_preserves_paths_comments_and_reborrow_precedence() {
    let source = "struct B{samples:[i32;2]} fn read(p:&[i32])->i32{return p.len();} fn relay(p:&mut B)->i32{return read(&*p /* root */ .samples);} fn main()->i32{let mut b=B{samples:[3,4]};return relay(&mut b);}";
    let project = Project::new(source);
    let first = project.typed("fmt");
    assert!(first.status.success(), "{first:?}");
    let formatted = String::from_utf8(first.stdout.clone()).unwrap();
    assert!(formatted.contains("&*p /* root */ .samples"), "{formatted}");
    fs::write(project.0.join("main.ox"), &first.stdout).unwrap();
    let second = project.typed("fmt");
    assert!(second.status.success(), "{second:?}");
    assert_eq!(first.stdout, second.stdout);
    assert_success(&project, "2\n");
}

fn rejection_cases() -> Vec<(String, &'static str, &'static str)> {
    let b = "struct B{left:[i32;1],right:[i32;1],scalar:i32}";
    let init = "let mut b=B{left:[1],right:[2],scalar:3};";
    let mut cases = Vec::new();
    for (params, args) in [
        ("a:&mut [i32],b:&mut [i32]", "&mut b.left,&mut b.right"),
        ("a:&[i32],b:&mut [i32]", "&b.left,&mut b.right"),
        ("a:&mut [i32],b:&[i32]", "&mut b.left,&b.right"),
        ("a:&mut [i32],b:&B", "&mut b.left,&b"),
        ("a:&B,b:&mut [i32]", "&b,&mut b.left"),
    ] {
        cases.push((
            format!(
                "{b} fn pair({params})->(){{return;}} fn main()->(){{{init}pair({args});return;}}"
            ),
            "E0311",
            "ownership",
        ));
    }
    cases.extend([
        (format!("{b} fn use_slice(p:&mut [i32],n:i32)->(){{return;}} fn main()->(){{{init}use_slice(&mut b.left,b.scalar);return;}}"), "E0311", "ownership"),
        (format!("{b} fn consume(b:B)->i32{{return b.scalar;}} fn use_slice(p:&[i32],n:i32)->(){{return;}} fn main()->(){{{init}use_slice(&b.left,consume(b));return;}}"), "E0311", "ownership"),
        (format!("{b} fn take(p:&[i32])->(){{return;}} fn main()->(){{{init}let moved=b;take(&b.left);return;}}"), "E0310", "ownership"),
        (format!("{b} fn take(p:&mut [i32])->(){{return;}} fn main()->(){{let b=B{{left:[1],right:[2],scalar:3}};take(&mut b.left);return;}}"), "E0304", "type"),
        (format!("{b} fn take(p:&[i32])->(){{return;}} fn relay(p:&B)->(){{take(&p.left);return;}}"), "E0312", "type"),
        (format!("{b} fn take(p:&mut [i32])->(){{return;}} fn relay(p:&B)->(){{take(&mut *p.left);return;}}"), "E0313", "ownership"),
        (format!("{b} fn take(p:&[i32])->(){{return;}} fn main()->(){{{init}take(&*b.left);return;}}"), "E0312", "type"),
        (format!("{b} fn exact(p:&[i32;1])->(){{return;}} fn main()->(){{{init}exact(&b.left);return;}}"), "E0300", "type"),
        (format!("{b} fn exact(p:&mut [i32;1])->(){{return;}} fn relay(p:&mut B)->(){{exact(&mut *p.left);return;}}"), "E0300", "type"),
        (format!("{b} fn take(p:&[bool])->(){{return;}} fn main()->(){{{init}take(&b.left);return;}}"), "E0300", "type"),
        (format!("{b} fn take(p:&[i32])->(){{return;}} fn main()->(){{{init}take(&mut b.left);return;}}"), "E0300", "type"),
        (format!("{b} fn take(p:&[i32])->(){{return;}} fn main()->(){{{init}take(&b.scalar);return;}}"), "E0305", "type"),
        (format!("{b} fn take(p:&[i32])->(){{return;}} fn main()->(){{{init}take(&b.left.unknown);return;}}"), "E0305", "type"),
        (format!("{b} fn take(p:&[i32])->(){{return;}} fn main()->(){{{init}take(&b.unknown);return;}}"), "E0305", "type"),
        (format!("{b} fn main()->(){{{init}let extracted=b.left;return;}}"), "E0305", "type"),
        ("struct Inner{values:[i32;1]} struct B{inner:Inner} fn take(p:&Inner)->(){return;} fn main()->(){let b=B{inner:Inner{values:[1]}};take(&b.inner);return;}".into(), "E0305", "type"),
    ]);
    for spelling in [
        "&(b.left)",
        "&(*b).left",
        "&*(b.left)",
        "&b.left[0]",
        "&b.left.len()",
        "&make().left",
    ] {
        cases.push((
            format!("{b} fn main()->(){{{init}take({spelling});return;}}"),
            if spelling == "&b.left[0]" {
                "E0101"
            } else {
                "E0100"
            },
            "parse",
        ));
    }
    cases
}

#[test]
fn projected_slice_public_rejects_conflicts_permissions_exact_views_and_nonarray_paths() {
    for (source, code, stage) in rejection_cases() {
        let project = Project::new(&source);
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

#[cfg(target_os = "linux")]
#[test]
fn projected_slice_public_checks_private_fields_at_every_hop() {
    for (child, projection) in [
        ("pub struct B{samples:[i32;1]} pub fn make()->B{return B{samples:[7]};}", "b.samples"),
        ("pub struct Inner{pub samples:[i32;1]} pub struct B{inner:Inner} pub fn make()->B{return B{inner:Inner{samples:[7]}};}", "b.inner.samples"),
        ("pub struct Inner{samples:[i32;1]} pub struct B{pub inner:Inner} pub fn make()->B{return B{inner:Inner{samples:[7]}};}", "b.inner.samples"),
    ] {
        let project = Project::new(&format!("mod data; {SUM} fn main()->i32{{let b=crate::data::make();return sum(&{projection});}}"));
        fs::write(project.0.join("data.ox"), child).unwrap();
        let output = project.json("check");
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let diagnostic = String::from_utf8(output.stdout).unwrap();
        assert!(diagnostic.contains("private"), "{diagnostic}");
        assert!(diagnostic.contains("\"stage\":\"type\""), "{diagnostic}");
    }
}

#[test]
fn projected_slice_public_bounds_use_actual_projected_length_and_access_span() {
    for (index, length) in [("-1", 2), ("p.len()", 2), ("0", 0)] {
        let source = format!("struct B{{samples:[i32;{length}],tag:i32}} fn read(p:&[i32])->i32{{return p[{index}];}} fn main()->i32{{let a:[i32;{length}]=[{}];let b=B{{samples:a,tag:19}};return read(&b.samples);}}", "7,".repeat(length));
        let project = Project::new(&source);
        assert!(project.typed("check").status.success());
        let output = project.json("run");
        assert_eq!(output.status.code(), Some(1));
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            text.contains("\"code\":\"E0606\",\"stage\":\"oir-owned-run\""),
            "{text}"
        );
        let access = format!("p[{index}]");
        let start = source.find(&access).unwrap();
        assert!(
            text.contains(&format!(
                "\"start\":{start},\"end\":{}",
                start + access.len()
            )),
            "{text}"
        );
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires LLVM/Clang/LLD 19.1.7; run projected-slice source-free native gate explicitly"]
fn projected_slice_public_source_free_native_parity() {
    for (source, expected) in success_cases() {
        let native = Project::new(&source).native_without_sources();
        assert!(native.status.success(), "{source}: {native:?}");
        assert_eq!(native.stdout, expected.as_bytes(), "{source}");
        assert!(native.stderr.is_empty());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn projected_slice_public_nested_module_reborrow_preserves_metadata() {
    let project = Project::new("mod data; fn main()->i32{let mut b=crate::data::make();let total=crate::data::update(&mut b);return total*100+b.tag;}");
    fs::write(project.0.join("data.ox"), format!("{SUM} {BUMP} pub struct Inner{{pub samples:[i32;3]}} pub struct Batch{{pub inner:Inner,pub tag:i32}} pub fn make()->Batch{{return Batch{{inner:Inner{{samples:[1,2,3]}},tag:17}};}} pub fn update(p:&mut Batch)->i32{{bump(&mut *p.inner.samples);return sum(&*p.inner.samples);}}" )).unwrap();
    assert_success(&project, "917\n");
}
