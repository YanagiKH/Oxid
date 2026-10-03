use super::*;
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

static SERIAL: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-unit2-parser-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    #[allow(clippy::result_large_err)]
    fn load(&self) -> Result<ProjectSources, LoadFailure> {
        ProjectSources::load_project_candidate(
            self.0.join("app.ox").to_str().unwrap(),
            ProjectLimits::default(),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn single_file_private_syntax_selects_project_flavor_without_filesystem_policy() {
    for text in [
        "pub fn main()->i32{return 0;}",
        "use crate::f; fn f()->(){return;}",
        "fn main()->(){crate::f();return;} fn f()->(){return;}",
        "struct R {pub n:i32}",
    ] {
        let f = Fixture::new();
        f.write("app.ox", text);
        let p = f.load().unwrap();
        assert_eq!(p.syntax_flavor(), SyntaxFlavor::ProjectSyntax);
        assert!(p.original_file().is_none());
        assert_eq!(p.usage().modules, 1);
        assert_eq!(p.usage().probes, 0);
        assert!(p.canonical_root.is_none());
    }
}

#[test]
fn typed_loader_uses_successful_ast_flavor_with_no_single_file_host_gate() {
    let f = Fixture::new();
    let limits = ProjectLimits {
        path_bytes: 0,
        component_bytes: 0,
        relative_bytes: 0,
        probes: 0,
        directory_entries: 0,
        directory_name_units: 0,
        ..ProjectLimits::default()
    };
    for (text, expected) in [
        ("", SyntaxFlavor::OriginalSingleFile),
        (
            "fn main()->i32{return 1;}",
            SyntaxFlavor::OriginalSingleFile,
        ),
        ("struct R {n:i32}", SyntaxFlavor::OriginalSingleFile),
        ("pub fn main()->i32{return 1;}", SyntaxFlavor::ProjectSyntax),
        ("struct R {pub n:i32}", SyntaxFlavor::ProjectSyntax),
        (
            "use crate::f; fn f()->(){return;}",
            SyntaxFlavor::ProjectSyntax,
        ),
        (
            "fn f()->(){crate::f();return;}",
            SyntaxFlavor::ProjectSyntax,
        ),
    ] {
        f.write("app.ox", text);
        let project =
            ProjectSources::load_typed(f.0.join("app.ox").to_str().unwrap(), limits).unwrap();
        assert_eq!(project.syntax_flavor(), expected, "{text}");
        assert_eq!(
            project.original_file().is_some(),
            expected == SyntaxFlavor::OriginalSingleFile
        );
        assert_eq!(project.usage().modules, 1);
        assert_eq!(project.usage().probes, 0);
        assert!(project.canonical_root.is_none());
        let source = project.sources().get(SourceFileId(0));
        assert_eq!(source.text(), text);
        assert!(project
            .try_file_ast(SourceFileId(0))
            .unwrap()
            .belongs_to(source));
    }
}

#[test]
fn failed_typed_loader_retains_sources_and_does_not_discover_children() {
    let f = Fixture::new();
    for text in [
        "fn pub()->i32{return 0;} mod missing;",
        "mod missing; struct R {pub n:}",
        "mod missing; struct R {n:crate::T}",
    ] {
        f.write("app.ox", text);
        let failure = ProjectSources::load_typed(
            f.0.join("app.ox").to_str().unwrap(),
            ProjectLimits::default(),
        )
        .unwrap_err();
        assert!(failure.diagnostics.iter().all(|d| d.stage == "parse"));
        assert_eq!(failure.sources.files().len(), 1);
        assert_eq!(failure.sources.get(SourceFileId(0)).text(), text);
        assert_eq!(failure.usage.probes, 0);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn global_owned_selection_scans_even_unused_loaded_modules() {
    let f = Fixture::new();
    f.write(
        "app.ox",
        "pub mod child; use crate::child::f; fn main()->i32{return crate::child::f();}",
    );
    f.write("child.ox", "pub fn f()->i32{return 1;}");
    let p = f.load().unwrap();
    assert!(!p.uses_owned_syntax());
    assert_eq!(p.syntax_flavor(), SyntaxFlavor::ProjectSyntax);
    f.write(
        "child.ox",
        "pub fn f()->i32{return 1;} struct Unused {x:i32}",
    );
    assert!(f.load().unwrap().uses_owned_syntax());
    f.write("child.ox", "pub fn f()->i32{let x:Unknown=1;return 1;}");
    assert!(f.load().unwrap().uses_owned_syntax());
    f.write(
        "child.ox",
        "pub fn f()->i32{let x:crate::Unknown=1;return 1;}",
    );
    assert!(f.load().unwrap().uses_owned_syntax());
}

#[test]
fn invalid_project_root_never_loads_its_declared_children() {
    let f = Fixture::new();
    f.write("app.ox", "mod missing; use crate: :x;");
    let failure = f.load().unwrap_err();
    assert_eq!(failure.diagnostics[0].stage, "parse");
    assert_eq!(failure.sources.files().len(), 1);
    assert_eq!(failure.usage.probes, 0);
}

#[cfg(target_os = "linux")]
#[test]
fn invalid_project_child_never_loads_its_declared_children() {
    let f = Fixture::new();
    f.write("app.ox", "mod child;");
    f.write("child.ox", "mod missing; pub use crate::f;");
    let failure = f.load().unwrap_err();
    assert_eq!(failure.diagnostics[0].stage, "parse");
    assert_eq!(
        failure.diagnostics[0].primary.unwrap().file,
        SourceFileId(1)
    );
    assert_eq!(failure.sources.files().len(), 2);
    assert_eq!(failure.usage.probes, 1);
}

#[test]
fn new_ast_payload_inventory_counts_every_path_and_nested_row() {
    let f = Fixture::new();
    f.write(
        "app.ox",
        "use crate::f; pub struct R {pub n:i32} pub fn f()->i32{return crate::f();}",
    );
    let p = f.load().unwrap();
    let a = p.try_file_ast(SourceFileId(0)).unwrap();
    let expected = size_of::<ast::ImportDecl>()
        + 2 * size_of::<ast::AbsolutePath>()
        + 4 * size_of::<Span>()
        + size_of::<ast::StructDecl>()
        + size_of::<ast::StructField>()
        + size_of::<ast::Function>()
        + size_of::<ast::BodyBlock>()
        + size_of::<ast::Stmt>()
        + size_of::<ast::Expr>()
        + 3 * size_of::<ast::ItemId>();
    assert_eq!(a.imports.len(), 1);
    assert_eq!(a.paths.len(), 2);
    assert_eq!(a.path_segments.len(), 4);
    assert_eq!(p.inventory().unwrap().ast_payload, expected);
    assert_eq!(
        p.inventory().unwrap().ast_headers,
        size_of::<ast::Program>()
    );
}

#[test]
fn remeasured_per_node_ast_envelope_is_inclusive() {
    let f = Fixture::new();
    // A function with one empty outer block is exactly one charged node.
    // Its Function row, BodyBlock row and lexical ItemId consume 288 bytes.
    f.write("app.ox", &"fn f()->(){}".repeat(100));
    let p = f.load().unwrap();
    assert_eq!(p.usage().syntax_nodes, 100);
    assert_eq!(
        p.inventory().unwrap().ast_payload,
        288 * p.usage().syntax_nodes
    );
    for text in [
        "fn f()->(){if true {} else {} return;}",
        "pub struct R {pub n:i32} fn f(x:&R)->(){g(&*x);return;}",
        "use crate::f; fn f()->i32{return crate::f();}",
        "fn f()->(){let x:R=R{n:g(1)};return;}",
    ] {
        f.write("app.ox", text);
        let p = f.load().unwrap();
        assert!(p.inventory().unwrap().ast_payload <= 288 * p.usage().syntax_nodes);
    }
}

#[test]
fn unit2_measured_layout_and_requested_inventory() {
    macro_rules! sizes { ($($ty:ty),* $(,)?) => { $(println!("layout {} {}", stringify!($ty), size_of::<$ty>());)* }; }
    sizes!(
        SourceMap,
        super::super::source::SourceFile,
        Span,
        lexer::Token,
        ast::Program,
        ast::Function,
        ast::Param,
        ast::TypeSyntax,
        ast::TypeSyntaxKind,
        ast::BodyBlock,
        ast::Stmt,
        ast::StructDecl,
        ast::StructField,
        ast::Expr,
        ast::Argument,
        ast::FieldInit,
        ast::ItemId,
        ast::ModuleDecl,
        ast::PathId,
        ast::ItemPath,
        ast::AbsolutePath,
        ast::ImportDecl,
        ItemPathRef,
        ModuleHeader,
        ProjectSources,
        FunctionAstKey,
        RecordAstKey,
        ExprKey,
        BlockKey,
        Frame,
        SourceUsage
    );
    let f = Fixture::new();
    f.write(
        "app.ox",
        "use crate::f; pub struct R {pub n:i32} pub fn f()->i32{return crate::f();}",
    );
    let p = f.load().unwrap();
    println!("usage {:?}", p.usage());
    println!("inventory {:?}", p.inventory().unwrap());
    let mut spans = 0;
    let a = p.try_file_ast(SourceFileId(0)).unwrap();
    assert!(a.validate_spans_and_ids(|span| {
        spans += 1;
        p.try_text(span).is_some()
    }));
    println!("validation span callbacks {spans}");
}

#[cfg(not(target_os = "linux"))]
#[test]
fn project_candidate_module_policy_is_explicit_on_unqualified_hosts() {
    let f = Fixture::new();
    f.write("app.ox", "pub mod child;");
    let failure = f.load().unwrap_err();
    let error = &failure.diagnostics[0];
    assert_eq!((error.code, error.stage), ("E0005", "source"));
    assert_eq!(
        error.message,
        "module source policy is not qualified on this host"
    );
    assert_eq!(
        error.primary,
        Some(Span {
            file: SourceFileId(0),
            start: 8,
            end: 13
        })
    );
    assert_eq!(failure.sources.files().len(), 1);
    assert_eq!(failure.usage.probes, 0);
}
