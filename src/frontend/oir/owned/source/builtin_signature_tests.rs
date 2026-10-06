//! Fresh paid signature/type controls for separate current and candidate lanes.
use super::*;
use crate::frontend::project::{ProjectLimits, ProjectSources};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const BOTH: &str = "use std::io::read_stdin as input; use std::io::ReadStatus as Status; enum Local { Value } fn helper()->i32{return 7;} fn relay(xs:&mut [i32])->Status{return input(&mut *xs);} fn main()->i32{let mut bytes=[1,2];let status=relay(&mut bytes);match status{Status::Eof(n)=>{return n;},Status::Full=>{return helper();},Status::IoError=>{return 0;},}}";

struct Fixture(PathBuf);
impl Fixture {
    fn new(text: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "oxid-builtin-signatures-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("main.ox"), text).unwrap();
        Self(directory)
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

fn with_index(text: &str, action: impl FnOnce(&DeclarationIndex<'_>)) {
    with_index_kind(text, false, action)
}

fn with_current_index(text: &str, action: impl FnOnce(&DeclarationIndex<'_>)) {
    with_index_kind(text, true, action)
}

fn with_index_kind(text: &str, current: bool, action: impl FnOnce(&DeclarationIndex<'_>)) {
    let fixture = Fixture::new(text);
    let project = if current {
        ProjectSources::load_typed(
            fixture.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
        )
        .unwrap()
    } else {
        fixture.load()
    };
    let owner = SourceOwner::project(&project);
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let facts = if current {
        index::collect_originals(owner, IndexLimits::default(), &work, &mut allocator)
    } else {
        index::collect_builtin_candidate(owner, IndexLimits::default(), &work, &mut allocator)
    }
    .unwrap();
    let index = facts.finish(&work, &mut allocator).unwrap();
    action(&index);
}

#[test]
fn bounded_stdin_current_paid_signatures_reconcile_source_bodies_and_import_suffix() {
    for (text, bodies, signatures, enumerations) in [
        (
            "use std::io::ReadStatus as S; fn main()->i32{return 0;}",
            1,
            1,
            1,
        ),
        (
            "use std::io::read_stdin as input; fn main()->i32{return 0;}",
            1,
            2,
            1,
        ),
        (BOTH, 3, 4, 2),
    ] {
        with_current_index(text, |index| {
            let work = WorkMeter::default();
            let (plan, heap) = super::super::reviewer_source::integration_measured(|| {
                super::super::hir_budget::preflight_current_hir(index, &work)
            });
            let plan = plan.unwrap().unwrap();
            assert_eq!(heap, (0, 0, 0));
            assert_eq!(
                (plan.counts.functions, plan.counts.signatures),
                (bodies, signatures)
            );
            let remaining = super::super::hir_budget::MAX_HIR_BYTES - plan.total;
            assert_eq!(
                plan.with_dynamic(remaining, index.sources().eof()).unwrap(),
                super::super::hir_budget::MAX_HIR_BYTES
            );
            assert_eq!(
                plan.with_dynamic(remaining + 1, index.sources().eof())
                    .unwrap_err()
                    .code,
                "E0400"
            );
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(512).unwrap();
            let (observed, (_, live, peak)) =
                super::super::reviewer_source::integration_measured(|| {
                    let typed = type_enum_source(index, &work, &mut allocator).unwrap();
                    assert_eq!(typed.admission(), SourceAdmission::Executable);
                    typed.validate_function_signatures().unwrap();
                    let observed = (
                        typed.functions().len(),
                        typed.signatures().len(),
                        typed.index().enum_count(),
                        typed.source_storage_bytes().unwrap(),
                    );
                    drop(typed);
                    observed
                });
            assert_eq!(
                (observed.0, observed.1, observed.2),
                (bodies, signatures, enumerations)
            );
            assert!(observed.3 >= plan.total);
            assert_eq!(live, 0);
            assert!(peak > 0);
            for (kind, expected) in [
                ("paid HIR functions", bodies),
                ("paid HIR signatures", signatures),
                ("paid typed bodies", bodies),
            ] {
                assert_eq!(
                    allocator
                        .trace
                        .iter()
                        .filter(|event| event.kind == kind)
                        .map(|event| event.length)
                        .sum::<usize>(),
                    expected,
                    "{kind}"
                );
            }
            assert_eq!(
                allocator
                    .trace
                    .iter()
                    .filter(|event| event.kind == "paid HIR parameters")
                    .count(),
                signatures
            );
            assert!(allocator.trace.iter().all(|event| event.success));
            assert!(!allocator.observer_trace_overflow);
            println!(
                "BUILTIN_CURRENT_PAID bodies={bodies} signatures={signatures} enums={enumerations} fixed={} total={} observed={} reserves={} live={live} peak={peak}",
                plan.fixed, plan.total, observed.3, allocator.attempts
            );
        });
    }
}

#[test]
fn bounded_stdin_current_preflight_keeps_ordinary_enum_work_and_empty_selection() {
    for text in [
        "fn main()->i32{return 0;}",
        "enum E{V} fn main()->i32{return 0;}",
    ] {
        with_current_index(text, |index| {
            let current_work = WorkMeter::default();
            let old_work = WorkMeter::default();
            let current =
                super::super::hir_budget::preflight_current_hir(index, &current_work).unwrap();
            let old = super::super::hir_budget::preflight_enum_hir(index, &old_work).unwrap();
            assert_eq!(current, old);
            assert_eq!(current_work.used(), old_work.used());
            if index.enum_count() == 0 {
                assert_eq!(current, None);
                assert_eq!(current_work.used(), 0);
            }
        });
    }
}

#[test]
fn bounded_stdin_current_imports_reject_private_and_legacy_consumers_before_reservation() {
    for text in ["use std::io::ReadStatus; fn main()->i32{return 0;}", BOTH] {
        with_current_index(text, |index| {
            let work = WorkMeter::default();
            let mut allocator = Allocator {
                fail_at: Some(1),
                ..Default::default()
            };
            assert!(type_builtin_source(index, &work, &mut allocator).is_err());
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
            assert!(super::super::hir_budget::preflight_enum_hir(index, &work).is_err());
            assert!(super::super::hir_budget::preflight_builtin_hir(index, &work).is_err());
            assert_eq!(allocator.attempts, 0);
            assert_eq!(work.used(), 0);
        });
    }
}

#[test]
fn bounded_stdin_current_owners_keep_private_checker_completions_closed() {
    with_current_index(BOTH, |index| {
        let work = WorkMeter::default();
        let plan = super::super::hir_budget::preflight_current_hir(index, &work)
            .unwrap()
            .unwrap();
        let empty = |admission| ResolvedOwnedProgram {
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
        let mut allocator = Allocator {
            fail_at: Some(1),
            ..Default::default()
        };
        let before = work.used();
        assert_eq!(
            super::super::typeck::check(empty(SourceAdmission::Executable)).unwrap_err()[0].code,
            "E0101"
        );
        assert_eq!(
            super::super::typeck::observe_enum_type_storage(
                &empty(SourceAdmission::ObserveEnumTypes),
                &plan,
                &mut allocator
            )
            .unwrap_err()[0]
                .code,
            "E0101"
        );
        assert_eq!(
            super::super::typeck::finish_enum_pipeline(
                empty(SourceAdmission::EnumPipeline),
                &plan,
                &mut allocator,
                EnumPipelineRequest::REFERENCE
            )
            .unwrap_err()[0]
                .code,
            "E0101"
        );
        assert_eq!(
            super::super::typeck::finish_builtin_source(
                empty(SourceAdmission::BuiltinPipeline),
                &plan,
                &mut allocator,
                0
            )
            .unwrap_err()[0]
                .code,
            "E0500"
        );
        assert_eq!(allocator.attempts, 0);
        assert_eq!(work.used(), before);
    });
}

#[test]
fn bounded_stdin_paid_signatures_split_bodies_and_import_suffix() {
    for (text, bodies, signatures, enumerations) in [
        ("fn main()->i32{return 0;}", 1, 1, 0),
        (
            "use std::io::ReadStatus as S; fn main()->i32{return 0;}",
            1,
            1,
            1,
        ),
        (
            "use std::io::read_stdin as input; fn main()->i32{return 0;}",
            1,
            2,
            1,
        ),
        (BOTH, 3, 4, 2),
    ] {
        with_index(text, |index| {
            let work = WorkMeter::default();
            let (plan, heap) = super::super::reviewer_source::integration_measured(|| {
                super::super::hir_budget::preflight_builtin_hir(index, &work)
            });
            let plan = plan.unwrap().unwrap();
            assert_eq!(heap, (0, 0, 0));
            assert_eq!(
                (plan.counts.functions, plan.counts.signatures),
                (bodies, signatures)
            );
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(512).unwrap();
            let (observed, (_, live, peak)) =
                super::super::reviewer_source::integration_measured(|| {
                    let typed = type_builtin_source(index, &work, &mut allocator).unwrap();
                    assert_eq!(typed.admission(), SourceAdmission::BuiltinPipeline);
                    assert!(!typed.admission().executable());
                    assert!(typed.admission().allows_lowering());
                    typed.validate_function_signatures().unwrap();
                    let observed = (
                        typed.functions().len(),
                        typed.signatures().len(),
                        typed.index().enum_count(),
                        typed.source_storage_bytes().unwrap(),
                    );
                    drop(typed);
                    observed
                });
            assert_eq!(
                (observed.0, observed.1, observed.2),
                (bodies, signatures, enumerations)
            );
            assert!(observed.3 >= plan.total);
            assert_eq!(live, 0);
            assert!(peak > 0);
            assert!(!allocator.observer_trace_overflow);
            let reserved_signatures: usize = allocator
                .trace
                .iter()
                .filter(|event| event.kind == "paid HIR signatures")
                .map(|event| event.length)
                .sum();
            let reserved_functions: usize = allocator
                .trace
                .iter()
                .filter(|event| event.kind == "paid HIR functions")
                .map(|event| event.length)
                .sum();
            assert_eq!(
                (reserved_functions, reserved_signatures),
                (bodies, signatures)
            );
        });
    }
}

#[test]
fn bounded_stdin_paid_signature_identity_rejects_every_suffix_mutation() {
    for current in [false, true] {
        with_index_kind(BOTH, current, |index| {
            for mutation in 0..8 {
                let work = WorkMeter::default();
                let plan = if current {
                    super::super::hir_budget::preflight_current_hir(index, &work)
                } else {
                    super::super::hir_budget::preflight_builtin_hir(index, &work)
                }
                .unwrap()
                .unwrap();
                let mut allocator = Allocator::default();
                let mut paid = PaidStorage::new(plan.counts);
                let (records, signatures, functions) =
                    resolve_index_impl(index, &work, &mut allocator, Some(&mut paid)).unwrap();
                let mut program = ResolvedOwnedProgram {
                    projection_bytes: std::cell::Cell::new(plan.total),
                    admission: if current {
                        SourceAdmission::Executable
                    } else {
                        SourceAdmission::BuiltinPipeline
                    },
                    index: IndexOwner::Borrowed(index),
                    work: MeterOwner::Borrowed(&work),
                    sources: index.sources().view(),
                    records,
                    signatures,
                    functions,
                    entry: index.root_original_main(),
                };
                program.validate_function_signatures().unwrap();
                let suffix = index.source_function_count();
                assert_eq!(
                    program.signatures[suffix].params.as_slice(),
                    [ParameterTy::Reference {
                        referent: BorrowedTy::ScalarSlice(Ty::I32),
                        kind: BorrowKind::Exclusive,
                    }]
                );
                assert_eq!(
                    program.signatures[suffix].result,
                    ValueTy::Owned(AggregateTy::Enum(
                        crate::frontend::oir::owned_types::EnumId(1)
                    ))
                );
                match mutation {
                    0 => {
                        program.signatures.pop();
                    }
                    1 => program.signatures.push(Signature {
                        params: Vec::new(),
                        result: ValueTy::Scalar(Ty::Unit),
                        span: index.sources().eof(),
                    }),
                    2 => program.signatures[suffix].params.clear(),
                    3 => {
                        program.signatures[suffix].params[0] = ParameterTy::Reference {
                            referent: BorrowedTy::ScalarSlice(Ty::I32),
                            kind: BorrowKind::Shared,
                        }
                    }
                    4 => {
                        program.signatures[suffix].result = ValueTy::Owned(AggregateTy::Enum(
                            crate::frontend::oir::owned_types::EnumId(0),
                        ))
                    }
                    5 => program.signatures[suffix].span = index.sources().eof(),
                    6 => program.signatures.swap(0, suffix),
                    7 => program.functions[0].id = DefId(suffix),
                    _ => unreachable!(),
                }
                assert_eq!(
                    program.validate_function_signatures().unwrap_err().code,
                    "E0500",
                    "mutation {mutation}"
                );
                if current {
                    let resolver_end = allocator.attempts;
                    assert_eq!(
                        super::super::typeck::finish_enum_source(
                            program,
                            &plan,
                            &mut allocator,
                            resolver_end
                        )
                        .unwrap_err()[0]
                            .code,
                        "E0500",
                        "production mutation {mutation}"
                    );
                    assert_eq!(allocator.attempts, resolver_end);
                }
            }
        });
    }
}

#[test]
fn bounded_stdin_candidate_rejects_earlier_paid_consumers_before_reservation() {
    for text in [
        "fn main()->i32{return 0;}",
        "use std::io::ReadStatus; fn main()->i32{return 0;}",
        BOTH,
    ] {
        with_index(text, |index| {
            let work = WorkMeter::default();
            let mut allocator = Allocator {
                fail_at: Some(1),
                ..Default::default()
            };
            assert!(type_enum_source(index, &work, &mut allocator).is_err());
            assert!(probe_enum_resolver_storage(index, &work, &mut allocator).is_err());
            assert!(probe_enum_type_storage(index, &work, &mut allocator).is_err());
            assert!(probe_enum_pipeline(
                index,
                &work,
                &mut allocator,
                EnumPipelineRequest::REFERENCE
            )
            .is_err());
            assert!(super::super::hir_budget::preflight_enum_hir(index, &work).is_err());
            assert!(super::super::hir_budget::preflight_current_hir(index, &work).is_err());
            assert_eq!(allocator.attempts, 0);
        });
    }
}

#[test]
fn bounded_stdin_paid_calls_reject_wrong_element_and_borrow_kinds() {
    for text in [
        "use std::io::read_stdin as input; fn main()->i32{let mut a=[true];let s=input(&mut a);return 0;}",
        "use std::io::read_stdin as input; fn main()->i32{let a=[0];let s=input(&a);return 0;}",
        "use std::io::read_stdin as input; fn main()->i32{let a=[0];let s=input(a);return 0;}",
        "use std::io::read_stdin as input; enum Same{Eof(i32),Full,IoError} fn main()->i32{let mut a=[0];let s:Same=input(&mut a);return 0;}",
    ] {
        with_index(text, |index| {
            let errors = type_builtin_source(index, &WorkMeter::default(), &mut Allocator::default()).unwrap_err();
            assert_eq!(errors[0].code, "E0300", "{text}: {errors:?}");
        });
    }
}

#[test]
fn bounded_stdin_paid_allocations_fail_at_each_requested_reservation() {
    with_index(BOTH, |index| {
        let work = WorkMeter::default();
        let mut baseline = Allocator::default();
        drop(type_builtin_source(index, &work, &mut baseline).unwrap());
        assert!(baseline.attempts > 1);
        for attempt in 1..=baseline.attempts {
            let work = WorkMeter::default();
            let mut allocator = Allocator {
                fail_at: Some(attempt),
                ..Default::default()
            };
            allocator.observer_trace_bound(512).unwrap();
            let ((failed, code), (_, live, _)) =
                super::super::reviewer_source::integration_measured(|| {
                    match type_builtin_source(index, &work, &mut allocator) {
                        Ok(typed) => {
                            drop(typed);
                            (false, false)
                        }
                        Err(errors) => (true, errors[0].code == "E0400"),
                    }
                });
            assert!(failed && code, "allocation {attempt}");
            assert_eq!(live, 0, "allocation {attempt}");
            assert!(allocator.attempts >= attempt);
            assert!(!allocator.trace[attempt - 1].success);
            assert!(allocator.trace[..attempt - 1]
                .iter()
                .all(|event| event.success));
            assert!(!allocator.observer_trace_overflow);
        }
    });
}

#[test]
fn bounded_stdin_paid_work_endpoint_is_exact() {
    for current in [false, true] {
        with_index_kind(BOTH, current, |index| {
            let measured = WorkMeter::default();
            let baseline = if current {
                type_enum_source(index, &measured, &mut Allocator::default())
            } else {
                type_builtin_source(index, &measured, &mut Allocator::default())
            };
            drop(baseline.unwrap());
            let required = measured.used();
            assert!(required > 0);
            for limit in [required - 1, required] {
                let work = WorkMeter::new(limit);
                let mut allocator = Allocator::default();
                allocator.observer_trace_bound(512).unwrap();
                let ((success, resource), (_, live, _)) =
                    super::super::reviewer_source::integration_measured(|| {
                        let result = if current {
                            type_enum_source(index, &work, &mut allocator)
                        } else {
                            type_builtin_source(index, &work, &mut allocator)
                        };
                        match result {
                            Ok(typed) => {
                                drop(typed);
                                (true, false)
                            }
                            Err(errors) => {
                                (false, errors.iter().any(|error| error.code == "E0400"))
                            }
                        }
                    });
                assert_eq!(success, limit == required);
                assert_eq!(resource, limit < required);
                assert_eq!(live, 0);
            }
            println!("BUILTIN_PAID_WORK current={current} exact={required}");
        });
    }
}

#[test]
fn bounded_stdin_builtin_parameter_real_null_drops_prior_signatures() {
    use crate::frontend::{
        oir::owned::reviewer_origins as raw, project::budget::real_null_observer as null,
    };
    use std::{alloc::Layout, mem::size_of};

    // Source-derived requests, not calibrated from a successful execution:
    // Records(0), Signatures(2), main Parameters(0), builtin Parameters(1).
    const TEXT: &str = "use std::io::read_stdin; fn main()->i32{return 0;}";
    for current in [false, true] {
        with_index_kind(TEXT, current, |index| {
            let expected_anchor = Span {
                file: index.sources().eof().file,
                start: 13,
                end: 23,
            };
            assert_eq!(
                index
                    .builtin_function_anchor(BuiltinFunction::ReadStdin)
                    .unwrap(),
                expected_anchor
            );
            let work = WorkMeter::default();
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(8).unwrap();
            let trace_capacity = allocator.trace.capacity();
            let target = null::Target {
                attempt: 4,
                kind: "paid HIR parameters",
                slots: 1,
                element_bytes: size_of::<ParameterTy>(),
                layout: Layout::array::<ParameterTy>(1).unwrap(),
            };
            let action = |allocator: &mut Allocator| {
                raw::integration_counted(|| {
                    super::super::reviewer_source::integration_measured(|| {
                        let result = if current {
                            type_enum_source(index, &work, allocator)
                        } else {
                            type_builtin_source(index, &work, allocator)
                        };
                        match result {
                            Ok(typed) => {
                                drop(typed);
                                (false, 0, None, None, None, false)
                            }
                            Err(errors) => (
                                true,
                                errors.len(),
                                errors.first().map(|error| error.code),
                                errors.first().map(|error| error.stage),
                                errors.first().and_then(|error| error.primary),
                                errors.iter().all(|error| {
                                    error.message == "affected HIR allocation failed"
                                        && error.secondary.is_empty()
                                        && error.notes.is_empty()
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
            assert_eq!(allocator.attempts, 4);
            assert_eq!(allocator.trace.len(), 4);
            assert_eq!(allocator.trace.capacity(), trace_capacity);
            for (row, expected) in allocator.trace.iter().zip([
                ("paid HIR records", 0, size_of::<Record>(), true),
                ("paid HIR signatures", 2, size_of::<Signature>(), true),
                ("paid HIR parameters", 0, size_of::<ParameterTy>(), true),
                ("paid HIR parameters", 1, size_of::<ParameterTy>(), false),
            ]) {
                assert_eq!(
                    (row.kind, row.length, row.element_bytes, row.success),
                    expected
                );
            }
            assert!(!allocator.observer_trace_overflow);
            assert!(
                !super::super::reviewer_source::integration_enabled()
                    && !raw::integration_enabled()
            );
            println!("BUILTIN_PARAMETER_REAL_NULL current={current} attempt=4 slots=1 width={} layout={:?} controls={control_bytes} live={} peak={}", target.element_bytes, target.layout, stats.1, stats.2);
        });
    }
}

#[test]
fn bounded_stdin_paid_new_carriers_have_actual_layout_receipts() {
    use std::mem::{align_of, size_of};
    assert_eq!(size_of::<SourceAdmission>(), 1);
    assert!(!SourceAdmission::BuiltinPipeline.executable());
    println!("BUILTIN_PAID_LAYOUT index_owner={}/{} resolved={}/{} typed={}/{} builtin_signature={}/{} signature_identity={}/{} source_dispatch={} type_dispatch={}",
        size_of::<IndexOwner<'static>>(), align_of::<IndexOwner<'static>>(),
        size_of::<ResolvedOwnedProgram<'static>>(), align_of::<ResolvedOwnedProgram<'static>>(),
        size_of::<super::super::typeck::TypedOwnedProgram<'static>>(), align_of::<super::super::typeck::TypedOwnedProgram<'static>>(),
        size_of::<BuiltinSignatureCarriers>(), align_of::<BuiltinSignatureCarriers>(),
        size_of::<SignatureIdentityCarriers>(), align_of::<SignatureIdentityCarriers>(),
        production_source_carrier_bytes(), super::super::typeck::production_type_carrier_bytes());
}
