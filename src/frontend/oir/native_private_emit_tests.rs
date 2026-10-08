//! Local descriptor/sink and negative native-boundary controls. Successful
//! imported emission is tested through the paid source leaf; no native tools.
use super::private_emit::{named_bytes, Admission, Failure, OutputMode};
use super::*;
use crate::frontend::project::budget::Allocator;

#[test]
fn private_emit_native_entry_errors_precede_final_text_admission() {
    let (program, sources) = super::tests::verified_with_sources("fn main()->i32 { return 7; }");
    for entry in [None, Some(hir::DefId(0)), Some(hir::DefId(usize::MAX))] {
        let mut allocator = Allocator::default();
        let admission = Admission::new(0, 0, usize::MAX, &mut allocator).unwrap();
        let result = program.native_module_private(entry, &sources, admission);
        if entry == Some(hir::DefId(0)) {
            assert!(matches!(result, Err(Failure::Budget)));
        } else {
            let expected = program.native_module(entry, &sources).unwrap_err();
            let Err(Failure::Diagnostic(actual)) = result else {
                panic!("expected authentic native entry diagnostic");
            };
            assert_eq!(actual.code, if entry.is_none() { "E0700" } else { "E0500" });
            assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        }
        assert_eq!(allocator.attempts, 0);
        assert!(allocator.trace.is_empty());
    }
}

#[test]
fn private_emit_local_exact_reserve_and_no_growth() {
    let fixed = named_bytes().unwrap();
    let mut allocator = Allocator::default();
    let admission = Admission::new(fixed + 3, fixed, 0, &mut allocator).unwrap();
    let mut output = OutputMode::Private(admission)
        .allocate(3, EntryPolicy::Result)
        .unwrap_or_else(|failure| panic!("unexpected reserve failure: {failure:?}"));
    output.write_str("abc").unwrap();
    assert_eq!(output.finish(3).unwrap(), "abc");
    assert_eq!(allocator.attempts, 1);
    assert_eq!(allocator.trace.len(), 1);
    assert_eq!(allocator.trace[0].kind, "private LLVM text");
    assert_eq!(allocator.trace[0].length, 3);
    assert!(allocator.trace[0].success);
}

#[test]
fn private_emit_descriptor_keeps_fixed_ceilings() {
    let mut allocator = Allocator::default();
    for (retained, scratch) in [
        (32 * 1024 * 1024 + 1, 16 * 1024 * 1024),
        (32 * 1024 * 1024, 16 * 1024 * 1024 + 1),
        (usize::MAX, usize::MAX),
    ] {
        assert!(matches!(
            Admission::new(retained, scratch, 0, &mut allocator),
            Err(Failure::Budget)
        ));
    }
    assert_eq!(allocator.attempts, 0);
    assert!(allocator.trace.is_empty());
}

#[test]
fn private_emit_preflights_complete_fixed_carriers_before_reserve() {
    let local = named_bytes().unwrap();
    let outside = 137;
    let complete = local + outside;
    for (retained, scratch, outside, counted) in [
        (complete + 3 - 1, complete, outside, 3),
        (complete + 3, complete - 1, outside, 3),
        (local + 3, local, outside, 3),
        (32 * 1024 * 1024, 16 * 1024 * 1024, 0, 32 * 1024 * 1024),
        (32 * 1024 * 1024, 16 * 1024 * 1024, 0, usize::MAX),
        (32 * 1024 * 1024, 16 * 1024 * 1024, usize::MAX, 0),
    ] {
        let mut allocator = Allocator::default();
        let admission = Admission::new(retained, scratch, outside, &mut allocator).unwrap();
        assert!(matches!(
            OutputMode::Private(admission).allocate(counted, EntryPolicy::Result),
            Err(Failure::Budget)
        ));
        assert_eq!(allocator.attempts, 0);
        assert!(allocator.trace.is_empty());
    }
    let mut allocator = Allocator::default();
    let admission = Admission::new(complete + 3, complete, outside, &mut allocator).unwrap();
    let mut output = OutputMode::Private(admission)
        .allocate(3, EntryPolicy::Result)
        .unwrap_or_else(|failure| panic!("unexpected reserve failure: {failure:?}"));
    output.write_str("abc").unwrap();
    assert_eq!(output.finish(3).unwrap(), "abc");
    assert_eq!(allocator.attempts, 1);
}

#[test]
fn private_emit_one_fallible_reserve_has_fixed_failure() {
    let fixed = named_bytes().unwrap();
    let mut allocator = Allocator {
        fail_at: Some(1),
        ..Allocator::default()
    };
    let admission = Admission::new(fixed + 3, fixed, 0, &mut allocator).unwrap();
    assert!(matches!(
        OutputMode::Private(admission).allocate(3, EntryPolicy::Result),
        Err(Failure::Allocation)
    ));
    assert_eq!(allocator.attempts, 1);
    assert_eq!(allocator.trace.len(), 1);
    assert_eq!(allocator.trace[0].kind, "private LLVM text");
    assert_eq!(allocator.trace[0].length, 3);
    assert!(!allocator.trace[0].success);
}

#[test]
fn private_emit_sink_latches_overrun_without_panicking_or_growing() {
    let fixed = named_bytes().unwrap();
    let mut allocator = Allocator::default();
    let admission = Admission::new(fixed + 3, fixed, 0, &mut allocator).unwrap();
    let mut output = OutputMode::Private(admission)
        .allocate(3, EntryPolicy::Result)
        .unwrap_or_else(|failure| panic!("unexpected reserve failure: {failure:?}"));
    output.write_str("ab").unwrap();
    let pointer = output.text.as_ref().unwrap().as_ptr();
    let overrun = "cd";
    write!(output, "{overrun}").unwrap();
    output.write_str("z").unwrap();
    let text = output.text.as_ref().unwrap();
    assert_eq!((text.as_str(), text.len(), text.capacity()), ("ab", 2, 3));
    assert_eq!(text.as_ptr(), pointer);
    assert_eq!(output.len, 2);
    assert!(matches!(output.finish(3), Err(Failure::Invariant)));
    assert_eq!(allocator.attempts, 1);
}

#[test]
fn private_emit_sink_rejects_underrun_capacity_and_count_mismatch() {
    for (capacity, expected, contents, counted) in [
        (3, 3, "ab", 3),
        (4, 3, "abc", 3),
        (2, 3, "abc", 3),
        (3, 3, "abc", 2),
    ] {
        // Synthetic sink fixtures exercise invariant failures only. They are
        // not native modules or allocation-admission substitutes.
        let mut output = Emission {
            len: 0,
            text: Some(String::with_capacity(capacity)),
            policy: EntryPolicy::Result,
            private: Some(RenderLimit {
                expected,
                failed: false,
            }),
        };
        let initial_capacity = output.text.as_ref().unwrap().capacity();
        output.write_str(contents).unwrap();
        assert_eq!(output.text.as_ref().unwrap().capacity(), initial_capacity);
        assert!(matches!(output.finish(counted), Err(Failure::Invariant)));
    }
    let mut missing = Emission {
        private: Some(RenderLimit {
            expected: 1,
            failed: false,
        }),
        ..Emission::default()
    };
    missing.write_str("x").unwrap();
    assert!(matches!(missing.finish(1), Err(Failure::Invariant)));
    let mut overflow = Emission {
        len: usize::MAX,
        text: Some(String::new()),
        policy: EntryPolicy::Result,
        private: Some(RenderLimit {
            expected: usize::MAX,
            failed: false,
        }),
    };
    overflow.write_str("x").unwrap();
    assert!(matches!(
        overflow.finish(usize::MAX),
        Err(Failure::Invariant)
    ));
}

#[test]
fn private_emit_default_sink_keeps_legacy_growth_and_diagnostic_box() {
    let mut output = OutputMode::Default
        .allocate(0, EntryPolicy::Result)
        .unwrap_or_else(|failure| panic!("unexpected default failure: {failure:?}"));
    output.write_str("abc").unwrap();
    assert_eq!(output.finish(3).unwrap(), "abc");
    let diagnostic = reject("unchanged native diagnostic", None);
    let original = diagnostic.as_ref() as *const Diagnostic;
    let Failure::Diagnostic(retained) = Failure::from(diagnostic) else {
        panic!("native diagnostic transport changed");
    };
    assert_eq!(retained.as_ref() as *const Diagnostic, original);
    assert_eq!(retained.message, "unchanged native diagnostic");
}

#[test]
fn private_emit_complete_carriers_are_measured_in_owning_scope() {
    use std::mem::{align_of, size_of};
    eprintln!(
        "private Emit native layouts: admission={}, admission-result={}, mode={}, sink={}/{}, limit={}, failure={}, emission-result={}, string-result={}, named={}",
        size_of::<Admission<'_>>(),
        size_of::<Result<Admission<'_>, Failure>>(),
        size_of::<OutputMode<'_>>(),
        size_of::<Emission>(),
        align_of::<Emission>(),
        size_of::<RenderLimit>(),
        size_of::<Failure>(),
        size_of::<Result<Emission, Failure>>(),
        size_of::<Result<String, Failure>>(),
        named_bytes().unwrap(),
    );
    assert!(named_bytes().unwrap() < 16 * 1024 * 1024);
    assert!(named_bytes().unwrap() > 2 * size_of::<Emission>());
}
