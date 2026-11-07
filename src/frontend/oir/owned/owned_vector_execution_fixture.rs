//! One independently specified caller shared by the real VM/native controls.
//! Constructors produce inert raw rows; only the original verifier seals them.
use super::*;
use crate::frontend::oir::owned::verified::VerifiedOwnedVectorProgram;

#[path = "owned_vector_resource_fixture.rs"]
pub(in crate::frontend::oir::owned) mod resources;

#[path = "owned_vector_file_fixture.rs"]
pub(in crate::frontend::oir::owned) mod files;

#[derive(Clone, Copy)]
enum Input {
    Borrow(OwnerPlaceId),
    Scalar(Operand),
    Owned(OwnerPlaceId),
}

#[derive(Clone, Copy)]
enum Schedule {
    TruncateThenTake,
    TakeThenTruncate,
}

struct Caller {
    function: RawOwnedFunction,
    pending: Vec<OwnedStatement>,
    span: Span,
    keys: [Function; 8],
    owned_start: usize,
    grow: EnumId,
}

impl Caller {
    fn new(span: Span, keys: [Function; 8]) -> Self {
        Self {
            function: consumer_fixtures::function(0, ValueTy::Scalar(hir::Ty::I32), span),
            pending: Vec::new(),
            span,
            keys,
            owned_start: 2,
            grow: EnumId(0),
        }
    }

    fn statement(&mut self, kind: OwnedInstruction) {
        self.pending
            .push(consumer_fixtures::instruction(kind, self.span));
    }

    fn scalar_slot(&mut self, ty: hir::Ty) -> LocalId {
        let local = LocalId(self.function.locals.len());
        self.function
            .locals
            .push(consumer_fixtures::scalar(ty, self.span));
        local
    }

    fn constant(&mut self, value: i32) -> Operand {
        let local = self.scalar_slot(hir::Ty::I32);
        self.pending.push(consumer_fixtures::assign(
            local.0,
            Rvalue::I32(value),
            self.span,
        ));
        Operand {
            local,
            span: self.span,
        }
    }

    fn owner(&mut self, aggregate: AggregateTy, kind: OwnerKind) -> OwnerPlaceId {
        let owner = OwnerPlaceId(self.function.owners.len());
        self.function.owners.push(OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
            kind,
            span: self.span,
        });
        if !matches!(
            kind,
            OwnerKind::CallResult { .. } | OwnerKind::StagedArgument { .. }
        ) {
            self.statement(OwnedInstruction::StorageLive(owner));
        }
        owner
    }

    fn byte_constant(&mut self, value: u8) -> Operand {
        let local = self.scalar_slot(hir::Ty::U8);
        self.pending.push(consumer_fixtures::assign(
            local.0,
            Rvalue::U8(value),
            self.span,
        ));
        Operand {
            local,
            span: self.span,
        }
    }

    fn record(&mut self, value: Operand) -> OwnerPlaceId {
        let record = RecordId(0);
        let owner = self.owner(AggregateTy::Record(record), OwnerKind::Temporary);
        self.statement(OwnedInstruction::Construct {
            destination: owner,
            fields: vec![(FieldId { record, index: 0 }, value)],
        });
        owner
    }

    fn call(&mut self, operation: Operation, inputs: &[Input]) -> CallResult {
        let rank = self
            .keys
            .iter()
            .position(|key| key.operation() == operation)
            .unwrap();
        let signature = self.keys[rank].concrete_signature(self.grow);
        self.call_signature(
            hir::DefId(rank + self.owned_start),
            signature.parameters(),
            signature.result,
            inputs,
        )
    }

    fn call_signature(
        &mut self,
        target: hir::DefId,
        parameters: &[ParameterTy],
        result_type: ValueTy,
        inputs: &[Input],
    ) -> CallResult {
        assert_eq!(parameters.len(), inputs.len());
        let call = CallSiteId(self.function.calls.len());
        let result = match result_type {
            ValueTy::Scalar(ty) => CallResult::Scalar(self.scalar_slot(ty)),
            ValueTy::Owned(aggregate) => {
                CallResult::Owned(self.owner(aggregate, OwnerKind::CallResult { call }))
            }
        };
        self.statement(OwnedInstruction::OpenCall(call));
        let mut arguments = Vec::new();
        for (argument, (input, expected)) in inputs.iter().zip(parameters).enumerate() {
            arguments.push(match (*input, *expected) {
                (Input::Borrow(owner), ParameterTy::Reference { kind, referent }) => {
                    let loan = LoanId(self.function.loans.len());
                    self.function.loans.push(LoanDecl {
                        projection: Vec::new(),
                        region: LoanRegion::Call { call, argument },
                        authority: AccessBase::Owner(owner),
                        kind,
                        referent: BorrowedSlot::check(referent).unwrap(),
                        span: self.span,
                    });
                    self.statement(OwnedInstruction::PrepareBorrow {
                        call,
                        argument,
                        loan,
                    });
                    ArgumentSlot::Borrow(loan)
                }
                (Input::Scalar(value), ParameterTy::Value(ValueTy::Scalar(_))) => {
                    self.statement(OwnedInstruction::PrepareScalar {
                        call,
                        argument,
                        value,
                    });
                    ArgumentSlot::Scalar
                }
                (Input::Owned(source), ParameterTy::Value(ValueTy::Owned(aggregate))) => {
                    let staged =
                        self.owner(aggregate, OwnerKind::StagedArgument { call, argument });
                    self.statement(OwnedInstruction::PrepareOwned {
                        call,
                        argument,
                        source,
                    });
                    ArgumentSlot::Owned(staged)
                }
                _ => panic!("incorrect contract-shaped test argument"),
            });
        }
        self.function.calls.push(CallDecl {
            target,
            arguments,
            result,
            parent: None,
            span: self.span,
        });
        let continuation = BlockId(self.function.blocks.len() + 1);
        self.finish_block(OwnedTerminatorKind::Invoke { call, continuation });
        result
    }

    fn finish_block(&mut self, terminator: OwnedTerminatorKind) {
        self.function.blocks.push(OwnedBlock {
            merge: None,
            span: self.span,
            statements: std::mem::take(&mut self.pending),
            terminator: consumer_fixtures::end(terminator, self.span),
        });
    }

    fn sum(&mut self, values: &[Operand]) -> Operand {
        let mut result = self.constant(0);
        for value in values {
            let local = self.scalar_slot(hir::Ty::I32);
            self.pending.push(consumer_fixtures::assign(
                local.0,
                Rvalue::CheckedI32 {
                    op: hir::ArithmeticOp::Add,
                    left: result,
                    right: *value,
                    operator_span: self.span,
                },
                self.span,
            ));
            result = Operand {
                local,
                span: self.span,
            };
        }
        result
    }

    fn scalar_result(&mut self, operation: Operation, inputs: &[Input]) -> Operand {
        match self.call(operation, inputs) {
            CallResult::Scalar(local) => Operand {
                local,
                span: self.span,
            },
            CallResult::Owned(_) => panic!("scalar test operation returned an owner"),
        }
    }

    fn owned_result(&mut self, operation: Operation, inputs: &[Input]) -> OwnerPlaceId {
        match self.call(operation, inputs) {
            CallResult::Owned(owner) => owner,
            CallResult::Scalar(_) => panic!("owned test operation returned a scalar"),
        }
    }
}

pub(in crate::frontend::oir::owned) fn all_operations_raw() -> (
    SourceMap,
    RawOwnedProgram,
    [Function; 8],
    OwnedVectorImportClaims,
    i32,
) {
    let (sources, mut raw, keys, imports) = fixture(None);
    let span = raw.records[0].span;
    let main = all_operations_main(
        Caller::new(span, keys),
        |caller, value, _| caller.record(value),
        Schedule::TruncateThenTake,
    );
    for function in &mut raw.functions {
        function.id.0 += 1;
    }
    raw.functions.insert(0, main);
    (sources, raw, keys, imports, 58)
}

fn all_operations_main(
    mut main: Caller,
    mut record: impl FnMut(&mut Caller, Operand, usize) -> OwnerPlaceId,
    schedule: Schedule,
) -> RawOwnedFunction {
    let span = main.span;
    let one = main.constant(1);
    let zero = main.constant(0);
    let first = main.constant(41);
    let second = main.constant(42);
    let created = main.owned_result(Operation::New, &[]);
    let vector = main.owner(
        AggregateTy::OwnedRecordVector(RecordId(0)),
        OwnerKind::Local { mutable: true },
    );
    main.statement(OwnedInstruction::MoveInitialize {
        destination: vector,
        source: created,
    });
    let status = main.owned_result(
        Operation::TryReserve,
        &[Input::Borrow(vector), Input::Scalar(one)],
    );
    main.statement(OwnedInstruction::Discard(status));
    let capacity = main.scalar_result(Operation::Capacity, &[Input::Borrow(vector)]);
    let initial_len = main.scalar_result(Operation::Len, &[Input::Borrow(vector)]);
    let first = record(&mut main, first, 0);
    main.scalar_result(
        Operation::PushReserved,
        &[Input::Borrow(vector), Input::Owned(first)],
    );
    let length = main.scalar_result(Operation::Len, &[Input::Borrow(vector)]);
    let popped = main.owned_result(Operation::PopLast, &[Input::Borrow(vector)]);
    let value = main.scalar_slot(hir::Ty::I32);
    main.statement(OwnedInstruction::ReadField {
        destination: value,
        base: AccessBase::Owner(popped),
        field: FieldId {
            record: RecordId(0),
            index: 0,
        },
    });
    main.statement(OwnedInstruction::Discard(popped));
    let second = record(&mut main, second, 1);
    main.scalar_result(
        Operation::PushReserved,
        &[Input::Borrow(vector), Input::Owned(second)],
    );
    let truncated_before_take = match schedule {
        Schedule::TruncateThenTake => {
            main.scalar_result(
                Operation::Truncate,
                &[Input::Borrow(vector), Input::Scalar(zero)],
            );
            Some(main.scalar_result(Operation::Len, &[Input::Borrow(vector)]))
        }
        Schedule::TakeThenTruncate => None,
    };
    let taken_result = main.owned_result(Operation::Take, &[Input::Borrow(vector)]);
    let taken = main.owner(
        AggregateTy::OwnedRecordVector(RecordId(0)),
        OwnerKind::Local {
            mutable: matches!(schedule, Schedule::TakeThenTruncate),
        },
    );
    main.statement(OwnedInstruction::MoveInitialize {
        destination: taken,
        source: taken_result,
    });
    let (length, truncated) = if let Some(truncated) = truncated_before_take {
        (length, truncated)
    } else {
        let length = main.scalar_result(Operation::Len, &[Input::Borrow(taken)]);
        main.scalar_result(
            Operation::Truncate,
            &[Input::Borrow(taken), Input::Scalar(zero)],
        );
        let truncated = main.scalar_result(Operation::Len, &[Input::Borrow(taken)]);
        (length, truncated)
    };
    let cleared = main.scalar_result(Operation::Capacity, &[Input::Borrow(vector)]);
    let taken_capacity = main.scalar_result(Operation::Capacity, &[Input::Borrow(taken)]);
    main.statement(OwnedInstruction::Discard(taken));
    main.statement(OwnedInstruction::StorageEnd(vector));
    let result = main.sum(&[
        Operand { local: value, span },
        capacity,
        initial_len,
        length,
        truncated,
        cleared,
        taken_capacity,
    ]);
    main.finish_block(OwnedTerminatorKind::ReturnScalar(result));
    //41 read from the moved record +8 capacity +1 length +8 taken capacity.
    main.function
}

pub(in crate::frontend::oir::owned) fn all_operations(
) -> (SourceMap, VerifiedOwnedVectorProgram, [Function; 8], i32) {
    let (sources, raw, keys, imports, expected) = all_operations_raw();
    let wrapper = prepare(&sources, raw, &keys, imports);
    let witness = VerifiedOwnedVectorProgram::verify(
        wrapper,
        &sources,
        budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap();
    (sources, witness, keys, expected)
}

#[test]
fn owned_vector_witness_all_operations_caller_has_real_full_ownership_and_storage_proof() {
    let (_, witness, keys, expected) = all_operations();
    assert_eq!(expected, 58);
    assert_eq!(witness.functions()[0].parameters.len(), 0);
    for key in keys {
        let target = witness
            .functions()
            .iter()
            .find(|function| witness.owned_vector_operation(function.id) == Some(key))
            .unwrap()
            .id;
        assert!(witness.functions()[0]
            .calls
            .iter()
            .any(|call| call.target == target));
    }
    let plan = plan::ExecutionPlan::build(&witness).unwrap();
    assert!(plan::native_storage::NativeStoragePlan::checked(&plan, true).is_ok());
}
