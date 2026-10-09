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
}
