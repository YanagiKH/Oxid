//! Independent B2a raw proof fixtures. No source lowerer, executable witness,
//! consumer plan, or production canonical-match builder supplies these graphs.
use super::consumer_fixtures as f;
use super::*;
use std::mem::size_of;

fn variant(index: usize) -> VariantId {
    VariantId {
        enumeration: EnumId(0),
        index,
    }
}
fn enumeration(payloads: &[Option<hir::Ty>], span: Span) -> RawEnumDecl {
    RawEnumDecl {
        id: EnumId(0),
        span,
        variants: payloads
            .iter()
            .enumerate()
            .map(|(index, &payload)| RawVariantDecl {
                id: variant(index),
                payload: payload.map(|ty| ParameterTy::Value(ValueTy::Scalar(ty))),
                span,
            })
            .collect(),
    }
}
fn enum_owner(kind: OwnerKind, span: Span) -> OwnerDecl {
    OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Enum(EnumId(0))).unwrap(),
        kind,
        span,
    }
}
fn block(statements: Vec<OwnedStatement>, kind: OwnedTerminatorKind, span: Span) -> OwnedBlock {
    OwnedBlock {
        merge: None,
        span,
        statements,
        terminator: f::end(kind, span),
    }
}
fn literal(ty: hir::Ty) -> Rvalue {
    match ty {
        hir::Ty::Bool => Rvalue::Bool(true),
        hir::Ty::I32 => Rvalue::I32(-71),
        hir::Ty::Unit => Rvalue::Unit,
    }
}
fn local(function: &mut RawOwnedFunction, ty: hir::Ty, kind: LocalKind, span: Span) -> LocalId {
    let id = LocalId(function.locals.len());
    function.locals.push(LocalDecl { ty, kind, span });
    id
}
/// Dispatches occupy 0..N; entries occupy N..2N. Variant order is caller supplied,
/// independently of declaration order. All entries return the dominating unit.
fn fixture(
    payloads: &[Option<hir::Ty>],
    order: &[usize],
    constructed: usize,
    span: Span,
) -> RawOwnedProgram {
    assert_eq!(payloads.len(), order.len());
    let n = order.len();
    let mut function = f::function(0, ValueTy::Scalar(hir::Ty::Unit), span);
    function.locals.push(f::scalar(hir::Ty::Unit, span));
    function
        .owners
        .push(enum_owner(OwnerKind::Local { mutable: false }, span));
    let mut preceding = vec![f::assign(0, Rvalue::Unit, span)];
    let payload = payloads[constructed].map(|ty| {
        let id = local(&mut function, ty, LocalKind::Temporary, span);
        preceding.push(f::assign(id.0, literal(ty), span));
        f::operand(id.0, span)
    });
    preceding.push(f::instruction(
        OwnedInstruction::StorageLive(OwnerPlaceId(0)),
        span,
    ));
    preceding.push(f::instruction(
        OwnedInstruction::ConstructEnum {
            destination: OwnerPlaceId(0),
            variant: variant(constructed),
            payload,
        },
        span,
    ));
    function.matches.push(MatchDecl {
        source: OwnerPlaceId(0),
        arms: order
            .iter()
            .enumerate()
            .map(|(arm, &index)| MatchArm {
                variant: variant(index),
                dispatch: BlockId(arm),
                entry: BlockId(n + arm),
            })
            .collect(),
        span,
    });
    for arm in 0..n {
        function.blocks.push(block(
            if arm == 0 {
                std::mem::take(&mut preceding)
            } else {
                vec![]
            },
            OwnedTerminatorKind::MatchDispatch {
                match_id: MatchId(0),
                arm,
            },
            span,
        ));
    }
    for (arm, &index) in order.iter().enumerate() {
        let destination =
            payloads[index].map(|ty| local(&mut function, ty, LocalKind::Binding, span));
        let mut statements = vec![f::instruction(
            OwnedInstruction::ConsumeVariant {
                match_id: MatchId(0),
                arm,
                destination,
            },
            span,
        )];
        if let Some(destination) = destination {
            let copy = local(
                &mut function,
                payloads[index].unwrap(),
                LocalKind::Temporary,
                span,
            );
            statements.push(f::assign(
                copy.0,
                Rvalue::Copy(f::operand(destination.0, span)),
                span,
            ));
        }
        statements.push(f::instruction(
            OwnedInstruction::StorageEnd(OwnerPlaceId(0)),
            span,
        ));
        function.blocks.push(block(
            statements,
            OwnedTerminatorKind::ReturnScalar(f::operand(0, span)),
            span,
        ));
    }
    RawOwnedProgram {
        builtins: BuiltinOrigins::None,
        enums: vec![enumeration(payloads, span)],
        records: vec![],
        functions: vec![function],
    }
}
fn pair(span: Span) -> RawOwnedProgram {
    fixture(&[None, Some(hir::Ty::I32)], &[0, 1], 0, span)
}
fn nullary_pair(span: Span) -> RawOwnedProgram {
    fixture(&[None, None], &[0, 1], 0, span)
}
fn parameter_pair(span: Span) -> RawOwnedProgram {
    let mut raw = nullary_pair(span);
    let function = &mut raw.functions[0];
    function.owners[0].kind = OwnerKind::Parameter { position: 0 };
    function.locals[0].kind = LocalKind::Parameter;
    function.parameters = vec![
        ParameterBinding::Owned(OwnerPlaceId(0)),
        ParameterBinding::Scalar(LocalId(0)),
    ];
    function.blocks[0].statements.clear();
    raw
}
fn probe(raw: &RawOwnedProgram, sources: &SourceMap) -> Result<OwnershipUsage, OwnedFailure> {
    verified::probe_enum_validation(raw, sources, budget::Limits::DEFAULT)
}
fn reject(raw: &RawOwnedProgram, sources: &SourceMap, label: &str) -> OwnedFailure {
    match probe(raw, sources) {
        Err(error) => error,
        Ok(_) => panic!("accepted malformed raw case: {label}"),
    }
}
fn consume_mut(raw: &mut RawOwnedProgram, arm: usize) -> &mut OwnedInstruction {
    let entry = raw.functions[0].matches[0].arms[arm].entry.0;
    &mut raw.functions[0].blocks[entry].statements[0].kind
}
fn binding(raw: &RawOwnedProgram, arm: usize) -> LocalId {
    let entry = raw.functions[0].matches[0].arms[arm].entry.0;
    match raw.functions[0].blocks[entry].statements[0].kind {
        OwnedInstruction::ConsumeVariant {
            destination: Some(id),
            ..
        } => id,
        _ => panic!("fixture arm has no binding"),
    }
}

#[test]
fn enum_raw_single_variants_and_every_mixed_written_order_prove() {
    let (sources, s) = f::context();
    for payload in [
        None,
        Some(hir::Ty::Bool),
        Some(hir::Ty::I32),
        Some(hir::Ty::Unit),
    ] {
        let raw = fixture(&[payload], &[0], 0, s(0));
        let usage = probe(&raw, &sources).unwrap();
        assert_eq!((usage.owner_cells, usage.owner_layout_bytes), (2, 8));
    }
    let payloads = [
        None,
        Some(hir::Ty::Bool),
        Some(hir::Ty::I32),
        Some(hir::Ty::Unit),
    ];
    let mut cases = 0;
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let order = [a, b, c, d];
                    if order
                        .iter()
                        .enumerate()
                        .any(|(i, v)| order[..i].contains(v))
                    {
                        continue;
                    }
                    for constructed in 0..4 {
                        let raw = fixture(&payloads, &order, constructed, s(0));
                        assert!(
                            probe(&raw, &sources).is_ok(),
                            "order {order:?}, constructed {constructed}"
                        );
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 96);
    assert!(probe(&parameter_pair(s(0)), &sources).is_ok());
}

#[test]
fn enum_raw_256_arms_prove_and_257_are_rejected_before_reserve() {
    let (sources, s) = f::context();
    let payloads = vec![None; 256];
    let order: Vec<_> = (0..256).rev().collect();
    let raw = fixture(&payloads, &order, 255, s(0));
    assert!(probe(&raw, &sources).is_ok());
    let mut excessive = nullary_pair(s(0));
    let arm = excessive.functions[0].matches[0].arms[0];
    excessive.functions[0].matches[0].arms = vec![arm; 257];
    let error = budget::fail_allocation_after(0, || probe(&excessive, &sources)).unwrap_err();
    assert!(matches!(error.kind, OwnedFailureKind::Resource(_)));
    assert_ne!(
        error.kind,
        OwnedFailureKind::Resource("injected allocation failure")
    );
    let excessive = fixture(&[None; 257], &(0..257).collect::<Vec<_>>(), 0, s(0));
    assert!(probe(&excessive, &sources).is_err());
}

#[test]
fn enum_raw_ids_are_checked_before_reachability_or_indexing() {
    let (sources, s) = f::context();
    let labels = [
        "source owner",
        "variant enum",
        "variant ordinal",
        "dispatch block",
        "entry block",
        "dispatch match",
        "dispatch arm",
        "consume match",
        "consume arm",
        "payload destination",
        "constructor owner",
        "constructor variant enum",
        "constructor variant ordinal",
        "constructor operand",
    ];
    for (case, label) in labels.into_iter().enumerate() {
        let mut raw = fixture(&[None, Some(hir::Ty::I32)], &[0, 1], 1, s(0));
        match case {
            0 => raw.functions[0].matches[0].source = OwnerPlaceId(usize::MAX),
            1 => raw.functions[0].matches[0].arms[0].variant.enumeration = EnumId(usize::MAX),
            2 => raw.functions[0].matches[0].arms[0].variant.index = usize::MAX,
            3 => raw.functions[0].matches[0].arms[0].dispatch = BlockId(usize::MAX),
            4 => raw.functions[0].matches[0].arms[0].entry = BlockId(usize::MAX),
            5 => {
                raw.functions[0].blocks[0].terminator.as_mut().unwrap().kind =
                    OwnedTerminatorKind::MatchDispatch {
                        match_id: MatchId(usize::MAX),
                        arm: 0,
                    }
            }
            6 => {
                raw.functions[0].blocks[0].terminator.as_mut().unwrap().kind =
                    OwnedTerminatorKind::MatchDispatch {
                        match_id: MatchId(0),
                        arm: usize::MAX,
                    }
            }
            7 => {
                if let OwnedInstruction::ConsumeVariant { match_id, .. } = consume_mut(&mut raw, 1)
                {
                    *match_id = MatchId(usize::MAX);
                }
            }
            8 => {
                if let OwnedInstruction::ConsumeVariant { arm, .. } = consume_mut(&mut raw, 1) {
                    *arm = usize::MAX;
                }
            }
            9 => {
                if let OwnedInstruction::ConsumeVariant { destination, .. } =
                    consume_mut(&mut raw, 1)
                {
                    *destination = Some(LocalId(usize::MAX));
                }
            }
            _ => {
                let instruction = raw.functions[0].blocks[0].statements.last_mut().unwrap();
                if let OwnedInstruction::ConstructEnum {
                    destination,
                    variant,
                    payload,
                } = &mut instruction.kind
                {
                    match case {
                        10 => *destination = OwnerPlaceId(usize::MAX),
                        11 => variant.enumeration = EnumId(usize::MAX),
                        12 => variant.index = usize::MAX,
                        13 => payload.as_mut().unwrap().local = LocalId(usize::MAX),
                        _ => unreachable!(),
                    }
                }
            }
        }
        reject(&raw, &sources, label);
    }
}

#[test]
fn enum_raw_coverage_and_canonical_sites_are_not_producer_authority() {
    let (sources, s) = f::context();
    let labels = [
        "empty arms",
        "missing variant",
        "duplicate variant",
        "foreign same-ordinal variant",
        "duplicate descriptor",
        "duplicate dispatch role",
        "duplicate entry role",
        "dispatch-entry overlap",
        "missing consume",
        "duplicate consume",
        "consume follows statement",
        "consume references another arm",
        "missing first dispatch",
        "final dispatch replaced by goto",
        "wrong chain order",
        "foreign consume site",
        "unused descriptor",
        "missing all descriptors",
    ];
    for (case, label) in labels.into_iter().enumerate() {
        let mut raw = pair(s(0));
        match case {
            0 => raw.functions[0].matches[0].arms.clear(),
            1 => {
                raw.functions[0].matches[0].arms.pop();
            }
            2 => raw.functions[0].matches[0].arms[1].variant = variant(0),
            3 => {
                let mut foreign = enumeration(&[None, Some(hir::Ty::I32)], s(0));
                foreign.id = EnumId(1);
                for member in &mut foreign.variants {
                    member.id.enumeration = EnumId(1);
                }
                raw.enums.push(foreign);
                raw.functions[0].matches[0].arms[0].variant.enumeration = EnumId(1);
            }
            4 => {
                let duplicate = raw.functions[0].matches[0].clone();
                raw.functions[0].matches.push(duplicate);
            }
            16 => {
                raw.functions[0].matches.push(MatchDecl {
                    source: OwnerPlaceId(0),
                    span: s(0),
                    arms: vec![
                        MatchArm {
                            variant: variant(0),
                            dispatch: BlockId(4),
                            entry: BlockId(6),
                        },
                        MatchArm {
                            variant: variant(1),
                            dispatch: BlockId(5),
                            entry: BlockId(7),
                        },
                    ],
                });
                for _ in 0..4 {
                    raw.functions[0].blocks.push(block(
                        vec![],
                        OwnedTerminatorKind::ReturnScalar(f::operand(0, s(0))),
                        s(0),
                    ));
                }
            }
            5 => raw.functions[0].matches[0].arms[1].dispatch = BlockId(0),
            6 => raw.functions[0].matches[0].arms[1].entry = BlockId(2),
            7 => raw.functions[0].matches[0].arms[1].entry = BlockId(0),
            8 => {
                raw.functions[0].blocks[3].statements.remove(0);
            }
            9 => {
                let duplicate = raw.functions[0].blocks[3].statements[0].clone();
                raw.functions[0].blocks[3].statements.insert(1, duplicate);
            }
            10 => {
                let id = local(
                    &mut raw.functions[0],
                    hir::Ty::Unit,
                    LocalKind::Temporary,
                    s(0),
                );
                raw.functions[0].blocks[3]
                    .statements
                    .insert(0, f::assign(id.0, Rvalue::Unit, s(0)));
            }
            11 => {
                if let OwnedInstruction::ConsumeVariant { arm, .. } = consume_mut(&mut raw, 1) {
                    *arm = 0;
                }
            }
            12 => {
                raw.functions[0].blocks[0].terminator =
                    f::end(OwnedTerminatorKind::Goto(BlockId(1)), s(0))
            }
            13 => {
                raw.functions[0].blocks[1].terminator =
                    f::end(OwnedTerminatorKind::Goto(BlockId(3)), s(0))
            }
            14 => raw.functions[0].matches[0].arms.swap(0, 1),
            15 => {
                let duplicate = raw.functions[0].blocks[3].statements[0].clone();
                raw.functions[0].blocks[0].statements.push(duplicate);
            }
            17 => raw.functions[0].matches.clear(),
            _ => unreachable!(),
        }
        reject(&raw, &sources, label);
    }
    // A single-arm descriptor still requires its explicit tag-check terminator.
    let mut raw = fixture(&[None], &[0], 0, s(0));
    raw.functions[0].blocks[0].terminator = f::end(OwnedTerminatorKind::Goto(BlockId(1)), s(0));
    reject(
        &raw,
        &sources,
        "single-variant final dispatch cannot become goto",
    );
}

fn merge(span: Span, destination: LocalId) -> BoolMerge {
    BoolMerge {
        operator_span: span,
        destination,
        span,
        incoming: [
            MergeInput {
                predecessor: BlockId(0),
                value: f::operand(destination.0, span),
            },
            MergeInput {
                predecessor: BlockId(1),
                value: f::operand(destination.0, span),
            },
        ],
    }
}
#[test]
fn enum_raw_intermediate_dispatches_and_entries_have_exact_roles() {
    let (sources, s) = f::context();
    for case in 0..4 {
        let mut raw = pair(s(0));
        let id = local(
            &mut raw.functions[0],
            hir::Ty::Bool,
            LocalKind::Temporary,
            s(0),
        );
        match case {
            0 => raw.functions[0].blocks[1].statements.push(f::assign(
                id.0,
                Rvalue::Bool(true),
                s(0),
            )),
            1 => raw.functions[0].blocks[1].merge = Some(merge(s(0), id)),
            2 => raw.functions[0].blocks[2].merge = Some(merge(s(0), id)),
            3 => raw.functions[0].blocks[3].merge = Some(merge(s(0), id)),
            _ => unreachable!(),
        }
        assert_eq!(
            reject(&raw, &sources, "noncanonical block role").kind,
            OwnedFailureKind::Malformed(Malformed::CanonicalSite)
        );
    }
    // The function entry is an implicit incoming edge, absent from predecessor
    // tables. Both owners and return operands are parameters to isolate this proof.
    for entry in [1, 2, 3] {
        let mut raw = parameter_pair(s(0));
        raw.functions[0].entry = BlockId(entry);
        assert_eq!(
            reject(&raw, &sources, "implicit entry bypass").kind,
            OwnedFailureKind::Malformed(Malformed::CanonicalSite)
        );
    }
}

#[test]
fn enum_raw_alternative_predecessors_and_duplicate_edges_reject() {
    let (sources, s) = f::context();
    for target in [1, 3] {
        let mut raw = parameter_pair(s(0));
        raw.functions[0].blocks[2].terminator =
            f::end(OwnedTerminatorKind::Goto(BlockId(target)), s(0));
        assert_eq!(
            reject(&raw, &sources, "alternative incoming edge").kind,
            OwnedFailureKind::Malformed(Malformed::CanonicalSite)
        );
        let mut raw = parameter_pair(s(0));
        let function = &mut raw.functions[0];
        let condition = local(function, hir::Ty::Bool, LocalKind::Parameter, s(0));
        function
            .parameters
            .push(ParameterBinding::Scalar(condition));
        function.blocks[2].terminator = f::end(OwnedTerminatorKind::Goto(BlockId(4)), s(0));
        function.blocks.push(block(
            vec![],
            OwnedTerminatorKind::Branch {
                condition: f::operand(condition.0, s(0)),
                then_block: BlockId(target),
                else_block: BlockId(target),
            },
            s(0),
        ));
        assert_eq!(
            reject(&raw, &sources, "parallel predecessor edges").kind,
            OwnedFailureKind::Malformed(Malformed::CanonicalSite)
        );
    }
}

#[test]
fn enum_raw_payload_arity_type_and_binding_class_are_exact() {
    let (sources, s) = f::context();
    for case in 0..6 {
        let mut raw = pair(s(0));
        let id = binding(&raw, 1);
        match case {
            0 => {
                if let OwnedInstruction::ConsumeVariant { destination, .. } =
                    consume_mut(&mut raw, 1)
                {
                    *destination = None;
                }
            }
            1 => {
                if let OwnedInstruction::ConsumeVariant { destination, .. } =
                    consume_mut(&mut raw, 0)
                {
                    *destination = Some(id);
                }
            }
            2 => raw.functions[0].locals[id.0].ty = hir::Ty::Bool,
            3 => raw.functions[0].locals[id.0].kind = LocalKind::Temporary,
            4 => raw.functions[0].locals[id.0].kind = LocalKind::Parameter,
            5 => raw.functions[0].blocks[0]
                .statements
                .push(f::assign(id.0, Rvalue::I32(12), s(0))),
            _ => unreachable!(),
        }
        reject(&raw, &sources, "payload binding contract");
    }
    let mut raw = fixture(&[Some(hir::Ty::Unit)], &[0], 0, s(0));
    if let OwnedInstruction::ConsumeVariant { destination, .. } = consume_mut(&mut raw, 0) {
        *destination = None;
    }
    reject(&raw, &sources, "unit payload is not nullary");
    for case in 0..3 {
        let mut raw = fixture(&[Some(hir::Ty::I32)], &[0], 0, s(0));
        let instruction = raw.functions[0].blocks[0].statements.last_mut().unwrap();
        if let OwnedInstruction::ConstructEnum { payload, .. } = &mut instruction.kind {
            *payload = if case == 0 {
                None
            } else {
                Some(f::operand(0, s(0)))
            };
        }
        if case == 2 {
            raw.enums[0].variants[0].payload = None;
        }
        reject(&raw, &sources, "constructor payload contract");
    }
}

#[test]
fn enum_raw_payload_definitions_obey_actual_scalar_dominance() {
    let (sources, s) = f::context();
    for case in 0..4 {
        let mut raw = fixture(&[Some(hir::Ty::I32), Some(hir::Ty::I32)], &[0, 1], 0, s(0));
        let first = binding(&raw, 0);
        let second = binding(&raw, 1);
        match case {
            0 => {
                let copy = &mut raw.functions[0].blocks[3].statements[1];
                if let OwnedInstruction::Scalar(Statement::Assign(assign)) = &mut copy.kind {
                    assign.value = Rvalue::Copy(f::operand(first.0, s(0)));
                }
            }
            1 => {
                let destination = local(
                    &mut raw.functions[0],
                    hir::Ty::I32,
                    LocalKind::Temporary,
                    s(0),
                );
                for arm in [2, 3] {
                    raw.functions[0].blocks[arm].terminator =
                        f::end(OwnedTerminatorKind::Goto(BlockId(4)), s(0));
                }
                raw.functions[0].blocks.push(block(
                    vec![f::assign(
                        destination.0,
                        Rvalue::Copy(f::operand(first.0, s(0))),
                        s(0),
                    )],
                    OwnedTerminatorKind::ReturnScalar(f::operand(0, s(0))),
                    s(0),
                ));
            }
            2 => {
                // Keep the canonical consume at statement zero. Moving the
                // correctly typed use before dispatch isolates SSA dominance.
                let copy = raw.functions[0].blocks[2].statements.remove(1);
                raw.functions[0].blocks[0].statements.push(copy);
            }
            3 => {
                if let OwnedInstruction::ConsumeVariant { destination, .. } =
                    consume_mut(&mut raw, 1)
                {
                    *destination = Some(first);
                }
                if let OwnedInstruction::Scalar(Statement::Assign(assign)) =
                    &mut raw.functions[0].blocks[3].statements[1].kind
                {
                    assign.value = Rvalue::Copy(f::operand(first.0, s(0)));
                }
                // Give the now-unused former binding a definition so only the
                // duplicate consume definition is the intentional defect.
                raw.functions[0].blocks[0].statements.push(f::assign(
                    second.0,
                    Rvalue::I32(5),
                    s(0),
                ));
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                reject(&raw, &sources, "payload dominance").kind,
                OwnedFailureKind::Malformed(Malformed::Scalar(_))
            ),
            "case {case}"
        );
    }
    let mut raw = fixture(&[Some(hir::Ty::I32)], &[0], 0, s(0));
    let payload = binding(&raw, 0);
    if let OwnedInstruction::ConstructEnum { payload: value, .. } = &mut raw.functions[0].blocks[0]
        .statements
        .last_mut()
        .unwrap()
        .kind
    {
        *value = Some(f::operand(payload.0, s(0)));
    }
    assert!(matches!(
        reject(&raw, &sources, "constructor operand is a scalar use").kind,
        OwnedFailureKind::Malformed(Malformed::Scalar(_))
    ));
}

#[test]
fn enum_raw_match_source_is_named_local_or_parameter_only() {
    let (sources, s) = f::context();
    let mut raw = nullary_pair(s(0));
    raw.functions[0].owners[0].kind = OwnerKind::Temporary;
    assert_eq!(
        reject(&raw, &sources, "temporary source").kind,
        OwnedFailureKind::Malformed(Malformed::OwnerClass)
    );
    for staged in [true, false] {
        let mut raw = nullary_pair(s(0));
        let source = OwnerPlaceId(raw.functions[0].owners.len());
        let mut callee = f::function(
            1,
            if staged {
                ValueTy::Scalar(hir::Ty::Unit)
            } else {
                ValueTy::Owned(AggregateTy::Enum(EnumId(0)))
            },
            s(0),
        );
        callee
            .owners
            .push(enum_owner(OwnerKind::Parameter { position: 0 }, s(0)));
        callee
            .parameters
            .push(ParameterBinding::Owned(OwnerPlaceId(0)));
        callee.locals.push(f::scalar(hir::Ty::Unit, s(0)));
        callee.blocks.push(block(
            vec![f::assign(0, Rvalue::Unit, s(0))],
            if staged {
                OwnedTerminatorKind::ReturnScalar(f::operand(0, s(0)))
            } else {
                OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0))
            },
            s(0),
        ));
        raw.functions.push(callee);
        let function = &mut raw.functions[0];
        let staged_owner = if staged { source } else { OwnerPlaceId(2) };
        function.owners.push(enum_owner(
            if staged {
                OwnerKind::StagedArgument {
                    call: CallSiteId(0),
                    argument: 0,
                }
            } else {
                OwnerKind::CallResult {
                    call: CallSiteId(0),
                }
            },
            s(0),
        ));
        if !staged {
            function.owners.push(enum_owner(
                OwnerKind::StagedArgument {
                    call: CallSiteId(0),
                    argument: 0,
                },
                s(0),
            ));
        }
        function.calls.push(CallDecl {
            target: hir::DefId(1),
            arguments: vec![ArgumentSlot::Owned(staged_owner)],
            result: if staged {
                CallResult::Scalar(LocalId(0))
            } else {
                CallResult::Owned(source)
            },
            parent: None,
            span: s(0),
        });
        function.matches[0].source = source;
        assert_eq!(
            reject(&raw, &sources, "staged/call-result source").kind,
            OwnedFailureKind::Malformed(Malformed::OwnerClass)
        );
    }
}

#[test]
fn enum_raw_unavailable_dispatch_and_use_after_consume_are_ownership_errors() {
    let (sources, s) = f::context();
    for case in 0..4 {
        let mut raw = nullary_pair(s(0));
        match case {
            0 => {
                raw.functions[0].blocks[0].statements.pop();
            } // Live, uninitialized source.
            1 => raw.functions[0].blocks[0].statements.push(f::instruction(
                OwnedInstruction::Discard(OwnerPlaceId(0)),
                s(0),
            )),
            2 => raw.functions[0].blocks[2].statements.insert(
                1,
                f::instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), s(0)),
            ),
            3 => raw.functions[0].blocks[3].statements.insert(
                1,
                f::instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), s(0)),
            ),
            _ => unreachable!(),
        }
        assert_eq!(
            reject(&raw, &sources, "enum source unavailable").kind,
            OwnedFailureKind::Ownership(Violation::Unavailable)
        );
    }
}

/// The mutable source survives the join; each restoration has its own ordinary
/// temporary lifetime and canonical constructor. No production graph helper.
fn restored_pair(restored: [bool; 2], span: Span) -> RawOwnedProgram {
    let mut raw = nullary_pair(span);
    let function = &mut raw.functions[0];
    function.owners[0].kind = OwnerKind::Local { mutable: true };
    for (arm, restore) in restored.into_iter().enumerate() {
        let entry = 2 + arm;
        function.blocks[entry].statements.truncate(1);
        if restore {
            let source = OwnerPlaceId(function.owners.len());
            function.owners.push(enum_owner(OwnerKind::Temporary, span));
            function.blocks[entry].statements.extend([
                f::instruction(OwnedInstruction::StorageLive(source), span),
                f::instruction(
                    OwnedInstruction::ConstructEnum {
                        destination: source,
                        variant: variant(arm),
                        payload: None,
                    },
                    span,
                ),
                f::instruction(
                    OwnedInstruction::Replace {
                        destination: OwnerPlaceId(0),
                        source,
                    },
                    span,
                ),
                f::instruction(OwnedInstruction::StorageEnd(source), span),
            ]);
        }
        function.blocks[entry].terminator = f::end(OwnedTerminatorKind::Goto(BlockId(4)), span);
    }
    function.blocks.push(block(
        vec![
            f::instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), span),
            f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), span),
        ],
        OwnedTerminatorKind::ReturnScalar(f::operand(0, span)),
        span,
    ));
    raw
}
#[test]
fn enum_raw_only_continuing_arms_restore_join_availability() {
    let (sources, s) = f::context();
    assert!(probe(&restored_pair([true, true], s(0)), &sources).is_ok());
    for restored in [[false, false], [true, false], [false, true]] {
        assert_eq!(
            reject(
                &restored_pair(restored, s(0)),
                &sources,
                "partial restoration"
            )
            .kind,
            OwnedFailureKind::Ownership(Violation::Unavailable)
        );
    }
    let mut raw = restored_pair([true, false], s(0));
    raw.functions[0].blocks[3].statements.push(f::instruction(
        OwnedInstruction::StorageEnd(OwnerPlaceId(0)),
        s(0),
    ));
    raw.functions[0].blocks[3].terminator =
        f::end(OwnedTerminatorKind::ReturnScalar(f::operand(0, s(0))), s(0));
    assert!(
        probe(&raw, &sources).is_ok(),
        "returning arm is not a join predecessor"
    );
}

fn looping_pair(restored: [bool; 2], span: Span) -> RawOwnedProgram {
    let mut raw = restored_pair(restored, span);
    let function = &mut raw.functions[0];
    let initial = std::mem::take(&mut function.blocks[0].statements);
    let exit = function.blocks.pop().unwrap();
    let condition = local(function, hir::Ty::Bool, LocalKind::Temporary, span);
    let mut preheader = initial;
    preheader.push(f::assign(condition.0, Rvalue::Bool(false), span));
    function.blocks.push(block(
        preheader,
        OwnedTerminatorKind::Goto(BlockId(0)),
        span,
    ));
    function.blocks.push(block(
        vec![],
        OwnedTerminatorKind::Branch {
            condition: f::operand(condition.0, span),
            then_block: BlockId(0),
            else_block: BlockId(6),
        },
        span,
    ));
    function.blocks.push(exit);
    function.entry = BlockId(4);
    for entry in [2, 3] {
        function.blocks[entry].terminator = f::end(OwnedTerminatorKind::Goto(BlockId(5)), span);
    }
    raw
}
#[test]
fn enum_raw_loop_backedges_require_restoration_or_a_fresh_lifetime() {
    let (sources, s) = f::context();
    assert!(probe(&looping_pair([true, true], s(0)), &sources).is_ok());
    for restored in [[false, false], [true, false], [false, true]] {
        assert_eq!(
            reject(
                &looping_pair(restored, s(0)),
                &sources,
                "loop backedge without restoration"
            )
            .kind,
            OwnedFailureKind::Ownership(Violation::Unavailable)
        );
    }
    let mut raw = nullary_pair(s(0));
    let function = &mut raw.functions[0];
    let unit = function.blocks[0].statements.remove(0);
    let condition = local(function, hir::Ty::Bool, LocalKind::Temporary, s(0));
    function.blocks.push(block(
        vec![unit, f::assign(condition.0, Rvalue::Bool(false), s(0))],
        OwnedTerminatorKind::Goto(BlockId(0)),
        s(0),
    ));
    function.blocks.push(block(
        vec![],
        OwnedTerminatorKind::Branch {
            condition: f::operand(condition.0, s(0)),
            then_block: BlockId(0),
            else_block: BlockId(6),
        },
        s(0),
    ));
    function.blocks.push(block(
        vec![],
        OwnedTerminatorKind::ReturnScalar(f::operand(0, s(0))),
        s(0),
    ));
    function.entry = BlockId(4);
    for entry in [2, 3] {
        function.blocks[entry].terminator = f::end(OwnedTerminatorKind::Goto(BlockId(5)), s(0));
    }
    assert!(
        probe(&raw, &sources).is_ok(),
        "loop-local source has a fresh live/construct/end cycle"
    );
}

fn nested_pair(span: Span) -> RawOwnedProgram {
    let mut raw = parameter_pair(span);
    let function = &mut raw.functions[0];
    function
        .owners
        .push(enum_owner(OwnerKind::Parameter { position: 2 }, span));
    function
        .parameters
        .push(ParameterBinding::Owned(OwnerPlaceId(1)));
    function.blocks[2].terminator = f::end(OwnedTerminatorKind::Goto(BlockId(4)), span);
    function.matches.push(MatchDecl {
        source: OwnerPlaceId(1),
        span,
        arms: vec![
            MatchArm {
                variant: variant(0),
                dispatch: BlockId(4),
                entry: BlockId(6),
            },
            MatchArm {
                variant: variant(1),
                dispatch: BlockId(5),
                entry: BlockId(7),
            },
        ],
    });
    for arm in 0..2 {
        function.blocks.push(block(
            vec![],
            OwnedTerminatorKind::MatchDispatch {
                match_id: MatchId(1),
                arm,
            },
            span,
        ));
    }
    for arm in 0..2 {
        function.blocks.push(block(
            vec![
                f::instruction(
                    OwnedInstruction::ConsumeVariant {
                        match_id: MatchId(1),
                        arm,
                        destination: None,
                    },
                    span,
                ),
                f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(1)), span),
            ],
            OwnedTerminatorKind::ReturnScalar(f::operand(0, span)),
            span,
        ));
    }
    raw
}
#[test]
fn enum_raw_nested_matches_require_globally_disjoint_roles_and_sources_available() {
    let (sources, s) = f::context();
    assert!(probe(&nested_pair(s(0)), &sources).is_ok());
    for case in 0..4 {
        let mut raw = nested_pair(s(0));
        match case {
            0 => {
                raw.functions[0].matches[1].arms[0].dispatch = BlockId(2);
                raw.functions[0].blocks[2].terminator = f::end(
                    OwnedTerminatorKind::MatchDispatch {
                        match_id: MatchId(1),
                        arm: 0,
                    },
                    s(0),
                );
            }
            1 => raw.functions[0].matches[1].arms[0].dispatch = BlockId(1),
            2 => raw.functions[0].matches[1].arms[0].entry = BlockId(3),
            3 => {
                if let OwnedInstruction::ConsumeVariant { match_id, .. } =
                    &mut raw.functions[0].blocks[6].statements[0].kind
                {
                    *match_id = MatchId(0);
                }
            }
            _ => unreachable!(),
        }
        assert_eq!(
            reject(&raw, &sources, "global match role collision").kind,
            OwnedFailureKind::Malformed(Malformed::CanonicalSite)
        );
    }
    let mut raw = nested_pair(s(0));
    raw.functions[0].matches[1].source = OwnerPlaceId(0);
    raw.functions[0].blocks[2].statements.truncate(1);
    assert_eq!(
        reject(&raw, &sources, "two matches consume the same owner").kind,
        OwnedFailureKind::Ownership(Violation::Unavailable)
    );
}

#[test]
fn enum_raw_match_diagnostics_cannot_be_spoofed_by_producer_spans() {
    let (sources, s) = f::context();
    for case in 0..4 {
        let mut raw = pair(s(0));
        match case {
            0 => raw.functions[0].blocks[0].terminator.as_mut().unwrap().span = s(1),
            1 => raw.functions[0].blocks[3].statements[0].span = s(1),
            2 => {
                raw.functions[0].blocks[0]
                    .terminator
                    .as_mut()
                    .unwrap()
                    .diagnostic_origins = Some(DiagnosticOrigins {
                    primary: s(1),
                    cause: s(2),
                })
            }
            3 => {
                raw.functions[0].blocks[3].statements[0].diagnostic_origins =
                    Some(DiagnosticOrigins {
                        primary: s(1),
                        cause: s(2),
                    })
            }
            _ => unreachable!(),
        }
        let error = reject(&raw, &sources, "spoofed match diagnostic");
        assert_eq!(
            error.kind,
            OwnedFailureKind::Malformed(Malformed::CanonicalSite)
        );
        assert_eq!(error.primary.get(), Some(s(0)));
    }
    let mut raw = pair(s(0));
    raw.functions[0].blocks[0]
        .terminator
        .as_mut()
        .unwrap()
        .diagnostic_origins = Some(DiagnosticOrigins {
        primary: s(0),
        cause: s(0),
    });
    raw.functions[0].blocks[3].statements[0].diagnostic_origins = Some(DiagnosticOrigins {
        primary: s(0),
        cause: s(0),
    });
    assert!(probe(&raw, &sources).is_ok());
}

fn scalar_program(span: Span) -> RawOwnedProgram {
    let mut function = f::function(0, ValueTy::Scalar(hir::Ty::Unit), span);
    function.locals.push(f::scalar(hir::Ty::Unit, span));
    function.blocks.push(block(
        vec![f::assign(0, Rvalue::Unit, span)],
        OwnedTerminatorKind::ReturnScalar(f::operand(0, span)),
        span,
    ));
    RawOwnedProgram {
        builtins: BuiltinOrigins::None,
        enums: vec![],
        records: vec![],
        functions: vec![function],
    }
}
#[test]
fn enum_raw_proof_is_required_by_production_and_executable_probes() {
    let (sources, s) = f::context();
    let make = || pair(s(0));
    let usage = probe(&make(), &sources).unwrap();
    assert_eq!(verify_owned(make(), &sources).unwrap().usage(), usage);
    assert!(verified::probe_array_validation(&make(), &sources, budget::Limits::DEFAULT).is_ok());
    assert!(verified::probe_array_reference(
        make(),
        &sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        execute::Limits::default(),
        execute::ObservationControl::default(),
    )
    .unwrap()
    .result
    .is_ok());
    assert!(verified::probe_array_native(
        make(),
        &sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        &sources,
        native::NativeControl::default(),
    )
    .unwrap()
    .result
    .is_ok());
    // Forging an operation or descriptor without the required declaration,
    // canonical sites and ownership proof still cannot mint a witness.
    for case in 0..4 {
        let mut raw = scalar_program(s(0));
        match case {
            0 => raw.functions[0].matches.push(MatchDecl {
                source: OwnerPlaceId(usize::MAX),
                arms: vec![],
                span: s(0),
            }),
            1 => raw.functions[0].blocks[0].statements.push(f::instruction(
                OwnedInstruction::ConstructEnum {
                    destination: OwnerPlaceId(usize::MAX),
                    variant: variant(0),
                    payload: None,
                },
                s(0),
            )),
            2 => raw.functions[0].blocks[0].statements.push(f::instruction(
                OwnedInstruction::ConsumeVariant {
                    match_id: MatchId(usize::MAX),
                    arm: 0,
                    destination: None,
                },
                s(0),
            )),
            3 => {
                raw.functions[0].blocks[0].terminator = f::end(
                    OwnedTerminatorKind::MatchDispatch {
                        match_id: MatchId(usize::MAX),
                        arm: 0,
                    },
                    s(0),
                )
            }
            _ => unreachable!(),
        }
        assert!(
            verify_owned(raw, &sources).is_err(),
            "unproved carrier {case}"
        );
    }
}

#[test]
fn enum_raw_empty_match_headers_preserve_old_program_proofs() {
    let (sources, s) = f::context();
    let raw = scalar_program(s(0));
    let usage = probe(&raw, &sources).unwrap();
    assert_eq!(usage, verify_owned(raw, &sources).unwrap().usage());
    assert_eq!(
        usage.metadata_bytes,
        size_of::<Vec<RawEnumDecl>>() + size_of::<Vec<MatchDecl>>()
    );
    assert_eq!(
        (
            usage.expanded_events,
            usage.work,
            usage.scratch_bytes,
            usage.owner_cells
        ),
        (0, 0, 0, 0)
    );
    // Independent old record carrier, with no dependency on frozen schedules.
    let mut raw = scalar_program(s(0));
    raw.records.push(f::record(&[], s(0)));
    raw.functions[0]
        .owners
        .push(f::owner(OwnerKind::Local { mutable: false }, s(0)));
    raw.functions[0].blocks[0].statements.extend([
        f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(0)),
        f::instruction(
            OwnedInstruction::Construct {
                destination: OwnerPlaceId(0),
                fields: vec![],
            },
            s(0),
        ),
        f::instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), s(0)),
        f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), s(0)),
    ]);
    let usage = probe(&raw, &sources).unwrap();
    assert_eq!(usage, verify_owned(raw, &sources).unwrap().usage());
    assert_eq!(usage.expanded_events, 5);
    assert_eq!(usage.work, 36 * (1 + 2 + 5 + 1 + 1));
}

#[test]
fn enum_raw_descriptor_inventory_is_independent_and_precedes_allocation() {
    let (sources, s) = f::context();
    let raw = nullary_pair(s(0));
    let usage = probe(&raw, &sources).unwrap();
    // Hand inventory: enum1/variants2, O1, V1, B4, E3, statements7,
    // matches1/arms2, no payload/local copies, merges, calls or projections.
    assert_eq!(usage.owners, 1);
    assert_eq!(usage.expanded_events, 3 + 7 + 1 + 2);
    assert_eq!(usage.work, 4 * 3 + 36 * (1 + 8 + 3 + 7 + 1 + 6 + 1 + 1));
    let metadata = size_of::<Vec<RawEnumDecl>>()
        + size_of::<RawEnumDecl>()
        + 2 * size_of::<RawVariantDecl>()
        + size_of::<Vec<MatchDecl>>()
        + size_of::<MatchDecl>()
        + 2 * size_of::<MatchArm>()
        + 11 * size_of::<Option<DiagnosticOrigins>>()
        + size_of::<shape::OwnerSites>();
    assert_eq!(usage.metadata_bytes, metadata);
    let scratch = (4 * (1 + 4 * size_of::<usize>()))
        .max(8 + 12 * size_of::<usize>())
        .max(4 * size_of::<shape::MatchBlockRole>() + 2);
    assert_eq!(usage.scratch_bytes, scratch);
    assert_eq!((usage.owner_cells, usage.owner_layout_bytes), (2, 8));
    for selector in 0..5 {
        let mut limits = budget::Limits::DEFAULT;
        let expected = match selector {
            0 => {
                limits.owners = usage.owners;
                "owners"
            }
            1 => {
                limits.events = usage.expanded_events;
                "expanded ownership events"
            }
            2 => {
                limits.work = usage.work;
                "ownership work"
            }
            3 => {
                limits.metadata = metadata;
                "ownership metadata"
            }
            _ => {
                limits.scratch = scratch;
                "ownership scratch"
            }
        };
        assert!(verified::probe_enum_validation(&raw, &sources, limits).is_ok());
        match selector {
            0 => limits.owners -= 1,
            1 => limits.events -= 1,
            2 => limits.work -= 1,
            3 => limits.metadata -= 1,
            _ => limits.scratch -= 1,
        }
        let error = budget::fail_allocation_after(0, || {
            verified::probe_enum_validation(&raw, &sources, limits)
        })
        .unwrap_err();
        assert_eq!(error.kind, OwnedFailureKind::Resource(expected));
    }
    assert_eq!(
        budget::fail_allocation_after(0, || budget::preflight(&raw, budget::Limits::DEFAULT))
            .unwrap()
            .metadata_bytes,
        metadata
    );
    let mut failures = 0;
    loop {
        match budget::fail_allocation_after(failures, || probe(&raw, &sources)) {
            Ok(_) => break,
            Err(error) => assert_eq!(
                error.kind,
                OwnedFailureKind::Resource("injected allocation failure")
            ),
        }
        failures += 1;
        assert!(
            failures < 256,
            "bounded allocation sweep did not reach success"
        );
    }
    assert!(
        failures > 4,
        "must exercise real nested match and ordinary proof reserves"
    );
}

#[test]
fn enum_raw_match_count_arithmetic_rejects_overflow_and_event_excess() {
    for counts in [
        budget::FunctionCounts {
            matches: usize::MAX,
            ..budget::FunctionCounts::default()
        },
        budget::FunctionCounts {
            match_arms: usize::MAX,
            ..budget::FunctionCounts::default()
        },
        budget::FunctionCounts {
            matches: MAX_ASSIGNMENTS + 1,
            ..budget::FunctionCounts::default()
        },
        budget::FunctionCounts {
            max_match_arms: 257,
            ..budget::FunctionCounts::default()
        },
    ] {
        let result = budget::fail_allocation_after(0, || {
            budget::account_function(
                counts,
                budget::Limits::DEFAULT,
                &mut budget::ProgramCounts::default(),
            )
        });
        assert!(matches!(
            result.unwrap_err().kind,
            OwnedFailureKind::Resource(_)
        ));
    }
}

#[test]
fn enum_raw_static_costs_are_fixed_without_constructing_an_enum_witness() {
    let (sources, s) = f::context();
    let witness = verify_owned(scalar_program(s(0)), &sources).unwrap();
    let plan = plan::ExecutionPlan::build(&witness).unwrap();
    let id = hir::DefId(0);
    // These descriptors are inert arguments to cost queries. They are never
    // inserted into the witness or sent to an execution/native consumer.
    let construction = OwnedInstruction::ConstructEnum {
        destination: OwnerPlaceId(usize::MAX),
        variant: VariantId {
            enumeration: EnumId(usize::MAX),
            index: usize::MAX,
        },
        payload: None,
    };
    assert_eq!(plan.statement_cost(id, &construction), 3);
    for destination in [None, Some(LocalId(usize::MAX))] {
        let consume = OwnedInstruction::ConsumeVariant {
            match_id: MatchId(usize::MAX),
            arm: usize::MAX,
            destination,
        };
        assert_eq!(plan.statement_cost(id, &consume), 3);
        for arm in [0, 1, 2, 255] {
            let dispatch = OwnedTerminatorKind::MatchDispatch {
                match_id: MatchId(usize::MAX),
                arm,
            };
            assert_eq!(plan.terminator_cost(id, &dispatch), 1);
            assert_eq!(
                (arm + 1) * plan.terminator_cost(id, &dispatch) + plan.statement_cost(id, &consume),
                arm + 4
            );
        }
    }
    assert_eq!(
        plan.statement_cost(
            id,
            &OwnedInstruction::Scalar(Statement::Assign(Assign {
                destination: LocalId(0),
                value: Rvalue::Unit,
                span: s(0),
            }))
        ),
        1
    );
    assert_eq!(
        plan.terminator_cost(id, &OwnedTerminatorKind::ReturnScalar(f::operand(0, s(0)))),
        1
    );
}
