use super::*;
use crate::frontend::{
    lexer, parser,
    source::{SourceFileId, SourceMap, Span},
};

fn empty() -> hir::Program {
    hir::Program {
        signatures: Vec::new(),
        functions: Vec::new(),
    }
}
fn span() -> Span {
    Span {
        file: SourceFileId(0),
        start: 0,
        end: 0,
    }
}
fn signature(params: Vec<hir::Ty>) -> hir::Signature {
    hir::Signature {
        params,
        result: hir::Ty::Unit,
        span: span(),
    }
}
fn canonical_rich() -> hir::Program {
    let mut sources = SourceMap::new();
    let id = sources.add(
        "allocation-shape.ox".into(),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/checked_hir_import/rich-source.txt"
        ))
        .into(),
    );
    let file = sources.get(id);
    let ast = parser::parse(file, lexer::lex(file).unwrap()).unwrap();
    hir::resolve(file, &ast).unwrap()
}

// Independent fixture schedule. A zero-length parameter vector still occupies
// ordinal 3, but consumes no allocation attempt. No trace generates this table.
const RICH: [(Key, usize); 17] = [
    (Key::Signatures, 2),
    (Key::Functions, 2),
    (Key::Parameters(0), 1),
    (Key::Parameters(1), 0),
    (Key::Locals(0), 1),
    (Key::Expressions(0), 1),
    (Key::Blocks(0), 1),
    (
        Key::Statements {
            function: 0,
            block: 0,
        },
        1,
    ),
    (Key::Locals(1), 1),
    (Key::Expressions(1), 10),
    (Key::Blocks(1), 4),
    (
        Key::Statements {
            function: 1,
            block: 0,
        },
        3,
    ),
    (
        Key::Statements {
            function: 1,
            block: 1,
        },
        1,
    ),
    (
        Key::Statements {
            function: 1,
            block: 2,
        },
        1,
    ),
    (
        Key::Statements {
            function: 1,
            block: 3,
        },
        1,
    ),
    (
        Key::Arguments {
            function: 1,
            expression: 1,
        },
        1,
    ),
    (
        Key::Arguments {
            function: 1,
            expression: 9,
        },
        1,
    ),
];

// Shape/reserve qualification only: drop the unfilled vector, never assemble a
// Program, and do not claim successful construction or candidate cleanup proof.
fn reserve_and_drop(session: &Session<'_, '_>, key: Key, slots: usize) -> Result<(), Failure> {
    match key.family() {
        Family::Signatures => drop(session.reserve::<hir::Signature>(key, slots)?),
        Family::Functions => drop(session.reserve::<hir::Function>(key, slots)?),
        Family::Parameters => drop(session.reserve::<hir::Ty>(key, slots)?),
        Family::Locals => drop(session.reserve::<hir::Local>(key, slots)?),
        Family::Expressions => drop(session.reserve::<hir::Expr>(key, slots)?),
        Family::Blocks => drop(session.reserve::<hir::BodyBlock>(key, slots)?),
        Family::Statements => drop(session.reserve::<hir::Stmt>(key, slots)?),
        Family::Arguments => drop(session.reserve::<hir::ExprId>(key, slots)?),
    }
    Ok(())
}

#[test]
fn checked_hir_allocation_empty_visits_and_finishes_without_requests() {
    let canonical = empty();
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(0).unwrap();
    {
        let session =
            Session::admit(&canonical, &mut allocator, 0, IndexLimits::default()).unwrap();
        let signatures = session
            .reserve::<hir::Signature>(Key::Signatures, 0)
            .unwrap();
        let functions = session.reserve::<hir::Function>(Key::Functions, 0).unwrap();
        assert_eq!(signatures.values.capacity(), 0);
        assert_eq!(functions.values.capacity(), 0);
        drop(signatures.finish().unwrap());
        drop(functions.finish().unwrap());
        let receipt = session.complete().unwrap();
        assert_eq!(receipt.requested, Counts::default());
        assert_eq!(receipt.vectors, 2);
        assert_eq!(receipt.reserves, 0);
        assert_eq!(receipt.candidate_request_bytes, 0);
        assert_eq!(receipt.canonical_payload_bytes, 0);
        assert_eq!(receipt.affected_bytes, receipt.fixed_bytes);
    }
    assert_eq!(allocator.attempts, 0);
    assert!(allocator.trace.is_empty());
    assert!(!allocator.observer_trace_overflow);
}

#[test]
fn checked_hir_allocation_rich_has_independent_exact_request_schedule() {
    let canonical = canonical_rich();
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(16).unwrap();
    let trace_capacity = allocator.trace.capacity();
    let receipt;
    {
        let session =
            Session::admit(&canonical, &mut allocator, 0, IndexLimits::default()).unwrap();
        receipt = session.receipt;
        for (ordinal, (key, slots)) in RICH.into_iter().enumerate() {
            assert_eq!(key_at(&canonical, Order::Reserve, ordinal), Some(key));
            reserve_and_drop(&session, key, slots).unwrap();
        }
        // Merely reserving every vector never satisfies completed construction.
        assert!(matches!(session.complete(), Err(Failure::Shape)));
    }
    assert_eq!(receipt.requested, Counts([2, 2, 1, 2, 11, 5, 7, 2]));
    assert_eq!(receipt.vectors, 17);
    assert_eq!(receipt.reserves, 16);
    assert_eq!(allocator.attempts, 16);
    assert_eq!(allocator.trace.len(), 16);
    let mut event = 0;
    let mut bytes = 0;
    for (key, slots) in RICH {
        if slots == 0 {
            continue;
        }
        let observed = &allocator.trace[event];
        assert_eq!(observed.kind, key.family().kind());
        assert_eq!(observed.length, slots);
        assert_eq!(observed.element_bytes, ELEMENT_BYTES[key.family().index()]);
        assert!(observed.success);
        bytes += slots * observed.element_bytes;
        event += 1;
    }
    assert_eq!(bytes, receipt.candidate_request_bytes);
    assert_eq!(allocator.trace.capacity(), trace_capacity);
    assert!(!allocator.observer_trace_overflow);
}

#[test]
fn checked_hir_allocation_every_logical_failure_preserves_absolute_history() {
    let canonical = canonical_rich();
    for preceding in [0, 1] {
        for relative_failure in 1..=16 {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(preceding + 16).unwrap();
            let mut sentinel = Vec::<u8>::new();
            if preceding != 0 {
                allocator
                    .vector_exact(&mut sentinel, 1, "prior request")
                    .unwrap();
            }
            allocator.fail_at = Some(preceding + relative_failure);
            let trace_capacity = allocator.trace.capacity();
            {
                let session =
                    Session::admit(&canonical, &mut allocator, 0, IndexLimits::default()).unwrap();
                let mut nonempty = 0;
                for (key, slots) in RICH {
                    nonempty += usize::from(slots != 0);
                    let result = reserve_and_drop(&session, key, slots);
                    if nonempty == relative_failure {
                        assert_eq!(result, Err(Failure::Allocation));
                        // A failed session cannot reserve again or become complete.
                        assert_eq!(reserve_and_drop(&session, key, slots), Err(Failure::Shape));
                        assert!(matches!(session.complete(), Err(Failure::Shape)));
                        break;
                    }
                    assert_eq!(result, Ok(()));
                }
            }
            assert_eq!(allocator.attempts, preceding + relative_failure);
            assert_eq!(allocator.trace.len(), preceding + relative_failure);
            assert!(allocator.trace[..allocator.trace.len() - 1]
                .iter()
                .all(|e| e.success));
            assert!(!allocator.trace.last().unwrap().success);
            assert_eq!(allocator.trace.capacity(), trace_capacity);
            assert!(!allocator.observer_trace_overflow);
        }
    }
}

#[test]
fn checked_hir_allocation_exact_nested_lengths_not_aggregate_totals() {
    let canonical = hir::Program {
        signatures: vec![
            signature(vec![hir::Ty::I32]),
            signature(vec![hir::Ty::Bool, hir::Ty::Bool]),
        ],
        functions: Vec::new(),
    };
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(3).unwrap();
    {
        let session =
            Session::admit(&canonical, &mut allocator, 0, IndexLimits::default()).unwrap();
        let _signatures = session
            .reserve::<hir::Signature>(Key::Signatures, 2)
            .unwrap();
        let _functions = session.reserve::<hir::Function>(Key::Functions, 0).unwrap();
        // Swapping [1,2] to [2,1] preserves family totals but fails this key.
        assert!(matches!(
            session.reserve::<hir::Ty>(Key::Parameters(0), 2),
            Err(Failure::Shape)
        ));
    }
    assert_eq!(allocator.attempts, 1);
    assert_eq!(Key::Parameters(2).shape(&canonical), Err(Failure::Shape));
    let rich = canonical_rich();
    assert_eq!(
        Key::Arguments {
            function: 0,
            expression: 0
        }
        .shape(&rich),
        Err(Failure::Shape)
    );
    assert_eq!(
        Key::Statements {
            function: 0,
            block: 1
        }
        .shape(&rich),
        Err(Failure::Shape)
    );
}

#[test]
fn checked_hir_allocation_wrong_family_key_and_skipped_order_fail_before_reserve() {
    let canonical = empty();
    for mode in 0..3 {
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(0).unwrap();
        {
            let session =
                Session::admit(&canonical, &mut allocator, 0, IndexLimits::default()).unwrap();
            let result = match mode {
                0 => session
                    .reserve::<hir::Function>(Key::Signatures, 0)
                    .map(drop),
                1 => session
                    .reserve::<hir::Function>(Key::Functions, 0)
                    .map(drop),
                _ => session.reserve::<hir::Ty>(Key::Parameters(0), 0).map(drop),
            };
            assert_eq!(result, Err(Failure::Shape));
        }
        assert_eq!(allocator.attempts, 0);
    }
}

#[test]
fn checked_hir_allocation_function_block_and_call_distribution_mismatches() {
    let mut canonical = canonical_rich();
    // Synthetic shape control only: first/second call lengths become [2,1].
    // Swapping them preserves the argument-family total but not the first key.
    let hir::ExprKind::Call { args, .. } = &mut canonical.functions[1].expressions[1].kind else {
        panic!("RICH first call changed")
    };
    args.push(hir::ExprId(0));
    for (target, wrong_len) in [
        (Key::Expressions(0), 10), // [1,10] -> [10,1]
        (
            Key::Statements {
                function: 1,
                block: 0,
            },
            1,
        ), // [3,1] -> [1,3]
        (
            Key::Arguments {
                function: 1,
                expression: 1,
            },
            1,
        ), // [2,1] -> [1,2]
    ] {
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(16).unwrap();
        let mut successful = 0;
        {
            let session =
                Session::admit(&canonical, &mut allocator, 0, IndexLimits::default()).unwrap();
            for (key, _) in RICH {
                if key == target {
                    assert_eq!(
                        reserve_and_drop(&session, key, wrong_len),
                        Err(Failure::Shape)
                    );
                    break;
                }
                let (slots, _) = key.shape(&canonical).unwrap();
                reserve_and_drop(&session, key, slots).unwrap();
                successful += usize::from(slots != 0);
            }
        }
        assert_eq!(allocator.attempts, successful);
        assert_eq!(allocator.trace.len(), successful);
        assert!(allocator.trace.iter().all(|event| event.success));
    }
}

#[test]
fn checked_hir_allocation_finish_order_duplicate_and_missing_are_rejected() {
    let canonical = empty();
    for mode in 0..3 {
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(0).unwrap();
        let session =
            Session::admit(&canonical, &mut allocator, 0, IndexLimits::default()).unwrap();
        let signatures = session
            .reserve::<hir::Signature>(Key::Signatures, 0)
            .unwrap();
        let functions = session.reserve::<hir::Function>(Key::Functions, 0).unwrap();
        match mode {
            0 => assert!(matches!(functions.finish(), Err(Failure::Shape))),
            1 => {
                let request = signatures.request;
                drop(signatures.finish().unwrap());
                // Only this nested test can fabricate the inaccessible fields.
                let duplicate = ExactVec::<hir::Signature> {
                    owner: &session,
                    request,
                    values: Vec::new(),
                };
                assert!(matches!(duplicate.finish(), Err(Failure::Shape)));
            }
            _ => {
                drop(signatures.finish().unwrap());
                drop(functions);
                assert!(matches!(session.complete(), Err(Failure::Shape)));
            }
        }
    }
}

#[test]
fn checked_hir_allocation_push_guards_zero_one_boundary_and_underfill() {
    for slots in [0, 1, MAX_ROWS] {
        let canonical = hir::Program {
            signatures: vec![signature(vec![hir::Ty::I32; slots])],
            functions: Vec::new(),
        };
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(2).unwrap();
        let session =
            Session::admit(&canonical, &mut allocator, 0, IndexLimits::default()).unwrap();
        let mut signatures = session
            .reserve::<hir::Signature>(Key::Signatures, 1)
            .unwrap();
        let functions = session.reserve::<hir::Function>(Key::Functions, 0).unwrap();
        let mut params = session
            .reserve::<hir::Ty>(Key::Parameters(0), slots)
            .unwrap();
        for index in 0..slots {
            params.push(index, hir::Ty::I32).unwrap();
        }
        assert_eq!(params.values.len(), slots);
        assert_eq!(params.values.capacity(), slots);
        signatures
            .push(0, signature(params.finish().unwrap()))
            .unwrap();
        drop(signatures.finish().unwrap());
        drop(functions.finish().unwrap());
        assert_eq!(session.complete().unwrap().requested.0[2], slots);
    }

    for mode in 0..3 {
        let canonical = hir::Program {
            signatures: vec![signature(vec![hir::Ty::I32])],
            functions: Vec::new(),
        };
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(2).unwrap();
        let session =
            Session::admit(&canonical, &mut allocator, 0, IndexLimits::default()).unwrap();
        let _signatures = session
            .reserve::<hir::Signature>(Key::Signatures, 1)
            .unwrap();
        let _functions = session.reserve::<hir::Function>(Key::Functions, 0).unwrap();
        let mut params = session.reserve::<hir::Ty>(Key::Parameters(0), 1).unwrap();
        match mode {
            0 => assert!(matches!(params.finish(), Err(Failure::Shape))),
            1 => assert_eq!(params.push(1, hir::Ty::I32), Err(Failure::Shape)),
            _ => {
                params.push(0, hir::Ty::I32).unwrap();
                assert_eq!(params.push(1, hir::Ty::I32), Err(Failure::Shape));
            }
        }
    }
    let spare = Vec::<hir::ExprId>::with_capacity(5);
    assert_eq!(exact_capacity(&spare, 4), Err(Failure::Shape));
    assert_eq!(exact_capacity(&spare, 6), Err(Failure::Shape));
    assert_eq!(exact_capacity(&spare, 5), Ok(()));
}

#[test]
fn checked_hir_allocation_admission_prices_canonical_capacity_and_fixed_roles() {
    let canonical = hir::Program {
        signatures: Vec::with_capacity(7),
        functions: Vec::with_capacity(9),
    };
    let outside = 137;
    let mut allocator = Allocator::default();
    let receipt = {
        let session =
            Session::admit(&canonical, &mut allocator, outside, IndexLimits::default()).unwrap();
        session.receipt
    };
    assert_eq!(receipt.candidate_request_bytes, 0);
    assert_eq!(
        receipt.canonical_payload_bytes,
        canonical.signatures.capacity() * size_of::<hir::Signature>()
            + canonical.functions.capacity() * size_of::<hir::Function>()
    );
    assert_eq!(receipt.fixed_bytes, outside + helper_named_bytes().unwrap());
    assert_eq!(
        receipt.affected_bytes,
        receipt.canonical_payload_bytes + receipt.fixed_bytes
    );
    let exact = IndexLimits {
        retained: receipt.affected_bytes as u64,
        scratch: receipt.fixed_bytes as u64,
        work: receipt.helper_work,
    };
    assert!(Session::admit(&canonical, &mut allocator, outside, exact).is_ok());
    for limits in [
        IndexLimits {
            retained: exact.retained - 1,
            ..exact
        },
        IndexLimits {
            scratch: exact.scratch - 1,
            ..exact
        },
        IndexLimits {
            work: exact.work - 1,
            ..exact
        },
    ] {
        assert!(matches!(
            Session::admit(&canonical, &mut allocator, outside, limits),
            Err(Failure::Admission)
        ));
    }
    assert_eq!(allocator.attempts, 0);
    assert!(matches!(
        Session::admit(&canonical, &mut allocator, usize::MAX, exact),
        Err(Failure::Overflow)
    ));
}

#[test]
fn checked_hir_allocation_limits_cannot_be_raised_and_arithmetic_is_checked() {
    let high = IndexLimits {
        retained: u64::MAX,
        scratch: u64::MAX,
        work: u64::MAX,
    };
    let ceilings = IndexLimits::default();
    assert_eq!(
        admit_limits(
            ceilings.retained as usize,
            ceilings.scratch as usize,
            ceilings.work,
            high
        ),
        Ok(())
    );
    assert_eq!(
        admit_limits(ceilings.retained as usize + 1, 0, 0, high),
        Err(Failure::Admission)
    );
    assert_eq!(
        admit_limits(0, ceilings.scratch as usize + 1, 0, high),
        Err(Failure::Admission)
    );
    assert_eq!(
        admit_limits(0, 0, ceilings.work + 1, high),
        Err(Failure::Admission)
    );
    assert_eq!(checked_sum([usize::MAX, 1]), Err(Failure::Overflow));
    assert!(matches!(
        checked_layout::<hir::ExprId>(usize::MAX),
        Err(Failure::Overflow)
    ));
    assert!(matches!(
        checked_layout::<u8>(isize::MAX as usize + 1),
        Err(Failure::Overflow)
    ));
    assert!(matches!(checked_layout::<()>(0), Err(Failure::Shape)));
    let inventory = Inventory {
        vectors: usize::MAX,
        ..Inventory::default()
    };
    assert_eq!(work_bound(&inventory), Err(Failure::Overflow));
    let bounded = Inventory {
        vectors: MAX_VECTORS,
        nonempty: MAX_VECTORS,
        requested: Counts([MAX_ROWS; 8]),
        canonical_capacity: Counts::default(),
    };
    assert!(work_bound(&bounded).unwrap() <= ceilings.work);
}

#[test]
fn checked_hir_allocation_attempt_overflow_precedes_every_request() {
    let canonical = hir::Program {
        signatures: vec![signature(Vec::new())],
        functions: Vec::new(),
    };
    let mut allocator = Allocator {
        attempts: usize::MAX,
        ..Allocator::default()
    };
    assert!(matches!(
        Session::admit(&canonical, &mut allocator, 0, IndexLimits::default()),
        Err(Failure::Overflow)
    ));
    assert_eq!(allocator.attempts, usize::MAX);
    assert!(allocator.trace.is_empty());
    let empty = empty();
    let session = Session::admit(&empty, &mut allocator, 0, IndexLimits::default()).unwrap();
    drop(
        session
            .reserve::<hir::Signature>(Key::Signatures, 0)
            .unwrap()
            .finish()
            .unwrap(),
    );
    drop(
        session
            .reserve::<hir::Function>(Key::Functions, 0)
            .unwrap()
            .finish()
            .unwrap(),
    );
    assert_eq!(session.complete().unwrap().reserves, 0);
}

#[test]
fn checked_hir_allocation_size_receipt_is_named_not_a_stack_measurement() {
    eprintln!(
        "checked-hir-allocation sizes: helper={} session={} inventory={} request={} progress={} receipt={} allocator={} element_bytes={:?} alignments={:?} max_vectors={}",
        helper_named_bytes().unwrap(), size_of::<Session<'_, '_>>(), size_of::<Inventory>(),
        size_of::<Request>(), size_of::<Progress>(), size_of::<Receipt>(), size_of::<Allocator>(),
        ELEMENT_BYTES, [std::mem::align_of::<hir::Signature>(), std::mem::align_of::<hir::Function>(),
            std::mem::align_of::<hir::Ty>(), std::mem::align_of::<hir::Local>(), std::mem::align_of::<hir::Expr>(),
            std::mem::align_of::<hir::BodyBlock>(), std::mem::align_of::<hir::Stmt>(), std::mem::align_of::<hir::ExprId>()], MAX_VECTORS,
    );
    assert!(helper_named_bytes().unwrap() < IndexLimits::default().scratch as usize);
}
