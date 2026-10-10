//! Source-free observer controls and a separately ignored source-only transport.
use super::*;

#[path = "array_pipeline_transport.rs"]
mod transport;

#[test]
fn unit3b2_observer_bytes_admission_checks_modules_and_copy_lengths() {
    let limits = ProjectLimits {
        modules: 1,
        source_bytes: 3,
        path_bytes: 7,
        ..ProjectLimits::default()
    };
    assert!(admit_bytes_input(7, 3, limits).is_ok());
    assert!(admit_bytes_input(
        7,
        3,
        ProjectLimits {
            modules: 0,
            ..limits
        }
    )
    .is_err());
    assert!(admit_bytes_input(8, 3, limits).is_err());
    assert!(admit_bytes_input(7, 4, limits).is_err());
    assert!(admit_bytes_input(
        0,
        0,
        ProjectLimits {
            modules: 0,
            ..limits
        }
    )
    .is_err());
}

#[test]
fn unit3b2_observer_row_exact_and_one_under_are_prechecked() {
    let expected = "[1,0,\"phase\",\"type\",\"attempted\"]\n";
    for (units, bytes, success) in [
        (6, expected.len(), true),
        (5, expected.len(), false),
        (6, expected.len() - 1, false),
    ] {
        let mut out = Output::new(
            Limits {
                units,
                bytes,
                ..Limits::default()
            },
            Control::Complete,
        )
        .unwrap();
        let capacity = out.text.capacity();
        assert_eq!(out.phase("type", "attempted").is_ok(), success);
        assert_eq!(out.text.capacity(), capacity);
        if success {
            assert_eq!(out.text, expected);
            assert_eq!(out.units, 6);
        } else {
            assert_eq!(
                out.finish(Outcome::Validated).outcome,
                Outcome::IncompleteObservation
            );
        }
    }
}

struct Changing<'a>(&'a std::cell::Cell<bool>, &'static str, &'static str);
impl fmt::Display for Changing<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0.replace(true) { self.2 } else { self.1 })
    }
}
#[test]
fn unit3b2_observer_actual_writes_reject_byte_and_unit_mismatch() {
    // First case would grow beyond the counted row; second has equal bytes but
    // a different number of JSON values. Neither can leave a success receipt.
    for (first, second) in [("0", "000000000000"), ("null", "[[]]")] {
        let mut out = Output::new(
            Limits {
                bytes: 64,
                ..Limits::default()
            },
            Control::Complete,
        )
        .unwrap();
        let capacity = out.text.capacity();
        let changed = std::cell::Cell::new(false);
        assert!(out
            .row(
                "phase",
                format_args!("{}", Changing(&changed, first, second))
            )
            .is_err());
        assert_eq!(out.text.capacity(), capacity);
        let receipt = out.finish(Outcome::Validated);
        assert_eq!(receipt.outcome, Outcome::IncompleteObservation);
        assert!(receipt.transcript.is_empty());
    }
}

#[test]
fn unit3b2_observer_terminal_exhaustion_discards_tentative_rows() {
    let mut out = Output::new(
        Limits {
            bytes: 64,
            units: 6,
            ..Limits::default()
        },
        Control::Complete,
    )
    .unwrap();
    out.phase("type", "attempted").unwrap();
    let receipt = out.finish(Outcome::Validated);
    assert_eq!(receipt.outcome, Outcome::IncompleteObservation);
    assert!(receipt.transcript.is_empty());
    assert_eq!((receipt.units, receipt.bytes), (0, 0));
}

#[test]
fn unit3b2_observer_initial_real_transcript_failure_is_incomplete() {
    assert!(Output::new(Limits::default(), Control::FailTranscriptReservation).is_err());
}

#[test]
fn unit3b2_observer_trace_cap_and_rebinding_guards_precede_mutation() {
    let mut allocator = Allocator::default();
    assert_eq!(
        allocator.observer_trace_bound(200_001),
        Err(crate::frontend::project::budget::ReserveFailure::Overflow)
    );
    assert_eq!(allocator.attempts, 0);
    assert!(allocator.trace.is_empty());
    allocator.observer_trace_bound(0).unwrap();
    assert_eq!(
        allocator.observer_trace_bound(1),
        Err(crate::frontend::project::budget::ReserveFailure::Overflow)
    );
    assert_eq!(allocator.observer_trace_limit, Some(0));
    let mut previous = Allocator::default();
    previous
        .vector_exact(&mut Vec::<u8>::new(), 0, "fixed control")
        .unwrap();
    assert_eq!(
        previous.observer_trace_bound(1),
        Err(crate::frontend::project::budget::ReserveFailure::Overflow)
    );
    assert_eq!(previous.attempts, 1);
    assert_eq!(previous.trace.len(), 1);
}

#[test]
fn unit3b2_observer_layout_without_source_invocation() {
    println!("OBSERVER_LAYOUT {{\"Output\":{},\"Counter\":{},\"BoundedWrite\":{},\"Limits\":{},\"LiteralRequest\":{},\"Receipt\":{},\"LoadFailure\":{},\"CaptureWrapper\":{}}}",
        size_of::<Output>(),size_of::<Counter>(),size_of::<BoundedWrite<'_>>(),size_of::<Limits>(),
        size_of::<LiteralRequest>(),size_of::<Receipt>(),size_of::<crate::frontend::project::LoadFailure>(),
        size_of::<RefCell<Option<Output>>>());
    macro_rules! layout {
        ($name:literal,$ty:ty) => {
            println!(
                "OWNERSHIP_LAYOUT {{\"name\":{},\"bytes\":{},\"align\":{}}}",
                Json($name),
                size_of::<$ty>(),
                std::mem::align_of::<$ty>()
            );
        };
    }
    layout!("usize", usize);
    layout!("Operand", Operand);
    layout!(
        "ReserveEvent",
        crate::frontend::project::budget::ReserveEvent
    );
    layout!("Option<DiagnosticOrigins>", Option<DiagnosticOrigins>);
    layout!("OwnerSites", crate::frontend::oir::owned::shape::OwnerSites);
    layout!("CallSites", crate::frontend::oir::owned::shape::CallSites);
    layout!(
        "Option<Site>",
        Option<crate::frontend::oir::owned::shape::Site>
    );
    layout!("(usize,bool)", (usize, bool));
    layout!("ParameterBinding", ParameterBinding);
    layout!("ReferenceDecl", ReferenceDecl);
    layout!("LoanDecl", LoanDecl);
    layout!("(FieldId,Operand)", (FieldId, Operand));
    layout!("RawOwnedProgram", RawOwnedProgram);
    layout!("RawOwnedFunction", RawOwnedFunction);
    layout!("RawRecordDecl", RawRecordDecl);
    layout!("RawFieldDecl", RawFieldDecl);
    layout!("LocalDecl", LocalDecl);
    layout!("PlaceDecl", PlaceDecl);
    layout!("OwnerDecl", OwnerDecl);
    layout!("CallDecl", CallDecl);
    layout!("ArgumentSlot", ArgumentSlot);
    layout!("OwnedBlock", OwnedBlock);
    layout!("OwnedStatement", OwnedStatement);
    layout!("SourceFile", SourceFile);
    layout!("ast::Program", ast::Program);
    layout!("ModuleHeader", crate::frontend::project::ModuleHeader);
    layout!("ast::ExprId", ast::ExprId);
    layout!("source::hir::ExprId", source_hir::ExprId);
    layout!("u8", u8);
    layout!("u32", u32);
}

fn downward<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name).map_or(default, |value| {
        value
            .parse::<T>()
            .ok()
            .expect("unsigned fixed transport limit")
    })
}
/// One requested source per process. The surrounding fixed runner binds the
/// executable/request/source hashes and writes stdout to a bounded file. These
/// environment names are read only by this ignored cfg(test) transport.
#[test]
#[ignore = "requires independently admitted source, observer implementation and wire contract"]
fn unit3b2_source_only_transport() {
    let input_kind =
        transport::InputKind::from_environment(std::env::var("OXID_ARRAY_PIPELINE_INPUT_KIND"))
            .expect("closed source transport input kind");
    let root = match input_kind {
        transport::InputKind::Root => {
            let root = std::env::var("OXID_ARRAY_PIPELINE_ROOT").expect("fixed source root");
            assert!(root.len() <= 4096, "bounded transport root path");
            Some(root)
        }
        transport::InputKind::Bytes => None,
    };
    let control = match std::env::var("OXID_ARRAY_PIPELINE_CONTROL").as_deref() {
        Ok("complete") | Err(_) => Control::Complete,
        Ok("literal-capacity-failure") => Control::FailLiteralOperand {
            ordinal: downward("OXID_ARRAY_PIPELINE_ORDINAL", 0),
        },
        Ok("expanded-boundary-first-operand-failure") => {
            Control::ExpandedBoundaryFirstOperandFailure
        }
        Ok("transcript-capacity-failure") => Control::FailTranscriptReservation,
        Ok("trace-capacity-failure") => Control::FailTraceReservation,
        _ => panic!("unknown closed source transport control"),
    };
    let mut limits = Limits::default();
    limits.units = downward("OXID_ARRAY_PIPELINE_UNITS", limits.units);
    limits.bytes = downward("OXID_ARRAY_PIPELINE_BYTES", limits.bytes);
    limits.auxiliary = downward("OXID_ARRAY_PIPELINE_AUXILIARY", limits.auxiliary);
    limits.trace_rows = downward("OXID_ARRAY_PIPELINE_TRACE_ROWS", limits.trace_rows);
    limits.raw_bytes = downward("OXID_ARRAY_PIPELINE_RAW_BYTES", limits.raw_bytes);
    limits.index.retained = downward("OXID_ARRAY_PIPELINE_INDEX_RETAINED", limits.index.retained);
    limits.index.scratch = downward("OXID_ARRAY_PIPELINE_INDEX_SCRATCH", limits.index.scratch);
    limits.index.work = downward("OXID_ARRAY_PIPELINE_INDEX_WORK", limits.index.work);
    limits.diagnostic_work = downward(
        "OXID_ARRAY_PIPELINE_DIAGNOSTIC_WORK",
        limits.diagnostic_work,
    );
    limits.verification.owners = downward(
        "OXID_ARRAY_PIPELINE_VERIFY_OWNERS",
        limits.verification.owners,
    );
    limits.verification.events = downward(
        "OXID_ARRAY_PIPELINE_VERIFY_EVENTS",
        limits.verification.events,
    );
    limits.verification.work =
        downward("OXID_ARRAY_PIPELINE_VERIFY_WORK", limits.verification.work);
    limits.verification.scratch = downward(
        "OXID_ARRAY_PIPELINE_VERIFY_SCRATCH",
        limits.verification.scratch,
    );
    limits.verification.metadata = downward(
        "OXID_ARRAY_PIPELINE_VERIFY_METADATA",
        limits.verification.metadata,
    );
    let receipt = match input_kind {
        transport::InputKind::Root => observe(
            SourceInput::Root {
                path: Path::new(root.as_deref().expect("selected Root input")),
            },
            Mode::Validate,
            limits,
            control,
        ),
        transport::InputKind::Bytes => {
            let frame = transport::read_bytes_frame(&mut std::io::stdin().lock())
                .expect("bounded original Bytes input frame");
            let receipt = observe(frame.input(), Mode::Validate, limits, control);
            drop(frame);
            receipt
        }
    };
    println!("\nOXID_ARRAY_PIPELINE_ROWS_BEGIN");
    print!("{}", receipt.transcript);
    println!("OXID_ARRAY_PIPELINE_ROWS_END");
    println!("OXID_ARRAY_PIPELINE_RECEIPT {{\"schema\":\"oxid-array-source-lowering-receipt-v1\",\"outcome\":{},\"reason\":{},\"units\":{},\"bytes\":{}}}",
        Json(receipt.outcome.name()),Optional(receipt.reason.map(Json)),receipt.units,receipt.bytes);
}

// Approved closed Bytes-source lexer-null controls. These tests are not an
// external provider/process or whole-process OOM qualification.
fn lexer_null_source(allocator: &mut Allocator) -> SourceMap {
    let mut text = String::new();
    allocator
        .string(&mut text, LEXER_NULL_TEXT.len(), "observer original source")
        .unwrap();
    text.push_str(LEXER_NULL_TEXT);
    let mut path = String::new();
    allocator
        .string(&mut path, LEXER_NULL_PATH.len(), "observer original path")
        .unwrap();
    path.push_str(LEXER_NULL_PATH);
    let mut sources = SourceMap::new();
    sources.try_add(path, text, allocator).unwrap();
    sources
}
fn lexer_null_prior_bank(trace_rows: usize) -> usize {
    [
        size_of::<Output>(),
        size_of::<Allocator>(),
        size_of::<RefCell<Option<Output>>>(),
        lexer::reservation_scratch_bytes(),
        lexer::reservation_observer_bytes(),
        trace_rows * size_of::<crate::frontend::project::budget::ReserveEvent>(),
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
    .unwrap()
}
#[test]
fn lexer_caller_source_null_layout_measurement_only() {
    use std::mem::{align_of, align_of_val, offset_of, size_of_val};
    // Setup the actual registered source and actual factory closure; neither
    // invoke the action nor install a selector in this measurement-only test.
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(7).unwrap();
    let sources = lexer_null_source(&mut allocator);
    let source = sources.get(crate::frontend::source::SourceFileId(0));
    let action = source_null_action(source, Limits::default().source.tokens);
    let tracker = super::super::reviewer_source::integration_tracker_layout();
    let bank = source_null_bank_bytes(&action).unwrap();
    println!("source-lexer-null-bank control={} align={} trackers={} align={} sizing={} action={} action-align={} facts={} facts-align={} report={} report-align={} row={} row-align={} fresh-selection={} growth-selection={} additional={} prior={} trace-requested={} trace-retained={} source-retained={} source-tls={} source-tls-align={} source-stats={} source-stats-align={} source-option={} source-option-align={} tracker-accessor-return={} tracker-accessor-caller={}",
        size_of::<SourceNullControlBank<'_>>(), align_of::<SourceNullControlBank<'_>>(),
        size_of::<SourceNullTrackerBank<'_>>(), align_of::<SourceNullTrackerBank<'_>>(),
        source_null_sizing_bytes(&action), size_of_val(&action), align_of_val(&action),
        size_of::<SourceNullFacts>(), align_of::<SourceNullFacts>(), size_of::<SourceNullReport>(), align_of::<SourceNullReport>(),
        size_of::<SourceNullRow>(), align_of::<SourceNullRow>(), lexer_null::selection_carriers_bytes(&action), lexer_growth::selection_carriers_bytes(&action),
        bank, lexer_null_prior_bank(7), 7 * size_of::<crate::frontend::project::budget::ReserveEvent>(),
        allocator.trace.capacity() * size_of::<crate::frontend::project::budget::ReserveEvent>(), sources.heap_capacity_bytes().unwrap(),
        tracker.0, tracker.1, tracker.2, tracker.3, tracker.4, tracker.5,
        size_of::<(usize, usize, usize, usize, usize, usize)>(), size_of_val(&tracker));
    macro_rules! fields {
        ($ty:ty; $($field:ident),+ $(,)?) => {
            $(println!("source-lexer-null-offset {}.{}={}", stringify!($ty), stringify!($field), offset_of!($ty, $field));)+
        };
    }
    fields!(SourceNullControlBank<'_>; control, caller_control, site, dispatch_site, facts, returned_facts, caller_facts,
        report, returned_report, caller_report, optional_failure, failure, returned_failure, caller_failure,
        fresh_target, growth_target, old_layout, new_layout, old_layout_result, new_layout_result,
        source, token_limit, source_identity, source_after, source_matches, report_matches, trace_matches,
        ordinal, old_slots, new_slots, old_bytes, new_bytes, trace_len, trace_capacity, trace_limit, configured_trace,
        lexical_return, lexical_caller, returned, caller, selected_return, selected_caller,
        row, row_return, row_caller, format_arguments, format_arguments_caller, trace_event, trace_index);
    fields!(SourceNullTrackerBank<'_>; raw_enabled_tls, raw_count_tls,
        raw_enabled_callback, raw_count_callback, source_tracker_callback,
        raw_read, raw_count, raw_increment, raw_write,
        delta, allocation, accessor_return, accessor_caller);
    println!("source-lexer-null-callback-references raw-enabled={} raw-count={} source-tracker-thin={} total={}",
        size_of::<&std::cell::Cell<bool>>(), size_of::<&std::cell::Cell<usize>>(),
        size_of::<&std::cell::Cell<()>>(),
        [size_of::<&std::cell::Cell<bool>>(), size_of::<&std::cell::Cell<usize>>(), size_of::<&std::cell::Cell<()>>()]
            .into_iter().try_fold(0usize, usize::checked_add).unwrap());
    fields!(SourceNullFacts; failure, unexpected_success);
    fields!(SourceNullRow; ordinal, operation, old_bytes, new_bytes, fired, reserve_failed, owner_unchanged,
        address_unchanged, length_unchanged, capacity_unchanged, drop_count, trace_preserved, source_preserved);
    assert_eq!(allocator.attempts, 4);
    assert!(source_null_trace_matches(&allocator, 4, false));
}
fn source_null_sizing_bytes<F>(_: &F) -> usize
where
    F: for<'a> FnOnce(&'a mut Allocator) -> SourceNullFacts,
{
    size_of::<SourceNullSizingCarriers<'_, F>>()
}
#[test]
fn lexer_caller_source_null_added_bank_exact_and_one_short_without_selection() {
    let mut setup = Allocator::default();
    setup.observer_trace_bound(7).unwrap();
    let sources = lexer_null_source(&mut setup);
    let action = source_null_action(sources.get(crate::frontend::source::SourceFileId(0)), 8);
    let bank = source_null_bank_bytes(&action).unwrap();
    let prior = lexer_null_prior_bank(7);
    let exact = prior.checked_add(bank).unwrap();
    // This isolated aux arithmetic control is distinct from a full route's
    // four already completed source-owner requests. It never calls the action.
    let allocator = Allocator::default();
    let selected_calls = std::cell::Cell::new(0usize);
    for (limit, admitted) in [(exact, true), (exact - 1, false)] {
        let mut out = Output::new(
            Limits {
                bytes: 256,
                auxiliary: limit,
                ..Limits::default()
            },
            Control::LexerNull {
                site: LexerNullSite::First,
            },
        )
        .unwrap();
        out.aux(prior).unwrap();
        assert_eq!(out.aux(bank).is_ok(), admitted);
        if admitted {
            assert_eq!(out.auxiliary, exact);
        }
        assert_eq!(allocator.attempts, 0);
        assert_eq!(selected_calls.get(), 0);
    }
}
#[test]
fn lexer_caller_source_null_full_prefix_bank_refusal_makes_no_new_selected_call() {
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(7).unwrap();
    let sources = lexer_null_source(&mut allocator);
    let source = sources.get(crate::frontend::source::SourceFileId(0));
    let action = source_null_action(source, 8);
    let bank = source_null_bank_bytes(&action).unwrap();
    let prior = lexer_null_prior_bank(7);
    let mut out = Output::new(
        Limits {
            bytes: 256,
            trace_rows: 7,
            auxiliary: prior + bank - 1,
            ..Limits::default()
        },
        Control::LexerNull {
            site: LexerNullSite::First,
        },
    )
    .unwrap();
    out.aux(prior).unwrap();
    assert_eq!(
        source_null_lex(source, 8, &mut allocator, &mut out, LexerNullSite::First),
        Err("auxiliary observation limit exceeded")
    );
    assert!(source_null_trace_matches(&allocator, 4, false));
    assert_eq!(
        sources.get(crate::frontend::source::SourceFileId(0)).text(),
        LEXER_NULL_TEXT
    );
    assert!(out.text.is_empty());
}
#[test]
fn lexer_caller_source_null_trace_one_short_refuses_even_fresh_before_selection() {
    for site in [
        LexerNullSite::First,
        LexerNullSite::GrowEight,
        LexerNullSite::GrowSixteen,
    ] {
        let rows = site.ordinal() - 1;
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(rows).unwrap();
        let sources = lexer_null_source(&mut allocator);
        let source = sources.get(crate::frontend::source::SourceFileId(0));
        let mut out = Output::new(
            Limits {
                bytes: 256,
                trace_rows: rows,
                ..Limits::default()
            },
            Control::LexerNull { site },
        )
        .unwrap();
        assert_eq!(
            source_null_lex(source, 8, &mut allocator, &mut out, site),
            Err("lexer null trace prepayment or prefix refused")
        );
        assert!(source_null_trace_matches(&allocator, 4, false));
        assert!(out.text.is_empty());
    }
}
#[test]
fn lexer_caller_source_null_unexpected_success_drops_tape_then_refuses() {
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(7).unwrap();
    let sources = lexer_null_source(&mut allocator);
    let source = sources.get(crate::frontend::source::SourceFileId(0));
    let action = source_null_action(source, 8);
    let (facts, stats) =
        super::super::reviewer_source::integration_measured(|| action(&mut allocator));
    assert!(facts.failure.is_none() && facts.unexpected_success);
    assert_eq!(
        source_null_failure(facts),
        Err("lexer null unexpected successful tape")
    );
    assert_eq!(
        stats.1, 0,
        "successful tape was ordinarily dropped before return"
    );
    assert!(source_null_trace_matches(&allocator, 7, false));
}
#[test]
fn lexer_caller_source_null_wrong_fixture_root_and_credit_refuse() {
    for (input, tokens) in [
        (
            SourceInput::Bytes {
                path: "wrong.ox",
                text: LEXER_NULL_TEXT,
            },
            8,
        ),
        (
            SourceInput::Bytes {
                path: LEXER_NULL_PATH,
                text: "",
            },
            8,
        ),
        (
            SourceInput::Bytes {
                path: LEXER_NULL_PATH,
                text: LEXER_NULL_TEXT,
            },
            7,
        ),
        (
            SourceInput::Root {
                path: Path::new("/not-opened/lexer-null.ox"),
            },
            8,
        ),
    ] {
        let receipt = observe(
            input,
            Mode::Validate,
            Limits {
                source: ProjectLimits {
                    tokens,
                    ..ProjectLimits::default()
                },
                ..Limits::default()
            },
            Control::LexerNull {
                site: LexerNullSite::First,
            },
        );
        assert_eq!(receipt.outcome, Outcome::IncompleteObservation);
        assert_eq!(
            receipt.reason,
            Some("lexer null fixture or token credit refused")
        );
        assert!(receipt.transcript.is_empty());
    }
}
#[test]
fn lexer_caller_actual_null_source_pipeline_three_sites_stop_before_parser() {
    for (site, start, end) in [
        (LexerNullSite::First, 0, 4),
        (LexerNullSite::GrowEight, 16, 20),
        (LexerNullSite::GrowSixteen, 32, 32),
    ] {
        let receipt = observe(
            SourceInput::Bytes {
                path: LEXER_NULL_PATH,
                text: LEXER_NULL_TEXT,
            },
            Mode::Validate,
            Limits {
                trace_rows: site.ordinal(),
                source: ProjectLimits {
                    tokens: 8,
                    ..ProjectLimits::default()
                },
                ..Limits::default()
            },
            Control::LexerNull { site },
        );
        assert_eq!(receipt.outcome, Outcome::Diagnostic, "{:?}", receipt.reason);
        assert!(receipt.reason.is_none());
        let text = &receipt.transcript;
        assert_eq!(text.matches("\"lexer-null\"").count(), 1);
        assert_eq!(
            text.matches("\"frontend-reservation\"").count(),
            site.ordinal()
        );
        assert!(text.contains(&format!(
            "\"frontend-trace\",{},{},{},",
            site.ordinal(),
            site.ordinal(),
            site.ordinal()
        )));
        assert!(text.contains("\"load-parse\",\"failed\""));
        assert!(!text.contains("\"load-parse\",\"completed\""));
        assert!(!text.contains("\"source-owner\""));
        assert!(!text.contains("\"resolve-index\""));
        assert!(!text.contains("\"lower\""));
        assert!(text.contains("E0400") && text.contains("token storage allocation failed"));
        // Existing diagnostic renderer retains the pending token's file-aware span.
        let mut sm = SourceMap::new();
        let id = sm.add(LEXER_NULL_PATH.into(), LEXER_NULL_TEXT.into());
        let expected = Diagnostic::new(
            "E0400",
            "lex",
            "token storage allocation failed",
            Some(sm.get(id).span(start, end)),
        );
        assert!(text.contains(&expected.render_json(&sm)));
        println!(
            "SOURCE_LEXER_NULL site={} old-new={:?} bytes={} units={} transcript={}",
            site.ordinal(),
            site.slots(),
            receipt.bytes,
            receipt.units,
            text
        );
    }
}
