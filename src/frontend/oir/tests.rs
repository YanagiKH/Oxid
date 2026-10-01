use super::*;
use crate::frontend::{lexer, parser};
fn checked(text: &str) -> (SourceMap, typeck::TypedProgram) {
    let mut sources = SourceMap::new();
    let id = sources.add("input.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    (sources, typed)
}
fn minimal() -> (SourceMap, Program) {
    let mut sources = SourceMap::new();
    let id = sources.add("test.ox".into(), "éx".into());
    let span = sources.get(id).span(2, 3);
    let operand = Operand {
        local: LocalId(0),
        span,
    };
    (
        sources,
        Program {
            functions: vec![Function {
                id: hir::DefId(0),
                span,
                result: hir::Ty::Bool,
                param_count: 1,
                locals: vec![LocalDecl {
                    ty: hir::Ty::Bool,
                    kind: LocalKind::Parameter,
                    span,
                }],
                entry: BlockId(0),
                blocks: vec![BasicBlock {
                    span,
                    statements: vec![],
                    terminator: Some(Terminator {
                        kind: TerminatorKind::Return(operand),
                        span,
                    }),
                }],
            }],
        },
    )
}
#[test]
fn actual_typed_program_reaches_verified_oir() {
    let (sources, typed) = checked("fn identity(x: bool) -> bool { return x; }");
    let verified = lower_and_verify(&typed, &sources).unwrap();
    assert_eq!(verified.function_count(), 1);
    assert_eq!(verified.program.functions[0].blocks.len(), 1);
    assert_eq!(verified.program.functions[0].locals.len(), 2);
}
#[test]
fn missing_terminator_is_rejected() {
    let (sources, mut program) = minimal();
    program.functions[0].blocks[0].terminator = None;
    assert_eq!(
        verify::verify(program, &sources).unwrap_err().kind,
        FailureKind::MissingTerminator
    );
}
fn call_program() -> (SourceMap, Program) {
    let (sources, mut program) = minimal();
    let f = &mut program.functions[0];
    let span = f.span;
    f.locals.push(LocalDecl {
        ty: hir::Ty::Bool,
        kind: LocalKind::Temporary,
        span,
    });
    f.blocks[0].terminator = Some(Terminator {
        span,
        kind: TerminatorKind::Call {
            target: hir::DefId(0),
            args: vec![Operand {
                local: LocalId(0),
                span,
            }],
            destination: LocalId(1),
            continuation: BlockId(1),
        },
    });
    f.blocks.push(BasicBlock {
        span,
        statements: vec![],
        terminator: Some(Terminator {
            span,
            kind: TerminatorKind::Return(Operand {
                local: LocalId(1),
                span,
            }),
        }),
    });
    (sources, program)
}
fn reject(sources: &SourceMap, program: Program, kind: FailureKind) {
    let error = verify::verify(program, sources).unwrap_err();
    assert_eq!(error.kind, kind);
    assert_eq!(error.stage, "oir-verify");
    let diagnostic = error.diagnostic(sources);
    assert_eq!(diagnostic.code, "E0500");
    assert!(diagnostic
        .primary
        .is_none_or(|span| sources.is_valid_span(span)));
    assert!(diagnostic
        .render_json(sources)
        .contains("internal compiler error"));
    assert!(diagnostic
        .render_human(sources)
        .contains("internal compiler error"));
}
macro_rules! malformed {
    ($name:ident, $fixture:ident, $kind:expr, |$p:ident| $body:block) => {
        #[test]
        fn $name() {
            let (sources, mut $p) = $fixture();
            $body
            reject(&sources, $p, $kind);
        }
    };
}
malformed!(
    out_of_range_function_id,
    minimal,
    FailureKind::InvalidFunctionId,
    |p| {
        p.functions[0].id = hir::DefId(usize::MAX);
    }
);
malformed!(
    duplicate_function_id,
    minimal,
    FailureKind::InvalidFunctionId,
    |p| {
        p.functions.push(p.functions[0].clone());
    }
);
malformed!(
    out_of_order_function_ids,
    minimal,
    FailureKind::InvalidFunctionId,
    |p| {
        let mut f = p.functions[0].clone();
        f.id = hir::DefId(1);
        p.functions.push(f);
        p.functions.swap(0, 1);
    }
);
malformed!(
    parameter_count_out_of_range,
    minimal,
    FailureKind::ParameterCount,
    |p| {
        p.functions[0].param_count = 2;
    }
);
malformed!(
    parameter_limit_exceeded,
    minimal,
    FailureKind::ParameterCount,
    |p| {
        let f = &mut p.functions[0];
        f.locals = vec![f.locals[0].clone(); parser::MAX_PARAMS + 1];
        f.param_count = parser::MAX_PARAMS + 1;
    }
);
malformed!(
    wrong_parameter_prefix,
    minimal,
    FailureKind::ParameterKind,
    |p| {
        p.functions[0].locals[0].kind = LocalKind::Binding;
    }
);
malformed!(
    parameter_outside_prefix,
    minimal,
    FailureKind::ParameterKind,
    |p| {
        p.functions[0].param_count = 0;
    }
);
malformed!(empty_block_table, minimal, FailureKind::NoBlocks, |p| {
    p.functions[0].blocks.clear();
});
malformed!(invalid_entry, minimal, FailureKind::InvalidBlock, |p| {
    p.functions[0].entry = BlockId(usize::MAX);
});
malformed!(
    invalid_continuation,
    call_program,
    FailureKind::InvalidBlock,
    |p| {
        if let Some(Terminator {
            kind: TerminatorKind::Call { continuation, .. },
            ..
        }) = &mut p.functions[0].blocks[0].terminator
        {
            *continuation = BlockId(usize::MAX);
        }
    }
);
malformed!(
    invalid_return_local,
    minimal,
    FailureKind::InvalidLocal,
    |p| {
        if let Some(Terminator {
            kind: TerminatorKind::Return(op),
            ..
        }) = &mut p.functions[0].blocks[0].terminator
        {
            op.local = LocalId(usize::MAX);
        }
    }
);
malformed!(
    invalid_call_target,
    call_program,
    FailureKind::InvalidTarget,
    |p| {
        if let Some(Terminator {
            kind: TerminatorKind::Call { target, .. },
            ..
        }) = &mut p.functions[0].blocks[0].terminator
        {
            *target = hir::DefId(usize::MAX);
        }
    }
);
malformed!(
    invalid_call_argument,
    call_program,
    FailureKind::InvalidLocal,
    |p| {
        if let Some(Terminator {
            kind: TerminatorKind::Call { args, .. },
            ..
        }) = &mut p.functions[0].blocks[0].terminator
        {
            args[0].local = LocalId(usize::MAX);
        }
    }
);
malformed!(
    invalid_call_destination,
    call_program,
    FailureKind::InvalidLocal,
    |p| {
        if let Some(Terminator {
            kind: TerminatorKind::Call { destination, .. },
            ..
        }) = &mut p.functions[0].blocks[0].terminator
        {
            *destination = LocalId(usize::MAX);
        }
    }
);
malformed!(call_arity_mismatch, call_program, FailureKind::Arity, |p| {
    if let Some(Terminator {
        kind: TerminatorKind::Call { args, .. },
        ..
    }) = &mut p.functions[0].blocks[0].terminator
    {
        args.clear();
    }
});
malformed!(
    call_argument_type_mismatch,
    call_program,
    FailureKind::TypeMismatch,
    |p| {
        let f = &mut p.functions[0];
        f.locals.push(LocalDecl {
            ty: hir::Ty::Unit,
            kind: LocalKind::Temporary,
            span: f.span,
        });
        if let Some(Terminator {
            kind: TerminatorKind::Call { args, .. },
            ..
        }) = &mut f.blocks[0].terminator
        {
            args[0].local = LocalId(2);
        }
    }
);
malformed!(
    call_result_type_mismatch,
    call_program,
    FailureKind::TypeMismatch,
    |p| {
        p.functions[0].locals[1].ty = hir::Ty::Unit;
    }
);
malformed!(
    return_type_mismatch,
    minimal,
    FailureKind::TypeMismatch,
    |p| {
        p.functions[0].result = hir::Ty::Unit;
    }
);
malformed!(
    return_before_initialization,
    minimal,
    FailureKind::Uninitialized,
    |p| {
        let f = &mut p.functions[0];
        f.param_count = 0;
        f.locals[0].kind = LocalKind::Temporary;
    }
);
malformed!(
    call_reads_its_uninitialized_destination,
    call_program,
    FailureKind::Uninitialized,
    |p| {
        if let Some(Terminator {
            kind: TerminatorKind::Call { args, .. },
            ..
        }) = &mut p.functions[0].blocks[0].terminator
        {
            args[0].local = LocalId(1);
        }
    }
);
malformed!(
    call_overwrites_parameter,
    call_program,
    FailureKind::AlreadyInitialized,
    |p| {
        if let Some(Terminator {
            kind: TerminatorKind::Call { destination, .. },
            ..
        }) = &mut p.functions[0].blocks[0].terminator
        {
            *destination = LocalId(0);
        }
    }
);
malformed!(self_cycle, call_program, FailureKind::Cycle, |p| {
    if let Some(Terminator {
        kind: TerminatorKind::Call { continuation, .. },
        ..
    }) = &mut p.functions[0].blocks[0].terminator
    {
        *continuation = BlockId(0);
    }
});
malformed!(two_block_cycle, call_program, FailureKind::Cycle, |p| {
    let f = &mut p.functions[0];
    f.locals.push(f.locals[1].clone());
    let mut t = f.blocks[0].terminator.clone().unwrap();
    if let TerminatorKind::Call {
        continuation,
        destination,
        ..
    } = &mut t.kind
    {
        *continuation = BlockId(0);
        *destination = LocalId(2);
    }
    f.blocks[1].terminator = Some(t);
});
malformed!(
    unreachable_valid_block,
    minimal,
    FailureKind::Unreachable,
    |p| {
        let block = p.functions[0].blocks[0].clone();
        p.functions[0].blocks.push(block);
    }
);
malformed!(
    malformed_unreachable_block_checked_first,
    minimal,
    FailureKind::MissingTerminator,
    |p| {
        let mut block = p.functions[0].blocks[0].clone();
        block.terminator = None;
        p.functions[0].blocks.push(block);
    }
);

fn assignment_program() -> (SourceMap, Program) {
    let (sources, mut p) = call_program();
    let f = &mut p.functions[0];
    f.blocks[0].statements.push(Assign {
        destination: LocalId(1),
        value: Rvalue::Copy(Operand {
            local: LocalId(0),
            span: f.span,
        }),
        span: f.span,
    });
    f.blocks[0].terminator = f.blocks[1].terminator.clone();
    f.blocks.pop();
    (sources, p)
}
malformed!(
    invalid_assignment_destination,
    assignment_program,
    FailureKind::InvalidLocal,
    |p| {
        p.functions[0].blocks[0].statements[0].destination = LocalId(usize::MAX);
    }
);
malformed!(
    invalid_copy_source,
    assignment_program,
    FailureKind::InvalidLocal,
    |p| {
        if let Rvalue::Copy(op) = &mut p.functions[0].blocks[0].statements[0].value {
            op.local = LocalId(usize::MAX);
        }
    }
);
malformed!(
    copy_type_mismatch,
    assignment_program,
    FailureKind::TypeMismatch,
    |p| {
        p.functions[0].locals[1].ty = hir::Ty::Unit;
    }
);
malformed!(
    unit_into_bool,
    assignment_program,
    FailureKind::TypeMismatch,
    |p| {
        p.functions[0].blocks[0].statements[0].value = Rvalue::Unit;
    }
);
malformed!(
    bool_into_unit,
    assignment_program,
    FailureKind::TypeMismatch,
    |p| {
        let f = &mut p.functions[0];
        f.locals[1].ty = hir::Ty::Unit;
        f.blocks[0].statements[0].value = Rvalue::Bool(true);
    }
);
malformed!(
    self_copy_before_initialization,
    assignment_program,
    FailureKind::Uninitialized,
    |p| {
        if let Rvalue::Copy(op) = &mut p.functions[0].blocks[0].statements[0].value {
            op.local = LocalId(1);
        }
    }
);
malformed!(
    copy_before_initialization,
    assignment_program,
    FailureKind::Uninitialized,
    |p| {
        let f = &mut p.functions[0];
        f.locals.push(f.locals[1].clone());
        if let Rvalue::Copy(op) = &mut f.blocks[0].statements[0].value {
            op.local = LocalId(2);
        }
    }
);
malformed!(
    assignment_overwrites_parameter,
    assignment_program,
    FailureKind::AlreadyInitialized,
    |p| {
        p.functions[0].blocks[0].statements[0].destination = LocalId(0);
    }
);
malformed!(
    duplicate_temporary_initialization,
    assignment_program,
    FailureKind::AlreadyInitialized,
    |p| {
        let f = &mut p.functions[0];
        let assign = f.blocks[0].statements[0].clone();
        f.blocks[0].statements.push(assign);
    }
);
malformed!(
    duplicate_binding_initialization,
    assignment_program,
    FailureKind::AlreadyInitialized,
    |p| {
        let f = &mut p.functions[0];
        f.locals[1].kind = LocalKind::Binding;
        let assign = f.blocks[0].statements[0].clone();
        f.blocks[0].statements.push(assign);
    }
);
malformed!(
    call_overwrites_initialized_temporary,
    call_program,
    FailureKind::AlreadyInitialized,
    |p| {
        let f = &mut p.functions[0];
        f.blocks[0].statements.push(Assign {
            destination: LocalId(1),
            value: Rvalue::Bool(false),
            span: f.span,
        });
    }
);
malformed!(
    assignment_overwrites_call_result,
    call_program,
    FailureKind::AlreadyInitialized,
    |p| {
        let f = &mut p.functions[0];
        f.blocks[1].statements.push(Assign {
            destination: LocalId(1),
            value: Rvalue::Bool(false),
            span: f.span,
        });
    }
);
malformed!(
    needed_definition_in_later_continuation,
    call_program,
    FailureKind::Uninitialized,
    |p| {
        let f = &mut p.functions[0];
        f.locals.push(f.locals[1].clone());
        f.blocks[0].statements.push(Assign {
            destination: LocalId(2),
            value: Rvalue::Copy(Operand {
                local: LocalId(1),
                span: f.span,
            }),
            span: f.span,
        });
    }
);

#[test]
fn invalid_spans_are_rejected_without_rendering_untrusted_offsets() {
    let (sources, base) = minimal();
    let valid = base.functions[0].span;
    for span in [
        Span {
            file: super::super::source::SourceFileId(usize::MAX),
            ..valid
        },
        Span {
            start: 3,
            end: 2,
            ..valid
        },
        Span {
            start: 0,
            end: 4,
            ..valid
        },
        Span {
            start: 1,
            end: 2,
            ..valid
        },
        Span {
            start: 0,
            end: 1,
            ..valid
        },
    ] {
        for location in 0..5 {
            let mut p = base.clone();
            let f = &mut p.functions[0];
            match location {
                0 => f.span = span,
                1 => f.locals[0].span = span,
                2 => f.blocks[0].span = span,
                3 => f.blocks[0].terminator.as_mut().unwrap().span = span,
                _ => {
                    if let TerminatorKind::Return(op) =
                        &mut f.blocks[0].terminator.as_mut().unwrap().kind
                    {
                        op.span = span;
                    }
                }
            }
            reject(&sources, p, FailureKind::InvalidSpan);
        }
        let (_, mut p) = assignment_program();
        p.functions[0].blocks[0].statements[0].span = span;
        reject(&sources, p, FailureKind::InvalidSpan);
        let (_, mut p) = assignment_program();
        if let Rvalue::Copy(op) = &mut p.functions[0].blocks[0].statements[0].value {
            op.span = span;
        }
        reject(&sources, p, FailureKind::InvalidSpan);
    }
}

#[test]
fn nested_distinct_calls_have_exact_left_to_right_continuations() {
    let text = "fn main() -> bool { return combine(left(true), right(false)); }\nfn left(x: bool) -> bool { return x; }\nfn right(x: bool) -> bool { return x; }\nfn combine(x: bool, y: bool) -> bool { return y; }";
    let (sources, typed) = checked(text);
    let verified = lower_and_verify(&typed, &sources).unwrap();
    assert_eq!(verified.function_count(), 4);
    let f = &verified.program.functions[0];
    assert_eq!(f.blocks.len(), 4);
    assert_eq!(f.locals.len(), 5);
    assert_eq!(f.blocks[0].statements[0].destination, LocalId(0));
    assert_eq!(f.blocks[0].statements[0].value, Rvalue::Bool(true));
    assert_eq!(f.blocks[1].statements[0].destination, LocalId(2));
    assert_eq!(f.blocks[1].statements[0].value, Rvalue::Bool(false));
    assert!(f.blocks[2].statements.is_empty());
    assert!(f.blocks[3].statements.is_empty());
    for (block, target, args, destination, continuation, spelling) in [
        (0, 1, vec![0], 1, 1, "left(true)"),
        (1, 2, vec![2], 3, 2, "right(false)"),
        (2, 3, vec![1, 3], 4, 3, "combine(left(true), right(false))"),
    ] {
        let end = f.blocks[block].terminator.as_ref().unwrap();
        assert_eq!(&text[end.span.start..end.span.end], spelling);
        match &end.kind {
            TerminatorKind::Call {
                target: actual_target,
                args: actual_args,
                destination: actual_dest,
                continuation: actual_cont,
            } => {
                assert_eq!(*actual_target, hir::DefId(target));
                assert_eq!(
                    actual_args.iter().map(|op| op.local.0).collect::<Vec<_>>(),
                    args
                );
                assert_eq!(*actual_dest, LocalId(destination));
                assert_eq!(*actual_cont, BlockId(continuation));
                assert_eq!(f.blocks[continuation].span, end.span);
            }
            _ => panic!("expected call"),
        }
    }
    assert!(matches!(
        f.blocks[3].terminator.as_ref().unwrap().kind,
        TerminatorKind::Return(Operand {
            local: LocalId(4),
            ..
        })
    ));
    assert_eq!(
        verified.program,
        lower_and_verify(&typed, &sources).unwrap().program
    );
}

#[test]
fn groups_bindings_uses_and_synthetic_unit_keep_exact_provenance() {
    let text = "// 雪🦀\r\nfn f(x: bool) -> () { let copied: bool = ((x)); sink(copied); return; }\r\nfn sink(value: bool) -> () { value; return (); }";
    let mut sources = SourceMap::new();
    sources.add("unused.ox".into(), "".into());
    let id = sources.add("unicode.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    let result = lower_and_verify(&typed, &sources).unwrap();
    let f = &result.program.functions[0];
    let slice = |span: Span| {
        assert_eq!(span.file, id);
        &text[span.start..span.end]
    };
    assert_eq!(slice(f.span), "f");
    assert_eq!(f.blocks[0].span, f.span);
    assert_eq!(slice(f.locals[0].span), "x");
    assert_eq!(slice(f.locals[1].span), "copied");
    assert_eq!(
        f.locals.iter().map(|l| l.ty).collect::<Vec<_>>(),
        [
            hir::Ty::Bool,
            hir::Ty::Bool,
            hir::Ty::Bool,
            hir::Ty::Bool,
            hir::Ty::Bool,
            hir::Ty::Bool,
            hir::Ty::Unit,
            hir::Ty::Unit
        ]
    );
    assert_eq!(
        f.blocks[0]
            .statements
            .iter()
            .map(|a| slice(a.span))
            .collect::<Vec<_>>(),
        ["x", "(x)", "((x))", "let copied: bool = ((x));", "copied"]
    );
    for (assign, expected) in f.blocks[0]
        .statements
        .iter()
        .zip(["x", "x", "(x)", "((x))", "copied"])
    {
        if let Rvalue::Copy(op) = assign.value {
            assert_eq!(slice(op.span), expected);
        } else {
            panic!("expected copy");
        }
    }
    let end = f.blocks[0].terminator.as_ref().unwrap();
    assert_eq!(slice(end.span), "sink(copied)");
    if let TerminatorKind::Call {
        args, destination, ..
    } = &end.kind
    {
        assert_eq!(slice(args[0].span), "copied");
        assert_eq!(*destination, LocalId(6));
    } else {
        panic!("expected discarded unit call");
    }
    assert_eq!(f.blocks[1].span, end.span);
    let unit = &f.blocks[1].statements[0];
    assert_eq!(unit.value, Rvalue::Unit);
    assert_eq!(slice(unit.span), "return;");
    assert_eq!(slice(f.locals[7].span), "return;");
    let end = f.blocks[1].terminator.as_ref().unwrap();
    assert_eq!(slice(end.span), "return;");
    if let TerminatorKind::Return(op) = end.kind {
        assert_eq!(op.local, LocalId(7));
        assert_eq!(slice(op.span), "return;");
    } else {
        panic!("expected return");
    }
}

#[test]
fn complete_source_subset_keeps_discarded_calls_copies_and_recursion() {
    for (text, count, blocks) in [
        ("", 0, 0),
        ("fn f(x: ()) -> () { let y: () = x; let z = (y); (); z; return z; }", 1, 1),
        ("fn f() -> () { return; } fn g() -> () { return (); }", 2, 2),
        ("fn f(x: bool) -> bool { return f(x); }", 1, 2),
        ("fn f(x: bool) -> bool { return g(x); } fn g(x: bool) -> bool { return f(x); }", 2, 4),
        ("fn f() -> () { let x = id(true); id(x); done(); return; } fn id(x: bool) -> bool { return x; } fn done() -> () { return; }", 3, 6),
    ] {
        let (sources, typed) = checked(text);
        let verified = lower_and_verify(&typed, &sources).unwrap();
        assert_eq!(verified.function_count(), count, "{text}");
        assert_eq!(verified.program.functions.iter().map(|f| f.blocks.len()).sum::<usize>(), blocks, "{text}");
        assert_eq!(verified.program, lower_and_verify(&typed, &sources).unwrap().program);
    }
}

#[test]
fn same_numeric_local_ids_stay_body_local_with_different_types() {
    let (sources, typed) = checked(
        "fn f(x: bool) -> bool { let y = x; return y; } fn g(x: ()) -> () { let y = x; return y; }",
    );
    let verified = lower_and_verify(&typed, &sources).unwrap();
    for (f, ty) in verified
        .program
        .functions
        .iter()
        .zip([hir::Ty::Bool, hir::Ty::Unit])
    {
        assert_eq!(f.locals.len(), 4);
        assert!(f.locals.iter().all(|local| local.ty == ty));
        assert_eq!(f.blocks[0].statements[0].destination, LocalId(2));
        assert_eq!(f.blocks[0].statements[1].destination, LocalId(1));
        assert_eq!(f.blocks[0].statements[2].destination, LocalId(3));
    }
}

#[test]
fn valid_call_result_on_continuation_and_empty_eof_spans_are_accepted() {
    let (sources, p) = call_program();
    assert_eq!(verify::verify(p, &sources).unwrap().function_count(), 1);
    let (sources, mut p) = minimal();
    let f = &mut p.functions[0];
    let eof = Span {
        start: 3,
        end: 3,
        ..f.span
    };
    f.span = eof;
    f.locals[0].span = eof;
    f.blocks[0].span = eof;
    let end = f.blocks[0].terminator.as_mut().unwrap();
    end.span = eof;
    if let TerminatorKind::Return(op) = &mut end.kind {
        op.span = eof;
    }
    assert_eq!(verify::verify(p, &sources).unwrap().function_count(), 1);
}

#[test]
fn bounded_accounting_checks_boundary_plus_one_and_overflow_without_allocation() {
    for (name, limit) in [
        ("locals", MAX_LOCALS),
        ("blocks", MAX_BLOCKS),
        ("assignments", MAX_ASSIGNMENTS),
    ] {
        let mut count = 0;
        Budget::add(&mut count, limit, limit, name, "oir-lower", None).unwrap();
        assert_eq!(count, limit);
        let failure = Budget::add(&mut count, 1, limit, name, "oir-lower", None).unwrap_err();
        assert_eq!(failure.kind, FailureKind::ResourceLimit(name));
        assert_eq!(count, limit);
        assert_eq!(failure.diagnostic(&SourceMap::new()).code, "E0400");
        let mut count = 1;
        assert_eq!(
            Budget::add(&mut count, usize::MAX, limit, name, "oir-verify", None)
                .unwrap_err()
                .kind,
            FailureKind::ResourceLimit(name)
        );
        assert_eq!(count, 1);
    }
}

fn long_chain(calls: usize) -> (SourceMap, Program) {
    let (sources, mut p) = minimal();
    let f = &mut p.functions[0];
    let span = f.span;
    f.locals.extend((0..calls).map(|_| LocalDecl {
        ty: hir::Ty::Bool,
        kind: LocalKind::Temporary,
        span,
    }));
    f.blocks = (0..calls)
        .map(|index| BasicBlock {
            span,
            statements: vec![],
            terminator: Some(Terminator {
                span,
                kind: TerminatorKind::Call {
                    target: hir::DefId(0),
                    args: vec![Operand {
                        local: LocalId(index),
                        span,
                    }],
                    destination: LocalId(index + 1),
                    continuation: BlockId(index + 1),
                },
            }),
        })
        .collect();
    f.blocks.push(BasicBlock {
        span,
        statements: vec![],
        terminator: Some(Terminator {
            span,
            kind: TerminatorKind::Return(Operand {
                local: LocalId(calls),
                span,
            }),
        }),
    });
    (sources, p)
}
#[test]
fn long_chain_is_iterative_and_rejects_late_invalid_reference_and_cycle() {
    let calls = 20_000;
    let (sources, p) = long_chain(calls);
    assert!(verify::verify(p.clone(), &sources).is_ok());
    let mut bad = p.clone();
    if let TerminatorKind::Return(op) = &mut bad.functions[0].blocks[calls]
        .terminator
        .as_mut()
        .unwrap()
        .kind
    {
        op.local = LocalId(usize::MAX);
    }
    reject(&sources, bad, FailureKind::InvalidLocal);
    let mut bad = p;
    let f = &mut bad.functions[0];
    let span = f.span;
    f.locals.push(f.locals[1].clone());
    f.blocks[calls].terminator = Some(Terminator {
        span,
        kind: TerminatorKind::Call {
            target: hir::DefId(0),
            args: vec![Operand {
                local: LocalId(calls),
                span,
            }],
            destination: LocalId(calls + 1),
            continuation: BlockId(calls - 1),
        },
    });
    reject(&sources, bad, FailureKind::Cycle);
}
#[test]
fn raw_aggregate_resource_limits_apply_before_traversal() {
    let (sources, mut p) = minimal();
    let f = &mut p.functions[0];
    let span = f.span;
    f.locals.extend((1..MAX_LOCALS).map(|_| LocalDecl {
        ty: hir::Ty::Bool,
        kind: LocalKind::Temporary,
        span,
    }));
    assert!(verify::verify(p.clone(), &sources).is_ok());
    p.functions[0].locals.push(LocalDecl {
        ty: hir::Ty::Bool,
        kind: LocalKind::Temporary,
        span,
    });
    reject(&sources, p, FailureKind::ResourceLimit("locals"));
    let (sources, p) = long_chain(MAX_LOCALS - 1);
    assert!(verify::verify(p, &sources).is_ok());
    let (_, mut p) = minimal();
    p.functions[0].blocks = vec![p.functions[0].blocks[0].clone(); MAX_BLOCKS + 1];
    reject(&sources, p, FailureKind::ResourceLimit("blocks"));
    let (_, mut p) = minimal();
    p.functions[0].blocks[0].statements = vec![
        Assign {
            destination: LocalId(0),
            value: Rvalue::Bool(true),
            span
        };
        MAX_ASSIGNMENTS + 1
    ];
    reject(&sources, p, FailureKind::ResourceLimit("assignments"));
    let (_, mut p) = call_program();
    if let TerminatorKind::Call { args, .. } =
        &mut p.functions[0].blocks[0].terminator.as_mut().unwrap().kind
    {
        *args = vec![
            Operand {
                local: LocalId(0),
                span
            };
            parser::MAX_PARAMS + 1
        ];
    }
    reject(&sources, p, FailureKind::ResourceLimit("call arguments"));
}

malformed!(
    call_argument_before_initialization,
    call_program,
    FailureKind::Uninitialized,
    |p| {
        let f = &mut p.functions[0];
        f.locals.push(f.locals[1].clone());
        if let TerminatorKind::Call { args, .. } =
            &mut f.blocks[0].terminator.as_mut().unwrap().kind
        {
            args[0].local = LocalId(2);
        }
    }
);
malformed!(
    second_call_overwrites_first_call_result,
    call_program,
    FailureKind::AlreadyInitialized,
    |p| {
        let f = &mut p.functions[0];
        f.blocks.push(f.blocks[1].clone());
        let mut end = f.blocks[0].terminator.clone().unwrap();
        if let TerminatorKind::Call { continuation, .. } = &mut end.kind {
            *continuation = BlockId(2);
        }
        f.blocks[1].terminator = Some(end);
    }
);
malformed!(
    invalid_call_operand_span,
    call_program,
    FailureKind::InvalidSpan,
    |p| {
        if let TerminatorKind::Call { args, .. } =
            &mut p.functions[0].blocks[0].terminator.as_mut().unwrap().kind
        {
            args[0].span.start = 1;
        }
    }
);
#[test]
fn chain_order_is_control_flow_order_not_block_table_order() {
    let (sources, mut p) = call_program();
    let f = &mut p.functions[0];
    f.blocks.swap(0, 1);
    f.entry = BlockId(1);
    if let TerminatorKind::Call { continuation, .. } =
        &mut f.blocks[1].terminator.as_mut().unwrap().kind
    {
        *continuation = BlockId(0);
    }
    assert!(verify::verify(p, &sources).is_ok());
}
#[test]
fn malformed_forward_callee_signature_is_checked_before_call_uses() {
    let (sources, mut p) = call_program();
    let mut callee = p.functions[0].clone();
    callee.id = hir::DefId(1);
    callee.param_count = usize::MAX;
    p.functions.push(callee);
    if let TerminatorKind::Call { target, .. } =
        &mut p.functions[0].blocks[0].terminator.as_mut().unwrap().kind
    {
        *target = hir::DefId(1);
    }
    reject(&sources, p, FailureKind::ParameterCount);
}

#[test]
fn boolean_source_lowers_real_arms_and_a_reachable_join() {
    let (sources, typed) =
        checked("fn f(flag: bool) -> bool { if flag { true; } else { false; } return flag; }");
    let verified = lower_and_verify(&typed, &sources).unwrap();
    assert_eq!(verified.program.functions[0].blocks.len(), 4);
}

#[test]
fn both_returning_arms_have_no_dead_join_and_optional_else_uses_real_join() {
    for (source, block_count, goto_count) in [
        (
            "fn f(flag: bool) -> bool { if flag { return true; } else { return false; } }",
            3,
            0,
        ),
        (
            "fn f(flag: bool) -> bool { if flag { return true; } return false; }",
            3,
            0,
        ),
        (
            "fn f(flag: bool) -> bool { if flag { true; } else { return false; } return flag; }",
            4,
            1,
        ),
        (
            "fn f(flag: bool) -> bool { if flag {} else {} return flag; }",
            4,
            2,
        ),
    ] {
        let (sources, typed) = checked(source);
        let verified = lower_and_verify(&typed, &sources).unwrap();
        let blocks = &verified.program.functions[0].blocks;
        assert_eq!(blocks.len(), block_count, "{source}");
        assert_eq!(
            blocks
                .iter()
                .filter(|b| matches!(
                    b.terminator.as_ref().unwrap().kind,
                    TerminatorKind::Goto { .. }
                ))
                .count(),
            goto_count
        );
        assert!(matches!(
            blocks[0].terminator.as_ref().unwrap().kind,
            TerminatorKind::Branch {
                then_block: BlockId(1),
                else_block: BlockId(2),
                ..
            }
        ));
        assert_eq!(
            verified.program,
            lower_and_verify(&typed, &sources).unwrap().program
        );
    }
}

#[test]
fn condition_arm_and_join_calls_remain_on_exact_paths() {
    let source = "fn f(flag: bool) -> bool { if condition(flag) { let a = left(flag); sink(a); } else { let b = right(flag); sink(b); } return after(flag); } fn condition(x: bool) -> bool { return x; } fn left(x: bool) -> bool { return x; } fn right(x: bool) -> bool { return x; } fn sink(x: bool) -> () { return; } fn after(x: bool) -> bool { return x; }";
    let (sources, typed) = checked(source);
    let p = lower_and_verify(&typed, &sources).unwrap().program;
    let f = &p.functions[0];
    assert_eq!(f.blocks.len(), 10);
    for (block, target, continuation) in [
        (0, 1, 1),
        (2, 2, 5),
        (5, 4, 6),
        (3, 3, 7),
        (7, 4, 8),
        (4, 5, 9),
    ] {
        let end = &f.blocks[block].terminator.as_ref().unwrap().kind;
        assert!(
            matches!(end, TerminatorKind::Call { target: actual_target, continuation: actual_continuation, .. } if actual_target.0 == target && actual_continuation.0 == continuation),
            "block {block}: {end:?}"
        );
    }
    assert!(matches!(
        f.blocks[1].terminator.as_ref().unwrap().kind,
        TerminatorKind::Branch {
            then_block: BlockId(2),
            else_block: BlockId(3),
            ..
        }
    ));
    for block in [6, 8] {
        assert!(matches!(
            f.blocks[block].terminator.as_ref().unwrap().kind,
            TerminatorKind::Goto { target: BlockId(4) }
        ));
        assert!(f.blocks[block].statements.is_empty());
    }
    assert!(matches!(
        f.blocks[9].terminator.as_ref().unwrap().kind,
        TerminatorKind::Return(_)
    ));
    // Reserved join 4 receives real predecessors 6/8, proving table order is
    // neither source expression order nor a topological order assumption.
    assert_eq!(p, lower_and_verify(&typed, &sources).unwrap().program);
}

#[test]
fn branch_origins_preserve_exact_unicode_crlf_source_bytes() {
    let text = "// 雪\r\nfn f(flag: bool) -> bool {\r\n if (flag) { true; } else { false; }\r\n return flag;\r\n}";
    let mut sources = SourceMap::new();
    sources.add("other.ox".into(), "".into());
    let file = sources.add("input.ox".into(), text.into());
    let source = sources.get(file);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    let p = lower_and_verify(&typed, &sources).unwrap().program;
    let f = &p.functions[0];
    let slice = |span: Span| {
        assert_eq!(span.file, file);
        &text[span.start..span.end]
    };
    let branch = f.blocks[0].terminator.as_ref().unwrap();
    assert_eq!(slice(branch.span), "if (flag) { true; } else { false; }");
    if let TerminatorKind::Branch { condition, .. } = branch.kind {
        assert_eq!(slice(condition.span), "(flag)");
    } else {
        panic!("expected Branch")
    }
    assert_eq!(slice(f.blocks[1].span), "{ true; }");
    assert_eq!(slice(f.blocks[2].span), "{ false; }");
    assert_eq!(slice(f.blocks[3].span), slice(branch.span));
    for (block, literal) in [(1, "true"), (2, "false")] {
        assert_eq!(slice(f.blocks[block].statements[0].span), literal);
        let end = f.blocks[block].terminator.as_ref().unwrap();
        assert_eq!(slice(end.span), "}");
        assert!(matches!(
            end.kind,
            TerminatorKind::Goto { target: BlockId(3) }
        ));
        assert_eq!(end.span.start, f.blocks[block].span.end - 1);
    }
    let returned = f.blocks[3].terminator.as_ref().unwrap();
    assert_eq!(slice(returned.span), "return flag;");
    if let TerminatorKind::Return(op) = returned.kind {
        assert_eq!(slice(op.span), "flag");
    } else {
        panic!("expected Return")
    }
}

#[test]
fn nested_branch_lowering_and_disjoint_binding_types_are_complete() {
    let source = "fn f(flag: bool) -> () { if flag { let value = true; if value { return; } else { value; } } else { let value = (); value; } let value = (); return value; }";
    let (sources, typed) = checked(source);
    let p = lower_and_verify(&typed, &sources).unwrap().program;
    let f = &p.functions[0];
    assert_eq!(
        f.locals
            .iter()
            .filter(|local| local.kind == LocalKind::Binding)
            .map(|local| local.ty)
            .collect::<Vec<_>>(),
        [hir::Ty::Bool, hir::Ty::Unit, hir::Ty::Unit]
    );
    assert_eq!(
        f.blocks
            .iter()
            .filter(|b| matches!(
                b.terminator.as_ref().unwrap().kind,
                TerminatorKind::Branch { .. }
            ))
            .count(),
        2
    );
    assert_eq!(
        f.blocks
            .iter()
            .filter(|b| matches!(
                b.terminator.as_ref().unwrap().kind,
                TerminatorKind::Return(_)
            ))
            .count(),
        2
    );
}

#[test]
fn many_source_locals_calls_and_joins_lower_with_one_expression_cursor() {
    let mut source = String::from("fn f(flag: bool) -> bool {");
    for i in 0..1000 {
        source.push_str(&format!(
            "let v{i} = flag; if v{i} {{ sink(v{i}); }} else {{ sink(flag); }}"
        ));
    }
    source.push_str("return flag; } fn sink(x: bool) -> () { return; }");
    let (sources, typed) = checked(&source);
    let p = lower_and_verify(&typed, &sources).unwrap().program;
    assert_eq!(p.functions[0].blocks.len(), 5001);
    assert_eq!(
        p.functions[0]
            .locals
            .iter()
            .filter(|l| l.kind == LocalKind::Binding)
            .count(),
        1000
    );
}
