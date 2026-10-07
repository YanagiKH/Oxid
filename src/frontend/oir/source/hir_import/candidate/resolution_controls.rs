use super::super::super::{Wire, OPA_BYTES, SUCCESS_BYTES};
use super::*;

fn next_row_of_kind(bytes: &[u8], kind: u8, after: u8) -> u8 {
    (after + 1..=bytes[8])
        .find(|&reference| bytes[COLUMN_STARTS[0] + usize::from(reference - 1)] & 63 == kind)
        .unwrap()
}

// Read the supplied words directly. Targets below come from OPA row kinds and
// links, never from the canonical resolver or a candidate HIR owner.
fn supplied_word(bytes: &[u8], column: usize, reference: u8) -> i32 {
    assert!((1..=bytes[8]).contains(&reference));
    i32::from_le_bytes(std::array::from_fn(|plane| {
        bytes[COLUMN_STARTS[column] + plane * CELLS + usize::from(reference - 1)]
    }))
}

fn mismatch_after_resolution_change(
    fixture: (&str, &[u8]),
    reference: u8,
    value: i32,
    reserves: usize,
) {
    let (source, original) = fixture;
    assert_ne!(supplied_word(original, 3, reference), value);
    let mut changed = original.to_vec();
    set_resolution(&mut changed, reference, value);
    assert_eq!(supplied_word(&changed, 3, reference), value);
    assert!(Wire::decode(&changed, source.len()).is_ok());
    assert_eq!(&changed[..OPA_BYTES], &original[..OPA_BYTES]);
    for (index, (&before, &after)) in original.iter().zip(&changed).enumerate() {
        let changed_cell = (0..4)
            .any(|plane| index == COLUMN_STARTS[3] + plane * CELLS + usize::from(reference - 1));
        if !changed_cell {
            assert_eq!(
                before, after,
                "only the selected resolution cell may change"
            );
        }
    }

    let mut allocator = allocator();
    let Err(leaf::Rejected::Mismatch(facts)) =
        compare_fixture(source, &changed, &mut allocator, IndexLimits::default())
    else {
        panic!("plausible supplied resolution must reach comparison and reject without repair");
    };
    assert!(!facts.candidate.equal);
    assert_eq!(facts.candidate.allocation.reserves, reserves);
    assert_eq!(allocator.attempts, reserves);
    assert_eq!(allocator.trace.len(), reserves);
    assert!(allocator.trace.iter().all(|event| event.success));
    assert!(!allocator.observer_trace_overflow);
    // These fixed facts prove contained comparison, not typed or executable
    // admission, and do not claim independent live-allocation evidence.
}

#[test]
fn checked_hir_import_candidate_wrong_same_function_binding_is_not_repaired() {
    // Comparison has two i32 parameters; arithmetic has only one parameter.
    let fixture = SCALARS[2];
    let wire = fixture.1;
    assert_eq!(
        (1..=wire[8])
            .filter(|&reference| wire[COLUMN_STARTS[0] + usize::from(reference - 1)] & 63 == 1)
            .count(),
        1
    );
    let first_parameter = row_reference(wire, 2);
    let second_parameter = next_row_of_kind(wire, 2, first_parameter);
    let name = row_reference(wire, 19);
    assert_eq!(supplied_word(wire, 3, name), i32::from(first_parameter));
    mismatch_after_resolution_change(fixture, name, i32::from(second_parameter), 7);
}

#[test]
fn checked_hir_import_candidate_wrong_callee_is_not_repaired() {
    let f = row_reference(RICH.1, 1);
    let main = next_row_of_kind(RICH.1, 1, f);
    let call = row_reference(RICH.1, 20);
    assert_eq!(supplied_word(RICH.1, 3, call), i32::from(f));
    mismatch_after_resolution_change(RICH, call, i32::from(main), 16);
}

#[test]
fn checked_hir_import_candidate_wrong_loop_targets_are_not_repaired() {
    let fixture = SCALARS[4];
    let wire = fixture.1;
    let while_row = row_reference(wire, 14);
    let if_row = row_reference(wire, 13);
    let while_body = u8::try_from(supplied_word(wire, 1, while_row) >> 8).unwrap();
    let then_body = u8::try_from(supplied_word(wire, 1, if_row) >> 8).unwrap();
    let else_body = u8::try_from(supplied_word(wire, 2, if_row) & 255).unwrap();
    for (kind, wrong_block) in [(11, then_body), (12, else_body)] {
        let exit = row_reference(wire, kind);
        assert_eq!(supplied_word(wire, 0, wrong_block) & 63, 5);
        assert_ne!(wrong_block, while_body);
        assert_eq!(supplied_word(wire, 3, exit), i32::from(while_body));
        // Each inner block is an in-function block reference, but neither is
        // the enclosing while body selected by the source break/continue.
        mismatch_after_resolution_change(fixture, exit, i32::from(wrong_block), 10);
    }
}

#[test]
fn checked_hir_import_candidate_wrong_let_annotation_is_not_repaired() {
    let fixture = SCALARS[5];
    let wire = fixture.1;
    let mutable_let = row_reference(wire, 7);
    let annotation = u8::try_from(supplied_word(wire, 1, mutable_let) >> 8).unwrap();
    assert_eq!(supplied_word(wire, 0, annotation) & 63, 3);
    assert_eq!(supplied_word(wire, 3, annotation), 2); // supplied i32
    mismatch_after_resolution_change(fixture, annotation, 1, 7); // supplied bool
}

#[test]
fn checked_hir_import_candidate_empty_valid_source_stays_contained() {
    let mut wire = [0; SUCCESS_BYTES];
    wire[..4].copy_from_slice(b"OPA1");
    wire[OPA_BYTES..OPA_BYTES + 4].copy_from_slice(b"STF1");
    let mut allocator = allocator();
    let facts = compare_fixture("", &wire, &mut allocator, IndexLimits::default()).unwrap();
    assert!(facts.candidate.equal);
    assert_eq!(facts.candidate.allocation.requested.0, [0; 8]);
    assert_eq!(facts.candidate.allocation.vectors, 2);
    assert_eq!(facts.candidate.allocation.reserves, 0);
    assert_eq!(allocator.attempts, 0);
    assert!(allocator.trace.is_empty());
    assert!(!allocator.observer_trace_overflow);
}
