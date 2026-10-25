//! Public debug settings and source-level debugging of real native programs.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-debug-雪-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let project = Self(root);
        project.write("src/main.ox", "fn main()->i32{return 42;}\n");
        project.manifest("");
        project
    }
    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn manifest(&self, extra: &str) {
        self.write("oxid.toml", &format!("[project]\nname=\"debugging\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n{extra}"));
    }
    fn run(&self, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command.args(args).current_dir(&self.0).stdin(Stdio::null());
        if let Some(llvm) = std::env::var_os("OXID_TEST_LLVM_BIN") {
            command.env("OXID_LLVM_BIN", llvm);
        }
        command.output().unwrap()
    }
    fn binary(&self) -> PathBuf {
        self.0.join(".oxid/bin/debugging")
    }
    fn no_outputs(&self) {
        assert!(!self.0.join(".oxid").exists());
        assert!(!self.0.join("oxid.lock").exists());
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("retained native debug fixture: {}", self.0.display());
        } else {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn success(output: Output) -> Output {
    assert!(output.status.success(), "{output:?}");
    output
}

#[test]
fn debug_flags_reject_duplicates_values_and_non_build_commands_before_effects() {
    let project = Project::new();
    for args in [
        vec!["build", "--debug-info", "--debug-info"],
        vec!["build", "--no-debug-info", "--debug-info"],
        vec!["build", "--debug-info=true"],
        vec!["build", "--debug-info", "false"],
        vec!["check", "--debug-info"],
        vec!["run", "--debug-info"],
    ] {
        let result = project.run(&args);
        assert!(!result.status.success(), "{args:?}: {result:?}");
        project.no_outputs();
    }
}

#[test]
fn manifest_debug_setting_requires_one_unquoted_boolean() {
    let project = Project::new();
    for value in ["1", "\"true\"", "yes", "true\ndebug-info=false"] {
        project.manifest(&format!("debug-info={value}\n"));
        let result = project.run(&["check"]);
        assert!(!result.status.success(), "{value}: {result:?}");
        assert!(String::from_utf8_lossy(&result.stderr).contains("debug-info"));
        project.no_outputs();
    }
    for value in ["true", "false # optional metadata"] {
        project.manifest(&format!("debug-info={value}\n"));
        success(project.run(&["check"]));
        project.no_outputs();
    }
}

#[test]
fn debug_info_does_not_admit_invalid_source_or_replace_existing_outputs() {
    let project = Project::new();
    project.write("src/main.ox", "fn main()->i32{return missing;}\n");
    fs::create_dir_all(project.binary().parent().unwrap()).unwrap();
    fs::write(project.binary(), b"previous executable").unwrap();
    let result = project.run(&["build", "--debug-info", "--incremental"]);
    assert!(!result.status.success(), "{result:?}");
    assert!(String::from_utf8_lossy(&result.stderr).contains("missing"));
    assert_eq!(fs::read(project.binary()).unwrap(), b"previous executable");
}

#[test]
fn virtual_workspaces_cannot_silently_ignore_debug_build_settings() {
    let project = Project::new();
    project.write(
        "oxid.toml",
        "[workspace]\nedition=\"typed-preview\"\nmembers=[\"member\"]\n[build]\ndebug-info=true\n",
    );
    project.write(
        "member/oxid.toml",
        "[project]\nname=\"member\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n",
    );
    project.write("member/main.ox", "fn main()->i32{return 0;}\n");
    let output = project.run(&["build", "--workspace", "--no-debug-info"]);
    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("virtual workspace"),
        "{output:?}"
    );
    project.no_outputs();
    assert!(!project.0.join("member/.oxid").exists());
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod native {
    use super::*;
    use std::path::Path;

    const MAIN: &str = "mod math;\nuse crate::math::answer;\nuse alloc::bytes::new;\nuse alloc::bytes::try_push;\nfn main()->i32{\n    let mut values=new();\n    try_push(&mut values,7u8);\n    return answer(values);\n}\n";
    const MATH: &str = "use alloc::bytes::ByteBuf;\nuse alloc::bytes::len;\nenum Decision<T>{Keep(T),Empty}\npub fn answer(values:ByteBuf)->i32{\n    let decision=Decision::Keep(values);\n    match decision{\n        Decision::Keep(value)=>{return len(&value)+41;},\n        Decision::Empty=>{return 0;}\n    }\n}\n";

    fn llvm(name: &str) -> PathBuf {
        std::env::var_os("OXID_TEST_LLVM_BIN")
            .map(PathBuf::from)
            .map_or_else(|| PathBuf::from(format!("{name}-19")), |dir| dir.join(name))
    }
    fn dwarf(binary: &Path, option: &str) -> Output {
        success(
            Command::new(llvm("llvm-dwarfdump"))
                .arg(option)
                .arg(binary)
                .output()
                .unwrap(),
        )
    }
    fn debugger(project: &Project, binary: &Path, breakpoint: &str) {
        let gdb = std::env::var_os("OXID_TEST_GDB").unwrap_or_else(|| "gdb".into());
        let result = success(
            Command::new(gdb)
                .current_dir(&project.0)
                .args([
                    "--batch",
                    "--quiet",
                    "-nx",
                    "-iex",
                    "set auto-load off",
                    "-ex",
                    "set disable-randomization off",
                    "-ex",
                    "set debuginfod enabled off",
                    "-ex",
                    &format!("break {breakpoint}"),
                    "-ex",
                    "run",
                    "-ex",
                    "backtrace",
                    "-ex",
                    "info line",
                    "-ex",
                    "disable 1",
                    "-ex",
                    "continue",
                    "--args",
                ])
                .arg(binary)
                .stdin(Stdio::null())
                .output()
                .unwrap(),
        );
        let text = String::from_utf8_lossy(&result.stdout);
        assert!(
            text.contains("Breakpoint 1,")
                && text.contains("src/math.ox:")
                && text.contains("src/main.ox:")
                && text.contains("exited normally"),
            "{result:?}"
        );
        println!("{text}");
    }
    fn output(binary: &Path) {
        let result = success(Command::new(binary).env_clear().output().unwrap());
        assert_eq!(result.stdout, b"42\n");
        assert!(result.stderr.is_empty(), "{result:?}");
    }

    fn debugger_script(project: &Project, commands: &[&str]) -> String {
        let gdb = std::env::var_os("OXID_TEST_GDB").unwrap_or_else(|| "gdb".into());
        let mut command = Command::new(gdb);
        command.current_dir(&project.0).args([
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
        let result = success(
            command
                .arg("--args")
                .arg(project.binary())
                .stdin(Stdio::null())
                .output()
                .unwrap(),
        );
        let text = String::from_utf8(result.stdout).unwrap();
        assert!(text.contains("exited normally"), "{text}");
        println!("{text}");
        text
    }

    #[test]
    #[ignore = "requires Linux x86_64 or ARM64, LLVM 19.1.7 and GDB with ptrace"]
    fn scalar_parameters_mutation_and_nested_scope_are_visible_in_gdb() {
        for policy in ["bounded", "hosted"] {
            let project = Project::new();
            project.write("src/main.ox", "fn inspect(input:i32,flag:bool)->i32{\n    let base=input+2;\n    let mut total=base;\n    total=total+3;\n    if flag{\n        let inside=7;\n        total=total+inside;\n    }\n    total=total+1;\n    return total;\n}\nfn main()->i32{return inspect(29,true);}\n");
            project.manifest(&format!(
                "debug-info=true\nreproducible=true\n[execution]\npolicy=\"{policy}\"\n"
            ));
            success(project.run(&["build"]));
            dwarf(&project.binary(), "--verify");
            let text = debugger_script(
                &project,
                &[
                    "break src/main.ox:7",
                    "run",
                    "echo INNER_START\\n",
                    "info args",
                    "info locals",
                    "echo INNER_END\\n",
                    "break src/main.ox:9",
                    "continue",
                    "echo OUTER_START\\n",
                    "info locals",
                    "echo OUTER_END\\n",
                    "continue",
                ],
            );
            let inner = text
                .split_once("INNER_START\n")
                .unwrap()
                .1
                .split_once("INNER_END\n")
                .unwrap()
                .0;
            for value in ["input = 29", "flag = true", "total = 34", "inside = 7"] {
                assert!(inner.contains(value), "{value}: {text}");
            }
            let outer = text
                .split_once("OUTER_START\n")
                .unwrap()
                .1
                .split_once("OUTER_END\n")
                .unwrap()
                .0;
            assert!(
                outer.contains("total = 41") && !outer.contains("inside ="),
                "{text}"
            );
            output(&project.binary());
        }
    }

    #[test]
    #[ignore = "requires Linux x86_64 or ARM64, LLVM 19.1.7 and GDB with ptrace"]
    fn range_bindings_leave_scope_and_compiler_cursors_are_hidden() {
        for policy in ["bounded", "hosted"] {
            let project = Project::new();
            project.write("src/main.ox", "fn main()->i32{\n    let mut total=0;\n    for n in 0..3{\n        let doubled=n*2;\n        total=total+doubled;\n    }\n    return total;\n}\n");
            project.manifest(&format!(
                "debug-info=true\nreproducible=true\n[execution]\npolicy=\"{policy}\"\n"
            ));
            success(project.run(&["build"]));
            dwarf(&project.binary(), "--verify");
            let text = debugger_script(
                &project,
                &[
                    "break src/main.ox:5",
                    "run",
                    "echo LOOP_START\\n",
                    "info locals",
                    "echo LOOP_END\\n",
                    "disable 1",
                    "break src/main.ox:7",
                    "continue",
                    "echo AFTER_START\\n",
                    "info locals",
                    "echo AFTER_END\\n",
                    "continue",
                ],
            );
            let body = text
                .split_once("LOOP_START\n")
                .unwrap()
                .1
                .split_once("LOOP_END\n")
                .unwrap()
                .0;
            for value in ["n = 0", "doubled = 0", "total = 0"] {
                assert!(body.contains(value), "{text}");
            }
            let after = text
                .split_once("AFTER_START\n")
                .unwrap()
                .1
                .split_once("AFTER_END\n")
                .unwrap()
                .0;
            assert!(
                after.contains("total = 6")
                    && !after.contains("n =")
                    && !after.contains("doubled ="),
                "{text}"
            );
            assert!(
                !body.contains("for n in") && !after.contains("for n in"),
                "{text}"
            );
            let result = success(Command::new(project.binary()).env_clear().output().unwrap());
            assert_eq!(result.stdout, b"6\n");
        }
    }

    #[test]
    #[ignore = "requires Linux x86_64 or ARM64, LLVM 19.1.7 and GDB with ptrace"]
    fn debugger_reads_all_scalar_widths_without_exposing_wider_storage_slots() {
        let source = "fn inspect(number:i64)->i32{\n    let boolean=true;\n    let signed8=-8i8;\n    let unsigned8=250u8;\n    let signed16=-1600i16;\n    let unsigned16=65000u16;\n    let signed32=-320000;\n    let unsigned32=4000000000u32;\n    let signed64=-9223372036854775807i64;\n    let unsigned64=18446744073709551615u64;\n    let pointer_signed=-123isize;\n    let pointer_unsigned=456usize;\n    let signed128=-170141183460469231731687303715884105727i128;\n    let unsigned128=340282366920938463463374607431768211455u128;\n    let float32=1.25f32;\n    let float64=2.5;\n    let character='雪';\n    let mut total=40;\n    total=total+2;\n    return total;\n}\nfn main()->i32{return inspect(17i64);}\n";
        for policy in ["bounded", "hosted"] {
            let project = Project::new();
            project.write("src/main.ox", source);
            project.manifest(&format!(
                "debug-info=true\nreproducible=true\n[execution]\npolicy=\"{policy}\"\n"
            ));
            for profile in ["dev", "release"] {
                success(project.run(&["build", "--profile", profile]));
                dwarf(&project.binary(), "--verify");
                output(&project.binary());
                if profile == "dev" {
                    let text = debugger_script(
                        &project,
                        &[
                            "break src/main.ox:19",
                            "run",
                            "info args",
                            "info locals",
                            "print/x character",
                            "ptype unsigned128",
                            "continue",
                        ],
                    );
                    for value in [
                        "number = 17",
                        "boolean = true",
                        "signed8 = -8",
                        "unsigned8 = 250",
                        "signed16 = -1600",
                        "unsigned16 = 65000",
                        "signed32 = -320000",
                        "unsigned32 = 4000000000",
                        "signed64 = -9223372036854775807",
                        "unsigned64 = 18446744073709551615",
                        "pointer_signed = -123",
                        "pointer_unsigned = 456",
                        "signed128 = -170141183460469231731687303715884105727",
                        "unsigned128 = 340282366920938463463374607431768211455",
                        "float32 = 1.25",
                        "float64 = 2.5",
                        "total = 40",
                        "0x96ea",
                        "type = u128",
                    ] {
                        assert!(text.contains(value), "{value}: {text}");
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "requires Linux x86_64 or ARM64, LLVM 19.1.7 and GDB with ptrace"]
    fn scalar_record_patterns_and_enum_payload_bindings_use_source_names() {
        for policy in ["bounded", "hosted"] {
            let project = Project::new();
            project.write("src/main.ox", "struct Pair{left:i64,right:i32}\nenum Choice{Number(i32),Empty}\nfn inspect(pair:Pair,choice:Choice)->i32{\n    let Pair{left:weight,right:bias}=pair;\n    match choice{\n        Choice::Number(payload)=>{\n            let combined=weight.to_i32_checked()+bias+payload;\n            return combined;\n        },\n        Choice::Empty=>{return 0;}\n    }\n}\nfn main()->i32{return inspect(Pair{left:30i64,right:5},Choice::Number(7));}\n");
            project.manifest(&format!(
                "debug-info=true\nreproducible=true\n[execution]\npolicy=\"{policy}\"\n"
            ));
            success(project.run(&["build"]));
            dwarf(&project.binary(), "--verify");
            let text = debugger_script(
                &project,
                &["break src/main.ox:8", "run", "info locals", "continue"],
            );
            for value in ["weight = 30", "bias = 5", "payload = 7", "combined = 42"] {
                assert!(text.contains(value), "{text}");
            }
            output(&project.binary());
        }
    }

    #[test]
    #[ignore = "requires Linux x86_64 or ARM64, LLVM 19.1.7 and GDB with ptrace"]
    fn source_breakpoints_stacks_cache_and_relocation_use_real_debug_information() {
        // The source-free result and debugger checks cover a multiline LLVM
        // switch, generic payload ownership and the C resource runtime.
        for profile in ["dev", "release"] {
            for policy in ["bounded", "hosted"] {
                let mut project = Project::new();
                project.write("src/main.ox", MAIN);
                project.write("src/math.ox", MATH);
                project.manifest(&format!("profile=\"{profile}\"\ndebug-info=true\nreproducible=true\n[execution]\npolicy=\"{policy}\"\n"));
                success(project.run(&["build", "--incremental"]));
                let expected = fs::read(project.binary()).unwrap();
                dwarf(&project.binary(), "--verify");
                let info = dwarf(&project.binary(), "--debug-info");
                let info = String::from_utf8_lossy(&info.stdout);
                assert!(
                    info.contains("DW_TAG_subprogram")
                        && info.contains("answer")
                        && info.contains("src/math.ox"),
                    "{info}"
                );
                assert!(
                    !info
                        .split_once(".debug_info contents:")
                        .unwrap()
                        .1
                        .contains(&*project.0.to_string_lossy()),
                    "{info}"
                );
                debugger(&project, &project.binary(), "answer");
                success(project.run(&["build", "--incremental"]));
                assert_eq!(fs::read(project.binary()).unwrap(), expected);
                let moved = project.0.with_extension("moved");
                fs::rename(&project.0, &moved).unwrap();
                project.0 = moved;
                fs::remove_dir_all(project.0.join(".oxid")).unwrap();
                success(project.run(&["build"]));
                assert_eq!(fs::read(project.binary()).unwrap(), expected);
                debugger(&project, &project.binary(), "src/math.ox:7");
                let deployed = project.0.join("deployed");
                fs::copy(project.binary(), &deployed).unwrap();
                fs::remove_dir_all(project.0.join("src")).unwrap();
                fs::remove_dir_all(project.0.join(".oxid")).unwrap();
                fs::remove_file(project.0.join("oxid.toml")).unwrap();
                output(&deployed);
            }
        }
        // The historical scalar emitter has a separate instruction writer.
        let mut project = Project::new();
        let quoted = project.0.with_extension("quote-\"-\\");
        fs::rename(&project.0, &quoted).unwrap();
        project.0 = quoted;
        project.write(
            "src/main.ox",
            "mod math;\nuse crate::math::answer;\nfn main()->i32{\n    return answer(40);\n}\n",
        );
        project.write(
            "src/math.ox",
            "pub fn answer(value:i32)->i32{\n    return value+2;\n}\n",
        );
        success(project.run(&["build", "--debug-info", "--incremental"]));
        dwarf(&project.binary(), "--verify");
        debugger(&project, &project.binary(), "answer");
        output(&project.binary());
        success(project.run(&["build", "--no-debug-info", "--incremental"]));
        let info = dwarf(&project.binary(), "--debug-info");
        assert!(!String::from_utf8_lossy(&info.stdout).contains("DW_TAG_compile_unit"));
        output(&project.binary());
        success(project.run(&["build", "--debug-info", "--incremental"]));
        assert!(
            String::from_utf8_lossy(&dwarf(&project.binary(), "--debug-info").stdout)
                .contains("DW_TAG_compile_unit")
        );
        output(&project.binary());

        let workspace = Project::new();
        workspace.write(
            "oxid.toml",
            "[workspace]\nedition=\"typed-preview\"\nmembers=[\"alpha\",\"beta\"]\n",
        );
        for (member, enabled) in [("alpha", true), ("beta", false)] {
            workspace.write(&format!("{member}/oxid.toml"), &format!("[project]\nname=\"{member}\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\ndebug-info={enabled}\n"));
            workspace.write(&format!("{member}/main.ox"), "fn main()->i32{return 42;}\n");
        }
        for (flag, expected) in [
            (None, [true, false]),
            (Some("--debug-info"), [true, true]),
            (Some("--no-debug-info"), [false, false]),
        ] {
            let mut args = vec!["build", "--workspace"];
            if let Some(flag) = flag {
                args.push(flag);
            }
            success(workspace.run(&args));
            for (member, expected) in ["alpha", "beta"].into_iter().zip(expected) {
                let binary = workspace.0.join(format!("{member}/.oxid/bin/{member}"));
                let info = dwarf(&binary, "--debug-info");
                assert_eq!(
                    String::from_utf8_lossy(&info.stdout).contains("DW_TAG_compile_unit"),
                    expected
                );
                output(&binary);
            }
        }
    }
}
