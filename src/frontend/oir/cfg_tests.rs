//! Adversarial raw CFG checks, separate from source lowering fixtures.
use super::*;

fn fixture(blocks: usize, locals: usize) -> (SourceMap, Program) {
    let mut sources = SourceMap::new();
    let file = sources.add("cfg.ox".into(), "éx".into());
    let span = sources.get(file).span(2, 3);
    let mut decls = vec![
        LocalDecl {
            ty: hir::Ty::Bool,
            kind: LocalKind::Temporary,
            span
        };
        locals
    ];
    decls[0].kind = LocalKind::Parameter;
    let block = BasicBlock {
        merge: None,
        span,
        statements: vec![],
        terminator: Some(Terminator {
            span,
            kind: TerminatorKind::Return(op(0, span)),
        }),
    };
    (
        sources,
        Program {
            functions: vec![Function {
                places: vec![],
                id: hir::DefId(0),
                span,
                result: hir::Ty::Bool,
                param_count: 1,
                locals: decls,
                entry: BlockId(0),
                blocks: vec![block; blocks],
            }],
        },
    )
}
fn op(local: usize, span: Span) -> Operand {
    Operand {
        local: LocalId(local),
        span,
    }
}
fn assign(f: &mut Function, block: usize, destination: usize, value: Rvalue) {
    f.blocks[block].statements.push(Statement::Assign(Assign {
        destination: LocalId(destination),
        value,
        span: f.span,
    }));
}
fn reject(sources: &SourceMap, program: Program, kind: FailureKind) {
    let err = verify::verify(program, sources).unwrap_err();
    assert_eq!(err.kind, kind);
    assert_eq!(err.stage, "oir-verify");
    let diagnostic = err.diagnostic(sources);
    assert_eq!(diagnostic.code, "E0500");
    assert!(diagnostic.primary.is_none_or(|s| sources.is_valid_span(s)));
    assert!(diagnostic
        .render_json(sources)
        .contains("internal compiler error"));
    assert!(diagnostic
        .render_human(sources)
        .contains("internal compiler error"));
}

#[test]
fn global_duplicate_definitions_are_checked_before_any_reads() {
    let (sources, mut p) = fixture(1, 4);
    let f = &mut p.functions[0];
    let span = f.span;
    assign(f, 0, 2, Rvalue::Copy(op(1, span)));
    assign(f, 0, 3, Rvalue::Bool(true));
    assign(f, 0, 3, Rvalue::Bool(false));
    reject(&sources, p, FailureKind::AlreadyInitialized);
}

fn end(f: &mut Function, block: usize, kind: TerminatorKind) {
    f.blocks[block].terminator = Some(Terminator { kind, span: f.span });
}
fn branch(f: &mut Function, block: usize, left: usize, right: usize) {
    end(
        f,
        block,
        TerminatorKind::Branch {
            condition: op(0, f.span),
            then_block: BlockId(left),
            else_block: BlockId(right),
        },
    );
}
fn goto(f: &mut Function, block: usize, target: usize) {
    end(
        f,
        block,
        TerminatorKind::Goto {
            target: BlockId(target),
        },
    );
}
fn ret(f: &mut Function, block: usize, local: usize) {
    end(f, block, TerminatorKind::Return(op(local, f.span)));
}
fn call(f: &mut Function, block: usize, destination: usize, continuation: usize) {
    end(
        f,
        block,
        TerminatorKind::Call {
            target: hir::DefId(0),
            args: vec![op(0, f.span)],
            destination: LocalId(destination),
            continuation: BlockId(continuation),
        },
    );
}
fn diamond() -> (SourceMap, Program) {
    let (s, mut p) = fixture(4, 4);
    let f = &mut p.functions[0];
    branch(f, 0, 1, 2);
    goto(f, 1, 3);
    goto(f, 2, 3);
    (s, p)
}

#[test]
fn diamond_joins_require_definitions_on_every_path() {
    let (s, mut p) = diamond();
    let f = &mut p.functions[0];
    assign(f, 1, 1, Rvalue::Bool(true));
    ret(f, 3, 1);
    reject(&s, p, FailureKind::Uninitialized);

    let (s, mut p) = diamond();
    let f = &mut p.functions[0];
    let span = f.span;
    assign(f, 0, 1, Rvalue::Bool(true));
    assign(f, 1, 2, Rvalue::Copy(op(1, span)));
    assign(f, 2, 3, Rvalue::Copy(op(1, span)));
    ret(f, 3, 1);
    verify::verify(p, &s).unwrap();
}

#[test]
fn mutually_exclusive_definitions_and_outer_overwrites_are_duplicates() {
    for (left, right) in [(1, 2), (0, 1), (0, 2)] {
        let (s, mut p) = diamond();
        let f = &mut p.functions[0];
        assign(f, left, 1, Rvalue::Bool(true));
        assign(f, right, 1, Rvalue::Bool(false));
        reject(&s, p, FailureKind::AlreadyInitialized);
    }
    let (s, mut p) = diamond();
    assign(&mut p.functions[0], 2, 0, Rvalue::Bool(true));
    reject(&s, p, FailureKind::AlreadyInitialized);

    let (s, mut p) = diamond();
    let f = &mut p.functions[0];
    call(f, 1, 1, 3);
    call(f, 2, 1, 3);
    reject(&s, p, FailureKind::AlreadyInitialized);
}

#[test]
fn call_result_availability_is_the_normal_edge_not_just_the_successor() {
    let (s, mut p) = diamond();
    let f = &mut p.functions[0];
    call(f, 1, 1, 3);
    ret(f, 3, 1);
    reject(&s, p, FailureKind::Uninitialized);

    let (s, mut p) = fixture(5, 3);
    let f = &mut p.functions[0];
    call(f, 0, 1, 1);
    branch(f, 1, 2, 3);
    goto(f, 2, 4);
    goto(f, 3, 4);
    ret(f, 4, 1);
    verify::verify(p.clone(), &s).unwrap();
    let f = &mut p.functions[0];
    let span = f.span;
    assign(f, 0, 2, Rvalue::Copy(op(1, span)));
    reject(&s, p, FailureKind::Uninitialized);
}

#[test]
fn returning_arm_is_not_a_predecessor_of_the_join() {
    let (s, mut p) = diamond();
    let f = &mut p.functions[0];
    ret(f, 1, 0);
    assign(f, 2, 1, Rvalue::Bool(false));
    ret(f, 3, 1);
    verify::verify(p, &s).unwrap();
}

#[test]
fn parameter_unused_slots_and_statement_order_are_distinct() {
    let (s, p) = diamond();
    verify::verify(p, &s).unwrap(); // Parameters dominate, undefined unused locals are legal.
    for self_use in [true, false] {
        let (s, mut p) = diamond();
        let f = &mut p.functions[0];
        let span = f.span;
        assign(
            f,
            1,
            1,
            Rvalue::Copy(op(if self_use { 1 } else { 2 }, span)),
        );
        if !self_use {
            assign(f, 1, 2, Rvalue::Bool(true));
        }
        reject(&s, p, FailureKind::Uninitialized);
    }
    let (s, mut p) = diamond();
    let f = &mut p.functions[0];
    let span = f.span;
    assign(f, 1, 1, Rvalue::Copy(op(0, span)));
    assign(f, 1, 2, Rvalue::Copy(op(1, span)));
    verify::verify(p, &s).unwrap();
}

#[test]
fn branch_conditions_and_both_targets_are_validated() {
    for target in [0, 1] {
        let (s, mut p) = diamond();
        let f = &mut p.functions[0];
        branch(
            f,
            0,
            if target == 0 { usize::MAX } else { 1 },
            if target == 1 { usize::MAX } else { 2 },
        );
        reject(&s, p, FailureKind::InvalidBlock);
    }
    let (s, mut p) = diamond();
    goto(&mut p.functions[0], 1, usize::MAX);
    reject(&s, p, FailureKind::InvalidBlock);
    for (id, expected) in [
        (usize::MAX, FailureKind::InvalidLocal),
        (1, FailureKind::Uninitialized),
    ] {
        let (s, mut p) = diamond();
        if let TerminatorKind::Branch { condition, .. } =
            &mut p.functions[0].blocks[0].terminator.as_mut().unwrap().kind
        {
            condition.local = LocalId(id);
        }
        reject(&s, p, expected);
    }
    let (s, mut p) = diamond();
    let f = &mut p.functions[0];
    f.locals[1].ty = hir::Ty::Unit;
    assign(f, 0, 1, Rvalue::Unit);
    if let TerminatorKind::Branch { condition, .. } =
        &mut f.blocks[0].terminator.as_mut().unwrap().kind
    {
        condition.local = LocalId(1);
    }
    reject(&s, p, FailureKind::TypeMismatch);
}

#[test]
fn new_terminators_check_spans_even_on_unreachable_blocks() {
    let (s, base) = diamond();
    let valid = base.functions[0].span;
    for bad in [
        Span {
            start: 1,
            end: 2,
            ..valid
        },
        Span {
            end: usize::MAX,
            ..valid
        },
        Span {
            file: super::super::source::SourceFileId(usize::MAX),
            ..valid
        },
    ] {
        for site in 0..3 {
            let mut p = base.clone();
            let f = &mut p.functions[0];
            match site {
                0 => f.blocks[0].terminator.as_mut().unwrap().span = bad,
                1 => {
                    if let TerminatorKind::Branch { condition, .. } =
                        &mut f.blocks[0].terminator.as_mut().unwrap().kind
                    {
                        condition.span = bad;
                    }
                }
                _ => f.blocks[1].terminator.as_mut().unwrap().span = bad,
            }
            reject(&s, p, FailureKind::InvalidSpan);
        }
        let mut p = base.clone();
        let f = &mut p.functions[0];
        f.blocks.push(BasicBlock {
            merge: None,
            span: valid,
            statements: vec![],
            terminator: Some(Terminator {
                span: bad,
                kind: TerminatorKind::Goto { target: BlockId(0) },
            }),
        });
        reject(&s, p, FailureKind::InvalidSpan);
    }
}

#[test]
fn cycles_and_unreachable_blocks_fail_after_structural_checks() {
    for target in [0, 1, 2] {
        let (s, mut p) = diamond();
        goto(&mut p.functions[0], 3, target);
        reject(&s, p, FailureKind::Cycle);
    }
    let (s, mut p) = diamond();
    let b = p.functions[0].blocks[3].clone();
    p.functions[0].blocks.push(b);
    reject(&s, p.clone(), FailureKind::Unreachable);
    goto(&mut p.functions[0], 4, usize::MAX);
    reject(&s, p.clone(), FailureKind::InvalidBlock);
    p.functions[0].blocks[4].terminator = None;
    reject(&s, p, FailureKind::MissingTerminator);
}

pub(super) fn permute(p: &mut Program, order: &[usize]) {
    let f = &mut p.functions[0];
    let old = f.blocks.clone();
    for (from, to) in order.iter().copied().enumerate() {
        f.blocks[to] = old[from].clone();
    }
    f.entry = BlockId(order[f.entry.0]);
    for block in &mut f.blocks {
        if let Some(merge) = &mut block.merge {
            for input in &mut merge.incoming {
                input.predecessor = BlockId(order[input.predecessor.0]);
            }
        }
        match &mut block.terminator.as_mut().unwrap().kind {
            TerminatorKind::Branch {
                then_block,
                else_block,
                ..
            } => {
                *then_block = BlockId(order[then_block.0]);
                *else_block = BlockId(order[else_block.0]);
            }
            TerminatorKind::Goto { target } => *target = BlockId(order[target.0]),
            TerminatorKind::Call { continuation, .. } => {
                *continuation = BlockId(order[continuation.0])
            }
            TerminatorKind::Return(_) => {}
        }
    }
}

#[test]
fn equal_successors_and_permuted_nonzero_entry_are_legal() {
    let (s, mut p) = fixture(2, 2);
    branch(&mut p.functions[0], 0, 1, 1);
    verify::verify(p.clone(), &s).unwrap();
    permute(&mut p, &[1, 0]);
    verify::verify(p, &s).unwrap();
    let (s, mut p) = diamond();
    assign(&mut p.functions[0], 0, 1, Rvalue::Bool(true));
    ret(&mut p.functions[0], 3, 1);
    permute(&mut p, &[2, 3, 0, 1]);
    verify::verify(p, &s).unwrap();
}

// Independent oracle: remove one vertex (or one call edge), then search paths.
// It does not construct dominator sets/trees or call production CFG helpers.
fn path_exists(
    edges: &[Vec<usize>],
    destination: usize,
    removed_vertex: Option<usize>,
    removed_edge: Option<(usize, usize)>,
) -> bool {
    let mut work = vec![0];
    let mut seen = vec![false; edges.len()];
    while let Some(node) = work.pop() {
        if Some(node) == removed_vertex || seen[node] {
            continue;
        }
        if node == destination {
            return true;
        }
        seen[node] = true;
        for &to in &edges[node] {
            if removed_edge != Some((node, to)) {
                work.push(to);
            }
        }
    }
    false
}
fn graph_fixture(edges: &[Vec<usize>]) -> (SourceMap, Program) {
    let (s, mut p) = fixture(edges.len(), 3);
    let f = &mut p.functions[0];
    for (from, to) in edges.iter().enumerate() {
        match to.as_slice() {
            [] => {}
            [target] => goto(f, from, *target),
            [left, right] => branch(f, from, *left, *right),
            _ => panic!("oracle only generates bounded out-degree"),
        }
    }
    (s, p)
}

#[test]
fn exhaustive_small_dags_match_path_removal_oracle_under_permutations() {
    let mut graph_count = 0;
    let mut comparisons = 0;
    for n in 1..=6 {
        let pairs: Vec<_> = (0..n)
            .flat_map(|a| (a + 1..n).map(move |b| (a, b)))
            .collect();
        for mask in 0..1usize << pairs.len() {
            let mut edges = vec![vec![]; n];
            for (bit, &(a, b)) in pairs.iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    edges[a].push(b);
                }
            }
            if edges.iter().any(|e| e.len() > 2)
                || (0..n).any(|v| !path_exists(&edges, v, None, None))
            {
                continue;
            }
            graph_count += 1;
            let (s, base) = graph_fixture(&edges);
            for definition in 0..n {
                for use_block in 0..n {
                    for call_result in [false, true] {
                        if call_result && edges[definition].len() != 1 {
                            continue;
                        }
                        let expected = if call_result {
                            definition != use_block
                                && !path_exists(
                                    &edges,
                                    use_block,
                                    None,
                                    Some((definition, edges[definition][0])),
                                )
                        } else {
                            !path_exists(&edges, use_block, Some(definition), None)
                        };
                        for reverse in [false, true] {
                            let mut p = base.clone();
                            let f = &mut p.functions[0];
                            let span = f.span;
                            if call_result {
                                call(f, definition, 1, edges[definition][0]);
                            } else {
                                assign(f, definition, 1, Rvalue::Bool(true));
                            }
                            assign(f, use_block, 2, Rvalue::Copy(op(1, span)));
                            if reverse {
                                permute(&mut p, &(0..n).rev().collect::<Vec<_>>());
                            }
                            let result = verify::verify(p, &s);
                            assert_eq!(result.is_ok(), expected, "edges={edges:?}, definition={definition}, use={use_block}, call={call_result}, reversed={reverse}, result={result:?}");
                            if let Err(err) = result {
                                assert_eq!(err.kind, FailureKind::Uninitialized);
                            }
                            comparisons += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(graph_count, 826);
    assert!(comparisons > 20_000);
}

#[test]
fn long_diamond_call_ladder_combines_many_locals_and_joins() {
    let steps = 20_000;
    let (s, mut p) = fixture(4 * steps + 1, 3 * steps + 1);
    let f = &mut p.functions[0];
    let span = f.span;
    for i in 0..steps {
        let block = 4 * i;
        let slot = 3 * i + 1;
        let prior = if i == 0 { 0 } else { slot - 3 };
        assign(f, block, slot, Rvalue::Copy(op(prior, span)));
        branch(f, block, block + 1, block + 2);
        call(f, block + 1, slot + 1, block + 3);
        goto(f, block + 2, block + 4);
        assign(f, block + 3, slot + 2, Rvalue::Copy(op(slot + 1, span)));
        goto(f, block + 3, block + 4);
    }
    ret(f, 4 * steps, 3 * (steps - 1) + 1);
    verify::verify(p.clone(), &s).unwrap();
    ret(&mut p.functions[0], 4 * steps, 3 * steps);
    reject(&s, p, FailureKind::Uninitialized);
}

#[test]
fn wide_branch_tree_has_many_call_predecessors_and_unique_locals() {
    let tree_blocks: usize = (1 << 14) - 1;
    let leaves = tree_blocks.div_ceil(2);
    let (s, mut p) = fixture(tree_blocks + 1, tree_blocks + leaves + 1);
    let f = &mut p.functions[0];
    let span = f.span;
    for b in 0..tree_blocks {
        assign(
            f,
            b,
            b + 1,
            Rvalue::Copy(op(if b == 0 { 0 } else { (b - 1) / 2 + 1 }, span)),
        );
        if b < tree_blocks / 2 {
            branch(f, b, 2 * b + 1, 2 * b + 2);
        } else {
            call(f, b, tree_blocks + 1 + b - tree_blocks / 2, tree_blocks);
        }
    }
    verify::verify(p.clone(), &s).unwrap();
    ret(&mut p.functions[0], tree_blocks, tree_blocks + 1);
    reject(&s, p, FailureKind::Uninitialized);
}

#[test]
fn exact_expanded_block_limit_is_accepted_and_plus_one_rejected() {
    let (s, mut p) = fixture(MAX_BLOCKS, 1);
    let f = &mut p.functions[0];
    for b in 0..MAX_BLOCKS - 1 {
        goto(f, b, b + 1);
    }
    verify::verify(p.clone(), &s).unwrap();
    let last = p.functions[0].blocks.last().unwrap().clone();
    p.functions[0].blocks.push(last);
    reject(&s, p, FailureKind::ResourceLimit("blocks"));
}

#[test]
fn scratch_dimensions_are_checked_at_zero_powers_bounds_and_overflow() {
    let (_, p) = fixture(1, 1);
    let span = p.functions[0].span;
    for (blocks, levels) in [
        (0, 0),
        (1, 1),
        (2, 2),
        (3, 2),
        (4, 3),
        (8, 4),
        (MAX_BLOCKS, 19),
    ] {
        let shape = verify::scratch_dimensions(blocks, 2 * blocks, span).unwrap();
        assert_eq!(shape.levels, levels);
        assert_eq!(shape.ancestors, blocks * levels);
        assert_eq!(shape.offsets, blocks + 1);
    }
    for (blocks, edges, name) in [
        (MAX_BLOCKS + 1, 0, "blocks"),
        (usize::MAX, 0, "blocks"),
        (MAX_BLOCKS, 2 * MAX_BLOCKS + 1, "CFG edges"),
        (1, 3, "CFG edges"),
        (1, usize::MAX, "CFG edges"),
    ] {
        assert_eq!(
            verify::scratch_dimensions(blocks, edges, span)
                .unwrap_err()
                .kind,
            FailureKind::ResourceLimit(name)
        );
    }
    assert_eq!(
        verify::checked_product(MAX_BLOCKS, 19, MAX_BLOCKS * 19, "test cells", span).unwrap(),
        MAX_BLOCKS * 19
    );
    for (left, right, limit) in [
        (MAX_BLOCKS, 19, MAX_BLOCKS * 19 - 1),
        (usize::MAX, 2, usize::MAX),
    ] {
        assert_eq!(
            verify::checked_product(left, right, limit, "test cells", span)
                .unwrap_err()
                .kind,
            FailureKind::ResourceLimit("test cells")
        );
    }
}

#[test]
fn exhaustive_merge_incoming_edges_match_independent_path_oracle() {
    let mut comparisons = 0;
    for n in 3..=6 {
        let pairs: Vec<_> = (0..n)
            .flat_map(|a| (a + 1..n).map(move |b| (a, b)))
            .collect();
        for mask in 0..1usize << pairs.len() {
            let mut edges = vec![vec![]; n];
            for (bit, &(a, b)) in pairs.iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    edges[a].push(b);
                }
            }
            if edges.iter().any(|e| e.len() > 2)
                || (0..n).any(|v| !path_exists(&edges, v, None, None))
            {
                continue;
            }
            for join in 1..n {
                let preds: Vec<_> = (0..n).filter(|&p| edges[p].contains(&join)).collect();
                if preds.len() != 2 {
                    continue;
                }
                for side in 0..2 {
                    for definition in 0..n {
                        for call_result in [false, true] {
                            if call_result && edges[definition].len() != 1 {
                                continue;
                            }
                            let pred = preds[side];
                            let expected = if call_result {
                                (definition == pred && edges[definition][0] == join)
                                    || (definition != pred
                                        && !path_exists(
                                            &edges,
                                            pred,
                                            None,
                                            Some((definition, edges[definition][0])),
                                        ))
                            } else {
                                !path_exists(&edges, pred, Some(definition), None)
                            };
                            for reverse in [false, true] {
                                let (sources, mut p) = graph_fixture(&edges);
                                let f = &mut p.functions[0];
                                let span = f.span;
                                if call_result {
                                    call(f, definition, 1, edges[definition][0]);
                                } else {
                                    assign(f, definition, 1, Rvalue::Bool(true));
                                }
                                let mut inputs = [
                                    MergeInput {
                                        predecessor: BlockId(preds[0]),
                                        value: op(0, span),
                                    },
                                    MergeInput {
                                        predecessor: BlockId(preds[1]),
                                        value: op(0, span),
                                    },
                                ];
                                inputs[side].value = op(1, span);
                                f.blocks[join].merge = Some(BoolMerge {
                                    destination: LocalId(2),
                                    incoming: inputs,
                                    span,
                                    operator_span: span,
                                });
                                if reverse {
                                    permute(&mut p, &(0..n).rev().collect::<Vec<_>>());
                                }
                                let result = verify::verify(p, &sources);
                                assert_eq!(result.is_ok(),expected,"edges={edges:?} join={join} pred={pred} definition={definition} call={call_result} reverse={reverse} result={result:?}");
                                if let Err(e) = result {
                                    assert_eq!(e.kind, FailureKind::Uninitialized);
                                }
                                comparisons += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(comparisons > 20_000, "{comparisons}");
}
