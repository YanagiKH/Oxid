//! Real stack-record debugging through the public native build command.
#![cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new(source: &str, policy: &str, profile: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-record-debug-雪-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), format!("[project]\nname=\"record-debug\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\nprofile=\"{profile}\"\ndebug-info=true\nreproducible=true\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
        Self(root)
    }

    fn build(&self, extra: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .arg("build")
            .args(extra)
            .current_dir(&self.0)
            .stdin(Stdio::null());
        if let Some(llvm) = std::env::var_os("OXID_TEST_LLVM_BIN") {
            command.env("OXID_LLVM_BIN", llvm);
        }
        success(command.output().unwrap())
    }

    fn binary(&self) -> PathBuf {
        self.0.join(".oxid/bin/record-debug")
    }

    fn verify(&self) {
        let llvm = std::env::var_os("OXID_TEST_LLVM_BIN")
            .map(PathBuf::from)
            .map_or_else(
                || PathBuf::from("llvm-dwarfdump-19"),
                |dir| dir.join("llvm-dwarfdump"),
            );
        let result = success(
            Command::new(llvm)
                .arg("--verify")
                .arg(self.binary())
                .output()
                .unwrap(),
        );
        fs::write(self.0.join("dwarf-verify.stdout"), &result.stdout).unwrap();
        fs::write(self.0.join("dwarf-verify.stderr"), &result.stderr).unwrap();
    }

    fn output(&self) {
        output(self.binary());
    }

    fn debugger(&self, commands: &[String]) -> String {
        let mut command =
            Command::new(std::env::var_os("OXID_TEST_GDB").unwrap_or_else(|| "gdb".into()));
        command.current_dir(&self.0).args([
            "--batch",
            "--quiet",
            "-nx",
            "-iex",
            "set auto-load off",
            "-ex",
            "set disable-randomization off",
            "-ex",
            "set debuginfod enabled off",
        ]);
        for instruction in commands {
            command.args(["-ex", instruction]);
        }
        let result = command
            .arg("--args")
            .arg(self.binary())
            .stdin(Stdio::null())
            .output()
            .unwrap();
        fs::write(self.0.join("gdb.stdout"), &result.stdout).unwrap();
        fs::write(self.0.join("gdb.stderr"), &result.stderr).unwrap();
        let result = success(result);
        let text = String::from_utf8(result.stdout).unwrap();
        assert!(text.contains("exited normally"), "{text}");
        println!("{text}");
        text
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        if std::thread::panicking() || std::env::var_os("OXID_TEST_KEEP_ARTIFACTS").is_some() {
            eprintln!(
                "retained Copy-record debugger fixture: {}",
                self.0.display()
            );
        } else {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn success(result: Output) -> Output {
    assert!(result.status.success(), "{result:?}");
    result
}
fn output(binary: PathBuf) {
    let result = success(
        Command::new(binary)
            .env_clear()
            .stdin(Stdio::null())
            .output()
            .unwrap(),
    );
    assert_eq!(result.stdout, b"42\n");
    assert!(result.stderr.is_empty(), "{result:?}");
}
fn breakpoint(source: &str, needle: &str) -> String {
    let line = source
        .lines()
        .position(|line| line.contains(needle))
        .unwrap()
        + 1;
    format!("break src/main.ox:{line}")
}
fn section<'a>(text: &'a str, name: &str) -> &'a str {
    text.split_once(&format!("{name}_START\n"))
        .unwrap()
        .1
        .split_once(&format!("{name}_END\n"))
        .unwrap()
        .0
}

const POINT: &str = "#[derive(Copy)]\nstruct Point{x:i32,y:i32}\nfn by_value(prefix:i32,incoming:Point,suffix:i32)->Point{\n    let returned=incoming;\n    return returned;\n}\nfn inspect(prefix:i32,incoming:Point,suffix:i32)->i32{\n    let point=incoming;\n    let mut changed=point;\n    changed.x=changed.x+3;\n    let snapshot=changed;\n    changed.y=changed.y+3;\n    if prefix==0{\n        let point=Point{x:99,y:100};\n        let inside=point.x;\n    }\n    return point.x+changed.x+snapshot.y+changed.y+prefix+suffix;\n}\nfn main()->i32{\n    let original=Point{x:17,y:1};\n    let forwarded=by_value(0,original,0);\n    let result=inspect(0,forwarded,0);\n    return result+original.y-1;\n}\n";

#[test]
#[ignore = "requires Linux x86_64 or ARM64, LLVM 19.1.7 and GDB with ptrace"]
fn copy_record_parameters_independent_mutation_and_shadowed_scopes_are_real_gdb_values() {
    for policy in ["bounded", "hosted"] {
        for profile in ["dev", "release"] {
            let project = Project::new(POINT, policy, profile);
            project.build(&[]);
            project.verify();
            project.output();
            if profile == "release" {
                continue;
            }
            let text = project.debugger(&[
                breakpoint(POINT, "return returned;"), "run".into(),
                "echo PARAMETER_START\\n".into(), "info args".into(), "info locals".into(), "print returned.x".into(),
                "printf \"PARAMETER_VALUES %d %d %d %d\\n\", prefix, incoming.x, incoming.y, suffix".into(),
                "echo PARAMETER_END\\n".into(),
                breakpoint(POINT, "let inside="), "continue".into(), "echo INNER_START\\n".into(),
                "info args".into(), "info locals".into(), "ptype changed".into(), "print changed".into(),
                "printf \"INNER_VALUES %d %d %d %d %d %d %d %d\\n\", incoming.x, incoming.y, point.x, point.y, changed.x, changed.y, snapshot.x, snapshot.y".into(),
                "echo INNER_END\\n".into(), breakpoint(POINT, "return point.x"), "continue".into(),
                "echo OUTER_START\\n".into(), "info locals".into(), "print point.x".into(), "print changed.y".into(),
                "printf \"OUTER_VALUES %d %d %d %d %d %d\\n\", point.x, point.y, changed.x, changed.y, snapshot.x, snapshot.y".into(),
                "echo OUTER_END\\n".into(), "continue".into(),
            ]);
            let parameter = section(&text, "PARAMETER");
            assert!(
                parameter.contains("PARAMETER_VALUES 0 17 1 0") && parameter.contains("= 17"),
                "{text}"
            );
            for name in ["prefix = 0", "incoming =", "suffix = 0", "returned ="] {
                assert!(parameter.contains(name), "{name}: {text}");
            }
            let inner = section(&text, "INNER");
            for name in [
                "prefix = 0",
                "incoming =",
                "suffix = 0",
                "point =",
                "changed =",
                "snapshot =",
                "Point",
                "i32",
            ] {
                assert!(inner.contains(name), "{name}: {text}");
            }
            assert!(
                inner.contains("INNER_VALUES 17 1 99 100 20 4 20 1"),
                "{text}"
            );
            let outer = section(&text, "OUTER");
            assert!(
                outer.contains("OUTER_VALUES 17 1 20 4 20 1") && !outer.contains("inside ="),
                "{text}"
            );
            project.output();
        }
    }
}

const NUMERIC: &str = "#[derive(Copy)]\nstruct Point{x:i32,y:i32}\n#[derive(Copy)]\nstruct Numeric{small:i8,byte:u8,short:i16,ushort:u16,word:i32,uword:u32,wide:i64,uwide:u64,pointer:isize,upointer:usize,huge:i128,uhuge:u128,single:f32,double:f64}\nfn inspect(input:Numeric)->i32{\n    let preceding=Point{x:1,y:2};\n    let copy=input;\n    let answer=42;\n    return answer+preceding.x-1;\n}\nfn main()->i32{\n    let numbers=Numeric{small:-8i8,byte:250u8,short:-1600i16,ushort:65000u16,word:-320000,uword:4000000000u32,wide:-9223372036854775807i64,uwide:18446744073709551615u64,pointer:-123isize,upointer:456usize,huge:-170141183460469231731687303715884105727i128,uhuge:340282366920938463463374607431768211455u128,single:1.25f32,double:2.5};\n    return inspect(numbers);\n}\n";

#[test]
#[ignore = "requires Linux x86_64 or ARM64, LLVM 19.1.7 and GDB with ptrace"]
fn numeric_record_fields_follow_real_padding_width_signedness_and_nonzero_owner_offsets() {
    let fields = [
        ("small", "-8", 0),
        ("byte", "250", 1),
        ("short", "-1600", 2),
        ("ushort", "65000", 4),
        ("word", "-320000", 8),
        ("uword", "4000000000", 12),
        ("wide", "-9223372036854775807", 16),
        ("uwide", "18446744073709551615", 24),
        ("pointer", "-123", 32),
        ("upointer", "456", 40),
        ("huge", "-170141183460469231731687303715884105727", 48),
        ("uhuge", "340282366920938463463374607431768211455", 64),
        ("single", "1.25", 80),
        ("double", "2.5", 88),
    ];
    for policy in ["bounded", "hosted"] {
        for profile in ["dev", "release"] {
            let project = Project::new(NUMERIC, policy, profile);
            project.build(&[]);
            project.verify();
            project.output();
            if profile == "release" {
                continue;
            }
            let mut commands = vec![
                breakpoint(NUMERIC, "let answer="),
                "run".into(),
                "info args".into(),
                "info locals".into(),
                "ptype copy".into(),
            ];
            for (field, _, _) in fields {
                commands.extend([
                    format!("echo {field}_START\\n"),
                    format!("print copy.{field}"),
                    format!("echo {field}_END\\n"),
                ]);
            }
            // Use GDB's C expression parser only for byte-address arithmetic;
            // the named values and member locations still come from DWARF.
            commands.extend([
                "set language c".into(),
                "echo LAYOUT_START\\n".into(),
                "print sizeof(copy)".into(),
            ]);
            for (field, _, _) in fields {
                commands.push(format!(
                    "printf \"OFFSET_{field} %ld\\n\", (char*)&copy.{field}-(char*)&copy"
                ));
            }
            commands.extend([
                "printf \"DISTINCT %d\\n\", &copy != &input".into(),
                "echo LAYOUT_END\\n".into(),
                "continue".into(),
            ]);
            let text = project.debugger(&commands);
            for (field, value, _) in fields {
                assert!(
                    section(&text, field).contains(&format!("= {value}")),
                    "{field}: {text}"
                );
            }
            let layout = section(&text, "LAYOUT");
            assert!(
                layout.contains("= 96") && layout.contains("DISTINCT 1"),
                "{text}"
            );
            for (field, _, offset) in fields {
                assert!(
                    layout
                        .lines()
                        .any(|line| line == format!("OFFSET_{field} {offset}")),
                    "{field}: {text}"
                );
            }
        }
    }
}

const UNSUPPORTED: &str = "use alloc::bytes::ByteBuf;\nuse alloc::bytes::new;\n#[derive(Copy)]struct Point{x:i32}\n#[derive(Copy)]struct Boolean{x:i32,flag:bool}\n#[derive(Copy)]struct Unit{x:i32,nothing:()}\n#[derive(Copy)]struct Character{x:i32,letter:char}\n#[derive(Copy)]struct Nested{point:Point}\n#[derive(Copy)]struct Empty{}\n#[derive(Copy)]struct Generic<T:Copy>{value:T}\nstruct Moved{x:i32}\nstruct Resource{bytes:ByteBuf}\nfn inspect(reference:&Point,moving:Moved)->i32{\n    let boolean=Boolean{x:1,flag:true};\n    let unit=Unit{x:2,nothing:()};\n    let character=Character{x:3,letter:'雪'};\n    let nested=Nested{point:Point{x:4}};\n    let empty=Empty{};\n    let generic=Generic::<i32>{value:5};\n    let resource=Resource{bytes:new()};\n    let array=[1,2];\n    let visible=Point{x:42};\n    let result=visible.x;\n    return result;\n}\nfn main()->i32{\n    let point=Point{x:1};\n    return inspect(&point,Moved{x:2});\n}\n";

#[test]
#[ignore = "requires Linux x86_64 or ARM64, LLVM 19.1.7 and GDB with ptrace"]
fn unsupported_whole_records_resources_references_and_arrays_are_not_fictitious_gdb_values() {
    for policy in ["bounded", "hosted"] {
        let project = Project::new(UNSUPPORTED, policy, "dev");
        project.build(&[]);
        project.verify();
        project.output();
        let text = project.debugger(&[
            breakpoint(UNSUPPORTED, "let result="),
            "run".into(),
            "echo SUPPORTED_START\\n".into(),
            "info args".into(),
            "info locals".into(),
            "print visible.x".into(),
            "echo SUPPORTED_END\\n".into(),
            "continue".into(),
        ]);
        let values = section(&text, "SUPPORTED");
        assert!(
            values.contains("visible =") && values.contains("= 42"),
            "{text}"
        );
        for name in [
            "reference",
            "moving",
            "boolean",
            "unit",
            "character",
            "nested",
            "empty",
            "generic",
            "resource",
            "array",
        ] {
            assert!(!values.contains(&format!("{name} =")), "{name}: {text}");
        }
    }
}

#[test]
#[ignore = "requires Linux x86_64 or ARM64, LLVM 19.1.7 and GDB with ptrace"]
fn record_debug_bytes_survive_repeat_cache_relocation_and_source_removal() {
    for policy in ["bounded", "hosted"] {
        for profile in ["dev", "release"] {
            let mut project = Project::new(POINT, policy, profile);
            project.build(&["--incremental"]);
            project.verify();
            let expected = fs::read(project.binary()).unwrap();
            project.build(&["--incremental"]);
            assert_eq!(fs::read(project.binary()).unwrap(), expected);
            let relocated = project.0.with_extension("relocated-雪");
            fs::rename(&project.0, &relocated).unwrap();
            project.0 = relocated;
            fs::remove_dir_all(project.0.join(".oxid")).unwrap();
            project.build(&[]);
            assert_eq!(fs::read(project.binary()).unwrap(), expected);
            project.verify();
            if profile == "dev" {
                let text = project.debugger(&[
                    breakpoint(POINT, "return point.x"),
                    "run".into(),
                    "print changed.y".into(),
                    "continue".into(),
                ]);
                assert!(text.contains("= 4"), "{text}");
            }
            let deployed = project.0.join("deployed");
            fs::copy(project.binary(), &deployed).unwrap();
            fs::remove_dir_all(project.0.join("src")).unwrap();
            fs::remove_dir_all(project.0.join(".oxid")).unwrap();
            fs::remove_file(project.0.join("oxid.toml")).unwrap();
            output(deployed);
        }
    }
}
