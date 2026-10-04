#[test]
#[ignore = "independent Unit2D extreme payload representation LLVM gate"]
fn independent_unit2d_payload_extremes_llvm() {
    let scratch = Scratch::new();
    for (label, value) in [("min", i32::MIN), ("max", i32::MAX)] {
        let (sources, mut raw, _, _, _) = native_core_fixture(hir::Ty::I32, 1, 0, 1);
        let OwnedInstruction::Scalar(Statement::Assign(assign)) = &mut raw.functions[0].blocks[0].statements[0].kind else { unreachable!() };
        assign.value = Rvalue::I32(value);
        let module = independent_native(raw, &sources, 1_000_000);
        let name = format!("payload-extreme-construct-{label}");
        let binary = scratch.compile(&module, &name);
        independent_assert_output(independent_run(&scratch, &binary, &name, &[]), Some(Scalar::I32(value)), None);

        let (sources, mut raw, _) = chain_fixture(hir::Ty::I32, 4);
        let base = raw.functions[0].span;
        let f = &mut raw.functions[0];
        let OwnedInstruction::Scalar(Statement::Assign(assign)) = &mut f.blocks[0].statements[3].kind else { unreachable!() };
        assert_eq!(assign.destination, LocalId(3));
        assign.value = Rvalue::I32(value);
        f.locals.extend([local(hir::Ty::I32, base), local(hir::Ty::I32, base)]);
        f.blocks[1].statements.extend([
            literal(5, Scalar::I32(3), s(base, 82)),
            ins(OwnedInstruction::ReadIndex {
                destination: LocalId(6), base: AccessBase::Owner(OwnerPlaceId(10)), index: op(5, s(base, 83)),
            }, s(base, 84)),
        ]);
        f.blocks[1].terminator.as_mut().unwrap().kind = OwnedTerminatorKind::ReturnScalar(op(6, s(base, 85)));
        let module = independent_native(raw, &sources, 1_000_000);
        let name = format!("payload-extreme-transfer-{label}");
        let binary = scratch.compile(&module, &name);
        independent_assert_output(independent_run(&scratch, &binary, &name, &[]), Some(Scalar::I32(value)), None);
    }
    eprintln!("independent extreme payloads: MIN/MAX through constructor-read and complete transfer/call chain, four source-free ELF cases");
}
