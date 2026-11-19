//! Unicode scalar values through ordinary commands and deployed programs.
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
            "oxid-characters-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), format!("[project]\nname=\"characters\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
        Self(root)
    }
    fn invoke(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
    fn text(&self, expected: &str) {
        let result = self.invoke(&["run"]);
        assert_eq!(
            (result.status.code(), result.stdout, result.stderr),
            (Some(0), format!("{expected}\n").into_bytes(), Vec::new())
        );
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn literals_preserve_unicode_and_json_escapes() {
    for policy in ["bounded", "hosted"] {
        for (literal, text, json) in [
            ("'a'", "a", "\"a\""),
            ("'中'", "中", "\"中\""),
            ("'🦀'", "🦀", "\"🦀\""),
            ("'\"'", "\"", "\"\\\"\""),
            (r"'\''", "'", "\"'\""),
            (r"'\\'", "\\", "\"\\\\\""),
            (r"'\n'", "\n", "\"\\n\""),
            (r"'\r'", "\r", "\"\\r\""),
            (r"'\t'", "\t", "\"\\t\""),
            (r"'\0'", "\0", "\"\\u0000\""),
            (r"'\x7f'", "\u{7f}", "\"\u{7f}\""),
            (r"'\u{10ffff}'", "\u{10ffff}", "\"\u{10ffff}\""),
        ] {
            let project = Project::new(&format!("fn main()->char{{return {literal};}}"), policy);
            project.text(text);
            let result = project.invoke(&["run", "--message-format=json"]);
            assert_eq!(result.status.code(), Some(0), "{result:?}");
            assert!(result.stderr.is_empty());
            assert_eq!(String::from_utf8(result.stdout).unwrap(), format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":true,\"errors\":0,\"result\":{{\"type\":\"char\",\"value\":{json}}}}}\n"));
        }
    }
}

#[test]
fn characters_survive_aggregates_borrows_and_function_calls() {
    for policy in ["bounded", "hosted"] {
        Project::new("struct Text{chars:[char;2]}fn first(text:&Text)->char{return text.chars[0];}fn main()->char{let mut text=Text{chars:['a','中']};text.chars[0]='🦀';return first(&text);}", policy).text("🦀");
        Project::new("enum Scalar{Value(char),Empty}fn get(value:Scalar)->char{match value{Scalar::Value(c)=>{return c;},Scalar::Empty=>{return '\\0';}}}fn main()->char{return get(Scalar::Value('中'));}", policy).text("中");
        Project::new("fn first(values:&[char])->char{return values[0];}fn main()->char{let values=['🦀'];return first(&values);}", policy).text("🦀");
        Project::new(
            "fn main()->bool{return 'a'<'中'&&'中'<'🦀'&&'🦀'=='\\u{1f980}'&&'a'!='b';}",
            policy,
        )
        .text("true");
        Project::new("fn main()->char{let n=340282366920938463463374607431768211455u128;let f=0.5;let c='中';if n>1u128&&f<1.0{return c;}return 'a';}", policy).text("中");
    }
}

#[test]
fn conversions_preserve_the_complete_integer_and_scalar() {
    for policy in ["bounded", "hosted"] {
        for ty in [
            "i8", "u8", "i16", "u16", "i32", "u32", "i64", "u64", "isize", "usize", "i128", "u128",
        ] {
            let binding = if ty == "u8" {
                "let base=65;let n=base.to_u8_checked();".to_owned()
            } else if ty == "i32" {
                "let n=65;".to_owned()
            } else {
                format!("let n=65{ty};")
            };
            Project::new(
                &format!("fn main()->char{{{binding}return n.to_char_checked();}}"),
                policy,
            )
            .text("A");
        }
        Project::new("fn main()->u32{let c='🦀';return c.to_u32();}", policy).text("129408");
        for literal in [
            "-1i8",
            "-1i16",
            "-1",
            "-1i64",
            "-1isize",
            "-1i128",
            "55296u16",
            "57343u32",
            "1114112u64",
            "4294967361u128",
            "340282366920938463463374607431768211455u128",
        ] {
            let project = Project::new(
                &format!("fn main()->char{{let n={literal};return n.to_char_checked();}}"),
                policy,
            );
            assert!(project.invoke(&["check"]).status.success());
            let result = project.invoke(&["run"]);
            assert_eq!(result.status.code(), Some(1));
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8(result.stderr)
                .unwrap()
                .contains("error[E0612]"));
        }
    }
}

#[test]
fn malformed_literals_and_invalid_operations_are_rejected_before_execution() {
    for (body, code) in [
        ("return '';", "E0101"),
        ("return 'ab';", "E0101"),
        ("return 'e\u{301}';", "E0101"),
        ("return '\n';", "E0101"),
        (r"return '\x80';", "E0101"),
        (r"return '\u{}';", "E0101"),
        (r"return '\u{d800}';", "E0101"),
        (r"return '\u{110000}';", "E0101"),
        (r"return '\q';", "E0101"),
        ("return 'a'+'b';", "E0300"),
        ("return -'a';", "E0300"),
        ("return ~'a';", "E0300"),
        ("return 'a'&'b';", "E0300"),
        ("let n=1.0;return n.to_char_checked();", "E0300"),
        ("let n=true;return n.to_char_checked();", "E0300"),
        ("let n=65;return n.to_u32();", "E0300"),
        ("let c='a';return c.to_f64();", "E0300"),
    ] {
        let project = Project::new(&format!("fn main()->char{{{body}}}"), "bounded");
        let result = project.invoke(&["check", "--message-format=json"]);
        assert_eq!(result.status.code(), Some(1), "{body}: {result:?}");
        assert!(result.stderr.is_empty());
        assert!(
            String::from_utf8(result.stdout)
                .unwrap()
                .contains(&format!("\"code\":\"{code}\"")),
            "{body}"
        );
        assert!(!project.0.join(".oxid").exists());
    }
    let project = Project::new("struct char{x:i32}fn main()->i32{return 0;}", "bounded");
    let result = project.invoke(&["check", "--message-format=json"]);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8(result.stdout)
        .unwrap()
        .contains("\"code\":\"E0208\""));
    Project::new(
        "fn char()->i32{return 7;}fn main()->i32{let value=char();return value;}",
        "bounded",
    )
    .text("7");
}

#[test]
fn formatting_preserves_quotes_escapes_and_character_conversions() {
    let project = Project::new(
        "fn main()->char{let quote='\"';let newline='\\n';let n=65u32;return n.to_char_checked();}",
        "bounded",
    );
    let formatted = project.invoke(&["fmt", "src/main.ox"]);
    assert!(formatted.status.success(), "{formatted:?}");
    fs::write(project.0.join("src/main.ox"), &formatted.stdout).unwrap();
    let first = formatted.stdout;
    let check = project.invoke(&["fmt", "--check", "src/main.ox"]);
    assert!(check.status.success(), "{check:?}");
    assert_eq!(first, fs::read(project.0.join("src/main.ox")).unwrap());
    project.text("A");
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]
fn native_characters_preserve_utf8_without_sources_and_report_closed_output() {
    use std::{os::fd::OwnedFd, os::unix::net::UnixStream, process::Stdio};
    for policy in ["bounded", "hosted"] {
        for literal in ["'a'", "'中'", "'🦀'", r"'\0'", r"'\u{10ffff}'"] {
            let project = Project::new(&format!("fn main()->char{{return {literal};}}"), policy);
            let expected = project.invoke(&["run"]);
            assert!(expected.status.success());
            let deployment = project.0.join("deployment");
            fs::create_dir(&deployment).unwrap();
            for profile in ["dev", "release"] {
                for _ in 0..2 {
                    let built = project.invoke(&["build", "--profile", profile]);
                    assert!(built.status.success(), "{built:?}");
                }
                fs::copy(
                    project.0.join(".oxid/bin/characters"),
                    deployment.join(profile),
                )
                .unwrap();
            }
            fs::remove_dir_all(project.0.join("src")).unwrap();
            fs::remove_dir_all(project.0.join(".oxid")).unwrap();
            fs::remove_file(project.0.join("oxid.toml")).unwrap();
            for profile in ["dev", "release"] {
                let output = Command::new(deployment.join(profile))
                    .current_dir(&deployment)
                    .env_clear()
                    .output()
                    .unwrap();
                assert_eq!(
                    (output.status.code(), output.stdout, output.stderr),
                    (Some(0), expected.stdout.clone(), Vec::new())
                );
                let (writer, reader) = UnixStream::pair().unwrap();
                drop(reader);
                let closed = Command::new(deployment.join(profile))
                    .current_dir(&deployment)
                    .env_clear()
                    .stdout(Stdio::from(OwnedFd::from(writer)))
                    .output()
                    .unwrap();
                assert_eq!(closed.status.code(), Some(74));
                assert!(closed.stderr.is_empty());
            }
        }
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
fn native_checked_conversions_match_runtime_errors_and_aggregate_values() {
    for policy in ["bounded", "hosted"] {
        for source in [
            "fn main()->char{let n=-1i8;return n.to_char_checked();}",
            "fn main()->char{let n=4294967361u128;return n.to_char_checked();}",
            "fn main()->char{let n=55296u16;return n.to_char_checked();}",
            "enum E{C(char)}fn get(e:E)->char{match e{E::C(c)=>{return c;}}}fn main()->char{let n=1114111u128;let c=n.to_char_checked();return get(E::C(c));}",
            "struct R{a:[char;2]}fn first(r:&R)->char{return r.a[0];}fn main()->char{let mut r=R{a:['a','b']};r.a[0]='🦀';return first(&r);}",
        ] {
            let project=Project::new(source,policy);let expected=project.invoke(&["run"]);
            let deployment=project.0.join("deployment");fs::create_dir(&deployment).unwrap();
            for profile in ["dev","release"] { let built=project.invoke(&["build","--profile",profile]);assert!(built.status.success(),"{built:?}");fs::copy(project.0.join(".oxid/bin/characters"),deployment.join(profile)).unwrap(); }
            fs::remove_dir_all(project.0.join("src")).unwrap();fs::remove_dir_all(project.0.join(".oxid")).unwrap();fs::remove_file(project.0.join("oxid.toml")).unwrap();
            for profile in ["dev","release"] { let result=Command::new(deployment.join(profile)).current_dir(&deployment).env_clear().output().unwrap();assert_eq!((result.status.code(),result.stdout,result.stderr),(expected.status.code(),expected.stdout.clone(),expected.stderr.clone())); }
        }
    }
}
