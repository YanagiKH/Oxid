//! Closed-stage raw authority checks, independent of the inert type query.
use super::*;

#[test]
fn owned_vector_type_new_raw_claims_cannot_mint_an_ordinary_seal() {
    enum Claim {
        Owner,
        Result,
        Reference,
        Loan,
    }
    for role in [Claim::Owner, Claim::Result, Claim::Reference, Claim::Loan] {
        let (sources, mut raw, _) = consumer_fixtures::shared_read();
        let declarations =
            Declarations::check_combined(&raw.records, &raw.enums, &sources).unwrap();
        assert!(shape::signatures(&raw, &declarations, &sources).is_ok());
        let aggregate = AggregateTy::OwnedRecordVector(RecordId(0));
        match role {
            Claim::Owner => {
                raw.functions[0].owners[0].aggregate =
                    AggregateSlot::try_from_aggregate(aggregate).unwrap()
            }
            Claim::Result => raw.functions[0].result = ValueTy::Owned(aggregate),
            Claim::Reference => {
                raw.functions[1].references[0].referent =
                    BorrowedSlot::check(BorrowedTy::Exact(aggregate)).unwrap()
            }
            Claim::Loan => {
                raw.functions[0].loans[0].referent =
                    BorrowedSlot::check(BorrowedTy::Exact(aggregate)).unwrap()
            }
        }
        let failure = shape::signatures(&raw, &declarations, &sources)
            .expect_err("closed owned-vector raw type accepted");
        assert!(matches!(
            failure.kind,
            OwnedFailureKind::Malformed(Malformed::Declaration(DeclarationError::TypeMismatch))
                | OwnedFailureKind::Malformed(Malformed::Type)
        ));
        assert!(verify_owned(raw, &sources).is_err());
    }
}

#[test]
fn owned_vector_type_old_vector_mapping_and_scalar_leaves_do_not_accept_new_identity() {
    let (sources, raw, _) = consumer_fixtures::shared_read();
    let declarations = Declarations::check_combined(&raw.records, &raw.enums, &sources).unwrap();
    let aggregate = AggregateTy::OwnedRecordVector(RecordId(0));
    assert_eq!(vector_type::VectorTy::from_aggregate(aggregate), None);
    assert_eq!(
        vector_type::VectorTy::from_borrowed(BorrowedTy::Exact(aggregate)),
        None
    );
    assert!(declarations.leaves(aggregate).is_err());
    assert!(declarations.contains_resources(aggregate).is_err());
    assert!(declarations.resource_leaves(aggregate, |_| Ok(0)).is_err());
    let owner = OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
        kind: OwnerKind::Local { mutable: false },
        span: raw.functions[0].span,
    };
    assert_eq!(owner.aggregate(), aggregate);
}
