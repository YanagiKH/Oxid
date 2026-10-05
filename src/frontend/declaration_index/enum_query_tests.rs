//! C2b pure index queries. Successful queries do not open source execution.
use super::*;
use crate::frontend::project::ProjectLimits;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering as AtomicOrdering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-c2b-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, AtomicOrdering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        for (name, text) in files {
            let path = root.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        Self(root)
    }
    fn load(&self) -> ProjectSources {
        ProjectSources::load_enum_index_candidate(
            self.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
            &mut Allocator::default(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn freeze(project: &ProjectSources) -> DeclarationIndex<'_> {
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    collect_enum_candidate(
        SourceOwner::project(project),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap()
}
fn paths(project: &ProjectSources, file: usize) -> Vec<QualifiedPathRef> {
    project
        .try_file_ast(SourceFileId(file))
        .unwrap()
        .expressions
        .iter()
        .filter_map(|expression| match expression.kind {
            ast::ExprKind::QualifiedValue { path, .. } => Some(QualifiedPathRef {
                file: SourceFileId(file),
                path,
            }),
            _ => None,
        })
        .collect()
}

#[test]
fn enum_query_literal_classification_work_and_legacy_callee_costs() {
    let fixture = Fixture::new(&[(
        "main.ox",
        "enum E{V} fn E()->(){E::V;crate::E();crate::E::V;return;}",
    )]);
    let project = fixture.load();
    let index = freeze(&project);
    let paths = paths(&project, 0);
    assert_eq!(paths.len(), 3);
    let variant = VariantId {
        enumeration: EnumId(0),
        index: 0,
    };
    for (position, (expected, cost, range, terminal)) in [
        (
            QualifiedValueEndpoint::Variant(variant),
            11,
            (21, 25),
            (24, 25),
        ),
        (
            QualifiedValueEndpoint::Function(DefId(0)),
            9,
            (26, 34),
            (33, 34),
        ),
        (
            QualifiedValueEndpoint::Variant(variant),
            11,
            (37, 48),
            (47, 48),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(paths[position].path, ast::PathId(position));
        let work = WorkMeter::new(cost);
        work.enable_observation();
        assert_eq!(
            index
                .query(&work)
                .qualified_value_endpoint(ModuleId(0), paths[position])
                .unwrap(),
            expected
        );
        assert_eq!(work.used(), cost);
        let origin = Span {
            file: SourceFileId(0),
            start: range.0,
            end: range.1,
        };
        if let QualifiedValueEndpoint::Variant(expected) = expected {
            assert!(work.observations.borrow().iter().any(|event|matches!(event,Observation::VariantTarget {origin:at,variant,..} if *at==origin && *variant==expected)));
            assert!(!work
                .observations
                .borrow()
                .iter()
                .any(|event| matches!(event, Observation::Target { .. })));
        }
        let denied = WorkMeter::new(cost - 1);
        denied.enable_observation();
        let error = index
            .query(&denied)
            .qualified_value_endpoint(ModuleId(0), paths[position])
            .unwrap_err();
        assert_eq!(
            (error.code, error.primary),
            (
                "E0400",
                Some(Span {
                    file: SourceFileId(0),
                    start: terminal.0,
                    end: terminal.1
                })
            )
        );
        assert_eq!(denied.used(), cost - 1);
        assert!(!denied.observations.borrow().iter().any(|event| matches!(
            event,
            Observation::Target { .. } | Observation::VariantTarget { .. }
        )));
    }
    let name = project.try_file_ast(SourceFileId(0)).unwrap().functions[0].name;
    for (path, cost) in [
        (ast::ItemPath::Unqualified(name), 8),
        (ast::ItemPath::Absolute(paths[1].path), 12),
    ] {
        let work = WorkMeter::new(cost);
        assert_eq!(
            index
                .query(&work)
                .callee(
                    ModuleId(0),
                    ItemPathRef {
                        file: SourceFileId(0),
                        path
                    },
                    false
                )
                .unwrap(),
            DefId(0)
        );
        assert_eq!(work.used(), cost);
    }
    assert_eq!(
        index
            .query(&WorkMeter::default())
            .variant(ModuleId(0), paths[0])
            .unwrap(),
        variant
    );
    assert_eq!(
        index
            .query(&WorkMeter::default())
            .variant(ModuleId(0), paths[1])
            .unwrap_err()
            .code,
        "E0202"
    );
    assert_eq!(
        index.require_current_source_pipeline().unwrap_err().code,
        "E0101"
    );
}

#[test]
fn enum_query_imported_alias_value_parameter_and_record_only_reference() {
    let fixture=Fixture::new(&[
        ("main.ox","mod m; use crate::m::E as A; fn main()->(){A::V;crate::m::E::V;crate::m::f();return;} fn take(x:A)->A{return x;} fn borrow(x:&A)->(){return;}"),
        ("m.ox","pub enum E{V} pub fn f()->(){return;}"),
    ]);
    let project = fixture.load();
    let index = freeze(&project);
    let paths = paths(&project, 0);
    let work = WorkMeter::default();
    for path in &paths[..2] {
        assert_eq!(
            index.query(&work).variant(ModuleId(0), *path).unwrap(),
            VariantId {
                enumeration: EnumId(0),
                index: 0
            }
        );
    }
    assert_eq!(
        index
            .query(&work)
            .qualified_value_endpoint(ModuleId(0), paths[2])
            .unwrap(),
        QualifiedValueEndpoint::Function(DefId(3))
    );
    let ast = project.try_file_ast(SourceFileId(0)).unwrap();
    let value = ast.functions[1].params[0].ty;
    assert_eq!(
        index
            .query(&work)
            .value_type(ModuleId(0), value, TypeContext::Value)
            .unwrap(),
        ValueTy::Owned(AggregateTy::Enum(EnumId(0)))
    );
    assert_eq!(
        index
            .query(&work)
            .parameter_type(ModuleId(0), value)
            .unwrap(),
        ParameterTy::Value(ValueTy::Owned(AggregateTy::Enum(EnumId(0))))
    );
    let ast::TypeSyntaxKind::Name(path) = value.kind else {
        panic!("nominal source parameter")
    };
    assert_eq!(
        index
            .query(&work)
            .nominal_type(
                ModuleId(0),
                ItemPathRef {
                    file: SourceFileId(0),
                    path
                },
                TypeContext::Value
            )
            .unwrap(),
        NominalId::Enum(EnumId(0))
    );
    assert_eq!(
        index
            .query(&work)
            .record_type(
                ModuleId(0),
                ItemPathRef {
                    file: SourceFileId(0),
                    path
                },
                TypeContext::Constructor
            )
            .unwrap_err()
            .code,
        "E0202"
    );
    assert_eq!(
        index
            .query(&work)
            .parameter_type(ModuleId(0), ast.functions[2].params[0].ty)
            .unwrap_err()
            .code,
        "E0202"
    );
    assert_eq!(
        index.require_current_source_pipeline().unwrap_err().code,
        "E0101"
    );
}

#[test]
fn enum_query_private_prefix_and_function_errors_never_fall_back() {
    let fixture = Fixture::new(&[
        (
            "main.ox",
            "mod m;fn main()->(){crate::m::E::Missing;crate::m::f();crate::m::E();return;}",
        ),
        (
            "m.ox",
            "enum E{V} fn f()->(){return;} pub fn E()->(){return;}",
        ),
    ]);
    let project = fixture.load();
    let index = freeze(&project);
    let paths = paths(&project, 0);
    for (position, name) in [(0, "E"), (1, "f")] {
        let work = WorkMeter::default();
        work.enable_observation();
        let error = index
            .query(&work)
            .qualified_value_endpoint(ModuleId(0), paths[position])
            .unwrap_err();
        assert_eq!(error.code, "E0206");
        assert_eq!(project.text(error.primary.unwrap()), name);
        assert!(!work
            .events
            .borrow()
            .iter()
            .any(|event| event.operation == "variant lookup probe"));
        assert!(!work.observations.borrow().iter().any(|event| matches!(
            event,
            Observation::Target { .. } | Observation::VariantTarget { .. }
        )));
    }
    assert_eq!(
        index
            .query(&WorkMeter::default())
            .qualified_value_endpoint(ModuleId(0), paths[2])
            .unwrap(),
        QualifiedValueEndpoint::Function(DefId(2))
    );
}

#[test]
fn enum_query_actual_fixed_and_prepared_name_carriers_are_measured() {
    use std::mem::align_of;
    println!("enum-query-layout Prefix={}/{} Endpoint={}/{} NominalExposure={}/{} LegacyExposure={}/{} NominalId={}/{} Prepared={}/{} OptionalPrepared={}/{} Fixed={}",size_of::<AbsolutePrefix>(),align_of::<AbsolutePrefix>(),size_of::<QualifiedValueEndpoint>(),align_of::<QualifiedValueEndpoint>(),size_of::<NominalExposure>(),align_of::<NominalExposure>(),size_of::<Exposure>(),align_of::<Exposure>(),size_of::<NominalId>(),align_of::<NominalId>(),size_of::<PreparedTypeName<'_>>(),align_of::<PreparedTypeName<'_>>(),size_of::<Option<PreparedTypeName<'_>>>(),align_of::<Option<PreparedTypeName<'_>>>(),FIXED_SCRATCH);
    assert!(FIXED_SCRATCH > 3968);
    const { assert!(FIXED_SCRATCH <= 4096) };
    #[cfg(target_pointer_width = "64")]
    assert_eq!(size_of::<PreparedTypeName<'_>>(), 576);
}
