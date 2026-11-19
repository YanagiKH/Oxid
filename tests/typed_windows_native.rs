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
        self.invoke_in("", arguments)
    }
    fn invoke_in(&self, directory: &str, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(arguments)
            .current_dir(self.0.join(directory))
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
    let output = fixture.invoke(&[
        "compile",
        "main.ox",
        "--edition=typed-preview",
        "--entry-mode=process",
        "--backend=llvm",
        "--output=program.exe",
        "--message-format=json",
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("\"code\":\"E0608\",\"stage\":\"oir-run\"")
            && text.contains(
                "\"message\":\"process execution requires Linux or macOS on x86_64 or ARM64\""
            ),
        "{text}"
    );
    assert!(
        text.contains(
            "\"kind\":\"compile-summary\",\"success\":false,\"errors\":1,\"output\":null"
        ),
        "{text}"
    );
    assert!(
        !text.contains("E0700") && !text.contains("E0701") && !text.contains("E0500"),
        "{text}"
    );
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

#[test]
fn new_windows_project_runs_declared_tests_and_reaches_native_tool_selection() {
    let fixture = Fixture::new("");
    let created = fixture.invoke(&["new", "app", "--edition=typed-preview"]);
    assert!(created.status.success(), "{created:?}");
    let manifest = fs::read_to_string(fixture.0.join("app/oxid.toml")).unwrap();
    assert!(manifest.contains("entry-mode = \"result\""));
    let readme = fs::read_to_string(fixture.0.join("app/README.md")).unwrap();
    assert!(readme.contains(r".oxid\bin\app.exe") && readme.contains("Windows SDK"));
    for args in [&["check"][..], &["test", "--message-format=json"][..]] {
        let output = fixture.invoke_in("app", args);
        assert!(output.status.success(), "{args:?}: {output:?}");
    }
    let output = fixture.invoke_in("app", &["run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"0\n");
    assert!(output.stderr.is_empty());
    let output = fixture.invoke_in("app", &["build", "--message-format=json"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("\"code\":\"E0701\"") && text.contains("missing-tools"),
        "{text}"
    );
    assert!(!text.contains("E0700") && !text.contains("E0006"), "{text}");
    assert!(!fixture.0.join("app/.oxid/bin/app.exe").exists());
}

fn declared_fixture() -> Fixture {
    let fixture =
        Fixture::new("this application body must not run when selecting its declared test");
    fs::create_dir(fixture.0.join("tests")).unwrap();
    fs::write(fixture.0.join("oxid.toml"), "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[tests]\nsmoke=\"tests/smoke.ox\"\n").unwrap();
    fs::write(
        fixture.0.join("tests/stray.ox"),
        "unlisted invalid source must never load",
    )
    .unwrap();
    fixture
}

#[test]
fn declared_pure_tests_use_full_i32_status_and_do_not_print_result_values() {
    let fixture = declared_fixture();
    for status in [0, 7, -1, 256, i32::MIN, i32::MAX] {
        fs::write(
            fixture.0.join("tests/smoke.ox"),
            format!("fn main()->i32{{return {status};}}"),
        )
        .unwrap();
        let output = fixture.invoke(&["test", "--message-format=json"]);
        assert_eq!(
            output.status.code(),
            Some(i32::from(status != 0)),
            "{output:?}"
        );
        assert!(output.stderr.is_empty(), "{output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            text.contains(&format!("\"exit_status\":{status}")),
            "{text}"
        );
        assert!(text.contains("\"stdout\":\"\",\"stderr\":\"\""), "{text}");
        assert!(
            text.contains(if status == 0 {
                "\"passed\":1,\"failed\":0"
            } else {
                "\"passed\":0,\"failed\":1"
            }),
            "{text}"
        );
        fixture.clean();
    }
}

#[test]
fn invalid_test_signatures_scope_and_runtime_errors_never_report_success() {
    let fixture = declared_fixture();
    for (source, code) in [
        ("fn main()->bool{while true{}return false;}", "E0600"),
        ("fn main()->(){let bad=1/0;return;}", "E0600"),
        (
            "fn main(value:i32)->i32{while true{}return value;}",
            "E0600",
        ),
        ("fn helper()->i32{return 0;}", "E0600"),
        ("struct P{}fn main()->P{return P{};}", "E0608"),
        ("struct P{}fn main()->i32{let bad=1/0;return 0;}", "E0608"),
        (
            "fn unused<T:Copy>(value:T)->T{return value;}fn main()->i32{return 0;}",
            "E0608",
        ),
        (
            "use std::io::write_stdout;fn main()->i32{return 0;}",
            "E0608",
        ),
        ("fn main()->i32{return 2147483647+1;}", "E0604"),
        ("fn main()->i32{return 1/0;}", "E0607"),
        ("fn main()->i32{while true{}return 0;}", "E0601"),
    ] {
        fs::write(fixture.0.join("tests/smoke.ox"), source).unwrap();
        let output = fixture.invoke(&["test", "--message-format=json"]);
        assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains(&format!("error[{code}]")), "{source}: {text}");
        assert!(
            text.contains("\"passed\":0,\"failed\":1") && text.contains("\"stdout\":\"\""),
            "{text}"
        );
        assert!(!text.contains("E0500"), "{text}");
        if code == "E0600" {
            assert!(
                text.contains("test entry requires")
                    && !text.contains("E0601")
                    && !text.contains("E0607"),
                "{text}"
            );
        }
        fixture.clean();
    }
    let manifest = fs::read_to_string(fixture.0.join("oxid.toml")).unwrap();
    fs::write(
        fixture.0.join("oxid.toml"),
        format!("{manifest}[execution]\npolicy=\"hosted\"\n"),
    )
    .unwrap();
    fs::write(
        fixture.0.join("tests/smoke.ox"),
        "fn main()->i32{while true{}return 0;}",
    )
    .unwrap();
    let output = fixture.invoke(&["test", "--message-format=json"]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("E0608")
            && text.contains("require bounded execution")
            && !text.contains("E0601"),
        "{text}"
    );
    fixture.clean();
}

#[test]
fn the_pure_test_adapter_cannot_be_selected_by_forged_general_run_options() {
    let fixture = declared_fixture();
    fs::write(
        fixture.0.join("tests/smoke.ox"),
        "fn main()->i32{return 0;}",
    )
    .unwrap();
    for arguments in [
        vec![
            "run",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--project-test=unknown",
        ],
        vec![
            "run",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--project-test=",
        ],
        vec![
            "run",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--project-test=smoke",
            "--project-test=smoke",
        ],
        vec![
            "run",
            "tests/smoke.ox",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--project-test=smoke",
        ],
        vec!["run", "--entry-mode=process", "--project-test=smoke"],
        vec!["run", "--edition=typed-preview", "--project-test=smoke"],
        vec![
            "run",
            "--edition=typed-preview",
            "--entry-mode=result",
            "--project-test=smoke",
        ],
        vec![
            "run",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--project-test=smoke",
            "--message-format=json",
        ],
        vec![
            "run",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--project-test=smoke",
            "--workspace",
        ],
        vec![
            "run",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--project-test=smoke",
            "--package=app",
        ],
    ] {
        let output = fixture.invoke(&arguments);
        assert_eq!(output.status.code(), Some(1), "{arguments:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}: {output:?}");
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(
            error.contains("invalid internal project test invocation")
                || error.contains("unknown declared project test"),
            "{arguments:?}: {error}"
        );
        fixture.clean();
    }
    // A general process run keeps its unchanged unsupported-host behavior.
    fs::write(fixture.0.join("main.ox"), "fn main()->i32{return 0;}").unwrap();
    let output = fixture.invoke(&["run", "--entry-mode=process"]);
    assert_eq!(output.status.code(), Some(74), "{output:?}");
    assert!(
        output.stdout.is_empty() && output.stderr.is_empty(),
        "{output:?}"
    );
}

#[test]
fn windows_test_option_errors_describe_the_pure_entry_contract() {
    let fixture = declared_fixture();
    fs::write(
        fixture.0.join("tests/smoke.ox"),
        "fn main()->i32{let bad=1/0;return 0;}",
    )
    .unwrap();
    let output = fixture.invoke(&["test", "--entry-mode=result"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty());
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(
        text.contains("declared tests require fn main() -> i32 with no parameters"),
        "{text}"
    );
    assert!(
        !text.contains("always use process mode") && !text.contains("E0607"),
        "{text}"
    );
    fixture.clean();
}
