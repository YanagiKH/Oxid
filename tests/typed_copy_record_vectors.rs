//! Copy record vector contracts through ordinary project commands.
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn output(command: &mut Command) -> Output {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let drain = |mut stream: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            bytes
        })
    };
    let stdout = drain(Box::new(child.stdout.take().unwrap()));
    let stderr = drain(Box::new(child.stderr.take().unwrap()));
    let deadline = Instant::now() + Duration::from_secs(60);
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
                "Copy vector command timed out: {command:?}; stdout={:?}; stderr={:?}",
                stdout.join().unwrap(),
                stderr.join().unwrap()
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

struct Project(PathBuf);
impl Project {
    fn new(body: &str, policy: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-copy-record-vector-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), format!("{API}\n{body}")).unwrap();
        fs::write(root.join("oxid.toml"), format!(
            "[project]\nname = \"copy-vector-contracts\"\nedition = \"typed-preview\"\nentry = \"main.ox\"\n[build]\nentry-mode = \"result\"\n[execution]\npolicy = \"{policy}\"\n"
        )).unwrap();
        Self(root)
    }
    fn invoke(&self, arguments: &[&str]) -> Output {
        output(
            Command::new(env!("CARGO_BIN_EXE_oxid"))
                .current_dir(&self.0)
                .env_remove("OXID_PATH")
                .env("OXID_CACHE_DIR", self.0.join("cache"))
                .args(arguments),
        )
    }
    fn no_artifacts(&self) {
        for name in [".oxid", "oxid.lock", "cache"] {
            assert!(!self.0.join(name).exists(), "unexpected {name}");
        }
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("Copy record vector failure artifacts: {}", self.0.display());
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

const API: &str = r#"
use alloc::vec::Vec;
use alloc::vec::GrowStatus;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use alloc::vec::try_reserve;
use alloc::vec::try_push;
use alloc::vec::try_extend;
use alloc::vec::try_resize;
use alloc::vec::truncate;
use alloc::vec::take;
#[derive(Copy)]
struct P { x: i32, y: i32 }
fn grown(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}
fn overflow(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return false; },
        GrowStatus::CapacityOverflow => { return true; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}
"#;

const OPERATIONS: &str = r#"
fn main() -> i32 {
    let mut p = P { x: 3, y: 4 };
    let mut values = new::<P>();
    if len(&values) != 0 || capacity(&values) != 0 { return -1; }
    if !grown(try_reserve(&mut values, 3)) { return -2; }
    if len(&values) != 0 || capacity(&values) < 3 { return -3; }
    if !grown(try_push(&mut values, p)) { return -4; }
    p.x = 30;
    if !grown(try_resize(&mut values, 3, p)) { return -5; }
    let first = values[0];
    let mut detached = values[1];
    detached.x = 8;
    let original_fill = values[1];
    if first.x != 3 || first.y != 4 || original_fill.x != 30 { return -6; }
    let old_capacity = capacity(&values);
    truncate(&mut values, 2);
    truncate(&mut values, 5);
    if len(&values) != 2 || capacity(&values) != old_capacity { return -7; }
    let moved = take(&mut values);
    if len(&values) != 0 || capacity(&values) != 0 { return -8; }
    let mut copied = new::<P>();
    if !grown(try_extend(&mut copied, &moved)) { return -9; }
    copied[0] = P { x: 11, y: 2 };
    let copied_capacity = capacity(&copied);
    if !grown(try_resize(&mut copied, 1, p)) { return -10; }
    if len(&copied) != 1 || capacity(&copied) != copied_capacity { return -11; }
    let changed = copied[0];
    if changed.x != 11 || changed.y != 2 || detached.x != 8 { return -12; }
    let left = moved[0];
    let right = moved[1];
    return left.x + left.y + right.x + right.y + p.x + p.y;
}
"#;

const FAILED_GROWTH: &str = r#"
fn main() -> i32 {
    let p = P { x: 20, y: 2 };
    let q = P { x: 7, y: 3 };
    let mut values = new::<P>();
    if !grown(try_push(&mut values, p)) { return -1; }
    if !grown(try_push(&mut values, q)) { return -2; }
    let old_capacity = capacity(&values);
    if !overflow(try_reserve(&mut values, 2147483647)) { return -3; }
    if len(&values) != 2 || capacity(&values) != old_capacity { return -4; }
    let first = values[0];
    let second = values[1];
    if first.x != p.x || first.y != p.y || second.x != q.x || second.y != q.y {
        return -5;
    }
    return len(&values) * 100 + first.x * 10 + second.y;
}
"#;

const INDEX_COPIES: &str = r#"
fn replace_first(values: &mut [P], value: P) -> i32 {
    values[0] = value;
    let stored = values[0];
    return stored.x + value.x;
}
fn main() -> i32 {
    let mut p = P { x: 20, y: 2 };
    let q = P { x: 7, y: 3 };
    let mut values = new::<P>();
    if !grown(try_push(&mut values, p)) { return -1; }
    if !grown(try_push(&mut values, q)) { return -2; }
    let mut detached = values[0];
    detached.x = 100;
    let unchanged = values[0];
    if unchanged.x != 20 || unchanged.y != 2 { return -5; }
    values[1] = p;
    p.y = 9;
    if replace_first(&mut values[0..1], q) != 14 { return -3; }
    let first = values[0];
    let second = values[1];
    if first.y != 3 || second.y != 2 || detached.x != 100 || p.x != 20 { return -4; }
    return first.x * 100 + second.x * 10 + p.y + detached.y;
}
"#;

const GENERIC_SLICES: &str = r#"
fn first<T: Copy>(values: &[T]) -> T { return values[0]; }
fn append<T: Copy>(values: &mut Vec<T>, value: T) -> T {
    let status = try_push(&mut *values, value);
    return value;
}
fn replace<T: Copy>(values: &mut [T], value: T) -> T {
    values[0] = value;
    return value;
}
fn relay<T: Copy>(values: &mut [T], value: T) -> T {
    return replace(&mut *values[0..1], value);
}
fn total(values: &[P]) -> i32 {
    let mut i = 0;
    let mut result = 0;
    while i < values.len() {
        let item = values[i];
        result = result + item.x + item.y;
        i = i + 1;
    }
    return result;
}
fn main() -> i32 {
    let p = P { x: 20, y: 2 };
    let q = P { x: 9, y: 1 };
    let r = P { x: 3, y: 4 };
    let mut values = new::<P>();
    let preserved = append(&mut values, p);
    if !grown(try_push(&mut values, q)) { return -1; }
    if !grown(try_push(&mut values, r)) { return -2; }
    let kept = relay(&mut values[1..3], r);
    if kept.x != 3 || kept.y != 4 { return -3; }
    if total(&values[3..3]) != 0 || total(&values[0..0]) != 0 { return -4; }
    let sum = total(&values);
    let mut copied = new::<P>();
    if !grown(try_extend(&mut copied, &values[1..3])) { return -5; }
    let selected = first(&copied);
    if selected.x != 3 || selected.y != 4 || p.x != 20 { return -6; }
    truncate(&mut values, 0);
    if len(&values) != 0 { return -7; }
    let mut scalars = new::<i32>();
    let original_scalar = append(&mut scalars, 6);
    let kept_scalar = replace(&mut scalars, 7);
    let scalar = first(&scalars);
    if original_scalar != 6 || kept_scalar != 7 || scalar != 7 { return -8; }
    return sum + total(&copied) + preserved.x + q.x + scalar;
}
"#;

const NESTED_WIDE: &str = r#"
#[derive(Copy)]
struct Wide { tag: u8, signed: i128, unsigned: u128, letter: char, flag: bool, unit: (), zero: f32, nan: f64 }
#[derive(Copy)]
struct Holder<T: Copy> { value: T, number: i32 }
fn wrap<T: Copy>(value: T) -> Holder<T> { return Holder::<T> { value: value, number: 5 }; }
fn first<T: Copy>(values: &[T]) -> T { return values[0]; }
fn main() -> i32 {
    let nan_bits = 9221120237041090602u64;
    let nan = f64::from_bits(nan_bits);
    let p = Wide {
        tag: 1u8,
        signed: -170141183460469231731687303715884105728i128,
        unsigned: 340282366920938463463374607431768211455u128,
        letter: '🦀', flag: true, unit: (), zero: -0.0f32, nan: nan
    };
    let original = wrap(p);
    let mut values = new::<Holder<Wide>>();
    if !grown(try_push(&mut values, original)) { return -1; }
    let mut changed = first(&values);
    changed.value.tag = 9u8;
    changed.value.letter = '雪';
    let unchanged = values[0];
    if unchanged.value.tag != 1u8 || unchanged.value.letter != '🦀' { return -7; }
    values[0] = changed;
    let stored = values[0];
    if original.value.tag != 1u8 || original.value.letter != '🦀' || p.tag != 1u8 { return -2; }
    if stored.value.tag != 9u8 || stored.value.letter != '雪' || !stored.value.flag { return -3; }
    if stored.value.signed != -170141183460469231731687303715884105728i128 { return -4; }
    if stored.value.unsigned != 340282366920938463463374607431768211455u128 { return -5; }
    let zero = stored.value.zero;
    let copied_nan = stored.value.nan;
    if zero.to_bits() != 2147483648u32 || copied_nan.to_bits() != 9221120237041090602u64 { return -6; }
    let unit = stored.value.unit;
    let old_tag = original.value.tag;
    let new_tag = stored.value.tag;
    return old_tag.to_i32() * 100 + new_tag.to_i32() * 10 + stored.number;
}
"#;

const EMPTY: &str = r#"
#[derive(Copy)]
struct Empty {}
fn tag(value: Empty) -> i32 { return 1; }
fn count(values: &[Empty]) -> i32 {
    let first = values[0];
    let again = first;
    return values.len() + tag(first) + tag(again);
}
fn main() -> i32 {
    let value = Empty {};
    let mut values = new::<Empty>();
    if !grown(try_resize(&mut values, 5, value)) { return -1; }
    let copied = values[3];
    let again = copied;
    if count(&values) != 7 { return -2; }
    return len(&values) * 10 + tag(value) + tag(copied) + tag(again);
}
"#;

fn accepts(body: &str, expected: &[u8]) {
    for policy in ["bounded", "hosted"] {
        let project = Project::new(body, policy);
        let checked = project.invoke(&["check"]);
        assert!(checked.status.success(), "{policy}: {checked:?}");
        assert!(checked.stderr.is_empty(), "{checked:?}");
        let ran = project.invoke(&["run"]);
        assert_eq!(ran.status.code(), Some(0), "{policy}: {ran:?}");
        assert_eq!(ran.stdout, expected, "{policy}");
        assert!(ran.stderr.is_empty(), "{ran:?}");
        project.no_artifacts();
    }
}

#[test]
fn copy_record_vectors_exercise_all_nine_operations_and_preserve_inputs() {
    // Retained values are (3,4), (30,4), and p=(30,4): 7 + 34 + 34 = 75.
    accepts(OPERATIONS, b"75\n");
}

#[test]
fn failed_growth_keeps_every_element_length_and_capacity() {
    // Two elements, first.x=20, second.y=3: 2*100 + 20*10 + 3 = 403.
    accepts(FAILED_GROWTH, b"403\n");
}

#[test]
fn indexed_copies_and_whole_element_replacement_are_independent() {
    // Vector x values are 7 and 20; retained p.y=9 and detached.y=2.
    accepts(INDEX_COPIES, b"911\n");
}

#[test]
fn generic_vectors_and_forwarded_record_slices_keep_scalar_specializations() {
    // 36 in values, 14 in copied, retained x values20+9, scalar7: 86.
    accepts(GENERIC_SLICES, b"86\n");
}

#[test]
fn nested_generic_records_keep_padding_wide_values_unicode_and_float_bits() {
    // The original tag is1, copied tag9 and number5: 100 + 90 + 5 = 195.
    accepts(NESTED_WIDE, b"195\n");
}

#[test]
fn empty_copy_record_elements_keep_independent_initialized_values() {
    accepts(EMPTY, b"53\n");
}

fn rejects(body: &str, code: &str, markers: &[&str]) {
    for policy in ["bounded", "hosted"] {
        let project = Project::new(body, policy);
        let checked = project.invoke(&["check"]);
        assert_eq!(
            checked.status.code(),
            Some(1),
            "{policy}: {body}: {checked:?}"
        );
        assert!(checked.stdout.is_empty(), "{checked:?}");
        let error = String::from_utf8(checked.stderr).unwrap();
        assert!(
            error.starts_with(&format!("error[{code}] ")),
            "{body}: {error}"
        );
        assert!(!error.contains("E0500"), "{error}");
        let lowered = error.to_ascii_lowercase();
        assert!(
            markers
                .iter()
                .any(|marker| lowered.contains(&marker.to_ascii_lowercase())),
            "{body}: {error}"
        );
        project.no_artifacts();
    }
}

#[test]
fn vector_elements_still_exclude_affine_resources_enums_nested_vectors_and_arrays() {
    for (body, code, markers) in [
        ("struct Affine { x:i32 } fn main()->i32 { let v=new::<Affine>(); return 0; }", "E0300", &["copy", "element"][..]),
        ("enum E { Value(i32) } fn main()->i32 { let v=new::<E>(); return 0; }", "E0300", &["copy", "element"][..]),
        ("fn main()->i32 { let v=new::<Vec<P>>(); return 0; }", "E0300", &["copy", "element"][..]),
        ("fn main()->i32 { let v=new::<[P;2]>(); return 0; }", "E0101", &["unsupported typed-preview construct `P`"][..]),
        ("use alloc::bytes::ByteBuf; fn main()->i32 { let v=new::<ByteBuf>(); return 0; }", "E0300", &["copy", "element"][..]),
        ("use std::fs::File; fn main()->i32 { let v=new::<File>(); return 0; }", "E0300", &["copy", "element"][..]),
        ("use alloc::bytes::ByteBuf; #[derive(Copy)]struct Bad { resource:ByteBuf } fn main()->i32 { let v=new::<Bad>(); return 0; }", "E0300", &["copy", "element"][..]),
        ("use std::fs::File; #[derive(Copy)]struct Bad { resource:File } fn main()->i32 { let v=new::<Bad>(); return 0; }", "E0300", &["copy", "element"][..]),
        ("#[derive(Copy)]struct Bad { resource:Vec<i32> } fn main()->i32 { let v=new::<Bad>(); return 0; }", "E0300", &["copy", "element"][..]),
        ("#[derive(Copy)]struct Bad { resource:[i32;1] } fn main()->i32 { let v=new::<Bad>(); return 0; }", "E0300", &["copy", "element"][..]),
    ] {
        rejects(body, code, markers);
    }
}

#[test]
fn equal_layouts_do_not_erase_record_element_nominal_identity() {
    for body in [
        "#[derive(Copy)]struct Q { x:i32,y:i32 } fn main()->i32 { let mut v=new::<P>(); let q=Q{x:1,y:2}; try_push(&mut v,q); return 0; }",
        "#[derive(Copy)]struct Q { x:i32,y:i32 } fn main()->i32 { let mut v=new::<P>(); let q=Q{x:1,y:2}; v[0]=q; return 0; }",
        "#[derive(Copy)]struct Q { x:i32,y:i32 } fn first(v:&[Q])->Q { return v[0]; } fn main()->i32 { let v=new::<P>(); let q=first(&v); return q.x; }",
    ] {
        rejects(body, "E0300", &["mismatch", "incompatible", "conflicting"]);
    }
}

#[test]
fn vector_ownership_mutability_and_live_slice_loans_remain_exclusive() {
    for (body, code, markers) in [
        ("fn main()->i32 { let v=new::<P>(); let moved=v; return len(&v); }", "E0310", &["moved", "available"][..]),
        ("fn main()->i32 { let v=new::<P>(); v[0]=P{x:1,y:2}; return 0; }", "E0300", &["mutable", "mutability"][..]),
        ("fn hold(a:&mut[P],b:&[P])->i32 { return 0; } fn main()->i32 { let mut v=new::<P>(); return hold(&mut v[0..1],&v[0..1]); }", "E0311", &["borrow", "overlap", "exclusive", "available"][..]),
        ("fn hold(a:&[P],b:&mut[P])->i32 { return 0; } fn main()->i32 { let mut v=new::<P>(); return hold(&v[0..1],&mut v[0..1]); }", "E0311", &["borrow", "overlap", "exclusive", "available"][..]),
        ("fn grow(a:&[P],v:&mut Vec<P>,p:P)->i32 { try_push(&mut *v,p); return a.len(); } fn main()->i32 { let mut v=new::<P>(); let p=P{x:1,y:2}; return grow(&v,&mut v,p); }", "E0311", &["borrow", "overlap", "exclusive", "available"][..]),
        ("fn use_item(v:&mut[P],p:P)->i32 { v[0]=p; return p.x; } fn main()->i32 { let mut v=new::<P>(); return use_item(&mut v,v[0]); }", "E0311", &["borrow", "overlap", "exclusive", "available"][..]),
    ] {
        rejects(body, code, markers);
    }
}

#[test]
fn element_and_field_references_cannot_be_stored_or_returned() {
    for (body, marker) in [
        ("fn main()->i32 { let v=new::<&P>(); return 0; }", "unsupported typed-preview construct `&`"),
        ("fn main()->i32 { let v=new::<P>(); let element=&v[0]; return 0; }", "unsupported typed-preview construct `[`"),
        ("fn read(value:&P)->i32 { return value.x; } fn main()->i32 { let v=new::<P>(); return read(&v[0]); }", "unsupported typed-preview construct `[`"),
        ("#[derive(Copy)]struct Node { x:i32 } #[derive(Copy)]struct Outer { node:Node } fn read(value:&Node)->i32 { return value.x; } fn main()->i32 { let v=new::<Outer>(); return read(&v[0].node); }", "unsupported typed-preview construct `[`"),
        ("struct Stored { value:&P } fn main()->i32 { let v=new::<P>(); return 0; }", "unsupported typed-preview construct `&`"),
        ("fn at(values:&[P])->&P { return &values[0]; } fn main()->i32 { let v=new::<P>(); return 0; }", "unsupported typed-preview construct `&`"),
    ] {
        rejects(body, "E0101", &[marker]);
    }
}

const READ_BOUNDS: &str =
    "fn main()->i32 { let v=new::<P>(); let item=v[0]; let zero=0; return item.x+1/zero; }";
const STORE_RHS_FIRST: &str = "fn fail()->P { let zero=0; let x=1/zero; return P{x:x,y:2}; } fn main()->i32 { let mut v=new::<P>(); v[1]=fail(); return 0; }";

#[test]
fn record_element_bounds_and_first_failure_order_match_ordinary_indexing() {
    for (body, code) in [
        (READ_BOUNDS, "E0606"),
        (STORE_RHS_FIRST, "E0607"),
        ("fn main()->i32 { let mut v=new::<P>(); v[0]=P{x:1,y:2}; return 0; }", "E0606"),
        ("fn main()->i32 { let v=new::<P>(); let item=v[-1]; return item.x; }", "E0606"),
        ("fn bad_index()->i32 { let zero=0; return 1/zero; } fn main()->i32 { let mut v=new::<P>(); v[bad_index()]=P{x:1,y:2}; return 0; }", "E0607"),
        ("fn ignore<T:Copy>(values:&[T])->i32 { return 0; } fn main()->i32 { let v=new::<P>(); return ignore(&v[0..1]); }", "E0606"),
        ("fn ignore<T:Copy>(values:&[T])->i32 { return 0; } fn main()->i32 { let mut v=new::<P>(); let p=P{x:1,y:2}; try_push(&mut v,p); try_push(&mut v,p); return ignore(&v[2..1]); }", "E0606"),
    ] {
        for policy in ["bounded", "hosted"] {
            let project = Project::new(body, policy);
            let checked = project.invoke(&["check"]);
            assert!(checked.status.success(), "{body}: {checked:?}");
            let ran = project.invoke(&["run"]);
            assert_eq!(ran.status.code(), Some(1), "{policy}: {body}: {ran:?}");
            assert!(ran.stdout.is_empty(), "{ran:?}");
            let error = String::from_utf8(ran.stderr).unwrap();
            assert!(error.contains(code) && !error.contains("E0500"), "{body}: {error}");
            project.no_artifacts();
        }
    }
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn deploy(body: &str, policy: &str) -> (Project, PathBuf) {
    let project = Project::new(body, policy);
    let deployment = project.0.join("deployment");
    fs::create_dir(&deployment).unwrap();
    for profile in ["dev", "release"] {
        let built = project.invoke(&["build", "--profile", profile]);
        assert!(built.status.success(), "{policy}: {profile}: {built:?}");
        let source = project.0.join(".oxid/bin/copy-vector-contracts");
        let header = fs::read(&source).unwrap();
        let magic = if cfg!(target_os = "linux") {
            b"\x7fELF"
        } else {
            b"\xcf\xfa\xed\xfe"
        };
        assert!(
            header.starts_with(magic),
            "native output is not a host executable"
        );
        fs::copy(source, deployment.join(profile)).unwrap();
    }
    fs::remove_file(project.0.join("main.ox")).unwrap();
    fs::remove_file(project.0.join("oxid.toml")).unwrap();
    for directory in [".oxid", "cache"] {
        if project.0.join(directory).exists() {
            fs::remove_dir_all(project.0.join(directory)).unwrap();
        }
    }
    (project, deployment)
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires real LLVM 19.1.7; Copy record vectors at O0/O2 after source removal"]
fn native_copy_record_vector_values_survive_source_and_cache_removal() {
    for (body, expected) in [
        (OPERATIONS, b"75\n".as_slice()),
        (FAILED_GROWTH, b"403\n".as_slice()),
        (INDEX_COPIES, b"911\n".as_slice()),
        (GENERIC_SLICES, b"86\n".as_slice()),
        (NESTED_WIDE, b"195\n".as_slice()),
        (EMPTY, b"53\n".as_slice()),
    ] {
        for policy in ["bounded", "hosted"] {
            let (_project, deployment) = deploy(body, policy);
            for profile in ["dev", "release"] {
                let ran = output(
                    Command::new(deployment.join(profile))
                        .env_clear()
                        .current_dir(&deployment),
                );
                assert_eq!(ran.status.code(), Some(0), "{policy}: {profile}: {ran:?}");
                assert_eq!(ran.stdout, expected, "{policy}: {profile}");
                assert!(ran.stderr.is_empty(), "{ran:?}");
            }
        }
    }
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires real LLVM 19.1.7; Copy record bounds and error order at O0/O2"]
fn native_copy_record_vector_failures_keep_the_first_diagnostic_without_sources() {
    for (body, code) in [(READ_BOUNDS, "E0606"), (STORE_RHS_FIRST, "E0607")] {
        for policy in ["bounded", "hosted"] {
            let (_project, deployment) = deploy(body, policy);
            for profile in ["dev", "release"] {
                let ran = output(
                    Command::new(deployment.join(profile))
                        .env_clear()
                        .current_dir(&deployment),
                );
                assert_eq!(ran.status.code(), Some(1), "{policy}: {profile}: {ran:?}");
                assert!(ran.stdout.is_empty(), "{ran:?}");
                let error = String::from_utf8(ran.stderr).unwrap();
                assert!(error.contains(code) && !error.contains("E0500"), "{error}");
            }
        }
    }
}
