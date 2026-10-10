// Additional current-only control, compiled in the isolated reviewer_unit1
// module. It is not one of the unchanged 69 original cases. This proves
// simulated reserve/counter diagnostics; actual allocator-null tests are separate.
#[test]
fn current_lexer_storage_failure_full_span_and_eof() {
    for (text, expected) in [
        ("", vec![(4, (0, 0, 0))]),
        (";;;;", vec![(4, (0, 0, 1)), (8, (0, 4, 4))]),
        (";;;;🦀", vec![(4, (0, 0, 1)), (8, (0, 4, 8))]),
    ] {
        let mut sources = SourceMap::new();
        sources.add("current-lexer-origins.ox".into(), text.into());
        let source = sources.get(SourceFileId(0));
        let mut successful = Allocator::default();
        let tape = lexer::lex_with_allocator(source, lexer::MAX_TOKENS, &mut successful).unwrap();
        assert_eq!(successful.attempts, expected.len());
        assert_eq!(successful.trace.len(), expected.len());
        assert_eq!(tape.last().unwrap().kind, lexer::Kind::Eof);
        for (index, (target, origin)) in expected.iter().copied().enumerate() {
            let event = &successful.trace[index];
            assert_eq!((event.kind, event.length, event.element_bytes, event.success),
                ("lexer token tape", target, std::mem::size_of::<lexer::Token>(), true));
            for overflow in [false, true] {
                let ordinal = index + 1;
                let mut allocator = if overflow {
                    Allocator { attempts: usize::MAX - (ordinal - 1), ..Allocator::default() }
                } else {
                    Allocator { fail_at: Some(ordinal), ..Allocator::default() }
                };
                let failure = lexer::lex_with_allocator(source, lexer::MAX_TOKENS, &mut allocator).unwrap_err();
                assert!(failure.is_storage());
                let d = failure.diagnostic();
                let message = if overflow { "token storage resource limit exceeded" }
                    else { "token storage allocation failed" };
                assert_eq!((d.code, d.stage, d.message.as_str()), ("E0400", "lex", message));
                assert_eq!(d.primary.map(tuple), Some(origin));
                assert!(d.secondary.is_empty() && d.notes.is_empty());
                assert_eq!(allocator.trace.len(), if overflow { index } else { ordinal });
                assert_eq!(allocator.attempts, if overflow { usize::MAX } else { ordinal });
                for (actual, baseline) in allocator.trace.iter().zip(&successful.trace) {
                    assert_eq!((actual.kind, actual.length, actual.element_bytes),
                        (baseline.kind, baseline.length, baseline.element_bytes));
                }
                assert_eq!(allocator.trace.iter().filter(|event| !event.success).count(), usize::from(!overflow));
            }
        }
    }
}
