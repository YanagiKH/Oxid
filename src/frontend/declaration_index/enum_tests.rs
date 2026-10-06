//! C2a index-only controls. No enum source executable witness is admitted.
//! Real child-module inputs require the production Linux filesystem policy.
//! Single-file index and carrier controls remain applicable on every host.
use super::*;
use crate::frontend::{hir, project::ProjectLimits};
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
            "oxid-c2a-{}-{}",
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
fn collect<'s>(
    sources: &'s ProjectSources,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> DeclarationFacts<'s> {
    collect_enum_candidate(
        SourceOwner::project(sources),
        IndexLimits::default(),
        work,
        allocator,
    )
    .unwrap()
}
#[cfg(target_os = "linux")]
fn bound(project: &ProjectSources, alias: usize) -> ItemPathRef {
    let import = &project.try_file_ast(SourceFileId(0)).unwrap().imports[alias];
    ItemPathRef {
        file: SourceFileId(0),
        path: ast::ItemPath::Unqualified(import.alias),
    }
}
#[cfg(target_os = "linux")]
fn identity_fixture() -> Fixture {
    Fixture::new(&[
        ("main.ox", "fn pre()->i32{return 0;} enum RootE{Z,U(())} pub struct RootR{pub x:i32} pub mod branch; pub enum RootTail{B(bool)} use crate::branch::Pair as Imported; use crate::branch::ChildR as ImportedR; fn main()->i32{return 0;}"),
        ("branch.ox", "pub struct ChildR{pub flag:bool} pub fn Pair()->i32{return 1;} pub enum Pair{No,Number(i32)} pub mod leaf; pub enum Last{Done} pub fn child_after()->(){return;}"),
        ("branch/leaf.ox", "pub enum LeafE{L} pub struct LeafR{pub unit:()} pub fn leaf_fn()->(){return;}"),
    ])
}
#[cfg(target_os = "linux")]
#[test]
fn enum_index_interleaved_identity_views_and_original_alias_targets() {
    let fixture = identity_fixture();
    let project = fixture.load();
    let work = WorkMeter::default();
    work.enable_observation();
    let mut allocator = Allocator::default();
    let facts = collect(&project, &work, &mut allocator);
    let plan = facts.plan();
    assert_eq!(
        (
            plan.counts.originals,
            plan.counts.functions,
            plan.counts.records,
            plan.counts.fields,
            plan.counts.enums,
            plan.counts.variants
        ),
        (15, 5, 3, 3, 5, 7)
    );
    assert_eq!(plan.counts.variant_duplicate_work, 14);
    let expected = size_of::<DeclarationIndex<'_>>() as u64
        + 15 * 40
        + 5 * 12
        + 3 * 28
        + 3 * 16
        + 5 * 20
        + 7 * 12
        + 3 * 68
        + 2 * 4
        + 2 * 40;
    assert_eq!(plan.retained, expected);
    assert_eq!(plan.scratch, FIXED_SCRATCH as u64 + 96);
    let index = facts.finish(&work, &mut allocator).unwrap();
    assert_eq!(
        index.complete_row_lengths(),
        [15, 15, 5, 3, 3, 3, 2, 2, 2, 2, 5, 7]
    );
    for (id, (file, local)) in [(0, 0), (0, 1), (1, 0), (1, 1), (2, 0)]
        .into_iter()
        .enumerate()
    {
        let (key, owner) = index.enumeration(EnumId(id)).unwrap();
        assert_eq!((key.file.0, key.index, owner.0), (file, local, file));
        assert_eq!(index.enum_for(key).unwrap(), EnumId(id));
    }
    for (id, (file, local)) in [(0, 0), (1, 0), (2, 0)].into_iter().enumerate() {
        let (key, _) = index.record(RecordId(id)).unwrap();
        assert_eq!((key.file.0, key.index), (file, local));
    }
    for (id, (file, local)) in [(0, 0), (0, 1), (1, 0), (1, 1), (2, 0)]
        .into_iter()
        .enumerate()
    {
        let (key, _) = index.function(DefId(id)).unwrap();
        assert_eq!((key.file.0, key.index), (file, local));
    }
    assert_eq!(
        index.enum_variant_counts().collect::<Vec<_>>(),
        [2, 1, 2, 1, 1]
    );
    let mut query = index.query(&work);
    assert_eq!(
        query.select(ModuleId(0), bound(&project, 0), true).unwrap(),
        Some(8)
    );
    assert_eq!(
        query.select(ModuleId(0), bound(&project, 1), true).unwrap(),
        Some(6)
    );
    assert_eq!(
        query
            .callee(ModuleId(0), bound(&project, 0), false)
            .unwrap(),
        DefId(2)
    );
    assert_eq!(
        query
            .record_type(ModuleId(0), bound(&project, 1), TypeContext::Value)
            .unwrap(),
        RecordId(1)
    );
    assert!(query
        .record_type(ModuleId(0), bound(&project, 0), TypeContext::Reference)
        .is_err());
    let root = index.enum_view(EnumId(0)).unwrap();
    assert_eq!(root.id(), EnumId(0));
    assert_eq!(
        root.variant(VariantId {
            enumeration: EnumId(0),
            index: 0
        })
        .unwrap()
        .payload(),
        None
    );
    let unit = root
        .variant(VariantId {
            enumeration: EnumId(0),
            index: 1,
        })
        .unwrap();
    assert_eq!(unit.payload(), Some(Ty::Unit));
    assert_eq!(project.text(unit.payload_span().unwrap()), "()");
    assert!(root
        .variant(VariantId {
            enumeration: EnumId(2),
            index: 1
        })
        .is_err());
    assert!(root
        .variant(VariantId {
            enumeration: EnumId(0),
            index: 2
        })
        .is_err());
    for (enumeration, ordinal, payload) in [(1, 0, Ty::Bool), (2, 1, Ty::I32)] {
        assert_eq!(
            index
                .enum_view(EnumId(enumeration))
                .unwrap()
                .variant(VariantId {
                    enumeration: EnumId(enumeration),
                    index: ordinal
                })
                .unwrap()
                .payload(),
            Some(payload)
        );
    }
    assert!(index.enum_view(EnumId(usize::MAX)).is_err());
    assert!(index
        .enum_for(EnumAstKey {
            file: SourceFileId(99),
            index: 0
        })
        .is_err());
    assert!(index
        .enum_for(EnumAstKey {
            file: SourceFileId(2),
            index: 1
        })
        .is_err());
    let events = work.observations.borrow();
    let imports: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Observation::NominalImport { ty, value, .. } => Some((*ty, *value)),
            _ => None,
        })
        .collect();
    assert_eq!(
        imports,
        [
            (Some(NominalId::Enum(EnumId(2))), Some(DefId(2))),
            (Some(NominalId::Record(RecordId(1))), None)
        ]
    );
    assert!(!events
        .iter()
        .any(|event| matches!(event, Observation::Import { .. })));
}

#[test]
fn enum_index_closed_collection_and_all_direct_source_producers_stay_closed() {
    for text in [
        "enum E{V} fn main()->(){return;}",
        "fn main()->(){E::V;return;}",
        "fn main()->(){match x{E::V=>{}} return;}",
        "fn helper()->(){return;} fn main()->(){crate::helper();return;}",
    ] {
        let fixture = Fixture::new(&[("main.ox", text)]);
        let project = fixture.load();
        let work = WorkMeter::default();
        work.enable_observation();
        let mut allocator = Allocator::default();
        let error = collect_closed(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap_err();
        assert_eq!((error.code, error.stage), ("E0101", "resolve"));
        assert_eq!(allocator.attempts, 0);
        let facts = collect(&project, &work, &mut allocator);
        let signature_error = hir::original_signatures(&facts, &work).unwrap_err();
        assert_eq!(signature_error[0].code, "E0101");
        let index = facts.finish(&work, &mut allocator).unwrap();
        for errors in [
            hir::resolve_project(&index, &work).unwrap_err(),
            hir::resolve_bodies(&index, &work, Vec::new()).unwrap_err(),
        ] {
            assert_eq!(errors[0].code, "E0101");
        }
        assert!(!work.observations.borrow().iter().any(|event| matches!(
            event,
            Observation::SignatureStart { .. } | Observation::RecordStart { .. }
        )));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn enum_index_closed_collection_reports_imported_enum_origin() {
    let fixture = Fixture::new(&[
        ("main.ox", "mod child; fn main()->(){return;}"),
        ("child.ox", "enum Unused{V}"),
    ]);
    let project = fixture.load();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let facts = collect(&project, &work, &mut allocator);
    assert_eq!(
        facts
            .require_current_source_pipeline()
            .unwrap_err()
            .primary
            .unwrap()
            .file,
        SourceFileId(1)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn enum_index_observation_schema_uses_complete_enum_identity_table_only() {
    for with_enum in [false, true] {
        let child = format!(
            "pub fn f()->(){{return;}} pub struct R{{}} {}",
            if with_enum { "enum Spare{V}" } else { "" }
        );
        let fixture=Fixture::new(&[("main.ox","fn z()->(){return;} mod m; use crate::m::R as A; fn main()->(){crate::m::f();return;}"),("m.ox",&child)]);
        let project = fixture.load();
        let work = WorkMeter::default();
        work.enable_observation();
        let mut allocator = Allocator::default();
        let facts = collect(&project, &work, &mut allocator);
        assert_eq!(
            facts.require_current_source_pipeline().unwrap_err().code,
            "E0101"
        );
        let index = facts.finish(&work, &mut allocator).unwrap();
        assert_eq!(
            index
                .query(&work)
                .select(ModuleId(0), bound(&project, 0), true)
                .unwrap(),
            Some(4)
        );
        let events = work.observations.borrow();
        if with_enum {
            assert!(events.iter().any(|e| matches!(
                e,
                Observation::NominalImport {
                    ty: Some(NominalId::Record(RecordId(0))),
                    ..
                }
            )));
            assert!(!events
                .iter()
                .any(|e| matches!(e, Observation::Import { .. })));
        } else {
            assert!(events.iter().any(|e| matches!(
                e,
                Observation::Import {
                    ty: Some(RecordId(0)),
                    ..
                }
            )));
            assert!(!events
                .iter()
                .any(|e| matches!(e, Observation::NominalImport { .. })));
        }
    }
}

#[test]
fn enum_index_nominal_legacy_wrapper_rejects_enums() {
    assert_eq!(NominalId::Record(RecordId(7)).legacy_record(), RecordId(7));
    assert!(std::panic::catch_unwind(|| NominalId::Enum(EnumId(7)).legacy_record()).is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn enum_index_paired_alias_rejection_does_not_poison_later_import() {
    let fixture = Fixture::new(&[
        (
            "main.ox",
            "mod m; use crate::m::Pair as bool; use crate::m::Pair as Good; fn main()->(){return;}",
        ),
        ("m.ox", "pub enum Pair{V} pub fn Pair()->i32{return 1;}"),
    ]);
    let project = fixture.load();
    let work = WorkMeter::default();
    work.enable_observation();
    let mut allocator = Allocator::default();
    let errors = collect(&project, &work, &mut allocator)
        .finish(&work, &mut allocator)
        .unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "E0202");
    let events = work.observations.borrow();
    let mut imports = events.iter().filter_map(|event| {
        if let Observation::NominalImport {
            committed,
            ty,
            value,
            aliases,
            seen,
            ..
        } = event
        {
            Some((*committed, *ty, *value, aliases, seen))
        } else {
            None
        }
    });
    let first = imports.next().unwrap();
    assert_eq!((first.0, first.1, first.2), (false, None, None));
    assert!(first.3.iter().all(|a| a.ty.is_none()
        && a.value.is_none()
        && a.type_first.is_none()
        && a.value_first.is_none()));
    assert!(first
        .4
        .iter()
        .all(|a| a.type_first.is_none() && a.value_first.is_none()));
    let second = imports.next().unwrap();
    assert_eq!(
        (second.0, second.1, second.2),
        (true, Some(NominalId::Enum(EnumId(0))), Some(DefId(1)))
    );
    assert!(!events
        .iter()
        .any(|event| matches!(event, Observation::Frozen { .. })));
}

#[test]
fn enum_index_duplicate_unused_variants_and_type_collisions_reject() {
    for (text, code) in [
        ("enum E{Same,Same} fn main()->(){return;}", "E0201"),
        ("enum E{V} struct E{} fn main()->(){return;}", "E0201"),
        ("enum bool{V} fn main()->(){return;}", "E0202"),
    ] {
        let fixture = Fixture::new(&[("main.ox", text)]);
        let project = fixture.load();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let errors = collect(&project, &work, &mut allocator)
            .finish(&work, &mut allocator)
            .unwrap_err();
        assert_eq!(errors[0].code, code);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn enum_index_complete_rows_have_preallocation_caps_and_all_reserve_failures() {
    let fixture = identity_fixture();
    let project = fixture.load();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let facts = collect(&project, &work, &mut allocator);
    let plan = facts.plan();
    assert_eq!(allocator.attempts, 16);
    assert!(allocator
        .trace
        .iter()
        .any(|event| event.kind == "index enums"
            && event.length == 5
            && event.element_bytes == 20));
    assert!(allocator
        .trace
        .iter()
        .any(|event| event.kind == "index variants"
            && event.length == 7
            && event.element_bytes == 12));
    for (retained, scratch, ok) in [
        (plan.retained, plan.scratch, true),
        (plan.retained - 1, plan.scratch, false),
        (plan.retained, plan.scratch - 1, false),
    ] {
        let mut allocator = Allocator::default();
        let result = collect_enum_candidate(
            SourceOwner::project(&project),
            IndexLimits {
                retained,
                scratch,
                ..IndexLimits::default()
            },
            &WorkMeter::default(),
            &mut allocator,
        );
        assert_eq!(result.is_ok(), ok);
        if !ok {
            assert_eq!(allocator.attempts, 0);
        }
    }
    for fail_at in 1..=16 {
        let mut allocator = Allocator {
            fail_at: Some(fail_at),
            ..Allocator::default()
        };
        assert_eq!(
            collect_enum_candidate(
                SourceOwner::project(&project),
                IndexLimits::default(),
                &WorkMeter::default(),
                &mut allocator
            )
            .unwrap_err()
            .code,
            "E0400"
        );
        assert_eq!(allocator.attempts, fail_at);
    }
}

#[test]
fn enum_index_actual_carriers_and_enum_free_header_growth() {
    println!(
        "enum-index-count-summary-layout RecordUsage={} EnumUsage={} fixed={}",
        size_of::<crate::frontend::oir::owned_types::DeclarationUsage>(),
        size_of::<crate::frontend::oir::owned_types::EnumUsage>(),
        FIXED_SCRATCH
    );
    println!("enum-index-layout Tables={} Index={} Facts={} Counts={} Plan={} Scratch={} Fixed={} Origin={} EnumRow={} VariantRow={} ModuleRow={} NominalId={} EnumView={} VariantView={} EnumViewOption={} VariantViewOption={} EnumCounts={} SourceCounts={}",size_of::<Tables<'_>>(),size_of::<DeclarationIndex<'_>>(),size_of::<DeclarationFacts<'_>>(),size_of::<Counts>(),size_of::<IndexPlan>(),size_of::<Scratch>(),FIXED_SCRATCH,size_of::<Option<CompactSpan>>(),size_of::<EnumRow>(),size_of::<VariantRow>(),size_of::<ModuleRow>(),size_of::<NominalId>(),size_of::<EnumView<'_>>(),size_of::<VariantView<'_>>(),size_of::<Option<EnumView<'_>>>(),size_of::<Option<VariantView<'_>>>(),size_of::<EnumVariantCounts<'_>>(),size_of::<sealed::EnumSourceCounts<'_>>());
    const { assert!(FIXED_SCRATCH > 3592) };
    const { assert!(FIXED_SCRATCH <= 4096) };
    let fixture = Fixture::new(&[("main.ox", "fn main()->(){return;}")]);
    let project = fixture.load();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let facts = collect(&project, &work, &mut allocator);
    let plan = facts.plan();
    assert_eq!(
        plan.retained,
        size_of::<DeclarationIndex<'_>>() as u64 + 40 + 12 + 68
    );
    assert_eq!(plan.scratch, FIXED_SCRATCH as u64 + 8);
    assert!(facts.require_current_source_pipeline().is_ok());
}

#[test]
fn enum_index_legacy_row_observation_is_exact_and_fails_closed_on_enums() {
    let fixture = Fixture::new(&[("main.ox", "fn main()->(){return;}")]);
    let project = fixture.load();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect(&project, &work, &mut allocator)
        .finish(&work, &mut allocator)
        .unwrap();
    assert_eq!(index.row_lengths(), [1, 1, 1, 0, 0, 1, 0, 0, 0, 0]);
    assert_eq!(
        index.complete_row_lengths(),
        [1, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0]
    );
    let fixture = Fixture::new(&[("main.ox", "enum E{V}")]);
    let project = fixture.load();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect(&project, &work, &mut allocator)
        .finish(&work, &mut allocator)
        .unwrap();
    assert!(std::panic::catch_unwind(|| index.row_lengths()).is_err());
    assert_eq!(
        index.complete_row_lengths(),
        [1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 1, 1]
    );
}

#[cfg(target_os = "linux")]
#[test]
fn enum_index_combined_declaration_limit_spans_real_modules() {
    let mut root = String::from("mod child;");
    for id in 0..2048 {
        root.push_str(&format!("struct R{id}{{}}"));
    }
    for (enum_count, ok) in [(2047, true), (2048, true), (2049, false)] {
        let child = (0..enum_count)
            .map(|id| format!("enum E{id}{{V}}"))
            .collect::<String>();
        let fixture = Fixture::new(&[("main.ox", &root), ("child.ox", &child)]);
        let project = fixture.load();
        assert!(project.usage().syntax_nodes < crate::frontend::parser::MAX_NODES);
        assert!(project.usage().non_eof_tokens < crate::frontend::lexer::MAX_TOKENS);
        let mut allocator = Allocator::default();
        let result = collect_enum_candidate(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut allocator,
        );
        assert_eq!(result.is_ok(), ok, "2048 records + {enum_count} enums");
        if !ok {
            assert_eq!(result.unwrap_err().code, "E0400");
            assert_eq!(allocator.attempts, 0);
        }
    }
}

#[test]
fn enum_index_combined_member_envelope_is_count_only_not_a_token_limit_test() {
    use crate::frontend::oir::owned_types::{admit_declaration_counts, admit_enum_counts};
    // These member maxima exceed the source token cap; this deliberately tests counts only.
    for (last_fields, ok) in [(767, true), (768, true), (769, false)] {
        let mut fields = vec![1024; 64];
        fields[63] = last_fields;
        let records = admit_declaration_counts(fields.into_iter()).unwrap();
        assert_eq!(admit_enum_counts([256].into_iter(), records).is_ok(), ok);
    }
    assert!(admit_declaration_counts([1025].into_iter()).is_err());
    for (variants, ok) in [(0, false), (1, true), (256, true), (257, false)] {
        let records = admit_declaration_counts(std::iter::empty()).unwrap();
        assert_eq!(
            admit_enum_counts([variants].into_iter(), records).is_ok(),
            ok
        );
    }
}

#[test]
fn enum_index_duplicate_scan_has_independent_pair_and_byte_work() {
    let fixture = Fixture::new(&[("main.ox", "enum E{AA,AB,B}")]);
    let project = fixture.load();
    let work = WorkMeter::default();
    work.enable_observation();
    let mut allocator = Allocator::default();
    let facts = collect(&project, &work, &mut allocator);
    let before = work.events.borrow().len();
    facts.finish(&work, &mut allocator).unwrap();
    let events = work.events.borrow();
    let pairs: Vec<_> = events[before..]
        .iter()
        .filter(|event| event.operation == "variant duplicate pair")
        .collect();
    assert_eq!(pairs.len(), 3);
    // At each of three pairs: one explicit pair debit, one comparison invocation,
    // followed by bytes AA/AB=2, AA/B=1, AB/B=1. No import/sort occurs afterward.
    let begin = events[before..]
        .iter()
        .position(|event| event.operation == "variant duplicate pair")
        .unwrap()
        + before;
    let actual: u64 = events[begin..]
        .iter()
        .filter(|event| {
            matches!(
                event.operation,
                "variant duplicate pair" | "comparison" | "compared byte"
            )
        })
        .map(|event| event.units)
        .sum();
    assert_eq!(actual, 10);
}

#[test]
fn enum_index_tiny_new_and_widened_rows_have_exact_capacity() {
    for (text, expected) in [
        ("enum E{V}", [1, 1, 1]),
        ("fn main()->(){return;}", [0, 0, 1]),
    ] {
        let fixture = Fixture::new(&[("main.ox", text)]);
        let project = fixture.load();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let index = collect(&project, &work, &mut allocator)
            .finish(&work, &mut allocator)
            .unwrap();
        assert_eq!(index.enum_scoped_row_capacities(), expected);
        assert_eq!(allocator.attempts, 16);
        let bytes: usize = allocator
            .trace
            .iter()
            .filter(|event| {
                matches!(
                    event.kind,
                    "index enums" | "index variants" | "index modules"
                )
            })
            .map(|event| event.length * event.element_bytes)
            .sum();
        assert_eq!(
            bytes,
            expected[0] * 20 + expected[1] * 12 + expected[2] * 68
        );
    }
}

#[cfg(target_os = "linux")]
type ImportSnapshot = (
    bool,
    Option<NominalId>,
    Option<DefId>,
    Vec<NominalAliasObservation>,
    Vec<SeenObservation>,
);
#[cfg(target_os = "linux")]
fn nominal_snapshots(work: &WorkMeter) -> Vec<ImportSnapshot> {
    work.observations
        .borrow()
        .iter()
        .filter_map(|event| {
            if let Observation::NominalImport {
                committed,
                ty,
                value,
                aliases,
                seen,
                ..
            } = event
            {
                Some((*committed, *ty, *value, aliases.clone(), seen.clone()))
            } else {
                None
            }
        })
        .collect()
}
#[cfg(target_os = "linux")]
fn empty_alias_snapshot(snapshot: &ImportSnapshot) {
    assert!(!snapshot.0);
    assert_eq!((snapshot.1, snapshot.2), (None, None));
    assert!(snapshot.3.iter().all(|row| row.ty.is_none()
        && row.value.is_none()
        && row.type_first.is_none()
        && row.value_first.is_none()));
    assert!(snapshot
        .4
        .iter()
        .all(|row| row.type_first.is_none() && row.value_first.is_none()));
}

#[cfg(target_os = "linux")]
#[test]
fn enum_index_paired_second_lane_and_privacy_failures_commit_nothing() {
    for (root, child, code) in [
        (
            "mod m; use crate::m::Pair as A; fn A()->(){return;} fn main()->(){return;}",
            "pub enum Pair{V} pub fn Pair()->i32{return 1;}",
            "E0201",
        ),
        (
            "mod m; use crate::m::Pair as A; fn main()->(){return;}",
            "pub enum Pair{V} fn Pair()->i32{return 1;}",
            "E0206",
        ),
        (
            "mod m; use crate::m::Pair as A; fn main()->(){return;}",
            "enum Pair{V} pub fn Pair()->i32{return 1;}",
            "E0206",
        ),
    ] {
        let fixture = Fixture::new(&[("main.ox", root), ("m.ox", child)]);
        let project = fixture.load();
        let work = WorkMeter::default();
        work.enable_observation();
        let mut allocator = Allocator::default();
        let errors = collect(&project, &work, &mut allocator)
            .finish(&work, &mut allocator)
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, code);
        let snapshots = nominal_snapshots(&work);
        assert_eq!(snapshots.len(), 1);
        empty_alias_snapshot(&snapshots[0]);
        assert!(!work
            .observations
            .borrow()
            .iter()
            .any(|e| matches!(e, Observation::Frozen { .. })));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn enum_index_second_alias_lane_collision_preserves_the_first_value() {
    let fixture = Fixture::new(&[
        (
            "main.ox",
            "mod m; use crate::m::Only as A; use crate::m::Pair as A; fn main()->(){return;}",
        ),
        (
            "m.ox",
            "pub fn Only()->(){return;} pub enum Pair{V} pub fn Pair()->(){return;}",
        ),
    ]);
    let project = fixture.load();
    let work = WorkMeter::default();
    work.enable_observation();
    let mut allocator = Allocator::default();
    let errors = collect(&project, &work, &mut allocator)
        .finish(&work, &mut allocator)
        .unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "E0201");
    let snapshots = nominal_snapshots(&work);
    assert_eq!(snapshots.len(), 2);
    assert_eq!(
        (snapshots[0].0, snapshots[0].1, snapshots[0].2),
        (true, None, Some(DefId(1)))
    );
    assert_eq!(
        (snapshots[1].0, snapshots[1].1, snapshots[1].2),
        (false, None, None)
    );
    for snapshot in &snapshots {
        assert_eq!(snapshot.3.len(), 1);
        let alias = &snapshot.3[0];
        assert_eq!(
            (alias.ty, alias.value, alias.type_first, alias.value_first),
            (None, Some(DefId(1)), None, Some(0))
        );
        assert_eq!(snapshot.4.len(), 2);
        assert_eq!(
            (snapshot.4[0].type_first, snapshot.4[0].value_first),
            (None, Some(0))
        );
        assert_eq!(
            (snapshot.4[1].type_first, snapshot.4[1].value_first),
            (None, None)
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn enum_index_repeated_pair_target_preserves_prior_alias_and_seen() {
    let fixture = Fixture::new(&[
        (
            "main.ox",
            "mod m; use crate::m::Pair as A; use crate::m::Pair as B; fn main()->(){return;}",
        ),
        ("m.ox", "pub enum Pair{V} pub fn Pair()->(){return;}"),
    ]);
    let project = fixture.load();
    let work = WorkMeter::default();
    work.enable_observation();
    let mut allocator = Allocator::default();
    let errors = collect(&project, &work, &mut allocator)
        .finish(&work, &mut allocator)
        .unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "E0201");
    let snapshots = nominal_snapshots(&work);
    assert_eq!(snapshots.len(), 2);
    assert_eq!(
        (snapshots[0].0, snapshots[0].1, snapshots[0].2),
        (true, Some(NominalId::Enum(EnumId(0))), Some(DefId(1)))
    );
    assert!(!snapshots[1].0);
    for snapshot in &snapshots {
        assert_eq!(snapshot.3.len(), 2);
        let first = &snapshot.3[0];
        let second = &snapshot.3[1];
        assert_eq!(
            (first.ty, first.value, first.type_first, first.value_first),
            (
                Some(NominalId::Enum(EnumId(0))),
                Some(DefId(1)),
                Some(0),
                Some(0)
            )
        );
        assert_eq!(
            (
                second.ty,
                second.value,
                second.type_first,
                second.value_first
            ),
            (None, None, None, None)
        );
        assert_eq!(snapshot.4.len(), 1);
        assert_eq!(
            (snapshot.4[0].type_first, snapshot.4[0].value_first),
            (Some(0), Some(0))
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn enum_index_every_import_stage_debit_failure_has_atomic_snapshots() {
    let fixture=Fixture::new(&[("main.ox","mod m; use crate::m::Pair as A; use crate::m::Other as B; fn main()->(){return;}"),("m.ox","pub enum Pair{V} pub fn Pair()->(){return;} pub enum Other{W} pub fn Other()->(){return;}")]);
    let project = fixture.load();
    let baseline = WorkMeter::default();
    baseline.enable_observation();
    let mut allocator = Allocator::default();
    let facts = collect(&project, &baseline, &mut allocator);
    let collected_work = baseline.used();
    let event_start = baseline.events.borrow().len();
    facts.finish(&baseline, &mut allocator).unwrap();
    let mut boundaries = Vec::new();
    let mut consumed = collected_work;
    let mut active = None;
    let mut transaction = 0;
    for event in &baseline.events.borrow()[event_start..] {
        if event.operation == "import transaction" {
            active = Some(transaction);
            transaction += 1;
        }
        if let Some(id) = active {
            assert!(event.units > 0);
            boundaries.push((
                consumed + event.units - 1,
                event.origin,
                id,
                event.operation,
            ));
        }
        consumed += event.units;
        if event.operation == "import staging" {
            active = None;
        }
    }
    assert_eq!(transaction, 2);
    assert!(boundaries.iter().any(|row| row.3 == "target permission"));
    assert!(boundaries
        .iter()
        .any(|row| row.3 == "import repeated target"));
    assert!(boundaries.iter().any(|row| row.3 == "import staging"));
    for (limit, origin, failing_import, operation) in boundaries {
        let work = WorkMeter::default();
        work.enable_observation();
        let mut allocator = Allocator::default();
        let facts = collect(&project, &work, &mut allocator);
        assert_eq!(work.used(), collected_work);
        // Stage-local boundary: collection was admitted under the default meter.
        work.restrict(limit);
        let errors = facts.finish(&work, &mut allocator).unwrap_err();
        assert_eq!(errors[0].code, "E0400", "{operation}");
        assert_eq!(errors[0].primary, Some(origin), "{operation}");
        let snapshots = nominal_snapshots(&work);
        assert_eq!(snapshots.len(), failing_import + 1, "{operation}");
        let last = snapshots.last().unwrap();
        assert_eq!((last.0, last.1, last.2), (false, None, None));
        if failing_import == 0 {
            empty_alias_snapshot(last);
        } else {
            assert_eq!(last.3.len(), 2);
            assert_eq!(
                (
                    last.3[0].ty,
                    last.3[0].value,
                    last.3[0].type_first,
                    last.3[0].value_first
                ),
                (
                    Some(NominalId::Enum(EnumId(0))),
                    Some(DefId(1)),
                    Some(0),
                    Some(0)
                )
            );
            assert_eq!(
                (
                    last.3[1].ty,
                    last.3[1].value,
                    last.3[1].type_first,
                    last.3[1].value_first
                ),
                (None, None, None, None)
            );
            assert_eq!(
                (last.4[0].type_first, last.4[0].value_first),
                (Some(0), Some(0))
            );
            assert_eq!((last.4[1].type_first, last.4[1].value_first), (None, None));
        }
        assert!(!work
            .observations
            .borrow()
            .iter()
            .any(|e| matches!(e, Observation::Frozen { .. })));
    }
}

fn exact_gate(error: &Diagnostic, origin: Span) {
    assert_eq!(
        (
            error.code,
            error.stage,
            error.message.as_str(),
            error.primary
        ),
        (
            "E0101",
            "resolve",
            "enum source syntax is unavailable",
            Some(origin)
        )
    );
    assert!(error.secondary.is_empty());
}
#[test]
fn enum_index_stored_origin_is_exact_at_facts_and_scalar_producer_gates() {
    for (text, fragment) in [
        ("enum E{V} fn main()->(){return;}", "enum E{V}"),
        ("fn main()->(){E::V;return;}", "E::V"),
        (
            "fn main()->(){match x{E::V=>{}} return;}",
            "match x{E::V=>{}}",
        ),
        (
            "fn helper()->(){return;} fn main()->(){crate::helper();return;}",
            "crate::helper()",
        ),
    ] {
        let fixture = Fixture::new(&[("main.ox", text)]);
        let project = fixture.load();
        let work = WorkMeter::default();
        work.enable_observation();
        let mut allocator = Allocator::default();
        let start = text.find(fragment).unwrap();
        let origin = Span {
            file: SourceFileId(0),
            start,
            end: start + fragment.len(),
        };
        let facts = collect(&project, &work, &mut allocator);
        exact_gate(
            &facts.require_current_source_pipeline().unwrap_err(),
            origin,
        );
        exact_gate(
            &hir::original_signatures(&facts, &work).unwrap_err()[0],
            origin,
        );
        let index = facts.finish(&work, &mut allocator).unwrap();
        exact_gate(
            &index.require_current_source_pipeline().unwrap_err(),
            origin,
        );
        exact_gate(&hir::resolve_project(&index, &work).unwrap_err()[0], origin);
        exact_gate(
            &hir::resolve_bodies(&index, &work, Vec::new()).unwrap_err()[0],
            origin,
        );
        assert!(!work.observations.borrow().iter().any(|e| matches!(
            e,
            Observation::SignatureStart { .. } | Observation::RecordStart { .. }
        )));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn enum_index_stored_origin_is_exact_for_imported_enum() {
    let fixture = Fixture::new(&[
        ("main.ox", "mod child; fn main()->(){return;}"),
        ("child.ox", "enum Unused{V}"),
    ]);
    let project = fixture.load();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    exact_gate(
        &collect(&project, &work, &mut allocator)
            .require_current_source_pipeline()
            .unwrap_err(),
        Span {
            file: SourceFileId(1),
            start: 0,
            end: 14,
        },
    );
}

#[cfg(target_os = "linux")]
#[test]
fn enum_index_mandatory_build_work_threshold_precedes_all_reservations() {
    let fixture = identity_fixture();
    let project = fixture.load();
    let work = WorkMeter::default();
    work.enable_observation();
    let mut allocator = Allocator::default();
    let facts = collect(&project, &work, &mut allocator);
    let plan = facts.plan();
    // Literal fixture counts: original-name bytes81, aliases17, two path weights21+23.
    assert_eq!(
        (
            plan.counts.original_bytes,
            plan.counts.alias_bytes,
            plan.counts.path_weight
        ),
        (81, 17, 44)
    );
    // 30 visits*16 +128 +14 duplicate bound +96*5 +19*2 +46*2 +30+324.
    assert_eq!(plan.build_work, 1586);
    let preflight: u64 = work
        .events
        .borrow()
        .iter()
        .filter(|event| event.operation == "preflight visit")
        .map(|event| event.units)
        .sum();
    let mandatory = preflight + 1586;
    facts.finish(&work, &mut allocator).unwrap();
    // This is a whole-pipeline single-meter control, separate from staged rollback tests.
    assert!(work.used() <= mandatory);
    for (limit, ok) in [
        (mandatory - 1, false),
        (mandatory, true),
        (mandatory + 1, true),
    ] {
        let work = WorkMeter::new(limit);
        let mut allocator = Allocator::default();
        let result = collect_enum_candidate(
            SourceOwner::project(&project),
            IndexLimits {
                work: limit,
                ..IndexLimits::default()
            },
            &work,
            &mut allocator,
        );
        assert_eq!(result.is_ok(), ok);
        if !ok {
            assert_eq!(result.unwrap_err().code, "E0400");
            assert_eq!(allocator.attempts, 0);
        } else {
            assert_eq!(allocator.attempts, 16);
        }
    }
}

#[test]
fn enum_index_changed_carrier_alignments_are_measured() {
    use std::mem::align_of;
    println!("enum-index-alignment Index={} Facts={} Tables={} EnumRow={} VariantRow={} ModuleRow={} EnumView={} VariantView={} EnumViewOption={} VariantViewOption={} Counts={} IndexPlan={}",align_of::<DeclarationIndex<'_>>(),align_of::<DeclarationFacts<'_>>(),align_of::<Tables<'_>>(),align_of::<EnumRow>(),align_of::<VariantRow>(),align_of::<ModuleRow>(),align_of::<EnumView<'_>>(),align_of::<VariantView<'_>>(),align_of::<Option<EnumView<'_>>>(),align_of::<Option<VariantView<'_>>>(),align_of::<Counts>(),align_of::<IndexPlan>());
}
