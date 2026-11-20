//! Plan-relation and admission controls, independent of the source parser.
use super::*;

fn raw_relation() -> (RawOwnedFunction, Operand) {
    let (_, mut raw, _) = super::super::super::consumer_fixtures::empty_record();
    let mut function = raw.functions.remove(0);
    let span = function.span;
    function.owners[0].aggregate =
        AggregateSlot::try_from_aggregate(AggregateTy::OwnedRecordVector(RecordId(0))).unwrap();
    function.locals.push(LocalDecl {
        ty: hir::Ty::I32,
        kind: LocalKind::Temporary,
        span,
    });
    let index = Operand {
        local: LocalId(function.locals.len() - 1),
        span,
    };
    function.loans.push(LoanDecl {
        region: LoanRegion::Local,
        authority: AccessBase::Owner(OwnerPlaceId(0)),
        projection: Vec::new(),
        kind: BorrowKind::Shared,
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        span,
    });
    function.blocks[0].statements.push(OwnedStatement {
        kind: OwnedInstruction::AcquireLocalElementBorrow {
            loan: LoanId(0),
            index,
        },
        span,
        diagnostic_origins: None,
    });
    (function, index)
}

fn table(function: &RawOwnedFunction) -> Result<Vec<Option<Operand>>, AdmissionFailure> {
    let mut bytes = requested_len(function) * size_of::<Option<Operand>>();
    build(function, &mut bytes, MAX_PLAN_BYTES)
}

#[test]
fn immutable_relation_getter_retains_the_index_without_raw_statement_access() {
    let (mut function, index) = raw_relation();
    let indices = table(&function).unwrap();
    assert_eq!(indices.len(), function.loans.len());
    let plan = FunctionPlan {
        usage: FrameUsage::default(),
        owner_offsets: Vec::new(),
        calls: Vec::new(),
        owned_stages: Vec::new(),
        borrowed_loans: Vec::new(),
        element_indices: indices,
    };
    // FunctionPlan owns only the relation, never the raw function/blocks.
    function.blocks.clear();
    assert_eq!(plan.element_index(LoanId(0)), Some(index));
    assert_eq!(plan.element_index(LoanId(1)), None);
}

#[test]
fn duplicate_missing_wrong_nominal_region_and_scalar_sites_cannot_form_a_table() {
    for mutation in 0..6 {
        let (mut function, _) = raw_relation();
        match mutation {
            0 => {
                let duplicate = function.blocks[0].statements.last().unwrap().clone();
                function.blocks[0].statements.push(duplicate);
            }
            1 => {
                function.blocks[0].statements.pop();
            }
            2 => {
                function.loans[0].referent =
                    BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(1))))
                        .unwrap();
            }
            3 => {
                function.loans[0].region = LoanRegion::Call {
                    call: CallSiteId(0),
                    argument: 0,
                };
            }
            4 => {
                function.locals.last_mut().unwrap().ty = hir::Ty::Bool;
            }
            5 => {
                let OwnedInstruction::AcquireLocalElementBorrow { loan, .. } =
                    &mut function.blocks[0].statements.last_mut().unwrap().kind
                else {
                    panic!("indexed fixture acquisition")
                };
                *loan = LoanId(1);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            table(&function).unwrap_err().name,
            "owned element loan plan relation",
            "invalid acquisition relation {mutation}"
        );
    }
}

#[test]
fn an_absent_table_does_not_allocate_but_still_has_a_work_request() {
    let (_, raw, _) = super::super::super::consumer_fixtures::empty_record();
    let function = &raw.functions[0];
    assert_eq!(requested_len(function), 0);
    let empty = fail_allocation_after(0, || table(function)).unwrap();
    assert_eq!((empty.len(), empty.capacity()), (0, 0));
    let functions = std::slice::from_ref(function);
    let work = additional_work(functions).unwrap();
    assert!(admit_work(functions, 0, work).is_ok());
    assert_eq!(
        admit_work(functions, 0, work - 1).unwrap_err().name,
        "owned element loan plan work"
    );
}

#[test]
fn complete_work_and_metadata_preflights_refuse_before_the_first_plan_allocation() {
    let (sources, raw, _) = super::super::super::consumer_fixtures::empty_record();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let work = add(
        witness.usage().work,
        additional_work(witness.functions()).unwrap(),
    )
    .unwrap();
    let plan = ExecutionPlan::build_with_test_budgets(&witness, MAX_PLAN_BYTES, work).unwrap();
    assert!(plan
        .functions()
        .iter()
        .all(|function| function.element_indices.is_empty()));
    let failure = fail_allocation_after(0, || {
        ExecutionPlan::build_with_test_budgets(&witness, MAX_PLAN_BYTES, work - 1)
    })
    .unwrap_err();
    assert_eq!(failure.name, "owned element loan plan work");
    let bytes = plan.metadata_bytes();
    assert!(ExecutionPlan::build_with_test_budgets(&witness, bytes, work).is_ok());
    let failure = fail_allocation_after(0, || {
        ExecutionPlan::build_with_test_budgets(&witness, bytes - 1, work)
    })
    .unwrap_err();
    assert_eq!(failure.name, "owned plan bytes");
}

#[test]
fn a_selected_sidecar_allocation_failure_and_actual_capacity_excess_refuse() {
    let (function, _) = raw_relation();
    let failure = fail_allocation_after(0, || table(&function)).unwrap_err();
    assert_eq!(failure.name, "injected owned allocation failure");
    let own = size_of::<Option<Operand>>();
    let mut bytes = own;
    assert!(owned_vector_accounting::admit_capacity::<Option<Operand>>(
        true,
        &mut bytes,
        2,
        1,
        own * 2,
    )
    .is_ok());
    assert_eq!(bytes, own * 2);
    let mut bytes = own;
    assert_eq!(
        owned_vector_accounting::admit_capacity::<Option<Operand>>(
            true,
            &mut bytes,
            2,
            1,
            own * 2 - 1,
        )
        .unwrap_err()
        .name,
        "owned plan bytes"
    );
}
