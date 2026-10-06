//! Independent C1b parser payload lifetimes, using the existing real allocator observer.
//! These tests never construct an executable/source-admission witness.
use super::source::reviewer_source::{integration_enabled, integration_measured};
use crate::frontend::{
    ast::{ExprKind, Program, StmtKind},
    declaration_index::{collect_closed, IndexLimits, SourceOwner, WorkMeter},
    diagnostic::Diagnostic,
    lexer,
    parser::{parse_enum_candidate_counted, SourceMode, MAX_NODES},
    project::budget::Allocator,
    source::{SourceFile, SourceFileId, SourceMap, SourceView, Span},
};
use std::mem::size_of;

// Qualification storage is admitted before tracking and is not parser payload.
const TRACE_ROWS: usize = 4096;
const SMALL: &str = "enum E{V}fn f()->(){match x{E::V=>{E::V;}}}";
const NESTED: &str = concat!(
    "pub mod child;use crate::child::f as alias;",
    "pub struct R{pub n:i32}pub enum E{V,I(i32),U(())}",
    "pub fn f(p:E,r:&mut R)->(){",
    "let a:[i32;2]=[1,2];",
    "let e:E=!!!!!E::I(0,!!!!!E::I(1,!!!!!true),g(R{n:1},[2,3],&mut *r));",
    "match e{",
    "E::V=>{return;},",
    "E::I(v)=>{crate::child::f(v);match e{",
    "E::V=>{E::I(1,2);},",
    "E::I(w)=>{!!!!!E::I(0,!!!!!false);}",
    "}},",
    "E::U(u)=>{return;}",
    "}}",
);

fn source(text: &str) -> SourceMap {
    let mut sources = SourceMap::new();
    sources.add("enum-parser-lifetimes.ox".into(), text.into());
    sources
}

fn prepared_allocator(fail_at: Option<usize>, rows: usize) -> Allocator {
    let mut allocator = Allocator {
        fail_at,
        ..Allocator::default()
    };
    allocator.observer_trace_bound(rows).unwrap();
    allocator
}

fn payload<T>(values: &Vec<T>) -> usize {
    values.capacity().checked_mul(size_of::<T>()).unwrap()
}

/// Independent traversal of every actual retained AST vector, excluding tokens
/// allocated before measurement. Vec headers are already inside their owners;
/// Program itself is a returned stack value, not a new heap allocation.
fn retained_payload(program: &Program) -> usize {
    let mut bytes = payload(&program.functions)
        + payload(&program.expressions)
        + payload(&program.records)
        + payload(&program.enums)
        + payload(&program.items)
        + payload(&program.modules)
        + payload(&program.paths)
        + payload(&program.path_segments)
        + payload(&program.imports);
    for enumeration in &program.enums {
        bytes += payload(&enumeration.variants);
    }
    for record in &program.records {
        bytes += payload(&record.fields);
    }
    for function in &program.functions {
        bytes += payload(&function.params) + payload(&function.blocks);
        for block in &function.blocks {
            bytes += payload(&block.body);
            for statement in &block.body {
                if let StmtKind::Match { arms, .. } = &statement.kind {
                    bytes += payload(arms);
                }
            }
        }
    }
    for expression in &program.expressions {
        bytes += match &expression.kind {
            ExprKind::Call { args, .. }
            | ExprKind::QualifiedValue {
                args: Some(args), ..
            } => payload(args),
            ExprKind::StructLiteral { fields, .. } => payload(fields),
            ExprKind::ArrayLiteral { elements } => payload(elements),
            _ => 0,
        };
    }
    bytes
}

fn complete_trace(allocator: &Allocator, prepared_capacity: usize) {
    assert!(!allocator.observer_trace_overflow);
    assert_eq!(allocator.trace.len(), allocator.attempts);
    assert_eq!(allocator.trace.capacity(), prepared_capacity);
}

// This is a historical closed-policy check, separate from public admission.
fn closed_semantics(file: &SourceFile, program: &Program) {
    let owner = SourceOwner::original(file, program, SourceView::Single(file)).unwrap();
    let mut index_allocator = Allocator::default();
    let error = collect_closed(
        owner,
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut index_allocator,
    )
    .unwrap_err();
    assert_eq!((error.code, error.stage), ("E0101", "resolve"));
    assert_eq!(error.message, "enum source syntax is unavailable");
    assert_eq!(index_allocator.attempts, 0);
}

#[test]
fn enum_parser_lifecycle_success_retains_only_independently_counted_payload() {
    for (name, text, expected_nodes) in [("small", SMALL, 11), ("nested", NESTED, 98)] {
        let sources = source(text);
        let file = sources.get(SourceFileId(0));
        let tokens = lexer::lex(file).unwrap();
        let token_bytes = payload(&tokens);
        let mut allocator = prepared_allocator(None, TRACE_ROWS);
        let trace_capacity = allocator.trace.capacity();
        let mut storage = Default::default();
        let (result, (calls, live, peak)) = integration_measured(|| {
            parse_enum_candidate_counted(
                file,
                tokens,
                SourceMode::ProjectCandidate,
                MAX_NODES,
                &mut allocator,
                &mut storage,
            )
        });
        let (program, nodes) = result.unwrap();
        complete_trace(&allocator, trace_capacity);
        assert!(allocator.trace.iter().all(|event| event.success));
        assert_eq!(nodes, expected_nodes);
        assert_eq!(payload(&program.tokens), token_bytes);
        assert!(program.belongs_to(file));
        assert!(program.validate_spans_and_ids(|span| file.try_text(span).is_some()));

        let independent_bytes = retained_payload(&program);
        assert_eq!(live, isize::try_from(independent_bytes).unwrap(), "{name}");
        assert_eq!(storage.retained_capacity, independent_bytes, "{name}");
        assert_eq!(storage.scratch_capacity, 0, "{name}");
        assert_eq!(calls, allocator.attempts, "hidden allocation in {name}");
        assert_eq!(calls, storage.growths, "{name}");
        assert!(peak >= live);
        // The existing observer sees requested realloc deltas. This is an upper
        // bound check, not equality, allocator-internal coexistence, or RSS.
        assert!(usize::try_from(peak).unwrap() <= storage.peak_capacity_bound);
        if text == NESTED {
            // Manual fixture inventory: 47 expressions + 9 statements + 5
            // declarations + 2 params + 1 record field + 3 variants + 1 literal
            // field + 5 arms + 24 path segments + 1 borrow = 98 nodes.
            let function = &program.functions[0];
            assert_eq!(program.expressions.len(), 47);
            assert_eq!(function.blocks.len(), 6);
            assert_eq!(function.params.len(), 2);
            assert_eq!(
                function.blocks.iter().map(|b| b.body.len()).sum::<usize>(),
                9
            );
            assert_eq!((program.paths.len(), program.path_segments.len()), (11, 24));
            assert_eq!(program.items.len(), 5);
            for label in [
                "syntax modules",
                "syntax imports",
                "syntax records",
                "syntax record fields",
                "enum declarations",
                "enum declaration variants",
                "syntax functions",
                "syntax parameters",
                "syntax blocks",
                "syntax statements",
                "syntax items",
                "qualified paths",
                "qualified path segments",
                "syntax expressions",
                "syntax expression heights",
                "syntax unary prefixes",
                "qualified value arguments",
                "syntax call arguments",
                "syntax literal fields",
                "array literal elements",
                "enum match arms",
            ] {
                assert!(allocator.trace.iter().any(|e| e.kind == label), "{label}");
            }
        }
        closed_semantics(file, &program);
        println!(
            "enum parser {name}: nodes={nodes}, growths={calls}, retained requested \
             payload={live}, observed logical peak={peak}, conservative \
             old+new bound={}, preexisting tokens={token_bytes}",
            storage.peak_capacity_bound
        );
        drop(program);

        // A distinct parse-and-drop measurement owns a token tape allocated
        // before tracking. Its deallocation is deliberately a negative baseline.
        let tokens = lexer::lex(file).unwrap();
        let token_bytes = payload(&tokens);
        let mut drop_allocator = prepared_allocator(None, TRACE_ROWS);
        let trace_capacity = drop_allocator.trace.capacity();
        let mut drop_storage = Default::default();
        let (nodes, (drop_calls, drop_live, drop_peak)) = integration_measured(|| {
            let (program, nodes) = parse_enum_candidate_counted(
                file,
                tokens,
                SourceMode::ProjectCandidate,
                MAX_NODES,
                &mut drop_allocator,
                &mut drop_storage,
            )
            .unwrap();
            drop(program);
            nodes
        });
        complete_trace(&drop_allocator, trace_capacity);
        assert_eq!(nodes, expected_nodes);
        assert_eq!(drop_calls, drop_allocator.attempts);
        assert_eq!(drop_live, -isize::try_from(token_bytes).unwrap(), "{name}");
        assert!(usize::try_from(drop_peak).unwrap() <= drop_storage.peak_capacity_bound);
        assert!(!integration_enabled());
        println!(
            "enum parser {name} parse-and-drop: calls={drop_calls}, \
             live={drop_live}, expected token baseline=-{token_bytes}"
        );
    }
}

#[derive(Debug)]
struct FailureReceipt {
    diagnostics: usize,
    code: &'static str,
    stage: &'static str,
    primary: Option<Span>,
    allocation_failure_message: bool,
    missing_semicolon_message: bool,
    all_original_spans: bool,
}

fn failure_receipt(file: &SourceFile, diagnostics: &[Diagnostic]) -> FailureReceipt {
    let first = &diagnostics[0];
    FailureReceipt {
        diagnostics: diagnostics.len(),
        code: first.code,
        stage: first.stage,
        primary: first.primary,
        allocation_failure_message: first.message == "syntax storage allocation failed",
        missing_semicolon_message: first.message == "statement requires `;`",
        all_original_spans: diagnostics.iter().all(|error| {
            error
                .primary
                .is_some_and(|span| file.try_text(span).is_some())
        }),
    }
}

#[test]
fn enum_parser_lifecycle_every_reserve_failure_drops_partial_owners_and_scratch() {
    for (name, text) in [("small", SMALL), ("nested", NESTED)] {
        let sources = source(text);
        let file = sources.get(SourceFileId(0));
        // Discover only real bounded-growth attempt ordinals. Expected retained
        // bytes in the success test come from a separate AST-capacity traversal.
        let mut baseline = prepared_allocator(None, TRACE_ROWS);
        let baseline_capacity = baseline.trace.capacity();
        let (program, _) = parse_enum_candidate_counted(
            file,
            lexer::lex(file).unwrap(),
            SourceMode::ProjectCandidate,
            MAX_NODES,
            &mut baseline,
            &mut Default::default(),
        )
        .unwrap();
        drop(program);
        complete_trace(&baseline, baseline_capacity);
        assert!(baseline.attempts > 0);
        assert!(baseline.trace.iter().all(|event| event.success));
        let mut maximum_failure_peak = 0;
        for fail_at in 1..=baseline.attempts {
            let tokens = lexer::lex(file).unwrap();
            let token_bytes = payload(&tokens);
            let mut allocator = prepared_allocator(Some(fail_at), TRACE_ROWS);
            let trace_capacity = allocator.trace.capacity();
            let mut storage = Default::default();
            let (receipt, (_calls, live, peak)) = integration_measured(|| {
                match parse_enum_candidate_counted(
                    file,
                    tokens,
                    SourceMode::ProjectCandidate,
                    MAX_NODES,
                    &mut allocator,
                    &mut storage,
                ) {
                    Ok((program, _)) => {
                        drop(program);
                        None
                    }
                    Err(diagnostics) => {
                        let receipt = failure_receipt(file, &diagnostics);
                        // Strings, boxed errors and the diagnostic Vec must all
                        // be gone before the observer snapshots live payload.
                        drop(diagnostics);
                        Some(receipt)
                    }
                }
            });
            let receipt = receipt.expect("injected reserve failure returned a Program");
            assert_eq!((receipt.code, receipt.stage), ("E0400", "parse"));
            assert!(receipt.allocation_failure_message, "{receipt:?}");
            assert!(receipt.diagnostics > 0 && receipt.all_original_spans);
            assert!(receipt.primary.is_some());
            complete_trace(&allocator, trace_capacity);
            assert!(allocator.attempts >= fail_at);
            assert!(allocator.trace[..fail_at - 1]
                .iter()
                .all(|event| event.success));
            let failed = &allocator.trace[fail_at - 1];
            let expected = &baseline.trace[fail_at - 1];
            assert_eq!(
                (
                    failed.kind,
                    failed.length,
                    failed.element_bytes,
                    failed.success
                ),
                (
                    expected.kind,
                    expected.length,
                    expected.element_bytes,
                    false
                ),
                "{name} attempt {fail_at}"
            );
            assert_eq!(
                allocator
                    .trace
                    .iter()
                    .filter(|event| !event.success)
                    .count(),
                1
            );
            assert_eq!(
                live,
                -isize::try_from(token_bytes).unwrap(),
                "leak in {name} attempt {fail_at} ({})",
                failed.kind
            );
            // Failure diagnostics allocate outside the parser vector ledger.
            // Do not compare these observed peaks/calls to its success bound.
            maximum_failure_peak = maximum_failure_peak.max(peak);
            assert!(!integration_enabled());
        }
        println!(
            "enum parser {name}: swept {} real capacity-overflow failure positions; \
             all partial owners/diagnostics dropped to negative token baseline; \
             maximum observed failure peak={maximum_failure_peak}",
            baseline.attempts
        );
    }
}

#[test]
fn enum_parser_lifecycle_syntax_failure_and_trace_exhaustion_are_distinct() {
    // Missing punctuation occurs in a later nested arm after outer arms,
    // arguments, expressions and concurrent prefix stacks have allocated.
    let malformed = NESTED.replace("!!!!!E::I(0,!!!!!false);", "!!!!!E::I(0,!!!!!false)");
    assert_ne!(malformed, NESTED);
    let sources = source(&malformed);
    let file = sources.get(SourceFileId(0));
    let tokens = lexer::lex(file).unwrap();
    let token_bytes = payload(&tokens);
    let mut allocator = prepared_allocator(None, TRACE_ROWS);
    let trace_capacity = allocator.trace.capacity();
    let mut storage = Default::default();
    let (receipt, (_calls, live, peak)) =
        integration_measured(|| {
            match parse_enum_candidate_counted(
                file,
                tokens,
                SourceMode::ProjectCandidate,
                MAX_NODES,
                &mut allocator,
                &mut storage,
            ) {
                Ok((program, _)) => {
                    drop(program);
                    None
                }
                Err(diagnostics) => {
                    let receipt = failure_receipt(file, &diagnostics);
                    drop(diagnostics);
                    Some(receipt)
                }
            }
        });
    let receipt = receipt.expect("missing semicolon parsed successfully");
    assert_eq!((receipt.code, receipt.stage), ("E0100", "parse"));
    assert!(receipt.missing_semicolon_message && receipt.all_original_spans);
    assert_eq!(file.text_at(receipt.primary.unwrap()), "}");
    complete_trace(&allocator, trace_capacity);
    assert!(allocator.trace.iter().all(|event| event.success));
    assert!(allocator.attempts > 0);
    assert_eq!(live, -isize::try_from(token_bytes).unwrap());
    println!(
        "enum parser late syntax failure: growths={}, live={live}, \
         expected token baseline=-{token_bytes}, observed peak={peak}",
        allocator.attempts
    );

    let sources = source(NESTED);
    let file = sources.get(SourceFileId(0));
    let mut baseline = prepared_allocator(None, TRACE_ROWS);
    let baseline_capacity = baseline.trace.capacity();
    let (program, _) = parse_enum_candidate_counted(
        file,
        lexer::lex(file).unwrap(),
        SourceMode::ProjectCandidate,
        MAX_NODES,
        &mut baseline,
        &mut Default::default(),
    )
    .unwrap();
    drop(program);
    complete_trace(&baseline, baseline_capacity);
    let attempts = baseline.attempts;
    assert!(attempts > 1);
    for rows in [0, attempts - 1, attempts] {
        let tokens = lexer::lex(file).unwrap();
        let mut allocator = prepared_allocator(None, rows);
        let trace_capacity = allocator.trace.capacity();
        let mut storage = Default::default();
        let (result, (calls, live, peak)) = integration_measured(|| {
            parse_enum_candidate_counted(
                file,
                tokens,
                SourceMode::ProjectCandidate,
                MAX_NODES,
                &mut allocator,
                &mut storage,
            )
        });
        let (program, nodes) = result.unwrap();
        assert_eq!(nodes, 98);
        assert_eq!(allocator.attempts, attempts);
        assert_eq!(calls, attempts, "trace growth contaminated measurement");
        assert_eq!(allocator.trace.capacity(), trace_capacity);
        assert_eq!(allocator.trace.len(), rows);
        assert_eq!(allocator.observer_trace_overflow, rows < attempts);
        assert!(allocator.trace.iter().all(|event| event.success));
        assert_eq!(live, isize::try_from(retained_payload(&program)).unwrap());
        assert!(usize::try_from(peak).unwrap() <= storage.peak_capacity_bound);
        // Overflow concerns qualification evidence only; it is not a parser
        // allocation failure and the truncated rows cannot support a sweep.
        println!(
            "enum parser trace rows={rows}/{attempts}, overflow={}, retained={live}",
            allocator.observer_trace_overflow
        );
        drop(program);
    }
    let mut trace_failure = Allocator::default();
    assert!(trace_failure
        .observer_trace_bound_capacity_failure(TRACE_ROWS)
        .is_err());
    assert_eq!(trace_failure.attempts, 0);
    assert!(trace_failure.trace.is_empty());
    assert_eq!(trace_failure.observer_trace_limit, None);
    assert!(!trace_failure.observer_trace_overflow);
    assert!(!integration_enabled());
}
