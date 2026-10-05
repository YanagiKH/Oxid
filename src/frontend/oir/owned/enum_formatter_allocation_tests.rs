//! Independent C1c formatter lifetimes through the existing real heap observer.
//! These syntax-only controls never construct a source/executable witness.
use super::source::reviewer_source::{integration_enabled, integration_measured};
use crate::frontend::{
    ast::{ExprKind, Program, StmtKind},
    diagnostic::Diagnostic,
    format::{format_enum_candidate_observed, EnumFormatMetrics},
    lexer,
    parser::{parse_enum_candidate_counted, SourceMode, MAX_NODES},
    project::budget::{Allocator, ReserveEvent},
    source::{SourceFile, SourceFileId, SourceMap, Span},
};
use std::mem::size_of;

const TRACE_ROWS: usize = 4096;
const SMALL: &str = "enum E{V}fn f(e:E)->(){match e{E::V=>{}}}";
const MIXED: &str = concat!(
    "// λ enum match :: => { fake }\n",
    "pub mod child;use crate::child::f as alias;",
    "pub struct R{pub n:i32}pub enum E{V,I(i32),U(()),B(bool),}",
    "pub fn f(p:E,r:&mut R)->(){",
    "let a:[i32;2]=[1,2];",
    "let e:E=!!!!!E::I(0,!!!!!E::I(1,!!!!!true),g(R{n:1},[2,3],&mut *r));",
    "crate::child::f(1,&*r,2,&mut *r);E::V;E::V();E::U(());",
    "match e{",
    "E::V/* before arrow => */=>{/* empty λ */},",
    "E::I/* before binder */(v)/* after binder */=>{crate::child::f(v);match e{",
    "E::V=>{E::I(1,2);},",
    "E::I(w)=>{!!!!!E::I(0,!!!!!false);}",
    "}},",
    "E::U(u)=>{return;},E::B(b)=>{}",
    "}}",
);
const NESTED: &str = concat!(
    "enum E{V,I(i32)}fn f(e:E)->(){while true{match e{",
    "E::V=>{break;},E::I(v)=>{match e{",
    "E::I(w)=>{continue;},E::V=>{return;}",
    "}}}}}",
);
const PARSER_LABELS: &[&str] = &[
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
];
const FORMAT_LABELS: &[&str] = &[
    "formatter token roles",
    "formatted source",
    "formatted source line starts",
    "formatted source files",
];

fn source(text: &str) -> (SourceMap, SourceFileId) {
    let mut sources = SourceMap::new();
    sources.add("unrelated-first.ox".into(), "// other λ\n".into());
    let id = sources.add("enum-formatter-lifetimes.ox".into(), text.into());
    assert_ne!(id, SourceFileId(0));
    (sources, id)
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

/// Count actual capacities, independently of reservation lengths and metrics.
/// Program is stack-owned. Nested Vec headers are already in enclosing carriers.
fn ast_heap(program: &Program) -> usize {
    let mut bytes = payload(&program.tokens)
        + payload(&program.functions)
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

fn complete_trace(allocator: &Allocator, capacity: usize) {
    assert!(
        !allocator.observer_trace_overflow,
        "truncated trace cannot support evidence"
    );
    assert_eq!(allocator.trace.len(), allocator.attempts);
    assert_eq!(
        allocator.trace.capacity(),
        capacity,
        "observer grew inside measurement"
    );
    assert!(allocator
        .trace
        .iter()
        .all(|event| PARSER_LABELS.contains(&event.kind) || FORMAT_LABELS.contains(&event.kind)));
}

fn same_events(actual: &[ReserveEvent], expected: &[ReserveEvent]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(
            (
                actual.kind,
                actual.length,
                actual.element_bytes,
                actual.success
            ),
            (
                expected.kind,
                expected.length,
                expected.element_bytes,
                expected.success
            )
        );
    }
}

struct ParseEvidence {
    calls: usize,
    heap: usize,
    tokens: usize,
    peak_bound: usize,
    allocator: Allocator,
}

/// A separate real parse establishes both capacity and seam-trace expectations.
/// Its token tape is allocated inside tracking, unlike caller-owned source/trace.
fn independent_parse(file: &SourceFile) -> ParseEvidence {
    let mut allocator = prepared_allocator(None, TRACE_ROWS);
    let trace_capacity = allocator.trace.capacity();
    let mut storage = Default::default();
    let (result, (calls, live, peak)) = integration_measured(|| {
        parse_enum_candidate_counted(
            file,
            lexer::lex(file).unwrap(),
            SourceMode::ProjectCandidate,
            MAX_NODES,
            &mut allocator,
            &mut storage,
        )
    });
    let (program, _) = result.unwrap();
    complete_trace(&allocator, trace_capacity);
    assert!(allocator.trace.iter().all(|event| event.success));
    assert!(program.belongs_to(file));
    assert!(program.validate_spans_and_ids(|span| file.try_text(span).is_some()));
    let heap = ast_heap(&program);
    let tokens = payload(&program.tokens);
    assert_eq!(live, isize::try_from(heap).unwrap());
    assert_eq!(heap, storage.retained_capacity + tokens);
    assert_eq!(storage.scratch_capacity, 0);
    let peak_bound = storage.peak_capacity_bound + tokens;
    assert!(usize::try_from(peak).unwrap() <= peak_bound);
    drop(program);
    assert!(!integration_enabled());
    ParseEvidence {
        calls,
        heap,
        tokens,
        peak_bound,
        allocator,
    }
}

fn check_success_metrics(
    file: &SourceFile,
    output: &str,
    capacity: usize,
    metrics: &EnumFormatMetrics,
    first: &ParseEvidence,
    second: &ParseEvidence,
    observed: (isize, isize),
) {
    let (live, peak) = observed;
    assert_eq!(metrics.parse_calls, 2);
    assert_eq!(
        metrics.input_source_heap,
        file.heap_capacity_bytes().unwrap()
    );
    assert!(metrics.input_source_heap >= file.text().len() + file.path().len());
    assert_eq!(metrics.first_ast_heap, first.heap);
    assert_eq!(metrics.first_token_heap, first.tokens);
    assert_eq!(metrics.first_parse_peak_bound, first.peak_bound);
    assert_eq!(metrics.second_ast_heap, second.heap);
    assert_eq!(metrics.second_token_heap, second.tokens);
    assert_eq!(metrics.second_parse_peak_bound, second.peak_bound);
    assert_eq!(metrics.roles_heap, file.text().len());
    assert_eq!(metrics.output_heap, capacity);
    let line_bytes =
        (output.bytes().filter(|&byte| byte == b'\n').count() + 1) * size_of::<usize>();
    let owner_overhead = size_of::<SourceFile>() + line_bytes;
    // Exactly one singleton SourceFile carrier and one moved text buffer.
    assert_eq!(metrics.source_owner_heap, owner_overhead + capacity);
    assert_eq!(
        metrics.source_owner_heap - metrics.output_heap,
        owner_overhead
    );
    assert_eq!(
        metrics.emit_live_heap,
        first.heap + file.text().len() + capacity
    );
    assert_eq!(
        metrics.reparse_live_heap,
        first.heap + owner_overhead + capacity + second.heap
    );
    let expected_bound = first
        .peak_bound
        .max(metrics.emit_live_heap)
        .max(first.heap + owner_overhead + capacity + second.peak_bound);
    assert_eq!(metrics.phase_peak_heap_bound, expected_bound);
    assert_eq!(
        live,
        isize::try_from(capacity).unwrap(),
        "retained bytes must be returned output only"
    );
    let peak = usize::try_from(peak).unwrap();
    assert!(
        peak >= metrics.emit_live_heap,
        "first AST, roles, and output must coexist"
    );
    assert!(
        peak >= metrics.reparse_live_heap,
        "first AST, source owner, and second AST must coexist"
    );
    // The observer counts requested-layout realloc deltas. This is not physical
    // old/new buffer overlap, allocator metadata, or process RSS.
    assert!(peak <= metrics.phase_peak_heap_bound);
}

#[test]
fn enum_formatter_lifecycle_success_retains_only_output_and_counts_both_parses() {
    for (name, text) in [
        ("small", SMALL),
        ("mixed", MIXED),
        ("nested", NESTED),
        ("empty", ""),
    ] {
        let (sources, id) = source(text);
        let file = sources.get(id);
        let identity = file.identity();
        let source_pointer = file.text().as_ptr();
        let first = independent_parse(file);
        let mut allocator = prepared_allocator(None, TRACE_ROWS);
        let trace_capacity = allocator.trace.capacity();
        let ((result, metrics), (calls, live, peak)) =
            integration_measured(|| format_enum_candidate_observed(file, &mut allocator));
        let output = result.unwrap();
        complete_trace(&allocator, trace_capacity);
        assert!(allocator.trace.iter().all(|event| event.success));
        let (formatted_sources, formatted_id) = source(&output);
        let second = independent_parse(formatted_sources.get(formatted_id));
        check_success_metrics(
            file,
            &output,
            output.capacity(),
            &metrics,
            &first,
            &second,
            (live, peak),
        );
        // Compare independently observed successful allocator-call counts for
        // both complete lexer+parser runs, plus the four formatter buffers.
        // This is deliberately NOT an equality with logical reserve attempts:
        // lexer growth is unseamed, and empty roles/output reserve zero bytes.
        let formatter_calls =
            usize::from(!file.text().is_empty()) + usize::from(output.capacity() != 0) + 2;
        assert_eq!(
            calls,
            first.calls + second.calls + formatter_calls,
            "unexpected formatter-created allocation for {name}"
        );
        let first_end = first.allocator.attempts;
        same_events(&allocator.trace[..first_end], &first.allocator.trace);
        for (offset, label) in FORMAT_LABELS.iter().enumerate() {
            assert_eq!(allocator.trace[first_end + offset].kind, *label);
        }
        same_events(
            &allocator.trace[first_end + FORMAT_LABELS.len()..],
            &second.allocator.trace,
        );
        if text == MIXED {
            for label in PARSER_LABELS {
                assert!(
                    first
                        .allocator
                        .trace
                        .iter()
                        .any(|event| event.kind == *label),
                    "{label}"
                );
                assert!(
                    second
                        .allocator
                        .trace
                        .iter()
                        .any(|event| event.kind == *label),
                    "{label}"
                );
            }
            assert!(output.contains("&*r") && output.contains("&mut *r"));
            assert!(output.contains("/* empty λ */"));
        }
        assert_eq!(file.text(), text);
        assert_eq!(file.text().as_ptr(), source_pointer);
        assert_eq!(file.identity(), identity);
        assert_eq!(file.span(0, file.text().len()).file, id);
        println!("enum formatter {name}: seam attempts={}, real calls={calls}, output-only live={live}, logical peak={peak}, conservative bound={}, external input={}",
            allocator.attempts, metrics.phase_peak_heap_bound, metrics.input_source_heap);
        drop(output);

        // Separate full format-and-drop observes lexer tokens, both ASTs,
        // source-owner line starts, and output all released in the same window.
        let mut drop_allocator = prepared_allocator(None, TRACE_ROWS);
        let trace_capacity = drop_allocator.trace.capacity();
        let (dropped_metrics, (_calls, live, peak)) = integration_measured(|| {
            let (result, metrics) = format_enum_candidate_observed(file, &mut drop_allocator);
            drop(result.unwrap());
            metrics
        });
        complete_trace(&drop_allocator, trace_capacity);
        assert_eq!(live, 0, "format-and-drop leaked for {name}");
        assert_eq!(dropped_metrics.parse_calls, 2);
        assert!(usize::try_from(peak).unwrap() <= dropped_metrics.phase_peak_heap_bound);
        assert_eq!(file.text(), text);
        assert!(!integration_enabled());
    }
}

#[derive(Debug)]
struct FailureReceipt {
    count: usize,
    code: &'static str,
    stage: &'static str,
    primary: Option<Span>,
    expected_message: bool,
    all_original_spans: bool,
    no_spans: bool,
    no_notes: bool,
}

fn failure_receipt(
    file: &SourceFile,
    errors: &[Diagnostic],
    expected_message: &str,
) -> FailureReceipt {
    let first = &errors[0];
    FailureReceipt {
        count: errors.len(),
        code: first.code,
        stage: first.stage,
        primary: first.primary,
        expected_message: first.message == expected_message,
        all_original_spans: errors.iter().all(|error| {
            error
                .primary
                .is_some_and(|span| file.try_text(span).is_some())
                && error
                    .secondary
                    .iter()
                    .all(|(span, _)| file.try_text(*span).is_some())
        }),
        no_spans: errors
            .iter()
            .all(|error| error.primary.is_none() && error.secondary.is_empty()),
        no_notes: errors.iter().all(|error| error.notes.is_empty()),
    }
}

#[test]
fn enum_formatter_lifecycle_every_reserve_failure_drops_both_parses_and_diagnostics() {
    for (name, text) in [
        ("small", SMALL),
        ("mixed", MIXED),
        ("nested", NESTED),
        ("empty", ""),
    ] {
        let (sources, id) = source(text);
        let file = sources.get(id);
        let identity = file.identity();
        let pointer = file.text().as_ptr();
        let mut baseline = prepared_allocator(None, TRACE_ROWS);
        let baseline_capacity = baseline.trace.capacity();
        let (result, metrics) = format_enum_candidate_observed(file, &mut baseline);
        drop(result.unwrap());
        assert_eq!(metrics.parse_calls, 2);
        complete_trace(&baseline, baseline_capacity);
        assert!(baseline.trace.iter().all(|event| event.success));
        let roles = baseline
            .trace
            .iter()
            .position(|event| event.kind == FORMAT_LABELS[0])
            .unwrap();
        for (offset, label) in FORMAT_LABELS.iter().enumerate() {
            assert_eq!(baseline.trace[roles + offset].kind, *label);
        }
        let second_parse = roles + FORMAT_LABELS.len();
        if !text.is_empty() {
            assert!(roles > 0 && second_parse < baseline.attempts);
        }
        let mut maximum_failure_peak = 0;
        for fail_at in 1..=baseline.attempts {
            let ordinal = fail_at - 1;
            let expected = &baseline.trace[ordinal];
            let expected_message = if ordinal < roles {
                "syntax storage allocation failed"
            } else if ordinal == roles {
                "formatter work table allocation failed"
            } else if ordinal < second_parse {
                "formatted source allocation failed"
            } else {
                "formatted source exceeds lexer or parser resource limits"
            };
            let mut allocator = prepared_allocator(Some(fail_at), TRACE_ROWS);
            let trace_capacity = allocator.trace.capacity();
            let ((receipt, failed_metrics), (_calls, live, peak)) = integration_measured(|| {
                let (result, metrics) = format_enum_candidate_observed(file, &mut allocator);
                let receipt = match result {
                    Ok(output) => {
                        drop(output);
                        None
                    }
                    Err(errors) => {
                        let receipt = failure_receipt(file, &errors, expected_message);
                        // Diagnostic Strings, boxes and Vec storage must be gone
                        // before taking the live-payload snapshot.
                        drop(errors);
                        Some(receipt)
                    }
                };
                (receipt, metrics)
            });
            let receipt = receipt.expect("injected real reserve failure returned formatted text");
            assert!(
                receipt.count > 0 && receipt.expected_message,
                "{name} {fail_at}: {receipt:?}"
            );
            assert_eq!(receipt.code, "E0400");
            if ordinal < roles {
                assert_eq!(receipt.stage, "parse");
                assert!(receipt.all_original_spans, "{receipt:?}");
                assert_eq!(receipt.primary.unwrap().file, id);
                assert_eq!(failed_metrics.parse_calls, 1);
            } else {
                assert_eq!(receipt.stage, "format");
                assert!(
                    receipt.no_spans && receipt.no_notes,
                    "dropped-owner diagnostic escaped: {receipt:?}"
                );
                assert_eq!(
                    failed_metrics.parse_calls,
                    if ordinal < second_parse { 1 } else { 2 }
                );
            }
            complete_trace(&allocator, trace_capacity);
            assert!(allocator.attempts >= fail_at);
            same_events(&allocator.trace[..ordinal], &baseline.trace[..ordinal]);
            let failed = &allocator.trace[ordinal];
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
                )
            );
            assert_eq!(
                allocator
                    .trace
                    .iter()
                    .filter(|event| !event.success)
                    .count(),
                1
            );
            // Parser recovery may attempt later reservations. It must still
            // discard everything, and must never reach a successful publish.
            assert_eq!(live, 0, "{name} attempt {fail_at} ({}) leaked", failed.kind);
            assert_eq!(file.text(), text);
            assert_eq!(file.text().as_ptr(), pointer);
            assert_eq!(file.identity(), identity);
            assert!(!integration_enabled());
            // Lexer/diagnostic allocations are outside the seam. Failure peaks
            // and global call counts are not compared with success ledgers.
            maximum_failure_peak = maximum_failure_peak.max(peak);
        }
        println!("enum formatter {name}: swept {} real seam ordinals (first parse {roles}, roles/output/source-owner 4, second parse {}); all failures dropped to zero; maximum logical failure peak={maximum_failure_peak}",
            baseline.attempts, baseline.attempts - second_parse);
    }
}

#[test]
fn enum_formatter_lifecycle_source_owner_moves_the_same_output_buffer_once() {
    // The already allocated buffer is deliberately outside the tracked domain.
    let output = String::from("enum E { V }\n// λ\n");
    assert_eq!(output.capacity(), output.len());
    let expected = output.clone();
    let pointer = output.as_ptr();
    let capacity = output.capacity();
    let line_bytes =
        (output.bytes().filter(|&byte| byte == b'\n').count() + 1) * size_of::<usize>();
    let owner_overhead = size_of::<SourceFile>() + line_bytes;
    let mut allocator = prepared_allocator(None, TRACE_ROWS);
    let trace_capacity = allocator.trace.capacity();
    let ((output, owner_heap, same_pointer), (calls, live, peak)) = integration_measured(|| {
        let mut owner = SourceMap::new();
        let id = owner
            .try_add_format_candidate(output, &mut allocator)
            .unwrap();
        let same_pointer = owner.get(id).text().as_ptr() == pointer;
        let owner_heap = owner.heap_capacity_bytes().unwrap();
        (owner.into_single_text(), owner_heap, same_pointer)
    });
    complete_trace(&allocator, trace_capacity);
    assert_eq!(allocator.attempts, 2);
    assert_eq!(allocator.trace[0].kind, "formatted source line starts");
    assert_eq!(allocator.trace[1].kind, "formatted source files");
    assert!(allocator.trace.iter().all(|event| event.success));
    assert_eq!(
        calls, 2,
        "only the nonempty line and singleton file buffers allocate here"
    );
    assert_eq!(
        live, 0,
        "moving preexisting text must not retain newly allocated bytes"
    );
    assert_eq!(peak, isize::try_from(owner_overhead).unwrap());
    assert_eq!(owner_heap, owner_overhead + capacity);
    assert!(same_pointer);
    assert_eq!(output.as_ptr(), pointer);
    assert_eq!(output.capacity(), capacity);
    assert_eq!(output, expected);
    assert!(!integration_enabled());
}

#[test]
fn enum_formatter_lifecycle_trace_exhaustion_is_explicit_and_never_a_failure_sweep() {
    let (sources, id) = source(MIXED);
    let file = sources.get(id);
    let mut baseline = prepared_allocator(None, TRACE_ROWS);
    let capacity = baseline.trace.capacity();
    let (result, _) = format_enum_candidate_observed(file, &mut baseline);
    let expected = result.unwrap();
    complete_trace(&baseline, capacity);
    let attempts = baseline.attempts;
    assert!(attempts > 4);
    let mut previous_observation = None;
    for rows in [0, attempts - 1, attempts] {
        let mut allocator = prepared_allocator(None, rows);
        let trace_capacity = allocator.trace.capacity();
        let ((result, metrics), (calls, live, peak)) =
            integration_measured(|| format_enum_candidate_observed(file, &mut allocator));
        let output = result.unwrap();
        assert_eq!(output, expected);
        assert_eq!(metrics.parse_calls, 2);
        assert_eq!(allocator.attempts, attempts);
        assert_eq!(allocator.trace.capacity(), trace_capacity);
        assert_eq!(allocator.trace.len(), rows);
        assert_eq!(allocator.observer_trace_overflow, rows < attempts);
        same_events(&allocator.trace, &baseline.trace[..rows]);
        assert_eq!(live, isize::try_from(output.capacity()).unwrap());
        assert!(usize::try_from(peak).unwrap() <= metrics.phase_peak_heap_bound);
        if let Some(previous) = previous_observation {
            assert_eq!(
                (calls, live, peak),
                previous,
                "qualification trace changed observed work"
            );
        }
        previous_observation = Some((calls, live, peak));
        // A truncated trace is valid evidence of overflow only. It is never
        // accepted as a complete reservation inventory or used for injection.
        if rows == attempts {
            complete_trace(&allocator, trace_capacity);
        }
        drop(output);
        assert!(!integration_enabled());
    }
    let mut trace_failure = Allocator::default();
    assert!(trace_failure
        .observer_trace_bound_capacity_failure(TRACE_ROWS)
        .is_err());
    assert_eq!(trace_failure.attempts, 0);
    assert!(trace_failure.trace.is_empty());
    assert_eq!(trace_failure.observer_trace_limit, None);
    assert!(!trace_failure.observer_trace_overflow);
}

#[test]
fn enum_formatter_lifecycle_rejections_release_partial_storage_without_mutating_source() {
    // The nested match braces and ordinary grouping are admitted independently
    // by the parser, but their combined formatter delimiter depth exceeds 128.
    let nested_delimiters = format!(
        "enum E{{V}}fn f()->(){{{}{}1{};{}}}",
        "match e{E::V=>{".repeat(60),
        "(".repeat(16),
        ")".repeat(16),
        "}}".repeat(60)
    );
    let too_many_variants = format!(
        "enum E{{{}}}",
        (0..257)
            .map(|n| format!("V{n}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let missing_semicolon = MIXED.replace("!!!!!E::I(0,!!!!!false);", "!!!!!E::I(0,!!!!!false)");
    assert_ne!(missing_semicolon, MIXED);
    for (name, text, expected_code, expected_stage, expected_message) in [
        (
            "split arrow",
            "enum E{V}fn f(e:E)->(){match e{E::V= >{}}}",
            "E0100",
            "parse",
            "match arm requires contiguous `=>`",
        ),
        (
            "comment-split arrow",
            "enum E{V}fn f(e:E)->(){match e{E::V=/* λ */>{}}}",
            "E0100",
            "parse",
            "match arm requires contiguous `=>`",
        ),
        (
            "late missing semicolon",
            missing_semicolon.as_str(),
            "E0100",
            "parse",
            "statement requires `;`",
        ),
        (
            "candidate variant limit",
            too_many_variants.as_str(),
            "E0400",
            "parse",
            "enum variant limit exceeded (256)",
        ),
        (
            "combined delimiters",
            nested_delimiters.as_str(),
            "E0400",
            "format",
            "formatter delimiter limit exceeded",
        ),
    ] {
        let (sources, id) = source(text);
        let file = sources.get(id);
        let identity = file.identity();
        let pointer = file.text().as_ptr();
        if expected_stage == "format" {
            drop(independent_parse(file));
        }
        let mut allocator = prepared_allocator(None, TRACE_ROWS);
        let capacity = allocator.trace.capacity();
        let ((receipt, metrics), (_calls, live, peak)) = integration_measured(|| {
            let (result, metrics) = format_enum_candidate_observed(file, &mut allocator);
            let receipt = match result {
                Ok(output) => {
                    drop(output);
                    None
                }
                Err(errors) => {
                    let receipt = failure_receipt(file, &errors, expected_message);
                    drop(errors);
                    Some(receipt)
                }
            };
            (receipt, metrics)
        });
        let receipt = receipt.expect("rejected syntax/resource source returned formatted text");
        assert_eq!(
            (receipt.code, receipt.stage),
            (expected_code, expected_stage),
            "{name}: {receipt:?}"
        );
        assert!(receipt.expected_message, "{name}: {receipt:?}");
        assert_eq!(metrics.parse_calls, 1);
        assert_eq!(metrics.output_heap, 0);
        assert_eq!(metrics.source_owner_heap, 0);
        assert_eq!(metrics.second_ast_heap, 0);
        complete_trace(&allocator, capacity);
        assert!(allocator.trace.iter().all(|event| event.success));
        if expected_stage == "parse" {
            assert!(receipt.all_original_spans, "{receipt:?}");
            assert_eq!(receipt.primary.unwrap().file, id);
            assert!(!allocator
                .trace
                .iter()
                .any(|event| FORMAT_LABELS.contains(&event.kind)));
        } else {
            assert!(receipt.no_spans && receipt.no_notes);
            assert_eq!(
                allocator.trace.last().unwrap().kind,
                "formatter token roles"
            );
        }
        assert_eq!(live, 0, "{name} leaked partial owner/diagnostic storage");
        assert_eq!(file.text(), text);
        assert_eq!(file.text().as_ptr(), pointer);
        assert_eq!(file.identity(), identity);
        assert!(!integration_enabled());
        println!("enum formatter rejection {name}: seam attempts={}, zero live after diagnostic drop, logical peak={peak}", allocator.attempts);
    }
}
