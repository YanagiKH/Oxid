use super::super::*;
use super::{budget, lower, resolve, typeck};
use crate::frontend::{lexer, parser, source::SourceMap};

fn with_typed<T>(
    text: &str,
    run: impl FnOnce(&typeck::TypedOwnedProgram<'_>, &SourceMap) -> T,
) -> T {
    let mut sources = SourceMap::new();
    let id = sources.add("source-budget.ox".into(), text.into());
    let file = sources.get(id);
    let ast = parser::parse_with_mode(
        file,
        lexer::lex(file).unwrap(),
        parser::SourceMode::OwnedCandidate,
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve(file, &ast).unwrap()).unwrap();
    run(&typed, &sources)
}

#[test]
fn source_payload_exact_count_and_one_under_precede_every_reservation() {
    with_typed(
        "struct T {} fn main() -> () { let x = T {}; return; }",
        |typed, sources| {
            let usage = budget::fail_allocation_after(0, || {
                budget::preflight(typed, budget::Limits::DEFAULT)
            })
            .unwrap();
            // Independently inventoried: one raw program/record/function, one scalar
            // slot, two owners, one block, seven statements, no nested payloads.
            let expected = std::mem::size_of::<RawOwnedProgram>()
                + std::mem::size_of::<RawRecordDecl>()
                + std::mem::size_of::<RawOwnedFunction>()
                + std::mem::size_of::<LocalDecl>()
                + 2 * std::mem::size_of::<OwnerDecl>()
                + std::mem::size_of::<OwnedBlock>()
                + 7 * std::mem::size_of::<OwnedStatement>();
            assert_eq!(usage.raw_bytes, expected);
            assert_eq!(
                usage.raw_bytes,
                2304 + std::mem::size_of::<Vec<MatchDecl>>()
            );
            assert_eq!(usage.analysis.expanded_events, 7);
            assert_eq!(usage.analysis.work, 552);
            assert_eq!(
                usage.analysis.metadata_bytes,
                568 + std::mem::size_of::<Vec<MatchDecl>>()
            );
            let raw = lower::lower_with_limits(
                typed,
                budget::Limits {
                    raw_bytes: expected,
                },
            )
            .unwrap();
            verified::verify_owned(raw, sources).unwrap();
            let denied = budget::fail_allocation_after(0, || {
                lower::lower_with_limits(
                    typed,
                    budget::Limits {
                        raw_bytes: expected - 1,
                    },
                )
            })
            .unwrap_err();
            assert_eq!(
                denied.kind,
                OwnedFailureKind::Resource("source raw payload")
            );
            assert_eq!(
                budget::fail_allocation_after(0, || lower::lower(typed))
                    .unwrap_err()
                    .kind,
                OwnedFailureKind::Resource("injected source allocation failure")
            );
        },
    );
}

#[test]
fn source_scalar_origin_only_output_is_counted_without_ownership_flow() {
    with_typed(
        "struct T {} fn main() -> () { return; }",
        |typed, sources| {
            let usage = budget::preflight(typed, budget::Limits::DEFAULT).unwrap();
            assert_eq!(usage.raw_bytes, 944 + std::mem::size_of::<Vec<MatchDecl>>());
            assert_eq!(usage.analysis.owners, 0);
            assert_eq!(usage.analysis.expanded_events, 0);
            assert_eq!(usage.analysis.work, 4);
            assert_eq!(
                usage.analysis.metadata_bytes,
                136 + std::mem::size_of::<Vec<MatchDecl>>()
            );
            let raw = lower::lower(typed).unwrap();
            assert!(!super::super::budget::active(&raw.functions[0]));
            assert_eq!(
                super::super::budget::preflight(&raw, super::super::budget::Limits::DEFAULT)
                    .unwrap(),
                usage.analysis
            );
            verified::verify_owned(raw, sources).unwrap();
        },
    );
}

#[test]
fn every_source_output_and_map_reservation_is_fallible() {
    let text = "struct T { value: i32 } fn relay(x: T) -> T { return x; } fn get(p: &T, b: bool) -> i32 { if b { return p.value; } else { return 0; } } fn main() -> i32 { let mut x = relay(T { value: 7 }); x.value = get(&x, true && (false || true)); return x.value; }";
    with_typed(text, |typed, sources| {
        let mut denied = 0;
        loop {
            match budget::fail_allocation_after(denied, || lower::lower(typed)) {
                Err(error) => {
                    assert_eq!(
                        error.kind,
                        OwnedFailureKind::Resource("injected source allocation failure")
                    );
                    denied += 1;
                    assert!(denied < 300);
                }
                Ok(raw) => {
                    assert_eq!(
                        super::super::budget::preflight(
                            &raw,
                            super::super::budget::Limits::DEFAULT
                        )
                        .unwrap(),
                        budget::preflight(typed, budget::Limits::DEFAULT)
                            .unwrap()
                            .analysis,
                    );
                    let witness = verified::verify_owned(raw, sources).unwrap();
                    assert_eq!(
                        execute::run(&witness, typed.entry()).unwrap(),
                        Scalar::I32(7)
                    );
                    break;
                }
            }
        }
        // Three top-level/schema vectors; twelve output/map reservations for
        // each of three functions; 1+3+7 block statement vectors; two call
        // argument vectors; one constructor payload. Empty vectors also pass
        // the reservation seam, so every added reservation attempt is tested.
        assert_eq!(denied, 3 + 12 * 3 + (1 + 3 + 7) + 2 + 1);
    });
}

#[test]
fn cleanup_expansion_is_rejected_by_counting_before_any_output_allocation() {
    let mut text = String::from("struct T {} fn f(flag: bool) -> () {");
    for i in 0..450 {
        text.push_str(&format!("let value{i} = T {{}};"));
    }
    for _ in 0..600 {
        text.push_str("if flag { return; }");
    }
    text.push_str("return; }");
    with_typed(&text, |typed, _| {
        let result = budget::fail_allocation_after(0, || lower::lower(typed)).unwrap_err();
        assert_eq!(result.kind, OwnedFailureKind::Resource("assignments"));
    });
}

#[test]
fn source_count_checked_arithmetic_and_lowered_ceiling_cannot_wrap_or_raise_default() {
    assert!(budget::add(usize::MAX, 1).is_err());
    assert!(budget::mul(usize::MAX, 2).is_err());
    assert!(budget::reserve::<u64>(usize::MAX).is_err());
    with_typed("struct T {} fn main() -> () { return; }", |typed, _| {
        assert_eq!(
            budget::preflight(
                typed,
                budget::Limits {
                    raw_bytes: usize::MAX
                }
            )
            .unwrap(),
            budget::preflight(typed, budget::Limits::DEFAULT).unwrap()
        );
    });
}

#[test]
#[ignore = "production-token-envelope source payload and separate RSS qualification"]
fn production_token_envelope_lowers_a_large_real_source_graph() {
    // The prefix/suffix have20 non-EOF tokens including their trivia. Each
    // unseparated `while false{}` contributes exactly5 more retained tokens.
    let loops = (lexer::MAX_TOKENS - 20) / 5;
    assert_eq!(20 + 5 * loops, lexer::MAX_TOKENS);
    let text = format!(
        "struct T{{}}fn main()->(){{ /**/ {}return;}}",
        "while false{}".repeat(loops)
    );
    assert!(text.len() < crate::frontend::source::MAX_SOURCE_BYTES);
    let mut sources = SourceMap::new();
    let id = sources.add("token-envelope.ox".into(), text.clone());
    let file = sources.get(id);
    let tokens = lexer::lex(file).unwrap();
    assert_eq!(tokens.len(), lexer::MAX_TOKENS + 1); // EOF is appended separately.
    let ast = parser::parse_with_mode(file, tokens, parser::SourceMode::OwnedCandidate).unwrap();
    let typed = typeck::check(resolve::resolve(file, &ast).unwrap()).unwrap();
    let usage = budget::preflight(&typed, budget::Limits::DEFAULT).unwrap();
    // One entry, three blocks per while, one condition slot/instruction per
    // loop and one synthetic return-unit slot/instruction. No owner slots.
    assert_eq!(
        usage.raw_bytes,
        1224 * loops + 944 + std::mem::size_of::<Vec<MatchDecl>>()
    );
    assert_eq!(usage.scratch_bytes, 64 * loops + 8);
    assert_eq!(usage.analysis.work, 8 * loops + 4);
    assert_eq!(
        usage.analysis.metadata_bytes,
        224 * loops + 136 + std::mem::size_of::<Vec<MatchDecl>>()
    );
    let raw = lower::lower(&typed).unwrap();
    assert_eq!(raw.functions[0].blocks.len(), 3 * loops + 1);
    assert_eq!(raw.functions[0].locals.len(), loops + 1);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        execute::run_observed(
            &witness,
            hir::DefId(0),
            execute::Limits::default(),
            &mut events
        )
        .unwrap(),
        Scalar::Unit
    );
    let fuel: usize = events
        .iter()
        .filter_map(|e| match e {
            execute::Event::Charge(_, cost) => Some(*cost),
            _ => None,
        })
        .sum();
    assert_eq!(fuel, 4 * loops + 4);
    let over = format!(
        "struct T{{}}fn main()->(){{ /**/ {}return;}}",
        "while false{}".repeat(loops + 1)
    );
    let mut extra = SourceMap::new();
    let id = extra.add("token-over.ox".into(), over);
    let error = lexer::lex(extra.get(id)).unwrap_err();
    assert_eq!((error.code, error.stage), ("E0400", "lex"));
    println!("production source envelope: bytes={} tokens={} loops={} blocks={} scalar_slots={} raw_payload={} producer_maps={} origin_work={} origin_metadata={} fuel={}", text.len(), lexer::MAX_TOKENS, loops, 3 * loops + 1, loops + 1, usage.raw_bytes, usage.scratch_bytes, usage.analysis.work, usage.analysis.metadata_bytes, fuel);
}

#[test]
fn comment_padded_source_checks_do_not_claim_the_file_loader_byte_gate() {
    let mut text = String::from("struct T{}fn main()->(){return;}");
    let maximum = crate::frontend::source::MAX_SOURCE_BYTES;
    while maximum - text.len() >= 4 {
        let width = (maximum - text.len()).min(lexer::MAX_TOKEN_BYTES);
        text.push_str("/*");
        text.extend(std::iter::repeat_n('x', width - 4));
        text.push_str("*/");
    }
    text.extend(std::iter::repeat_n(' ', maximum - text.len()));
    assert_eq!(text.len(), maximum);
    for extra in [false, true] {
        if extra {
            text.push(' ');
        }
        with_typed(&text, |typed, sources| {
            let usage = budget::preflight(typed, budget::Limits::DEFAULT).unwrap();
            assert_eq!(usage.raw_bytes, 944 + std::mem::size_of::<Vec<MatchDecl>>());
            let witness = verified::verify_owned(lower::lower(typed).unwrap(), sources).unwrap();
            assert_eq!(execute::run(&witness, typed.entry()).unwrap(), Scalar::Unit);
        });
    }
    // The private SourceMap.add path intentionally also accepts one byte over.
    // Only the production file loader proves MAX_SOURCE_BYTES admission.
    assert_eq!(text.len(), maximum + 1);
}
