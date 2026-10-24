//! f32/f64 values through ordinary project commands and deployed native programs.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::process::Stdio;
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
            "oxid-floats-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"),format!("[project]\nname=\"floats\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
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
        let output = self.invoke(&["run"]);
        assert_eq!(
            (output.status.code(), output.stdout, output.stderr),
            (Some(0), format!("{expected}\n").into_bytes(), Vec::new())
        );
        let output = self.invoke(&["run", "--message-format=json"]);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert!(output.stderr.is_empty());
        assert_eq!(String::from_utf8(output.stdout).unwrap(),format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":true,\"errors\":0,\"result\":{{\"type\":\"{ty}\",\"value\":\"{expected}\"}}}}\n"));
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn both_float_widths_preserve_all_ieee_classes_and_their_decimal_text() {
    for policy in ["bounded", "hosted"] {
        for bits in [
            0, 1, 0x007fffff, 0x00800000, 0x3dcccccd, 0x7f7fffff, 0x80000000, 0x7f800000,
            0xff800000, 0x7fc00001,
        ] {
            let expected = f32::from_bits(bits).to_string();
            Project::new(
                &format!("fn main()->f32{{let bits={bits}u32;return f32::from_bits(bits);}}"),
                policy,
            )
            .result("f32", &expected);
        }
        for bits in [
            0,
            1,
            0x000fffffffffffff,
            0x0010000000000000,
            0x3fb999999999999a,
            0x7fefffffffffffff,
            0x8000000000000000,
            0x7ff0000000000000,
            0xfff0000000000000,
            0x7ff8000000000001,
        ] {
            let expected = f64::from_bits(bits).to_string();
            Project::new(
                &format!("fn main()->f64{{let bits={bits}u64;return f64::from_bits(bits);}}"),
                policy,
            )
            .result("f64", &expected);
        }
    }
}

#[test]
fn literal_precision_exponents_and_explicit_rounding_use_the_selected_width() {
    for policy in ["bounded", "hosted"] {
        for (ty, body, expected) in [
            ("f64", "return 0.1+0.2;", "0.30000000000000004"),
            ("f64", "return 9007199254740993.0;", "9007199254740992"),
            ("f32", "return 0.1f32+0.2f32;", "0.3"),
            ("f64", "return 1e-3+1e+2;", "100.001"),
            ("f32", "let n=-9i128;return n.to_f32();", "-9"),
            (
                "f32",
                "let n=340282366920938463463374607431768211455u128;return n.to_f32();",
                "inf",
            ),
            ("f64", "let n=0.5f32;return n.to_f64();", "0.5"),
            ("f32", "let n=0.1;return n.to_f32();", "0.1"),
            ("f32", "let n=0f32;return -n;", "-0"),
            ("f64", "let n=-7f64;return n%2f64;", "-1"),
        ] {
            Project::new(&format!("fn main()->{ty}{{{body}}}"), policy).result(ty, expected);
        }
    }
}

#[test]
fn arithmetic_nan_bits_are_canonical_and_bit_constructors_preserve_payloads() {
    for policy in ["bounded", "hosted"] {
        for (ty,body,expected) in [
            ("u32","let bits=2143289345u32;let n=f32::from_bits(bits);return n.to_bits();","2143289345"),
            ("u32","let bits=2143289345u32;let n=f32::from_bits(bits);let x=n+0f32;return x.to_bits();","2143289344"),
            ("u64","let n=0f64/0f64;return n.to_bits();","9221120237041090560"),
            ("bool","let n=0f64/0f64;return n!=n;","true"),
            ("bool","let n=0f64/0f64;return n<n;","false"),
            ("bool","return -0.0==0.0;","true"),
        ] {
            let output=Project::new(&format!("fn main()->{ty}{{{body}}}"),policy).invoke(&["run"]);
            assert_eq!((output.status.code(),output.stdout,output.stderr),(Some(0),format!("{expected}\n").into_bytes(),Vec::new()));
        }
    }
}

#[test]
fn float_values_survive_records_arrays_borrows_enums_and_function_calls() {
    for policy in ["bounded", "hosted"] {
        Project::new("struct Packet{values:[f32;2],scale:f64}fn scaled(packet:&Packet)->f64{let first=packet.values[0];let value=first.to_f64();return value*packet.scale;}fn main()->f64{let mut packet=Packet{values:[0.5f32,-0.25f32],scale:8f64};packet.values[0]=1.5f32;return scaled(&packet);}",policy).result("f64","12");
        Project::new("enum Number{Value(f64),Empty}fn unpack(value:Number)->f64{match value{Number::Value(n)=>{return n;},Number::Empty=>{return 0.0;}}}fn main()->f64{return unpack(Number::Value(0.25));}",policy).result("f64","0.25");
    }
}

#[test]
fn invalid_literals_and_mixed_types_fail_during_checking() {
    for (source, code) in [
        ("fn main()->f32{return 3.5e38f32;}", "E0301"),
        ("fn main()->f64{return 1e309;}", "E0301"),
        ("fn main()->f64{return 1e-;}", "E0101"),
        ("fn main()->f32{return 1.0;}", "E0300"),
        ("fn main()->f64{return 1.0+1;}", "E0300"),
        ("fn main()->f64{return 1.0+1f32;}", "E0300"),
        (
            "fn main()->f32{let bits=1u64;return f32::from_bits(bits);}",
            "E0300",
        ),
        ("fn main()->u32{let n=1;return n.to_bits();}", "E0300"),
        ("fn main()->f64{return 1.0&1.0;}", "E0300"),
        ("struct f32{value:i32}fn main()->i32{return 0;}", "E0208"),
    ] {
        let output = Project::new(source, "bounded").invoke(&["check", "--message-format=json"]);
        assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
        assert!(output.stderr.is_empty());
        let message = String::from_utf8(output.stdout).unwrap();
        assert!(
            message.contains(&format!("\"code\":\"{code}\"")),
            "{source}: {message}"
        );
        assert!(!message.contains("E0500"));
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires Linux x86_64 and LLVM 19.1.7"]
fn native_float_output_survives_source_removal_and_reports_broken_pipes() {
    for policy in ["bounded", "hosted"] {
        for (ty, bits, expected) in [
            ("f32", "1u32", f32::from_bits(1).to_string()),
            ("f64", "1u64", f64::from_bits(1).to_string()),
            ("f64", "9223372036854775808u64", "-0".to_owned()),
            ("f32", "2139095040u32", "inf".to_owned()),
            ("f64", "9221120237041090561u64", "NaN".to_owned()),
        ] {
            let project = Project::new(
                &format!("fn main()->{ty}{{let bits={bits};return {ty}::from_bits(bits);}}"),
                policy,
            );
            let deployment = project.0.join("deployment");
            fs::create_dir(&deployment).unwrap();
            for profile in ["dev", "release"] {
                for _ in 0..2 {
                    let built = project.invoke(&["build", "--profile", profile]);
                    assert!(built.status.success(), "{built:?}");
                    assert!(built.stderr.is_empty());
                }
                fs::copy(project.0.join(".oxid/bin/floats"), deployment.join(profile)).unwrap();
            }
            fs::remove_dir_all(project.0.join("src")).unwrap();
            fs::remove_dir_all(project.0.join(".oxid")).unwrap();
            fs::remove_file(project.0.join("oxid.toml")).unwrap();
            for profile in ["dev", "release"] {
                let binary = deployment.join(profile);
                let output = Command::new(&binary)
                    .current_dir(&deployment)
                    .env_clear()
                    .env("PATH", "/no-tools")
                    .output()
                    .unwrap();
                assert_eq!(
                    (output.status.code(), output.stdout, output.stderr),
                    (Some(0), format!("{expected}\n").into_bytes(), Vec::new())
                );
                let (writer, reader) = std::os::unix::net::UnixStream::pair().unwrap();
                drop(reader);
                let output = Command::new(&binary)
                    .current_dir(&deployment)
                    .env_clear()
                    .stdout(Stdio::from(std::os::fd::OwnedFd::from(writer)))
                    .output()
                    .unwrap();
                assert_eq!(output.status.code(), Some(74));
                assert!(output.stderr.is_empty());
            }
        }
    }
}

#[test]
fn formatting_preserves_scientific_literals_and_qualified_bit_constructors() {
    let project=Project::new("fn main()->f64{let bits=4607182418800017408u64;let n=f64::from_bits(bits);return n*1e-3+1e+2;}","bounded");
    let formatted = project.invoke(&["fmt", "src/main.ox"]);
    assert!(formatted.status.success(), "{formatted:?}");
    assert!(formatted.stderr.is_empty());
    let text = String::from_utf8(formatted.stdout).unwrap();
    assert!(
        text.contains("1e-3") && text.contains("1e+2") && text.contains("f64::from_bits(bits)"),
        "{text}"
    );
    fs::write(project.0.join("src/main.ox"), text).unwrap();
    project.result("f64", "100.001");
    let repeated = project.invoke(&["fmt", "--check", "src/main.ox"]);
    assert!(repeated.status.success(), "{repeated:?}");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires Linux x86_64 and LLVM 19.1.7"]
fn native_float_aggregates_and_quad_conversions_keep_their_values() {
    for policy in ["bounded", "hosted"] {
        for (source, expected) in [
            ("struct Packet{values:[f32;2],scale:f64}fn scaled(packet:&Packet)->f64{let first=packet.values[0];let value=first.to_f64();return value*packet.scale;}fn main()->f64{let mut packet=Packet{values:[0.5f32,-0.25f32],scale:8f64};packet.values[0]=1.5f32;return scaled(&packet);}","12"),
            ("enum Number{Value(f64),Empty}fn unpack(value:Number)->f64{match value{Number::Value(n)=>{return n;},Number::Empty=>{return 0.0;}}}fn main()->f64{return unpack(Number::Value(0.25));}","0.25"),
            ("fn convert(n:u128)->f32{return n.to_f32();}fn main()->f32{return convert(340282366920938463463374607431768211455u128);}","inf"),
        ] {
            let project=Project::new(source,policy);
            for profile in ["dev","release"] {
                let built=project.invoke(&["build","--profile",profile]);assert!(built.status.success(),"{built:?}");assert!(built.stderr.is_empty());
                let output=Command::new(project.0.join(".oxid/bin/floats")).env_clear().output().unwrap();
                assert_eq!((output.status.code(),output.stdout,output.stderr),(Some(0),format!("{expected}\n").into_bytes(),Vec::new()));
            }
        }
    }
}

#[test]
fn primitive_type_names_keep_the_existing_value_namespace() {
    for policy in ["bounded", "hosted"] {
        let result = Project::new(
            "fn f32()->i32{return 7;}fn main()->i32{let f64=1;return f32()+f64;}",
            policy,
        )
        .invoke(&["run"]);
        assert_eq!(
            (result.status.code(), result.stdout, result.stderr),
            (Some(0), b"8\n".to_vec(), Vec::new())
        );
    }
}

#[test]
fn unused_float_expressions_are_checked_without_legacy_evaluation() {
    let project = Project::new("fn main()->(){9007199254740993.0;return;}", "bounded");
    let checked = project.invoke(&["check"]);
    assert!(checked.status.success(), "{checked:?}");
    let result = project.invoke(&["run"]);
    assert_eq!(
        (result.status.code(), result.stdout, result.stderr),
        (Some(0), b"()\n".to_vec(), Vec::new())
    );
}
