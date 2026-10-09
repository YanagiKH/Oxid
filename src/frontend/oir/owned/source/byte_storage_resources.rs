//! RFC0031 explicit fixed source-fence carrier successor.
//!
//! These roles are distinct from unchanged enclosing ValueTy/Span/Result and
//! static diagnostic formatting carriers. No old inventory bank is reused as
//! spare. The two guards execute sequentially, but both complete schemas are
//! conservatively summed: the caller's field-type-origin selection and the
//! callee's record_field_type exclusion. This is not machine-stack/RSS evidence.
use crate::frontend::oir::{hir::Ty, owned_types::FixedArrayTy};
use std::mem::size_of;

#[allow(dead_code)]
struct ByteFieldGuardRoles {
    // matches! copies the structural descriptor into its guard binding.
    array_capture: FixedArrayTy,
    // element(self) has a separate by-value receiver and returned tag.
    element_receiver: FixedArrayTy,
    element_return: Ty,
    compared_tag: Ty,
    equality_result: bool,
    pattern_result: bool,
    // Includes caller short-circuit selection / callee branch decision.
    selected_condition: bool,
}

pub(super) const fn fixed_bytes() -> usize {
    2 * size_of::<ByteFieldGuardRoles>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::source::{SourceFileId, SourceMap};
    use std::mem::align_of;

    #[test]
    fn byte_storage_fixed_fence_successor_measures_complete_guard_roles() {
        // Independently summed member payload plus exact alignment rounding.
        let members = 2 * size_of::<FixedArrayTy>() + 2 * size_of::<Ty>() + 3;
        let alignment = align_of::<ByteFieldGuardRoles>();
        let rounded = members.div_ceil(alignment) * alignment;
        assert_eq!(size_of::<ByteFieldGuardRoles>(), rounded);
        assert_eq!(fixed_bytes(), 2 * rounded);
        println!("RFC0031 source fence guard members={members} size={} align={alignment} caller+callee={} unchanged-format-backing=0", size_of::<ByteFieldGuardRoles>(), fixed_bytes());
    }

    #[test]
    fn byte_storage_current_fixed_endpoint_preserves_historical_bank_and_cap() {
        use super::super::{HirCounts, HirPlan, MAX_HIR_BYTES};
        let mut sources = SourceMap::new();
        sources.add("byte-storage-fixed-resource.ox".into(), "x".into());
        let at = sources.get(SourceFileId(0)).span(0, 1);
        let plan = HirPlan::calculate(HirCounts::default(), at).unwrap();
        let successor = fixed_bytes();
        let inherited = plan.total.checked_sub(successor).unwrap();
        // Independently executed immutable b5455ad baseline unit probe:
        // c3a_complete_fallible_return_envelopes_and_copies_are_prepaid.
        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(inherited, 157_896);
            assert_eq!(plan.total, 157_896 + successor);
        }
        let current_remaining = MAX_HIR_BYTES.checked_sub(plan.total).unwrap();
        assert_eq!(
            plan.with_dynamic(current_remaining, at).unwrap(),
            MAX_HIR_BYTES
        );
        let one_over = plan.with_dynamic(current_remaining + 1, at).unwrap_err();
        assert_eq!((one_over.code, one_over.stage), ("E0400", "resolve"));
        assert_eq!(one_over.primary, Some(at));
        // The old remaining-byte endpoint is retained as a predecessor value,
        // explicitly denied by current admission rather than silently relabeled.
        let historical_remaining = MAX_HIR_BYTES - inherited;
        assert!(plan.with_dynamic(historical_remaining, at).is_err());
        assert_eq!(historical_remaining - current_remaining, successor);
        println!("RFC0031 fixed endpoint inherited_total={inherited} current_total={} addition={successor} historical_remaining={historical_remaining} current_remaining={current_remaining} unchanged_cap={MAX_HIR_BYTES}", plan.total);
    }
    #[test]
    fn byte_storage_hir_work_is_independently_counted_before_allocation() {
        use super::super::{count_function, HirCounts};
        use crate::frontend::{
            declaration_index::WorkMeter, lexer, parser, project::budget::Allocator,
        };
        for length in [0usize, 1, 1024] {
            let elements = (0..length).map(|_| "b").collect::<Vec<_>>().join(",");
            let text =
                format!("fn f(b:u8)->i32{{let a:[u8;{length}]=[{elements}];return a.len();}}");
            let mut sources = SourceMap::new();
            let id = sources.add("byte-hir-work.ox".into(), text);
            let file = sources.get(id);
            let (ast, _) = parser::parse_typed_counted(
                file,
                lexer::lex(file).unwrap(),
                parser::SourceMode::OwnedCandidate,
                parser::MAX_NODES,
                &mut Allocator::default(),
                &mut parser::SyntaxStorage::default(),
            )
            .unwrap();
            // count_function visits parameter1, body1, statements2;
            // count_expression visits array1 + namesN + length1 and separately
            // debits the N literal edges. Stack cursors are fixed arrays; this
            // count-only traversal contains no reserve or heap construction.
            let demand = 1 + 1 + 2 + (length + 2) + length;
            for limit in [demand - 1, demand] {
                let work = WorkMeter::new(limit as u64);
                let mut counts = HirCounts::default();
                let result = count_function(&ast, &ast.functions[0], &work, &mut counts);
                if limit == demand {
                    result.unwrap();
                    assert_eq!(work.used(), demand as u64);
                    assert_eq!(
                        (
                            counts.parameters,
                            counts.blocks,
                            counts.statements,
                            counts.expressions,
                            counts.array_literals,
                            counts.array_entries
                        ),
                        (1, 1, 2, length + 2, 1, length)
                    );
                } else {
                    let error = result.unwrap_err();
                    assert_eq!(error.message, "declaration index work limit exceeded");
                    // The last visited node is the return's ArrayLength. All
                    // earlier debits fit exactly, so the failure is local and
                    // occurs before any affected HIR allocation.
                    let start = file.text().find("a.len()").unwrap();
                    assert_eq!(
                        error.primary,
                        Some(file.span(start, start + "a.len()".len()))
                    );
                    assert_eq!(work.used(), limit as u64);
                }
            }
        }
    }

    #[test]
    fn byte_storage_raw_count_fill_demand_and_allocation_sites_are_independent() {
        use crate::frontend::oir::owned::{
            source::{budget, byte_storage_tests, lower},
            *,
        };
        use std::mem::size_of;
        for length in [0usize, 1, 1024] {
            let elements = (0..length).map(|_| "b").collect::<Vec<_>>().join(",");
            let text =
                format!("fn f(b:u8)->i32{{let a:[u8;{length}]=[{elements}];return a.len();}}");
            byte_storage_tests::with_raw(&text, |sources, typed, _| {
                // Independently follow lower.rs templates: one parameter local,
                // N name snapshots and len local; two owners (temporary/local);
                // N copies + live/construct/live/move/end/len/cleanup = N+7.
                let expected = size_of::<RawOwnedProgram>()
                    + size_of::<RawOwnedFunction>()
                    + size_of::<ParameterBinding>()
                    + (length + 2) * size_of::<LocalDecl>()
                    + 2 * size_of::<OwnerDecl>()
                    + size_of::<OwnedBlock>()
                    + (length + 7) * size_of::<OwnedStatement>()
                    + length * size_of::<Operand>();
                let exact = budget::Limits {
                    raw_bytes: expected,
                };
                let usage =
                    budget::fail_allocation_after(0, || budget::preflight(typed, exact)).unwrap();
                assert_eq!(usage.raw_bytes, expected);
                let short = budget::fail_allocation_after(0, || {
                    lower::lower_with_limits(
                        typed,
                        budget::Limits {
                            raw_bytes: expected - 1,
                        },
                    )
                })
                .unwrap_err();
                assert_eq!(short.kind, OwnedFailureKind::Resource("source raw payload"));
                // Two top-level vectors; twelve per-function output/map vectors
                // (including counted blocks); one statement vector; one literal
                // operand vector, even when empty. These are exactly the sites
                // lower.rs reserves for this no-record/no-call shape.
                const SITES: usize = 2 + 12 + 1 + 1;
                for fail in 0..SITES {
                    let error = budget::fail_allocation_after(fail, || {
                        lower::lower_with_limits(typed, exact)
                    })
                    .unwrap_err();
                    assert_eq!(
                        error.kind,
                        OwnedFailureKind::Resource("injected source allocation failure"),
                        "length={length}, fail={fail}"
                    );
                }
                let raw =
                    budget::fail_allocation_after(SITES, || lower::lower_with_limits(typed, exact))
                        .unwrap();
                let f = &raw.functions[0];
                assert_eq!(
                    (
                        f.parameters.len(),
                        f.locals.len(),
                        f.owners.len(),
                        f.blocks.len(),
                        f.blocks[0].statements.len()
                    ),
                    (1, length + 2, 2, 1, length + 7)
                );
                verified::verify_owned(raw, sources).unwrap();
            });
        }
    }
}
