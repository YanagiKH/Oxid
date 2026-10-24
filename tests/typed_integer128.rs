//! Full-width integers through ordinary commands, including source-free ELFs.
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
            "oxid-integer128-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), format!("[project]\nname=\"quad\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
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
        let check = self.invoke(&["check"]);
        assert!(check.status.success(), "{check:?}");
        let result = self.invoke(&["run"]);
        assert_eq!(
            (result.status.code(), result.stdout, result.stderr),
            (Some(0), format!("{expected}\n").into_bytes(), Vec::new())
        );
        let result = self.invoke(&["run", "--message-format=json"]);
        assert_eq!(result.status.code(), Some(0), "{result:?}");
        assert!(result.stderr.is_empty(), "{result:?}");
        assert_eq!(String::from_utf8(result.stdout).unwrap(), format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":true,\"errors\":0,\"result\":{{\"type\":\"{ty}\",\"value\":\"{expected}\"}}}}\n"));
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn complete_extrema_and_values_beyond_u64_keep_exact_decimal_results() {
    for (ty, expected) in [
        ("i128", "-170141183460469231731687303715884105728"),
        ("i128", "170141183460469231731687303715884105727"),
        ("u128", "340282366920938463463374607431768211455"),
        ("u128", "18446744073709551616"),
    ] {
        for policy in ["bounded", "hosted"] {
            Project::new(
                &format!("fn main()->{ty}{{let n={expected}{ty};let copy=n;return copy;}}"),
                policy,
            )
            .result(ty, expected);
        }
    }
}

#[test]
fn arithmetic_and_checked_cross_width_conversions_preserve_signedness() {
    for (ty, body, expected) in [
        (
            "u128",
            "let a=18446744073709551616u128;return a+7u128;",
            "18446744073709551623",
        ),
        (
            "i128",
            "let a=-9223372036854775808i128;return a*18446744073709551616i128;",
            "-170141183460469231731687303715884105728",
        ),
        (
            "i128",
            "let a=-170141183460469231731687303715884105728i128;return a/3i128;",
            "-56713727820156410577229101238628035242",
        ),
        (
            "i128",
            "let a=-170141183460469231731687303715884105728i128;return a%3i128;",
            "-2",
        ),
        (
            "i128",
            "let a=-170141183460469231731687303715884105727i128;return -a;",
            "170141183460469231731687303715884105727",
        ),
        (
            "u128",
            "let a=18446744073709551615u64;return a.to_u128_checked();",
            "18446744073709551615",
        ),
        (
            "i128",
            "let a=-9223372036854775808i64;return a.to_i128_checked();",
            "-9223372036854775808",
        ),
        (
            "i128",
            "let a=170141183460469231731687303715884105727u128;return a.to_i128_checked();",
            "170141183460469231731687303715884105727",
        ),
    ] {
        for policy in ["bounded", "hosted"] {
            Project::new(&format!("fn main()->{ty}{{{body}}}"), policy).result(ty, expected);
        }
    }
}

#[test]
fn runtime_failures_keep_the_executed_operator_origin() {
    for (ty, body, code, token) in [
        (
            "u128",
            "let a=340282366920938463463374607431768211455u128;return a+1u128;",
            "E0604",
            "+",
        ),
        ("u128", "let a=0u128;return a-1u128;", "E0604", "-"),
        (
            "i128",
            "let a=-170141183460469231731687303715884105728i128;return a/-1i128;",
            "E0604",
            "/",
        ),
        (
            "i128",
            "let a=-170141183460469231731687303715884105728i128;return a%-1i128;",
            "E0604",
            "%",
        ),
        ("i128", "let a=1i128;return a/0i128;", "E0607", "/"),
        ("u128", "let a=1u128;return a%0u128;", "E0607", "%"),
        (
            "u64",
            "let a=18446744073709551616u128;return a.to_u64_checked();",
            "E0610",
            "to_u64_checked",
        ),
        (
            "i128",
            "let a=170141183460469231731687303715884105728u128;return a.to_i128_checked();",
            "E0610",
            "to_i128_checked",
        ),
    ] {
        let prefix = format!("// 整數\r\nfn main()->{ty}{{");
        let start = prefix.len() + body.find(token).unwrap();
        for policy in ["bounded", "hosted"] {
            let project = Project::new(&format!("{prefix}{body}}}"), policy);
            let result = project.invoke(&["run", "--message-format=json"]);
            assert_eq!(result.status.code(), Some(1), "{result:?}");
            assert!(result.stderr.is_empty(), "{result:?}");
            let text = String::from_utf8(result.stdout).unwrap();
            assert!(text.contains(&format!("\"code\":\"{code}\"")), "{text}");
            assert!(
                text.contains(&format!(
                    "\"start\":{start},\"end\":{}",
                    start + token.len()
                )),
                "{text}"
            );
            assert!(text.contains("\"result\":null"), "{text}");
        }
    }
}

const AGGREGATE: &str = "struct Pair{small:u8,number:u128}enum Packet{Empty,Value(Pair),Signed(i128)}fn read(packet:Packet)->u128{match packet{Packet::Empty=>{return 0u128;},Packet::Value(pair)=>{return pair.number;},Packet::Signed(n)=>{return n.to_u128_checked();},}}fn main()->u128{let pair=Pair{small:7u8,number:340282366920938463463374607431768211455u128};let packet=Packet::Value(pair);let result=read(packet);if 0u128<result&&result>0u128&&result>=170141183460469231731687303715884105728u128&&0u128<=result{return result;}return 0u128;}";

const RESOURCE: &str = "use alloc::bytes::ByteBuf;use alloc::bytes::GrowStatus;use alloc::bytes::new;use alloc::bytes::len;use alloc::bytes::try_push;enum Packet{Number(u128),Bytes(ByteBuf)}fn main()->u128{let mut bytes=new();let status=try_push(&mut bytes,255u8);match status{GrowStatus::Ok=>{},GrowStatus::CapacityOverflow=>{return 0u128;},GrowStatus::AllocationFailed=>{return 0u128;},GrowStatus::MemoryLimit=>{return 0u128;},}let packet=Packet::Bytes(bytes);match packet{Packet::Number(n)=>{return n;},Packet::Bytes(payload)=>{let count=len(&payload);return count.to_u128_checked()*255u128;},}}";

#[test]
fn full_width_values_move_through_aligned_records_enums_calls_and_array_loans() {
    for policy in ["bounded", "hosted"] {
        Project::new(AGGREGATE, policy).result("u128", "340282366920938463463374607431768211455");
        Project::new("fn read(values:&[i128;2])->i128{return values[1];}fn main()->i128{let values=[1i128,-170141183460469231731687303715884105728i128];return read(&values);}", policy).result("i128", "-170141183460469231731687303715884105728");
    }
}

#[test]
fn resource_payloads_use_the_alignment_of_the_largest_enum_variant() {
    for policy in ["bounded", "hosted"] {
        Project::new(RESOURCE, policy).result("u128", "255");
    }
}

#[test]
fn process_entries_can_calculate_with_full_width_values_and_return_i32_status() {
    for policy in ["bounded", "hosted"] {
        let project = Project::new("fn main()->i32{let a=340282366920938463463374607431768211455u128;let status=a%256u128;return status.to_i32_checked();}", policy);
        let manifest = project.0.join("oxid.toml");
        fs::write(
            &manifest,
            fs::read_to_string(&manifest)
                .unwrap()
                .replace("entry-mode=\"result\"", "entry-mode=\"process\""),
        )
        .unwrap();
        let result = project.invoke(&["run"]);
        assert_eq!(
            (result.status.code(), result.stdout, result.stderr),
            (Some(255), Vec::new(), Vec::new())
        );
    }
}

#[test]
fn linked_scalar_modules_and_inferred_full_width_temporaries_select_complete_storage() {
    for policy in ["bounded", "hosted"] {
        let project = Project::new(
            "mod value;fn main()->i128{let n=crate::value::number();return n+1i128;}",
            policy,
        );
        fs::write(
            project.0.join("src/value.ox"),
            "pub fn number()->i128{return 18446744073709551616i128;}",
        )
        .unwrap();
        project.result("i128", "18446744073709551617");
    }
}

#[test]
fn mixed_types_invalid_literals_and_non_i32_process_entries_are_rejected() {
    for source in [
        "fn main()->u128{return 1u128+1u64;}",
        "fn main()->u128{return -1u128;}",
        "fn main()->i128{return 170141183460469231731687303715884105728i128;}",
        "fn main()->u128{return 340282366920938463463374607431768211456u128;}",
    ] {
        let result = Project::new(source, "bounded").invoke(&["check"]);
        assert_eq!(result.status.code(), Some(1), "{result:?}");
        assert!(
            !String::from_utf8_lossy(&result.stderr).contains("E0500"),
            "{result:?}"
        );
    }
    let project = Project::new("fn main()->i128{return 0i128;}", "bounded");
    let manifest = project.0.join("oxid.toml");
    fs::write(
        &manifest,
        fs::read_to_string(&manifest)
            .unwrap()
            .replace("entry-mode=\"result\"", "entry-mode=\"process\""),
    )
    .unwrap();
    assert_eq!(project.invoke(&["run"]).status.code(), Some(1));
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires LLVM 19.1.7"]
fn native_full_width_aggregates_remain_source_free_in_both_build_profiles() {
    let llvm = std::env::var_os("OXID_TEST_LLVM_BIN").expect("LLVM tools");
    for (source, expected) in [
        (AGGREGATE, "340282366920938463463374607431768211455"),
        (RESOURCE, "255"),
    ] {
        for policy in ["bounded", "hosted"] {
            let project = Project::new(source, policy);
            let deployment = Project::new("", policy);
            for profile in ["dev", "release"] {
                for _ in 0..2 {
                    let built = Command::new(env!("CARGO_BIN_EXE_oxid"))
                        .current_dir(&project.0)
                        .env("OXID_LLVM_BIN", &llvm)
                        .args(["build", "--profile", profile, "--incremental"])
                        .output()
                        .unwrap();
                    assert!(built.status.success(), "{built:?}");
                }
                let binary = deployment.0.join(format!("{policy}-{profile}"));
                fs::copy(project.0.join(".oxid/bin/quad"), &binary).unwrap();
            }
            fs::remove_dir_all(project.0.join("src")).unwrap();
            fs::remove_dir_all(project.0.join(".oxid")).unwrap();
            fs::remove_file(project.0.join("oxid.toml")).unwrap();
            fs::remove_dir_all(deployment.0.join("src")).unwrap();
            fs::remove_file(deployment.0.join("oxid.toml")).unwrap();
            for profile in ["dev", "release"] {
                let binary = deployment.0.join(format!("{policy}-{profile}"));
                let result = Command::new(binary)
                    .current_dir(&deployment.0)
                    .env_clear()
                    .output()
                    .unwrap();
                assert_eq!(
                    (result.status.code(), result.stdout, result.stderr),
                    (Some(0), format!("{expected}\n").into_bytes(), Vec::new())
                );
            }
        }
    }
}
