use super::*;
use crate::frontend::{
    declaration_index::{IndexLimits, SourceOwner, WorkMeter},
    oir,
};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-package-session-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    fn spec(&self, name: &str, dependencies: &[(&str, usize)]) -> PackageSpec {
        PackageSpec {
            root: self.0.join(name),
            entry: self.0.join(name).join("src/lib.ox"),
            dependencies: dependencies
                .iter()
                .map(|(alias, package)| PackageDependency {
                    alias: (*alias).into(),
                    package: *package,
                })
                .collect(),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn execute(specs: &[PackageSpec]) -> Result<oir::Scalar, Vec<Diagnostic>> {
    let package =
        PackageSources::load(specs, None, ProjectLimits::default()).map_err(|f| f.diagnostics)?;
    let limits = IndexLimits::default();
    let checked = oir::project::check_package_executable(
        &package,
        limits,
        &WorkMeter::new(limits.work),
        &mut Allocator::default(),
    )?;
    checked.run().map_err(|e| vec![*e])
}

#[cfg(target_os = "linux")]
#[test]
fn package_session_scopes_crate_roots_and_retains_real_source_ids() {
    let f = Fixture::new();
    f.write(
        "app/src/lib.ox",
        "fn main()->i32{return crate::left::value()+crate::right::value();}",
    );
    for (name, value) in [("left", 20), ("right", 22)] {
        f.write(&format!("{name}/src/lib.ox"), "mod inner; pub fn value()->i32{return crate::inner::value();} fn main()->bool{return true;}");
        f.write(
            &format!("{name}/src/inner.ox"),
            &format!("pub fn value()->i32{{return {value};}}"),
        );
    }
    let specs = [
        f.spec("app", &[("left", 1), ("right", 2)]),
        f.spec("left", &[]),
        f.spec("right", &[]),
    ];
    assert_eq!(execute(&specs).unwrap(), oir::Scalar::I32(42));
    let loaded = PackageSources::load(&specs, None, ProjectLimits::default()).unwrap();
    assert_eq!(loaded.project.modules().len(), 5);
    for file in loaded.project.sources().files() {
        assert_eq!(file.text().as_bytes(), std::fs::read(file.path()).unwrap());
        let ast = loaded.project.try_file_ast(file.span(0, 0).file).unwrap();
        assert!(ast.belongs_to(file));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn package_session_diamond_shares_nominal_identity_and_entry_override() {
    let f = Fixture::new();
    f.write("app/src/lib.ox", "use crate::token::Token; fn main()->i32{let t=Token{value:42};return crate::reader::read(&t);}");
    f.write(
        "reader/src/lib.ox",
        "use crate::token::Token; pub fn read(t:&Token)->i32{return t.value;}",
    );
    f.write("token/src/lib.ox", "pub struct Token{pub value:i32}");
    f.write(
        "app/tests/check.ox",
        "fn main()->i32{return crate::reader::number();}",
    );
    let specs = [
        f.spec("app", &[("reader", 1), ("token", 2)]),
        f.spec("reader", &[("token", 2)]),
        f.spec("token", &[]),
    ];
    assert_eq!(execute(&specs).unwrap(), oir::Scalar::I32(42));
    let loaded = PackageSources::load(&specs, None, ProjectLimits::default()).unwrap();
    assert_eq!(loaded.project.modules().len(), 3);
    f.write("reader/src/lib.ox", "pub fn number()->i32{return 17;}");
    let override_path = f.0.join("app/tests/check.ox");
    let loaded =
        PackageSources::load(&specs, Some(&override_path), ProjectLimits::default()).unwrap();
    let limits = IndexLimits::default();
    let checked = oir::project::check_package_executable(
        &loaded,
        limits,
        &WorkMeter::new(limits.work),
        &mut Allocator::default(),
    )
    .unwrap();
    assert_eq!(checked.run().unwrap(), oir::Scalar::I32(17));
}

#[cfg(target_os = "linux")]
#[test]
fn package_session_same_spelling_from_distinct_roots_has_distinct_nominal_identity() {
    let f = Fixture::new();
    f.write("app/src/lib.ox", "use crate::left::Token; fn main()->i32{let t=Token{value:42};return crate::right::read(&t);}");
    let source = "pub struct Token{pub value:i32} pub fn read(t:&Token)->i32{return t.value;}";
    f.write("left/src/lib.ox", source);
    f.write("right/src/lib.ox", source);
    let specs = [
        f.spec("app", &[("left", 1), ("right", 2)]),
        f.spec("left", &[]),
        f.spec("right", &[]),
    ];
    let errors = execute(&specs).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.code == "E0300" && e.message.contains("does not match parameter")),
        "{errors:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn package_session_enum_identity_is_shared_only_for_one_canonical_root() {
    let f = Fixture::new();
    f.write(
        "app/src/lib.ox",
        "use crate::shape::Event;fn main()->i32{return crate::reader::read(Event::Value(42));}",
    );
    f.write("shape/src/lib.ox", "pub enum Event{Value(i32)}");
    f.write(
        "reader/src/lib.ox",
        "use crate::shape::Event;pub fn read(e:Event)->i32{match e{Event::Value(n)=>{return n;},}}",
    );
    let mut specs = vec![
        f.spec("app", &[("shape", 1), ("reader", 2)]),
        f.spec("shape", &[]),
        f.spec("reader", &[("shape", 1)]),
    ];
    assert_eq!(execute(&specs).unwrap(), oir::Scalar::I32(42));
    f.write("other/src/lib.ox", "pub enum Event{Value(i32)}");
    specs.push(f.spec("other", &[]));
    specs[2].dependencies[0].package = 3;
    let errors = execute(&specs).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.code == "E0300" && e.message.contains("does not match parameter")),
        "{errors:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn package_session_a_later_root_cannot_read_an_earlier_roots_private_items() {
    let f = Fixture::new();
    f.write(
        "app/src/lib.ox",
        "fn main()->i32{return crate::reader::value();}",
    );
    f.write("secret/src/lib.ox", "pub struct Token{value:i32} pub fn make()->Token{return Token{value:42};} fn hidden()->i32{return 42;}");
    let specs = [
        f.spec("app", &[("reader", 2)]),
        f.spec("secret", &[]),
        f.spec("reader", &[("secret", 1)]),
    ];
    for source in [
        "pub fn value()->i32{return crate::secret::hidden();}",
        "pub fn value()->i32{let t=crate::secret::make();return t.value;}",
    ] {
        f.write("reader/src/lib.ox", source);
        let errors = execute(&specs).unwrap_err();
        assert!(errors.iter().any(|e| e.code == "E0206"), "{errors:?}");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn package_session_private_items_and_transitive_edges_are_not_exposed() {
    let f = Fixture::new();
    f.write(
        "left/src/lib.ox",
        "fn hidden()->i32{return 1;} pub fn public()->i32{return crate::leaf::value();}",
    );
    f.write("leaf/src/lib.ox", "pub fn value()->i32{return 42;}");
    let specs = [
        f.spec("app", &[("left", 1)]),
        f.spec("left", &[("leaf", 2)]),
        f.spec("leaf", &[]),
    ];
    for (source, code) in [
        ("fn main()->i32{return crate::left::hidden();}", "E0206"),
        ("fn main()->i32{return crate::leaf::value();}", "E0205"),
        (
            "fn main()->i32{return crate::left::leaf::value();}",
            "E0205",
        ),
    ] {
        f.write("app/src/lib.ox", source);
        let errors = execute(&specs).unwrap_err();
        assert!(errors.iter().any(|e| e.code == code), "{errors:?}");
    }
    f.write(
        "app/src/lib.ox",
        "fn main()->i32{return crate::left::public();}",
    );
    assert_eq!(execute(&specs).unwrap(), oir::Scalar::I32(42));
    f.write("left/src/lib.ox", "pub mod inner;");
    f.write("left/src/inner.ox", "pub fn value()->i32{return 19;}");
    f.write(
        "app/src/lib.ox",
        "fn main()->i32{return crate::left::inner::value();}",
    );
    assert_eq!(execute(&specs).unwrap(), oir::Scalar::I32(19));
    f.write("left/src/lib.ox", "mod inner;");
    assert!(execute(&specs)
        .unwrap_err()
        .iter()
        .any(|e| e.code == "E0206"));
}

#[cfg(target_os = "linux")]
#[test]
fn package_session_dependency_calls_keep_exclusive_borrow_rules() {
    let f = Fixture::new();
    f.write(
        "lib/src/lib.ox",
        "pub struct Token{pub value:i32} pub fn take(a:&mut Token,b:&Token)->i32{return b.value;}",
    );
    f.write("app/src/lib.ox", "use crate::lib::Token;fn main()->i32{let mut t=Token{value:1};return crate::lib::take(&mut t,&t);}");
    let specs = [f.spec("app", &[("lib", 1)]), f.spec("lib", &[])];
    let errors = execute(&specs).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.stage == "borrow" || e.code.starts_with("E03")),
        "{errors:?}"
    );
}

#[test]
fn package_session_graph_refuses_cycles_duplicates_unknown_and_unreachable_nodes() {
    let f = Fixture::new();
    f.write("app/src/lib.ox", "fn main()->i32{return 0;}");
    f.write("lib/src/lib.ox", "pub fn value()->i32{return 0;}");
    let valid = [f.spec("app", &[("lib", 1)]), f.spec("lib", &[])];
    assert!(validate_graph(&valid, None).is_ok());
    let mut invalid = valid.clone();
    invalid[1].dependencies.push(PackageDependency {
        alias: "app".into(),
        package: 0,
    });
    assert!(validate_graph(&invalid, None)
        .unwrap_err()
        .message
        .contains("cycle"));
    invalid = valid.clone();
    invalid[1] = invalid[0].clone();
    assert!(validate_graph(&invalid, None)
        .unwrap_err()
        .message
        .contains("duplicate canonical"));
    invalid = valid.clone();
    invalid[0].dependencies[0].package = 2;
    assert!(validate_graph(&invalid, None)
        .unwrap_err()
        .message
        .contains("unknown package"));
    invalid = valid.clone();
    invalid[0].dependencies.clear();
    assert!(validate_graph(&invalid, None)
        .unwrap_err()
        .message
        .contains("undeclared package"));
    invalid = valid.clone();
    invalid[0].dependencies.push(PackageDependency {
        alias: "LIB".into(),
        package: 1,
    });
    assert!(validate_graph(&invalid, None)
        .unwrap_err()
        .message
        .contains("case folding"));
    invalid = valid.clone();
    invalid[0].dependencies[0].alias = "std".into();
    assert!(validate_graph(&invalid, None)
        .unwrap_err()
        .message
        .contains("alias"));
    invalid[0].dependencies[0].alias = "x".repeat(65_536);
    let error = validate_graph(&invalid, None).unwrap_err();
    assert!(error.message.contains("alias") && error.message.len() < 160);
    invalid = valid.clone();
    invalid[0].entry = f.0.join("lib/src/lib.ox");
    assert!(validate_graph(&invalid, None)
        .unwrap_err()
        .message
        .contains("escapes"));
    assert!(validate_graph(&vec![valid[0].clone(); 257], None)
        .unwrap_err()
        .message
        .contains("count"));
    invalid = valid.clone();
    invalid[0].dependencies = vec![valid[0].dependencies[0].clone(); 1025];
    assert!(validate_graph(&invalid, None)
        .unwrap_err()
        .message
        .contains("dependency limit"));
}

#[test]
fn package_session_shared_suffix_depth_is_independent_of_dependency_order() {
    let f = Fixture::new();
    for count in [33, 34] {
        let mut specs = Vec::new();
        for node in 0..count {
            let name = format!("p{node}");
            f.write(&format!("{name}/src/lib.ox"), "fn main()->i32{return 0;}");
            let dependencies = if node + 1 < count {
                vec![("next", node + 1)]
            } else {
                vec![]
            };
            specs.push(f.spec(&name, &dependencies));
        }
        specs[0].dependencies.insert(
            0,
            PackageDependency {
                alias: "shortcut".into(),
                package: count - 3,
            },
        );
        for _ in 0..2 {
            let result = validate_graph(&specs, None);
            if count == 33 {
                assert!(result.is_ok(), "32-edge path must be accepted: {result:?}");
            } else {
                assert!(result.unwrap_err().message.contains("depth"));
            }
            specs[0].dependencies.reverse();
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn package_session_alias_collision_and_aggregate_source_caps_fail_before_execution() {
    let f = Fixture::new();
    f.write("app/src/lib.ox", "mod library;fn main()->i32{return 0;}");
    f.write("lib/src/lib.ox", "pub fn value()->i32{return 0;}");
    let specs = [f.spec("app", &[("library", 1)]), f.spec("lib", &[])];
    let errors = PackageSources::load(&specs, None, ProjectLimits::default()).unwrap_err();
    assert_eq!(errors.diagnostics[0].code, "E0201");
    f.write("app/src/lib.ox", "fn main()->i32{return 0;}");
    for limits in [
        ProjectLimits {
            modules: 1,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            source_bytes: 30,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            tokens: 12,
            ..ProjectLimits::default()
        },
    ] {
        let failure = PackageSources::load(&specs, None, limits).unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "E0400", "{failure:?}");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn package_session_raised_caller_limit_still_caps_source_identity_at_256_modules() {
    let f = Fixture::new();
    let mut source = String::from("fn main()->i32{return 0;}");
    for child in 0..255 {
        source.push_str(&format!("mod m{child};"));
        f.write(&format!("app/src/m{child}.ox"), "");
    }
    f.write("app/src/lib.ox", &source);
    let specs = [f.spec("app", &[])];
    let limits = ProjectLimits {
        modules: usize::MAX,
        ..ProjectLimits::default()
    };
    assert_eq!(
        PackageSources::load(&specs, None, limits)
            .unwrap()
            .project
            .modules()
            .len(),
        256
    );
    source.push_str("mod m255;");
    f.write("app/src/lib.ox", &source);
    f.write("app/src/m255.ox", "");
    let failure = PackageSources::load(&specs, None, limits).unwrap_err();
    assert_eq!(failure.diagnostics[0].code, "E0400");
    assert!(failure.diagnostics[0].message.contains("module count"));
    assert_eq!(failure.usage.modules, 256);
}

#[cfg(target_os = "linux")]
#[test]
fn package_session_entry_and_module_symlinks_cannot_cross_package_boundaries() {
    let f = Fixture::new();
    f.write(
        "app/src/lib.ox",
        "fn main()->i32{return crate::lib::value();}",
    );
    f.write("lib/src/lib.ox", "pub fn value()->i32{return 42;}");
    let mut specs = [f.spec("app", &[("lib", 1)]), f.spec("lib", &[])];
    let linked = f.0.join("lib/src/link.ox");
    std::os::unix::fs::symlink(&specs[1].entry, &linked).unwrap();
    specs[1].entry = linked;
    assert!(validate_graph(&specs, None)
        .unwrap_err()
        .message
        .contains("symbolic link"));
    specs[1].entry = f.0.join("lib/src/lib.ox");
    f.write("app/src/lib.ox", "mod linked;fn main()->i32{return 0;}");
    std::os::unix::fs::symlink(&specs[1].entry, f.0.join("app/src/linked.ox")).unwrap();
    let failure = PackageSources::load(&specs, None, ProjectLimits::default()).unwrap_err();
    assert_eq!(failure.diagnostics[0].code, "E0005");
    assert!(failure.diagnostics[0].message.contains("symbolic link"));
}

#[test]
fn package_session_keeps_single_package_source_owner_layout() {
    #[allow(dead_code)]
    enum Before<'a> {
        Project(&'a ProjectSources),
        Original {
            file: &'a SourceFile,
            ast: &'a ast::Program,
            view: super::super::super::source::SourceView<'a>,
        },
    }
    assert_eq!(
        std::mem::size_of::<SourceOwner<'_>>(),
        std::mem::size_of::<Before<'_>>()
    );
    assert_eq!(
        std::mem::align_of::<SourceOwner<'_>>(),
        std::mem::align_of::<Before<'_>>()
    );
}
