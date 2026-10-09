//! Per-association occurrence uniqueness, released before a witness escapes.
//! One prefix table uses admitted numeric source-owner slots; one dense bitset
//! covers their exact AST expression extents. There is no per-function reset.
use super::*;

/// Closed immutable source-owner selector. Unlike an arbitrary callback its
/// complete inline transport and finite lookup/error paths can be inventoried.
#[derive(Clone, Copy, Debug)]
pub(in crate::frontend::oir) enum ConversionOwners<'a> {
    Original(&'a ast::Program),
    Indexed(crate::frontend::declaration_index::SourceOwner<'a>),
    #[cfg(test)]
    Extents(&'a [usize]),
}
impl ConversionOwners<'_> {
    fn count(self) -> usize {
        match self {
            Self::Original(_) => 1,
            Self::Indexed(sources) => sources.count(),
            #[cfg(test)]
            Self::Extents(extents) => extents.len(),
        }
    }
    fn extent(self, owner: usize) -> Result<usize, Box<Diagnostic>> {
        match self {
            Self::Original(ast) if owner == 0 => Ok(ast.expressions.len()),
            Self::Indexed(sources) => Ok(sources
                .try_ast_borrowed(crate::frontend::project::ModuleId(owner))
                .ok_or_else(bad)?
                .expressions
                .len()),
            #[cfg(test)]
            Self::Extents(extents) => extents.get(owner).copied().ok_or_else(bad),
            _ => Err(bad()),
        }
    }
}

#[derive(Debug, Default)]
pub(in crate::frontend::oir) struct ConversionSeen {
    offsets: Vec<usize>,
    words: Vec<u64>,
}
impl ConversionSeen {
    /// Counted zero-byte route: no tracker payload is allocated.
    pub(in crate::frontend::oir) fn empty() -> Self {
        Self::default()
    }

    pub(in crate::frontend::oir) fn new(
        sources: ConversionOwners<'_>,
        visitor: &mut Visitor<'_>,
    ) -> Result<Self, Box<Diagnostic>> {
        Self::new_impl(sources, None, visitor)
    }

    fn new_impl(
        sources: ConversionOwners<'_>,
        byte_limit: Option<usize>,
        visitor: &mut Visitor<'_>,
    ) -> Result<Self, Box<Diagnostic>> {
        let owners = sources.count();
        let slots = owners.checked_add(1).ok_or_else(resource)?;
        let mut total = 0usize;
        for owner in 0..owners {
            inspect(visitor)?;
            total = total
                .checked_add(sources.extent(owner)?)
                .ok_or_else(resource)?;
        }
        let words = total.checked_add(63).ok_or_else(resource)? / 64;
        let bytes = super::conversion_seen_bytes(owners, total)?;
        if byte_limit.is_some_and(|limit| bytes > limit) {
            return Err(resource());
        }
        let offsets = reserve(slots, visitor)?;
        let bit_words = reserve(words, visitor)?;
        let mut result = Self {
            offsets,
            words: bit_words,
        };
        inspect(visitor)?;
        result.offsets.push(0);
        for owner in 0..owners {
            inspect(visitor)?;
            let end = result
                .offsets
                .last()
                .copied()
                .ok_or_else(bad)?
                .checked_add(sources.extent(owner)?)
                .ok_or_else(resource)?;
            result.offsets.push(end);
        }
        if result.offsets.last().copied() != Some(total) {
            return Err(bad());
        }
        for _ in 0..words {
            inspect(visitor)?;
            result.words.push(0);
        }
        Ok(result)
    }

    pub(in crate::frontend::oir) fn modeled_bytes(&self) -> Result<usize, Box<Diagnostic>> {
        if self.offsets.is_empty() {
            return Ok(0);
        }
        super::conversion_seen_bytes(
            self.offsets.len() - 1,
            *self.offsets.last().ok_or_else(bad)?,
        )
    }

    /// Invoke only after the complete operation/origin/snapshot tuple is valid.
    pub(in crate::frontend::oir) fn mark(
        &mut self,
        owner: usize,
        expression: ast::ExprId,
        visitor: &mut Visitor<'_>,
    ) -> Result<(), Box<Diagnostic>> {
        inspect(visitor)?;
        let start = *self.offsets.get(owner).ok_or_else(bad)?;
        inspect(visitor)?;
        let end = *self
            .offsets
            .get(owner.checked_add(1).ok_or_else(bad)?)
            .ok_or_else(bad)?;
        inspect(visitor)?;
        let bit = start
            .checked_add(expression.0)
            .filter(|bit| *bit < end)
            .ok_or_else(bad)?;
        inspect(visitor)?;
        let word = self.words.get_mut(bit / 64).ok_or_else(bad)?;
        let mask = 1u64 << (bit % 64);
        inspect(visitor)?;
        if *word & mask != 0 {
            return Err(bad());
        }
        *word |= mask;
        Ok(())
    }
}
fn inspect(visitor: &mut Visitor<'_>) -> Result<(), Box<Diagnostic>> {
    increment(&mut visitor.dimensions)
}
fn resource() -> Box<Diagnostic> {
    Diagnostic::new(
        "E0400",
        "oir-project-bind",
        ASSOCIATION_RESOURCE_MESSAGE,
        None,
    )
}
/// Exact requested tracker/header/helper bytes derived from already admitted
/// source extents. The caller includes this live term in its own phase plan.
pub(in crate::frontend::oir) fn conversion_seen_bytes(
    owners: usize,
    expressions: usize,
) -> Result<usize, Box<Diagnostic>> {
    let offsets = owners.checked_add(1).ok_or_else(resource)?;
    let words = expressions.checked_add(63).ok_or_else(resource)? / 64;
    requested_bytes(offsets, words)
}
// Named argument/result/loop/lookup roles used by constructor, fallible
// reservation and O(1) marking. Deliberately additive: it never credits reuse
// of stack slots, optimized-away headers, or allocator implementation details.
#[allow(dead_code)]
struct ByteFormulaCarriers {
    // new_impl's dimensions remain in SeenCarriers while these helpers run.
    formula_parameters: (usize, usize),
    formula_offsets_and_words: (usize, usize),
    formula_checked: [Option<usize>; 2],
    formula_checked_results: [Result<usize, Box<Diagnostic>>; 2],
    requested_call_arguments: (usize, usize),
    requested_parameters: (usize, usize),
    // Seven named results and their checked Option -> Result transports.
    requested_values: [usize; 7],
    requested_checked: [Option<usize>; 7],
    requested_checked_results: [Result<usize, Box<Diagnostic>>; 7],
    requested_return: Result<usize, Box<Diagnostic>>,
    formula_return: Result<usize, Box<Diagnostic>>,
    modeled_owner_count: usize,
    modeled_extent_lookup: Option<&'static usize>,
    modeled_extent_result: Result<&'static usize, Box<Diagnostic>>,
    modeled_call_arguments: (usize, usize),
}
#[allow(dead_code)]
struct SeenCarriers {
    byte_formula: ByteFormulaCarriers,
    owners: usize,
    source_argument: ConversionOwners<'static>, // new argument
    source_forwarded: ConversionOwners<'static>, // new_impl argument
    source_receiver: ConversionOwners<'static>, // count(self) receiver
    extent_receiver: ConversionOwners<'static>, // extent(self, owner) receiver
    indexed_bindings: [crate::frontend::declaration_index::SourceOwner<'static>; 2], // count + extent branches
    original_extent_ast: &'static ast::Program,
    extent_owner_parameter: usize,
    ast_result: Option<&'static ast::Program>,
    source_lookup_receiver: &'static crate::frontend::declaration_index::SourceOwner<'static>,
    source_lookup_header: &'static (),
    source_lookup_ast: &'static ast::Program,
    source_lookup_file: &'static crate::frontend::source::SourceFile,
    source_lookup_return: Option<&'static ast::Program>,
    module: crate::frontend::project::ModuleId,
    visitor: &'static Visitor<'static>,
    byte_limit: Option<usize>,
    slots: usize,
    total: usize,
    words: usize,
    bytes: usize,
    owner_loops: [std::ops::Range<usize>; 2],
    word_loop: std::ops::Range<usize>,
    owner: usize,
    extent_result: Result<usize, Box<Diagnostic>>,
    next_extent: Option<usize>,
    prefix_reservation: Result<Vec<usize>, Box<Diagnostic>>,
    word_reservation: Result<Vec<u64>, Box<Diagnostic>>,
    prefix_reserve_local: Vec<usize>,
    word_reserve_local: Vec<u64>,
    constructor_offsets: Vec<usize>,
    constructor_words: Vec<u64>,
    reserve_visitors: [&'static Visitor<'static>; 2],
    observed_capacities: [usize; 2],
    observed_results: [Result<(), Box<Diagnostic>>; 2],
    modeled_byte_receiver: &'static ConversionSeen,
    modeled_byte_result: Result<usize, Box<Diagnostic>>,
    reserve_counts: [usize; 2],
    reserve_requests: [usize; 2],
    reserve_results: [Result<(), std::collections::TryReserveError>; 2],
    seen_result: Result<ConversionSeen, Box<Diagnostic>>,
    seen_receiver: &'static ConversionSeen,
    owner_slot: usize,
    expression: ast::ExprId,
    start: usize,
    end: usize,
    bit: usize,
    mask: u64,
    offset_lookup: Option<&'static usize>,
    word_lookup: Option<&'static u64>,
    word: &'static u64,
    inspection_result: Result<(), Box<Diagnostic>>,
    marker_result: Result<(), Box<Diagnostic>>,
}
pub(in crate::frontend::oir) const fn conversion_seen_carrier_bytes() -> usize {
    std::mem::size_of::<SeenCarriers>()
}
fn requested_bytes(offsets: usize, words: usize) -> Result<usize, Box<Diagnostic>> {
    let prefix_bytes = offsets
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(resource)?;
    let bit_bytes = words
        .checked_mul(std::mem::size_of::<u64>())
        .ok_or_else(resource)?;
    let payload = prefix_bytes.checked_add(bit_bytes).ok_or_else(resource)?;
    let headers = payload
        .checked_add(std::mem::size_of::<ConversionSeen>())
        .ok_or_else(resource)?;
    let authentication = headers
        .checked_add(conversion_carrier_bytes())
        .ok_or_else(resource)?;
    let helpers = authentication
        .checked_add(super::conversion_seen_carrier_bytes())
        .ok_or_else(resource)?;
    let errors = helpers
        .checked_add(association_error_bytes())
        .ok_or_else(resource)?;
    Ok(errors)
}

fn reserve<T>(count: usize, visitor: &mut Visitor<'_>) -> Result<Vec<T>, Box<Diagnostic>> {
    #[cfg(test)]
    let requested = RESERVE_FAILURE.with(|failure| match failure.get() {
        Some(0) => {
            failure.set(None);
            usize::MAX
        }
        Some(left) => {
            failure.set(Some(left - 1));
            count
        }
        None => count,
    });
    #[cfg(test)]
    let requested = RESERVE_OVERCAPACITY.with(|failure| match failure.get() {
        Some(0) => {
            failure.set(None);
            requested.checked_add(1).unwrap()
        }
        Some(left) => {
            failure.set(Some(left - 1));
            requested
        }
        None => requested,
    });
    #[cfg(not(test))]
    let requested = count;
    let mut values = Vec::new();
    values
        .try_reserve_exact(requested)
        .map_err(|_| resource())?;
    inspect(visitor)?;
    let observed = values.capacity();
    // Both production instantiations are non-ZST (usize/u64). A successful
    // allocation is retained only when its observable capacity matches the
    // admitted payload exactly. Opaque allocator overhead is outside the model.
    if observed != count {
        return Err(resource());
    }
    #[cfg(test)]
    LAST_RESERVED_CAPACITY.set(Some(observed));
    Ok(values)
}
#[cfg(test)]
thread_local! {
    static RESERVE_FAILURE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static RESERVE_OVERCAPACITY: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static LAST_RESERVED_CAPACITY: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(super) fn assert_no_reservation(action: impl FnOnce()) {
    RESERVE_FAILURE.set(Some(0));
    RESERVE_OVERCAPACITY.set(Some(0));
    action();
    assert_eq!(RESERVE_FAILURE.get(), Some(0));
    assert_eq!(RESERVE_OVERCAPACITY.get(), Some(0));
    RESERVE_FAILURE.set(None);
    RESERVE_OVERCAPACITY.set(None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_requested_byte_boundary_and_one_short_precede_reservation() {
        let exact = requested_bytes(4, 3).unwrap(); // owners 65,0,65 =>130 bits
        let mut visitor = Visitor::count();
        let seen = ConversionSeen::new_impl(
            ConversionOwners::Extents(&[65, 0, 65]),
            Some(exact),
            &mut visitor,
        )
        .unwrap();
        assert_eq!((seen.offsets.len(), seen.words.len()), (4, 3));
        assert_eq!(visitor.dimensions, 12); // two owner walks + prefix0 +3 zero words +2 capacities
        assert_no_reservation(|| {
            assert!(ConversionSeen::new_impl(
                ConversionOwners::Extents(&[65, 0, 65]),
                Some(exact - 1),
                &mut Visitor::count()
            )
            .is_err());
        });
        assert!(requested_bytes(usize::MAX, 0).is_err());
        assert!(requested_bytes(0, usize::MAX).is_err());
        assert!(conversion_seen_bytes(usize::MAX, 0).is_err());
        println!("u8_association_seen requested_bytes={exact} offsets_capacity={} words_capacity={} header={}",seen.offsets.capacity(),seen.words.capacity(),std::mem::size_of::<ConversionSeen>());
    }
    #[test]
    fn every_fallible_reserve_failure_leaves_no_tracker_or_witness() {
        for allocation in 0..2 {
            LAST_RESERVED_CAPACITY.set(None);
            RESERVE_FAILURE.set(Some(allocation));
            let error =
                ConversionSeen::new(ConversionOwners::Extents(&[65, 65]), &mut Visitor::count())
                    .unwrap_err();
            assert_eq!((error.code, error.stage), ("E0400", "oir-project-bind"));
            assert_eq!(RESERVE_FAILURE.get(), None);
            // On the second failure the successfully reserved prefix remains
            // in the constructor's `offsets` local until error propagation.
            assert_eq!(LAST_RESERVED_CAPACITY.get(), (allocation == 1).then_some(3));
        }
        assert!(
            ConversionSeen::new(ConversionOwners::Extents(&[65, 65]), &mut Visitor::count())
                .is_ok()
        );
    }
    #[test]
    fn excess_observable_capacity_is_rejected_before_retention() {
        for allocation in 0..2 {
            LAST_RESERVED_CAPACITY.set(None);
            RESERVE_OVERCAPACITY.set(Some(allocation));
            let failure =
                ConversionSeen::new(ConversionOwners::Extents(&[65, 65]), &mut Visitor::count())
                    .unwrap_err();
            assert_eq!((failure.code, failure.stage), ("E0400", "oir-project-bind"));
            assert_eq!(failure.primary, None);
            assert_eq!(RESERVE_OVERCAPACITY.get(), None);
            assert_eq!(LAST_RESERVED_CAPACITY.get(), (allocation == 1).then_some(3));
        }
    }
    #[test]
    fn admitted_aggregate_node_and_owner_extents_have_exact_payload_bound() {
        use std::mem::size_of;
        const OWNERS: usize = 256;
        let nodes = crate::frontend::parser::MAX_NODES;
        let words = nodes.div_ceil(64);
        let prefix_bytes = (OWNERS + 1) * size_of::<usize>();
        let bit_bytes = words * size_of::<u64>();
        let controls = size_of::<ConversionSeen>()
            + conversion_carrier_bytes()
            + conversion_seen_carrier_bytes()
            + association_error_bytes();
        assert_eq!(
            conversion_seen_bytes(OWNERS, nodes).unwrap(),
            controls + prefix_bytes + bit_bytes
        );
        let mut extents = [0; OWNERS];
        extents[OWNERS - 1] = nodes;
        let mut visitor = Visitor::count();
        let seen = ConversionSeen::new(ConversionOwners::Extents(&extents), &mut visitor).unwrap();
        assert_eq!(
            (seen.offsets.capacity(), seen.words.capacity()),
            (OWNERS + 1, words)
        );
        assert_eq!(visitor.dimensions, 2 * OWNERS + 1 + words + 2);
        println!("u8_seen_bound owners={OWNERS} nodes={nodes} prefix={prefix_bytes} bits={bit_bytes} controls={controls} total={} work={} formula_carriers={} formula_align={}", seen.modeled_bytes().unwrap(), visitor.dimensions, size_of::<ByteFormulaCarriers>(), std::mem::align_of::<ByteFormulaCarriers>());
    }
    #[test]
    fn owners_last_bits_duplicates_and_reordered_visits_stay_distinct() {
        let mut seen = ConversionSeen::new(
            ConversionOwners::Extents(&[65, 0, 65]),
            &mut Visitor::count(),
        )
        .unwrap();
        let mut visits = Visitor::count();
        for (owner, id) in [(2, 64), (0, 64), (2, 0), (0, 0), (2, 63), (0, 63)] {
            seen.mark(owner, ast::ExprId(id), &mut visits).unwrap();
        }
        assert_eq!(visits.dimensions, 30);
        for (owner, id) in [
            (0, 64),
            (2, 64),
            (1, 0),
            (0, 65),
            (3, 0),
            (usize::MAX, 0),
            (0, usize::MAX),
        ] {
            assert!(seen.mark(owner, ast::ExprId(id), &mut visits).is_err());
        }
    }
    #[test]
    fn empty_tracker_has_zero_payload_and_no_reservations() {
        RESERVE_FAILURE.set(Some(0));
        let seen = ConversionSeen::empty();
        assert_eq!((seen.offsets.capacity(), seen.words.capacity()), (0, 0));
        assert_eq!(RESERVE_FAILURE.get(), Some(0));
        RESERVE_FAILURE.set(None);
    }
}
