use super::tests::{formatted, source};
use super::*;
use crate::frontend::source::SourceFileId;

fn error(input: &str, stage: &str) {
    let errors = formatted(input).unwrap_err();
    assert_eq!(errors[0].code, "E0400", "{errors:?}");
    assert_eq!(errors[0].stage, stage, "{errors:?}");
}

#[test]
fn exact_source_output_and_single_token_limits() {
    let comment = format!("/*{}*/", "x".repeat(65_531));
    let tight = comment.repeat(16);
    let canonical = vec![comment.as_str(); 16].join(" ") + "\n";
    assert_eq!(canonical.len(), MAX_SOURCE_BYTES);
    assert_eq!(formatted(&tight).unwrap(), canonical);
    assert_eq!(formatted(&canonical).unwrap(), canonical);
    error(&(canonical + " "), "format");
    error(
        &(format!("/*{}*/", "x".repeat(65_532)) + &comment.repeat(15)),
        "format",
    );
    let max_token = format!("/*{}*/", "x".repeat(65_532));
    assert_eq!(max_token.len(), lexer::MAX_TOKEN_BYTES);
    assert_eq!(formatted(&max_token).unwrap(), max_token + "\n");
    error(&format!("/*{}*/", "x".repeat(65_533)), "lex");
    assert_eq!(formatted(&" ".repeat(65_536)).unwrap(), "");
    error(&" ".repeat(65_537), "lex");
}

#[test]
fn input_and_candidate_token_limits_count_added_gaps() {
    let input = "/**/".repeat(50_000);
    let output = vec!["/**/"; 50_000].join(" ") + "\n";
    assert_eq!(formatted(&input).unwrap(), output);
    assert_eq!(formatted(&output).unwrap(), output);
    error(&(output + "/**/"), "lex");
    error(&"/**/".repeat(50_001), "format");
}

#[test]
fn parser_depth_parameters_arguments_and_path_limits_remain_in_force() {
    let grouped = |depth| {
        format!(
            "fn f()->bool{{return {}true{};}}",
            "(".repeat(depth),
            ")".repeat(depth)
        )
    };
    assert!(formatted(&grouped(63)).is_ok());
    error(&grouped(64), "parse");
    let sum = |count| format!("fn f()->i32{{return {};}}", vec!["1"; count].join("+"));
    assert!(formatted(&sum(64)).is_ok());
    error(&sum(65), "parse");
    let blocks = |count| {
        format!(
            "fn f()->(){{{}return;{}}}",
            "if true{".repeat(count),
            "}".repeat(count)
        )
    };
    assert!(formatted(&blocks(63)).is_ok());
    error(&blocks(64), "parse");
    let params = |count| {
        format!(
            "fn f({})->(){{return;}}",
            (0..count)
                .map(|i| format!("p{i}:i32"))
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    assert!(formatted(&params(256)).is_ok());
    error(&params(257), "parse");
    let args = |count| {
        format!(
            "fn f()->(){{sink({});return;}}",
            vec!["true"; count].join(",")
        )
    };
    assert!(formatted(&args(256)).is_ok());
    error(&args(257), "parse");
    let path = |count| format!("use crate{};", "::p".repeat(count));
    assert!(formatted(&path(33)).is_ok());
    error(&path(34), "parse");
}

#[test]
fn isolated_delimiter_work_and_overflow_boundaries() {
    let input = "fn f()->(){sink((true));return;}";
    let sources = source(input);
    let file = sources.get(SourceFileId(0));
    let limits = Limits {
        delimiters: 3,
        work_bytes: input.len(),
    };
    assert!(format_with_limits(file, &mut Allocator::default(), limits).is_ok());
    for limits in [
        Limits {
            delimiters: 2,
            ..limits
        },
        Limits {
            work_bytes: input.len() - 1,
            ..limits
        },
    ] {
        let errors = format_with_limits(file, &mut Allocator::default(), limits).unwrap_err();
        assert_eq!((errors[0].code, errors[0].stage), ("E0400", "format"));
    }
    assert_eq!(Limits::DEFAULT.delimiters, 128);
    assert_eq!(Limits::DEFAULT.work_bytes, 8 * 1024 * 1024);
    assert_eq!(
        output_length(MAX_SOURCE_BYTES - 1, 1).unwrap(),
        MAX_SOURCE_BYTES
    );
    assert!(output_length(MAX_SOURCE_BYTES, 1).is_err());
    assert!(output_length(usize::MAX, 1).is_err());
}

#[test]
fn fallible_reservations_reject_without_returning_a_candidate() {
    let sources = source("pub mod absent; fn f()->(){return;}");
    let file = sources.get(SourceFileId(0));
    let mut baseline = Allocator::default();
    assert!(format_with_allocator(file, &mut baseline).is_ok());
    assert!(baseline
        .trace
        .iter()
        .any(|event| event.kind == "formatter token roles"));
    assert!(baseline
        .trace
        .iter()
        .any(|event| event.kind == "formatted source"));
    for attempt in 1..=baseline.attempts {
        let mut allocator = Allocator {
            fail_at: Some(attempt),
            ..Allocator::default()
        };
        let errors = format_with_allocator(file, &mut allocator).unwrap_err();
        assert!(
            errors.iter().any(|error| error.code == "E0400"),
            "attempt {attempt}: {errors:?}"
        );
    }
}

#[test]
fn node_and_diagnostic_limits_are_not_relaxed() {
    assert_eq!(parser::MAX_NODES, 100_000);
    let sources = source("fn f()->(){true;return;}");
    let file = sources.get(SourceFileId(0));
    for limit in [4, 3] {
        let result = parser::parse_counted(
            file,
            lexer::lex(file).unwrap(),
            SourceMode::ProjectCandidate,
            limit,
            &mut Allocator::default(),
        );
        assert_eq!(result.is_ok(), limit == 4);
    }
    let errors = formatted(&"fn bad()->(){return}\n".repeat(150)).unwrap_err();
    assert_eq!(errors.len(), parser::MAX_DIAGNOSTICS);
}

#[test]
fn indentation_cannot_push_candidate_whitespace_past_token_limit() {
    let input = |breaks| format!("fn f()->(){{{}return;}}", "\n".repeat(breaks));
    assert!(formatted(&input(65_532)).is_ok());
    error(&input(65_533), "format");
}

#[test]
fn array_syntax_and_delimiters_keep_bounded_admission() {
    assert!(formatted("fn f(a:[i32;1024])->(){return;}").is_ok());
    error("fn f(a:[i32;1025])->(){return;}", "parse");
    let literal = |count| {
        format!(
            "fn f()->(){{let a=[{}];return;}}",
            vec!["1"; count].join(",")
        )
    };
    assert!(formatted(&literal(1024)).is_ok());
    error(&literal(1025), "parse");
    let input = "fn f()->(){sink([a[0]]);return;}";
    let sources = source(input);
    let file = sources.get(SourceFileId(0));
    for depth in [4, 3] {
        let result = format_with_limits(
            file,
            &mut Allocator::default(),
            Limits {
                delimiters: depth,
                ..Limits::DEFAULT
            },
        );
        assert_eq!(result.is_ok(), depth == 4);
        if let Err(errors) = result {
            assert_eq!((errors[0].code, errors[0].stage), ("E0400", "format"));
        }
    }
    let mut baseline = Allocator::default();
    assert!(format_with_allocator(file, &mut baseline).is_ok());
    assert!(baseline
        .trace
        .iter()
        .any(|event| event.kind == "array literal elements"));
    for attempt in 1..=baseline.attempts {
        let mut allocator = Allocator {
            fail_at: Some(attempt),
            ..Allocator::default()
        };
        let errors = format_with_allocator(file, &mut allocator).unwrap_err();
        assert!(
            errors.iter().any(|error| error.code == "E0400"),
            "array attempt {attempt}: {errors:?}"
        );
    }
}

#[test]
fn u8_formatter_preserves_exact_work_height_and_allocation_endpoints() {
    let text = "fn f(x:i32)->u8{return (x.to_u8_checked());}";
    let sources = source(text);
    let file = sources.get(SourceFileId(0));
    let exact = Limits {
        delimiters: 3,
        work_bytes: text.len(),
    };
    let mut allocator = Allocator::default();
    let result = format_with_limits(file, &mut allocator, exact).unwrap();
    assert_eq!(formatted(&result).unwrap(), result);
    for limits in [
        Limits {
            delimiters: 2,
            ..exact
        },
        Limits {
            work_bytes: text.len() - 1,
            ..exact
        },
    ] {
        let errors = format_with_limits(file, &mut Allocator::default(), limits).unwrap_err();
        assert_eq!((errors[0].code, errors[0].stage), ("E0400", "format"));
    }
    for attempt in 1..=allocator.attempts {
        let mut failed = Allocator {
            fail_at: Some(attempt),
            ..Allocator::default()
        };
        assert!(
            format_with_limits(file, &mut failed, exact).is_err(),
            "reserve {attempt}"
        );
    }
    for (groups, accepted) in [(62, true), (63, false)] {
        let input = format!(
            "fn f(x:i32)->u8{{return {}x.to_u8_checked(){};}}",
            "(".repeat(groups),
            ")".repeat(groups)
        );
        assert_eq!(formatted(&input).is_ok(), accepted);
    }
    let (result, metrics) = format_enum_candidate_observed(file, &mut Allocator::default());
    result.unwrap();
    assert_eq!(metrics.parse_calls, 2);
    assert_eq!(metrics.roles_heap, text.len());
    assert!(metrics.phase_peak_heap_bound >= metrics.reparse_live_heap);
    println!("U8_FORMAT_SUCCESSOR {metrics:?}");
}
