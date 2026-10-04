fn independent_alias_fixture(shared: bool) -> (SourceMap, RawOwnedProgram, Span) {
    let (sources, mut raw, _) = reference_fixture(hir::Ty::I32, 1, 0, array(hir::Ty::I32, 1), false);
    let base = raw.functions[0].span;
    let kind = if shared { BorrowKind::Shared } else { BorrowKind::Exclusive };
    let second = s(base, 90);
    let f = &mut raw.functions[0];
    f.loans[0].kind = kind;
    f.calls[0].arguments.push(ArgumentSlot::Borrow(LoanId(1)));
    f.loans.push(LoanDecl {
        call: CallSiteId(0), argument: 1, authority: AccessBase::Owner(OwnerPlaceId(0)),
        kind, aggregate: AggregateSlot::try_from_aggregate(array(hir::Ty::I32, 1)).unwrap(), span: second,
    });
    f.blocks[0].statements.push(ins(OwnedInstruction::PrepareBorrow {
        call: CallSiteId(0), argument: 1, loan: LoanId(1),
    }, second));
    let g = &mut raw.functions[1];
    g.references[0].kind = kind;
    g.references.push(ReferenceDecl {
        aggregate: AggregateSlot::try_from_aggregate(array(hir::Ty::I32, 1)).unwrap(),
        kind, position: 1, span: second,
    });
    g.parameters.push(ParameterBinding::Reference(ReferenceParamId(1)));
    (sources, raw, second)
}
fn independent_expect_raw_denial(raw: RawOwnedProgram, sources: &SourceMap, kind: OwnedFailureKind, span: Span) {
    let result = verified::probe_array_native(raw, sources, budget::Limits::DEFAULT,
        Some(hir::DefId(0)), sources, NativeControl { fail_after: Some(0), ..NativeControl::default() });
    let failure = match result { Err(failure) => failure, Ok(_) => panic!("invalid raw input reached native work") };
    assert_eq!(failure.kind, kind);
    assert_eq!(failure.primary.get(), Some(span));
}
#[test]
fn independent_unit2d_raw_authority_denials_precede_native_work() {
    // Zero length is still a full read in U and M states.
    for moved in [false, true] {
        let (sources, mut raw, _, _, _) = native_core_fixture(hir::Ty::I32, 0, 0, 0);
        let body = &mut raw.functions[0].blocks[0].statements;
        let length_at = body.last().unwrap().span;
        if moved {
            let at = Span { start: 210, end: 216, ..length_at };
            body.insert(4, fixtures::instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), at));
        } else {
            body.swap(3, 4); // Retain the canonical constructor site, but read while U.
        }
        independent_expect_raw_denial(raw, &sources, OwnedFailureKind::Ownership(Violation::Unavailable), length_at);
    }
    // A matching shared signature cannot authorize a write through that parameter.
    let (sources, mut raw, _) = effects_fixture(0);
    let write_at = raw.functions[1].blocks[0].statements[2].span;
    raw.functions[0].loans[0].kind = BorrowKind::Shared;
    raw.functions[1].references[0].kind = BorrowKind::Shared;
    independent_expect_raw_denial(raw, &sources, OwnedFailureKind::Ownership(Violation::Permission), write_at);
    // The second exclusive argument is acquired immediately, before invocation.
    let (sources, raw, second) = independent_alias_fixture(false);
    independent_expect_raw_denial(raw, &sources, OwnedFailureKind::Ownership(Violation::LoanConflict), second);
    // A pending exclusive argument also blocks N0 length at the caller.
    let (sources, mut raw, _) = reference_fixture(hir::Ty::I32, 0, 2, array(hir::Ty::I32, 1), false);
    let at = s(raw.functions[0].span, 91);
    raw.functions[0].blocks[0].statements.push(ins(OwnedInstruction::ArrayLength {
        destination: LocalId(3), base: AccessBase::Owner(OwnerPlaceId(0)),
    }, at));
    independent_expect_raw_denial(raw, &sources, OwnedFailureKind::Ownership(Violation::LoanConflict), at);
    // Missing/excess construction elements and bad scalar IDs are raw-shape failures.
    let (sources, mut raw, _, _, _) = native_core_fixture(hir::Ty::I32, 1, 0, 1);
    let statement = raw.functions[0].blocks[0].statements.iter_mut().find(|s| matches!(s.kind, OwnedInstruction::ConstructArray { .. })).unwrap();
    let at = statement.span;
    let OwnedInstruction::ConstructArray { elements, .. } = &mut statement.kind else { unreachable!() };
    elements.push(elements[0]);
    independent_expect_raw_denial(raw, &sources, OwnedFailureKind::Malformed(Malformed::Type), at);
    let (sources, mut raw, _, _, _) = native_core_fixture(hir::Ty::I32, 1, 0, 1);
    let statement = raw.functions[0].blocks[0].statements.iter_mut().find(|s| matches!(s.kind, OwnedInstruction::ReadIndex { .. })).unwrap();
    let OwnedInstruction::ReadIndex { index, .. } = &mut statement.kind else { unreachable!() };
    index.local = LocalId(999);
    let at = index.span;
    independent_expect_raw_denial(raw, &sources, OwnedFailureKind::Malformed(Malformed::Id), at);
    // Equal shared roots remain legal; this is a separate positive alias control.
    let (sources, raw, _) = independent_alias_fixture(true);
    assert!(verified::probe_array_native(raw, &sources, budget::Limits::DEFAULT,
        Some(hir::DefId(0)), &sources, NativeControl::default()).unwrap().result.is_ok());
    eprintln!("independent raw/native boundary: 7 unavailable/permission/alias/malformed denials and 1 legal shared-alias emission");
}
#[test]
#[ignore = "independent Unit2D shared-alias source-free ELF gate"]
fn independent_unit2d_shared_aliases_llvm() {
    let (sources, raw, _) = independent_alias_fixture(true);
    let module = independent_native(raw, &sources, 1_000_000);
    let scratch = Scratch::new();
    let binary = scratch.compile(&module, "shared-array-aliases");
    independent_assert_output(independent_run(&scratch, &binary, "shared-array-aliases", &[]), Some(Scalar::I32(77)), None);
}
