//! The import candidate does not enable any ordinary source consumer.
use super::*;
use crate::frontend::{parser, project::ProjectLimits};
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
            "oxid-builtin-index-{}-{}",
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
        ProjectSources::load_builtin_candidate(
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
fn collect<'a>(
    project: &'a ProjectSources,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> DeclarationFacts<'a> {
    collect_builtin_candidate(
        SourceOwner::project(project),
        IndexLimits::default(),
        work,
        allocator,
    )
    .unwrap()
}
fn bound(project: &ProjectSources, module: usize, import: usize) -> ItemPathRef {
    let alias = project.try_file_ast(SourceFileId(module)).unwrap().imports[import].alias;
    ItemPathRef {
        file: alias.file,
        path: ast::ItemPath::Unqualified(alias),
    }
}
#[test]
fn builtin_index_sets_keep_original_ids_and_source_only_rows() {
    for (imports, expected) in [
        ("", BuiltinSet::None),
        ("use std::io::ReadStatus as Status;", BuiltinSet::ReadStatus),
        ("use std::io::read_stdin as input;", BuiltinSet::ReadStdin),
        (
            "use std::io::read_stdin as input; use std::io::ReadStatus as Status;",
            BuiltinSet::ReadStdin,
        ),
        (
            "use std::io::ReadStatus as Status; use std::io::read_stdin as input;",
            BuiltinSet::ReadStdin,
        ),
    ] {
        let text = format!(
            "{imports} enum User{{Z}} fn helper()->i32{{return 1;}} fn main()->i32{{return 0;}}"
        );
        let fixture = Fixture::new(&[("main.ox", &text)]);
        let project = fixture.load();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let facts = collect(&project, &work, &mut allocator);
        assert_eq!(
            (
                facts.plan().counts.functions,
                facts.plan().counts.enums,
                facts.plan().counts.variants
            ),
            (2, 1, 1)
        );
        assert!(facts.require_builtin_candidate_pipeline().is_ok());
        assert!(facts.require_current_source_pipeline().is_err());
        assert!(facts.require_no_builtin_candidate().is_err());
        let index = facts.finish(&work, &mut allocator).unwrap();
        assert_eq!(index.builtin_set(), expected);
        assert_eq!(
            (index.source_function_count(), index.source_enum_count()),
            (2, 1)
        );
        assert_eq!(
            (index.function_count(), index.enum_count()),
            (2 + expected.extra_functions(), 1 + expected.extra_enums())
        );
        assert_eq!(index.root_original_main(), Some(DefId(1)));
        assert!(index.function(DefId(2)).is_err());
        assert!(index.enumeration(EnumId(1)).is_err());
        assert_eq!(
            index.enum_origin(EnumId(0)).unwrap(),
            DeclarationOrigin::Source
        );
        let mut counts = index.enum_variant_counts();
        assert_eq!(counts.len(), index.enum_count());
        assert_eq!(counts.next(), Some(1));
        assert_eq!(counts.next(), (expected.extra_enums() == 1).then_some(3));
        assert_eq!(counts.next(), None);
        if expected != BuiltinSet::None {
            let enumeration = index.builtin_enum_id(BuiltinEnum::ReadStatus).unwrap();
            assert_eq!(enumeration, EnumId(1));
            let view = index.enum_view(enumeration).unwrap();
            assert_eq!(view.name(), "ReadStatus");
            assert!(view.source_syntax().is_none());
            assert_eq!(
                project.text(view.diagnostic_span()),
                if imports.starts_with("use std::io::read_stdin") {
                    "read_stdin"
                } else {
                    "ReadStatus"
                }
            );
            assert_eq!(
                index
                    .query(&work)
                    .prepare_nominal_type_name(NominalId::Enum(enumeration), view.diagnostic_span())
                    .unwrap()
                    .to_string(),
                "std::io::ReadStatus"
            );
            for (member, name, payload) in [
                (0, "Eof", Some(Ty::I32)),
                (1, "Full", None),
                (2, "IoError", None),
            ] {
                let variant = view
                    .variant(VariantId {
                        enumeration,
                        index: member,
                    })
                    .unwrap();
                assert_eq!(variant.name(), name);
                assert_eq!(variant.payload(), payload);
                assert_eq!(variant.diagnostic_span(), view.diagnostic_span());
            }
            assert!(view
                .variant(VariantId {
                    enumeration,
                    index: 3
                })
                .is_err());
            assert!(view
                .variant(VariantId {
                    enumeration: EnumId(0),
                    index: 0
                })
                .is_err());
        }
        if expected == BuiltinSet::ReadStdin {
            assert_eq!(
                index
                    .builtin_function_id(BuiltinFunction::ReadStdin)
                    .unwrap(),
                DefId(2)
            );
            let import = usize::from(imports.starts_with("use std::io::ReadStatus"));
            assert_eq!(
                index
                    .query(&work)
                    .callee(ModuleId(0), bound(&project, 0, import), false)
                    .unwrap(),
                DefId(2)
            );
        }
    }
}
#[cfg(target_os = "linux")]
#[test]
fn builtin_index_aliases_across_modules_coexist_with_crate_std_and_source_names() {
    let fixture=Fixture::new(&[
        ("main.ox","use std::io::read_stdin as input; use std::io::ReadStatus as Status; pub mod std; pub mod child; fn read_stdin()->i32{return 7;} fn main()->i32{return read_stdin();}"),
        ("std.ox","pub enum ReadStatus{Eof(i32),Full,IoError} pub fn read_stdin()->i32{return 2;}"),
        ("child.ox","use std::io::ReadStatus as S; use std::io::read_stdin as read; use crate::std::ReadStatus as User; use crate::std::read_stdin as user_read; pub fn helper()->i32{return user_read();}"),
    ]);
    let project = fixture.load();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect(&project, &work, &mut allocator)
        .finish(&work, &mut allocator)
        .unwrap();
    let builtin = index.builtin_enum_id(BuiltinEnum::ReadStatus).unwrap();
    let mut query = index.query(&work);
    assert_eq!(
        query
            .nominal_type(ModuleId(0), bound(&project, 0, 1), TypeContext::Value)
            .unwrap(),
        NominalId::Enum(builtin)
    );
    assert_eq!(
        query
            .nominal_type(ModuleId(2), bound(&project, 2, 0), TypeContext::Value)
            .unwrap(),
        NominalId::Enum(builtin)
    );
    assert_eq!(
        query
            .nominal_type(ModuleId(2), bound(&project, 2, 2), TypeContext::Value)
            .unwrap(),
        NominalId::Enum(EnumId(0))
    );
    assert_eq!(
        query
            .callee(ModuleId(2), bound(&project, 2, 1), false)
            .unwrap(),
        index
            .builtin_function_id(BuiltinFunction::ReadStdin)
            .unwrap()
    );
    assert_eq!(
        query
            .callee(ModuleId(2), bound(&project, 2, 3), false)
            .unwrap(),
        DefId(2)
    );
    assert_eq!(
        index
            .builtin_enum_anchor(BuiltinEnum::ReadStatus)
            .unwrap()
            .file,
        SourceFileId(0)
    );
}
#[test]
fn builtin_index_invalid_imports_and_legacy_gates_fail_before_index_allocation() {
    for text in [
        "use std::fs::ReadStatus; fn main()->(){return;}",
        "use std::io::missing; fn main()->(){return;}",
    ] {
        let fixture = Fixture::new(&[("main.ox", text)]);
        let project = fixture.load();
        let mut allocator = Allocator::default();
        let error = collect_builtin_candidate(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut allocator,
        )
        .unwrap_err();
        assert_eq!(error.code, "E0205");
        assert_eq!(allocator.attempts, 0);
    }
    for text in [
        "use std::io::ReadStatus as A; use std::io::ReadStatus as B; fn main()->(){return;}",
        "use std::io::read_stdin as read; use std::io::read_stdin as again; fn main()->(){return;}",
        "use std::io::ReadStatus as bool; fn main()->(){return;}",
        "use std::io::read_stdin as main; fn main()->(){return;}",
    ] {
        let fixture = Fixture::new(&[("main.ox", text)]);
        let project = fixture.load();
        let work = WorkMeter::default();
        work.enable_observation();
        let mut allocator = Allocator::default();
        assert!(collect(&project, &work, &mut allocator)
            .finish(&work, &mut allocator)
            .is_err());
    }
    let fixture = Fixture::new(&[("main.ox", "use std::io::read_stdin; fn main()->(){return;}")]);
    let project = fixture.load();
    assert!(SourceOwner::project(&project)
        .owned(&WorkMeter::default())
        .unwrap());
    for collect in [collect_originals, collect_closed, collect_enum_candidate] {
        let mut allocator = Allocator::default();
        assert!(collect(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut allocator
        )
        .is_err());
        assert_eq!(allocator.attempts, 0);
    }
    assert!(ProjectSources::load_typed(
        fixture.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default()
    )
    .is_err());
    let _ = parser::StdImportPolicy::Closed;
}
#[test]
fn builtin_index_resource_endpoints_and_layouts_are_explicit() {
    let fixture = Fixture::new(&[("main.ox", "use std::io::read_stdin; fn main()->(){return;}")]);
    let project = fixture.load();
    let facts = collect(&project, &WorkMeter::default(), &mut Allocator::default());
    let plan = facts.plan();
    for (retained, scratch, expected) in [
        (plan.retained, plan.scratch, true),
        (plan.retained - 1, plan.scratch, false),
        (plan.retained, plan.scratch - 1, false),
    ] {
        let mut allocator = Allocator::default();
        let result = collect_builtin_candidate(
            SourceOwner::project(&project),
            IndexLimits {
                retained,
                scratch,
                ..IndexLimits::default()
            },
            &WorkMeter::default(),
            &mut allocator,
        );
        assert_eq!(result.is_ok(), expected);
        if !expected {
            assert_eq!(allocator.attempts, 0);
            assert_eq!(result.unwrap_err().code, "E0400");
        }
    }
    println!("builtin-index-layout set={} admission={} marker={} handle={} projection={} projection-result={} handle-option={} handle-result={} item-result={} origin-result={} tables={} index={} facts={} counts={} plan={} prepared={} prepared-option={} enum-view={} enum-option={} enum-result={} variant-view={} variant-option={} variant-result={} enum-counts={} fixed={}",
        size_of::<BuiltinSet>(),size_of::<BuiltinAdmission>(),size_of::<CandidateOrigin>(),size_of::<DeclarationHandle>(),size_of::<DeclarationProjection>(),size_of::<Result<DeclarationProjection,Box<Diagnostic>>>(),size_of::<Option<DeclarationHandle>>(),size_of::<Result<Option<DeclarationHandle>,Box<Diagnostic>>>(),size_of::<Result<BuiltinItem,Box<Diagnostic>>>(),size_of::<Result<DeclarationOrigin,Box<Diagnostic>>>(),size_of::<Tables<'_>>(),size_of::<DeclarationIndex<'_>>(),size_of::<DeclarationFacts<'_>>(),size_of::<Counts>(),size_of::<IndexPlan>(),size_of::<PreparedTypeName<'_>>(),size_of::<Option<PreparedTypeName<'_>>>(),size_of::<EnumView<'_>>(),size_of::<Option<EnumView<'_>>>(),size_of::<Result<EnumView<'_>,Box<Diagnostic>>>(),size_of::<VariantView<'_>>(),size_of::<Option<VariantView<'_>>>(),size_of::<Result<VariantView<'_>,Box<Diagnostic>>>(),size_of::<EnumVariantCounts<'_>>(),FIXED_SCRATCH);
    assert_eq!(size_of::<EnumView<'_>>(), 16);
    assert_eq!(size_of::<VariantView<'_>>(), 24);
    assert!(FIXED_SCRATCH <= 4096);
}

#[test]
fn builtin_index_observed_status_only_import_uses_nominal_schema() {
    let fixture = Fixture::new(&[(
        "main.ox",
        "use std::io::ReadStatus as S; fn main()->(){return;}",
    )]);
    let project = fixture.load();
    let work = WorkMeter::default();
    work.enable_observation();
    let mut allocator = Allocator::default();
    let index = collect(&project, &work, &mut allocator)
        .finish(&work, &mut allocator)
        .unwrap();
    assert_eq!(index.enum_count(), 1);
    assert!(work.observations.borrow().iter().any(|event| matches!(
        event,
        Observation::NominalImport {
            ty: Some(NominalId::Enum(EnumId(0))),
            committed: true,
            ..
        }
    )));
}

#[test]
fn builtin_index_function_dependency_does_not_bind_a_local_type_name() {
    let fixture = Fixture::new(&[(
        "main.ox",
        "use std::io::read_stdin; fn helper()->ReadStatus{return;} fn main()->(){return;}",
    )]);
    let project = fixture.load();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect(&project, &work, &mut allocator)
        .finish(&work, &mut allocator)
        .unwrap();
    let result = project.try_file_ast(SourceFileId(0)).unwrap().functions[0].result;
    let error = index
        .query(&work)
        .value_type(ModuleId(0), result, TypeContext::Value)
        .unwrap_err();
    assert_eq!(error.code, "E0202");
    assert_eq!(index.enum_count(), 1);
}

#[test]
fn builtin_index_allocation_injection_precedes_frozen_identity() {
    let fixture = Fixture::new(&[(
        "main.ox",
        "use std::io::ReadStatus as S; use std::io::read_stdin as input; fn main()->(){return;}",
    )]);
    let project = fixture.load();
    let mut baseline = Allocator::default();
    let _ = collect(&project, &WorkMeter::default(), &mut baseline);
    for failure in 1..=baseline.attempts {
        let mut allocator = Allocator {
            fail_at: Some(failure),
            ..Allocator::default()
        };
        let error = collect_builtin_candidate(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut allocator,
        )
        .unwrap_err();
        assert_eq!(error.code, "E0400");
        assert_eq!(allocator.attempts, failure);
    }
}
