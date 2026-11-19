//! Existing entry policies over the genuine extended witness. Process checks
//! stay effect-free and never establish the test runner's global SIGPIPE policy.
use super::*;
use crate::frontend::oir::owned::{
    owned_vector_descriptors::tests as fixture, verified::VerifiedOwnedVectorProgram,
};
use crate::frontend::project::budget::Allocator;

fn scalar_witness(ty: hir::Ty, value: Rvalue) -> (SourceMap, VerifiedOwnedVectorProgram) {
    let (sources, mut raw, keys, imports) = fixture::fixture(None);
    let span = raw.records[0].span;
    let mut main = consumer_fixtures::function(0, ValueTy::Scalar(ty), span);
    main.locals.push(consumer_fixtures::scalar(ty, span));
    main.blocks.push(OwnedBlock {
        span,
        merge: None,
        statements: vec![consumer_fixtures::assign(0, value, span)],
        terminator: consumer_fixtures::end(
            OwnedTerminatorKind::ReturnScalar(consumer_fixtures::operand(0, span)),
            span,
        ),
    });
    for function in &mut raw.functions {
        function.id.0 += 1;
    }
    raw.functions.insert(0, main);
    let wrapper = fixture::prepare(&sources, raw, &keys, imports);
    let witness = VerifiedOwnedVectorProgram::verify(
        wrapper,
        &sources,
        budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap();
    (sources, witness)
}

#[test]
fn owned_vector_entry_existing_result_interfaces_execute_and_release_live_children() {
    let (_, witness, _, expected) = fixture::execution_fixture::resources::buffers_with_live_take();
    let entry = Some(hir::DefId(0));
    for execution in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
        let limits = Limits {
            fuel: if execution.hosted() {
                0
            } else {
                Limits::default().fuel
            },
            ..Limits::default()
        };
        for scalar_api in [false, true] {
            let (result, observation) = byte_buffer::testing::capture(Default::default(), || {
                if scalar_api {
                    run_value(&witness, entry, limits, execution).map(RuntimeValue::from)
                } else {
                    run_result(&witness, entry, limits, execution)
                }
            });
            assert_eq!(result, Ok(RuntimeValue::from(Scalar::I32(expected))));
            assert_eq!(observation.before_cleanup.live_allocations, 0);
            assert_eq!(observation.before_cleanup.allocations, 3);
            assert_eq!(observation.before_cleanup.releases, 3);
            assert_eq!(observation.before_cleanup.peak_bytes, 360);
            assert_eq!(observation.before_cleanup, observation.after_cleanup);
        }
        assert_eq!(
            run_with_execution(&witness, entry, execution),
            Ok(Scalar::I32(expected))
        );
    }
    assert_eq!(run(&witness, entry), Ok(Scalar::I32(expected)));
    assert_eq!(
        run_limits(&witness, entry, Limits::default()),
        Ok(Scalar::I32(expected))
    );
    for result in [
        run_result(
            &witness,
            entry,
            Limits {
                fuel: 0,
                ..Limits::default()
            },
            ExecutionPolicy::Bounded,
        ),
        run_value(
            &witness,
            entry,
            Limits {
                fuel: 0,
                ..Limits::default()
            },
            ExecutionPolicy::Bounded,
        )
        .map(RuntimeValue::from),
    ] {
        assert!(matches!(
            result,
            Err(OwnedRunFailure::Scalar(RunFailure::Fuel(_)))
        ));
    }
}

#[test]
fn owned_vector_entry_typed_results_preserve_byte_wide_and_quad_policies() {
    for (ty, value, expected, scalar) in [
        (
            hir::Ty::U8,
            Rvalue::U8(255),
            RuntimeValue::from(Scalar::U8(255)),
            Some(RuntimeScalar::from(Scalar::U8(255))),
        ),
        (
            hir::Ty::I64,
            Rvalue::I64(i64::MAX),
            RuntimeValue::from(RuntimeScalar::I64(i64::MAX)),
            Some(RuntimeScalar::I64(i64::MAX)),
        ),
        (
            hir::Ty::I128,
            Rvalue::Integer128 {
                ty: hir::Ty::I128,
                low: 0,
                high: 1 << 63,
            },
            RuntimeValue::I128(i128::MIN),
            None,
        ),
        (
            hir::Ty::U128,
            Rvalue::Integer128 {
                ty: hir::Ty::U128,
                low: u64::MAX,
                high: u64::MAX,
            },
            RuntimeValue::U128(u128::MAX),
            None,
        ),
    ] {
        let (_sources, witness) = scalar_witness(ty, value);
        let entry = Some(hir::DefId(0));
        for execution in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            assert_eq!(
                run_result(&witness, entry, Limits::default(), execution),
                Ok(expected)
            );
            let actual = run_value(&witness, entry, Limits::default(), execution);
            if let Some(expected) = scalar {
                assert_eq!(actual, Ok(expected));
            } else {
                assert_eq!(
                    actual,
                    Err(RunFailure::QuadEntry(witness.functions()[0].span).into())
                );
            }
        }
    }
}

#[test]
fn owned_vector_entry_denials_precede_allocation_for_unchanged_scalar_interfaces() {
    let (_sources, witness) = scalar_witness(hir::Ty::U8, Rvalue::U8(7));
    let entry = Some(hir::DefId(0));
    let span = witness.functions()[0].span;
    for execution in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
        assert_eq!(
            plan::fail_allocation_after(0, || run_with_execution(&witness, entry, execution)),
            Err(RunFailure::ByteEntry(span).into()),
        );
    }
    assert_eq!(
        plan::fail_allocation_after(0, || run(&witness, entry)),
        Err(RunFailure::ByteEntry(span).into()),
    );
    assert_eq!(
        plan::fail_allocation_after(0, || run_limits(
            &witness,
            entry,
            Limits {
                fuel: 0,
                ..Limits::default()
            }
        )),
        Err(RunFailure::ByteEntry(span).into()),
    );
    for (entry, expected) in [
        (None, RunFailure::Entry(None).into()),
        (
            Some(hir::DefId(usize::MAX)),
            OwnedRunFailure::Invariant("entry identity", None),
        ),
        // The shared scalar-vector new body has no parameters and an owned result.
        (
            Some(hir::DefId(1)),
            OwnedRunFailure::EntryResult(witness.functions()[1].span),
        ),
        // The owned-record vector len body retains its shared-reference parameter.
        (
            Some(hir::DefId(3)),
            RunFailure::Entry(Some(witness.functions()[3].span)).into(),
        ),
    ] {
        assert_eq!(
            plan::fail_allocation_after(0, || run_result(
                &witness,
                entry,
                Limits::default(),
                ExecutionPolicy::Bounded
            )),
            Err(expected),
        );
    }
}

#[test]
fn owned_vector_entry_process_signatures_and_result_policy_require_no_global_setup() {
    type Witness = VerifiedOwnedVectorProgram;
    let _bounded: fn(&Witness, Option<hir::DefId>, Limits) -> Result<Scalar> =
        run_process_limits::<Witness>;
    let _execution: fn(&Witness, Option<hir::DefId>, ExecutionPolicy) -> Result<Scalar> =
        run_process_with_execution::<Witness>;
    let _policy: fn(&Witness, Option<hir::DefId>, Limits, ExecutionPolicy) -> Result<Scalar> =
        run_process_policy::<Witness>;
    let (_sources, byte) = scalar_witness(hir::Ty::U8, Rvalue::U8(7));
    let span = byte.functions()[0].span;
    assert_eq!(
        plan::fail_allocation_after(0, || checked_entry_policy(
            &byte,
            Some(hir::DefId(0)),
            EntryPolicy::Process
        )),
        if output::supported_host() {
            Err(OwnedRunFailure::ProcessEntry(span))
        } else {
            Err(OwnedRunFailure::ProcessHost)
        },
    );
    for value in [58, 256] {
        let (_sources, witness) = scalar_witness(hir::Ty::I32, Rvalue::I32(value));
        let admitted = plan::fail_allocation_after(0, || {
            checked_entry_policy(&witness, Some(hir::DefId(0)), EntryPolicy::Process)
        });
        if !output::supported_host() {
            assert_eq!(admitted, Err(OwnedRunFailure::ProcessHost));
            continue;
        }
        let entry = admitted.unwrap();
        let plan = ExecutionPlan::build(&witness).unwrap();
        for execution in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let actual = execute_result_plan(
                &plan,
                entry,
                Limits::default(),
                EntryPolicy::Process,
                execution,
            );
            if value == 58 {
                assert_eq!(actual, Ok(RuntimeValue::from(Scalar::I32(58))));
            } else {
                assert_eq!(
                    actual,
                    Err(OwnedRunFailure::ProcessResult(witness.functions()[0].span))
                );
            }
        }
    }
}
