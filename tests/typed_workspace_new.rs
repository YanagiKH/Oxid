//! `oxid new PATH --edition typed-preview --workspace`: an explicit virtual
//! workspace scaffold beside the unchanged plain typed and legacy templates.
//! Ordinary interpreter/formatter routes only; no native toolchain is used.
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// The exact generated inventory: three root files and two ordinary members.
const SEVEN: [&str; 7] = [
    ".gitignore",
    "README.md",
    "app/main.ox",
    "app/oxid.toml",
    "library/lib.ox",
    "library/oxid.toml",
    "oxid.toml",
];
const APP_PASS: &[u8] =
    b"PASS test_answer\nPASS test_unit\ntest result: 2 passed; 0 failed; 2 total\n";
const LIB_PASS: &[u8] = b"PASS test_library\ntest result: 1 passed; 0 failed; 1 total\n";
const MISSING_PACKAGE: &str = "available packages: workspace-app, workspace-library";

// Exact fixed template bytes from the reviewed workspace scaffold source
// (candidate-manifest generated_template_files); not a loosened oracle.
const WS_FILES: [(&str, &str); 7] = [
    (".gitignore", ".oxid/\n"),
    ("README.md", "# Typed Oxid workspace\n\nThis workspace uses the experimental typed-preview edition.\nIt contains two ordinary projects, workspace-app and workspace-library.\nThe app explicitly imports its local library through the math path dependency.\nWorkspace membership alone does not make an import available.\n\nFrom this directory:\n\n- Check: `oxid check --manifest-path oxid.toml --package workspace-app`\n- Interpret: `oxid run --manifest-path oxid.toml --package workspace-app`\n- Test: `oxid test --manifest-path oxid.toml --package workspace-app`\n- Build: `oxid build --manifest-path oxid.toml --package workspace-app`\n- Run the library project: `oxid run --manifest-path oxid.toml --package workspace-library`\n- Check every member: `oxid check --manifest-path oxid.toml --workspace`\n- Test every member: `oxid test --manifest-path oxid.toml --workspace`\n\nThe app prints 7 and its two tests pass. The library project prints 91.\nEdit `library/lib.ox` to change the public number function used by the app.\nWith two members, single-package commands require --package, which selects the project name, not a path.\nThe --workspace commands check or interpreter-test each member in declared order and report package totals.\nBuild requires Linux x86_64 and LLVM 19.1.7 (clang, opt and ld.lld).\nApplication outputs belong under the selected member's .oxid directory, not the virtual root.\nThis scaffold has only local path dependencies and creates no lockfile or downloaded packages.\nTyped-preview is not a stable language release.\n"),
    ("app/main.ox", "use math::number;\nfn main() -> i32 { return number(); }\nfn test_answer() -> bool { return number() == 7; }\nfn test_unit() -> () { return; }\n"),
    ("app/oxid.toml", "[project]\nname = \"workspace-app\"\nedition = \"typed-preview\"\nentry = \"main.ox\"\n\n[dependencies]\nmath = \"../library\"\n"),
    ("library/lib.ox", "pub fn number() -> i32 { return 7; }\nfn main() -> i32 { return 91; }\nfn test_library() -> bool { return true; }\n"),
    ("library/oxid.toml", "[project]\nname = \"workspace-library\"\nedition = \"typed-preview\"\nentry = \"lib.ox\"\n"),
    ("oxid.toml", "[workspace]\nmembers = [\"app\", \"library\"]\n"),
];

// Plain typed `new` bytes at the base revision; the workspace flag must not change them.
const PLAIN_MAIN: &str = "mod answer;\nfn main() -> i32 {\n    return crate::answer::value();\n}\n";
const PLAIN_ANSWER: &str = "pub fn value() -> i32 {\n    return 7;\n}\n";
const PLAIN_README: &str = "# Typed Oxid project\n\nThis project uses the typed-preview edition.\n\nFrom this directory:\n\n- Check: `oxid check --manifest-path oxid.toml`\n- Interpret: `oxid run --manifest-path oxid.toml`\n- Build: `oxid build --manifest-path oxid.toml`\n\nThe example returns 7. Result-mode execution prints the value and exits successfully.\nEdit `src/answer.ox` to change it. Build currently requires Linux x86_64 and LLVM 19.1.7.\nThe generated project has no external dependencies. Typed-preview is not a stable language release.\n";

#[derive(Debug, PartialEq, Eq, Clone)]
enum Entry {
    Dir,
    File(Vec<u8>),
    Link(PathBuf),
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        for _ in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "oxid-typed-workspace-new-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("{error}"),
            }
        }
        panic!("scratch collisions");
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .args(args)
            .stdin(Stdio::null())
            .env_remove("OXID_PATH")
            .output()
            .unwrap()
    }
    fn snapshot(&self) -> BTreeMap<String, Entry> {
        tree(&self.0)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Every entry below `root` (not following links), keyed by `/`-joined relative path.
fn tree(root: &Path) -> BTreeMap<String, Entry> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Entry>) {
        for item in fs::read_dir(dir).unwrap() {
            let path = item.unwrap().path();
            let key = path
                .strip_prefix(root)
                .unwrap()
                .components()
                .map(|part| part.as_os_str().to_str().unwrap().to_owned())
                .collect::<Vec<_>>()
                .join("/");
            let meta = fs::symlink_metadata(&path).unwrap();
            if meta.file_type().is_symlink() {
                out.insert(key, Entry::Link(fs::read_link(&path).unwrap()));
            } else if meta.is_dir() {
                out.insert(key, Entry::Dir);
                walk(root, &path, out);
            } else {
                out.insert(key, Entry::File(fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn success(out: Output) -> Vec<u8> {
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    out.stdout
}

fn exact(out: Output, stdout: &[u8]) {
    assert_eq!(success(out), stdout);
}

/// A rejected CLI request: exit 1, text diagnostic on stderr, nothing on stdout.
fn rejected(out: &Output) {
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert!(!out.stderr.is_empty(), "{out:?}");
}

fn create_workspace(s: &Scratch, path: &str) -> Vec<u8> {
    success(s.command(&["new", path, "--edition", "typed-preview", "--workspace"]))
}

fn assert_seven(root: &Path) {
    let inventory = tree(root);
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for (key, entry) in &inventory {
        match entry {
            Entry::Dir => dirs.push(key.as_str()),
            Entry::File(_) => files.push(key.as_str()),
            Entry::Link(_) => panic!("unexpected symlink {key}"),
        }
    }
    assert_eq!(dirs, ["app", "library"], "{inventory:?}");
    assert_eq!(files, SEVEN, "{inventory:?}");
}

fn no_state(root: &Path) {
    for key in tree(root).keys() {
        let leaf = key.rsplit('/').next().unwrap();
        assert!(
            !matches!(leaf, ".oxid" | ".git" | "oxid.lock" | "target" | "cache"),
            "unexpected generated state {key}"
        );
    }
}

fn read(root: &Path, relative: &str) -> String {
    fs::read_to_string(root.join(relative)).unwrap()
}

#[test]
fn workspace_new_creates_exactly_the_seven_file_virtual_workspace() {
    let s = Scratch::new();
    let stdout = create_workspace(&s, "demo");
    assert_eq!(stdout, b"created typed workspace: \"demo\"\n");
    assert_eq!(s.snapshot().keys().next().map(String::as_str), Some("demo"));
    assert_eq!(s.snapshot().len(), 10);
    let root = s.0.join("demo");
    assert_seven(&root);
    no_state(&root);
    let mut expected: BTreeMap<String, Entry> = WS_FILES
        .iter()
        .map(|(key, text)| ((*key).to_owned(), Entry::File(text.as_bytes().to_vec())))
        .collect();
    expected.insert("app".to_owned(), Entry::Dir);
    expected.insert("library".to_owned(), Entry::Dir);
    assert_eq!(tree(&root), expected);
}

#[test]
fn generated_members_check_run_and_test_through_the_interpreter() {
    let s = Scratch::new();
    create_workspace(&s, "ws");
    let root = s.0.join("ws");
    let generated = tree(&root);
    let m = "ws/oxid.toml";
    for package in ["workspace-app", "workspace-library"] {
        success(s.command(&["check", "--manifest-path", m, "--package", package]));
    }
    exact(
        s.command(&["run", "--manifest-path", m, "--package=workspace-app"]),
        b"7\n",
    );
    exact(
        s.command(&["test", "--manifest-path", m, "--package=workspace-app"]),
        APP_PASS,
    );
    exact(
        s.command(&[
            "run",
            "--manifest-path",
            m,
            "--package",
            "workspace-library",
        ]),
        b"91\n",
    );
    exact(
        s.command(&[
            "test",
            "--manifest-path",
            m,
            "--package",
            "workspace-library",
        ]),
        LIB_PASS,
    );
    // Members are ordinary projects: their own manifests work directly as well.
    exact(
        s.command(&["run", "--manifest-path", "ws/app/oxid.toml"]),
        b"7\n",
    );
    exact(
        s.command(&["run", "--manifest-path", "ws/library/oxid.toml"]),
        b"91\n",
    );
    // Interpreter routes create no lock, cache or artifact state anywhere.
    assert_eq!(tree(&root), generated);
    no_state(&root);
    // The explicit dependency is live: editing the library changes the app.
    fs::write(
        root.join("library/lib.ox"),
        "pub fn number() -> i32 { return 8; }\nfn main() -> i32 { return 91; }\nfn test_library() -> bool { return true; }\n",
    )
    .unwrap();
    exact(
        s.command(&["run", "--manifest-path", m, "--package=workspace-app"]),
        b"8\n",
    );
    let out = s.command(&["test", "--manifest-path", m, "--package=workspace-app"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(
        out.stdout,
        b"FAIL test_answer\nPASS test_unit\ntest result: 1 passed; 1 failed; 2 total\n"
    );
    assert!(out.stderr.is_empty(), "{out:?}");
}

#[test]
fn generated_member_sources_are_already_formatter_normalized() {
    let s = Scratch::new();
    create_workspace(&s, "ws");
    let generated = s.snapshot();
    for package in ["workspace-app", "workspace-library"] {
        success(s.command(&[
            "fmt",
            "--manifest-path",
            "ws/oxid.toml",
            "--package",
            package,
            "--check",
        ]));
    }
    for member in ["ws/app/oxid.toml", "ws/library/oxid.toml"] {
        success(s.command(&["fmt", "--manifest-path", member, "--check"]));
    }
    assert_eq!(s.snapshot(), generated);
}

#[test]
fn generated_workspace_requires_an_explicit_package() {
    let s = Scratch::new();
    create_workspace(&s, "ws");
    let generated = s.snapshot();
    for command in ["check", "run", "test"] {
        let out = s.command(&[command, "--manifest-path", "ws/oxid.toml"]);
        rejected(&out);
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(MISSING_PACKAGE),
            "{out:?}"
        );
        let out = s.command(&[
            command,
            "--manifest-path",
            "ws/oxid.toml",
            "--package=workspace",
        ]);
        rejected(&out);
    }
    assert_eq!(s.snapshot(), generated);
}

#[test]
fn plain_typed_and_legacy_new_are_unchanged() {
    let s = Scratch::new();
    exact(
        s.command(&["new", "--edition", "typed-preview", "plain"]),
        b"created typed project: \"plain\"\n",
    );
    let plain = tree(&s.0.join("plain"));
    let file = |text: &str| Entry::File(text.as_bytes().to_vec());
    let expected: BTreeMap<String, Entry> = [
        (".gitignore", file(".oxid/\n")),
        ("README.md", file(PLAIN_README)),
        (
            "oxid.toml",
            file("[project]\nname = \"plain\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n"),
        ),
        ("src", Entry::Dir),
        ("src/answer.ox", file(PLAIN_ANSWER)),
        ("src/main.ox", file(PLAIN_MAIN)),
    ]
    .into_iter()
    .map(|(key, entry)| (key.to_owned(), entry))
    .collect();
    assert_eq!(plain, expected);
    exact(
        s.command(&["run", "--manifest-path", "plain/oxid.toml"]),
        b"7\n",
    );

    success(s.command(&["new", "legacy"]));
    let manifest = read(&s.0, "legacy/oxid.toml");
    assert!(!manifest.contains("typed-preview") && !manifest.contains("[workspace]"));
    assert!(manifest.contains("[dependencies]"));
    assert!(read(&s.0, "legacy/src/main.ox").contains("fun main()"));
}

#[test]
fn workspace_flag_positions_and_separator_ownership() {
    let s = Scratch::new();
    for (args, name) in [
        (
            vec!["--edition=typed-preview", "new", "--workspace", "a"],
            "a",
        ),
        (
            vec!["new", "--workspace", "b", "--edition", "typed-preview"],
            "b",
        ),
        (
            vec!["new", "--edition=typed-preview", "--workspace", "--", "c"],
            "c",
        ),
    ] {
        success(s.command(&args));
        assert_seven(&s.0.join(name));
    }
    // After `--`, `--workspace` is a literal operand, never the flag.
    let before = s.snapshot();
    let out = s.command(&["new", "--edition=typed-preview", "--", "--workspace"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert_eq!(s.snapshot(), before);
    let out = s.command(&["new", "--edition=typed-preview", "d", "--", "--workspace"]);
    rejected(&out);
    assert_eq!(s.snapshot(), before);
    // Without the flag, a separated path still selects the plain typed template.
    success(s.command(&["new", "--edition=typed-preview", "--", "e"]));
    assert!(s.0.join("e/src/main.ox").is_file() && !s.0.join("e/app").exists());
}

#[test]
fn invalid_workspace_requests_have_no_filesystem_effects() {
    let s = Scratch::new();
    let typed: &[&[&str]] = &[
        &["new", "--edition=typed-preview", "--workspace"],
        &["new", "--edition=typed-preview", "--workspace", ""],
        &["new", "--edition=typed-preview", "--workspace", "a", "b"],
        &["new", "--edition=typed-preview", "--workspace=true", "ws"],
        &["new", "--edition=typed-preview", "--workspace=", "ws"],
        &[
            "new",
            "--edition=typed-preview",
            "--workspace",
            "--workspace",
            "ws",
        ],
        &[
            "new",
            "--edition=typed-preview",
            "--workspace",
            "ws",
            "--workspace",
        ],
        &["--workspace", "new", "--edition=typed-preview", "ws"],
        &[
            "new",
            "--edition=typed-preview",
            "--workspace",
            "--backend=llvm",
            "ws",
        ],
        &[
            "new",
            "--edition=typed-preview",
            "--workspace",
            "--engine=native",
            "ws",
        ],
        &[
            "new",
            "--edition=typed-preview",
            "--workspace",
            "--entry-mode=process",
            "ws",
        ],
        &[
            "new",
            "--edition=typed-preview",
            "--workspace",
            "--package=ws",
            "ws",
        ],
        &["new", "--edition=typed-preview", "--workspaces", "ws"],
        &[
            "new",
            "--edition=typed-preview",
            "--workspace",
            "missing/ws",
        ],
    ];
    // An unseparated flag without explicit typed-preview is refused, never a
    // legacy project directory literally named `--workspace`.
    let untyped: &[&[&str]] = &[
        &["new", "--workspace"],
        &["new", "--workspace", "ws"],
        &["new", "ws", "--workspace"],
        &["--edition=legacy-0.9", "new", "--workspace", "ws"],
        &["new", "--edition", "legacy-0.9", "ws", "--workspace"],
    ];
    for args in typed.iter().chain(untyped) {
        let out = s.command(args);
        rejected(&out);
        assert_eq!(fs::read_dir(&s.0).unwrap().count(), 0, "{args:?}");
    }
    for args in untyped {
        let out = s.command(args);
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("--workspace"),
            "{args:?}: {out:?}"
        );
    }
    // JSON keeps the existing typed `new` contract: stdout diagnostic, no files.
    let out = s.command(&[
        "new",
        "--edition=typed-preview",
        "--workspace",
        "--message-format=json",
        "ws",
    ]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("typed-preview new does not support --message-format=json"));
    assert_eq!(text.lines().count(), 2);
    assert_eq!(fs::read_dir(&s.0).unwrap().count(), 0);
}

#[test]
fn workspace_new_never_overwrites_or_follows_an_existing_destination() {
    let s = Scratch::new();
    fs::write(s.0.join("file"), b"keep").unwrap();
    fs::create_dir(s.0.join("empty")).unwrap();
    fs::create_dir(s.0.join("target")).unwrap();
    let mut cases = vec!["file", "empty"];
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("target", s.0.join("link")).unwrap();
        std::os::unix::fs::symlink("absent", s.0.join("dangling")).unwrap();
        cases.extend(["link", "dangling"]);
    }
    let before = s.snapshot();
    for name in cases {
        let out = s.command(&["new", "--edition=typed-preview", "--workspace", name]);
        rejected(&out);
        assert_eq!(s.snapshot(), before, "{name}");
    }
    assert!(!s.0.join("absent").exists());
    // A first successful creation is followed by a refusal that keeps its bytes.
    create_workspace(&s, "ws");
    let created = s.snapshot();
    let out = s.command(&["new", "--edition=typed-preview", "--workspace", "ws"]);
    rejected(&out);
    assert_eq!(s.snapshot(), created);
    // The parent directory must already exist; it is never created.
    let out = s.command(&[
        "new",
        "--edition=typed-preview",
        "--workspace",
        "no/such/ws",
    ]);
    rejected(&out);
    assert_eq!(s.snapshot(), created);
}

#[cfg(unix)]
#[test]
fn script_arguments_keep_workspace_words_literal() {
    let s = Scratch::new();
    fs::write(s.0.join("payload.sh"), "printf '%s\\n' \"$@\"\n").unwrap();
    fs::write(
        s.0.join("oxid.toml"),
        "[scripts]\npayload = \"sh payload.sh\"\n",
    )
    .unwrap();
    let before = s.snapshot();
    let out = s.command(&[
        "script",
        "payload",
        "new",
        "--edition=typed-preview",
        "--workspace",
        "ws",
    ]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        out.stdout,
        b"new\n--edition=typed-preview\n--workspace\nws\n"
    );
    assert!(out.stderr.is_empty(), "{out:?}");
    assert_eq!(s.snapshot(), before);
}
