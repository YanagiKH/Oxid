//! Closed source-output precursor: passive plans and local paid resolver parts
//! are observable; no positive typed owner is admitted by these controls.
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
    fn new(text: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "oxid-output-typing-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("main.ox"), text).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn with_index(text: &str, action: impl FnOnce(&DeclarationIndex<'_>)) {
    let fixture = Fixture::new(text);
    let project = ProjectSources::load_output_candidate(
        fixture.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default(),
        &mut Allocator::default(),
    )
    .unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = index::collect_output_candidate(
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

#[test]
fn bounded_stdout_closed_typing_denies_every_output_marker_before_reserve() {
    for (imports, _, _, _) in INVENTORIES {
        with_index(&format!("{imports} fn main()->i32{{return 0;}}"), |index| {
            let work = WorkMeter::default();
            let mut allocator = Allocator {
                fail_at: Some(1),
                ..Default::default()
            };
            assert!(type_builtin_source(index, &work, &mut allocator).is_err());
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
        });
    }
}

#[test]
fn bounded_stdout_preflight_and_paid_resolver_reconcile_all_nine_inventories() {
    use super::super::{hir_budget, reviewer_source, typeck};
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
                    admission: SourceAdmission::BuiltinPipeline,
                    index: IndexOwner::Borrowed(index),
                    work: MeterOwner::Borrowed(&work),
                    sources: index.sources().view(),
                    records,
                    signatures,
                    functions,
                    entry: index.root_original_main(),
                };
                program.validate_function_signatures().unwrap();
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
                    let signature = &program.signatures[ordinal];
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
                let resolver_end = allocator.attempts;
                let error =
                    typeck::finish_builtin_source(program, &plan, &mut allocator, resolver_end)
                        .unwrap_err();
                assert_eq!(error[0].code, "E0500");
                assert_eq!(allocator.attempts, resolver_end);
                resolver_end
            });
            assert_eq!(live, 0);
            assert!(peak > 0);
            for (kind, length) in [
                ("paid HIR signatures", 1 + builtins),
                ("paid HIR parameters", builtins),
                ("paid HIR functions", 1),
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
            println!("OUTPUT_CLOSED_RESOLVER inventory={expected:?} fixed={} total={} reserves={observed} live={live} peak={peak}", plan.fixed, plan.total);
        });
    }
}

#[test]
fn bounded_stdout_closed_typing_carriers_have_actual_layout_receipts() {
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
    println!("OUTPUT_TYPING_UNCHANGED_STORAGE resolver_paid={}/{} resolver_fixed={} type_plan={}/{} type_fixed={} preparation={} reconciliation={}",
        size_of::<PaidStorage>(), align_of::<PaidStorage>(), storage::fixed_carrier_bytes(),
        size_of::<super::super::type_storage::TypePlan<'static>>(), align_of::<super::super::type_storage::TypePlan<'static>>(),
        super::super::type_storage::fixed_control_carrier_bytes(), super::super::type_storage::preparation_carrier_bytes(),
        super::super::type_storage::typed_reconciliation_carrier_bytes());
}

#[test]
fn bounded_stdout_signature_identity_rejects_family_and_suffix_corruption() {
    const BOTH: &str = "use std::io::write_stdout as output; use std::io::read_stdin as input; enum Local{One} fn main()->i32{return 0;}";
    with_index(BOTH, |index| {
        for mutation in 0..12 {
            let work = WorkMeter::default();
            let plan = super::super::hir_budget::preflight_builtin_hir(index, &work)
                .unwrap()
                .unwrap();
            let mut allocator = Allocator::default();
            let mut paid = PaidStorage::new(plan.counts);
            let (records, signatures, functions) =
                resolve_index_impl(index, &work, &mut allocator, Some(&mut paid)).unwrap();
            let mut program = ResolvedOwnedProgram {
                projection_bytes: std::cell::Cell::new(plan.total),
                admission: SourceAdmission::BuiltinPipeline,
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
                super::super::typeck::finish_builtin_source(program, &plan, &mut allocator, before)
                    .unwrap_err()[0]
                    .code,
                "E0500"
            );
            assert_eq!(allocator.attempts, before);
        }
    });
}

#[test]
fn bounded_stdout_resolver_reserve_failures_drop_prior_family_storage() {
    for imports in [
        "use std::io::write_stdout;",
        "use std::io::write_stdout; use std::io::read_stdin;",
    ] {
        with_index(&format!("{imports} fn main()->i32{{return 0;}}"), |index| {
            let work = WorkMeter::default();
            let plan = super::super::hir_budget::preflight_builtin_hir(index, &work)
                .unwrap()
                .unwrap();
            let mut baseline = Allocator::default();
            let mut paid = PaidStorage::new(plan.counts);
            drop(resolve_index_impl(index, &work, &mut baseline, Some(&mut paid)).unwrap());
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
                        let mut paid = PaidStorage::new(plan.counts);
                        match resolve_index_impl(index, &work, &mut allocator, Some(&mut paid)) {
                            Ok(parts) => {
                                drop(parts);
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
                "OUTPUT_RESOLVER_FAILURES inventory={:?} attempts={}",
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
    with_index(TEXT, |index| {
        let work = WorkMeter::default();
        let plan = super::super::hir_budget::preflight_builtin_hir(index, &work)
            .unwrap()
            .unwrap();
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
                    let mut paid = PaidStorage::new(plan.counts);
                    match resolve_index_impl(index, &work, allocator, Some(&mut paid)) {
                        Ok(parts) => {
                            drop(parts);
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
