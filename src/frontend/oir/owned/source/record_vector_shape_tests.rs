//! Malformed raw callers cannot turn shape coincidence into record Copy authority.
use super::super::*;

const SOURCE:&str="use alloc::vec::new;use alloc::vec::try_push;#[derive(Copy)]struct Holder<T:Copy>{value:T}#[derive(Copy)]struct P{valid:bool,x:i32}#[derive(Copy)]struct Q{valid:bool,x:i32}fn main()->i32{let p=P{valid:true,x:21};let mut v=new::<P>();try_push(&mut v,p);let q=v[0];v[0]=p;return q.x+p.x;}";

#[test]
fn record_vector_keys_use_concrete_record_rank_after_opaque_applications() {
    source::copy_record_tests::with_typed(SOURCE, |typed, map| {
        assert_eq!(typed.records().len(), 2); // the opaque Holder<T> has no layout.
        let raw = source::lower::lower(typed).unwrap();
        assert!(raw
            .vector_functions
            .iter()
            .all(|key| key.record_id() == Some(RecordId(0))));
        let witness = verified::verify_owned(raw, map).unwrap();
        assert_eq!(
            execute::run(&witness, typed.entry()).unwrap(),
            Scalar::I32(42)
        );
    });
}
#[test]
fn record_vector_raw_index_copy_and_store_recheck_exact_types_and_copy_authority() {
    source::copy_record_tests::with_typed(SOURCE, |typed, map| {
        let entry = typed.entry().unwrap();
        for mutation in 0..7 {
            let mut raw = source::lower::lower(typed).unwrap();
            match mutation {
                0 => raw.records[0].is_copy = false,
                1 => {
                    let f = &mut raw.functions[entry.0];
                    let destination = f
                        .blocks
                        .iter()
                        .flat_map(|block| &block.statements)
                        .find_map(|statement| match statement.kind {
                            OwnedInstruction::CopyIndexInitialize { destination, .. } => {
                                Some(destination)
                            }
                            _ => None,
                        })
                        .unwrap();
                    f.owners[destination.0].aggregate =
                        AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(1)))
                            .unwrap();
                }
                2 => {
                    let f = &mut raw.functions[entry.0];
                    let source = f
                        .blocks
                        .iter()
                        .flat_map(|block| &block.statements)
                        .find_map(|statement| match statement.kind {
                            OwnedInstruction::CopyIndexWrite { source, .. } => Some(source),
                            _ => None,
                        })
                        .unwrap();
                    f.owners[source.0].aggregate =
                        AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(1)))
                            .unwrap();
                }
                3 => {
                    let f = &mut raw.functions[entry.0];
                    let boolean = LocalId(
                        f.locals
                            .iter()
                            .position(|local| local.ty == hir::Ty::Bool)
                            .unwrap(),
                    );
                    let instruction = f
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.statements)
                        .find(|statement| {
                            matches!(statement.kind, OwnedInstruction::CopyIndexInitialize { .. })
                        })
                        .unwrap();
                    let OwnedInstruction::CopyIndexInitialize { index, .. } = &mut instruction.kind
                    else {
                        unreachable!()
                    };
                    index.local = boolean;
                }
                4 => {
                    let key = raw
                        .vector_functions
                        .iter()
                        .position(|key| {
                            key.operation()
                                == crate::frontend::builtin_catalog::vector::Operation::TryPush
                        })
                        .unwrap();
                    let function =
                        raw.fixed_builtin_base().unwrap() + raw.builtins.extra_functions() + key;
                    raw.functions[function].owners[0].aggregate =
                        AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(1)))
                            .unwrap();
                }
                5 => {
                    let f = &mut raw.functions[entry.0];
                    let statement = f
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.statements)
                        .find(|statement| {
                            matches!(statement.kind, OwnedInstruction::CopyIndexInitialize { .. })
                        })
                        .unwrap();
                    let OwnedInstruction::CopyIndexInitialize { path, .. } = &mut statement.kind
                    else {
                        unreachable!()
                    };
                    path.push(FieldId {
                        record: RecordId(1),
                        index: 0,
                    });
                }
                6 => {
                    let f = &mut raw.functions[entry.0];
                    let statement = f
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.statements)
                        .find(|statement| {
                            matches!(statement.kind, OwnedInstruction::CopyIndexInitialize { .. })
                        })
                        .unwrap();
                    let OwnedInstruction::CopyIndexInitialize { destination, .. } =
                        &mut statement.kind
                    else {
                        unreachable!()
                    };
                    *destination = OwnerPlaceId(usize::MAX);
                }
                _ => unreachable!(),
            }
            assert!(
                verified::verify_owned(raw, map).is_err(),
                "mutation={mutation}"
            );
        }
    });
}
