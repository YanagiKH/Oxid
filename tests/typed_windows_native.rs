//! Windows MSVC scalar native scope through the real public compiler commands.
#![cfg(all(
    target_os = "windows",
    target_env = "msvc",
    target_arch = "x86_64",
    target_pointer_width = "64",
    target_endian = "little"
))]
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-windows-native-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("main.ox"), source).unwrap();
        if source.starts_with("mod marker;") {
            fs::write(path.join("marker.ox"), "").unwrap();
        }
        Self(path)
    }
    fn invoke(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(arguments)
            .current_dir(&self.0)
            .env("OXID_LLVM_BIN", self.0.join("missing-tools"))
            .env("OXID_CACHE_DIR", self.0.join("external-cache"))
            .output()
            .unwrap()
    }
    fn clean(&self) {
        for name in [".oxid", "external-cache", "program.exe", "oxid.lock"] {
            assert!(!self.0.join(name).exists(), "unexpected {name}");
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn denial(output: Output) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("\"code\":\"E0700\"") && text.contains("Windows native"),
        "{text}"
    );
    assert!(!text.contains("E0701") && !text.contains("E0500"), "{text}");
}
#[test]
fn unverified_unused_declarations_are_rejected_before_tools_and_artifacts() {
    for source in [
        "struct P{x:i32} fn main()->i32{return 42;}",
        "#[derive(Copy)]struct P{x:i32} fn main()->i32{return 42;}",
        "enum E{Value} fn main()->i32{return 42;}",
        "struct P{x:i32} fn unused(p:&P)->i32{return p.x;} fn main()->i32{return 42;}",
        "fn unused(a:&[i32])->i32{return a[0];} fn main()->i32{return 42;}",
        "fn unused()->i32{let a:[i32;1]=[1];return a[0];} fn main()->i32{return 42;}",
        "use alloc::vec::new;fn unused()->(){let values=new::<i32>();return;}fn main()->i32{return 42;}",
        "use std::fs::File;fn unused(file:File)->(){return;}fn main()->i32{return 42;}",
        "fn unused<T:Copy>(value:T)->T{return value;} fn main()->i32{return 42;}",
        "fn unused(value:u8)->u8{return value;} fn main()->i32{return 42;}",
        "fn unused(value:i64)->i64{return value;} fn main()->i32{return 42;}",
        "fn unused(value:i128)->i128{return value;} fn main()->i32{return 42;}",
        "fn unused(value:u128)->u128{return value;} fn main()->i32{return 42;}",
        "fn unused(value:f32)->f32{return value;} fn main()->i32{return 42;}",
        "fn unused(value:f64)->f64{return value;} fn main()->i32{return 42;}",
        "fn unused(value:char)->char{return value;} fn main()->i32{return 42;}",
        "mod marker;fn unused()->i32{let n=1;let f=n.to_f64();return 0;}fn main()->i32{return 42;}",
        "mod marker;fn unused()->i32{let n=1;let f=n.to_f32();return 0;}fn main()->i32{return 42;}",
        "mod marker;fn unused()->i32{let n=65;let c=n.to_char_checked();return 0;}fn main()->i32{return 42;}",
        "mod marker;fn unused()->i32{let n=65;let c=n.to_char_checked();let scalar=c.to_u32();return 0;}fn main()->i32{return 42;}",
        "mod marker;fn unused()->i32{let n=1;let f=n.to_f64();let root=f.sqrt();return 0;}fn main()->i32{return 42;}",
        "mod marker;fn unused()->i32{let n=1;let f=n.to_f64();let bits=f.to_bits();return 0;}fn main()->i32{return 42;}",
        "mod marker;fn unused()->i32{let n=1;let f=n.to_f64();let bits=f.to_bits();let back=f64::from_bits(bits);return 0;}fn main()->i32{return 42;}",
    ] {
        let fixture = Fixture::new(source);
        let checked = fixture.invoke(&["check", "main.ox", "--edition=typed-preview", "--message-format=json"]);
        assert!(checked.status.success(), "fixture must be valid source: {source}: {checked:?}");
        denial(fixture.invoke(&["compile", "main.ox", "--edition=typed-preview", "--backend=llvm", "--output=program.exe", "--message-format=json"]));
        fixture.clean();
    }
}
#[test]
fn unsupported_execution_and_debug_settings_do_not_create_managed_outputs() {
    for (execution, option) in [("hosted", "--no-debug-info"), ("bounded", "--debug-info")] {
        let fixture = Fixture::new("fn main()->i32{return 42;}");
        fs::write(fixture.0.join("oxid.toml"), format!("[project]\nname=\"native-scope\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{execution}\"\n")).unwrap();
        denial(fixture.invoke(&[
            "build",
            "--profile=release",
            option,
            "--message-format=json",
        ]));
        fixture.clean();
    }
    let fixture = Fixture::new("fn main()->i32{return 0;}");
    denial(fixture.invoke(&[
        "compile",
        "main.ox",
        "--edition=typed-preview",
        "--entry-mode=process",
        "--backend=llvm",
        "--output=program.exe",
        "--message-format=json",
    ]));
    fixture.clean();
}

#[test]
fn unused_child_module_features_are_rejected_by_file_and_both_project_profiles() {
    for hidden in [
        "struct Private{x:i32}",
        "fn unused<T:Copy>(value:T)->T{return value;}",
        "fn unused()->i32{let n=1;let value=n.to_f64();return 0;}",
    ] {
        let fixture = Fixture::new("mod hidden;fn main()->i32{return 42;}");
        fs::write(fixture.0.join("hidden.ox"), hidden).unwrap();
        let checked = fixture.invoke(&[
            "check",
            "main.ox",
            "--edition=typed-preview",
            "--message-format=json",
        ]);
        assert!(
            checked.status.success(),
            "valid unused module: {hidden}: {checked:?}"
        );
        denial(fixture.invoke(&[
            "compile",
            "main.ox",
            "--edition=typed-preview",
            "--backend=llvm",
            "--output=program.exe",
            "--message-format=json",
        ]));
        fixture.clean();
        fs::write(fixture.0.join("oxid.toml"), "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n").unwrap();
        for profile in ["dev", "release"] {
            denial(fixture.invoke(&["build", "--profile", profile, "--message-format=json"]));
            fixture.clean();
        }
    }
}
