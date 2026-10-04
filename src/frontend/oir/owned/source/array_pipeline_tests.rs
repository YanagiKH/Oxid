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
