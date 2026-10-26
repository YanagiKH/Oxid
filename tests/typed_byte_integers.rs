//! Byte arithmetic and result entries through ordinary project commands.
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
            "oxid-byte-integers-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), format!("[project]\nname=\"bytes\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
        Self(root)
    }
    fn invoke(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
    fn refused_modules(&self) {
        let before = fs::read(self.0.join("src/main.ox")).unwrap();
        for operation in ["check", "run"] {
            let out = self.invoke(&[operation, "--message-format=json"]);
            assert_eq!(out.status.code(), Some(1), "{out:?}");
            assert!(out.stderr.is_empty(), "{out:?}");
            let text = String::from_utf8(out.stdout).unwrap();
            assert!(
                text.contains("\"code\":\"E0005\"")
                    && text.contains("module source policy is not qualified on this host"),
                "{text}"
            );
        }
        assert_eq!(fs::read(self.0.join("src/main.ox")).unwrap(), before);
        assert!(!self.0.join(".oxid").exists());
        assert!(!self.0.join("oxid.lock").exists());
    }
    fn result(&self, expected: u8) {
        let checked = self.invoke(&["check", "--message-format=json"]);
        assert!(checked.status.success(), "{checked:?}");
        let plain = self.invoke(&["run"]);
        assert_eq!(
            (plain.status.code(), plain.stdout, plain.stderr),
            (Some(0), format!("{expected}\n").into_bytes(), Vec::new())
        );
        let json = self.invoke(&["run", "--message-format=json"]);
        assert_eq!(json.status.code(), Some(0), "{json:?}");
        assert!(json.stderr.is_empty());
        assert_eq!(String::from_utf8(json.stdout).unwrap(), format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":true,\"errors\":0,\"result\":{{\"type\":\"u8\",\"value\":{expected}}}}}\n"));
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn all_byte_results_keep_their_exact_text_and_json_value() {
    for value in 0u8..=255 {
        for policy in ["bounded", "hosted"] {
            Project::new(&format!("fn main()->u8{{return {value}u8;}}"), policy).result(value);
        }
    }
}

#[test]
fn named_conversions_and_arithmetic_have_the_same_behavior_in_both_source_contexts() {
    for (body, expected) in [
        ("let a=250u8;return a+5u8;", 255),
        ("let a=255u8;return a-254u8;", 1),
        ("let a=15u8;return a*17u8;", 255),
        ("let a=255u8;return a/2u8;", 127),
        ("let a=255u8;return a%2u8;", 1),
        (
            "let n=255;let byte=n.to_u8_checked();return byte.to_u8_checked();",
            255,
        ),
        (
            "let byte=255u8;if byte>127u8&&byte>=255u8&&byte!=0u8{return byte;}return 0u8;",
            255,
        ),
    ] {
        for declarations in ["", "struct Unused{value:u8}"] {
            for policy in ["bounded", "hosted"] {
                Project::new(&format!("{declarations}fn main()->u8{{{body}}}"), policy)
                    .result(expected);
            }
        }
    }
}

#[test]
fn byte_failures_preserve_the_executed_operator_and_never_print_a_result() {
    for (body, code, token) in [
        ("let a=255u8;return a+1u8;", "E0604", "+"),
        ("let a=0u8;return a-1u8;", "E0604", "-"),
        ("let a=128u8;return a*2u8;", "E0604", "*"),
        ("let a=1u8;return a/0u8;", "E0607", "/"),
        ("let a=1u8;return a%0u8;", "E0607", "%"),
    ] {
        for declarations in ["", "struct Unused{value:u8}"] {
            let prefix = format!("// 位元組\r\n{declarations}fn main()->u8{{");
            let start = prefix.len() + body.find(token).unwrap();
            let source = format!("{prefix}{body}}}");
            for policy in ["bounded", "hosted"] {
                let out = Project::new(&source, policy).invoke(&["run", "--message-format=json"]);
                assert_eq!(out.status.code(), Some(1), "{source}: {out:?}");
                assert!(out.stderr.is_empty());
                let text = String::from_utf8(out.stdout).unwrap();
                assert!(text.contains(&format!("\"code\":\"{code}\"")), "{text}");
                assert!(
                    text.contains(&format!("\"start\":{start},\"end\":{}", start + 1)),
                    "{text}"
                );
                assert!(text.contains("\"result\":null"));
                assert!(!text.contains("E0500"));
            }
        }
    }
}

const STORAGE: &str = "struct Packet{left:i32,byte:u8,right:i32}enum Value{Byte(u8),Packet(Packet)}fn bump(bytes:&mut[u8])->(){bytes[0]=bytes[0]+1u8;return;}fn take(value:Value)->u8{match value{Value::Byte(byte)=>{return byte;},Value::Packet(packet)=>{if packet.left!=16909060||packet.right!=84281096{return 0u8;}return packet.byte;},}}fn main()->u8{let mut bytes=[254u8,17u8];bump(&mut bytes);if bytes[1]!=17u8{return 0u8;}let packet=Packet{left:16909060,byte:bytes[0],right:84281096};let byte=take(Value::Packet(packet));return take(Value::Byte(byte));}";

#[test]
fn byte_results_survive_record_array_and_enum_storage() {
    for policy in ["bounded", "hosted"] {
        Project::new(STORAGE, policy).result(255);
    }
}

#[test]
fn linked_scalar_byte_modules_and_inferred_wide_temporaries_use_the_typed_checker() {
    for policy in ["bounded", "hosted"] {
        let p = Project::new(
            "mod data;fn main()->u8{return crate::data::answer();}",
            policy,
        );
        fs::write(
            p.0.join("src/data.ox"),
            "pub fn answer()->u8{let n=250u8;return n+5u8;}",
        )
        .unwrap();
        if cfg!(target_os = "linux") {
            p.result(255);
        } else {
            p.refused_modules();
        }
        let p = Project::new(
            "mod data;fn main()->i32{return crate::data::answer();}",
            policy,
        );
        fs::write(
            p.0.join("src/data.ox"),
            "pub fn answer()->i32{let n=7;let wide=n.to_i64();return wide.to_i32_checked();}",
        )
        .unwrap();
        if cfg!(target_os = "linux") {
            let out = p.invoke(&["run"]);
            assert_eq!(
                (out.status.code(), out.stdout, out.stderr),
                (Some(0), b"7\n".to_vec(), Vec::new())
            );
        } else {
            p.refused_modules();
        }
    }
}

#[test]
fn byte_arithmetic_stays_strict_and_process_entries_still_require_i32() {
    for (body, code) in [
        ("return 1u8+1;", "E0300"),
        ("return 1;", "E0300"),
        ("let a=1u8;return -a;", "E0300"),
        ("return 256u8;", "E0203"),
    ] {
        let out = Project::new(&format!("fn main()->u8{{{body}}}"), "bounded")
            .invoke(&["check", "--message-format=json"]);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(String::from_utf8(out.stdout)
            .unwrap()
            .contains(&format!("\"code\":\"{code}\"")));
    }
    let out = Project::new("fn main()->u8{return 255u8;}", "bounded")
        .invoke(&["run", "--entry-mode=process"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty());
    let error = String::from_utf8(out.stderr).unwrap();
    if cfg!(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
        assert!(
            error.contains("[E0600]") && error.contains("fn main() -> i32"),
            "{error}"
        );
    } else {
        assert!(error.contains("[E0608]"), "{error}");
    }
}

#[test]
#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7 in OXID_LLVM_BIN"]
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn byte_result_modules_execute_after_removing_the_project() {
    for policy in ["bounded", "hosted"] {
        for profile in ["dev", "release"] {
            let p = Project::new(
                "mod data;use crate::data::answer;fn main()->u8{return answer();}",
                policy,
            );
            fs::write(
                p.0.join("src/data.ox"),
                STORAGE.replace("fn main()->u8", "pub fn answer()->u8"),
            )
            .unwrap();
            let build = p.invoke(&["build", "--profile", profile, "--incremental"]);
            assert!(build.status.success(), "{build:?}");
            let warm = p.invoke(&[
                "build",
                "--profile",
                profile,
                "--incremental",
                "--message-format=json",
            ]);
            assert!(warm.status.success(), "{warm:?}");
            assert!(String::from_utf8(warm.stdout)
                .unwrap()
                .contains("\"status\":\"hit\""));
            let executable = p.0.join("bytes");
            fs::copy(p.0.join(".oxid/bin/bytes"), &executable).unwrap();
            fs::remove_dir_all(p.0.join("src")).unwrap();
            fs::remove_dir_all(p.0.join(".oxid")).unwrap();
            fs::remove_file(p.0.join("oxid.toml")).unwrap();
            let out = Command::new(executable)
                .current_dir(&p.0)
                .env_clear()
                .output()
                .unwrap();
            assert_eq!(
                (out.status.code(), out.stdout, out.stderr),
                (Some(0), b"255\n".to_vec(), Vec::new())
            );
        }
    }
}
