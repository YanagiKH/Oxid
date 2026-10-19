use super::*;
use crate::frontend::{
    options::{self, Route},
    project::{forest::PackageGraph, ModuleId},
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-path-session-unit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn package(&self, name: &str, dependencies: &str, source: &str) {
        let root = self.0.join(name);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("oxid.toml"), format!("[project]\nedition=\"typed-preview\"\nentry=\"lib.ox\"\nname=\"same-name\"\n[dependencies]\n{dependencies}")).unwrap();
        fs::write(root.join("lib.ox"), source).unwrap();
    }
    fn request(&self) -> Box<[Request; 1]> {
        let args = [
            "check".to_string(),
            format!("--manifest-path={}/app/oxid.toml", self.0.display()),
        ];
        let Route::ManifestCheck { request } = options::route(&args) else {
            panic!("manifest route");
        };
        request
    }
    fn load(&self) -> CompilerSession {
        CompilerSession::load(
            self.request(),
            Configuration::new(
                Command::Check,
                Engine::Interpreter,
                EntryPolicy::Result,
                false,
            ),
        )
        .unwrap_or_else(|(_, d)| panic!("{d:?}"))
    }
    fn graph(&self) -> PackageGraph {
        manifest::graph::resolve(self.request(), Command::Check, false)
            .unwrap()
            .graph
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn nominal_mismatch(
    session: &CompilerSession,
    file_suffix: &str,
    expression: &str,
    expected: &str,
) -> String {
    let errors = session.check().unwrap_err();
    assert!(errors.iter().all(|d| d.code != "E0500"), "{errors:?}");
    assert_eq!(errors.len(), 1, "{errors:?}");
    let error = &errors[0];
    assert_eq!((error.code, error.stage), ("E0300", "type"));
    assert_eq!(error.message, expected);
    let source = session
        .project()
        .sources()
        .files()
        .iter()
        .find(|file| file.path().ends_with(file_suffix))
        .unwrap();
    let start = source.text().rfind(expression).unwrap();
    assert_eq!(
        error.primary,
        Some(source.span(start, start + expression.len()))
    );
    let (line, column) = source.location(start);
    assert!(error
        .render_human(session.project().sources())
        .contains(&format!("{}:{line}:{column}", source.path())));
    error.message.clone()
}

#[test]
fn path_dependency_nominal_root_and_nested_mismatches_keep_offending_spans() {
    for (declaration, ty, value) in [
        ("pub struct Record{}", "Record", "Record{}"),
        ("pub enum State{Ready}", "State", "State::Ready"),
    ] {
        for nested in [false, true] {
            let f = Fixture::new();
            let module = if nested { "model::" } else { "" };
            let source = format!("{declaration}\npub fn make()->{ty}{{return {value};}}");
            f.package("dep", "", if nested { "pub mod model;" } else { &source });
            if nested {
                fs::write(f.0.join("dep/model.ox"), &source).unwrap();
            }
            f.package(
                "app",
                "dep=\"../dep\"",
                &format!("use dep::{module}make;\nfn main()->i32{{return make();}}"),
            );
            let expected = format!("type mismatch: expected i32, found package#1::{module}{ty}");
            nominal_mismatch(&f.load(), "/app/lib.ox", "make()", &expected);

            // An unused erroneous dependency body retains its own file and range.
            f.package("app", "dep=\"../dep\"", "fn main()->i32{return 0;}");
            let source = format!("{declaration}\npub fn bad()->i32{{return {value};}}");
            let file = if nested { "dep/model.ox" } else { "dep/lib.ox" };
            fs::write(f.0.join(file), source).unwrap();
            nominal_mismatch(&f.load(), &format!("/{file}"), value, &expected);
        }
    }
}

#[test]
fn path_dependency_nominal_identity_ignores_aliases_and_project_names() {
    for (declaration, ty, value) in [
        ("pub struct Record{}", "Record", "Record{}"),
        ("pub enum State{Ready}", "State", "State::Ready"),
    ] {
        let f = Fixture::new();
        let source = format!("{declaration}\npub fn make()->{ty}{{return {value};}}");
        for (left, right) in [("left", "right"), ("renamed_left", "renamed_right")] {
            f.package("left", "", &source);
            f.package("right", "", &source);
            f.package("app", &format!("{left}=\"../left\"\n{right}=\"../right\""), &format!("use {left}::{ty} as Expected; use {right}::make;\nfn wrong()->Expected{{return make();}} fn main()->i32{{return 0;}}"));
            for package in ["app", "left", "right"] {
                let path = f.0.join(package).join("oxid.toml");
                let manifest = fs::read_to_string(&path).unwrap();
                fs::write(path, manifest.replace("same-name", left)).unwrap();
            }
            nominal_mismatch(
                &f.load(),
                "/app/lib.ox",
                "make()",
                &format!("type mismatch: expected package#1::{ty}, found package#2::{ty}"),
            );
        }
        f.package("app", "dep=\"../right\"", &format!("use dep::make;\n{declaration}\nfn wrong()->{ty}{{return make();}} fn main()->i32{{return 0;}}"));
        nominal_mismatch(
            &f.load(),
            "/app/lib.ox",
            "make()",
            &format!("type mismatch: expected package#0::{ty}, found package#1::{ty}"),
        );
    }
}

#[test]
fn path_dependency_nominal_single_package_spelling_is_unchanged() {
    for (declaration, ty, value) in [
        ("pub struct Record{}", "Record", "Record{}"),
        ("pub enum State{Ready}", "State", "State::Ready"),
    ] {
        for (declaration, prefix) in [
            (declaration, "crate::"),
            (declaration.trim_start_matches("pub "), ""),
        ] {
            let f = Fixture::new();
            f.package(
                "app",
                "",
                &format!("{declaration}\nfn main()->i32{{return {value};}}"),
            );
            nominal_mismatch(
                &f.load(),
                "/app/lib.ox",
                value,
                &format!("type mismatch: expected i32, found {prefix}{ty}"),
            );
        }
    }
}

#[test]
fn path_dependency_nominal_long_names_keep_package_identity_within_display_bound() {
    use crate::frontend::{
        declaration_index::{
            collect_enum_candidate, IndexLimits, NominalId, SourceOwner, WorkMeter,
        },
        oir::owned_types::{EnumId, RecordId},
        project::budget::Allocator,
        source::SourceFileId,
    };
    let f = Fixture::new();
    let module = "m".repeat(200);
    f.package(
        "app",
        "left=\"../left\"\nright=\"../right\"",
        "fn main()->i32{return 0;}",
    );
    for package in ["left", "right"] {
        f.package(package, "", &format!("pub mod {module};"));
        fs::write(
            f.0.join(package).join(format!("{module}.ox")),
            "pub struct Record{} pub enum State{Ready}",
        )
        .unwrap();
    }
    let session = f.load();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect_enum_candidate(
        SourceOwner::project(session.project()),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap();
    let at = session.project().sources().get(SourceFileId(0)).span(0, 0);
    for id in 0..2 {
        for (nominal, kind, terminal) in [
            (NominalId::Record(RecordId(id)), "record", "Record"),
            (NominalId::Enum(EnumId(id)), "enum", "State"),
        ] {
            let name = index
                .query(&work)
                .prepare_nominal_type_name(nominal, at)
                .unwrap();
            let used = work.used();
            let display = name.to_string();
            assert_eq!(display.len(), 160);
            assert!(display.starts_with(&format!("package#{}::", id + 1)));
            assert!(display.ends_with(&format!("...::{terminal} [{kind} #{id}]")));
            assert_eq!(name.to_string(), display);
            assert_eq!(work.used(), used);
        }
    }
}

#[test]
fn path_dependency_library_main_and_import_alias_never_select_app_entry() {
    let f = Fixture::new();
    f.package(
        "app",
        "dep=\"../dep\"",
        "use dep::main; fn test_app()->bool{return true;}",
    );
    f.package(
        "dep",
        "",
        "pub fn main()->i32{return 99;} fn test_library(x:i32)->i32{return x;}",
    );
    let session = f.load();
    let program = session.check().unwrap();
    assert_eq!(program.entry(), None);
    assert_eq!(program.run().unwrap_err().code, "E0600");
    let suite = session.tests().unwrap();
    assert_eq!(suite.len(), 1);
    assert_eq!(suite.name(0), Some("test_app"));
    assert!(suite.run(0).unwrap());
}

#[test]
fn path_dependency_session_deduplicates_diamond_and_pins_one_forest() {
    let f = Fixture::new();
    f.package("app", "left=\"../left\"\nright=\"../right\"", "use left::answer as a; use right::answer as b; fn prelude()->i32{return 0;} fn main()->i32{return a()+b();} fn test_sum()->bool{return a()+b()==14;}");
    f.package("left", "shared=\"../shared\"", "pub mod child; use shared::number; pub fn answer()->i32{return number();} fn main()->i32{return 91;}");
    fs::write(f.0.join("left/child.ox"), "fn hidden()->i32{return 0;}").unwrap();
    f.package(
        "shared",
        "",
        "pub fn number()->i32{return 7;} fn test_dependency(x:i32)->i32{return x;}",
    );
    f.package(
        "right",
        "same=\"../shared\"",
        "use same::number; pub fn answer()->i32{return number();}",
    );
    let session = f.load();
    assert_eq!(session.selected, PackageId(0));
    assert_eq!(session.configuration.edition, Edition::TypedPreview);
    assert_eq!(session.configuration.target, Target::Host);
    assert_eq!(session.configuration.profile, Profile::Default);
    let p = session.project();
    let graph = p.graph().unwrap();
    assert_eq!(graph.packages.len(), 4);
    assert_eq!(
        graph.packages.iter().map(|p| p.root.0).collect::<Vec<_>>(),
        [0, 1, 3, 4]
    );
    assert_eq!(
        graph.packages[1].dependencies[0].target,
        graph.packages[3].dependencies[0].target
    );
    for (m, header) in p.modules().iter().enumerate() {
        assert_eq!(header.file.0, m);
        assert_eq!(p.package_root(ModuleId(m)).unwrap().0, [0, 1, 1, 3, 4][m]);
    }
    let checked = session.check().unwrap();
    assert_eq!(checked.entry(), Some(crate::frontend::hir::DefId(1)));
    assert_eq!(checked.function_count(), 9);
    assert_eq!(checked.run().unwrap(), oir::Scalar::I32(14));
    let native = checked.native_module().unwrap();
    let suite = session.tests().unwrap();
    assert_eq!(suite.len(), 1);
    assert!(suite.run(0).unwrap());
    assert!(suite
        .native_module(0)
        .unwrap()
        .contains("xor i1 %value, true"));
    // Subsequent consumption reads only this retained source/manifest snapshot.
    fs::write(f.0.join("shared/lib.ox"), "pub fn number()->i32{return 9;}").unwrap();
    fs::remove_file(f.0.join("left/oxid.toml")).unwrap();
    assert_eq!(session.check().unwrap().native_module().unwrap(), native);
    assert_eq!(
        session.check().unwrap().run().unwrap(),
        oir::Scalar::I32(14)
    );
}

#[test]
fn path_dependency_source_limits_are_aggregate_and_not_multiplied() {
    let f = Fixture::new();
    f.package("app", "dep=\"../dep\"", "fn main()->i32{return 1;}");
    f.package("dep", "", "pub fn value()->i32{return 2;}");
    for limits in [
        ProjectLimits {
            modules: 1,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            source_bytes: 35,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            tokens: 15,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            nodes: 4,
            ..ProjectLimits::default()
        },
    ] {
        let error = ProjectSources::load_forest(f.graph(), limits).unwrap_err();
        assert!(
            error.diagnostics.iter().any(|d| d.code == "E0400"),
            "{:?}",
            error.diagnostics
        );
    }
}

#[test]
fn path_dependency_forest_boundaries_keep_private_roots_private() {
    let f = Fixture::new();
    f.package(
        "app",
        "dep=\"../dep\"",
        "fn secret()->i32{return 1;} fn main()->i32{return 0;}",
    );
    f.package("dep", "", "pub fn value()->i32{return crate::secret();}");
    let session = f.load();
    let errors = session.check().unwrap_err();
    assert!(errors.iter().any(|d| d
        .render_human(session.project().sources())
        .contains("dep/lib.ox")));
    f.package(
        "app",
        "dep=\"../dep\"",
        "use dep::secret; fn main()->i32{return secret();}",
    );
    f.package("dep", "", "fn secret()->i32{return 1;}");
    assert!(f
        .load()
        .check()
        .unwrap_err()
        .iter()
        .any(|d| d.code == "E0206"));
}
