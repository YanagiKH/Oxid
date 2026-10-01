//! Raw cyclic CFG proofs, independent of source lowering and execution.
use super::*;

fn fixture(edges: &[Vec<usize>], locals: usize) -> (SourceMap, Program) {
    let mut sources = SourceMap::new();
    let file = sources.add("cyclic-雪.ox".into(), "éx".into());
    let span = sources.get(file).span(2, 3);
    let mut declarations = vec![
        LocalDecl {
            ty: hir::Ty::Bool,
            kind: LocalKind::Temporary,
            span,
        };
        locals
    ];
    declarations[0].kind = LocalKind::Parameter;
    let blocks = edges
        .iter()
        .map(|targets| BasicBlock {
            span,
            merge: None,
            statements: vec![],
            terminator: Some(Terminator {
                span,
                kind: match targets.as_slice() {
                    [] => TerminatorKind::Return(op(0, span)),
                    [target] => TerminatorKind::Goto {
                        target: BlockId(*target),
                    },
                    [left, right] => TerminatorKind::Branch {
                        condition: op(0, span),
                        then_block: BlockId(*left),
                        else_block: BlockId(*right),
                    },
                    _ => panic!("bounded out-degree fixture"),
                },
            }),
        })
        .collect();
    (
        sources,
        Program {
            functions: vec![Function {
                id: hir::DefId(0),
                span,
                result: hir::Ty::Bool,
                param_count: 1,
                locals: declarations,
                places: vec![],
                entry: BlockId(0),
                blocks,
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

#[test]
fn reachable_self_loop_and_irreducible_cycle_are_legal() {
    for edges in [
        vec![vec![0]],
        vec![vec![1, 2], vec![2], vec![1]],
        vec![vec![1, 1], vec![0, 0]],
    ] {
        let (sources, program) = fixture(&edges, 1);
        verify(program, &sources).unwrap();
    }
}

fn assign(function: &mut Function, block: usize, destination: usize, value: Rvalue) {
    function.blocks[block]
        .statements
        .push(Statement::Assign(Assign {
            destination: LocalId(destination),
            value,
            span: function.span,
        }));
}

fn call(function: &mut Function, block: usize, destination: usize, continuation: usize) {
    function.blocks[block].terminator = Some(Terminator {
        span: function.span,
        kind: TerminatorKind::Call {
            target: hir::DefId(0),
            args: vec![op(0, function.span)],
            destination: LocalId(destination),
            continuation: BlockId(continuation),
        },
    });
}

fn reject(sources: &SourceMap, program: Program, expected: FailureKind) {
    let error = verify(program, sources).unwrap_err();
    assert_eq!(error.kind, expected);
    assert_eq!(error.stage, "oir-verify");
    assert_eq!(error.diagnostic(sources).code, "E0500");
}

// Independent oracle: reachability after deleting a vertex or a call edge.
// No DFS numbers, semidominators, dominator trees, or production CFG helpers.
fn reachable(
    edges: &[Vec<usize>],
    entry: usize,
    removed_vertex: Option<usize>,
    removed_edge: Option<(usize, usize)>,
) -> Vec<bool> {
    let mut seen = vec![false; edges.len()];
    let mut work = vec![entry];
    while let Some(node) = work.pop() {
        if Some(node) == removed_vertex || seen[node] {
            continue;
        }
        seen[node] = true;
        for &next in &edges[node] {
            if removed_edge != Some((node, next)) {
                work.push(next);
            }
        }
    }
    seen
}

// All simple directed graphs of bounded out-degree, including self edges,
// irreducible regions, and graphs with no exit. Edge order and multiplicity
// receive separate checks below instead of silently removing those cases.
fn graphs(maximum: usize, mut check: impl FnMut(&[Vec<usize>])) -> usize {
    let mut count = 0;
    for size in 1..=maximum {
        let mut choices = vec![vec![]];
        for left in 0..size {
            choices.push(vec![left]);
            for right in left + 1..size {
                choices.push(vec![left, right]);
            }
        }
        for mut encoding in 0..choices.len().pow(size as u32) {
            let mut edges = Vec::with_capacity(size);
            for _ in 0..size {
                edges.push(choices[encoding % choices.len()].clone());
                encoding /= choices.len();
            }
            if reachable(&edges, 0, None, None).iter().all(|&seen| seen) {
                count += 1;
                check(&edges);
            }
        }
    }
    count
}

#[test]
fn exhaustive_arbitrary_cfg_dominance_matches_path_removal() {
    let mut comparisons = 0;
    let graph_count = graphs(4, |edges| {
        let (_, original) = fixture(edges, 1);
        let count = edges.len();
        for rotation in 0..count {
            let mut program = original.clone();
            // Reverse then rotate: every raw block position becomes entry.
            let order: Vec<_> = (0..count)
                .map(|block| (count - 1 - block + rotation) % count)
                .collect();
            super::super::cfg_tests::permute(&mut program, &order);
            let function = &program.functions[0];
            let predecessors = predecessors(function).unwrap();
            let dfs = depth_first(function).unwrap();
            let proof = dominance(function, &predecessors, &dfs).unwrap();
            for definition in 0..count {
                let avoiding = reachable(edges, 0, Some(definition), None);
                for use_block in 0..count {
                    let actual = proof
                        .contains(order[definition], order[use_block], function.span)
                        .unwrap();
                    assert_eq!(
                        actual, !avoiding[use_block],
                        "edges={edges:?}, definition={definition}, use={use_block}, rotation={rotation}"
                    );
                    comparisons += 1;
                }
            }
        }
    });
    assert_eq!(graph_count, 4_829);
    assert!(comparisons > 100_000, "{comparisons}");
}

fn storage(function: &mut Function) -> Place {
    function.places.push(PlaceDecl {
        ty: hir::Ty::Bool,
        span: function.span,
    });
    Place {
        id: PlaceId(0),
        span: function.span,
    }
}

fn initialize(function: &mut Function, block: usize, target: Place) {
    function.blocks[block]
        .statements
        .push(Statement::Initialize {
            place: target,
            value: op(0, function.span),
            span: function.span,
        });
}

fn store(function: &mut Function, block: usize, target: Place) {
    function.blocks[block].statements.push(Statement::Store {
        place: target,
        value: op(0, function.span),
        operator_span: function.span,
        span: function.span,
    });
}

#[test]
fn exhaustive_cyclic_value_call_and_place_reads_match_path_removal() {
    let mut comparisons = 0;
    graphs(3, |edges| {
        let (sources, original) = fixture(edges, 3);
        for definition in 0..edges.len() {
            for use_block in 0..edges.len() {
                for mode in 0..3 {
                    if mode == 1 && edges[definition].len() != 1 {
                        continue;
                    }
                    let expected = if mode == 1 {
                        definition != use_block
                            && !reachable(edges, 0, None, Some((definition, edges[definition][0])))
                                [use_block]
                    } else {
                        !reachable(edges, 0, Some(definition), None)[use_block]
                    };
                    for reverse in [false, true] {
                        let mut program = original.clone();
                        let function = &mut program.functions[0];
                        let span = function.span;
                        match mode {
                            0 => {
                                assign(function, definition, 1, Rvalue::Bool(true));
                                assign(function, use_block, 2, Rvalue::Copy(op(1, span)));
                            }
                            1 => {
                                call(function, definition, 1, edges[definition][0]);
                                assign(function, use_block, 2, Rvalue::Copy(op(1, span)));
                            }
                            _ => {
                                let target = storage(function);
                                initialize(function, definition, target);
                                store(function, use_block, target);
                                assign(function, use_block, 2, Rvalue::Load(target));
                            }
                        }
                        if reverse {
                            super::super::cfg_tests::permute(
                                &mut program,
                                &(0..edges.len()).rev().collect::<Vec<_>>(),
                            );
                        }
                        let result = verify(program, &sources);
                        assert_eq!(result.is_ok(), expected, "edges={edges:?}, definition={definition}, use={use_block}, mode={mode}, reversed={reverse}, result={result:?}");
                        if let Err(error) = result {
                            assert_eq!(error.kind, FailureKind::Uninitialized);
                        }
                        comparisons += 1;
                    }
                }
            }
        }
    });
    assert!(comparisons > 5_000, "{comparisons}");
}

fn merge(function: &mut Function, block: usize, predecessors: [usize; 2]) {
    function.blocks[block].merge = Some(BoolMerge {
        destination: LocalId(2),
        incoming: predecessors.map(|predecessor| MergeInput {
            predecessor: BlockId(predecessor),
            value: op(0, function.span),
        }),
        span: function.span,
        operator_span: function.span,
    });
}

#[test]
fn exhaustive_cyclic_merge_edges_match_path_removal() {
    let mut comparisons = 0;
    graphs(3, |edges| {
        for join in 1..edges.len() {
            let predecessors: Vec<_> = (0..edges.len())
                .filter(|&block| edges[block].contains(&join))
                .collect();
            let [left, right] = predecessors.as_slice() else {
                continue;
            };
            for (side, &pred) in predecessors.iter().enumerate() {
                for definition in 0..edges.len() {
                    for is_call in [false, true] {
                        if is_call && edges[definition].len() != 1 {
                            continue;
                        }
                        let expected = if is_call {
                            (definition == pred && edges[definition][0] == join)
                                || (definition != pred
                                    && !reachable(
                                        edges,
                                        0,
                                        None,
                                        Some((definition, edges[definition][0])),
                                    )[pred])
                        } else {
                            !reachable(edges, 0, Some(definition), None)[pred]
                        };
                        for reverse in [false, true] {
                            let (sources, mut program) = fixture(edges, 3);
                            let function = &mut program.functions[0];
                            let span = function.span;
                            if is_call {
                                call(function, definition, 1, edges[definition][0]);
                            } else {
                                assign(function, definition, 1, Rvalue::Bool(true));
                            }
                            merge(function, join, [*left, *right]);
                            function.blocks[join].merge.as_mut().unwrap().incoming[side].value =
                                op(1, span);
                            if reverse {
                                super::super::cfg_tests::permute(
                                    &mut program,
                                    &(0..edges.len()).rev().collect::<Vec<_>>(),
                                );
                            }
                            let result = verify(program, &sources);
                            assert_eq!(result.is_ok(), expected, "edges={edges:?}, join={join}, pred={pred}, definition={definition}, call={is_call}, reversed={reverse}, result={result:?}");
                            if let Err(error) = result {
                                assert_eq!(error.kind, FailureKind::Uninitialized);
                            }
                            comparisons += 1;
                        }
                    }
                }
            }
        }
    });
    assert!(comparisons > 1_000, "{comparisons}");
}

#[test]
fn larger_cyclic_graphs_and_successor_order_match_path_removal() {
    let mut seed = 0x5a17_69d3u32;
    for _ in 0..128 {
        let size = 64;
        let mut edges: Vec<_> = (0..size)
            .map(|block| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                vec![(block + 1) % size, (seed >> 16) as usize % size]
            })
            .collect();
        for reversed in [false, true] {
            if reversed {
                for targets in &mut edges {
                    targets.reverse();
                }
            }
            let (_, program) = fixture(&edges, 1);
            let function = &program.functions[0];
            let predecessors = predecessors(function).unwrap();
            let dfs = depth_first(function).unwrap();
            let proof = dominance(function, &predecessors, &dfs).unwrap();
            for definition in 0..size {
                for (use_block, avoiding) in reachable(&edges, 0, Some(definition), None)
                    .into_iter()
                    .enumerate()
                {
                    assert_eq!(
                        proof
                            .contains(definition, use_block, function.span)
                            .unwrap(),
                        !avoiding,
                        "definition={definition}, use={use_block}, edges={edges:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn cyclic_statement_order_and_static_unique_definitions_stay_strict() {
    let edges = [vec![1], vec![1, 2], vec![]];
    let (sources, original) = fixture(&edges, 4);
    for self_read in [false, true] {
        let mut program = original.clone();
        let function = &mut program.functions[0];
        let span = function.span;
        assign(
            function,
            1,
            1,
            Rvalue::Copy(op(if self_read { 1 } else { 2 }, span)),
        );
        assign(function, 1, 2, Rvalue::Bool(true));
        reject(&sources, program, FailureKind::Uninitialized);
    }
    for (left, right) in [(0, 1), (1, 1), (1, 2)] {
        let mut program = original.clone();
        let function = &mut program.functions[0];
        assign(function, left, 1, Rvalue::Bool(true));
        assign(function, right, 1, Rvalue::Bool(false));
        reject(&sources, program, FailureKind::AlreadyInitialized);
    }
    let mut program = original.clone();
    assign(&mut program.functions[0], 1, 0, Rvalue::Bool(true));
    reject(&sources, program, FailureKind::AlreadyInitialized);

    let mut program = original.clone();
    let function = &mut program.functions[0];
    assign(function, 1, 1, Rvalue::Bool(true));
    call(function, 2, 1, 1);
    reject(&sources, program, FailureKind::AlreadyInitialized);

    let mut program = original;
    let function = &mut program.functions[0];
    merge(function, 1, [0, 1]);
    assign(function, 1, 2, Rvalue::Bool(true));
    reject(&sources, program, FailureKind::AlreadyInitialized);
}

#[test]
fn cyclic_calls_define_only_the_normal_edge() {
    // A self-call result may feed its own incoming merge edge, but it cannot
    // supply statements or arguments before that call's first execution.
    let (sources, mut program) = fixture(&[vec![1], vec![1]], 4);
    let function = &mut program.functions[0];
    let span = function.span;
    call(function, 1, 1, 1);
    merge(function, 1, [0, 1]);
    function.blocks[1].merge.as_mut().unwrap().incoming[1].value = op(1, span);
    assign(function, 1, 3, Rvalue::Copy(op(2, span)));
    verify(program.clone(), &sources).unwrap();

    let mut bad = program.clone();
    bad.functions[0].blocks[1].statements[0]
        .assignment_mut()
        .value = Rvalue::Copy(op(1, span));
    reject(&sources, bad, FailureKind::Uninitialized);
    let mut bad = program;
    if let TerminatorKind::Call { args, .. } =
        &mut bad.functions[0].blocks[1].terminator.as_mut().unwrap().kind
    {
        args[0] = op(1, span);
    }
    reject(&sources, bad, FailureKind::Uninitialized);
}

#[test]
fn cyclic_merge_definitions_and_predecessor_shape_are_exact() {
    let (sources, mut program) = fixture(&[vec![1], vec![1, 2], vec![]], 3);
    let function = &mut program.functions[0];
    let span = function.span;
    merge(function, 1, [0, 1]);
    function.blocks[1].merge.as_mut().unwrap().incoming[1].value = op(2, span);
    function.blocks[2].terminator.as_mut().unwrap().kind = TerminatorKind::Return(op(2, span));
    verify(program.clone(), &sources).unwrap();
    let mut bad = program.clone();
    bad.functions[0].blocks[1].merge.as_mut().unwrap().incoming[0].value = op(2, span);
    reject(&sources, bad, FailureKind::Uninitialized);
    for wrong in [0, 2] {
        let mut bad = program.clone();
        bad.functions[0].blocks[1].merge.as_mut().unwrap().incoming[1].predecessor = BlockId(wrong);
        reject(&sources, bad, FailureKind::InvalidMerge);
    }
    let (sources, mut program) = fixture(&[vec![1, 1], vec![1]], 3);
    merge(&mut program.functions[0], 1, [0, 1]);
    reject(&sources, program, FailureKind::InvalidMerge);
    let (sources, mut program) = fixture(&[vec![0, 1], vec![0]], 3);
    merge(&mut program.functions[0], 0, [0, 1]);
    reject(&sources, program, FailureKind::InvalidMerge);
}

#[test]
fn loop_place_initialization_is_static_unique_and_first_visit_safe() {
    let (sources, original) = fixture(&[vec![1], vec![1, 2], vec![]], 3);
    let mut program = original.clone();
    let function = &mut program.functions[0];
    let target = storage(function);
    initialize(function, 1, target);
    store(function, 1, target);
    assign(function, 1, 1, Rvalue::Load(target));
    assign(function, 2, 2, Rvalue::Load(target));
    verify(program.clone(), &sources).unwrap();

    let mut bad = program.clone();
    bad.functions[0].blocks[1].statements.swap(0, 1);
    reject(&sources, bad, FailureKind::Uninitialized);
    let mut bad = program.clone();
    bad.functions[0].blocks[1].statements.swap(0, 2);
    reject(&sources, bad, FailureKind::Uninitialized);
    let mut bad = program.clone();
    initialize(&mut bad.functions[0], 2, target);
    reject(&sources, bad, FailureKind::AlreadyInitialized);
    let mut bad = program;
    bad.functions[0].blocks[1].statements.remove(0);
    reject(&sources, bad, FailureKind::Uninitialized);

    // A body-only initialization does not cover the zero-iteration exit or
    // the header's first visit, even though it reaches both on a backedge.
    for use_block in [1, 3] {
        let (sources, mut program) = fixture(&[vec![1], vec![2, 3], vec![1], vec![]], 2);
        let function = &mut program.functions[0];
        let target = storage(function);
        initialize(function, 2, target);
        store(function, use_block, target);
        assign(function, use_block, 1, Rvalue::Load(target));
        reject(&sources, program, FailureKind::Uninitialized);
    }
}

#[test]
fn unreachable_cycles_still_require_structural_validity_and_reachability() {
    let (sources, program) = fixture(&[vec![0], vec![1]], 1);
    reject(&sources, program.clone(), FailureKind::Unreachable);
    let mut bad = program.clone();
    bad.functions[0].blocks[1].terminator = None;
    reject(&sources, bad, FailureKind::MissingTerminator);
    let mut bad = program.clone();
    bad.functions[0].blocks[1].terminator.as_mut().unwrap().kind = TerminatorKind::Goto {
        target: BlockId(usize::MAX),
    };
    reject(&sources, bad, FailureKind::InvalidBlock);
    let mut bad = program;
    bad.functions[0].blocks[1].span.start = 1; // Inside the leading UTF-8 scalar.
    reject(&sources, bad, FailureKind::InvalidSpan);
}

#[test]
fn exact_block_limit_cycle_uses_iterative_linear_scratch() {
    // A backedge to block 1 forces an almost MAX_BLOCKS-long COMPRESS path.
    // A backedge only to entry would exercise deep DFS but not deep eval.
    let edges: Vec<_> = (0..MAX_BLOCKS)
        .map(|block| {
            vec![if block + 1 == MAX_BLOCKS {
                1
            } else {
                block + 1
            }]
        })
        .collect();
    let (sources, program) = fixture(&edges, 1);
    verify(program, &sources).unwrap();
}

#[test]
fn deep_irreducible_ladder_needs_no_source_reducibility_or_recursion() {
    let pairs = 30_000;
    let mut edges = Vec::with_capacity(2 * pairs + 1);
    edges.push(vec![1, 2]);
    for pair in 0..pairs {
        let left = 2 * pair + 1;
        let right = left + 1;
        if pair + 1 == pairs {
            edges.push(vec![right, 0]);
            edges.push(vec![left, 0]);
        } else {
            edges.push(vec![right, left + 2]);
            edges.push(vec![left, right + 2]);
        }
    }
    let (sources, mut program) = fixture(&edges, 3);
    let function = &mut program.functions[0];
    let span = function.span;
    assign(function, 0, 1, Rvalue::Bool(true));
    assign(function, 2 * pairs, 2, Rvalue::Copy(op(1, span)));
    verify(program, &sources).unwrap();
}

mod execution {
    use super::*;

    fn executable(edges: &[Vec<usize>], locals: usize, initial: bool) -> (SourceMap, Program) {
        let (sources, mut program) = fixture(edges, locals);
        let function = &mut program.functions[0];
        function.param_count = 0;
        function.locals[0].kind = LocalKind::Temporary;
        assign(function, 0, 0, Rvalue::Bool(initial));
        (sources, program)
    }

    fn branch(function: &mut Function, block: usize, condition: usize, left: usize, right: usize) {
        function.blocks[block].terminator.as_mut().unwrap().kind = TerminatorKind::Branch {
            condition: op(condition, function.span),
            then_block: BlockId(left),
            else_block: BlockId(right),
        };
    }

    fn ret(function: &mut Function, block: usize, value: usize) {
        function.blocks[block].terminator.as_mut().unwrap().kind =
            TerminatorKind::Return(op(value, function.span));
    }

    fn not(function: &mut Function, block: usize, destination: usize, source: usize) {
        assign(
            function,
            block,
            destination,
            Rvalue::NotBool {
                operand: op(source, function.span),
                operator_span: function.span,
            },
        );
    }

    fn helper(program: &mut Program, negate: bool) {
        let span = program.functions[0].span;
        let mut function = Function {
            id: hir::DefId(1),
            span,
            result: hir::Ty::Bool,
            param_count: 1,
            locals: vec![LocalDecl {
                ty: hir::Ty::Bool,
                kind: LocalKind::Parameter,
                span,
            }],
            places: vec![],
            entry: BlockId(0),
            blocks: vec![BasicBlock {
                span,
                merge: None,
                statements: vec![],
                terminator: Some(Terminator {
                    span,
                    kind: TerminatorKind::Return(op(0, span)),
                }),
            }],
        };
        if negate {
            function.locals.push(LocalDecl {
                ty: hir::Ty::Bool,
                kind: LocalKind::Temporary,
                span,
            });
            not(&mut function, 0, 1, 0);
            ret(&mut function, 0, 1);
        }
        program.functions.push(function);
    }

    fn call_helper(
        function: &mut Function,
        block: usize,
        value: usize,
        result: usize,
        next: usize,
    ) {
        call(function, block, result, next);
        if let TerminatorKind::Call { target, args, .. } =
            &mut function.blocks[block].terminator.as_mut().unwrap().kind
        {
            *target = hir::DefId(1);
            *args = vec![op(value, function.span)];
        }
    }

    fn self_merge(own_input: bool) -> (SourceMap, Program) {
        let (sources, mut program) = executable(&[vec![1], vec![1, 2], vec![]], 3, false);
        let function = &mut program.functions[0];
        let span = function.span;
        merge(function, 1, [0, 1]);
        function.blocks[1].merge.as_mut().unwrap().incoming[1].value =
            op(if own_input { 2 } else { 1 }, span);
        // The second visit must take the previous v1 before this instruction
        // overwrites v1. A self-input reads the previous phi slot itself.
        not(function, 1, 1, 2);
        branch(function, 1, 1, 1, 2);
        ret(function, 2, 2);
        (sources, program)
    }

    fn backedge_merge(with_call: bool, overflow: bool) -> (SourceMap, Program) {
        let (sources, mut program) = executable(&[vec![1], vec![3, 2], vec![1], vec![]], 4, false);
        let function = &mut program.functions[0];
        let span = function.span;
        merge(function, 1, [0, 2]);
        function.blocks[1].merge.as_mut().unwrap().incoming[1].value =
            op(if with_call { 3 } else { 1 }, span);
        branch(function, 1, 2, 3, 2);
        not(function, 2, 1, 2);
        if with_call {
            for _ in 0..3 {
                function.locals.push(LocalDecl {
                    ty: hir::Ty::I32,
                    kind: LocalKind::Temporary,
                    span,
                });
            }
            assign(
                function,
                2,
                4,
                Rvalue::I32(if overflow { i32::MAX } else { 1 }),
            );
            assign(function, 2, 5, Rvalue::I32(1));
            assign(
                function,
                2,
                6,
                Rvalue::CheckedI32 {
                    op: hir::ArithmeticOp::Add,
                    left: op(4, span),
                    right: op(5, span),
                    operator_span: span,
                },
            );
            call_helper(function, 2, 1, 3, 1);
        }
        ret(function, 3, 2);
        if with_call {
            helper(&mut program, false);
        }
        (sources, program)
    }

    fn self_call_merge() -> (SourceMap, Program) {
        let (sources, mut program) = executable(&[vec![1], vec![1]], 3, false);
        let function = &mut program.functions[0];
        let span = function.span;
        merge(function, 1, [0, 1]);
        function.blocks[1].merge.as_mut().unwrap().incoming[1].value = op(1, span);
        call_helper(function, 1, 2, 1, 1);
        helper(&mut program, true);
        (sources, program)
    }

    fn normal_call_cycle() -> (SourceMap, Program) {
        let (sources, mut program) = executable(&[vec![1], vec![2], vec![1, 3], vec![]], 4, false);
        let function = &mut program.functions[0];
        let span = function.span;
        let target = storage(function);
        initialize(function, 0, target);
        assign(function, 1, 3, Rvalue::Load(target));
        call_helper(function, 1, 3, 1, 2);
        not(function, 2, 2, 1);
        function.blocks[2].statements.push(Statement::Store {
            place: target,
            value: op(2, span),
            operator_span: span,
            span,
        });
        branch(function, 2, 1, 3, 1);
        ret(function, 3, 1);
        helper(&mut program, false);
        (sources, program)
    }

    fn irreducible(chosen: bool) -> (SourceMap, Program) {
        let (sources, mut program) =
            executable(&[vec![1, 2], vec![2, 3], vec![1, 3], vec![]], 7, chosen);
        let function = &mut program.functions[0];
        let span = function.span;
        let target = storage(function);
        assign(function, 0, 6, Rvalue::Bool(false));
        function.blocks[0].statements.push(Statement::Initialize {
            place: target,
            value: op(6, span),
            span,
        });
        for (block, value, next, other) in [(1, 1, 2, 2), (2, 3, 4, 1)] {
            assign(function, block, value, Rvalue::Load(target));
            not(function, block, next, value);
            function.blocks[block].statements.push(Statement::Store {
                place: target,
                value: op(next, span),
                operator_span: span,
                span,
            });
            branch(function, block, value, 3, other);
        }
        assign(function, 3, 5, Rvalue::Load(target));
        ret(function, 3, 5);
        (sources, program)
    }

    fn cases() -> Vec<(SourceMap, Program, Result<Scalar, RunFailure>)> {
        let mut cases = vec![];
        for (sources, program, expected) in [
            (self_merge(false), Some(Scalar::Bool(true))),
            (self_merge(true), None),
            (backedge_merge(false, false), Some(Scalar::Bool(true))),
            (backedge_merge(true, false), Some(Scalar::Bool(true))),
            (self_call_merge(), None),
            (normal_call_cycle(), Some(Scalar::Bool(true))),
            (irreducible(false), Some(Scalar::Bool(false))),
            (irreducible(true), Some(Scalar::Bool(false))),
        ]
        .map(|((sources, program), result)| {
            let expected = result.ok_or(RunFailure::Fuel(program.functions[0].span));
            (sources, program, expected)
        }) {
            cases.push((sources, program, expected));
        }
        let (sources, program) = backedge_merge(true, true);
        let span = program.functions[0].span;
        cases.push((sources, program, Err(RunFailure::Overflow(span))));
        cases
    }

    #[test]
    fn raw_cyclic_execution_preserves_phi_edges_and_overwrite_order() {
        for (sources, original, expected) in cases() {
            for reversed in [false, true] {
                let mut program = original.clone();
                if reversed {
                    let count = program.functions[0].blocks.len();
                    super::super::super::cfg_tests::permute(
                        &mut program,
                        &(0..count).rev().collect::<Vec<_>>(),
                    );
                    assert_ne!(program.functions[0].entry, BlockId(0));
                }
                assert_eq!(
                    verify(program, &sources).unwrap().run(Some(hir::DefId(0))),
                    expected
                );
            }
        }
    }

    struct FuelCase {
        sources: SourceMap,
        program: Program,
        steps: Vec<(usize, Span)>,
        overflow_after: Option<usize>,
    }
    impl FuelCase {
        fn expected(&self, mut fuel: usize) -> Result<Scalar, RunFailure> {
            for (index, &(cost, span)) in self.steps.iter().enumerate() {
                if fuel < cost {
                    return Err(RunFailure::Fuel(span));
                }
                fuel -= cost;
                if self.overflow_after == Some(index) {
                    return Err(RunFailure::Overflow(span));
                }
            }
            Ok(Scalar::Bool(true))
        }
    }

    fn fuel_case(with_call: bool, overflow: bool) -> FuelCase {
        let (mut sources, mut program) = if with_call {
            backedge_merge(true, overflow)
        } else {
            self_merge(false)
        };
        let file = sources.add("fuel-雪.ox".into(), "abcdefghijkl".into());
        let spans: Vec<_> = (0..12).map(|i| sources.get(file).span(i, i + 1)).collect();
        let function = &mut program.functions[0];
        function.span = spans[0];
        function.blocks[0].statements[0].assignment_mut().span = spans[1];
        function.blocks[0].terminator.as_mut().unwrap().span = spans[2];
        function.blocks[1].merge.as_mut().unwrap().span = spans[3];
        let costs = if with_call {
            function.blocks[1].terminator.as_mut().unwrap().span = spans[4];
            for (index, instruction) in function.blocks[2].statements.iter_mut().enumerate() {
                let assignment = instruction.assignment_mut();
                assignment.span = spans[5 + index];
                if let Rvalue::CheckedI32 { operator_span, .. } = &mut assignment.value {
                    *operator_span = spans[5 + index];
                }
            }
            function.blocks[2].terminator.as_mut().unwrap().span = spans[9];
            function.blocks[3].terminator.as_mut().unwrap().span = spans[11];
            program.functions[1].blocks[0]
                .terminator
                .as_mut()
                .unwrap()
                .span = spans[10];
            // Main has 7 slots; call atomically pays 1 + 1 argument + 1 callee
            // slot. Returning from the helper costs its own terminator unit.
            vec![
                (8, 0),
                (1, 1),
                (1, 2),
                (1, 3),
                (1, 4),
                (1, 5),
                (1, 6),
                (1, 7),
                (1, 8),
                (3, 9),
                (1, 10),
                (1, 3),
                (1, 4),
                (1, 11),
            ]
        } else {
            function.blocks[1].statements[0].assignment_mut().span = spans[4];
            function.blocks[1].terminator.as_mut().unwrap().span = spans[5];
            function.blocks[2].terminator.as_mut().unwrap().span = spans[6];
            // Root: 1 + 3 slots; then init, goto, two (merge, not, branch)
            // iterations, and return. Exactly 13 abstract-machine units.
            vec![
                (4, 0),
                (1, 1),
                (1, 2),
                (1, 3),
                (1, 4),
                (1, 5),
                (1, 3),
                (1, 4),
                (1, 5),
                (1, 6),
            ]
        };
        FuelCase {
            sources,
            program,
            steps: costs
                .into_iter()
                .map(|(cost, index)| (cost, spans[index]))
                .collect(),
            overflow_after: overflow.then_some(8),
        }
    }

    #[test]
    fn raw_cyclic_reference_fuel_cutoffs_match_hand_counted_events() {
        for (with_call, exact) in [(false, 13), (true, 23)] {
            let case = fuel_case(with_call, false);
            assert_eq!(
                case.steps.iter().map(|&(cost, _)| cost).sum::<usize>(),
                exact
            );
            let verified = verify(case.program.clone(), &case.sources).unwrap();
            for fuel in 0..=exact {
                assert_eq!(
                    super::super::super::execute::run_with_fuel(&verified, hir::DefId(0), fuel),
                    case.expected(fuel),
                    "with_call={with_call}, fuel={fuel}"
                );
            }
            assert!(matches!(case.expected(exact - 1), Err(RunFailure::Fuel(_))));
            assert_eq!(case.expected(exact), Ok(Scalar::Bool(true)));
        }
        let case = fuel_case(true, true);
        let verified = verify(case.program.clone(), &case.sources).unwrap();
        for fuel in [15, 16] {
            assert_eq!(
                super::super::super::execute::run_with_fuel(&verified, hir::DefId(0), fuel),
                case.expected(fuel)
            );
        }
        assert!(matches!(case.expected(15), Err(RunFailure::Fuel(_))));
        assert!(matches!(case.expected(16), Err(RunFailure::Overflow(_))));
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    #[ignore = "requires pinned LLVM 19.1.7; explicitly run in the native CI job"]
    fn raw_cyclic_witnesses_use_real_llvm() {
        struct Scratch(std::path::PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("oxid-raw-cycles-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let scratch = Scratch(root);
        let mut artifacts = 0;
        let mut invalid_modules = 0;
        let mut run = |module: &str, expected: &Result<Scalar, RunFailure>, sources: &SourceMap| {
            let output = scratch.0.join(format!("artifact-{artifacts}"));
            crate::frontend::native::compile(module, output.to_str().unwrap()).unwrap();
            let result = std::process::Command::new(output)
                .env_clear()
                .env("PATH", scratch.0.join("no-tools"))
                .output()
                .unwrap();
            match expected {
                Ok(value) => {
                    assert_eq!(result.status.code(), Some(0));
                    assert!(result.stderr.is_empty());
                    assert_eq!(result.stdout, format!("{value}\n").as_bytes());
                }
                Err(error) => {
                    assert_eq!(result.status.code(), Some(1));
                    assert!(result.stdout.is_empty());
                    assert_eq!(
                        result.stderr,
                        error.diagnostic(sources).render_human(sources).as_bytes()
                    );
                }
            }
            artifacts += 1;
        };
        for (sources, original, expected) in cases() {
            for reversed in [false, true] {
                let mut program = original.clone();
                if reversed {
                    let count = program.functions[0].blocks.len();
                    super::super::super::cfg_tests::permute(
                        &mut program,
                        &(0..count).rev().collect::<Vec<_>>(),
                    );
                    assert_ne!(program.functions[0].entry, BlockId(0));
                }
                let verified = verify(program, &sources).unwrap();
                assert_eq!(verified.run(Some(hir::DefId(0))), expected);
                let module = verified
                    .native_module(Some(hir::DefId(0)), &sources)
                    .unwrap();
                run(&module, &expected, &sources);
                if original.functions[0].locals.len() == 7 && original.functions.len() == 2 {
                    let predecessor = if reversed { 1 } else { 2 };
                    let correct = format!("[ %v3, %g{predecessor}_5_ok ]");
                    let wrong = module.replace(&correct, &format!("[ %v3, %b{predecessor} ]"));
                    assert_ne!(wrong, module);
                    let output = scratch.0.join(format!("invalid-{invalid_modules}"));
                    let error = crate::frontend::native::compile(&wrong, output.to_str().unwrap())
                        .unwrap_err();
                    assert_eq!(error.code, "E0701");
                    assert!(error.message.contains("PHI"), "{}", error.message);
                    assert!(!output.exists());
                    invalid_modules += 1;
                }
            }
        }
        // Every cutoff, including merge/call units, is independently specified
        // above. This is stronger than merely asking the engines to agree.
        for (with_call, exact) in [(false, 13), (true, 23)] {
            let case = fuel_case(with_call, false);
            assert_eq!(
                case.steps.iter().map(|&(cost, _)| cost).sum::<usize>(),
                exact
            );
            let verified = verify(case.program.clone(), &case.sources).unwrap();
            for fuel in 0..=exact {
                let expected = case.expected(fuel);
                assert_eq!(
                    super::super::super::execute::run_with_fuel(&verified, hir::DefId(0), fuel),
                    expected
                );
                let module = verified
                    .native_module_with_fuel(hir::DefId(0), &case.sources, fuel)
                    .unwrap();
                run(&module, &expected, &case.sources);
            }
        }
        let case = fuel_case(true, true);
        let verified = verify(case.program.clone(), &case.sources).unwrap();
        for fuel in [15, 16] {
            let expected = case.expected(fuel);
            assert_eq!(
                super::super::super::execute::run_with_fuel(&verified, hir::DefId(0), fuel),
                expected
            );
            let module = verified
                .native_module_with_fuel(hir::DefId(0), &case.sources, fuel)
                .unwrap();
            run(&module, &expected, &case.sources);
        }
        assert_eq!(artifacts, 58); // 18 default-fuel shapes + 40 exact-cutoff artifacts.
        assert_eq!(invalid_modules, 4);
    }
}
