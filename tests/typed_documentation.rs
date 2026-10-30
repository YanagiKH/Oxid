//! API pages use the ordinary checked project/workspace and lockfile routes.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-api-docs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn library(&self, prefix: &str, name: &str, extra: &str, source: &str) {
        self.write(&format!("{prefix}oxid.toml"), &format!("[project]\nname=\"{name}\"\nversion=\"1.2.3\"\nedition=\"typed-preview\"\nkind=\"lib\"\n{extra}"));
        self.write(&format!("{prefix}src/lib.ox"), source);
    }
    fn at(&self, directory: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(self.0.join(directory))
            .env_remove("OXID_PATH")
            .env("OXID_CACHE_DIR", self.0.join("download-cache"))
            .env("OXID_LLVM_BIN", self.0.join("no-native-tools"))
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    fn run(&self, args: &[&str]) -> Output {
        self.at("", args)
    }
    fn page(&self) -> String {
        fs::read_to_string(self.0.join(".oxid/doc/index.html")).unwrap()
    }
    fn no_page(&self) {
        assert!(!self.0.join(".oxid").exists());
        assert!(!self.0.join("oxid.lock").exists());
        assert!(!self.0.join("download-cache").exists());
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(output: Output) -> Output {
    assert!(output.status.success(), "{output:?}");
    output
}
fn failure(output: Output) -> Output {
    assert!(!output.status.success(), "{output:?}");
    output
}
fn source_policy_failure(output: Output, expected: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let error = String::from_utf8(output.stderr).unwrap();
    assert_eq!(error.lines().next(), Some(expected), "{error}");
}

#[test]
fn public_api_comments_generics_and_private_boundaries_are_documented() {
    let p = Project::new();
    p.library(
        "",
        "reference",
        "",
        r#"//! Public & useful module documentation.
/// Mathematical helpers.
pub mod math;
mod hidden;
/// 說明 <script>alert('x')</script> & text.
/// A second line.
pub fn identity<T>(value: T) -> T { return value; }
/// A value with private state.
pub struct Cell<T: Copy> {
    /// Public value.
    pub value: T,
    credential: i32
}
/// Possible answers.
pub enum Answer<T> {
    /// Found a value.
    Found(T),
    Missing
}
fn not_public() -> i32 { return 123456; }
"#,
    );
    p.write(
        "src/math.ox",
        "/// Increment a value.\npub fn add(value: i32) -> i32 { return value + 987654; }\n",
    );
    p.write(
        "src/hidden.ox",
        "pub fn hidden_public() -> i32 { return 765432; }\n",
    );
    if !cfg!(any(
        target_os = "linux",
        target_os = "macos",
        all(
            target_os = "windows",
            target_arch = "x86_64",
            target_pointer_width = "64",
            target_endian = "little"
        )
    )) {
        source_policy_failure(
            p.run(&["doc"]),
            "error[E0001] (project): error[E0005] (source): module source policy is not qualified on this host",
        );
        p.no_page();
        return;
    }
    success(p.run(&["doc"]));
    let page = p.page();
    for text in [
        "reference 1.2.3",
        "pub fn identity&lt;T&gt;(value: T) -&gt; T;",
        "Cell&lt;T: Copy&gt;",
        "pub value: T",
        "Found(T)",
        "Public value.",
        "Found a value.",
        "crate::math",
        "pub fn add",
        "說明 &lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt; &amp; text.",
        "A second line.",
        "Public &amp; useful module documentation.",
        "Mathematical helpers.",
    ] {
        assert!(page.contains(text), "missing {text:?}: {page}");
    }
    for hidden in [
        "credential",
        "not_public",
        "hidden_public",
        "987654",
        "765432",
        "123456",
        "<script>",
    ] {
        assert!(!page.contains(hidden), "{hidden}: {page}");
    }
    assert!(!p.0.join(".oxid/bin").exists());
    assert!(!p.0.join("oxid.lock").exists());
}

#[test]
fn public_copy_records_keep_their_contract_after_formatting_without_exposing_private_fields() {
    let p = Project::new();
    p.library(
        "",
        "snapshots",
        "",
        r#"/// An ordinary owned value.
pub struct Affine { pub value: i32, affine_secret: bool }
/// Coordinates <&>.
# /* not public API <script> */ [ derive ( Copy ) ]
pub struct Point { pub x: i32, point_secret: u128 }
#[derive(Copy)]
/// Generic & reusable.
pub struct Holder<T: Copy> { pub value: T, holder_secret: i32 }
#[derive(Copy)]
pub struct Closed { closed_secret: Point }
#[derive(Copy)]
struct HiddenCopy { hidden_secret: i32 }
pub fn twice(point: Point) -> i32 {
    let second = point;
    return point.x + second.x;
}
"#,
    );
    let public_contract = |page: &str| {
        for declaration in [
            "<pre><code>pub struct Affine {\n",
            "<pre><code>#[derive(Copy)]\npub struct Point {\n",
            "<pre><code>#[derive(Copy)]\npub struct Holder&lt;T: Copy&gt; {\n",
            "<pre><code>#[derive(Copy)]\npub struct Closed {\n",
        ] {
            assert!(
                page.contains(declaration),
                "missing {declaration:?}: {page}"
            );
        }
        assert_eq!(page.matches("#[derive(Copy)]").count(), 3, "{page}");
        assert!(page.contains("snapshots 1.2.3"));
        assert!(page.contains("Coordinates &lt;&amp;&gt;."));
        assert!(page.contains("Generic &amp; reusable."));
        assert!(page.contains("// private fields"));
        for hidden in [
            "affine_secret",
            "point_secret",
            "holder_secret",
            "closed_secret",
            "HiddenCopy",
            "hidden_secret",
            "not public API",
            "<script>",
        ] {
            assert!(!page.contains(hidden), "{hidden}: {page}");
        }
    };
    success(p.run(&["doc"]));
    public_contract(&p.page());
    let formatted = success(p.run(&["fmt", "src/lib.ox"]));
    p.write(
        "src/lib.ox",
        std::str::from_utf8(&formatted.stdout).unwrap(),
    );
    success(p.run(&["doc"]));
    let page = p.page();
    public_contract(&page);
    p.write(
        "src/lib.ox",
        "use alloc::bytes::ByteBuf;\n#[derive(Copy)]\npub struct Invalid { pub values: ByteBuf }\n",
    );
    let invalid = failure(p.run(&["doc"]));
    assert!(
        String::from_utf8_lossy(&invalid.stderr).contains("Copy"),
        "{invalid:?}"
    );
    assert_eq!(p.page(), page);
    assert!(!p.0.join(".oxid/bin").exists());
    assert!(!p.0.join("oxid.lock").exists());
    assert!(!p.0.join("download-cache").exists());
}

#[test]
fn docs_check_dependencies_without_documenting_them_and_enforce_frozen_inputs() {
    let p = Project::new();
    p.library(
        "dep/",
        "dependency",
        "",
        "pub fn dependency_value() -> i32 { return 3; }\n",
    );
    p.library("", "application", "[dependencies]\nmath=\"dep\"\n", "use crate::math::dependency_value;\npub fn public_value() -> i32 { return dependency_value(); }\n");
    failure(p.run(&["doc", "--frozen"]));
    p.no_page();
    if !cfg!(any(
        target_os = "linux",
        target_os = "macos",
        all(
            windows,
            target_arch = "x86_64",
            target_pointer_width = "64",
            target_endian = "little"
        )
    )) {
        for command in ["lock", "doc"] {
            source_policy_failure(
                p.run(&[command]),
                "error[E0001] (project): error[E0006] (package): package source policy is not qualified on this host",
            );
            p.no_page();
        }
        return;
    }
    success(p.run(&["lock"]));
    let lock = fs::read(p.0.join("oxid.lock")).unwrap();
    success(p.run(&["doc", "--frozen"]));
    let original = p.page();
    assert!(original.contains("pub fn public_value"));
    assert!(!original.contains("pub fn dependency_value"));
    p.write(
        "dep/src/lib.ox",
        "pub fn dependency_value() -> i32 { return 4; }\n",
    );
    failure(p.run(&["doc", "--locked"]));
    assert_eq!(p.page(), original);
    assert_eq!(fs::read(p.0.join("oxid.lock")).unwrap(), lock);
    success(p.run(&["lock", "--update"]));
    success(p.run(&["doc", "--offline"]));
    p.write(
        "dep/src/lib.ox",
        "pub fn dependency_value() -> i32 { return true; }\n",
    );
    success(p.run(&["lock", "--update"]));
    failure(p.run(&["doc"]));
    assert_eq!(p.page(), original);
}

#[test]
fn workspace_selection_generates_one_page_and_later_errors_preserve_it() {
    let p = Project::new();
    p.write("oxid.toml", "[workspace]\nedition=\"typed-preview\"\nmembers=[\"one\",\"two\"]\ndefault-members=[\"one\"]\n");
    p.library("one/", "one", "", "pub fn first() -> i32 { return 1; }\n");
    p.library("two/", "two", "", "pub fn second() -> i32 { return 2; }\n");
    success(p.run(&["doc"]));
    assert!(p.page().contains("pub fn first"));
    assert!(!p.page().contains("pub fn second"));
    success(p.run(&["doc", "--package", "two"]));
    assert!(!p.page().contains("pub fn first"));
    assert!(p.page().contains("pub fn second"));
    let report = success(p.run(&["doc", "--workspace", "--message-format=json"]));
    let report = String::from_utf8(report.stdout).unwrap();
    assert!(
        report.contains("\"kind\":\"doc-summary\"") && report.contains("\"packages\":2"),
        "{report}"
    );
    let original = p.page();
    assert!(original.contains("pub fn first") && original.contains("pub fn second"));
    p.write(
        "two/src/lib.ox",
        "pub fn second() -> i32 { return true; }\n",
    );
    failure(p.run(&["doc", "--workspace"]));
    assert_eq!(p.page(), original);
    assert!(!p.0.join("one/.oxid").exists());
    assert!(!p.0.join("two/.oxid").exists());
}

#[test]
fn docs_never_run_the_program_or_overwrite_source_documentation() {
    let p = Project::new();
    p.write(
        "oxid.toml",
        "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n",
    );
    p.write(
        "src/main.ox",
        "pub fn value() -> i32 { return 1; }\nfn main() -> i32 { return 1 / 0; }\n",
    );
    p.write("docs/API.md", "source documentation must stay untouched");
    success(p.at("src", &["doc"]));
    assert!(p.page().contains("pub fn value"));
    assert_eq!(
        fs::read_to_string(p.0.join("docs/API.md")).unwrap(),
        "source documentation must stay untouched"
    );
    assert!(!p.0.join(".oxid/bin").exists());
    assert!(!p.0.join("download-cache").exists());
}

#[test]
fn invalid_options_and_source_errors_do_not_create_or_replace_pages() {
    let p = Project::new();
    p.library("", "library", "", "pub fn value() -> i32 { return 1; }\n");
    for args in [
        vec!["doc", "src/lib.ox"],
        vec!["doc", "--entry-mode=result"],
        vec!["doc", "--profile=release"],
        vec!["doc", "--update"],
        vec!["doc", "--workspace"],
        vec!["doc", "--unknown"],
    ] {
        failure(p.run(&args));
        p.no_page();
    }
    success(p.run(&["doc", "--edition=typed-preview"]));
    let original = p.page();
    p.write("src/lib.ox", "pub fn value() -> i32 { return false; }\n");
    failure(p.run(&["doc"]));
    assert_eq!(p.page(), original);
}

#[test]
fn identical_sources_produce_identical_html_after_relocation() {
    let first = Project::new();
    let second = Project::new();
    for p in [&first, &second] {
        p.library(
            "",
            "portable",
            "",
            "/// Portable & stable.\npub fn value() -> i32 { return 9; }\n",
        );
        success(p.run(&["doc"]));
    }
    assert_eq!(first.page(), second.page());
    assert!(!first.page().contains(first.0.to_str().unwrap()));
    assert!(!second.page().contains(second.0.to_str().unwrap()));
}

#[test]
fn nonregular_index_is_preserved() {
    let p = Project::new();
    p.library("", "library", "", "pub fn value() -> i32 { return 1; }\n");
    p.write(".oxid/doc/index.html/keep", "untouched");
    failure(p.run(&["doc"]));
    assert_eq!(
        fs::read_to_string(p.0.join(".oxid/doc/index.html/keep")).unwrap(),
        "untouched"
    );
    assert_eq!(fs::read_dir(p.0.join(".oxid/doc")).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn symlink_output_directories_and_indexes_are_preserved() {
    use std::os::unix::fs::symlink;
    for location in [".oxid", ".oxid/doc", ".oxid/doc/index.html"] {
        let p = Project::new();
        p.library("", "library", "", "pub fn value() -> i32 { return 1; }\n");
        p.write("outside/keep", "untouched");
        let link = p.0.join(location);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(
            p.0.join(if location.ends_with("html") {
                "outside/keep"
            } else {
                "outside"
            }),
            &link,
        )
        .unwrap();
        failure(p.run(&["doc"]));
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_to_string(p.0.join("outside/keep")).unwrap(),
            "untouched"
        );
        assert_eq!(fs::read_dir(p.0.join("outside")).unwrap().count(), 1);
    }
}
