//! Companion acceptance for the typed-preview project documentation command
//! `oxid doc --manifest-path PATH [--package NAME]`.
//!
//! The ordinary workspace `examples/projects/api_docs` (members
//! `api-docs-geometry` and `api-docs-app`, plus the unnamed path dependency
//! `geometry/vendor/units`) is copied byte for byte into a fresh temp tree; the
//! in-tree example is never documented in place. Every expected document below
//! is a frozen literal written by hand from the handoff layout, never produced
//! by the renderer. Each invocation gets a regular-file stdin whose shared
//! offset must stay at zero, recording LLVM stubs on `OXID_LLVM_BIN` and `PATH`
//! that must never run, and a whole-tree byte snapshot that must not change.
//! Failures compare their first diagnostic code/stage with manifest `check`.
//! No native, held, fuel, capacity or provider work is exercised.
#![cfg(target_os = "linux")]
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Seek,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const EXAMPLE: &str = "examples/projects/api_docs";
const GEOMETRY: &str = "api-docs-geometry";
const APP: &str = "api-docs-app";
const STDIN: &[u8] = b"stdin belongs to the caller and is never consumed\n";

/// Every example file, relative to `EXAMPLE`, copied in this fixed order.
const FILES: [&str; 17] = [
    "README.md",
    "oxid.toml",
    "app/oxid.toml",
    "app/src/main.ox",
    "geometry/oxid.toml",
    "geometry/src/lib.ox",
    "geometry/src/shapes.ox",
    "geometry/src/shapes/metrics.ox",
    "geometry/src/shapes/draft.ox",
    "geometry/src/shapes/hidden.ox",
    "geometry/src/notes.ox",
    "geometry/src/internal.ox",
    "geometry/src/internal/exposed.ox",
    "geometry/src/name_rules.ox",
    "geometry/src/name_rules/alias_pairs.ox",
    "geometry/vendor/units/oxid.toml",
    "geometry/vendor/units/lib.ox",
];

/// `oxid doc --manifest-path oxid.toml --package api-docs-geometry`.
/// Module order is discovery DFS (`crate`, `shapes`, `shapes::metrics`,
/// `shapes::draft`, `notes`, `name_rules`, `name_rules::alias_pairs`); private
/// `shapes::hidden`, `internal` and its public child `internal::exposed` are
/// omitted. Heading and metadata punctuation (including `_`) is escaped while
/// `oxid` code blocks stay verbatim. `name_rules` pairs a local `struct Local`
/// with the function alias `use units::origin as Local;` and `alias_pairs`
/// pairs the type and value aliases `Span`: only the real type import is a
/// spelling guide. The `"> \n"` pieces are the rendering of an empty `///` line.
const GEOMETRY_DOC: &str = concat!(
    r##"# Typed API: api\-docs\-geometry

Edition: typed-preview (experimental).

The selected package was checked; dependencies are checked but not documented.
No code was executed. Source paths are relative to the selected entry directory.

## Module crate

Source: lib\.ox

### Signature imports

```oxid
use units::Point as UnitPoint;
use units::Axis as Direction;
use crate::shapes::Circle;
```

### struct crate::Point

Source: lib\.ox:18:12

```oxid
pub struct Point {
    pub x: i32,
    pub y: i32,
}
```

> A point on the plane\: \`x\` and \`y\` are \<i32\> values\.
"##,
    "> \n",
    r##"> See \[the guide\]\(https\:\/\/example\.com\/guide\?q\=1\&r\=2\) — café\, 日本語\, 🦀\.

### fn crate::zoom

Source: lib\.ox:25:8

```oxid
pub fn zoom(p: &mut Point, factor: i32) -> ();
```

> Scales \`p\` in place by \`factor\`\.

### struct crate::Account

Source: lib\.ox:32:12

```oxid
pub struct Account {
    pub id: i32,
    pub tags: [i32; 3],
    // private fields omitted
}
```

> Account with hidden state\.

### fn crate::convert

Source: lib\.ox:39:8

```oxid
pub fn convert(p: &UnitPoint) -> Point;
```

### fn crate::start

Source: lib\.ox:43:8

```oxid
pub fn start() -> UnitPoint;
```

### enum crate::Shape

Source: lib\.ox:48:10

```oxid
pub enum Shape {
    Empty,
    Dot(Point),
    Ring(Circle),
    Line([i32; 4]),
    Flag(bool),
    Nothing(()),
    Toward(Direction),
}
```

> Every payload form\.

### fn crate::total

Source: lib\.ox:59:9

```oxid
pub fn total(values: &[i32], bytes: &mut [u8], window: &[i32; 4]) -> i32;
```

### struct crate::Marker

Source: lib\.ox:78:12

```oxid
pub struct Marker {
}
```

> Marker with no fields\.

### struct crate::Sealed

Source: lib\.ox:80:12

```oxid
pub struct Sealed {
    // private fields omitted
}
```

## Module crate::shapes

Source: shapes\.ox

> Shapes built on the root \`Point\`\.

### struct crate::shapes::Circle

Source: shapes\.ox:8:12

```oxid
pub struct Circle {
    pub radius: i32,
    // private fields omitted
}
```

> A circle around the root point\.

### fn crate::shapes::make\_circle

Source: shapes\.ox:14:8

```oxid
pub fn make_circle(radius: i32) -> Circle;
```

> Builds a circle at the origin\.

### fn crate::shapes::center

Source: shapes\.ox:19:8

```oxid
pub fn center(c: &Circle) -> crate::Point;
```

> Returns the center through an absolute path\.

### fn crate::shapes::area

Source: shapes\.ox:24:8

```oxid
pub fn area(c: &Circle) -> i32;
```

> Same spelling as \`units\:\:area\`\; only this one is documented\.

## Module crate::shapes::metrics

Source: shapes\/metrics\.ox

> Shape measurements\.

### Signature imports

```oxid
use crate::shapes::Circle as Round;
use units::Meters;
```

### fn crate::shapes::metrics::diameter

Source: shapes\/metrics\.ox:6:8

```oxid
pub fn diameter(c: &Round) -> i32;
```

> Diameter of \`c\`\.

### fn crate::shapes::metrics::span

Source: shapes\/metrics\.ox:10:8

```oxid
pub fn span(m: Meters) -> Meters;
```

### fn crate::shapes::metrics::scaled

Source: shapes\/metrics\.ox:15:8

```oxid
pub fn scaled(lengths: &mut [i32; 2], by: i32) -> ();
```

> Scales the first length\.

## Module crate::shapes::draft

Source: shapes\/draft\.ox

> Draft API with no public items yet\.

## Module crate::notes

Source: notes\.ox

> Comment attachment boundaries\.
"##,
    "> \n",
    r##"> Each function below shows one rule\.

### fn crate::notes::plain

Source: notes\.ox:2:8

```oxid
pub fn plain() -> i32;
```

### fn crate::notes::after\_blank

Source: notes\.ox:8:8

```oxid
pub fn after_blank() -> i32;
```

### fn crate::notes::after\_ordinary

Source: notes\.ox:14:8

```oxid
pub fn after_ordinary() -> i32;
```

### fn crate::notes::four\_slashes

Source: notes\.ox:19:8

```oxid
pub fn four_slashes() -> i32;
```

### fn crate::notes::four\_breaks

Source: notes\.ox:25:8

```oxid
pub fn four_breaks() -> i32;
```

### fn crate::notes::code\_before

Source: notes\.ox:30:46

```oxid
pub fn code_before() -> i32;
```

### fn crate::notes::after\_block\_token

Source: notes\.ox:33:20

```oxid
pub fn after_block_token() -> i32;
```

### fn crate::notes::inside\_block

Source: notes\.ox:39:8

```oxid
pub fn inside_block() -> i32;
```

### fn crate::notes::after\_byte\_literal

Source: notes\.ox:44:8

```oxid
pub fn after_byte_literal() -> i32;
```

### fn crate::notes::indented

Source: notes\.ox:53:12

```oxid
pub fn indented() -> i32;
```

> Indented documentation is allowed\.
> No space after the slashes\.
>  Two spaces\: one is removed\.
"##,
    "> \n",
    r##">     Indented content stays content\.

### fn crate::notes::escaped

Source: notes\.ox:62:8

```oxid
pub fn escaped() -> i32;
```

> Escaped\: \\ \! \" \# \$ \% \& \' \( \) \* \+ \, \- \. \/ \: \; \< \= \> \? \@ \[ \] \^ \_ \` \{ \| \} \~
> HTML \<script\>alert\(1\)\<\/script\> and \<\!\-\- comment \-\-\> stay text\.
> Links \[x\]\(javascript\:alert\) \<https\:\/\/example\.com\> and \`\`\`fences\`\`\` stay text\.
> Not markup\: \# heading\, \> quote\, \- item\, 1\. item\, \| table \|\, \*em\* \_\_strong\_\_
> Literal \[U\+0007\] is text\, not a control\.

### fn crate::notes::unicode

Source: notes\.ox:67:8

```oxid
pub fn unicode() -> i32;
```

> Unicode stays\: café\, 日本語\, Ελληνικά\, 🦀\, e"##,
    "\u{301}",
    r##" \(combining\)\.

### fn crate::notes::wide\_column

Source: notes\.ox:71:18

```oxid
pub fn wide_column() -> i32;
```

### fn crate::notes::tabbed

Source: notes\.ox:75:8

```oxid
pub fn tabbed() -> i32;
```

## Module crate::name\_rules

Source: name\_rules\.ox

> Name and alias rules\.

### struct crate::name\_rules::Local

Source: name\_rules\.ox:6:12

```oxid
pub struct Local {
    pub seed: i32,
}
```

> A local record whose name is also a function alias\.

### fn crate::name\_rules::parse\_value

Source: name\_rules\.ox:11:8

```oxid
pub fn parse_value(seed: i32) -> Local;
```

> Returns the local \`Local\` record\, not \`units\:\:origin\`\.

### fn crate::name\_rules::plain\_count

Source: name\_rules\.ox:17:8

```oxid
pub fn plain_count(n: i32) -> i32;
```

> An \`i32\`\-only signature lists no import\.

## Module crate::name\_rules::alias\_pairs

Source: name\_rules\/alias\_pairs\.ox

> Type and value aliases sharing one spelling\.

### Signature imports

```oxid
use units::Meters as Span;
```

### fn crate::name\_rules::alias\_pairs::widen

Source: name\_rules\/alias\_pairs\.ox:5:8

```oxid
pub fn widen(m: Span) -> Span;
```

> Only \`Span\` the type needs a spelling guide\.

"##
);

/// `oxid doc --manifest-path oxid.toml --package api-docs-app`: only the
/// app's own public API; geometry's same-named `area` is not documented and
/// the private `main` is neither documented nor run.
const APP_DOC: &str = r##"# Typed API: api\-docs\-app

Edition: typed-preview (experimental).

The selected package was checked; dependencies are checked but not documented.
No code was executed. Source paths are relative to the selected entry directory.

## Module crate

Source: main\.ox

### Signature imports

```oxid
use geometry::Point;
```

### fn crate::doubled

Source: main\.ox:15:8

```oxid
pub fn doubled(x: i32) -> Point;
```

> Doubles a point built from \`x\`\.

### fn crate::area\_of

Source: main\.ox:22:8

```oxid
pub fn area_of(radius: i32) -> i32;
```

> Same spelling as \`geometry\:\:shapes\:\:area\`\.

"##;

/// `oxid doc --manifest-path geometry/vendor/units/oxid.toml`: the unnamed
/// dependency documented on its own, with its private `hidden` omitted. NAME
/// `(unnamed)` is metadata, so its parentheses are escaped like any NAME.
const UNITS_DOC: &str = r##"# Typed API: \(unnamed\)

Edition: typed-preview (experimental).

The selected package was checked; dependencies are checked but not documented.
No code was executed. Source paths are relative to the selected entry directory.

## Module crate

Source: lib\.ox

### struct crate::Point

Source: lib\.ox:2:12

```oxid
pub struct Point {
    pub x: i32,
    pub y: i32,
}
```

> Units point\: same spelling as the geometry root \`Point\`\.

### struct crate::Meters

Source: lib\.ox:7:12

```oxid
pub struct Meters {
    pub value: i32,
}
```

### enum crate::Axis

Source: lib\.ox:11:10

```oxid
pub enum Axis {
    Horizontal,
    Vertical,
}
```

### fn crate::origin

Source: lib\.ox:16:8

```oxid
pub fn origin() -> Point;
```

### fn crate::meters

Source: lib\.ox:20:8

```oxid
pub fn meters(value: i32) -> Meters;
```

### fn crate::area

Source: lib\.ox:25:8

```oxid
pub fn area(p: &Point) -> i32;
```

> Same spelling as \`geometry\:\:shapes\:\:area\`\.

"##;

/// A direct project whose root has no public item: the root module section is
/// still present. The private `main` would print 3 if it were run.
const PRIVATE_ROOT_SOURCE: &str = "fn main() -> i32 {\n    return 3;\n}\n";
const PRIVATE_ROOT_DOC: &str = r##"# Typed API: \(unnamed\)

Edition: typed-preview (experimental).

The selected package was checked; dependencies are checked but not documented.
No code was executed. Source paths are relative to the selected entry directory.

## Module crate

Source: main\.ox

"##;

/// Metadata and comment control characters (C0, DEL, C1, tab and a lone CR
/// that is not part of a CRLF terminator) render as the escaped notation
/// `[U+XXXX]`; like all prose/metadata ASCII punctuation, its `[`, `+` and `]`
/// are backslash-escaped (`\[U\+0007\]`). The manifest name and entry
/// basename are escaped metadata.
const CONTROL_MANIFEST: &str = "[project]\nname = \"doc*[name]_<x>\\u0007\"\n\
    edition = \"typed-preview\"\nentry = \"my-entry.ox\"\n";
const CONTROL_SOURCE: &str = "/// nul\u{0} bell\u{7} esc\u{1b}[31m \
    del\u{7f} nel\u{85} tab\t cr\r mid\npub fn controls() -> i32 {\n    return 0;\n}\n";
const CONTROL_DOC: &str = r##"# Typed API: doc\*\[name\]\_\<x\>\[U\+0007\]

Edition: typed-preview (experimental).

The selected package was checked; dependencies are checked but not documented.
No code was executed. Source paths are relative to the selected entry directory.

## Module crate

Source: my\-entry\.ox

### fn crate::controls

Source: my\-entry\.ox:2:8

```oxid
pub fn controls() -> i32;
```

> nul\[U\+0000\] bell\[U\+0007\] esc\[U\+001B\]\[31m del\[U\+007F\] nel\[U\+0085\] tab\[U\+0009\] cr\[U\+000D\] mid

"##;

/// Signature-import lanes in a direct library project (no `main`), independent
/// of the example workspace: a value-only import aliased `i32` beside an
/// `i32`-only signature is not listed; an explicit same-name type alias
/// `as Mode` stays explicit; a std type import used by a displayed signature
/// is listed; listed imports keep source order (`std` before `crate`). The
/// `__kinds__` module and `__make__` function are escaped in headings and
/// metadata but verbatim inside `oxid` code blocks.
const LANES_MANIFEST: &str =
    "[project]\nname = \"alias-lanes\"\nedition = \"typed-preview\"\nentry = \"main.ox\"\n";
const LANES_ROOT: &str = r#"pub mod __kinds__;
use std::io::ReadStatus;
use crate::__kinds__::__make__ as i32;
use crate::__kinds__::Mode as Mode;

/// A value import aliased `i32` is not a spelling guide.
pub fn count(n: i32) -> i32 {
    return n;
}

/// An explicit same-name alias stays explicit.
pub fn pick(m: Mode) -> i32 {
    match m {
        Mode::On => { return 1; },
        Mode::Off => { return 0; },
    }
}

/// A std type import is a spelling guide.
pub fn status(s: ReadStatus) -> i32 {
    match s {
        ReadStatus::Eof(n) => { return n; },
        ReadStatus::Full => { return -1; },
        ReadStatus::IoError => { return -2; },
    }
}
"#;
const LANES_KINDS: &str = r#"pub fn __make__() -> i32 {
    return 1;
}

pub enum Mode {
    On,
    Off,
}
"#;
const LANES_DOC: &str = r##"# Typed API: alias\-lanes

Edition: typed-preview (experimental).

The selected package was checked; dependencies are checked but not documented.
No code was executed. Source paths are relative to the selected entry directory.

## Module crate

Source: main\.ox

### Signature imports

```oxid
use std::io::ReadStatus;
use crate::__kinds__::Mode as Mode;
```

### fn crate::count

Source: main\.ox:7:8

```oxid
pub fn count(n: i32) -> i32;
```

> A value import aliased \`i32\` is not a spelling guide\.

### fn crate::pick

Source: main\.ox:12:8

```oxid
pub fn pick(m: Mode) -> i32;
```

> An explicit same\-name alias stays explicit\.

### fn crate::status

Source: main\.ox:20:8

```oxid
pub fn status(s: ReadStatus) -> i32;
```

> A std type import is a spelling guide\.

## Module crate::\_\_kinds\_\_

Source: \_\_kinds\_\_\.ox

### fn crate::\_\_kinds\_\_::\_\_make\_\_

Source: \_\_kinds\_\_\.ox:1:8

```oxid
pub fn __make__() -> i32;
```

### enum crate::\_\_kinds\_\_::Mode

Source: \_\_kinds\_\_\.ox:5:10

```oxid
pub enum Mode {
    On,
    Off,
}
```

"##;

/// `root/ws` is the working directory (a copy of the example or empty),
/// `root/tools` holds recording LLVM stubs that log to `root/probe.log`, and
/// `root/stdin.bin` is the caller-owned stdin. Snapshots cover all of `root`.
struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn empty() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-project-doc-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("ws")).unwrap();
        fs::create_dir(root.join("tools")).unwrap();
        fs::write(root.join("stdin.bin"), STDIN).unwrap();
        let fixture = Self { root };
        fixture.recording_tools();
        fixture
    }
    /// A byte-for-byte copy of the example workspace with relative paths kept.
    fn example() -> Self {
        let fixture = Self::empty();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join(EXAMPLE);
        for path in FILES {
            fixture.write(path, &fs::read(repository.join(path)).unwrap());
        }
        fixture
    }
    fn recording_tools(&self) {
        use std::os::unix::fs::PermissionsExt;
        let log = self.root.join("probe.log");
        for name in ["clang", "opt", "ld.lld", "llc"] {
            let path = self.root.join("tools").join(name);
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\nprintf '%s\\n' \"$0 $*\" >> '{}'\nexit 3\n",
                    log.display()
                ),
            )
            .unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    fn ws(&self) -> PathBuf {
        self.root.join("ws")
    }
    fn write(&self, path: &str, bytes: &[u8]) {
        let path = self.ws().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    fn read(&self, path: &str) -> Vec<u8> {
        fs::read(self.ws().join(path)).unwrap()
    }
    fn append(&self, path: &str, text: &str) {
        let mut bytes = self.read(path);
        bytes.extend_from_slice(text.as_bytes());
        self.write(path, &bytes);
    }
    fn probed(&self) -> bool {
        self.root.join("probe.log").exists()
    }
    /// Every path under `root` and its bytes (`None` for directories).
    fn snapshot(&self) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
        fn visit(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let relative = entry.path().strip_prefix(root).unwrap().to_path_buf();
                if entry.file_type().unwrap().is_dir() {
                    out.insert(relative, None);
                    visit(root, &entry.path(), out);
                } else {
                    out.insert(relative, Some(fs::read(entry.path()).unwrap()));
                }
            }
        }
        let mut out = BTreeMap::new();
        visit(&self.root, &self.root, &mut out);
        out
    }
    /// Runs Oxid in `cwd` with the given stdout. Stdin is a regular file whose
    /// open file description is shared with the child, so any read by the
    /// child would move the offset observed here.
    fn run_with(&self, cwd: &Path, args: &[&str], stdout: Stdio) -> Output {
        let mut stdin = File::open(self.root.join("stdin.bin")).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(cwd)
            .env("OXID_LLVM_BIN", self.root.join("tools"))
            .env("PATH", self.root.join("tools"))
            .env("OXID_CACHE_DIR", self.root.join("cache"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::from(stdin.try_clone().unwrap()))
            .stdout(stdout)
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        assert_eq!(stdin.stream_position().unwrap(), 0, "stdin read: {args:?}");
        output
    }
    fn run_in(&self, cwd: &Path, args: &[&str]) -> Output {
        self.run_with(cwd, args, Stdio::piped())
    }
    fn oxid(&self, args: &[&str]) -> Output {
        self.run_in(&self.ws(), args)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// A successful document: status 0, empty stderr, exact stdout bytes.
fn assert_document(label: &str, output: &Output, expected: &str) {
    assert_eq!(output.status.code(), Some(0), "{label}: {output:?}");
    assert!(
        output.stderr.is_empty(),
        "{label}: {}",
        text(&output.stderr)
    );
    assert_eq!(text(&output.stdout), expected, "{label}");
    assert_eq!(output.stdout, expected.as_bytes(), "{label}");
}

/// A doc failure: status 1, no document bytes, one text (not JSON) diagnostic
/// stream on stderr. Returns stderr.
fn assert_refused(label: &str, output: &Output) -> String {
    assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
    assert!(
        output.stdout.is_empty(),
        "{label}: {}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(stderr.starts_with("error["), "{label}: {stderr}");
    assert!(!stderr.contains("\"code\""), "{label}: {stderr}");
    assert!(!stderr.contains("# Typed API"), "{label}: {stderr}");
    stderr
}

/// `error[CODE] (stage)` of the first diagnostic line.
fn classification(stderr: &str) -> &str {
    let line = stderr.lines().next().unwrap_or_default();
    &line[..line.find("): ").map_or(line.len(), |end| end + 1)]
}

fn geometry(extra: &[&'static str]) -> Vec<&'static str> {
    let mut args = vec![
        "doc",
        "--manifest-path=oxid.toml",
        "--package=api-docs-geometry",
    ];
    args.extend_from_slice(extra);
    args
}

/// The selected doc failure and manifest `check` with the same selector agree
/// on code and stage; `expected` pins it independently when known.
fn assert_check_parity(
    f: &Fixture,
    label: &str,
    selector: &[&str],
    expected: Option<&str>,
) -> String {
    let mut doc = vec!["doc"];
    doc.extend_from_slice(selector);
    let before = f.snapshot();
    let refused = assert_refused(label, &f.oxid(&doc));
    assert_eq!(f.snapshot(), before, "{label}: doc changed files");
    let mut check = vec!["check"];
    check.extend_from_slice(selector);
    let checked = f.oxid(&check);
    assert_eq!(checked.status.code(), Some(1), "{label}: {checked:?}");
    let checked = text(&checked.stderr);
    assert_eq!(
        classification(&refused),
        classification(&checked),
        "{label}"
    );
    if let Some(expected) = expected {
        assert_eq!(classification(&refused), expected, "{label}: {refused}");
    }
    assert!(!f.probed(), "{label}: toolchain invoked");
    refused
}

#[test]
fn geometry_document_is_exact_for_every_flag_spelling_and_global_placement() {
    let f = Fixture::example();
    let before = f.snapshot();
    let spellings: &[&[&str]] = &[
        &["doc", "--manifest-path", "oxid.toml", "--package", GEOMETRY],
        &[
            "doc",
            "--manifest-path=oxid.toml",
            "--package=api-docs-geometry",
        ],
        &["doc", "--package", GEOMETRY, "--manifest-path=oxid.toml"],
        &[
            "doc",
            "--package=api-docs-geometry",
            "--manifest-path",
            "oxid.toml",
        ],
        &[
            "--edition",
            "typed-preview",
            "doc",
            "--manifest-path=oxid.toml",
            "--package",
            GEOMETRY,
        ],
        &[
            "--edition=typed-preview",
            "--message-format=text",
            "doc",
            "--manifest-path",
            "oxid.toml",
            "--package=api-docs-geometry",
        ],
        &[
            "doc",
            "--manifest-path=oxid.toml",
            "--package",
            GEOMETRY,
            "--edition",
            "typed-preview",
            "--message-format",
            "text",
        ],
        &[
            "--message-format",
            "text",
            "doc",
            "--package=api-docs-geometry",
            "--edition=typed-preview",
            "--manifest-path=oxid.toml",
        ],
        &[
            "doc",
            "--edition=typed-preview",
            "--manifest-path=oxid.toml",
            "--message-format=text",
            "--package",
            GEOMETRY,
        ],
    ];
    for args in spellings {
        assert_document(&format!("{args:?}"), &f.oxid(args), GEOMETRY_DOC);
    }
    // Repeated runs are byte-identical.
    let first = f.oxid(&geometry(&[]));
    for _ in 0..2 {
        assert_eq!(f.oxid(&geometry(&[])).stdout, first.stdout);
    }
    assert_document("repeat", &first, GEOMETRY_DOC);
    assert!(!f.probed(), "documentation invoked the toolchain");
    assert_eq!(f.snapshot(), before, "documentation changed files");
    assert!(!f.ws().join("docs").exists() && !f.root.join("cache").exists());
}

#[test]
fn direct_and_absolute_manifests_match_workspace_selection() {
    let f = Fixture::example();
    let before = f.snapshot();
    for args in [
        &["doc", "--manifest-path=geometry/oxid.toml"][..],
        &["doc", "--manifest-path", "geometry/oxid.toml"],
    ] {
        assert_document("direct", &f.oxid(args), GEOMETRY_DOC);
    }
    // From an unrelated working directory with absolute selectors, no host
    // path leaks into the document.
    let canonical = fs::canonicalize(f.ws()).unwrap();
    let workspace = canonical.join("oxid.toml");
    let member = canonical.join("geometry/oxid.toml");
    let selected = format!("--manifest-path={}", workspace.display());
    let elsewhere = f.root.join("tools");
    for args in [
        &["doc", selected.as_str(), "--package", GEOMETRY][..],
        &["doc", "--manifest-path", member.to_str().unwrap()],
    ] {
        let output = f.run_in(&elsewhere, args);
        assert_document("absolute", &output, GEOMETRY_DOC);
        let document = text(&output.stdout);
        assert!(!document.contains(f.root.to_str().unwrap()));
        assert!(!document.contains(canonical.to_str().unwrap()));
    }
    assert!(!f.probed());
    assert_eq!(f.snapshot(), before);
}

#[test]
fn app_member_documents_only_its_api_and_never_runs_main() {
    let f = Fixture::example();
    let before = f.snapshot();
    for args in [
        &["doc", "--manifest-path=oxid.toml", "--package=api-docs-app"][..],
        &["doc", "--package", APP, "--manifest-path", "oxid.toml"],
        &["doc", "--manifest-path=app/oxid.toml"],
    ] {
        assert_document("app", &f.oxid(args), APP_DOC);
    }
    assert!(!f.probed());
    assert_eq!(f.snapshot(), before);
}

#[test]
fn unnamed_dependency_documented_standalone() {
    let f = Fixture::example();
    let before = f.snapshot();
    let output = f.oxid(&["doc", "--manifest-path=geometry/vendor/units/oxid.toml"]);
    assert_document("units", &output, UNITS_DOC);
    assert!(!f.probed());
    assert_eq!(f.snapshot(), before);
}

#[test]
fn crlf_sources_render_the_same_documents() {
    let f = Fixture::example();
    for path in FILES.iter().filter(|path| path.ends_with(".ox")) {
        let source = f.read(path);
        assert!(!source.contains(&b'\r'), "{path} already has CR");
        let crlf = text(&source).replace('\n', "\r\n");
        f.write(path, crlf.as_bytes());
    }
    assert!(f
        .read("geometry/src/notes.ox")
        .windows(2)
        .any(|w| w == b"\r\n"));
    let before = f.snapshot();
    assert_document("crlf geometry", &f.oxid(&geometry(&[])), GEOMETRY_DOC);
    let app = f.oxid(&["doc", "--manifest-path=oxid.toml", "--package", APP]);
    assert_document("crlf app", &app, APP_DOC);
    let units = f.oxid(&["doc", "--manifest-path=geometry/vendor/units/oxid.toml"]);
    assert_document("crlf units", &units, UNITS_DOC);
    assert!(!f.probed());
    assert_eq!(f.snapshot(), before);
}

#[test]
fn private_root_still_has_its_module_section_and_main_is_not_run() {
    let f = Fixture::empty();
    f.write("main.ox", PRIVATE_ROOT_SOURCE.as_bytes());
    for manifest in [
        "[project]\nedition = \"typed-preview\"\nentry = \"main.ox\"\n",
        "[project]\nname = \"\"\nedition = \"typed-preview\"\nentry = \"main.ox\"\n",
    ] {
        f.write("oxid.toml", manifest.as_bytes());
        let before = f.snapshot();
        let output = f.oxid(&["doc", "--manifest-path=oxid.toml"]);
        assert_document(manifest, &output, PRIVATE_ROOT_DOC);
        assert_eq!(f.snapshot(), before);
    }
    assert!(!f.probed());
}

#[test]
fn control_characters_and_metadata_punctuation_are_escaped() {
    let f = Fixture::empty();
    f.write("oxid.toml", CONTROL_MANIFEST.as_bytes());
    f.write("my-entry.ox", CONTROL_SOURCE.as_bytes());
    let before = f.snapshot();
    let output = f.oxid(&["doc", "--manifest-path=oxid.toml"]);
    assert_document("controls", &output, CONTROL_DOC);
    let document = text(&output.stdout);
    assert!(
        !document.chars().any(|c| c.is_control() && c != '\n'),
        "raw control in {document:?}"
    );
    assert!(!f.probed());
    assert_eq!(f.snapshot(), before);
}

#[test]
fn signature_imports_follow_type_lanes_in_a_direct_library() {
    let f = Fixture::empty();
    f.write("oxid.toml", LANES_MANIFEST.as_bytes());
    f.write("main.ox", LANES_ROOT.as_bytes());
    f.write("__kinds__.ox", LANES_KINDS.as_bytes());
    let before = f.snapshot();
    let output = f.oxid(&["doc", "--manifest-path=oxid.toml"]);
    assert_document("lanes", &output, LANES_DOC);
    let document = text(&output.stdout);
    assert!(!document.contains("as i32"), "{document}");
    assert!(!document.contains("__make__ as"), "{document}");
    assert!(!f.probed());
    assert_eq!(f.snapshot(), before);
}

#[test]
fn check_failures_precede_any_document() {
    // (file, appended source, code/stage); each starts from a fresh copy.
    let cases: &[(&str, &str, &str)] = &[
        (
            "geometry/src/notes.ox",
            "pub fn broken( {\n",
            "error[E0100] (parse)",
        ),
        (
            "geometry/src/notes.ox",
            "/* never closed\n",
            "error[E0100] (lex)",
        ),
        // A `///`-looking line inside a byte literal is never documentation:
        // the multiline literal is an existing parse error.
        (
            "geometry/src/notes.ox",
            "fn fake() -> [u8; 1] { return b\"\n/// fake\n\"; }\n\
             pub fn after_fake() -> i32 { return 0; }\n",
            "error[E0101] (parse)",
        ),
        (
            "geometry/src/shapes.ox",
            "pub fn lost() -> i32 { return missing(); }\n",
            "error[E0200] (resolve)",
        ),
        (
            "geometry/src/lib.ox",
            "use units::hidden;\n",
            "error[E0206] (resolve)",
        ),
        (
            "geometry/src/shapes/metrics.ox",
            "pub fn wide(x: i64) -> i32 { return 0; }\n",
            "error[E0202] (resolve)",
        ),
        (
            "geometry/src/notes.ox",
            "struct Secret {}\npub fn reveal() -> Secret { return Secret {}; }\n",
            "error[E0207] (resolve)",
        ),
        (
            "geometry/src/shapes/draft.ox",
            "fn wrong() -> i32 { return true; }\n",
            "error[E0300] (type)",
        ),
        // Dependency bodies are checked even though they are not documented.
        (
            "geometry/vendor/units/lib.ox",
            "fn unused() -> i32 { return true; }\n",
            "error[E0300] (type)",
        ),
        (
            "geometry/src/lib.ox",
            "pub mod absent;\n",
            "error[E0002] (source)",
        ),
        // A private module edge blocks an import from the selected app.
        (
            "app/src/main.ox",
            "use geometry::internal::internal_api;\n",
            "error[E0206] (resolve)",
        ),
    ];
    for (path, source, expected) in cases {
        let f = Fixture::example();
        f.append(path, source);
        let selector: &[&str] = if path.starts_with("app/") {
            &["--manifest-path=oxid.toml", "--package=api-docs-app"]
        } else {
            &["--manifest-path=oxid.toml", "--package=api-docs-geometry"]
        };
        let label = format!("{path}: {source:?}");
        assert_check_parity(&f, &label, selector, Some(expected));
    }
}

#[test]
fn manifest_failures_keep_existing_classification() {
    let f = Fixture::example();
    let workspace = ["--manifest-path=oxid.toml", "--package", GEOMETRY];
    let rejected = assert_check_parity(&f, "no package", &workspace[..1], None);
    assert!(
        rejected.contains("workspace requires --package"),
        "{rejected}"
    );
    assert!(
        rejected.contains(GEOMETRY) && rejected.contains(APP),
        "{rejected}"
    );
    let unknown = ["--manifest-path=oxid.toml", "--package=absent"];
    assert_check_parity(&f, "unknown package", &unknown, None);
    let direct = assert_check_parity(
        &f,
        "direct --package",
        &["--manifest-path=geometry/oxid.toml", "--package", GEOMETRY],
        Some("error[E0001] (cli)"),
    );
    assert!(direct.contains("--package requires a virtual workspace manifest"));
    let missing = assert_check_parity(&f, "missing", &["--manifest-path=absent.toml"], None);
    assert!(missing.starts_with("error[E0002]"), "{missing}");
    assert!(missing.contains("cannot read manifest"), "{missing}");
    let units = "geometry/vendor/units/oxid.toml";
    for manifest in [
        "[project]\nedition = \"legacy-0.9\"\nentry = \"lib.ox\"\n",
        "[project]\nedition = \"typed-preview\"\n",
        "[project]\nedition = \"typed-preview\"\nentry = \"lib.ox\"\n\
         [scripts]\nprobe = \"touch forbidden\"\n",
    ] {
        f.write(units, manifest.as_bytes());
        assert_check_parity(&f, manifest, &workspace, None);
    }
    fs::remove_dir_all(f.ws().join("geometry/vendor")).unwrap();
    assert_check_parity(&f, "absent dependency", &workspace, None);
}

#[test]
fn cli_rejections_are_e0001_without_effects() {
    let f = Fixture::example();
    let before = f.snapshot();
    let mut rejected: Vec<Vec<&str>> = [
        &["--message-format=json"][..],
        &["--message-format", "json"],
        &["--message-format=bad"],
        &["--edition=legacy-0.9"],
        &["--edition", "legacy-0.9"],
        &["--edition=bogus"],
        &["--profile=dev"],
        &["--profile", "release"],
        &["--entry-mode=result"],
        &["--engine=interpreter"],
        &["--engine", "native"],
        &["--backend=llvm"],
        &["--target=x86_64-unknown-linux-gnu"],
        &["--output=api.md"],
        &["-o", "api.md"],
        &["--native"],
        &["--filter=area"],
        &["--experimental-hir-import=x.json"],
        &["--experimental-hir-producers=x"],
        &["--experimental-lexical-provider=x"],
        &["src/lib.ox"],
        &["geometry"],
        &["--manifest-path=oxid.toml"],
        &["--package", APP],
        &["--edition=typed-preview", "--edition", "typed-preview"],
        &["--message-format=text", "--message-format", "text"],
        &["--", "extra"],
        &["--", "--package=api-docs-app"],
        &["--", "--manifest-path=oxid.toml"],
    ]
    .iter()
    .map(|extra| geometry(extra))
    .collect();
    rejected.extend([
        vec![
            "--edition=legacy-0.9",
            "doc",
            "--manifest-path=oxid.toml",
            "--package",
            GEOMETRY,
        ],
        vec![
            "--message-format=json",
            "doc",
            "--manifest-path=oxid.toml",
            "--package",
            GEOMETRY,
        ],
        vec!["--manifest-path=oxid.toml", "doc", "--package", GEOMETRY],
        vec!["--manifest-path", "oxid.toml", "doc"],
        vec!["doc", "--manifest-path=", "--package", GEOMETRY],
        vec!["doc", "--manifest-path=oxid.toml", "--package="],
        vec!["doc", "--package", GEOMETRY, "--manifest-path"],
    ]);
    for args in &rejected {
        let label = format!("{args:?}");
        let stderr = assert_refused(&label, &f.oxid(args));
        assert_eq!(
            classification(&stderr),
            "error[E0001] (cli)",
            "{label}: {stderr}"
        );
    }
    assert!(!f.probed());
    assert_eq!(f.snapshot(), before, "a rejected request changed files");
}

#[test]
fn explicit_typed_doc_without_selector_stays_unsupported() {
    let f = Fixture::example();
    let before = f.snapshot();
    for args in [
        &["doc", "--edition", "typed-preview"][..],
        &["--edition=typed-preview", "doc"],
        &["doc", "--edition=typed-preview", "--package", GEOMETRY],
        &[
            "--edition=typed-preview",
            "doc",
            "--",
            "--manifest-path=oxid.toml",
        ],
    ] {
        let label = format!("{args:?}");
        let stderr = assert_refused(&label, &f.oxid(args));
        assert_eq!(
            classification(&stderr),
            "error[E0001] (cli)",
            "{label}: {stderr}"
        );
        assert!(stderr.contains("`doc`"), "{label}: {stderr}");
    }
    assert!(!f.probed());
    assert_eq!(f.snapshot(), before);
}

#[test]
fn legacy_doc_is_preserved_in_disposable_directories() {
    let legacy: &[&[&str]] = &[
        &["doc"],
        &["--edition", "legacy-0.9", "doc"],
        &["doc", "--edition=legacy-0.9"],
        &["doc", "--", "--manifest-path=oxid.toml"],
    ];
    let mut reference: Option<Vec<u8>> = None;
    for args in legacy {
        let f = Fixture::empty();
        let output = f.oxid(args);
        assert_eq!(output.status.code(), Some(0), "{args:?}: {output:?}");
        assert!(
            output.stdout.is_empty() && output.stderr.is_empty(),
            "{output:?}"
        );
        let api = f.read("docs/API.md");
        assert!(api.starts_with(b"# Oxid API\n\n## Built-ins\n"), "{args:?}");
        assert_eq!(
            reference.get_or_insert_with(|| api.clone()),
            &api,
            "{args:?}"
        );
        assert!(!f.probed());
    }
    // A typed workspace in the working directory is not discovered implicitly.
    let f = Fixture::example();
    let mut expected = f.snapshot();
    let output = f.oxid(&["doc"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        output.stdout.is_empty() && output.stderr.is_empty(),
        "{output:?}"
    );
    expected.insert(PathBuf::from("ws/docs"), None);
    expected.insert(PathBuf::from("ws/docs/API.md"), reference);
    assert_eq!(f.snapshot(), expected);
    assert!(!f.probed());
}

#[test]
fn output_writer_failure_is_e0002_project_doc() {
    let f = Fixture::example();
    let before = f.snapshot();
    let app = vec!["doc", "--manifest-path", "oxid.toml", "--package", APP];
    for args in [geometry(&[]), app] {
        let full = fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .unwrap();
        let output = f.run_with(&f.ws(), &args, Stdio::from(full));
        assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
        let stderr = text(&output.stderr);
        assert!(
            stderr.contains("error[E0002] (project-doc)"),
            "{args:?}: {stderr}"
        );
        // The only reader of the pipe is closed before the child starts, so
        // every write fails with EPIPE regardless of scheduling.
        let (reader, writer) = std::io::pipe().unwrap();
        drop(reader);
        let output = f.run_with(&f.ws(), &args, Stdio::from(writer));
        assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
        let stderr = text(&output.stderr);
        assert!(
            stderr.contains("error[E0002] (project-doc)"),
            "{args:?}: {stderr}"
        );
    }
    assert!(!f.probed());
    assert_eq!(f.snapshot(), before);
}
