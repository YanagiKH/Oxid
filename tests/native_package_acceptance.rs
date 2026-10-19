//! Independent native PACKAGE acceptance (separate from the author's
//! `native_package_core`). Archives are checked with a ustar reader and a JSON
//! reader written here, then extracted by the system `tar` and executed.
//! Tests marked `#[ignore]` need LLVM 19.1.7: run them with
//! `OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --test
//! native_package_acceptance -- --include-ignored`. Opting in without that
//! variable fails the LLVM gate; a missing tool is never a pass. Synthetic ELF
//! bytes are never used. No qualification, capacity or fuel probes.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{symlink, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const APP: &str = "npkg-app";
const LIB: &str = "npkg-library";
const PROC: &str = "npkg-proc";
const EXAMPLE: [(&str, &str); 7] = [
    ("oxid.toml", "[workspace]\nmembers = [\"app\", \"library\", \"proc\"]\n"),
    ("app/oxid.toml", "[project]\nname = \"npkg-app\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n\n[dependencies]\nlib = \"../library\"\n"),
    ("app/src/main.ox", "use lib::scale;\nuse lib::model::make_pair;\nuse lib::model::pair_sum;\nfn main() -> i32 {\n    let p = make_pair();\n    return scale(pair_sum(&p));\n}\n"),
    ("library/oxid.toml", "[project]\nname = \"npkg-library\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/lib.ox\"\n"),
    ("library/src/lib.ox", "pub mod model;\nfn factor() -> i32 {\n    return 3;\n}\npub fn scale(x: i32) -> i32 {\n    return x * crate::factor();\n}\nfn main() -> i32 {\n    return 55;\n}\n"),
    ("library/src/model.ox", "pub struct Pair {\n    a: i32,\n    b: i32\n}\npub fn make_pair() -> Pair {\n    return Pair { a: 4, b: 7 };\n}\npub fn pair_sum(p: &Pair) -> i32 {\n    return p.a + p.b;\n}\n"),
    ("proc/oxid.toml", "[project]\nname = \"npkg-proc\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n\n[dependencies]\nlib = \"../library\"\n"),
];
const PROC_MAIN: &str = "use lib::scale;\nuse std::io::write_stdout;\nfn main() -> i32 {\n    let bytes = [80, 75, 71, 10];\n    let written = write_stdout(&bytes);\n    return scale(1);\n}\n";

// ---------------------------------------------------------------- JSON reader
#[derive(Clone, Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}
impl Json {
    fn parse(text: &str) -> Json {
        let mut p = Parser(text.as_bytes(), 0);
        let value = p.value();
        p.ws();
        assert_eq!(p.1, p.0.len(), "trailing JSON bytes in {text}");
        value
    }
    fn get(&self, key: &str) -> &Json {
        match self {
            Json::Obj(fields) => {
                let found: Vec<_> = fields.iter().filter(|(k, _)| k == key).collect();
                assert_eq!(found.len(), 1, "key {key} in {self:?}");
                &found[0].1
            }
            _ => panic!("not an object: {self:?}"),
        }
    }
    fn keys(&self) -> Vec<&str> {
        match self {
            Json::Obj(fields) => fields.iter().map(|(k, _)| k.as_str()).collect(),
            _ => panic!("not an object: {self:?}"),
        }
    }
    fn str(&self) -> &str {
        match self {
            Json::Str(s) => s,
            _ => panic!("not a string: {self:?}"),
        }
    }
}
struct Parser<'a>(&'a [u8], usize);
impl Parser<'_> {
    fn ws(&mut self) {
        while self.1 < self.0.len() && b" \t\r\n".contains(&self.0[self.1]) {
            self.1 += 1;
        }
    }
    fn eat(&mut self, b: u8) {
        self.ws();
        assert_eq!(self.0.get(self.1), Some(&b), "expected {}", b as char);
        self.1 += 1;
    }
    fn lit(&mut self, word: &str, value: Json) -> Json {
        assert!(self.0[self.1..].starts_with(word.as_bytes()));
        self.1 += word.len();
        value
    }
    fn string(&mut self) -> String {
        self.eat(b'"');
        let mut out = String::new();
        loop {
            let c = self.0[self.1];
            self.1 += 1;
            match c {
                b'"' => return out,
                b'\\' => {
                    let e = self.0[self.1];
                    self.1 += 1;
                    match e {
                        b'"' | b'\\' | b'/' => out.push(e as char),
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'r' => out.push('\r'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'u' => {
                            let hex = std::str::from_utf8(&self.0[self.1..self.1 + 4]).unwrap();
                            self.1 += 4;
                            out.push(
                                char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap(),
                            );
                        }
                        _ => panic!("bad escape"),
                    }
                }
                c if c < 0x20 => panic!("raw control byte in JSON string"),
                _ => {
                    let start = self.1 - 1;
                    let mut end = self.1;
                    while end < self.0.len() && self.0[end] & 0xC0 == 0x80 {
                        end += 1;
                    }
                    out.push_str(std::str::from_utf8(&self.0[start..end]).unwrap());
                    self.1 = end;
                }
            }
        }
    }
    fn value(&mut self) -> Json {
        self.ws();
        match self.0[self.1] {
            b'n' => self.lit("null", Json::Null),
            b't' => self.lit("true", Json::Bool(true)),
            b'f' => self.lit("false", Json::Bool(false)),
            b'"' => Json::Str(self.string()),
            b'[' => {
                self.1 += 1;
                let mut items = Vec::new();
                self.ws();
                if self.0[self.1] == b']' {
                    self.1 += 1;
                    return Json::Arr(items);
                }
                loop {
                    items.push(self.value());
                    self.ws();
                    self.1 += 1;
                    match self.0[self.1 - 1] {
                        b',' => continue,
                        b']' => return Json::Arr(items),
                        _ => panic!("bad array"),
                    }
                }
            }
            b'{' => {
                self.1 += 1;
                let mut fields = Vec::new();
                self.ws();
                if self.0[self.1] == b'}' {
                    self.1 += 1;
                    return Json::Obj(fields);
                }
                loop {
                    let key = self.string();
                    self.eat(b':');
                    fields.push((key, self.value()));
                    self.ws();
                    self.1 += 1;
                    match self.0[self.1 - 1] {
                        b',' => continue,
                        b'}' => return Json::Obj(fields),
                        _ => panic!("bad object"),
                    }
                }
            }
            _ => {
                let start = self.1;
                while self.1 < self.0.len() && b"-+.eE0123456789".contains(&self.0[self.1]) {
                    self.1 += 1;
                }
                Json::Num(
                    std::str::from_utf8(&self.0[start..self.1])
                        .unwrap()
                        .parse()
                        .unwrap(),
                )
            }
        }
    }
}

// ---------------------------------------------------------------- ustar reader
#[derive(Debug)]
struct Member {
    name: String,
    mode: u64,
    uid: u64,
    gid: u64,
    mtime: u64,
    data: Vec<u8>,
}
fn octal_field(field: &[u8]) -> u64 {
    let end = field
        .iter()
        .position(|b| *b == 0 || *b == b' ')
        .unwrap_or(field.len());
    assert!(end > 0, "empty numeric field {field:?}");
    let text = std::str::from_utf8(&field[..end]).unwrap();
    assert!(text.bytes().all(|b| (b'0'..=b'7').contains(&b)), "{text}");
    u64::from_str_radix(text, 8).unwrap()
}
fn cstr(field: &[u8]) -> String {
    let end = field.iter().position(|b| *b == 0).unwrap_or(field.len());
    assert!(
        field[end..].iter().all(|b| *b == 0),
        "garbage after NUL: {field:?}"
    );
    String::from_utf8(field[..end].to_vec()).unwrap()
}
/// Strict reader: regular ustar members, valid checksums, zero padding, and
/// exactly two zero end blocks with nothing after them.
fn read_tar(bytes: &[u8]) -> Vec<Member> {
    assert_eq!(bytes.len() % 512, 0, "archive is not block aligned");
    let mut members = Vec::new();
    let mut at = 0;
    loop {
        let header = &bytes[at..at + 512];
        if header.iter().all(|b| *b == 0) {
            assert_eq!(
                bytes.len(),
                at + 1024,
                "end marker must be the final two blocks"
            );
            assert!(bytes[at..].iter().all(|b| *b == 0));
            return members;
        }
        let mut sum: u64 = header.iter().map(|b| u64::from(*b)).sum();
        sum -= header[148..156].iter().map(|b| u64::from(*b)).sum::<u64>();
        sum += 8 * u64::from(b' ');
        assert_eq!(octal_field(&header[148..156]), sum, "checksum");
        assert_eq!(&header[257..263], b"ustar\0");
        assert_eq!(&header[263..265], b"00");
        assert_eq!(header[156], b'0', "regular file typeflag");
        assert_eq!(cstr(&header[157..257]), "", "linkname");
        assert_eq!(cstr(&header[265..297]), "", "uname");
        assert_eq!(cstr(&header[297..329]), "", "gname");
        assert_eq!(cstr(&header[345..500]), "", "prefix");
        let size = octal_field(&header[124..136]) as usize;
        let data = bytes[at + 512..at + 512 + size].to_vec();
        let padded = size.div_ceil(512) * 512;
        assert!(bytes[at + 512 + size..at + 512 + padded]
            .iter()
            .all(|b| *b == 0));
        members.push(Member {
            name: cstr(&header[0..100]),
            mode: octal_field(&header[100..108]),
            uid: octal_field(&header[108..116]),
            gid: octal_field(&header[116..124]),
            mtime: octal_field(&header[136..148]),
            data,
        });
        at += 512 + padded;
    }
}

// ---------------------------------------------------------------- fixture
fn llvm_bin() -> PathBuf {
    let dir = PathBuf::from(
        std::env::var_os("OXID_TEST_LLVM_BIN")
            .expect("native package acceptance requires OXID_TEST_LLVM_BIN (LLVM 19.1.7 bin)"),
    );
    for (tool, marker) in [
        ("clang", "clang version 19.1.7"),
        ("opt", "LLVM version 19.1.7"),
        ("ld.lld", "LLD 19.1.7"),
    ] {
        let version = Command::new(dir.join(tool))
            .arg("--version")
            .output()
            .unwrap_or_else(|e| panic!("missing native tool {tool}: {e}"));
        assert!(
            String::from_utf8_lossy(&version.stdout).contains(marker),
            "{tool}: {version:?}"
        );
    }
    dir
}

struct Fixture {
    root: PathBuf,
    llvm: PathBuf,
}
impl Fixture {
    fn new(llvm: Option<PathBuf>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-npkg-accept-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self {
            llvm: llvm.unwrap_or_else(|| root.join("absent-llvm")),
            root,
        };
        let example = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/projects/native_package_acceptance");
        let mut files: Vec<(&str, &str)> = EXAMPLE.to_vec();
        files.push(("proc/src/main.ox", PROC_MAIN));
        for (file, expected) in files {
            let text = fs::read_to_string(example.join(file)).unwrap();
            assert_eq!(text, expected, "example drifted: {file}");
            fixture.write(&format!("ws/{file}"), &text);
        }
        fs::create_dir_all(fixture.root.join("caller/deeper")).unwrap();
        fixture
    }
    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
    fn write(&self, rel: &str, text: &str) {
        let path = self.path(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn command(&self, cwd: &str, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .args(args)
            .current_dir(self.path(cwd))
            .env_remove("OXID_PATH")
            .env("OXID_LLVM_BIN", &self.llvm)
            .env("OXID_CACHE_DIR", self.path("cache"))
            .stdin(Stdio::null());
        command
    }
    fn oxid(&self, cwd: &str, args: &[&str]) -> Output {
        self.command(cwd, args).output().unwrap()
    }
    /// JSON package invocation from `cwd`, selecting `member` from the root.
    fn package(&self, cwd: &str, manifest: &str, member: &str, mode: &str, out: &str) -> Output {
        self.oxid(
            cwd,
            &[
                "package",
                "--manifest-path",
                manifest,
                "--package",
                member,
                "--entry-mode",
                mode,
                "--output",
                out,
                "--message-format",
                "json",
            ],
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::set_permissions(self.path("caller/ro"), fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Exactly one package-summary, last, and no intermediate build/check summary.
fn records(stdout: &[u8]) -> (Vec<Json>, Json) {
    let text = String::from_utf8(stdout.to_vec()).unwrap();
    let mut all: Vec<Json> = text.lines().map(Json::parse).collect();
    let kinds: Vec<String> = all
        .iter()
        .map(|r| r.get("kind").str().to_string())
        .collect();
    assert_eq!(
        kinds.iter().filter(|k| *k == "package-summary").count(),
        1,
        "{text}"
    );
    assert_eq!(
        kinds.last().map(String::as_str),
        Some("package-summary"),
        "{text}"
    );
    assert!(
        kinds[..kinds.len() - 1].iter().all(|k| k == "diagnostic"),
        "{text}"
    );
    let summary = all.pop().unwrap();
    assert_eq!(
        summary.keys(),
        [
            "schema_version",
            "edition",
            "kind",
            "success",
            "errors",
            "entry_mode",
            "managed_published",
            "managed_output",
            "archive_published",
            "archive_output"
        ]
    );
    assert_eq!(summary.get("errors"), &Json::Num(all.len() as f64));
    assert_eq!(summary.get("success"), &Json::Bool(all.is_empty()));
    (all, summary)
}
fn ok_summary(output: &Output, mode: &str) -> Json {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let (diagnostics, summary) = records(&output.stdout);
    assert!(diagnostics.is_empty());
    assert_eq!(summary.get("entry_mode").str(), mode);
    assert_eq!(summary.get("managed_published"), &Json::Bool(true));
    assert_eq!(summary.get("archive_published"), &Json::Bool(true));
    summary
}
fn failed(output: &Output, code: &str, text: &str) -> Json {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let (diagnostics, summary) = records(&output.stdout);
    assert!(
        diagnostics
            .iter()
            .any(|d| d.get("code").str() == code && d.get("message").str().contains(text)),
        "{code} {text}: {diagnostics:?}"
    );
    let all = String::from_utf8_lossy(&output.stdout).to_lowercase();
    assert!(
        !all.contains("rollback") && !all.contains("rolled back"),
        "{all}"
    );
    summary
}
fn unpublished(summary: &Json) {
    assert_eq!(summary.get("managed_published"), &Json::Bool(false));
    assert_eq!(summary.get("managed_output"), &Json::Null);
    assert_eq!(summary.get("archive_published"), &Json::Bool(false));
    assert_eq!(summary.get("archive_output"), &Json::Null);
}

/// Every entry below `dir` (no symlink following): kind, inode and content.
fn tree(dir: &Path) -> BTreeMap<String, (char, u64, Vec<u8>)> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, (char, u64, Vec<u8>)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            let rel = path.strip_prefix(base).unwrap().display().to_string();
            if meta.file_type().is_symlink() {
                let target = fs::read_link(&path).unwrap();
                out.insert(
                    rel,
                    (
                        'l',
                        meta.ino(),
                        target.as_os_str().as_encoded_bytes().to_vec(),
                    ),
                );
            } else if meta.is_dir() {
                out.insert(rel, ('d', meta.ino(), Vec::new()));
                walk(base, &path, out);
            } else {
                out.insert(rel, ('f', meta.ino(), fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// `tree` with `current`/`next` receipt names folded away: a later build may
/// normalize a committed `next` into `current` before it fails, so failures are
/// judged by inodes and bytes, not by which receipt slot holds them.
type Tree = BTreeMap<String, (char, u64, Vec<u8>)>;
fn tree_slots_folded(dir: &Path) -> (Tree, Vec<(String, u64, Vec<u8>)>) {
    let mut receipts = Vec::new();
    let mut rest = Tree::new();
    for (path, entry) in tree(dir) {
        match path.rsplit_once('/') {
            Some((parent, "current" | "next")) if parent.ends_with(".build") => {
                receipts.push((parent.to_string(), entry.1, entry.2))
            }
            _ => {
                rest.insert(path, entry);
            }
        }
    }
    receipts.sort();
    (rest, receipts)
}

/// Ownership invariant without assuming slot names: the managed output is a
/// regular file hard-linked to one of the `current`/`next` receipts. A first
/// publication is not assumed to create `current`.
fn owned(namespace: &Path, name: &str) -> (u64, Vec<u8>) {
    let output = namespace.join(name);
    let meta = fs::symlink_metadata(&output).unwrap();
    assert!(meta.file_type().is_file(), "{output:?}");
    assert_eq!(
        meta.permissions().mode() & 0o111,
        0o111,
        "managed executable bit"
    );
    let state = namespace.join(format!(".{name}.build"));
    let receipts: Vec<u64> = ["current", "next"]
        .iter()
        .filter_map(|r| fs::symlink_metadata(state.join(r)).ok())
        .map(|m| {
            assert!(m.file_type().is_file());
            m.ino()
        })
        .collect();
    assert!(receipts.contains(&meta.ino()), "unowned output {output:?}");
    assert!(state.join("owner").is_file());
    assert!(!state.join("stage").exists());
    (meta.ino(), fs::read(output).unwrap())
}

/// The single receipt slot holding the old inode after failures (normalization
/// may already have moved `next` to `current`).
fn receipt_slot(namespace: &Path, name: &str) -> &'static str {
    let state = namespace.join(format!(".{name}.build"));
    let present: Vec<&'static str> = ["current", "next"]
        .into_iter()
        .filter(|r| state.join(r).exists())
        .collect();
    assert_eq!(present.len(), 1, "{present:?}");
    present[0]
}

/// Read the archive independently, compare with the managed executable, then
/// extract with the system tar and run it from the extraction directory.
fn verify_archive(f: &Fixture, archive: &Path, name: &str, mode: &str, managed: &[u8]) -> Vec<u8> {
    let bytes = fs::read(archive).unwrap();
    let members = read_tar(&bytes);
    let names: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(
        names,
        [format!("bin/{name}").as_str(), "package.json", "README.txt"]
    );
    for (member, expected_mode) in members.iter().zip([0o755, 0o644, 0o644]) {
        assert_eq!(member.mode, expected_mode, "{}", member.name);
        assert_eq!(
            (member.uid, member.gid, member.mtime),
            (0, 0, 0),
            "{}",
            member.name
        );
    }
    let executable = &members[0].data;
    assert_eq!(
        executable.as_slice(),
        managed,
        "archive must hold the retained executable"
    );
    // A real LLVM-linked x86_64 ELF (ET_DYN for this -pie slice), not synthetic.
    assert_eq!(&executable[..4], b"\x7fELF");
    assert_eq!(&executable[4..7], &[2, 1, 1]);
    assert_eq!(&executable[16..20], &[3, 0, 62, 0]);
    assert!(executable.len() > 4096, "{}", executable.len());
    let digest = format!("{:x}", Sha256::digest(executable));
    let text = std::str::from_utf8(&members[1].data).unwrap();
    assert!(text.ends_with('\n'));
    let metadata = Json::parse(text);
    assert_eq!(
        metadata.keys(),
        [
            "schema",
            "name",
            "target",
            "profile",
            "entry_mode",
            "executable",
            "sha256",
            "dynamic_runtime"
        ]
    );
    for (key, value) in [
        ("schema", "oxid-native-bundle-v1"),
        ("name", name),
        ("target", "x86_64-unknown-linux-gnu"),
        ("profile", "native-preview-O0"),
        ("entry_mode", mode),
        ("executable", &format!("bin/{name}")),
        ("sha256", &digest),
        ("dynamic_runtime", "host-provided"),
    ] {
        assert_eq!(metadata.get(key).str(), value, "{key}");
    }
    let readme = std::str::from_utf8(&members[2].data).unwrap();
    assert!(
        readme.contains(&format!("./bin/{name}"))
            && readme.contains(&format!("Entry mode: {mode}."))
    );
    assert!(readme.contains("not run while packaging") && readme.contains("not included"));

    let extract = f.path(&format!("extract-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir(&extract).unwrap();
    let status = Command::new("tar")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(&extract)
        .status()
        .unwrap();
    assert!(status.success());
    let extracted = extract.join("bin").join(name);
    assert_eq!(fs::read(&extracted).unwrap(), managed);
    assert_eq!(
        fs::metadata(&extracted).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert_eq!(
        fs::read(extract.join("package.json")).unwrap(),
        members[1].data
    );
    assert_eq!(
        fs::read(extract.join("README.txt")).unwrap(),
        members[2].data
    );
    let mut listing: Vec<_> = fs::read_dir(&extract)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    listing.sort();
    assert_eq!(listing, ["README.txt", "bin", "package.json"]);
    bytes
}
fn run_extracted(f: &Fixture, archive: &Path, name: &str) -> Output {
    let extract = f.path(&format!("run-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir(&extract).unwrap();
    assert!(Command::new("tar")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(&extract)
        .status()
        .unwrap()
        .success());
    Command::new(format!("./bin/{name}"))
        .current_dir(&extract)
        .stdin(Stdio::null())
        .env_clear()
        .output()
        .unwrap()
}
fn namespace(f: &Fixture, member: &str, mode: &str) -> PathBuf {
    f.path(&format!("ws/{member}/.oxid/typed-preview/{mode}"))
}

// ---------------------------------------------------------------- ordinary
#[test]
fn argv_errors_json_precedence_and_no_side_effects() {
    let f = Fixture::new(None);
    let before = tree(&f.root);
    for (args, text) in [
        (
            vec!["--output=a.tar", "--output", "b.tar"],
            "--output may only be specified once",
        ),
        (
            vec!["--output", "a.tar", "--output=a.tar"],
            "--output may only be specified once",
        ),
        (vec![], "requires --output ARCHIVE"),
        (vec!["--output"], "--output requires a value"),
        (vec!["--output="], "--output requires a value"),
        // A following flag is never consumed as the archive path.
        (
            vec!["--output", "--entry-mode=process"],
            "--output requires a value",
        ),
        (
            vec!["--output=a.tar", "--package"],
            "--package requires a value",
        ),
        (
            vec!["--output=a.tar", "--entry-mode=both"],
            "unknown --entry-mode",
        ),
    ] {
        // JSON wins wherever --message-format=json appears, even after the error.
        for json_first in [true, false] {
            let mut argv = vec!["package", "--manifest-path=ws/oxid.toml"];
            if json_first {
                argv.push("--message-format=json");
            }
            argv.extend(&args);
            if !json_first {
                argv.push("--message-format=json");
            }
            let summary = failed(&f.oxid(".", &argv), "E0001", text);
            unpublished(&summary);
        }
    }
    // Text mode reports the same failure on stderr and a text summary on stdout.
    let text = f.oxid(
        ".",
        &[
            "package",
            "--manifest-path=ws/oxid.toml",
            "--package=npkg-app",
        ],
    );
    assert_eq!(text.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&text.stderr)
            .contains("error[E0001] (cli): requires --output ARCHIVE")
            || String::from_utf8_lossy(&text.stderr).contains("requires --output ARCHIVE")
    );
    assert_eq!(
        String::from_utf8_lossy(&text.stdout),
        "typed-preview native package failed: managed executable not published; archive not published\n"
    );
    assert_eq!(
        tree(&f.root),
        before,
        "CLI errors must not touch the filesystem"
    );
}

#[test]
fn existing_archive_paths_are_refused_before_compilation_and_left_intact() {
    let f = Fixture::new(None);
    fs::write(f.path("caller/target.bin"), b"live target").unwrap();
    fs::create_dir(f.path("caller/dir")).unwrap();
    fs::write(f.path("caller/dir/inner"), b"inner").unwrap();
    fs::write(f.path("caller/file.tar"), b"keep me").unwrap();
    symlink("target.bin", f.path("caller/live.tar")).unwrap();
    symlink("dir", f.path("caller/live-dir.tar")).unwrap();
    symlink("missing-target", f.path("caller/dangling.tar")).unwrap();
    symlink("dangling.tar", f.path("caller/chain.tar")).unwrap();
    let before = tree(&f.root);
    for out in [
        "file.tar",
        "dir",
        "dir/",
        "live.tar",
        "live-dir.tar",
        "dangling.tar",
        "chain.tar",
    ] {
        let summary = failed(
            &f.package("caller", "../ws/oxid.toml", APP, "result", out),
            "E0720",
            "already exists",
        );
        unpublished(&summary);
        assert_eq!(tree(&f.root), before, "{out}");
    }
    for (out, text) in [
        ("absent/out.tar", "output parent"),
        ("dangling.tar/out.tar", "output parent"),
        (
            "file.tar/out.tar",
            "inspect package output: Not a directory",
        ),
    ] {
        let summary = failed(
            &f.package("caller", "../ws/oxid.toml", APP, "result", out),
            "E0720",
            text,
        );
        unpublished(&summary);
        assert_eq!(tree(&f.root), before, "{out}");
    }
    assert!(
        !f.path("ws/app/.oxid").exists(),
        "no compilation before preflight"
    );
}

#[test]
fn legacy_route_without_manifest_selector_is_unchanged() {
    let f = Fixture::new(None);
    let before = tree(&f.root);
    // Expected bytes match the pre-package binary at 7c831e26 (same args).
    const UNKNOWN: &str = "error: unknown subcommand: package\n";
    const LEGACY_JSON: &str = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0001\",\"stage\":\"cli\",\"message\":\"--message-format requires --edition typed-preview\",\"primary\":null,\"secondary\":[],\"notes\":[]}\n{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":1,\"functions\":null}\n";
    for (args, stdout, stderr) in [
        (vec!["package"], "", UNKNOWN),
        (vec!["package", "--output=x.tar"], "", UNKNOWN),
        (
            vec!["package", "ws/app/src/main.ox", "--output", "x.tar"],
            "",
            UNKNOWN,
        ),
        (
            vec![
                "package",
                "--package",
                "npkg-app",
                "--output",
                "x.tar",
                "--message-format=json",
            ],
            LEGACY_JSON,
            "",
        ),
    ] {
        let output = f.oxid(".", &args);
        assert_eq!(output.status.code(), Some(1), "{args:?} {output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "{args:?}");
        assert_eq!(String::from_utf8_lossy(&output.stderr), stderr, "{args:?}");
    }
    assert_eq!(tree(&f.root), before);
}

// ---------------------------------------------------------------- native
#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN and a standard tar reader"]
fn native_result_and_process_packages_are_inert_independently_read_and_run() {
    let f = Fixture::new(Some(llvm_bin()));
    for (member, mode, stdout, code) in [
        (APP, "result", b"33\n".as_slice(), 0),
        (PROC, "process", b"PKG\n".as_slice(), 3),
    ] {
        let out = format!("{member}-{mode}.tar");
        let output = f.package("ws", "oxid.toml", member, mode, &out);
        // Packaging never runs the program: no "33"/"PKG" next to the summary.
        let summary = ok_summary(&output, mode);
        assert_eq!(String::from_utf8_lossy(&output.stdout).lines().count(), 1);
        let dir = &member[5..];
        let managed_rel = format!("{dir}/.oxid/typed-preview/{mode}/{member}");
        assert_eq!(summary.get("managed_output").str(), managed_rel);
        assert_eq!(summary.get("archive_output").str(), out);
        let (_, managed) = owned(&namespace(&f, &member[5..], mode), member);
        verify_archive(&f, &f.path(&format!("ws/{out}")), member, mode, &managed);
        let ran = run_extracted(&f, &f.path(&format!("ws/{out}")), member);
        assert_eq!(ran.status.code(), Some(code), "{ran:?}");
        assert_eq!(ran.stdout, stdout, "{ran:?}");
        assert!(ran.stderr.is_empty(), "{ran:?}");
    }
    // Library in Process mode: exit status is main's value, nothing on stdout.
    let output = f.package("ws", "oxid.toml", LIB, "process", "lib-process.tar");
    ok_summary(&output, "process");
    let ran = run_extracted(&f, &f.path("ws/lib-process.tar"), LIB);
    assert_eq!(
        (ran.status.code(), ran.stdout.as_slice()),
        (Some(55), b"".as_slice())
    );
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN and a standard tar reader"]
fn native_workspace_selection_path_dependency_and_caller_relative_archive() {
    let f = Fixture::new(Some(llvm_bin()));
    // Library member from a deeper, unrelated cwd; archive relative to that cwd.
    let output = f.package(
        "caller/deeper",
        "../../ws/oxid.toml",
        LIB,
        "result",
        "lib.tar",
    );
    let summary = ok_summary(&output, "result");
    assert_eq!(
        summary.get("managed_output").str(),
        "../../ws/library/.oxid/typed-preview/result/npkg-library"
    );
    assert_eq!(summary.get("archive_output").str(), "lib.tar");
    assert!(f.path("caller/deeper/lib.tar").is_file());
    for stray in [
        "ws/lib.tar",
        "ws/library/lib.tar",
        "caller/lib.tar",
        "ws/.oxid",
        "ws/app/.oxid",
        "ws/proc/.oxid",
    ] {
        assert!(!f.path(stray).exists(), "{stray}");
    }
    let (_, lib_exe) = owned(&namespace(&f, "library", "result"), LIB);
    verify_archive(
        &f,
        &f.path("caller/deeper/lib.tar"),
        LIB,
        "result",
        &lib_exe,
    );
    let ran = run_extracted(&f, &f.path("caller/deeper/lib.tar"), LIB);
    assert_eq!(
        (ran.status.code(), ran.stdout.as_slice()),
        (Some(0), b"55\n".as_slice())
    );

    // App through its path dependency: managed output only below app; the
    // library's managed tree is untouched (same inodes and bytes).
    let library_before = tree(&f.path("ws/library"));
    let output = f.package(
        "caller",
        "../ws/oxid.toml",
        APP,
        "result",
        "../caller/deeper/app.tar",
    );
    let summary = ok_summary(&output, "result");
    assert_eq!(
        summary.get("managed_output").str(),
        "../ws/app/.oxid/typed-preview/result/npkg-app"
    );
    assert_eq!(tree(&f.path("ws/library")), library_before);
    assert!(!f.path("ws/proc/.oxid").exists() && !f.path("ws/.oxid").exists());
    let (_, app_exe) = owned(&namespace(&f, "app", "result"), APP);
    verify_archive(
        &f,
        &f.path("caller/deeper/app.tar"),
        APP,
        "result",
        &app_exe,
    );
    let ran = run_extracted(&f, &f.path("caller/deeper/app.tar"), APP);
    assert_eq!(
        (ran.status.code(), ran.stdout.as_slice()),
        (Some(0), b"33\n".as_slice())
    );

    // Member manifest directly (no --package) with an absolute output path.
    let absolute = f.path("caller/abs.tar");
    let output = f.oxid(
        "caller/deeper",
        &[
            "package",
            "--manifest-path=../../ws/app/oxid.toml",
            "--output",
            absolute.to_str().unwrap(),
            "--message-format=json",
        ],
    );
    let summary = ok_summary(&output, "result");
    assert_eq!(
        summary.get("archive_output").str(),
        absolute.to_str().unwrap()
    );
    let (_, app_exe) = owned(&namespace(&f, "app", "result"), APP);
    verify_archive(&f, &absolute, APP, "result", &app_exe);

    // A workspace root without --package selects nothing and publishes nothing.
    let app_before = tree(&f.path("ws/app"));
    let output = f.oxid(
        "caller",
        &[
            "package",
            "--manifest-path=../ws/oxid.toml",
            "--output=none.tar",
            "--message-format=json",
        ],
    );
    unpublished(&failed(&output, "E0001", "requires --package"));
    let output = f.package(
        "caller",
        "../ws/oxid.toml",
        "npkg-absent",
        "result",
        "none.tar",
    );
    unpublished(&failed(
        &output,
        "E0001",
        "npkg-app, npkg-library, npkg-proc",
    ));
    assert!(!f.path("caller/none.tar").exists());
    assert_eq!(tree(&f.path("ws/app")), app_before);
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN and a standard tar reader"]
fn native_identical_executable_and_metadata_give_identical_archive_bytes() {
    let f = Fixture::new(Some(llvm_bin()));
    let first = f.package("ws", "oxid.toml", APP, "result", "a.tar");
    ok_summary(&first, "result");
    let (ino_a, exe_a) = owned(&namespace(&f, "app", "result"), APP);
    let tar_a = verify_archive(&f, &f.path("ws/a.tar"), APP, "result", &exe_a);
    // Second invocation from the same cwd rebuilds and republishes under a
    // different archive name (the name is not archive content).
    let second = f.package("ws", "oxid.toml", APP, "result", "b-other-name.tar");
    ok_summary(&second, "result");
    let (ino_b, exe_b) = owned(&namespace(&f, "app", "result"), APP);
    assert_ne!(
        ino_a, ino_b,
        "each package build publishes a new managed inode"
    );
    let tar_b = verify_archive(&f, &f.path("ws/b-other-name.tar"), APP, "result", &exe_b);
    if exe_a == exe_b {
        assert_eq!(
            tar_a, tar_b,
            "same executable and metadata must give identical archive bytes"
        );
        eprintln!(
            "compared: identical executable bytes, archives byte-identical ({} B)",
            tar_a.len()
        );
    } else {
        // Not a reproducible-build claim; only the archive function is in scope.
        assert_ne!(tar_a, tar_b);
        eprintln!("same-cwd executables differ; archive identity not comparable");
    }
    // Observation only: another caller cwd changes caller-relative source paths
    // embedded in runtime diagnostics, so executable (and archive) bytes may
    // differ. The archive must still describe exactly the retained bytes.
    let third = f.package("caller", "../ws/oxid.toml", APP, "result", "c.tar");
    ok_summary(&third, "result");
    let (_, exe_c) = owned(&namespace(&f, "app", "result"), APP);
    let tar_c = verify_archive(&f, &f.path("caller/c.tar"), APP, "result", &exe_c);
    eprintln!(
        "other-cwd executable identical: {}; archive identical: {}",
        exe_c == exe_a,
        tar_c == tar_a
    );
    assert_eq!(exe_c == exe_a, tar_c == tar_a);
    // Different entry metadata over the same source changes package.json.
    let process = f.package("ws", "oxid.toml", APP, "process", "p.tar");
    ok_summary(&process, "process");
    let (_, exe_p) = owned(&namespace(&f, "app", "process"), APP);
    let tar_p = verify_archive(&f, &f.path("ws/p.tar"), APP, "process", &exe_p);
    assert_ne!(tar_p, tar_a);
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN and a standard tar reader"]
fn native_source_type_tool_and_ownership_failures_keep_old_artifact() {
    let llvm = llvm_bin();
    let f = Fixture::new(Some(llvm.clone()));
    ok_summary(
        &f.package("ws", "oxid.toml", APP, "result", "v1.tar"),
        "result",
    );
    let ns = namespace(&f, "app", "result");
    let (ino, exe) = owned(&ns, APP);
    // First publication: never assume a `current` receipt already exists.
    let snapshot = tree_slots_folded(&f.path("ws"));
    let main = EXAMPLE[2].1;

    let check = |label: &str, output: Output, code: &str, text: &str, f: &Fixture| {
        let summary = failed(&output, code, text);
        unpublished(&summary);
        assert_eq!(
            tree_slots_folded(&f.path("ws")),
            snapshot,
            "{label}: old artifact/receipts changed"
        );
        assert_eq!(owned(&ns, APP), (ino, exe.clone()), "{label}");
    };
    // Source error (unknown module item) and type error in the dependency.
    f.write(
        "ws/app/src/main.ox",
        &main.replace("make_pair()", "make_pear()"),
    );
    let snapshot_src = tree(&f.path("ws"));
    let out = f.package("ws", "oxid.toml", APP, "result", "src.tar");
    assert!(!f.path("ws/src.tar").exists());
    let summary = failed(&out, "E0200", "make_pear");
    unpublished(&summary);
    assert_eq!(tree(&f.path("ws")), snapshot_src);
    f.write("ws/app/src/main.ox", main);
    let lib = EXAMPLE[4].1;
    f.write(
        "ws/library/src/lib.ox",
        &lib.replace("return 3;", "return true;"),
    );
    let snapshot_ty = tree(&f.path("ws"));
    let out = f.package("ws", "oxid.toml", APP, "result", "ty.tar");
    unpublished(&failed(&out, "E0300", "type mismatch"));
    assert_eq!(tree(&f.path("ws")), snapshot_ty);
    assert!(!f.path("ws/ty.tar").exists());
    f.write("ws/library/src/lib.ox", lib);
    assert_eq!(tree_slots_folded(&f.path("ws")), snapshot);

    // Tool error: compiler cannot launch after staging; stage is cleaned up.
    let mut absent = f.command(
        "ws",
        &[
            "package",
            "--manifest-path=oxid.toml",
            "--package=npkg-app",
            "--output=tool.tar",
            "--message-format=json",
        ],
    );
    absent.env("OXID_LLVM_BIN", f.path("absent-llvm"));
    check(
        "tool",
        absent.output().unwrap(),
        "E0701",
        "cannot launch",
        &f,
    );
    assert!(!f.path("ws/tool.tar").exists());

    // Entry-mode error: bounded stdout program in Result mode.
    let proc_before = tree(&f.path("ws/proc"));
    unpublished(&failed(
        &f.package("ws", "oxid.toml", PROC, "result", "e.tar"),
        "E0609",
        "process entry mode",
    ));
    assert_eq!(tree(&f.path("ws/proc")), proc_before);
    assert_eq!(tree_slots_folded(&f.path("ws")), snapshot);

    // Stale stage: explicit refusal, stage left for the user, output intact.
    fs::create_dir(ns.join(format!(".{APP}.build/stage"))).unwrap();
    let staged = tree_slots_folded(&f.path("ws"));
    let summary = failed(
        &f.package("ws", "oxid.toml", APP, "result", "stage.tar"),
        "E0710",
        "interrupted build staging exists",
    );
    unpublished(&summary);
    assert_eq!(tree_slots_folded(&f.path("ws")), staged);
    fs::remove_dir(ns.join(format!(".{APP}.build/stage"))).unwrap();

    // Unowned replacement and symlinked output are refused and preserved.
    fs::remove_file(ns.join(APP)).unwrap();
    fs::write(ns.join(APP), b"unowned bytes").unwrap();
    let unowned = tree_slots_folded(&f.path("ws"));
    unpublished(&failed(
        &f.package("ws", "oxid.toml", APP, "result", "u.tar"),
        "E0710",
        "unowned",
    ));
    assert_eq!(tree_slots_folded(&f.path("ws")), unowned);
    fs::remove_file(ns.join(APP)).unwrap();
    // Point at whichever receipt slot currently holds the old inode.
    let slot = receipt_slot(&ns, APP);
    symlink(format!(".{APP}.build/{slot}"), ns.join(APP)).unwrap();
    let linked = tree_slots_folded(&f.path("ws"));
    unpublished(&failed(
        &f.package("ws", "oxid.toml", APP, "result", "s.tar"),
        "E0710",
        "symlinks are rejected",
    ));
    assert_eq!(tree_slots_folded(&f.path("ws")), linked);
    assert!(!f.path("ws/u.tar").exists() && !f.path("ws/s.tar").exists());

    // Restoring the owned name lets a later package publish again.
    fs::remove_file(ns.join(APP)).unwrap();
    let slot = receipt_slot(&ns, APP);
    fs::hard_link(ns.join(format!(".{APP}.build/{slot}")), ns.join(APP)).unwrap();
    ok_summary(
        &f.package("ws", "oxid.toml", APP, "result", "v2.tar"),
        "result",
    );
    let (ino2, exe2) = owned(&ns, APP);
    assert_ne!(ino2, ino);
    verify_archive(&f, &f.path("ws/v2.tar"), APP, "result", &exe2);
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN and a standard tar reader"]
fn native_packaging_failure_after_build_acknowledges_new_managed_output() {
    // Reliable injection: an existing but unwritable output parent passes the
    // preflight, then the temporary archive cannot be created. Root ignores
    // directory permissions, so the injection would be unreliable there.
    assert_ne!(
        unsafe { libc::geteuid() },
        0,
        "permission injection is unreliable as root"
    );
    let f = Fixture::new(Some(llvm_bin()));
    ok_summary(
        &f.package("ws", "oxid.toml", APP, "result", "old.tar"),
        "result",
    );
    let ns = namespace(&f, "app", "result");
    let (old_ino, _) = owned(&ns, APP);
    fs::create_dir(f.path("caller/ro")).unwrap();
    fs::set_permissions(f.path("caller/ro"), fs::Permissions::from_mode(0o555)).unwrap();
    for json in [true, false] {
        let mut args = vec![
            "package",
            "--manifest-path=../ws/oxid.toml",
            "--package=npkg-app",
            "--output=ro/new.tar",
        ];
        if json {
            args.push("--message-format=json");
        }
        let output = f.oxid("caller", &args);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let (new_ino, new_exe) = owned(&ns, APP);
        assert_ne!(new_ino, old_ino, "the new managed executable was published");
        if json {
            assert!(output.stderr.is_empty());
            let (diagnostics, summary) = records(&output.stdout);
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].get("code").str(), "E0720");
            assert!(diagnostics[0]
                .get("message")
                .str()
                .contains("create package temporary file"));
            assert_eq!(summary.get("managed_published"), &Json::Bool(true));
            let managed = summary.get("managed_output").str();
            assert_eq!(managed, "../ws/app/.oxid/typed-preview/result/npkg-app");
            assert_eq!(fs::read(f.path("caller").join(managed)).unwrap(), new_exe);
            assert_eq!(summary.get("archive_published"), &Json::Bool(false));
            assert_eq!(summary.get("archive_output"), &Json::Null);
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            assert!(
                err.contains("E0720") && err.contains("create package temporary file"),
                "{err}"
            );
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                "typed-preview native package failed: managed executable \"../ws/app/.oxid/typed-preview/result/npkg-app\"; archive not published\n"
            );
        }
        let all = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .to_lowercase();
        assert!(
            !all.contains("rollback") && !all.contains("rolled back") && !all.contains("restored"),
            "{all}"
        );
        assert_eq!(
            fs::read_dir(f.path("caller/ro")).unwrap().count(),
            0,
            "no archive or temporary"
        );
    }
    fs::set_permissions(f.path("caller/ro"), fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN, /dev/full and a standard tar reader"]
fn native_report_stream_failure_keeps_published_archive() {
    let f = Fixture::new(Some(llvm_bin()));
    let ns = namespace(&f, "app", "result");
    for (json, out) in [(true, "full-json.tar"), (false, "full-text.tar")] {
        let mut args = vec![
            "package",
            "--manifest-path=../ws/oxid.toml",
            "--package=npkg-app",
            "--output",
            out,
        ];
        if json {
            args.push("--message-format=json");
        }
        let full = fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .unwrap();
        let output = f.command("caller", &args).stdout(full).output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let err = String::from_utf8_lossy(&output.stderr);
        assert!(
            err.contains("E0721") && err.contains("package report failed"),
            "{err}"
        );
        assert!(
            err.contains("managed executable \"../ws/app/.oxid/typed-preview/result/npkg-app\""),
            "{err}"
        );
        assert!(
            err.contains(&format!("archive \"{out}\""))
                && err.contains("published files were retained"),
            "{err}"
        );
        let lower = err.to_lowercase();
        assert!(
            !lower.contains("rollback") && !lower.contains("rolled back"),
            "{err}"
        );
        let (_, exe) = owned(&ns, APP);
        verify_archive(&f, &f.path(&format!("caller/{out}")), APP, "result", &exe);
    }
    let leftovers: Vec<_> = fs::read_dir(f.path("caller"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .filter(|n| n.to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}
