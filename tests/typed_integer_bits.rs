//! Integer masks, shifts and formatting through the public project commands.
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
            "oxid-integer-bits-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), format!("[project]\nname=\"bits\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
        Self(root)
    }
    fn invoke(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
    fn result(&self, ty: &str, expected: &str) {
        let checked = self.invoke(&["check"]);
        assert!(checked.status.success(), "{checked:?}");
        let result = self.invoke(&["run"]);
        assert_eq!(
            (result.status.code(), result.stdout, result.stderr),
            (Some(0), format!("{expected}\n").into_bytes(), Vec::new())
        );
        let result = self.invoke(&["run", "--message-format=json"]);
        assert_eq!(result.status.code(), Some(0), "{result:?}");
        assert!(result.stderr.is_empty());
        let value = if matches!(ty, "i64" | "u64" | "isize" | "usize" | "i128" | "u128") {
            format!("\"{expected}\"")
        } else {
            expected.to_owned()
        };
        assert_eq!(String::from_utf8(result.stdout).unwrap(), format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":true,\"errors\":0,\"result\":{{\"type\":\"{ty}\",\"value\":{value}}}}}\n"));
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
const TYPES: [(&str, u32, bool); 12] = [
    ("i8", 8, true),
    ("u8", 8, false),
    ("i16", 16, true),
    ("u16", 16, false),
    ("i32", 32, true),
    ("u32", 32, false),
    ("i64", 64, true),
    ("u64", 64, false),
    ("isize", 64, true),
    ("usize", 64, false),
    ("i128", 128, true),
    ("u128", 128, false),
];
fn literal(ty: &str, value: &str) -> String {
    if ty == "i32" {
        value.to_owned()
    } else {
        format!("{value}{ty}")
    }
}

#[test]
fn every_integer_width_preserves_its_bit_pattern_and_declared_result_type() {
    for (ty, width, signed) in TYPES {
        let all = if signed {
            "-1".to_owned()
        } else {
            (u128::MAX >> (128 - width)).to_string()
        };
        let top = 1u128 << (width - 1);
        let top = if signed {
            format!("-{top}")
        } else {
            top.to_string()
        };
        for (expression, expected) in [
            (format!("~{}", literal(ty, "0")), all.clone()),
            (
                format!("{}&{}", literal(ty, &all), literal(ty, "1")),
                "1".into(),
            ),
            (
                format!("{}|{}", literal(ty, "0"), literal(ty, "1")),
                "1".into(),
            ),
            (
                format!("{}^{}", literal(ty, "1"), literal(ty, "1")),
                "0".into(),
            ),
            (format!("{}<<{}", literal(ty, "1"), width - 1), top),
            (
                format!("{}>>{}", literal(ty, &all), width - 1),
                if signed { "-1".into() } else { "1".into() },
            ),
        ] {
            for policy in ["bounded", "hosted"] {
                Project::new(&format!("fn main()->{ty}{{return {expression};}}"), policy)
                    .result(ty, &expected);
            }
        }
    }
}

#[test]
fn precedence_associativity_calls_and_truncating_left_shifts_are_stable() {
    for (ty, body, expected) in [
        ("i32", "return 1|2&4;", "1"),
        ("i32", "return 4^3&1;", "5"),
        ("i32", "return 1<<2+3;", "32"),
        ("i32", "return 1<<1<<2;", "8"),
        ("i32", "return 8>>1+1;", "2"),
        ("u8", "return 255u8<<1;", "254"),
        ("i8", "return -128i8<<1;", "0"),
    ] {
        for policy in ["bounded", "hosted"] {
            Project::new(&format!("fn main()->{ty}{{{body}}}"), policy).result(ty, expected);
        }
    }
    Project::new(
        "fn apply(a:u8,b:u8)->u8{return a&b;}fn main()->u8{return apply(240u8,15u8);}",
        "bounded",
    )
    .result("u8", "0");
}

#[test]
fn shift_counts_accept_every_integer_type_without_implicit_operand_conversion() {
    for (left, _, _) in TYPES {
        for (right, _, _) in TYPES {
            let source = format!(
                "fn main()->{left}{{let amount={};return {}<<amount;}}",
                literal(right, "1"),
                literal(left, "1")
            );
            Project::new(&source, "bounded").result(left, "2");
        }
    }
    for source in [
        "fn main()->bool{return true&false;}",
        "fn main()->i32{return 1|true;}",
        "fn main()->u8{return 1u8^1;}",
        "fn main()->i32{return ~true;}",
        "fn main()->i32{return 1<<false;}",
        "fn main()->i32{return 1 < < 2;}",
    ] {
        let result = Project::new(source, "bounded").invoke(&["check"]);
        assert_eq!(result.status.code(), Some(1), "{result:?}");
        assert!(
            !String::from_utf8_lossy(&result.stderr).contains("E0500"),
            "{result:?}"
        );
    }
}

#[test]
fn invalid_shift_counts_identify_the_whole_executed_operator() {
    for (ty, body, operator) in [
        ("u8", "let n=-1;return 1u8<<n;", "<<"),
        ("i8", "let n=8u128;return -1i8>>n;", ">>"),
        ("u128", "let n=128;return 1u128<<n;", "<<"),
        (
            "i32",
            "let n=-170141183460469231731687303715884105728i128;return 1>>n;",
            ">>",
        ),
    ] {
        let prefix = format!("// 位元\r\nfn main()->{ty}{{");
        let start = prefix.len() + body.find(operator).unwrap();
        for policy in ["bounded", "hosted"] {
            let result = Project::new(&format!("{prefix}{body}}}"), policy)
                .invoke(&["run", "--message-format=json"]);
            assert_eq!(result.status.code(), Some(1), "{result:?}");
            assert!(result.stderr.is_empty());
            let text = String::from_utf8(result.stdout).unwrap();
            assert!(text.contains("\"code\":\"E0611\""), "{text}");
            assert!(
                text.contains(&format!("\"start\":{start},\"end\":{}", start + 2)),
                "{text}"
            );
            assert!(text.contains("\"result\":null"), "{text}");
        }
    }
}

#[test]
fn formatter_keeps_shift_pairs_and_distinguishes_masks_from_borrows() {
    let project=Project::new("fn read(bytes:&[u8;1])->u8{return (~bytes[0]&255u8)<<1;}fn main()->u8{let bytes=[1u8];return read(&bytes);}","bounded");
    let first = project.invoke(&["fmt", "src/main.ox"]);
    assert!(first.status.success(), "{first:?}");
    let text = String::from_utf8(first.stdout).unwrap();
    assert!(text.contains("<<"), "{text}");
    assert!(text.contains(" & "), "{text}");
    fs::write(project.0.join("src/main.ox"), &text).unwrap();
    project.result("u8", "252");
    let again = project.invoke(&["fmt", "src/main.ox"]);
    assert_eq!(again.stdout, text.into_bytes());
    assert!(again.status.success());
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires LLVM 19.1.7"]
fn native_shift_parameters_cover_every_counter_width_after_source_removal() {
    let llvm = std::env::var_os("OXID_TEST_LLVM_BIN").expect("LLVM tools");
    for left in ["u8", "i32", "u64", "i128"] {
        let mut source = String::new();
        let mut body = String::new();
        for (right, _, _) in TYPES {
            source.push_str(&format!(
                "fn shift_{right}(count:{right})->{left}{{return {}<<count;}}",
                literal(left, "1")
            ));
            body.push_str(&format!(
                "if shift_{right}({})!={}{{return -1;}}",
                literal(right, "1"),
                literal(left, "2")
            ));
        }
        source.push_str(&format!("fn main()->i32{{{body}return 0;}}"));
        for policy in ["bounded", "hosted"] {
            let project = Project::new(&source, policy);
            let deployment = Project::new("", policy);
            for profile in ["dev", "release"] {
                for _ in 0..2 {
                    let result = Command::new(env!("CARGO_BIN_EXE_oxid"))
                        .current_dir(&project.0)
                        .env("OXID_LLVM_BIN", &llvm)
                        .args(["build", "--profile", profile, "--incremental"])
                        .output()
                        .unwrap();
                    assert!(result.status.success(), "{result:?}");
                }
                fs::copy(project.0.join(".oxid/bin/bits"), deployment.0.join(profile)).unwrap();
            }
            fs::remove_dir_all(project.0.join("src")).unwrap();
            fs::remove_dir_all(project.0.join(".oxid")).unwrap();
            fs::remove_file(project.0.join("oxid.toml")).unwrap();
            fs::remove_dir_all(deployment.0.join("src")).unwrap();
            fs::remove_file(deployment.0.join("oxid.toml")).unwrap();
            for profile in ["dev", "release"] {
                let result = Command::new(deployment.0.join(profile))
                    .current_dir(&deployment.0)
                    .env_clear()
                    .output()
                    .unwrap();
                assert_eq!(
                    (result.status.code(), result.stdout, result.stderr),
                    (Some(0), b"0\n".to_vec(), Vec::new())
                );
            }
        }
    }
}
