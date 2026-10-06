//! Current imports and the permanently private candidate share nominal facts.
use super::*;
use crate::frontend::{
    parser,
    project::{LoadFailure, ProjectLimits},
};
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
    fn load_output(&self) -> ProjectSources {
        ProjectSources::load_output_candidate(
            self.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
            &mut Allocator::default(),
        )
        .unwrap()
    }
    fn load_current(&self) -> ProjectSources {
        ProjectSources::load_typed(
            self.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn assert_module_policy_denial(failure: LoadFailure, primary: Span) {
    assert_eq!(failure.diagnostics.len(), 1);
    let diagnostic = &failure.diagnostics[0];
    assert_eq!((diagnostic.code, diagnostic.stage), ("E0005", "source"));
    assert_eq!(
        diagnostic.message,
        "module source policy is not qualified on this host"
    );
    assert_eq!(diagnostic.primary, Some(primary));
    assert!(diagnostic.secondary.is_empty());
    assert!(diagnostic.notes.is_empty());
    assert_eq!(failure.sources.files().len(), 1);
    assert_eq!(failure.usage.modules, 1);
    assert_eq!(failure.usage.probes, 0);
    assert_eq!(failure.usage.directory_entries, 0);
    assert_eq!(failure.usage.directory_name_units, 0);
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

#[test]
fn builtin_current_index_keeps_source_ids_anchors_and_private_markers_distinct() {
    for (imports, expected) in [
        ("", BuiltinSet::None),
        ("use std::io::ReadStatus as S;", BuiltinSet::ReadStatus),
        ("use std::io::read_stdin as input;", BuiltinSet::ReadStdin),
        (
            "use std::io::ReadStatus as S; use std::io::read_stdin as input;",
            BuiltinSet::ReadStdin,
        ),
        (
            "use std::io::read_stdin as input; use std::io::ReadStatus as S;",
            BuiltinSet::ReadStdin,
        ),
    ] {
        let text = format!(
            "{imports} enum User{{Z}} fn read_stdin()->i32{{return 1;}} fn main()->i32{{return read_stdin();}}"
        );
        let fixture = Fixture::new(&[("main.ox", &text)]);
        let project = fixture.load_current();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let facts = collect_originals(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap();
        assert!(facts.require_current_source_pipeline().is_ok());
        assert!(facts.require_builtin_candidate_pipeline().is_err());
        assert_eq!(
            facts.require_no_builtin_candidate().is_ok(),
            expected == BuiltinSet::None
        );
        let index = facts.finish(&work, &mut allocator).unwrap();
        assert!(index.require_current_source_pipeline().is_ok());
        assert!(index.require_builtin_candidate_pipeline().is_err());
        assert_eq!(
            index.require_no_builtin_candidate().is_ok(),
            expected == BuiltinSet::None
        );
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
        assert_eq!(
            index.function_origin(DefId(0)).unwrap(),
            DeclarationOrigin::Source
        );
        assert_eq!(
            index.enum_origin(EnumId(0)).unwrap(),
            DeclarationOrigin::Source
        );
        if expected != BuiltinSet::None {
            assert_eq!(
                index.builtin_enum_id(BuiltinEnum::ReadStatus).unwrap(),
                EnumId(1)
            );
            let anchor = index.builtin_enum_anchor(BuiltinEnum::ReadStatus).unwrap();
            assert_eq!(anchor.file, SourceFileId(0));
            assert_eq!(anchor.start, 13);
            assert!(index.enumeration(EnumId(1)).is_err());
        }
        if expected == BuiltinSet::ReadStdin {
            assert_eq!(
                index
                    .builtin_function_id(BuiltinFunction::ReadStdin)
                    .unwrap(),
                DefId(2)
            );
            assert!(index.function(DefId(2)).is_err());
        }
        let candidate = collect(&project, &work, &mut allocator);
        assert!(candidate.require_builtin_candidate_pipeline().is_ok());
        assert!(candidate.require_current_source_pipeline().is_err());
        let candidate = candidate.finish(&work, &mut allocator).unwrap();
        assert!(candidate.require_builtin_candidate_pipeline().is_ok());
        assert!(candidate.require_current_source_pipeline().is_err());
        assert!(candidate.require_no_builtin_candidate().is_err());
    }
}

#[test]
fn builtin_current_no_stdin_work_trace_and_boundaries_match_closed_policy() {
    for text in [
        "fn main()->i32{return 1;}",
        "enum User{V} fn main()->i32{return 1;}",
        "use crate::helper as h; fn helper()->i32{return 1;} fn main()->i32{return h();}",
        "use crate::User as S; enum User{V} fn main()->i32{return 1;}",
    ] {
        let fixture = Fixture::new(&[("main.ox", text)]);
        let project = fixture.load_current();
        assert!(!project
            .try_file_ast(SourceFileId(0))
            .unwrap()
            .uses_std_imports());
        let mut expected_trace = None;
        let mut mandatory = 0;
        for collect in [collect_std_closed, collect_originals] {
            let work = WorkMeter::default();
            work.enable_observation();
            let mut allocator = Allocator::default();
            let facts = collect(
                SourceOwner::project(&project),
                IndexLimits::default(),
                &work,
                &mut allocator,
            )
            .unwrap();
            let preflight: u64 = work
                .events
                .borrow()
                .iter()
                .filter(|event| event.operation == "preflight visit")
                .map(|event| event.units)
                .sum();
            let boundary = preflight + facts.plan().build_work;
            facts.finish(&work, &mut allocator).unwrap();
            let trace: Vec<_> = work
                .events
                .borrow()
                .iter()
                .map(|event| (event.operation, event.origin, event.units))
                .collect();
            if let Some(expected) = &expected_trace {
                assert_eq!(&trace, expected);
                assert_eq!(boundary, mandatory);
            } else {
                expected_trace = Some(trace);
                mandatory = boundary;
            }
        }
        for (limit, ok) in [(mandatory - 1, false), (mandatory, true)] {
            for collect in [collect_std_closed, collect_originals] {
                let work = WorkMeter::new(limit);
                let mut allocator = Allocator::default();
                let facts = collect(
                    SourceOwner::project(&project),
                    IndexLimits {
                        work: limit,
                        ..IndexLimits::default()
                    },
                    &work,
                    &mut allocator,
                );
                assert_eq!(facts.is_ok(), ok);
                if ok {
                    facts.unwrap().finish(&work, &mut allocator).unwrap();
                } else {
                    assert_eq!(allocator.attempts, 0);
                    assert_eq!(facts.unwrap_err().code, "E0400");
                }
            }
        }
    }
}
#[cfg(target_os = "linux")]
#[test]
fn builtin_index_aliases_across_modules_coexist_with_crate_std_and_source_names() {
    for current in [false, true] {
        let fixture=Fixture::new(&[
        ("main.ox","use std::io::read_stdin as input; use std::io::ReadStatus as Status; pub mod std; pub mod child; fn read_stdin()->i32{return 7;} fn main()->i32{return read_stdin();}"),
        ("std.ox","pub enum ReadStatus{Eof(i32),Full,IoError} pub fn read_stdin()->i32{return 2;}"),
        ("child.ox","use std::io::ReadStatus as S; use std::io::read_stdin as read; use crate::std::ReadStatus as User; use crate::std::read_stdin as user_read; pub fn helper()->i32{return user_read();}"),
    ]);
        let project = if current {
            fixture.load_current()
        } else {
            fixture.load()
        };
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let facts = if current {
            collect_originals(
                SourceOwner::project(&project),
                IndexLimits::default(),
                &work,
                &mut allocator,
            )
            .unwrap()
        } else {
            collect(&project, &work, &mut allocator)
        };
        let index = facts.finish(&work, &mut allocator).unwrap();
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
}
#[test]
fn builtin_index_invalid_imports_and_legacy_gates_fail_before_index_allocation() {
    for text in [
        "use std::fs::ReadStatus; fn main()->(){return;}",
        "use std::io::missing; fn main()->(){return;}",
    ] {
        let fixture = Fixture::new(&[("main.ox", text)]);
        let project = fixture.load();
        for collect in [collect_builtin_candidate, collect_originals] {
            let mut allocator = Allocator::default();
            let error = collect(
                SourceOwner::project(&project),
                IndexLimits::default(),
                &WorkMeter::default(),
                &mut allocator,
            )
            .unwrap_err();
            assert_eq!(error.code, "E0205");
            assert_eq!(allocator.attempts, 0);
        }
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
        assert!(collect_originals(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &work,
            &mut allocator
        )
        .unwrap()
        .finish(&work, &mut allocator)
        .is_err());
    }
    let fixture = Fixture::new(&[("main.ox", "use std::io::read_stdin; fn main()->(){return;}")]);
    let project = fixture.load();
    assert!(SourceOwner::project(&project)
        .owned(&WorkMeter::default())
        .unwrap());
    for collect in [collect_std_closed, collect_closed, collect_enum_candidate] {
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
    assert!(ProjectSources::load_typed_closed_std(
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
        for collect in [collect_builtin_candidate, collect_originals] {
            let mut allocator = Allocator::default();
            let result = collect(
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
    }
    println!("builtin-index-layout set={} admission={} marker={} handle={} projection={} projection-result={} handle-option={} handle-result={} item-result={} origin-result={} tables={} index={} facts={} counts={} plan={} prepared={} prepared-option={} enum-view={} enum-option={} enum-result={} variant-view={} variant-option={} variant-result={} enum-counts={} fixed={}",
        size_of::<BuiltinSet>(),size_of::<BuiltinAdmission<'static>>(),size_of::<CandidateOrigin>(),size_of::<DeclarationHandle>(),size_of::<DeclarationProjection>(),size_of::<Result<DeclarationProjection,Box<Diagnostic>>>(),size_of::<Option<DeclarationHandle>>(),size_of::<Result<Option<DeclarationHandle>,Box<Diagnostic>>>(),size_of::<Result<BuiltinItem,Box<Diagnostic>>>(),size_of::<Result<DeclarationOrigin,Box<Diagnostic>>>(),size_of::<Tables<'_>>(),size_of::<DeclarationIndex<'_>>(),size_of::<DeclarationFacts<'_>>(),size_of::<Counts>(),size_of::<IndexPlan>(),size_of::<PreparedTypeName<'_>>(),size_of::<Option<PreparedTypeName<'_>>>(),size_of::<EnumView<'_>>(),size_of::<Option<EnumView<'_>>>(),size_of::<Result<EnumView<'_>,Box<Diagnostic>>>(),size_of::<VariantView<'_>>(),size_of::<Option<VariantView<'_>>>(),size_of::<Result<VariantView<'_>,Box<Diagnostic>>>(),size_of::<EnumVariantCounts<'_>>(),FIXED_SCRATCH);
    assert_eq!(size_of::<EnumView<'_>>(), 16);
    assert_eq!(size_of::<VariantView<'_>>(), 24);
    const { assert!(FIXED_SCRATCH <= 4096) };
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
    for current in [false, true] {
        let fixture = Fixture::new(&[(
            "main.ox",
            "use std::io::read_stdin; fn helper()->ReadStatus{return;} fn main()->(){return;}",
        )]);
        let project = if current {
            fixture.load_current()
        } else {
            fixture.load()
        };
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let facts = if current {
            collect_originals(
                SourceOwner::project(&project),
                IndexLimits::default(),
                &work,
                &mut allocator,
            )
            .unwrap()
        } else {
            collect(&project, &work, &mut allocator)
        };
        let index = facts.finish(&work, &mut allocator).unwrap();
        let result = project.try_file_ast(SourceFileId(0)).unwrap().functions[0].result;
        let error = index
            .query(&work)
            .value_type(ModuleId(0), result, TypeContext::Value)
            .unwrap_err();
        assert_eq!(error.code, "E0202");
        assert_eq!(index.enum_count(), 1);
    }
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

/// Disconnected RFC 0025 layouts. These types are never constructed by
/// collection, admitted by a ledger, or used by an effect consumer. New anchor
/// projection/control roles remain unpriced: residual space is not admission.
#[allow(dead_code)]
mod output_layout_feasibility {
    use super::*;
    use std::mem::align_of;

    enum Inventory {
        None,
        ReadStatus,
        ReadStdin,
        WriteStatus,
        ReadStatusWriteStatus,
        ReadStdinWriteStatus,
        WriteStdout,
        ReadStatusWriteStdout,
        ReadStdinWriteStdout,
    }
    enum Enumeration {
        ReadStatus,
        WriteStatus,
    }
    enum Function {
        ReadStdin,
        WriteStdout,
    }
    enum NestedItem {
        Enum(Enumeration),
        Function(Function),
    }
    enum FlatItem {
        ReadStatus,
        ReadStdin,
        WriteStatus,
        WriteStdout,
    }
    enum NestedOrigin {
        Source,
        Builtin(NestedItem),
    }
    enum FlatOrigin {
        Source,
        Builtin(FlatItem),
    }
    enum NestedProjection {
        SourceOriginal(u32),
        Builtin(NestedItem),
    }
    enum FlatProjection {
        SourceOriginal(u32),
        Builtin(FlatItem),
    }
    // Exactly the predecessor admission's per-family payload alternatives.
    enum FamilySpanState {
        None,
        Status(CompactSpan),
        Function {
            status: CompactSpan,
            function: CompactSpan,
        },
    }
    struct DuplicatedSpanAdmission {
        families: [FamilySpanState; 2],
    }
    // A future sealed constructor must establish dependency closure and the
    // exact retained ImportRow association. NONE marks absent anchor slots.
    struct FamilyImportAnchors {
        enumeration: u32,
        function: u32,
    }
    struct ImportOrdinalAdmission {
        families: [FamilyImportAnchors; 2],
    }
    struct BorrowedAdmission<'s> {
        enumerations: [Option<&'s Span>; 2],
        functions: [Option<&'s Span>; 2],
    }

    // Complete actual Tables fields, in actual declaration order. Each macro
    // expansion is a disconnected private type; production is not generalized.
    macro_rules! tables_model {
        ($name:ident, $admission:ty) => {
            struct $name<'s> {
                sources: SourceOwner<'s>,
                originals: Vec<OriginalRow>,
                original_order: Vec<u32>,
                functions: Vec<FunctionRow>,
                records: Vec<RecordRow>,
                fields: Vec<FieldRow>,
                enums: Vec<EnumRow>,
                variants: Vec<VariantRow>,
                modules: Vec<ModuleRow>,
                children: Vec<u32>,
                imports: Vec<ImportRow>,
                aliases: Vec<AliasCell>,
                alias_order: Vec<u32>,
                root_main: u32,
                candidate_source_origin: CandidateOrigin,
                builtins: $admission,
            }
        };
    }
    tables_model!(BaselineTables, FamilySpanState);
    tables_model!(SpanTables, DuplicatedSpanAdmission);
    tables_model!(OrdinalTables, ImportOrdinalAdmission);
    macro_rules! enclosing_models {
        ($index:ident, $facts:ident, $tables:ident) => {
            struct $index<'s> {
                tables: $tables<'s>,
            }
            struct $facts<'s> {
                tables: $tables<'s>,
                scratch: Scratch,
                plan: IndexPlan,
            }
        };
    }
    enclosing_models!(BaselineIndex, BaselineFacts, BaselineTables);
    enclosing_models!(SpanIndex, SpanFacts, SpanTables);
    enclosing_models!(OrdinalIndex, OrdinalFacts, OrdinalTables);

    // This separate mirror binds its admission references to the same existing
    // source lifetime. The references never point into this movable carrier.
    struct BorrowedTables<'s> {
        sources: SourceOwner<'s>,
        originals: Vec<OriginalRow>,
        original_order: Vec<u32>,
        functions: Vec<FunctionRow>,
        records: Vec<RecordRow>,
        fields: Vec<FieldRow>,
        enums: Vec<EnumRow>,
        variants: Vec<VariantRow>,
        modules: Vec<ModuleRow>,
        children: Vec<u32>,
        imports: Vec<ImportRow>,
        aliases: Vec<AliasCell>,
        alias_order: Vec<u32>,
        root_main: u32,
        candidate_source_origin: CandidateOrigin,
        builtins: BorrowedAdmission<'s>,
    }
    enclosing_models!(BorrowedIndex, BorrowedFacts, BorrowedTables);

    // Compare the exact old/new endpoint roles without claiming credit from
    // an unspecified historical bank. Both include the same existing lookup,
    // validation and public-return transports. The new validated local is
    // separate from its borrowed constructor argument, so that copy is visible.
    struct ExistingAnchorTransports<'s> {
        endpoint_option: Option<&'s Span>,
        endpoint_result: Result<&'s Span, Box<Diagnostic>>,
        selected_endpoint: Span,
        validation_argument: Span,
        validation_return: Result<CompactSpan, Box<Diagnostic>>,
        constructor_argument: CompactSpan,
        item_argument: BuiltinItem,
        getter_local: CompactSpan,
        getter_span_return: Span,
        getter_option_return: Option<Span>,
        public_anchor_return: Result<Span, Box<Diagnostic>>,
    }
    struct BorrowedAnchorTransports<'s> {
        endpoint_option: Option<&'s Span>,
        endpoint_result: Result<&'s Span, Box<Diagnostic>>,
        selected_endpoint: &'s Span,
        validation_argument: Span,
        validation_return: Result<CompactSpan, Box<Diagnostic>>,
        validated_local: CompactSpan,
        constructor_argument: &'s Span,
        item_argument: NestedItem,
        getter_local: &'s Span,
        getter_span_return: Span,
        getter_option_return: Option<Span>,
        public_anchor_return: Result<Span, Box<Diagnostic>>,
    }

    fn same_layout<T, U>() {
        assert_eq!(size_of::<T>(), size_of::<U>());
        assert_eq!(align_of::<T>(), align_of::<U>());
    }
    fn report<T>(name: &str) {
        println!(
            "OUTPUT_INDEX_LAYOUT {name} bytes={} align={}",
            size_of::<T>(),
            align_of::<T>()
        );
    }
    fn substituted_fixed<TablesModel, Admission, Projection, Item, Origin>() -> usize {
        // Start from the preserved 4094-byte predecessor bank. Embedded
        // admission is replaced once through its mirror; the local is separate.
        let old = size_of::<BaselineTables<'static>>()
            + size_of::<FamilySpanState>()
            + size_of::<Result<DeclarationProjection, Box<Diagnostic>>>()
            + size_of::<Result<BuiltinItem, Box<Diagnostic>>>()
            + size_of::<Result<DeclarationOrigin, Box<Diagnostic>>>();
        let new = size_of::<TablesModel>()
            + size_of::<Admission>()
            + size_of::<Result<Projection, Box<Diagnostic>>>()
            + size_of::<Result<Item, Box<Diagnostic>>>()
            + size_of::<Result<Origin, Box<Diagnostic>>>();
        4094usize
            .checked_sub(old)
            .unwrap()
            .checked_add(new)
            .unwrap()
    }
    fn candidate(name: &str, substituted: usize) {
        let headroom = 4096i128 - substituted as i128;
        let delta = substituted as i128 - 4094;
        println!(
            "OUTPUT_INDEX_CANDIDATE {name} predecessor_fixed=4094 actual_fixed={FIXED_SCRATCH} \
             substituted_fixed={substituted} predecessor_delta={delta} ceiling=4096 \
             headroom_before_new_roles={headroom} new_projection_control_bytes=UNPRICED \
             admission=NOT_ESTABLISHED"
        );
    }

    #[test]
    fn bounded_stdout_index_disconnected_enclosing_layout_feasibility() {
        assert_eq!(size_of::<BaselineTables<'static>>(), 368);
        assert_eq!(size_of::<BaselineIndex<'static>>(), 368);
        assert_eq!(size_of::<BaselineFacts<'static>>(), 584);
        assert_eq!(size_of::<FamilySpanState>(), 28);
        same_layout::<BorrowedTables<'static>, Tables<'static>>();
        same_layout::<BorrowedIndex<'static>, DeclarationIndex<'static>>();
        same_layout::<BorrowedFacts<'static>, DeclarationFacts<'static>>();
        same_layout::<BorrowedAdmission<'static>, BuiltinAdmission<'static>>();
        same_layout::<
            Result<BorrowedTables<'static>, Box<Diagnostic>>,
            Result<Tables<'static>, Box<Diagnostic>>,
        >();
        same_layout::<
            Result<BorrowedIndex<'static>, Box<Diagnostic>>,
            Result<DeclarationIndex<'static>, Box<Diagnostic>>,
        >();
        same_layout::<
            Result<BorrowedFacts<'static>, Box<Diagnostic>>,
            Result<DeclarationFacts<'static>, Box<Diagnostic>>,
        >();
        same_layout::<
            Result<BorrowedIndex<'static>, Vec<Diagnostic>>,
            Result<DeclarationIndex<'static>, Vec<Diagnostic>>,
        >();
        assert_eq!(size_of::<ImportOrdinalAdmission>(), 4 * size_of::<u32>());
        assert_eq!(
            substituted_fixed::<
                BaselineTables<'static>,
                FamilySpanState,
                DeclarationProjection,
                BuiltinItem,
                DeclarationOrigin,
            >(),
            4094
        );
        #[cfg(target_pointer_width = "64")]
        assert_eq!(FIXED_SCRATCH, 4096);
        const { assert!(FIXED_SCRATCH <= 4096) };

        macro_rules! layouts {
            ($($ty:ty),+ $(,)?) => {$(report::<$ty>(stringify!($ty));)+};
        }
        macro_rules! envelopes {
            ($tables:ident, $index:ident, $facts:ident) => {
                layouts!(
                    $tables<'static>,
                    $index<'static>,
                    $facts<'static>,
                    Result<$tables<'static>, Box<Diagnostic>>,
                    Result<$index<'static>, Box<Diagnostic>>,
                    Result<$facts<'static>, Box<Diagnostic>>,
                    Result<$index<'static>, Vec<Diagnostic>>,
                );
            };
        }
        layouts!(
            BuiltinSet, BuiltinEnum, BuiltinFunction, BuiltinItem,
            DeclarationOrigin, DeclarationProjection, BuiltinAdmission<'static>,
            Result<DeclarationProjection, Box<Diagnostic>>,
            Result<BuiltinItem, Box<Diagnostic>>,
            Result<DeclarationOrigin, Box<Diagnostic>>,
            PreparedTypeName<'static>, Option<PreparedTypeName<'static>>,
            sealed::EnumSourceCounts<'static>, Inventory, Enumeration, Function,
            NestedItem, FlatItem, NestedOrigin, FlatOrigin,
            NestedProjection, FlatProjection,
            Result<NestedItem, Box<Diagnostic>>, Result<FlatItem, Box<Diagnostic>>,
            Result<NestedOrigin, Box<Diagnostic>>, Result<FlatOrigin, Box<Diagnostic>>,
            Result<NestedProjection, Box<Diagnostic>>, Result<FlatProjection, Box<Diagnostic>>,
            FamilySpanState, DuplicatedSpanAdmission,
            FamilyImportAnchors, ImportOrdinalAdmission,
        );
        envelopes!(Tables, DeclarationIndex, DeclarationFacts);
        envelopes!(BaselineTables, BaselineIndex, BaselineFacts);
        envelopes!(SpanTables, SpanIndex, SpanFacts);
        envelopes!(OrdinalTables, OrdinalIndex, OrdinalFacts);

        candidate(
            "duplicated_spans_nested_item",
            substituted_fixed::<
                SpanTables<'static>,
                DuplicatedSpanAdmission,
                NestedProjection,
                NestedItem,
                NestedOrigin,
            >(),
        );
        candidate(
            "duplicated_spans_flat_item",
            substituted_fixed::<
                SpanTables<'static>,
                DuplicatedSpanAdmission,
                FlatProjection,
                FlatItem,
                FlatOrigin,
            >(),
        );
        candidate(
            "four_import_ordinals_nested_item",
            substituted_fixed::<
                OrdinalTables<'static>,
                ImportOrdinalAdmission,
                NestedProjection,
                NestedItem,
                NestedOrigin,
            >(),
        );
        candidate(
            "four_import_ordinals_flat_item",
            substituted_fixed::<
                OrdinalTables<'static>,
                ImportOrdinalAdmission,
                FlatProjection,
                FlatItem,
                FlatOrigin,
            >(),
        );
        println!(
            "OUTPUT_INDEX_UNPRICED_ROLES collection_import_ordinal_cursor; \
             checked_ordinal_conversion_and_return; anchor_selector_and_return; \
             retained_import_row_projection_and_return; source_ast_path_projection; \
             checked_endpoint_projection_and_return; caller_anchor_return; \
             family_iteration_current_next_and_rank; \
             any_changed_EnumSourceCounts_or_PreparedTypeName_carriers"
        );
        println!(
            "OUTPUT_INDEX_SCOPE disconnected_compiled_type_layouts_only; \
             no_collection_no_source_association_no_effect_no_consumer_authority; \
             no_claim_of_complete_retained_scratch_coexistence"
        );
    }

    #[test]
    fn bounded_stdout_borrowed_anchor_enclosing_layout_feasibility() {
        same_layout::<BorrowedTables<'static>, Tables<'static>>();
        same_layout::<BorrowedIndex<'static>, DeclarationIndex<'static>>();
        same_layout::<BorrowedFacts<'static>, DeclarationFacts<'static>>();
        same_layout::<
            Result<BorrowedFacts<'static>, Box<Diagnostic>>,
            Result<DeclarationFacts<'static>, Box<Diagnostic>>,
        >();
        let substituted = substituted_fixed::<
            BorrowedTables<'static>,
            BorrowedAdmission<'static>,
            NestedProjection,
            NestedItem,
            NestedOrigin,
        >()
        .checked_sub(size_of::<Option<usize>>())
        .unwrap()
        .checked_add(size_of::<u32>())
        .unwrap();
        assert_eq!(substituted, 4094);
        assert_eq!(
            FIXED_SCRATCH,
            substituted + size_of::<BuiltinEnum>() + size_of::<BuiltinFunction>()
        );
        let old_transports = size_of::<ExistingAnchorTransports<'static>>();
        let new_transports = size_of::<BorrowedAnchorTransports<'static>>();
        assert!(new_transports <= old_transports);
        assert_eq!(size_of::<Option<&Span>>(), size_of::<&Span>());
        assert_eq!(
            size_of::<BorrowedAdmission<'static>>(),
            4 * size_of::<&Span>()
        );
        macro_rules! layouts {
            ($($ty:ty),+ $(,)?) => {$(report::<$ty>(stringify!($ty));)+};
        }
        layouts!(
            BorrowedAdmission<'static>,
            BorrowedTables<'static>, BorrowedIndex<'static>, BorrowedFacts<'static>,
            Result<BorrowedTables<'static>, Box<Diagnostic>>,
            Result<BorrowedIndex<'static>, Box<Diagnostic>>,
            Result<BorrowedFacts<'static>, Box<Diagnostic>>,
            Result<BorrowedIndex<'static>, Vec<Diagnostic>>,
            Option<usize>, u32, Option<&Span>, Result<&Span, Box<Diagnostic>>,
            Span, &Span, CompactSpan, Result<CompactSpan, Box<Diagnostic>>,
            Option<Span>, Result<Span, Box<Diagnostic>>,
            ExistingAnchorTransports<'static>, BorrowedAnchorTransports<'static>,
        );
        println!(
            "OUTPUT_INDEX_BORROWED_CANDIDATE actual_fixed={FIXED_SCRATCH} \
             substituted_fixed={substituted} ceiling=4096 headroom={} \
             old_endpoint_transports={old_transports} new_endpoint_transports={new_transports} \
             transport_credit_used=0 phase_overlay=false ordinal_projection=false \
             source_identity=NOT_ESTABLISHED all_nine_family_construction=NOT_ESTABLISHED \
             admission=NOT_ESTABLISHED",
            4096i128 - substituted as i128,
        );
    }

    // Uses actual immutable AST storage and the existing checked path view.
    // It returns only a borrow, never a catalog identity or an executable owner.
    fn borrow_real_endpoint<'s>(
        owner: SourceOwner<'s>,
        path: QualifiedPathRef,
    ) -> Result<&'s Span, Box<Diagnostic>> {
        let endpoint = {
            let view = owner.import_path(path)?;
            let endpoint = view.segments().last().ok_or_else(|| bad(view.span()))?;
            CompactSpan::new(*endpoint)?;
            endpoint
        };
        Ok(endpoint)
    }

    #[test]
    fn bounded_stdout_borrowed_anchor_outlives_real_path_view() {
        let fixture = Fixture::new(&[
            (
                "main.ox",
                "mod child; use std::io::ReadStatus as Status; fn main()->(){return;}",
            ),
            (
                "child.ox",
                "use std::io::read_stdin as input; fn helper()->(){return;}",
            ),
        ]);
        let result = ProjectSources::load_typed(
            fixture.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
        );
        if !cfg!(target_os = "linux") {
            assert_module_policy_denial(
                result.unwrap_err(),
                Span {
                    file: SourceFileId(0),
                    start: 4,
                    end: 9,
                },
            );
            return;
        }
        let project = result.unwrap();
        let owner = SourceOwner::project(&project);
        let root_ast = owner.ast(ModuleId(0)).unwrap();
        let child_ast = owner.ast(ModuleId(1)).unwrap();
        let root_import = &root_ast.imports[0];
        let child_import = &child_ast.imports[0];
        let root_path = QualifiedPathRef {
            file: root_import.span.file,
            path: root_import.path,
        };
        let child_path = QualifiedPathRef {
            file: child_import.span.file,
            path: child_import.path,
        };
        let expected_status = root_ast
            .path_segments(root_import.path)
            .unwrap()
            .last()
            .unwrap();
        let expected_function = child_ast
            .path_segments(child_import.path)
            .unwrap()
            .last()
            .unwrap();
        let borrowed = {
            let copied_owner = owner;
            BorrowedAdmission {
                enumerations: [
                    Some(borrow_real_endpoint(copied_owner, root_path).unwrap()),
                    None,
                ],
                functions: [
                    Some(borrow_real_endpoint(copied_owner, child_path).unwrap()),
                    None,
                ],
            }
        };
        let candidate = BorrowedIndex {
            tables: BorrowedTables {
                sources: owner,
                originals: Vec::new(),
                original_order: Vec::new(),
                functions: Vec::new(),
                records: Vec::new(),
                fields: Vec::new(),
                enums: Vec::new(),
                variants: Vec::new(),
                modules: Vec::new(),
                children: Vec::new(),
                imports: Vec::new(),
                aliases: Vec::new(),
                alias_order: Vec::new(),
                root_main: NONE,
                candidate_source_origin: CandidateOrigin::Current,
                builtins: borrowed,
            },
        };
        let moved = candidate;
        let status = moved.tables.builtins.enumerations[0].unwrap();
        let function = moved.tables.builtins.functions[0].unwrap();
        assert!(std::ptr::eq(status, expected_status));
        assert!(std::ptr::eq(function, expected_function));
        assert_eq!(*status, *expected_status);
        assert_eq!(*function, *expected_function);
        assert_ne!(function.file, SourceFileId(0));
        assert_eq!(moved.tables.sources.text(*status).unwrap(), "ReadStatus");
        assert_eq!(moved.tables.sources.text(*function).unwrap(), "read_stdin");
        assert!(moved.tables.builtins.enumerations[1].is_none());
        assert!(moved.tables.builtins.functions[1].is_none());
    }

    #[derive(Debug, PartialEq, Eq)]
    enum StepFailure {
        Order,
        Catalog,
        Compact,
    }
    // These two inert steps compare control ordering only. catalog_ok models
    // the existing catalog check's success/failure and grants no identity.
    fn existing_order_step(
        endpoint: Span,
        previous: &mut Option<usize>,
        catalog_ok: bool,
    ) -> Result<CompactSpan, StepFailure> {
        if previous.is_some_and(|offset| endpoint.start <= offset) {
            return Err(StepFailure::Order);
        }
        *previous = Some(endpoint.start);
        if !catalog_ok {
            return Err(StepFailure::Catalog);
        }
        CompactSpan::new(endpoint).map_err(|_| StepFailure::Compact)
    }
    fn borrowed_order_step<'s>(
        endpoint: &'s Span,
        previous: &mut u32,
        catalog_ok: bool,
    ) -> Result<&'s Span, StepFailure> {
        if *previous != NONE && endpoint.start <= *previous as usize {
            return Err(StepFailure::Order);
        }
        if !catalog_ok {
            return Err(StepFailure::Catalog);
        }
        let checked = CompactSpan::new(*endpoint).map_err(|_| StepFailure::Compact)?;
        *previous = checked.start;
        Ok(endpoint)
    }

    #[test]
    fn bounded_stdout_borrowed_anchor_sentinel_and_error_order() {
        let span = |file, start, end| Span {
            file: SourceFileId(file),
            start,
            end,
        };
        let valid = span(0, 3, 4);
        let mut previous = NONE;
        assert!(std::ptr::eq(
            borrowed_order_step(&valid, &mut previous, true).unwrap(),
            &valid
        ));
        assert_eq!(previous, 3);
        let zero = span(0, 0, 0);
        let mut zero_previous = NONE;
        assert!(borrowed_order_step(&zero, &mut zero_previous, true).is_ok());
        assert_eq!(zero_previous, 0);

        let largest = (BUILTIN_CONFLICT - 1) as usize;
        assert!(CompactSpan::new(span(largest, largest, largest)).is_ok());
        for reserved in [BUILTIN_CONFLICT as usize, NONE as usize] {
            for invalid in [
                span(reserved, 3, 4),
                span(0, reserved, reserved),
                span(0, 3, reserved),
            ] {
                assert!(CompactSpan::new(invalid).is_err());
                let mut previous = NONE;
                assert_eq!(
                    borrowed_order_step(&invalid, &mut previous, true),
                    Err(StepFailure::Compact)
                );
                assert_eq!(previous, NONE);
            }
        }

        for (endpoint, before, catalog_ok, expected) in [
            (
                span(NONE as usize, 2, 4),
                Some(3),
                false,
                StepFailure::Order,
            ),
            (
                span(NONE as usize, 4, 5),
                Some(3),
                false,
                StepFailure::Catalog,
            ),
            (
                span(NONE as usize, 4, 5),
                Some(3),
                true,
                StepFailure::Compact,
            ),
            (
                span(0, NONE as usize, NONE as usize),
                None,
                false,
                StepFailure::Catalog,
            ),
        ] {
            let mut old_previous = before;
            let mut new_previous = before.map_or(NONE, |value| u32::try_from(value).unwrap());
            let old = existing_order_step(endpoint, &mut old_previous, catalog_ok).unwrap_err();
            let new = borrowed_order_step(&endpoint, &mut new_previous, catalog_ok).unwrap_err();
            assert_eq!(old, expected);
            assert_eq!(new, expected);
            // The old assignment was earlier, but failures return before any
            // next iteration. The new cursor changes only after full validation.
            assert_eq!(new_previous, before.map_or(NONE, |value| value as u32));
        }
    }
}

// Preserve the pinned predecessor receipt from 35e7/e33b while exercising the
// actual successor. The new selector arguments add two scratch bytes; none of
// the old endpoint evidence is relabeled as a successor execution.
#[test]
fn bounded_stdout_retained_header_predecessor_endpoints() {
    assert_eq!(size_of::<DeclarationIndex<'static>>(), 376);
    assert_eq!(size_of::<DeclarationFacts<'static>>(), 592);
    assert_eq!(FIXED_SCRATCH, 4096);
    for (name, source, old_retained, old_scratch) in [
        ("absent", "fn main()->(){return;}", 488, 4102),
        (
            "read_stdin",
            "use std::io::read_stdin; fn main()->(){return;}",
            528,
            4114,
        ),
    ] {
        let fixture = Fixture::new(&[("main.ox", source)]);
        let project = fixture.load();
        let facts = collect(&project, &WorkMeter::default(), &mut Allocator::default());
        let plan = facts.plan();
        assert_eq!(plan.retained, old_retained + 8);
        assert_eq!(plan.scratch, old_scratch + 2);
        for (retained, scratch, expected) in [
            (old_retained, plan.scratch, false),
            (plan.retained - 1, plan.scratch, false),
            (plan.retained, old_scratch, false),
            (plan.retained, plan.scratch - 1, false),
            (plan.retained, plan.scratch, true),
        ] {
            let mut allocator = Allocator::default();
            let result = collect_originals(
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
        println!("OUTPUT_RETAINED_SUCCESSOR name={name} predecessor_header=368 predecessor_retained={old_retained} predecessor_scratch={old_scratch} header=376 retained={} scratch={} fixed={} selector_bytes=2 source={source:?}", plan.retained, plan.scratch, FIXED_SCRATCH);
    }
}

fn output_inventory_cases() -> [(&'static str, &'static str, BuiltinSet); 9] {
    [
        ("", "", BuiltinSet::None),
        (
            "use std::io::ReadStatus as InStatus;",
            "",
            BuiltinSet::ReadStatus,
        ),
        (
            "use std::io::read_stdin as input;",
            "",
            BuiltinSet::ReadStdin,
        ),
        (
            "",
            "use std::io::WriteStatus as OutStatus;",
            BuiltinSet::WriteStatus,
        ),
        (
            "",
            "use std::io::write_stdout as output;",
            BuiltinSet::WriteStdout,
        ),
        (
            "use std::io::ReadStatus as InStatus;",
            "use std::io::WriteStatus as OutStatus;",
            BuiltinSet::ReadStatusWriteStatus,
        ),
        (
            "use std::io::ReadStatus as InStatus;",
            "use std::io::write_stdout as output;",
            BuiltinSet::ReadStatusWriteStdout,
        ),
        (
            "use std::io::read_stdin as input;",
            "use std::io::WriteStatus as OutStatus;",
            BuiltinSet::ReadStdinWriteStatus,
        ),
        (
            "use std::io::read_stdin as input;",
            "use std::io::write_stdout as output;",
            BuiltinSet::ReadStdinWriteStdout,
        ),
    ]
}
fn output_facts<'s>(
    project: &'s ProjectSources,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> DeclarationFacts<'s> {
    collect_output_candidate(
        SourceOwner::project(project),
        IndexLimits::default(),
        work,
        allocator,
    )
    .unwrap()
}

#[test]
fn bounded_stdout_catalog_nine_state_closure_ranks_and_signatures() {
    let cases = output_inventory_cases();
    for (left_rank, (_, _, left)) in cases.iter().enumerate() {
        assert!(cases[..left_rank].iter().all(|(_, _, other)| left != other));
        assert_eq!(left.union(*left), *left);
        assert_eq!(left.union(BuiltinSet::None), *left);
        assert_eq!(
            left.has_output(),
            left.contains_enum(BuiltinEnum::WriteStatus)
        );
        assert_eq!(
            left.has_output_function(),
            left.contains_function(BuiltinFunction::WriteStdout)
        );
        assert_eq!(
            left.extra_enums(),
            BuiltinEnum::ALL
                .into_iter()
                .filter(|item| left.contains_enum(*item))
                .count()
        );
        assert_eq!(
            left.extra_functions(),
            BuiltinFunction::ALL
                .into_iter()
                .filter(|item| left.contains_function(*item))
                .count()
        );
        for item in BuiltinEnum::ALL {
            assert_eq!(left.enum_rank(item).is_some(), left.contains_enum(item));
        }
        for item in BuiltinFunction::ALL {
            assert_eq!(
                left.function_rank(item).is_some(),
                left.contains_function(item)
            );
        }
        assert_eq!(
            left.enum_rank(BuiltinEnum::WriteStatus),
            left.has_output()
                .then_some(usize::from(left.contains_enum(BuiltinEnum::ReadStatus)))
        );
        assert_eq!(
            left.function_rank(BuiltinFunction::WriteStdout),
            left.has_output_function().then_some(usize::from(
                left.contains_function(BuiltinFunction::ReadStdin)
            ))
        );
        for (_, _, right) in cases {
            assert_eq!(left.union(right), right.union(*left));
            assert!(cases
                .iter()
                .any(|(_, _, candidate)| *candidate == left.union(right)));
            for item in BuiltinEnum::ALL {
                assert_eq!(
                    left.union(right).contains_enum(item),
                    left.contains_enum(item) || right.contains_enum(item)
                );
            }
            for item in BuiltinFunction::ALL {
                assert_eq!(
                    left.union(right).contains_function(item),
                    left.contains_function(item) || right.contains_function(item)
                );
            }
            for (_, _, third) in cases {
                assert_eq!(
                    left.union(right).union(third),
                    left.union(right.union(third))
                );
            }
        }
    }
    for (function, status, kind) in [
        (
            BuiltinFunction::ReadStdin,
            BuiltinEnum::ReadStatus,
            BorrowKind::Exclusive,
        ),
        (
            BuiltinFunction::WriteStdout,
            BuiltinEnum::WriteStatus,
            BorrowKind::Shared,
        ),
    ] {
        let set = BuiltinSet::None.admit(BuiltinItem::Function(function));
        assert!(set.contains_enum(status));
        assert!(set.contains_function(function));
        assert_eq!(
            function.signature(EnumId(7)),
            (
                ParameterTy::Reference {
                    referent: BorrowedTy::ScalarSlice(Ty::I32),
                    kind,
                },
                ValueTy::Owned(AggregateTy::Enum(EnumId(7)))
            )
        );
        assert_eq!(
            BuiltinItem::Function(function).path(),
            ["std", "io", function.name()]
        );
        for member in 0..4 {
            assert_eq!(
                status.member_payload(member),
                (member
                    == if status == BuiltinEnum::ReadStatus {
                        0
                    } else {
                        2
                    })
                .then_some(Ty::I32)
            );
        }
    }
}

#[test]
fn bounded_stdout_private_index_all_nine_suffixes_queries_and_views() {
    for (input, output, expected) in output_inventory_cases() {
        for output_first in [false, true] {
            let imports = if output_first {
                format!("{output} {input}")
            } else {
                format!("{input} {output}")
            };
            // Source declarations deliberately duplicate the catalog's spellings
            // and shapes. Their IDs and origins must stay in the source prefix.
            let text = format!("{imports} enum ReadStatus{{Eof(i32),Full,IoError}} enum WriteStatus{{Complete,InvalidInput,IoError(i32)}} fn read_stdin()->(){{return;}} fn write_stdout()->(){{return;}} fn main()->(){{return;}}");
            let fixture = Fixture::new(&[("main.ox", &text)]);
            let project = fixture.load_output();
            let work = WorkMeter::default();
            let mut allocator = Allocator::default();
            let facts = output_facts(&project, &work, &mut allocator);
            assert_eq!(
                (
                    facts.plan().counts.functions,
                    facts.plan().counts.enums,
                    facts.plan().counts.variants
                ),
                (3, 2, 6)
            );
            assert!(facts.require_current_source_pipeline().is_err());
            assert!(facts.require_builtin_candidate_pipeline().is_ok());
            assert!(facts.require_no_builtin_candidate().is_err());
            let index = facts.finish(&work, &mut allocator).unwrap();
            assert!(index.require_current_source_pipeline().is_err());
            assert!(index.require_builtin_candidate_pipeline().is_ok());
            assert!(index.require_no_builtin_candidate().is_err());
            assert!(index.require_output_candidate_pipeline().is_ok());
            assert!(index.is_output_candidate_pipeline());
            assert_eq!(index.builtin_set(), expected);
            assert_eq!(index.root_original_main(), Some(DefId(2)));
            assert_eq!(index.enum_count(), 2 + expected.extra_enums());
            assert_eq!(index.function_count(), 3 + expected.extra_functions());
            assert_eq!(
                index.enum_variant_counts().collect::<Vec<_>>(),
                vec![3; index.enum_count()]
            );
            for id in 0..2 {
                assert_eq!(
                    index.enum_origin(EnumId(id)).unwrap(),
                    DeclarationOrigin::Source
                );
            }
            for id in 0..3 {
                assert_eq!(
                    index.function_origin(DefId(id)).unwrap(),
                    DeclarationOrigin::Source
                );
            }
            assert!(index.enum_origin(EnumId(index.enum_count())).is_err());
            assert!(index
                .function_origin(DefId(index.function_count()))
                .is_err());
            for item in BuiltinEnum::ALL {
                if let Some(rank) = expected.enum_rank(item) {
                    let id = index.builtin_enum_id(item).unwrap();
                    assert_eq!(id, EnumId(2 + rank));
                    assert_eq!(
                        index.enum_origin(id).unwrap(),
                        DeclarationOrigin::Builtin(BuiltinItem::Enum(item))
                    );
                    let view = index.enum_view(id).unwrap();
                    assert_eq!(view.name(), item.name());
                    assert_eq!(view.variant_count(), 3);
                    assert!(view.source_syntax().is_none());
                    assert_eq!(
                        index
                            .query(&work)
                            .prepare_nominal_type_name(NominalId::Enum(id), view.diagnostic_span())
                            .unwrap()
                            .to_string(),
                        format!("std::io::{}", item.name())
                    );
                    let source_enum = usize::from(item == BuiltinEnum::WriteStatus);
                    for member in 0..3 {
                        let variant_id = VariantId {
                            enumeration: id,
                            index: member,
                        };
                        let view = view.variant(variant_id).unwrap();
                        assert_eq!(view.name(), item.member_name(member).unwrap());
                        assert_eq!(view.payload(), item.member_payload(member));
                        assert_eq!(
                            view.origin(),
                            DeclarationOrigin::Builtin(BuiltinItem::Enum(item))
                        );
                        assert!(view.source_syntax().is_none());
                        let name_span = project.try_file_ast(SourceFileId(0)).unwrap().enums
                            [source_enum]
                            .variants[member]
                            .name;
                        assert_eq!(
                            index
                                .query(&work)
                                .tables
                                .lookup_variant(id, name_span, &work)
                                .unwrap(),
                            variant_id
                        );
                    }
                } else {
                    assert!(index.builtin_enum_id(item).is_err());
                    assert!(index.builtin_enum_anchor(item).is_err());
                }
            }
            for item in BuiltinFunction::ALL {
                if let Some(rank) = expected.function_rank(item) {
                    let id = index.builtin_function_id(item).unwrap();
                    assert_eq!(id, DefId(3 + rank));
                    assert_eq!(
                        index.function_origin(id).unwrap(),
                        DeclarationOrigin::Builtin(BuiltinItem::Function(item))
                    );
                    assert_eq!(
                        project.text(index.builtin_function_anchor(item).unwrap()),
                        item.name()
                    );
                } else {
                    assert!(index.builtin_function_id(item).is_err());
                    assert!(index.builtin_function_anchor(item).is_err());
                }
            }
            for (position, import) in project
                .try_file_ast(SourceFileId(0))
                .unwrap()
                .imports
                .iter()
                .enumerate()
            {
                let endpoint = *project
                    .try_file_ast(SourceFileId(0))
                    .unwrap()
                    .path_segments(import.path)
                    .unwrap()
                    .last()
                    .unwrap();
                match project.text(endpoint) {
                    "ReadStatus" | "WriteStatus" => {
                        let item = if project.text(endpoint) == "ReadStatus" {
                            BuiltinEnum::ReadStatus
                        } else {
                            BuiltinEnum::WriteStatus
                        };
                        assert_eq!(
                            index
                                .query(&work)
                                .nominal_type(
                                    ModuleId(0),
                                    bound(&project, 0, position),
                                    TypeContext::Value
                                )
                                .unwrap(),
                            NominalId::Enum(index.builtin_enum_id(item).unwrap())
                        );
                    }
                    "read_stdin" | "write_stdout" => {
                        let item = if project.text(endpoint) == "read_stdin" {
                            BuiltinFunction::ReadStdin
                        } else {
                            BuiltinFunction::WriteStdout
                        };
                        assert_eq!(
                            index
                                .query(&work)
                                .callee(ModuleId(0), bound(&project, 0, position), false)
                                .unwrap(),
                            index.builtin_function_id(item).unwrap()
                        );
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}

#[test]
fn bounded_stdout_current_output_admitted_but_historical_collections_stay_closed() {
    for endpoint in ["WriteStatus", "write_stdout"] {
        let text = format!("use std::io::{endpoint} as Output; fn main()->(){{return;}}");
        let fixture = Fixture::new(&[("main.ox", &text)]);
        let project = fixture.load_current();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let index = collect_originals(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
        assert!(index.is_current_source_pipeline());
        assert!(index.builtin_set().has_output());
        let project = fixture.load_output();
        for collect in [
            collect_builtin_candidate,
            collect_std_closed,
            collect_closed,
            collect_enum_candidate,
        ] {
            let mut allocator = Allocator::default();
            assert!(collect(
                SourceOwner::project(&project),
                IndexLimits::default(),
                &WorkMeter::default(),
                &mut allocator
            )
            .is_err());
            assert_eq!(allocator.attempts, 0, "laundered {endpoint}");
        }
    }
}

#[test]
fn bounded_stdout_private_index_all_nine_resource_boundaries() {
    for (input, output, expected) in output_inventory_cases() {
        let text = format!("{input} {output} fn main()->(){{return;}}");
        let fixture = Fixture::new(&[("main.ox", &text)]);
        let project = fixture.load_output();
        let work = WorkMeter::default();
        work.enable_observation();
        let mut allocator = Allocator::default();
        let facts = output_facts(&project, &work, &mut allocator);
        let plan = facts.plan();
        let mandatory = work
            .events
            .borrow()
            .iter()
            .take_while(|event| event.operation != "initialize admitted rows")
            .map(|event| event.units)
            .sum::<u64>()
            + plan.build_work;
        facts.finish(&work, &mut allocator).unwrap();
        for (retained, scratch, work_limit, ok) in [
            (plan.retained, plan.scratch, mandatory, true),
            (plan.retained - 1, plan.scratch, mandatory, false),
            (plan.retained, plan.scratch - 1, mandatory, false),
            (plan.retained, plan.scratch, mandatory - 1, false),
        ] {
            let mut allocator = Allocator::default();
            let work = WorkMeter::new(work_limit);
            let result = collect_output_candidate(
                SourceOwner::project(&project),
                IndexLimits {
                    retained,
                    scratch,
                    work: work_limit,
                },
                &work,
                &mut allocator,
            );
            assert_eq!(
                result.is_ok(),
                ok,
                "{expected:?}: retained={retained} scratch={scratch} work={work_limit}"
            );
            if ok {
                assert_eq!(
                    result
                        .unwrap()
                        .finish(&work, &mut allocator)
                        .unwrap()
                        .builtin_set(),
                    expected
                );
            } else {
                assert_eq!(allocator.attempts, 0);
                assert_eq!(result.unwrap_err().code, "E0400");
            }
        }
        println!("OUTPUT_PRIVATE_INDEX_ENDPOINT set={expected:?} retained={} scratch={} mandatory_work={mandatory}", plan.retained, plan.scratch);
    }
}

#[test]
fn bounded_stdout_private_function_dependency_does_not_bind_output_status() {
    let fixture = Fixture::new(&[("main.ox", "use std::io::write_stdout as output; fn helper()->WriteStatus{return;} fn main()->(){return;}")]);
    let project = fixture.load_output();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = output_facts(&project, &work, &mut allocator)
        .finish(&work, &mut allocator)
        .unwrap();
    assert_eq!(index.enum_count(), 1);
    let result = project.try_file_ast(SourceFileId(0)).unwrap().functions[0].result;
    assert_eq!(
        index
            .query(&work)
            .value_type(ModuleId(0), result, TypeContext::Value)
            .unwrap_err()
            .code,
        "E0202"
    );
}

#[test]
fn bounded_stdout_private_borrowed_anchors_preserve_dependency_and_module_order() {
    for first_function in [false, true] {
        let root = if first_function {
            "pub mod first; pub mod second; use std::io::read_stdin as input; use std::io::WriteStatus as OutStatus; fn main()->(){return;}"
        } else {
            "pub mod first; pub mod second; use std::io::ReadStatus as InStatus; use std::io::write_stdout as output; fn main()->(){return;}"
        };
        let child = if first_function {
            "use std::io::ReadStatus as InStatus; use std::io::write_stdout as output; pub fn helper()->(){return;}"
        } else {
            "use std::io::read_stdin as input; use std::io::WriteStatus as OutStatus; pub fn helper()->(){return;}"
        };
        let fixture = Fixture::new(&[("main.ox", root), ("first.ox", child), ("second.ox", "use std::io::write_stdout as again; use std::io::read_stdin as input; pub fn other()->(){return;}")]);
        let result = ProjectSources::load_output_candidate(
            fixture.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
            &mut Allocator::default(),
        );
        if !cfg!(target_os = "linux") {
            assert_module_policy_denial(
                result.unwrap_err(),
                Span {
                    file: SourceFileId(0),
                    start: 8,
                    end: 13,
                },
            );
            continue;
        }
        let project = result.unwrap();
        let owner = SourceOwner::project(&project);
        let endpoint = |module: usize, import: usize| {
            let ast = owner.ast(ModuleId(module)).unwrap();
            ast.path_segments(ast.imports[import].path)
                .unwrap()
                .last()
                .unwrap()
        };
        let expected = [
            endpoint(0, 0),
            if first_function {
                endpoint(0, 0)
            } else {
                endpoint(1, 0)
            },
            endpoint(0, 1),
            if first_function {
                endpoint(1, 1)
            } else {
                endpoint(0, 1)
            },
        ];
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let original = output_facts(&project, &work, &mut allocator)
            .finish(&work, &mut allocator)
            .unwrap();
        let moved = original;
        assert_eq!(moved.builtin_set(), BuiltinSet::ReadStdinWriteStdout);
        for (actual, expected) in moved
            .builtin_anchor_borrows_for_test()
            .into_iter()
            .zip(expected)
        {
            assert!(std::ptr::eq(actual.unwrap(), expected));
            assert_eq!(
                owner.text(*actual.unwrap()).unwrap(),
                owner.text(*expected).unwrap()
            );
        }
        assert_eq!(
            moved.builtin_enum_anchor(BuiltinEnum::ReadStatus).unwrap(),
            *expected[0]
        );
        assert_eq!(
            moved
                .builtin_function_anchor(BuiltinFunction::ReadStdin)
                .unwrap(),
            *expected[1]
        );
        assert_eq!(
            moved.builtin_enum_anchor(BuiltinEnum::WriteStatus).unwrap(),
            *expected[2]
        );
        assert_eq!(
            moved
                .builtin_function_anchor(BuiltinFunction::WriteStdout)
                .unwrap(),
            *expected[3]
        );
        assert!(expected[usize::from(first_function) * 2 + 1].file != SourceFileId(0));
    }
}

#[test]
fn bounded_stdout_private_borrowed_anchors_choose_first_within_module() {
    for function_first in [false, true] {
        let text = if function_first {
            "use std::io::write_stdout as output; use std::io::read_stdin as input; use std::io::WriteStatus as OutStatus; use std::io::ReadStatus as InStatus; fn main()->(){return;}"
        } else {
            "use std::io::WriteStatus as OutStatus; use std::io::ReadStatus as InStatus; use std::io::write_stdout as output; use std::io::read_stdin as input; fn main()->(){return;}"
        };
        let fixture = Fixture::new(&[("main.ox", text)]);
        let project = fixture.load_output();
        let ast = project.try_file_ast(SourceFileId(0)).unwrap();
        let endpoint = |import: usize| {
            ast.path_segments(ast.imports[import].path)
                .unwrap()
                .last()
                .unwrap()
        };
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let index = output_facts(&project, &work, &mut allocator)
            .finish(&work, &mut allocator)
            .unwrap();
        let expected = [
            endpoint(1),
            endpoint(if function_first { 1 } else { 3 }),
            endpoint(0),
            endpoint(if function_first { 0 } else { 2 }),
        ];
        for (actual, expected) in index
            .builtin_anchor_borrows_for_test()
            .into_iter()
            .zip(expected)
        {
            assert!(std::ptr::eq(actual.unwrap(), expected));
        }
    }
}

#[test]
fn bounded_stdout_original_owner_keeps_absent_inventory_and_rejects_std_ast() {
    use crate::frontend::{lexer, source::SourceMap};
    for text in [
        "fn main()->(){return;}",
        "use std::io::write_stdout; fn main()->(){return;}",
        "use std::io::read_stdin; fn main()->(){return;}",
    ] {
        let mut map = SourceMap::new();
        map.add("padding.ox".into(), String::new());
        let id = map.add("original.ox".into(), text.into());
        let file = map.get(id);
        let ast = parser::parse_output_candidate_counted(
            file,
            lexer::lex(file).unwrap(),
            parser::SourceMode::ProjectCandidate,
            parser::MAX_NODES,
            &mut Allocator::default(),
            &mut Default::default(),
        )
        .unwrap()
        .0;
        let owner = SourceOwner::original(file, &ast, SourceView::Map(&map)).unwrap();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let result = collect_output_candidate(owner, IndexLimits::default(), &work, &mut allocator);
        if text.starts_with("use") {
            assert_eq!(result.unwrap_err().code, "E0500");
            assert_eq!(allocator.attempts, 0);
        } else {
            let index = result.unwrap().finish(&work, &mut allocator).unwrap();
            assert_eq!(index.builtin_set(), BuiltinSet::None);
            assert_eq!(index.builtin_anchor_borrows_for_test(), [None; 4]);
            assert_eq!(index.function(DefId(0)).unwrap().0.file, id);
            assert!(index.require_current_source_pipeline().is_err());
            assert!(index.require_builtin_candidate_pipeline().is_ok());
        }
    }
}

#[test]
fn bounded_stdout_private_invalid_imports_and_duplicate_aliases_fail_closed() {
    for text in [
        "use std::fs::write_stdout; fn main()->(){return;}",
        "use std::io::missing; fn main()->(){return;}",
        "use std::io::WriteStatus::Complete; fn main()->(){return;}",
    ] {
        let fixture = Fixture::new(&[("main.ox", text)]);
        let project = fixture.load_output();
        let mut allocator = Allocator::default();
        assert_eq!(
            collect_output_candidate(
                SourceOwner::project(&project),
                IndexLimits::default(),
                &WorkMeter::default(),
                &mut allocator
            )
            .unwrap_err()
            .code,
            "E0205"
        );
        assert_eq!(allocator.attempts, 0);
    }
    for text in [
        "use std::io::WriteStatus as A; use std::io::WriteStatus as B; fn main()->(){return;}",
        "use std::io::write_stdout as a; use std::io::write_stdout as b; fn main()->(){return;}",
        "use std::io::ReadStatus as Status; use std::io::WriteStatus as Status; fn main()->(){return;}",
        "use std::io::read_stdin as io; use std::io::write_stdout as io; fn main()->(){return;}",
        "use std::io::WriteStatus as bool; fn main()->(){return;}",
        "use std::io::write_stdout as main; fn main()->(){return;}",
    ] {
        let fixture = Fixture::new(&[("main.ox", text)]);
        let project = fixture.load_output();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let errors = output_facts(&project, &work, &mut allocator).finish(&work, &mut allocator).unwrap_err();
        assert!(errors.iter().any(|error| matches!(error.code, "E0201" | "E0202")));
    }
}

#[test]
fn bounded_stdout_private_allocation_failures_never_freeze_identity() {
    let fixture = Fixture::new(&[("main.ox", "use std::io::ReadStatus as InStatus; use std::io::read_stdin as input; use std::io::WriteStatus as OutStatus; use std::io::write_stdout as output; enum User{V} fn main()->(){return;}")]);
    let project = fixture.load_output();
    let mut baseline = Allocator::default();
    output_facts(&project, &WorkMeter::default(), &mut baseline);
    for fail_at in 1..=baseline.attempts {
        let work = WorkMeter::default();
        work.enable_observation();
        let mut allocator = Allocator {
            fail_at: Some(fail_at),
            ..Allocator::default()
        };
        assert_eq!(
            collect_output_candidate(
                SourceOwner::project(&project),
                IndexLimits::default(),
                &work,
                &mut allocator
            )
            .unwrap_err()
            .code,
            "E0400"
        );
        assert_eq!(allocator.attempts, fail_at);
        assert!(!work
            .observations
            .borrow()
            .iter()
            .any(|event| matches!(event, Observation::Frozen { .. })));
    }
}
