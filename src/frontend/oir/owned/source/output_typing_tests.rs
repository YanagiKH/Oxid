//! Fresh private source-output typing. These controls produce no runtime I/O
//! and never grant production admission to a private source marker.
use super::*;
use crate::frontend::project::{ProjectLimits, ProjectSources};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const INVENTORIES: [(&str, BuiltinSet, usize, usize); 9] = [
    ("", BuiltinSet::None, 0, 0),
    ("use std::io::ReadStatus;", BuiltinSet::ReadStatus, 1, 0),
    ("use std::io::read_stdin;", BuiltinSet::ReadStdin, 1, 1),
    ("use std::io::WriteStatus;", BuiltinSet::WriteStatus, 1, 0),
    ("use std::io::write_stdout;", BuiltinSet::WriteStdout, 1, 1),
    (
        "use std::io::WriteStatus; use std::io::ReadStatus;",
        BuiltinSet::ReadStatusWriteStatus,
        2,
        0,
    ),
    (
        "use std::io::write_stdout; use std::io::ReadStatus;",
        BuiltinSet::ReadStatusWriteStdout,
        2,
        1,
    ),
    (
        "use std::io::WriteStatus; use std::io::read_stdin;",
        BuiltinSet::ReadStdinWriteStatus,
        2,
        1,
    ),
    (
        "use std::io::write_stdout; use std::io::read_stdin;",
        BuiltinSet::ReadStdinWriteStdout,
        2,
        2,
    ),
];

struct Fixture(PathBuf);
impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "oxid-output-typing-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        for (name, text) in files {
            fs::write(path.join(name), text).unwrap();
        }
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn with_index(text: &str, action: impl FnOnce(&DeclarationIndex<'_>)) {
    with_files(&[("main.ox", text)], action)
}
fn with_files(files: &[(&str, &str)], action: impl FnOnce(&DeclarationIndex<'_>)) {
    with_files_kind(files, false, action)
}
fn with_current_index(text: &str, action: impl FnOnce(&DeclarationIndex<'_>)) {
    with_files_kind(&[("main.ox", text)], true, action)
}
fn with_both_indices(text: &str, mut action: impl FnMut(&DeclarationIndex<'_>)) {
    with_index(text, &mut action);
    with_current_index(text, &mut action);
}
fn with_both_files(files: &[(&str, &str)], mut action: impl FnMut(&DeclarationIndex<'_>)) {
    with_files_kind(files, false, &mut action);
    with_files_kind(files, true, &mut action);
}
fn with_files_kind(
    files: &[(&str, &str)],
    current: bool,
    action: impl FnOnce(&DeclarationIndex<'_>),
) {
    let fixture = Fixture::new(files);
    let path = fixture.0.join("main.ox");
    let project = if current {
        ProjectSources::load_typed(path.to_str().unwrap(), ProjectLimits::default())
    } else {
        ProjectSources::load_output_candidate(
            path.to_str().unwrap(),
            ProjectLimits::default(),
            &mut Allocator::default(),
        )
    }
    .unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let collect = if current {
        index::collect_originals
    } else {
        index::collect_output_candidate
    };
    let index = collect(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap();
    action(&index);
}

fn expected_admission(index: &DeclarationIndex<'_>) -> SourceAdmission {
    if index.is_current_source_pipeline() {
        SourceAdmission::Executable
    } else {
        SourceAdmission::BuiltinPipeline
    }
}
fn type_source<'s>(
    index: &'s DeclarationIndex<'s>,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<super::super::typeck::TypedOwnedProgram<'s>, Vec<Diagnostic>> {
    if index.is_current_source_pipeline() {
        type_enum_source(index, work, allocator)
    } else {
        type_builtin_source(index, work, allocator)
    }
}
fn preflight_source(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
) -> Result<Option<super::super::hir_budget::HirPlan>, Box<Diagnostic>> {
    if index.is_current_source_pipeline() {
        super::super::hir_budget::preflight_current_hir(index, work)
    } else {
        super::super::hir_budget::preflight_builtin_hir(index, work)
    }
}
fn finish_source<'s>(
    program: ResolvedOwnedProgram<'s>,
    plan: &super::super::hir_budget::HirPlan,
    allocator: &mut Allocator,
    checkpoint: usize,
) -> Result<super::super::typeck::TypedOwnedProgram<'s>, Vec<Diagnostic>> {
    if program.index().is_current_source_pipeline() {
        super::super::typeck::finish_enum_source(program, plan, allocator, checkpoint)
    } else {
        super::super::typeck::finish_builtin_source(program, plan, allocator, checkpoint)
    }
}

#[test]
fn bounded_stdout_typing_denies_public_and_observer_admissions_before_reserve() {
    for (imports, _, _, _) in INVENTORIES {
        with_index(&format!("{imports} fn main()->i32{{return 0;}}"), |index| {
            let work = WorkMeter::default();
            let mut allocator = Allocator {
                fail_at: Some(1),
                ..Default::default()
            };
            for admission in [
                SourceAdmission::Executable,
                SourceAdmission::ObserveArrayTypes,
                SourceAdmission::ObserveEnumTypes,
                SourceAdmission::EnumPipeline,
                SourceAdmission::ObserveArrayPipeline,
                SourceAdmission::ArrayConsumer,
            ] {
                assert!(type_paid_source(index, &work, &mut allocator, admission).is_err());
            }
            assert!(type_enum_source(index, &work, &mut allocator).is_err());
            assert!(resolve_index(index, &work, &mut allocator).is_err());
            assert!(probe_enum_resolver_storage(index, &work, &mut allocator).is_err());
            assert!(probe_enum_type_storage(index, &work, &mut allocator).is_err());
            assert!(probe_enum_pipeline(
                index,
                &work,
                &mut allocator,
                EnumPipelineRequest::REFERENCE
            )
            .is_err());
            assert!(super::super::hir_budget::preflight_current_hir(index, &work).is_err());
            assert!(super::super::hir_budget::preflight_enum_hir(index, &work).is_err());
            assert_eq!(allocator.attempts, 0);
            assert_eq!(work.used(), 0);
            let plan = super::super::hir_budget::preflight_builtin_hir(index, &work)
                .unwrap()
                .unwrap();
            let before = work.used();
            for admission in [
                SourceAdmission::Executable,
                SourceAdmission::ObserveArrayTypes,
                SourceAdmission::ObserveEnumTypes,
                SourceAdmission::EnumPipeline,
                SourceAdmission::ObserveArrayPipeline,
                SourceAdmission::ArrayConsumer,
            ] {
                for route in 0..3 {
                    // Empty rows make an accidental advance past provenance
                    // observable without granting any paid construction.
                    let program = ResolvedOwnedProgram {
                        projection_bytes: std::cell::Cell::new(plan.total),
                        admission,
                        index: IndexOwner::Borrowed(index),
                        work: MeterOwner::Borrowed(&work),
                        sources: index.sources().view(),
                        records: Vec::new(),
                        signatures: Vec::new(),
                        functions: Vec::new(),
                        entry: index.root_original_main(),
                    };
                    let result = match route {
                        0 => super::super::typeck::finish_builtin_source(
                            program,
                            &plan,
                            &mut allocator,
                            0,
                        ),
                        1 => super::super::typeck::finish_enum_source(
                            program,
                            &plan,
                            &mut allocator,
                            0,
                        ),
                        2 => super::super::typeck::check(program),
                        _ => unreachable!(),
                    };
                    assert!(result.is_err());
                    assert_eq!(allocator.attempts, 0);
                    assert_eq!(work.used(), before);
                }
            }
        });
    }
}

#[test]
fn bounded_stdout_fresh_paid_typing_reconciles_all_nine_inventories() {
    use super::super::{hir_budget, reviewer_source};
    for (imports, expected, enums, builtins) in INVENTORIES {
        with_index(&format!("{imports} fn main()->i32{{return 0;}}"), |index| {
            assert_eq!(index.builtin_set(), expected);
            let work = WorkMeter::default();
            let (plan, heap) = reviewer_source::integration_measured(|| {
                hir_budget::preflight_builtin_hir(index, &work)
            });
            let plan = plan.unwrap().unwrap();
            assert_eq!(heap, (0, 0, 0));
            assert_eq!(
                (
                    plan.counts.functions,
                    plan.counts.signatures,
                    plan.counts.parameters
                ),
                (1, 1 + builtins, builtins)
            );
            assert_eq!(index.enum_count(), enums);
            assert_eq!(index.enum_variant_counts().sum::<usize>(), 3 * enums);
            for (family, payload_member) in
                [(BuiltinEnum::ReadStatus, 0), (BuiltinEnum::WriteStatus, 2)]
            {
                if !expected.contains_enum(family) {
                    continue;
                }
                let enumeration = index.builtin_enum_id(family).unwrap();
                let view = index.enum_view(enumeration).unwrap();
                for member in 0..3 {
                    let variant = view
                        .variant(VariantId {
                            enumeration,
                            index: member,
                        })
                        .unwrap();
                    assert_eq!(
                        variant.payload(),
                        (member == payload_member).then_some(Ty::I32)
                    );
                }
            }
            let remaining = hir_budget::MAX_HIR_BYTES - plan.total;
            assert_eq!(
                plan.with_dynamic(remaining, index.sources().eof()).unwrap(),
                hir_budget::MAX_HIR_BYTES
            );
            assert_eq!(
                plan.with_dynamic(remaining + 1, index.sources().eof())
                    .unwrap_err()
                    .code,
                "E0400"
            );
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(128).unwrap();
            let (observed, (_, live, peak)) = reviewer_source::integration_measured(|| {
                let typed = type_builtin_source(index, &work, &mut allocator).unwrap();
                assert_eq!(typed.admission(), SourceAdmission::BuiltinPipeline);
                assert!(!typed.admission().executable());
                assert_eq!(typed.functions().len(), 1);
                assert_eq!(typed.signatures().len(), 1 + builtins);
                assert_eq!(typed.source_storage_bytes(), Some(plan.total));
                typed.validate_function_signatures().unwrap();
                for builtin in BuiltinFunction::ALL {
                    if !expected.contains_function(builtin) {
                        continue;
                    }
                    let ordinal = index.builtin_function_id(builtin).unwrap().0;
                    let enum_id = index
                        .builtin_enum_id(match builtin {
                            BuiltinFunction::ReadStdin => BuiltinEnum::ReadStatus,
                            BuiltinFunction::WriteStdout => BuiltinEnum::WriteStatus,
                        })
                        .unwrap();
                    let signature = &typed.signatures()[ordinal];
                    let kind = if builtin == BuiltinFunction::ReadStdin {
                        BorrowKind::Exclusive
                    } else {
                        BorrowKind::Shared
                    };
                    assert_eq!(
                        signature.params.as_slice(),
                        [ParameterTy::Reference {
                            referent: BorrowedTy::ScalarSlice(Ty::I32),
                            kind,
                        }]
                    );
                    assert_eq!(signature.result, ValueTy::Owned(AggregateTy::Enum(enum_id)));
                }
                drop(typed);
                allocator.attempts
            });
            assert_eq!(live, 0);
            assert!(peak > 0);
            for (kind, length) in [
                ("paid HIR signatures", 1 + builtins),
                ("paid HIR parameters", builtins),
                ("paid HIR functions", 1),
                ("paid typed bodies", 1),
            ] {
                assert_eq!(
                    allocator
                        .trace
                        .iter()
                        .filter(|row| row.kind == kind)
                        .map(|row| row.length)
                        .sum::<usize>(),
                    length
                );
            }
            assert!(allocator.trace.iter().all(|row| row.success));
            assert!(!allocator.observer_trace_overflow);
            println!("OUTPUT_FRESH_PAID_TYPING inventory={expected:?} fixed={} total={} reserves={observed} live={live} peak={peak}", plan.fixed, plan.total);
        });
    }
}

#[test]
fn bounded_stdout_typing_carriers_have_actual_layout_receipts() {
    use std::mem::{align_of, size_of};
    assert_eq!(size_of::<SourceAdmission>(), 1);
    assert!(!SourceAdmission::BuiltinPipeline.executable());
    println!("OUTPUT_TYPING_LAYOUT resolved={}/{} typed={}/{} signature={}/{} identity={}/{} source_dispatch={} type_dispatch={} preflight={:?}",
        size_of::<ResolvedOwnedProgram<'static>>(), align_of::<ResolvedOwnedProgram<'static>>(),
        size_of::<super::super::typeck::TypedOwnedProgram<'static>>(), align_of::<super::super::typeck::TypedOwnedProgram<'static>>(),
        size_of::<BuiltinSignatureCarriers>(), align_of::<BuiltinSignatureCarriers>(),
        size_of::<SignatureIdentityCarriers>(), align_of::<SignatureIdentityCarriers>(),
        production_source_carrier_bytes(), super::super::typeck::production_type_carrier_bytes(),
        super::super::hir_budget::output_preflight_carrier_layouts());
    println!("OUTPUT_TYPING_STORAGE_RECEIPT resolver_paid={}/{} resolver_fixed={} type_plan={}/{} type_fixed={} preparation={} reconciliation={}",
        size_of::<PaidStorage>(), align_of::<PaidStorage>(), storage::fixed_carrier_bytes(),
        size_of::<super::super::type_storage::TypePlan<'static>>(), align_of::<super::super::type_storage::TypePlan<'static>>(),
        super::super::type_storage::fixed_control_carrier_bytes(), super::super::type_storage::preparation_carrier_bytes(),
        super::super::type_storage::typed_reconciliation_carrier_bytes());
}

#[test]
fn bounded_stdout_signature_identity_rejects_family_and_suffix_corruption() {
    const BOTH: &str = "use std::io::write_stdout as output; use std::io::read_stdin as input; enum Local{One} fn main()->i32{return 0;}";
    with_both_indices(BOTH, |index| {
        for mutation in 0..12 {
            let work = WorkMeter::default();
            let plan = preflight_source(index, &work).unwrap().unwrap();
            let mut allocator = Allocator::default();
            let mut paid = PaidStorage::new(plan.counts);
            let (records, signatures, functions) =
                resolve_index_impl(index, &work, &mut allocator, Some(&mut paid)).unwrap();
            let mut program = ResolvedOwnedProgram {
                projection_bytes: std::cell::Cell::new(plan.total),
                admission: expected_admission(index),
                index: IndexOwner::Borrowed(index),
                work: MeterOwner::Borrowed(&work),
                sources: index.sources().view(),
                records,
                signatures,
                functions,
                entry: index.root_original_main(),
            };
            program.validate_function_signatures().unwrap();
            let input = index
                .builtin_function_id(BuiltinFunction::ReadStdin)
                .unwrap()
                .0;
            let output = index
                .builtin_function_id(BuiltinFunction::WriteStdout)
                .unwrap()
                .0;
            assert_eq!((input, output), (1, 2));
            match mutation {
                0 => {
                    program.signatures.pop();
                }
                1 => program.signatures.push(Signature {
                    params: Vec::new(),
                    result: ValueTy::Scalar(Ty::Unit),
                    span: index.sources().eof(),
                }),
                2 => program.signatures[output].params.clear(),
                3 => {
                    program.signatures[output].params[0] = ParameterTy::Reference {
                        referent: BorrowedTy::ScalarSlice(Ty::I32),
                        kind: BorrowKind::Exclusive,
                    }
                }
                4 => {
                    program.signatures[input].params[0] = ParameterTy::Reference {
                        referent: BorrowedTy::ScalarSlice(Ty::I32),
                        kind: BorrowKind::Shared,
                    }
                }
                5 => program.signatures[output].result = program.signatures[input].result,
                6 => program.signatures[input].result = program.signatures[output].result,
                7 => program.signatures[output].span = program.signatures[input].span,
                8 => program.signatures.swap(input, output),
                9 => program.signatures.swap(0, output),
                10 => program.functions[0].id = DefId(output),
                11 => {
                    program.signatures[output].params[0] = ParameterTy::Reference {
                        referent: BorrowedTy::ScalarSlice(Ty::Bool),
                        kind: BorrowKind::Shared,
                    }
                }
                _ => unreachable!(),
            }
            let before = allocator.attempts;
            assert_eq!(
                program.validate_function_signatures().unwrap_err().code,
                "E0500",
                "mutation {mutation}"
            );
            assert_eq!(
                finish_source(program, &plan, &mut allocator, before).unwrap_err()[0].code,
                "E0500"
            );
            assert_eq!(allocator.attempts, before);
        }
    });
}

#[test]
fn bounded_stdout_fresh_typing_reserve_failures_drop_prior_family_storage() {
    for imports in [
        "use std::io::write_stdout;",
        "use std::io::write_stdout; use std::io::read_stdin;",
    ] {
        with_both_indices(&format!("{imports} fn main()->i32{{return 0;}}"), |index| {
            let work = WorkMeter::default();
            let mut baseline = Allocator::default();
            drop(type_source(index, &work, &mut baseline).unwrap());
            assert!(baseline.attempts > 4);
            for attempt in 1..=baseline.attempts {
                let work = WorkMeter::default();
                let mut allocator = Allocator {
                    fail_at: Some(attempt),
                    ..Default::default()
                };
                allocator.observer_trace_bound(128).unwrap();
                let (resource_failure, (_, live, _)) =
                    super::super::reviewer_source::integration_measured(|| {
                        match type_source(index, &work, &mut allocator) {
                            Ok(typed) => {
                                drop(typed);
                                false
                            }
                            Err(errors) => errors.iter().any(|error| error.code == "E0400"),
                        }
                    });
                assert!(resource_failure, "{imports}: reserve {attempt}");
                assert_eq!(live, 0, "{imports}: reserve {attempt}");
                assert!(allocator.attempts >= attempt);
                assert!(!allocator.trace[attempt - 1].success);
                assert!(allocator.trace[..attempt - 1].iter().all(|row| row.success));
                assert!(!allocator.observer_trace_overflow);
            }
            println!(
                "OUTPUT_FRESH_TYPING_FAILURES inventory={:?} attempts={}",
                index.builtin_set(),
                baseline.attempts
            );
        });
    }
}

#[test]
fn bounded_stdout_second_parameter_real_null_cleans_both_suffixes() {
    use crate::frontend::{
        oir::owned::reviewer_origins as raw, project::budget::real_null_observer as null,
    };
    use std::{alloc::Layout, mem::size_of};
    // Source-derived request order: records(0), signatures(3), main params(0),
    // input params(1), output params(1). Select the actual output allocation.
    const TEXT: &str =
        "use std::io::write_stdout; use std::io::read_stdin; fn main()->i32{return 0;}";
    with_both_indices(TEXT, |index| {
        let work = WorkMeter::default();
        let expected_anchor = index
            .builtin_function_anchor(BuiltinFunction::WriteStdout)
            .unwrap();
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(8).unwrap();
        let trace_capacity = allocator.trace.capacity();
        let target = null::Target {
            attempt: 5,
            kind: "paid HIR parameters",
            slots: 1,
            element_bytes: size_of::<ParameterTy>(),
            layout: Layout::array::<ParameterTy>(1).unwrap(),
        };
        let action = |allocator: &mut Allocator| {
            raw::integration_counted(|| {
                super::super::reviewer_source::integration_measured(|| {
                    match type_source(index, &work, allocator) {
                        Ok(typed) => {
                            drop(typed);
                            (false, 0, None, None, None, false)
                        }
                        Err(errors) => (
                            true,
                            errors.len(),
                            errors.first().map(|e| e.code),
                            errors.first().map(|e| e.stage),
                            errors.first().and_then(|e| e.primary),
                            errors.iter().all(|e| {
                                e.message == "affected HIR allocation failed"
                                    && e.secondary.is_empty()
                                    && e.notes.is_empty()
                            }),
                        ),
                    }
                })
            })
        };
        let control_bytes = null::selection_carriers_bytes(&action);
        let (((facts, stats), raw_calls), report) =
            null::with_selected(&mut allocator, target, action).unwrap();
        assert_eq!(
            facts,
            (
                true,
                1,
                Some("E0400"),
                Some("resolve"),
                Some(expected_anchor),
                true
            )
        );
        assert_eq!(report.target, target);
        assert!(report.selected && report.matched && report.fired);
        assert_eq!(report.rejection, None);
        assert_eq!(
            report.actual,
            Some(null::GlobalEvent {
                operation: null::Operation::Alloc,
                layout: target.layout,
                new_size: None
            })
        );
        assert_eq!(raw_calls, stats.0 + 1);
        assert_eq!(stats.1, 0);
        assert!(stats.0 > 0 && stats.2 > 0);
        assert_eq!(allocator.fail_at, None);
        assert_eq!((allocator.attempts, allocator.trace.len()), (5, 5));
        assert_eq!(allocator.trace.capacity(), trace_capacity);
        for (row, expected) in allocator.trace.iter().zip([
            ("paid HIR records", 0, size_of::<Record>(), true),
            ("paid HIR signatures", 3, size_of::<Signature>(), true),
            ("paid HIR parameters", 0, size_of::<ParameterTy>(), true),
            ("paid HIR parameters", 1, size_of::<ParameterTy>(), true),
            ("paid HIR parameters", 1, size_of::<ParameterTy>(), false),
        ]) {
            assert_eq!(
                (row.kind, row.length, row.element_bytes, row.success),
                expected
            );
        }
        assert!(!allocator.observer_trace_overflow);
        assert!(
            !super::super::reviewer_source::integration_enabled() && !raw::integration_enabled()
        );
        println!("OUTPUT_SECOND_PARAMETER_REAL_NULL slots=1 width={} controls={control_bytes} live={} peak={}", target.element_bytes, stats.1, stats.2);
    });
}

const WRITE_CALL: &str = "use std::io::write_stdout as output; use std::io::WriteStatus as W; fn relay(xs:&[i32])->W{return output(&*xs);} fn main()->i32{let bytes=[0,255];let status=relay(&bytes);match status{W::Complete=>{return 0;},W::InvalidInput=>{return 1;},W::IoError(progress)=>{return progress;},}}";
const BOTH_CALLS: &str = "use std::io::write_stdout as output; use std::io::WriteStatus as W; use std::io::read_stdin as input; use std::io::ReadStatus as R; fn count(status:R)->i32{match status{R::Eof(n)=>{return n;},R::Full=>{return 2;},R::IoError=>{return 0;},}} fn main()->i32{let mut bytes=[0,0];let read=input(&mut bytes);let n=count(read);let written=output(&bytes);match written{W::Complete=>{return n;},W::InvalidInput=>{return 1;},W::IoError(progress)=>{return progress;},}}";

#[test]
fn bounded_stdout_fresh_typing_accepts_calls_forwarded_views_and_status_payloads() {
    for text in [
        WRITE_CALL,
        BOTH_CALLS,
        "use std::io::WriteStatus as W; fn consume(status:W)->i32{match status{W::Complete=>{return 0;},W::InvalidInput=>{return 1;},W::IoError(progress)=>{return progress;},}} fn main()->i32{let complete=W::Complete;let invalid=W::InvalidInput;let error=W::IoError(7);return consume(complete)+consume(invalid)+consume(error);}",
        "use std::io::write_stdout; use std::io::WriteStatus as W; struct Packet{bytes:[i32;2]} fn main()->i32{let packet=Packet{bytes:[0,255]};let status=write_stdout(&packet.bytes);match status{W::Complete=>{return 0;},W::InvalidInput=>{return 1;},W::IoError(n)=>{return n;},}}",
    ] {
        with_both_indices(text, |index| {
            let work = WorkMeter::default();
            let plan = preflight_source(index, &work).unwrap().unwrap();
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(512).unwrap();
            let (charged, (_, live, peak)) = super::super::reviewer_source::integration_measured(|| {
                let typed = type_source(index, &work, &mut allocator).unwrap();
                assert_eq!(typed.admission(), expected_admission(index));
                typed.validate_function_signatures().unwrap();
                let charged = typed.source_storage_bytes().unwrap();
                drop(typed);
                charged
            });
            assert!(charged >= plan.total);
            assert_eq!(live, 0);
            assert!(peak > 0);
            assert!(allocator.trace.iter().all(|row| row.success));
            assert!(!allocator.observer_trace_overflow);
            assert_eq!(allocator.trace.iter().filter(|row| row.kind == "paid typed bodies").map(|row| row.length).sum::<usize>(), index.source_function_count());
            println!("OUTPUT_TYPED_CALLS inventory={:?} total={} charged={charged} work={} reserves={} live={live} peak={peak}", index.builtin_set(), plan.total, work.used(), allocator.attempts);
        });
    }
}

#[test]
fn bounded_stdout_fresh_typing_rejects_wrong_borrows_nominals_and_payloads() {
    for text in [
        "use std::io::write_stdout; fn main()->i32{let mut a=[1];let status=write_stdout(&mut a);return 0;}",
        "use std::io::write_stdout; fn main()->i32{let a=[true];let status=write_stdout(&a);return 0;}",
        "use std::io::write_stdout; fn main()->i32{let a=[1];let status=write_stdout(a);return 0;}",
        "use std::io::write_stdout; enum Same{Complete,InvalidInput,IoError(i32)} fn main()->i32{let a=[1];let status:Same=write_stdout(&a);return 0;}",
        "use std::io::write_stdout; use std::io::ReadStatus; fn main()->i32{let a=[1];let status:ReadStatus=write_stdout(&a);return 0;}",
        "use std::io::WriteStatus as W; fn main()->i32{let status=W::Complete(7);return 0;}",
        "use std::io::WriteStatus as W; fn main()->i32{let status=W::IoError;return 0;}",
        "use std::io::WriteStatus as W; fn main()->i32{let status=W::IoError(true);return 0;}",
        "use std::io::WriteStatus as W; fn main()->i32{let status=W::Complete;match status{W::Complete=>{return 0;},W::InvalidInput=>{return 1;},}}",
    ] {
        with_both_indices(text, |index| {
            let work = WorkMeter::default();
            let mut allocator = Allocator::default();
            // The qualification trace outlives heap observation; reserve its
            // separate backing first so only compiler-owned cleanup is sampled.
            allocator.observer_trace_bound(512).unwrap();
            let trace_capacity = allocator.trace.capacity();
            let (codes, (_, live, _)) = super::super::reviewer_source::integration_measured(|| {
                match type_source(index, &work, &mut allocator) {
                    Ok(typed) => { drop(typed); (false, false) }
                    Err(errors) => (true, errors.iter().all(|error| error.code == "E0300")),
                }
            });
            assert_eq!(codes, (true, true), "{text}");
            assert_eq!(live, 0, "{text}");
            assert_eq!(allocator.trace.capacity(), trace_capacity);
            assert!(!allocator.observer_trace_overflow);
        });
    }
}

#[test]
fn bounded_stdout_fresh_typing_work_endpoint_is_exact() {
    with_both_indices(BOTH_CALLS, |index| {
        let baseline = WorkMeter::default();
        drop(type_source(index, &baseline, &mut Allocator::default()).unwrap());
        let required = baseline.used();
        assert!(required > 0);
        for limit in [required - 1, required] {
            let work = WorkMeter::new(limit);
            let mut allocator = Allocator::default();
            // The qualification trace outlives heap observation; reserve its
            // separate backing first so only compiler-owned cleanup is sampled.
            allocator.observer_trace_bound(512).unwrap();
            let trace_capacity = allocator.trace.capacity();
            let ((succeeded, exhausted), (_, live, _)) =
                super::super::reviewer_source::integration_measured(|| {
                    match type_source(index, &work, &mut allocator) {
                        Ok(typed) => {
                            drop(typed);
                            (true, false)
                        }
                        Err(errors) => (false, errors.iter().any(|error| error.code == "E0400")),
                    }
                });
            assert_eq!(succeeded, limit == required);
            assert_eq!(exhausted, limit < required);
            assert_eq!(live, 0);
            assert_eq!(allocator.trace.capacity(), trace_capacity);
            assert!(!allocator.observer_trace_overflow);
        }
        println!("OUTPUT_FRESH_TYPING_WORK exact={required}");
    });
}

#[test]
fn bounded_stdout_fresh_typing_preserves_child_aliases_and_first_anchor() {
    // RFC0015 module calls use crate-qualified paths; bare E::member selects
    // enum qualification and does not express a relative module call.
    let files = [
        ("main.ox", "use std::io::write_stdout as first; pub mod output; fn main()->i32{let bytes=[65];return crate::output::send(&bytes);}"),
        ("output.ox", "use std::io::write_stdout as later; use std::io::WriteStatus as W; pub fn send(xs:&[i32])->i32{let status=later(&*xs);match status{W::Complete=>{return 0;},W::InvalidInput=>{return 1;},W::IoError(n)=>{return n;},}}"),
    ];
    #[cfg(not(target_os = "linux"))]
    {
        let fixture = Fixture::new(&files);
        let failure = ProjectSources::load_output_candidate(
            fixture.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
            &mut Allocator::default(),
        )
        .unwrap_err();
        assert_eq!(failure.diagnostics.len(), 1);
        let error = &failure.diagnostics[0];
        assert_eq!(
            (error.code, error.stage, error.message.as_str()),
            (
                "E0005",
                "source",
                "module source policy is not qualified on this host"
            ),
        );
    }
    #[cfg(target_os = "linux")]
    with_both_files(&files, |index| {
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let typed = type_source(index, &work, &mut allocator).unwrap();
        typed.validate_function_signatures().unwrap();
        let output = index
            .builtin_function_id(BuiltinFunction::WriteStdout)
            .unwrap();
        let anchor = index
            .builtin_function_anchor(BuiltinFunction::WriteStdout)
            .unwrap();
        assert_eq!(typed.signatures()[output.0].span, anchor);
        assert_eq!((anchor.start, anchor.end), (13, 25));
        // Root preorder wins even though only the child's alias is called.
        assert_eq!(anchor.file, index.sources().eof().file);
        assert_eq!(typed.functions().len(), 2);
        assert_eq!(typed.signatures().len(), 3);
    });
}

#[test]
fn bounded_stdout_completion_rejects_seed_checkpoint_and_plan_mismatch_before_typed_reserve() {
    with_both_indices(BOTH_CALLS, |index| {
        for mutation in 0..3 {
            let work = WorkMeter::default();
            let mut plan = preflight_source(index, &work).unwrap().unwrap();
            let mut allocator = Allocator::default();
            // The qualification trace outlives heap observation; reserve its
            // separate backing first so only compiler-owned cleanup is sampled.
            allocator.observer_trace_bound(512).unwrap();
            let trace_capacity = allocator.trace.capacity();
            let (code, (_, live, _)) = super::super::reviewer_source::integration_measured(|| {
                let mut paid = PaidStorage::new(plan.counts);
                let parts =
                    resolve_index_impl(index, &work, &mut allocator, Some(&mut paid)).unwrap();
                let inventory =
                    storage::inventory_parts(&parts, &work, index.sources().eof()).unwrap();
                paid.reconcile(
                    &plan,
                    inventory,
                    allocator.attempts,
                    &work,
                    index.sources().eof(),
                )
                .unwrap();
                let (records, signatures, functions) = parts;
                let program = ResolvedOwnedProgram {
                    projection_bytes: std::cell::Cell::new(plan.total),
                    admission: expected_admission(index),
                    index: IndexOwner::Borrowed(index),
                    work: MeterOwner::Borrowed(&work),
                    sources: index.sources().view(),
                    records,
                    signatures,
                    functions,
                    entry: index.root_original_main(),
                };
                let before = allocator.attempts;
                let mut checkpoint = before;
                match mutation {
                    0 => program.type_storage_cell().set(plan.total + 1),
                    1 => checkpoint -= 1,
                    2 => plan.counts.signatures -= 1,
                    _ => unreachable!(),
                }
                let errors = finish_source(program, &plan, &mut allocator, checkpoint).unwrap_err();
                assert_eq!(allocator.attempts, before);
                errors[0].code
            });
            assert_eq!(code, "E0500", "mutation {mutation}");
            assert_eq!(live, 0, "mutation {mutation}");
            assert_eq!(allocator.trace.capacity(), trace_capacity);
            assert!(!allocator.observer_trace_overflow);
        }
    });
}
