//! Public typed formatter contract, isolated from legacy fixture discovery.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Project(PathBuf);
impl Project {
    fn new(source: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-typed-formatter-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("input.ox"), source).unwrap();
        Self(path)
    }
    fn command(&self, arguments: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .args(arguments)
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env("OXID_LLVM_BIN", self.0.join("native-tools"))
            .env_remove("OXID_PATH");
        command
    }
    fn run(&self, arguments: &[&str]) -> Output {
        self.command(arguments).output().unwrap()
    }
    fn format(&self, check: bool) -> Output {
        let mut args = vec!["fmt", "--edition=typed-preview", "input.ox"];
        if check {
            args.push("--check");
        }
        self.run(&args)
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn assert_error(output: &Output, expected: &str) {
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(expected), "{stderr}");
    assert!(!stderr.contains("formatting required"), "{stderr}");
}

#[test]
fn explicit_formatter_outputs_only_complete_source_without_writing_input() {
    let input = b"fn  main ( )->bool{ return true;}";
    let project = Project::new(input);
    let output = project.format(false);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"fn main() -> bool { return true; }\n");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(fs::read(project.0.join("input.ox")).unwrap(), input);
    assert_eq!(fs::read_dir(&project.0).unwrap().count(), 1);
}

#[test]
fn check_distinguishes_exact_match_drift_and_source_errors() {
    let project = Project::new(b"fn main()->bool{return true;}");
    let drift = project.format(true);
    assert_eq!(drift.status.code(), Some(1), "{drift:?}");
    assert!(drift.stdout.is_empty());
    assert_eq!(drift.stderr, b"typed-preview fmt: formatting required\n");
    let formatted = project.format(false);
    assert_eq!(formatted.status.code(), Some(0), "{formatted:?}");
    fs::write(project.0.join("input.ox"), formatted.stdout).unwrap();
    let equal = project.format(true);
    assert_eq!(equal.status.code(), Some(0), "{equal:?}");
    assert!(equal.stdout.is_empty());
    assert!(equal.stderr.is_empty());
    fs::write(project.0.join("input.ox"), "fn broken(").unwrap();
    assert_error(&project.format(true), "error[");
}

#[test]
fn named_single_file_formats_without_semantic_checks_or_module_discovery() {
    let input = b"mod missing; use crate::missing::Thing; fn f()->Unknown{return absent(999999999999999999999999999999999999999);} fn wrong()->bool{return 1;}";
    let project = Project::new(input);
    // A module lookup would fail on this non-file. The formatter must never try.
    fs::create_dir(project.0.join("missing.ox")).unwrap();
    fs::write(project.0.join("oxid.toml"), "this is not a valid manifest").unwrap();
    let output = project.format(false);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("999999999999999999999999999999999999999")
    );
    assert_eq!(fs::read(project.0.join("input.ox")).unwrap(), input);
    assert_eq!(fs::read_dir(&project.0).unwrap().count(), 3);
}

#[test]
fn cli_flag_positions_and_explicit_text_match() {
    let project = Project::new(b"fn main()->bool{return true;}");
    let expected = b"fn main() -> bool { return true; }\n";
    for args in [
        &["--edition=typed-preview", "fmt", "input.ox"][..],
        &["fmt", "--edition", "typed-preview", "input.ox"],
        &[
            "fmt",
            "input.ox",
            "--edition=typed-preview",
            "--message-format=text",
        ],
        &[
            "--message-format",
            "text",
            "fmt",
            "input.ox",
            "--edition=typed-preview",
        ],
    ] {
        let output = project.run(args);
        assert_eq!(output.status.code(), Some(0), "{args:?}: {output:?}");
        assert_eq!(output.stdout, expected);
        assert!(output.stderr.is_empty());
    }
    for args in [
        &["--edition=typed-preview", "fmt", "--check", "input.ox"][..],
        &["fmt", "input.ox", "--edition=typed-preview", "--check"],
    ] {
        let output = project.run(args);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"typed-preview fmt: formatting required\n");
    }
}

#[test]
fn selected_cli_errors_are_text_code_two_and_do_not_read_source() {
    let project = Project::new(b"not valid syntax");
    for args in [
        &[][..],
        &["input.ox", "second.ox"],
        &["--check", "--check", "input.ox"],
        &["--check=true", "input.ox"],
        &["--write", "input.ox"],
        &["--output", "result.ox", "input.ox"],
        &["--width=80", "input.ox"],
        &["--message-format=json", "input.ox"],
        &["-"],
        &["--", "-"],
    ] {
        let mut full = vec!["fmt", "--edition=typed-preview"];
        full.extend_from_slice(args);
        let output = project.run(&full);
        assert_error(&output, "error[E0001] (cli)");
        assert!(!project.0.join("result.ox").exists());
    }
}

#[test]
fn preselection_global_errors_keep_existing_json_contract() {
    let project = Project::new(b"");
    let output = project.run(&["fmt", "input.ox", "--edition=nope", "--message-format=json"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"kind\":\"diagnostic\""));
    assert!(stdout.contains("\"kind\":\"check-summary\""));
    let output = project.run(&["--check", "fmt", "input.ox", "--edition=typed-preview"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("command `--check` is unavailable"));
}

#[test]
fn separator_keeps_dash_names_literal_but_dash_alone_is_never_stdin() {
    let project = Project::new(b"");
    for name in ["--check", "--flag-named.ox", "-"] {
        fs::write(project.0.join(name), "// named").unwrap();
        let selected = if name == "-" { "./-" } else { name };
        let output = project.run(&["fmt", "--edition=typed-preview", "--", selected]);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert_eq!(output.stdout, b"// named\n");
        assert!(output.stderr.is_empty());
    }
    assert_error(
        &project.run(&["fmt", "--edition=typed-preview", "--", "-"]),
        "stdin",
    );
}

#[test]
fn empty_whitespace_and_comment_only_sources_use_exact_output_contract() {
    for (input, expected) in [
        (&b""[..], &b""[..]),
        (&b" \t\r\n\n"[..], &b""[..]),
        (
            &b" // a\r\n\n /* b\n  c */ \n"[..],
            &b"// a\r\n\n/* b\n  c */\n"[..],
        ),
    ] {
        let project = Project::new(input);
        let output = project.format(false);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert_eq!(output.stdout, expected);
        assert!(output.stderr.is_empty());
        assert_eq!(fs::read(project.0.join("input.ox")).unwrap(), input);
    }
}

#[test]
fn malformed_encoding_syntax_and_oversized_input_never_emit_a_prefix() {
    for input in [
        &b"\xff"[..],
        &b"fn f()->bool{return true;} fn broken("[..],
        &b"fn f()->bool{return \"unsupported\";}"[..],
        &b"use crate: :thing;"[..],
    ] {
        let project = Project::new(input);
        for check in [false, true] {
            assert_error(&project.format(check), "error[");
        }
        assert_eq!(fs::read(project.0.join("input.ox")).unwrap(), input);
    }
    let project = Project::new(&vec![b' '; 1_048_577]);
    assert_error(&project.format(false), "E0400");
    assert_error(&project.format(true), "E0400");
}

#[test]
fn missing_sources_and_directories_are_read_errors() {
    let project = Project::new(b"");
    for path in ["missing.ox", "."] {
        assert_error(
            &project.run(&["fmt", "--edition=typed-preview", path]),
            "error[E0002] (read)",
        );
    }
}

#[test]
fn default_and_explicit_legacy_formatter_behavior_match() {
    let original = b"fn main() {\nreturn true;\n}\n";
    let default = Project::new(original);
    let explicit = Project::new(original);
    let a = default.run(&["fmt", "input.ox"]);
    let b = explicit.run(&["fmt", "input.ox", "--edition=legacy-0.9"]);
    assert_eq!(a.status.code(), b.status.code());
    assert_eq!(a.stdout, b.stdout);
    assert_eq!(a.stderr, b.stderr);
    assert_eq!(
        fs::read(default.0.join("input.ox")).unwrap(),
        fs::read(explicit.0.join("input.ox")).unwrap()
    );
}

#[cfg(unix)]
#[test]
fn regular_symlinks_are_accepted_and_nonregular_files_rejected_without_blocking() {
    use std::os::unix::fs::symlink;
    use std::time::{Duration, Instant};
    let project = Project::new(b"// linked");
    symlink("input.ox", project.0.join("linked.ox")).unwrap();
    let output = project.run(&["fmt", "--edition=typed-preview", "linked.ox"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"// linked\n");
    assert!(Command::new("mkfifo")
        .arg(project.0.join("pipe.ox"))
        .status()
        .unwrap()
        .success());
    let mut child = project
        .command(&["fmt", "--edition=typed-preview", "pipe.ox"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("formatter blocked opening a FIFO");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_error(&child.wait_with_output().unwrap(), "error[E0002] (read)");
    assert_error(
        &project.run(&["fmt", "--edition=typed-preview", "/dev/null"]),
        "regular source file",
    );
}

#[cfg(unix)]
#[test]
fn source_and_diagnostic_control_bytes_are_preserved_and_escaped_respectively() {
    let project = Project::new(b"// exact\t\x1b\r\n");
    let output = project.format(false);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"// exact\t\x1b\r\n");
    let filename = "bad\n\x1b.ox";
    fs::write(project.0.join(filename), "fn broken(").unwrap();
    let output = project.run(&["fmt", "--edition=typed-preview", filename]);
    assert_error(&output, "bad\\n\\u{1b}.ox");
    assert!(!output.stderr.contains(&0x1b));
}

#[cfg(unix)]
#[test]
fn native_tool_and_module_fifo_traps_remain_untouched() {
    use std::os::unix::fs::PermissionsExt;
    let project = Project::new(b"mod trap; fn main()->bool{return main();}");
    let tools = project.0.join("native-tools");
    fs::create_dir(&tools).unwrap();
    for name in ["clang", "llc", "opt", "ld.lld", "cc"] {
        let path = tools.join(name);
        fs::write(&path, "#!/bin/sh\nprintf invoked > tool-invoked\nexit 99\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert!(Command::new("mkfifo")
        .arg(project.0.join("trap.ox"))
        .status()
        .unwrap()
        .success());
    // Keep a writer open so an accidental module read cannot indefinitely block.
    let fifo = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(project.0.join("trap.ox"))
        .unwrap();
    let mut output = project
        .command(&["fmt", "--edition=typed-preview", "input.ox"])
        .env("PATH", &tools)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if output.try_wait().unwrap().is_some() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            output.kill().unwrap();
            let _ = output.wait();
            panic!("formatter touched the module FIFO");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    drop(fifo);
    let output = output.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        output.stdout,
        b"mod trap; fn main() -> bool { return main(); }\n"
    );
    assert!(output.stderr.is_empty());
    assert!(!project.0.join("tool-invoked").exists());
    assert!(!project.0.join("cache").exists());
    assert_eq!(fs::read_dir(&project.0).unwrap().count(), 3);
}

#[test]
fn arrays_format_without_typechecking_loading_or_native_tools() {
    let input = b"mod missing;fn f(a:&mut [i32;2])->[bool;0]{a[0]=1;let empty=[];sink(&mut *a,[true,1]);return a.len();}";
    let expected = b"mod missing; fn f(a: &mut [i32; 2]) -> [bool; 0] { a[0] = 1; let empty = []; sink(&mut *a, [true, 1]); return a.len(); }\n";
    let project = Project::new(input);
    fs::create_dir(project.0.join("missing.ox")).unwrap();
    fs::write(project.0.join("oxid.toml"), "invalid manifest").unwrap();
    let output = project.format(false);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read(project.0.join("input.ox")).unwrap(), input);
    assert_eq!(fs::read_dir(&project.0).unwrap().count(), 3);
    fs::write(project.0.join("input.ox"), expected).unwrap();
    let checked = project.format(true);
    assert_eq!(checked.status.code(), Some(0), "{checked:?}");
    assert!(checked.stdout.is_empty());
    assert!(checked.stderr.is_empty());
    fs::write(
        project.0.join("input.ox"),
        "fn broken()->(){let a=[1 2];}fn intact()->(){return;}",
    )
    .unwrap();
    assert_error(&project.format(false), "error[E0100] (parse)");
    assert_error(&project.format(true), "error[E0100] (parse)");
    assert_eq!(fs::read_dir(&project.0).unwrap().count(), 3);
}
