//! Reviewer-owned high-owner/low-scalar source controls.
use super::*;
use crate::frontend::oir::owned::source::resource_fixtures;

#[test]
fn reviewer_source_resource_many_empty_owners_combined_slots() {
    for extra in [1, 2] {
        let mut text = String::from("struct E{} fn main()->(){");
        for n in 0..127 {
            text.push_str(&format!("let x{n}=E{{}};"));
        }
        for _ in 0..extra {
            text.push_str("0;");
        }
        text.push_str("return;}");
        let case = resource_fixtures::checked(&text);
        let entry = case.entry;
        let witness = case.witness;
        let sources = case.sources;
        // Every let emits a temporary and binding owner. All empty owners reserve
        // one byte/cell. Only zero expressions and synthetic unit return use scalars.
        let plan = ExecutionPlan::build(&witness).unwrap();
        let usage = plan.function(entry).usage();
        assert_eq!(usage.scalar_slots, 1 + extra);
        assert_eq!(usage.owners, 254);
        assert_eq!(usage.owner_cells, 254);
        assert_eq!(usage.payload_bytes, 254);
        assert_eq!(usage.expanded_cells, 1271 + extra);
        assert_eq!(usage.reference_bytes, 8390 + 8 * extra);
        assert_eq!(usage.native_bytes, 264 + 8 * extra);
        assert_eq!(execute::run(&witness, Some(entry)).unwrap(), Scalar::Unit);
        if extra == 1 {
            let bounds = admit(&plan, Limits::DEFAULT).unwrap();
            assert_eq!(bounds[entry.0].cost + 1, 2800);
            assert!(native_module(&witness, Some(entry), &sources).is_ok());
        } else {
            let failure = admit(&plan, Limits::DEFAULT).unwrap_err();
            assert_eq!(failure.code, "E0700");
            assert!(failure
                .message
                .contains("scalar and owner slots per function"));
        }
    }
}
