//! Declaration-only resource work cannot manufacture an executable witness.
use super::{consumer_fixtures as f, *};

fn scalar_function(id: usize, span: Span) -> RawOwnedFunction {
    let mut function = f::function(id, ValueTy::Scalar(hir::Ty::I32), span);
    function.locals.push(f::scalar(hir::Ty::I32, span));
    function.blocks.push(OwnedBlock {
        merge: None,
        span,
        statements: vec![f::assign(0, Rvalue::I32(0), span)],
        terminator: f::end(OwnedTerminatorKind::ReturnScalar(f::operand(0, span)), span),
    });
    function
}

#[test]
fn bytebuf_resource_authority_rejects_unused_declarations_and_signatures() {
    let (sources, at) = f::context();
    let span = at(0);
    for case in 0..5 {
        let mut raw = RawOwnedProgram {
            builtins: BuiltinOrigins::None,
            enums: vec![],
            records: vec![],
            functions: vec![scalar_function(0, span), scalar_function(1, span)],
        };
        match case {
            0 => {
                let mut record = f::record(&[hir::Ty::Unit], span);
                record.fields[0].ty = ParameterTy::Value(ValueTy::Owned(AggregateTy::ByteBuf));
                raw.records.push(record);
            }
            1 => raw.enums.push(RawEnumDecl {
                id: EnumId(0),
                span,
                variants: vec![RawVariantDecl {
                    id: VariantId {
                        enumeration: EnumId(0),
                        index: 0,
                    },
                    span,
                    payload: Some(ParameterTy::Value(ValueTy::Owned(AggregateTy::ByteBuf))),
                }],
            }),
            2 => raw.functions[1].result = ValueTy::Owned(AggregateTy::ByteBuf),
            3 => raw.functions[1].owners.push(OwnerDecl {
                aggregate: AggregateSlot::try_from_aggregate(AggregateTy::ByteBuf).unwrap(),
                kind: OwnerKind::Temporary,
                span,
            }),
            4 => {
                raw.functions[1]
                    .parameters
                    .push(ParameterBinding::Reference(ReferenceParamId(0)));
                raw.functions[1].references.push(ReferenceDecl {
                    referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::ByteBuf)).unwrap(),
                    kind: BorrowKind::Exclusive,
                    position: 0,
                    span,
                });
            }
            _ => unreachable!(),
        }
        assert!(
            verified::verify_owned(raw, &sources).is_err(),
            "case {case}"
        );
    }
}
