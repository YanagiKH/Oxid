use super::consumer_fixtures as f;
use super::*;
use std::mem::size_of;

fn enumeration(span: Span) -> RawEnumDecl {
    RawEnumDecl {
        id: EnumId(0),
        span,
        variants: vec![RawVariantDecl {
            id: VariantId {
                enumeration: EnumId(0),
                index: 0,
            },
            payload: None,
            span,
        }],
    }
}
fn raw(span: Span) -> RawOwnedProgram {
    let mut function = f::function(0, ValueTy::Scalar(hir::Ty::Unit), span);
    function.locals.push(f::scalar(hir::Ty::Unit, span));
    function.blocks.push(OwnedBlock {
        merge: None,
        span,
        statements: vec![f::assign(0, Rvalue::Unit, span)],
        terminator: f::end(OwnedTerminatorKind::ReturnScalar(f::operand(0, span)), span),
    });
    RawOwnedProgram {
        enums: vec![],
        records: vec![],
        functions: vec![function],
    }
}

#[test]
fn bounded_enum_raw_declarations_cannot_acquire_any_executable_witness() {
    let (sources, span) = f::context();
    let span = span(0);
    let make = || {
        let mut p = raw(span);
        p.enums.push(enumeration(span));
        p
    };
    assert!(verify_owned(raw(span), &sources).is_ok());
    assert_eq!(
        verify_owned(make(), &sources).unwrap_err().kind,
        OwnedFailureKind::Malformed(Malformed::Type)
    );
    assert!(verified::probe_array_validation(&make(), &sources, budget::Limits::DEFAULT).is_err());
    assert!(verified::probe_array_reference(
        make(),
        &sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        execute::Limits::default(),
        execute::ObservationControl::default()
    )
    .is_err());
    assert!(verified::probe_array_native(
        make(),
        &sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        &sources,
        native::NativeControl::default()
    )
    .is_err());
}

#[test]
fn bounded_enum_raw_undeclared_equal_owner_and_result_ids_reject() {
    let (sources, span) = f::context();
    let span = span(0);
    for id in [0, 4096, u32::MAX as usize] {
        for result in [false, true] {
            let mut p = raw(span);
            let ty = AggregateTy::Enum(EnumId(id));
            p.functions[0].owners.push(OwnerDecl {
                aggregate: AggregateSlot::try_from_aggregate(ty).unwrap(),
                kind: OwnerKind::Local { mutable: true },
                span,
            });
            if result {
                p.functions[0].result = ValueTy::Owned(ty);
            }
            assert_eq!(
                verify_owned(p, &sources).unwrap_err().kind,
                OwnedFailureKind::Malformed(Malformed::Declaration(
                    DeclarationError::InvalidEnumId(EnumId(id))
                ))
            );
        }
    }
}

#[test]
fn bounded_enum_raw_inventory_charges_rows_members_and_empty_header() {
    let (_, span) = f::context();
    let span = span(0);
    let mut p = raw(span);
    let empty = budget::preflight(&p, budget::Limits::DEFAULT).unwrap();
    assert_eq!(
        empty.metadata_bytes,
        (size_of::<Vec<RawEnumDecl>>() + size_of::<Vec<MatchDecl>>())
    );
    p.enums.push(enumeration(span));
    let usage = budget::fail_allocation_after(0, || budget::preflight(&p, budget::Limits::DEFAULT))
        .unwrap();
    assert_eq!(
        usage.metadata_bytes,
        (size_of::<Vec<RawEnumDecl>>() + size_of::<Vec<MatchDecl>>())
            + size_of::<RawEnumDecl>()
            + size_of::<RawVariantDecl>()
    );
    assert_eq!(usage.expanded_events, 2);
    assert_eq!(usage.work, 8);
    for selector in 0..3 {
        let mut limits = budget::Limits::DEFAULT;
        match selector {
            0 => limits.metadata = usage.metadata_bytes,
            1 => limits.events = usage.expanded_events,
            _ => limits.work = usage.work,
        }
        assert!(budget::preflight(&p, limits).is_ok());
        match selector {
            0 => limits.metadata -= 1,
            1 => limits.events -= 1,
            _ => limits.work -= 1,
        }
        assert!(budget::preflight(&p, limits).is_err());
    }
    assert!(budget::preflight(
        &raw(span),
        budget::Limits {
            metadata: (size_of::<Vec<RawEnumDecl>>() + size_of::<Vec<MatchDecl>>()) - 1,
            ..budget::Limits::DEFAULT
        }
    )
    .is_err());
    #[cfg(target_pointer_width = "64")]
    assert_eq!(size_of::<RawOwnedProgram>(), 72);
}
