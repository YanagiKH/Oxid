//! Exact integer widths and strict conversions through ordinary project commands.
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
            "oxid-integer-widths-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), format!("[project]\nname=\"integers\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
        Self(root)
    }
    fn invoke(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
    fn result(&self, expected: &str) {
        let out = self.invoke(&["run"]);
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        assert_eq!(out.stdout, format!("{expected}\n").as_bytes());
        assert!(out.stderr.is_empty(), "{out:?}");
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn extrema_copies_and_json_preserve_the_complete_declared_integer() {
    for (ty, literal, expected, string) in [
        ("i8", "-128i8", "-128", false),
        ("i8", "127i8", "127", false),
        ("i16", "-32768i16", "-32768", false),
        ("i16", "32767i16", "32767", false),
        ("u16", "65535u16", "65535", false),
        ("u32", "4294967295u32", "4294967295", false),
        (
            "u64",
            "18446744073709551615u64",
            "18446744073709551615",
            true,
        ),
        (
            "isize",
            "-9223372036854775808isize",
            "-9223372036854775808",
            true,
        ),
        (
            "isize",
            "9223372036854775807isize",
            "9223372036854775807",
            true,
        ),
        (
            "usize",
            "18446744073709551615usize",
            "18446744073709551615",
            true,
        ),
        ("u64", "00000000000000000000000001u64", "1", true),
    ] {
        let p = Project::new(&format!("fn copy(value:{ty})->{ty}{{return value;}}fn main()->{ty}{{let value={literal};let copied:{ty}=value;return copy(copied);}}"), "bounded");
        p.result(expected);
        let out = p.invoke(&["run", "--message-format=json"]);
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        assert!(out.stderr.is_empty());
        let value = if string {
            format!("\"{expected}\"")
        } else {
            expected.to_string()
        };
        assert_eq!(String::from_utf8(out.stdout).unwrap(), format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":true,\"errors\":0,\"result\":{{\"type\":\"{ty}\",\"value\":{value}}}}}\n"));
    }
    Project::new(
        "fn main()->i32{let byte=255u8;return byte.to_i32();}",
        "bounded",
    )
    .result("255");
}

#[test]
fn arithmetic_division_and_unsigned_comparisons_follow_mathematical_values() {
    for (source, expected) in [
        ("fn main()->i8{let a=-100i8;return a+99i8;}", "-1"),
        ("fn main()->i16{let a=-30000i16;return a+29999i16;}", "-1"),
        ("fn main()->u16{let a=65535u16;return a/2u16;}", "32767"),
        ("fn main()->u32{let a=65535u32;return a*a;}", "4294836225"),
        (
            "fn main()->u64{let a=18446744073709551615u64;return a/3u64;}",
            "6148914691236517205",
        ),
        (
            "fn main()->isize{let a=-9223372036854775808isize;return a/3isize;}",
            "-3074457345618258602",
        ),
        (
            "fn main()->isize{let a=-9223372036854775808isize;return a%3isize;}",
            "-2",
        ),
        (
            "fn main()->usize{let a=18446744073709551615usize;return a%3usize;}",
            "0",
        ),
        (
            "fn main()->bool{return 18446744073709551615u64>9223372036854775807u64;}",
            "true",
        ),
        (
            "fn main()->bool{return 4294967295u32<2147483648u32;}",
            "false",
        ),
    ] {
        for policy in ["bounded", "hosted"] {
            Project::new(source, policy).result(expected);
        }
    }
}

#[test]
fn overflow_zero_divisors_and_narrowing_keep_the_executed_source_origin() {
    for (ty, body, code, marker, adjustment, width) in [
        ("i8", "let a=127i8;return a+1i8;", "E0604", "+", 0, 1),
        ("i16", "let a=-32768i16;return a-1i16;", "E0604", "a-", 1, 1),
        ("u16", "let a=0u16;return a-1u16;", "E0604", "a-", 1, 1),
        (
            "u32",
            "let a=4294967295u32;return a*2u32;",
            "E0604",
            "*",
            0,
            1,
        ),
        (
            "u64",
            "let a=18446744073709551615u64;return a+1u64;",
            "E0604",
            "+",
            0,
            1,
        ),
        (
            "usize",
            "let a=0usize;return a-1usize;",
            "E0604",
            "a-",
            1,
            1,
        ),
        (
            "isize",
            "let a=-9223372036854775808isize;return -a;",
            "E0604",
            "-a",
            0,
            1,
        ),
        ("i8", "let a=-128i8;return a/-1i8;", "E0604", "/", 0, 1),
        ("i8", "let a=-128i8;return a%-1i8;", "E0604", "%", 0, 1),
        ("u64", "let a=1u64;return a/0u64;", "E0607", "/", 0, 1),
        ("u16", "let a=1u16;return a%0u16;", "E0607", "%", 0, 1),
        (
            "u64",
            "let a=-1i64;return a.to_u64_checked();",
            "E0610",
            "to_u64_checked",
            0,
            14,
        ),
        (
            "i64",
            "let a=18446744073709551615u64;return a.to_i64_checked();",
            "E0610",
            "to_i64_checked",
            0,
            14,
        ),
        (
            "i8",
            "let a=128i16;return a.to_i8_checked();",
            "E0610",
            "to_i8_checked",
            0,
            13,
        ),
        (
            "i16",
            "let a=65535u16;return a.to_i16_checked();",
            "E0610",
            "to_i16_checked",
            0,
            14,
        ),
        (
            "i32",
            "let a=18446744073709551615usize;return a.to_i32_checked();",
            "E0610",
            "to_i32_checked",
            0,
            14,
        ),
    ] {
        let prefix = format!("// 範圍\r\nfn main()->{ty}{{");
        let start = prefix.len() + body.find(marker).unwrap() + adjustment;
        let source = format!("{prefix}{body}}}");
        for policy in ["bounded", "hosted"] {
            let out = Project::new(&source, policy).invoke(&["run", "--message-format=json"]);
            assert_eq!(out.status.code(), Some(1), "{source}: {out:?}");
            assert!(out.stderr.is_empty());
            let text = String::from_utf8(out.stdout).unwrap();
            assert!(
                text.contains(&format!("\"code\":\"{code}\"")),
                "{source}: {text}"
            );
            assert!(
                text.contains(&format!("\"start\":{start},\"end\":{}", start + width)),
                "{source}: {text}"
            );
            assert!(text.contains("\"result\":null"));
        }
    }
}

#[test]
fn literal_ranges_primitive_names_and_mixed_types_fail_before_execution() {
    for (source, code) in [
        ("fn main()->i8{return 128i8;}", "E0203"),
        ("fn main()->i8{return -129i8;}", "E0203"),
        ("fn main()->i16{return 32768i16;}", "E0203"),
        ("fn main()->u16{return 65536u16;}", "E0203"),
        ("fn main()->u32{return 4294967296u32;}", "E0203"),
        ("fn main()->u64{return 18446744073709551616u64;}", "E0203"),
        ("fn main()->u64{return -1u64;}", "E0203"),
        (
            "fn main()->isize{return 9223372036854775808isize;}",
            "E0203",
        ),
        (
            "fn main()->usize{return 18446744073709551616usize;}",
            "E0203",
        ),
        ("fn main()->i16{let x:i16=1;return x;}", "E0300"),
        ("fn main()->i8{return 1i8+1i16;}", "E0300"),
        ("fn main()->bool{return 1u64==1;}", "E0300"),
        ("fn main()->u64{let a=1u64;return -a;}", "E0300"),
        ("struct u64{value:i32}fn main()->i32{return 0;}", "E0208"),
        ("enum i16{Value}fn main()->i32{return 0;}", "E0208"),
        (
            "struct Named{value:i32}use crate::Named as usize;fn main()->i32{return 0;}",
            "E0208",
        ),
    ] {
        for action in ["check", "run"] {
            let out = Project::new(source, "bounded").invoke(&[action, "--message-format=json"]);
            assert_eq!(out.status.code(), Some(1), "{source}: {out:?}");
            let text = String::from_utf8(out.stdout).unwrap();
            assert!(
                text.contains(&format!("\"code\":\"{code}\"")),
                "{source}: {text}"
            );
            assert!(!text.contains("E0500"), "{source}: {text}");
        }
    }
}

const COMPOSITE: &str = "struct Packet{lead:u8,small:i16,wide:u64,tail:u16,values:[u32;2]}
enum Reply{Value(Packet),Small(i8),Empty}
fn pass(value:Reply)->Reply{return value;}
fn bump(values:&mut[u32])->(){values[0]=values[0]+1u32;return;}
fn score(value:Reply)->u64{match value{Reply::Value(packet)=>{let mut moved=packet;bump(&mut moved.values);if moved.lead!=255u8||moved.small!=-32768i16||moved.tail!=65535u16||moved.values[0]!=4294967295u32||moved.values[1]!=17u32{return 0u64;}return moved.wide;},Reply::Small(number)=>{return 0u64;},Reply::Empty=>{return 0u64;},}}
fn main()->u64{let packet=Packet{lead:255u8,small:-32768i16,wide:18446744073709551615u64,tail:65535u16,values:[4294967294u32,17u32]};return score(pass(Reply::Value(packet)));}";

#[test]
fn mixed_alignment_array_loans_enum_payloads_and_early_snapshots_keep_their_values() {
    for policy in ["bounded", "hosted"] {
        Project::new(COMPOSITE, policy).result("18446744073709551615");
        Project::new("struct Counter{value:u64}fn next(value:&mut Counter)->u64{value.value=value.value+1u64;return value.value;}fn pair(a:u64,b:u64)->u64{return a*10u64+b;}fn main()->u64{let mut value=Counter{value:7000000000u64};return pair(value.value,next(&mut value));}", policy).result("77000000001");
    }
}

fn storage_cases() -> Vec<(String, &'static str)> {
    [
        ("i8", "-128i8", "127i8", "-128"),
        ("i16", "-32768i16", "32767i16", "-32768"),
        ("u16", "65535u16", "0u16", "65535"),
        ("u32", "4294967295u32", "0u32", "4294967295"),
        ("u64", "18446744073709551615u64", "0u64", "18446744073709551615"),
        ("isize", "-9223372036854775808isize", "9223372036854775807isize", "-9223372036854775808"),
        ("usize", "18446744073709551615usize", "0usize", "18446744073709551615"),
    ].into_iter().map(|(ty, value, other, expected)| {
        (format!("struct Packet{{lead:u8,value:{ty},tail:i32}}enum Value{{Direct({ty}),Record(Packet),Empty}}fn set(a:&mut[{ty}])->(){{a[0]={value};return;}}fn get(value:Value)->{ty}{{match value{{Value::Direct(number)=>{{return number;}},Value::Record(packet)=>{{if packet.lead!=255u8||packet.tail!=17{{return {other};}}return packet.value;}},Value::Empty=>{{return {other};}},}}}}fn main()->{ty}{{let mut a=[{other},{other}];set(&mut a);if a[1]!={other}{{return {other};}}let packet=Packet{{lead:255u8,value:a[0],tail:17}};let first=get(Value::Record(packet));return get(Value::Direct(first));}}"), expected)
    }).collect()
}

#[test]
fn every_width_survives_packed_fields_array_loans_and_both_enum_payload_forms() {
    for (source, expected) in storage_cases() {
        for policy in ["bounded", "hosted"] {
            Project::new(&source, policy).result(expected);
        }
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
fn native_all_integer_storage_widths_run_after_project_removal() {
    for (source, expected) in storage_cases() {
        for policy in ["bounded", "hosted"] {
            for profile in ["dev", "release"] {
                let p = Project::new(&source, policy);
                let built = p.invoke(&["build", "--profile", profile]);
                assert!(built.status.success(), "{source}: {built:?}");
                assert!(built.stderr.is_empty());
                let executable = p.0.join("integers");
                fs::copy(p.0.join(".oxid/bin/integers"), &executable).unwrap();
                fs::remove_dir_all(p.0.join("src")).unwrap();
                fs::remove_dir_all(p.0.join(".oxid")).unwrap();
                fs::remove_file(p.0.join("oxid.toml")).unwrap();
                let out = Command::new(executable)
                    .current_dir(&p.0)
                    .env_clear()
                    .output()
                    .unwrap();
                assert_eq!(out.status.code(), Some(0), "{out:?}");
                assert_eq!(out.stdout, format!("{expected}\n").as_bytes());
                assert!(out.stderr.is_empty());
            }
        }
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
fn native_integer_module_runs_without_sources_in_both_policies_and_profiles() {
    for policy in ["bounded", "hosted"] {
        for profile in ["dev", "release"] {
            let p = Project::new(
                "mod numbers;use crate::numbers::answer;fn main()->u64{return answer();}",
                policy,
            );
            fs::write(
                p.0.join("src/numbers.ox"),
                COMPOSITE.replace("fn main()->u64", "pub fn answer()->u64"),
            )
            .unwrap();
            let built = p.invoke(&["build", "--profile", profile]);
            assert!(built.status.success(), "{built:?}");
            assert!(built.stderr.is_empty());
            let executable = p.0.join("integers");
            fs::copy(p.0.join(".oxid/bin/integers"), &executable).unwrap();
            fs::remove_dir_all(p.0.join("src")).unwrap();
            fs::remove_dir_all(p.0.join(".oxid")).unwrap();
            fs::remove_file(p.0.join("oxid.toml")).unwrap();
            let out = Command::new(executable)
                .current_dir(&p.0)
                .env_clear()
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(0), "{out:?}");
            assert_eq!(out.stdout, b"18446744073709551615\n");
            assert!(out.stderr.is_empty());
        }
    }
}

#[test]
fn byte_suffixes_and_enum_payloads_reach_the_paid_public_pipeline() {
    for policy in ["bounded", "hosted"] {
        Project::new("enum E{Value(u8),Empty}fn take(value:E)->i32{match value{E::Value(byte)=>{return byte.to_i32();},E::Empty=>{return 0;},}}fn main()->i32{return take(E::Value(255u8));}", policy).result("255");
        Project::new("enum E{Value(u8)}fn main()->i32{return 0;}", policy).result("0");
    }
}
