//! Square roots retain the operand width and the published IEEE bit contracts.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new(source: &str, policy: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-float-roots-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"),format!("[project]\nname=\"roots\"\nedition=\"typed-preview\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
        Self(root)
    }
    fn invoke(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    fn result(&self, expected: &str) {
        let output = self.invoke(&["run"]);
        assert_eq!(
            (output.status.code(), output.stdout, output.stderr),
            (Some(0), format!("{expected}\n").into_bytes(), Vec::new())
        );
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("retained float root fixture: {}", self.0.display());
        } else {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
const COMPOSED:&str="struct Reading{value:f64}enum Number{Value(f32),Empty}fn identity<T>(value:T)->T{return value;}fn measured(reading:&Reading)->f64{let field=reading.value;let value=identity(field);return value.sqrt();}fn unpack(number:Number)->f32{match number{Number::Value(field)=>{let value=identity(field);return value.sqrt();},Number::Empty=>{return 0f32;}}}fn main()->i32{let reading=Reading{value:81f64};let small=unpack(Number::Value(16f32));if measured(&reading)!=9f64{return 1;}if small!=4f32{return 2;}return 0;}";

#[test]
fn square_root_bits_cover_both_widths_and_ieee_boundaries() {
    for policy in ["bounded", "hosted"] {
        Project::new(
            "fn main()->f64{let value=81f64;return value.sqrt();}",
            policy,
        )
        .result("9");
        Project::new(
            "fn main()->f32{let zero=0f32;let value=-zero;return value.sqrt();}",
            policy,
        )
        .result("-0");
        for (input, expected) in [
            (0u32, 0u32),
            (0x80000000, 0x80000000),
            (1, 0x1a3504f3),
            (0x00800000, 0x20000000),
            (0x40000000, 0x3fb504f3),
            (0x7f800000, 0x7f800000),
            (0xff800000, 0x7fc00000),
            (0x7f800001, 0x7fc00000),
            (0xbf800000, 0x7fc00000),
        ] {
            Project::new(&format!("fn main()->u32{{let bits={input}u32;let value=f32::from_bits(bits);let root=value.sqrt();return root.to_bits();}}"),policy).result(&expected.to_string());
        }
        for (input, expected) in [
            (0u64, 0u64),
            (0x8000000000000000, 0x8000000000000000),
            (1, 0x1e60000000000000),
            (0x0010000000000000, 0x2000000000000000),
            (0x4000000000000000, 0x3ff6a09e667f3bcd),
            (0x7ff0000000000000, 0x7ff0000000000000),
            (0xfff0000000000000, 0x7ff8000000000000),
            (0x7ff0000000000001, 0x7ff8000000000000),
            (0xbff0000000000000, 0x7ff8000000000000),
        ] {
            Project::new(&format!("fn main()->u64{{let bits={input}u64;let value=f64::from_bits(bits);let root=value.sqrt();return root.to_bits();}}"),policy).result(&expected.to_string());
        }
    }
}

#[test]
fn square_roots_reject_other_types_and_keep_bare_field_names() {
    for source in [
        "fn main()->i32{let value=4;return value.sqrt();}",
        "fn main()->bool{let value=true;return value.sqrt();}",
        "fn main()->f32{let value=4f64;return value.sqrt();}",
        "fn main()->f64{let value=[4f64];return value.sqrt();}",
        "fn root<T>(value:T)->T{return value.sqrt();}fn main()->i32{return 0;}",
    ] {
        let project = Project::new(source, "bounded");
        let output = project.invoke(&["check", "--message-format=json"]);
        assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
        assert!(output.stderr.is_empty());
        let message = String::from_utf8(output.stdout).unwrap();
        assert!(message.contains("\"code\":\"E0300\""), "{message}");
        assert!(!message.contains("E0500"));
        assert!(!project.0.join(".oxid/bin/roots").exists());
    }
    Project::new(
        "struct Reading{sqrt:i32}fn main()->i32{let value=Reading{sqrt:7};return value.sqrt;}",
        "bounded",
    )
    .result("7");
}

#[test]
fn square_roots_compose_with_generics_payloads_borrows_and_formatting() {
    for policy in ["bounded", "hosted"] {
        let project = Project::new(COMPOSED, policy);
        project.result("0");
        let formatted = project.invoke(&["fmt", "--edition=typed-preview", "src/main.ox"]);
        assert!(formatted.status.success(), "{formatted:?}");
        assert!(formatted.stderr.is_empty());
        fs::write(project.0.join("src/main.ox"), formatted.stdout).unwrap();
        project.result("0");
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
fn native_square_roots_keep_composed_results_after_source_removal() {
    for policy in ["bounded", "hosted"] {
        let project = Project::new(COMPOSED, policy);
        let mut executables = Vec::new();
        for profile in ["dev", "release"] {
            let output = project.invoke(&["build", "--profile", profile]);
            assert!(output.status.success(), "{output:?}");
            let path = project.0.join(format!("deployed-{profile}"));
            fs::copy(project.0.join(".oxid/bin/roots"), &path).unwrap();
            executables.push(path);
        }
        fs::remove_dir_all(project.0.join("src")).unwrap();
        for path in executables {
            let output = Command::new(path)
                .env_clear()
                .current_dir(&project.0)
                .stdin(Stdio::null())
                .output()
                .unwrap();
            assert_eq!(
                (output.status.code(), output.stdout, output.stderr),
                (Some(0), b"0\n".to_vec(), Vec::new())
            );
        }
    }
}
