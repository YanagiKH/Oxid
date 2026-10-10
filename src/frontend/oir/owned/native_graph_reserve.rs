//! SOURCE PROPOSAL ONLY: isolated fallible owned-native graph reservations.
//! This module does not select faults, observe allocator operations, or replace
//! the existing native phase Accounting stream. Logical request bytes below are
//! not a claim about the allocator's actual Layout or the returned Vec capacity.
use super::{Diagnostic, Span};
use std::mem::size_of;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GraphFamily {
    Callers,
    Remaining,
    CallerEdges,
    CallReady,
    Bounds,
    Incoming,
    CfgReady,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GraphOperation {
    InitExact,
    InitialQueuePush,
    EdgePush,
    UnlockPush,
    SuccessorPush,
}

// Inline qualification facts are deliberately retained for later independent
// qualification. Only facts unused by release diagnostics get field-level allows.
#[derive(Clone, Copy, Debug)]
pub(super) struct Provenance {
    #[cfg_attr(not(test), allow(dead_code))]
    pub function: Option<usize>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub block: Option<usize>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub target: Option<usize>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub caller: Option<usize>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Site {
    pub family: GraphFamily,
    #[cfg_attr(not(test), allow(dead_code))]
    pub operation: GraphOperation,
    pub provenance: Provenance,
}

impl Site {
    pub(super) fn new(family: GraphFamily, operation: GraphOperation, span: Span) -> Self {
        Self {
            family,
            operation,
            provenance: Provenance {
                function: None,
                block: None,
                target: None,
                caller: None,
                span,
            },
        }
    }

    pub(super) fn function(mut self, function: usize) -> Self {
        self.provenance.function = Some(function);
        self
    }

    pub(super) fn block(mut self, block: usize) -> Self {
        self.provenance.block = Some(block);
        self
    }

    pub(super) fn target(mut self, target: usize) -> Self {
        self.provenance.target = Some(target);
        self
    }

    pub(super) fn caller(mut self, caller: usize) -> Self {
        self.provenance.caller = Some(caller);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Precondition {
    ExactRequiresUnusedVector,
    UnsupportedZeroSizedElement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FailureKind {
    Overflow,
    Allocation,
    Precondition(Precondition),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RequestFacts {
    pub len: usize,
    pub capacity: usize,
    pub additional: usize,
    pub element_bytes: usize,
    // None distinguishes an uncomputed/overflowing fact from a valid zero.
    #[cfg_attr(not(test), allow(dead_code))]
    pub requested_len: Option<usize>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub requested_bytes: Option<usize>,
}

impl RequestFacts {
    fn for_vector<T>(vector: &Vec<T>, additional: usize) -> Self {
        Self {
            len: vector.len(),
            capacity: vector.capacity(),
            additional,
            element_bytes: size_of::<T>(),
            requested_len: None,
            requested_bytes: None,
        }
    }

    fn checked(mut self, site: Site) -> Result<Self, Failure> {
        let Some(length) = self.len.checked_add(self.additional) else {
            return Err(Failure::before_reserve(FailureKind::Overflow, site, self));
        };
        self.requested_len = Some(length);
        let Some(bytes) = length.checked_mul(self.element_bytes) else {
            return Err(Failure::before_reserve(FailureKind::Overflow, site, self));
        };
        self.requested_bytes = Some(bytes);
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Failure {
    pub kind: FailureKind,
    pub site: Site,
    #[cfg_attr(not(test), allow(dead_code))]
    pub ordinal: Option<usize>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub request: RequestFacts,
}

impl Failure {
    fn before_reserve(kind: FailureKind, site: Site, request: RequestFacts) -> Self {
        Self { kind, site, ordinal: None, request }
    }

    fn allocation(site: Site, ordinal: usize, request: RequestFacts) -> Self {
        Self { kind: FailureKind::Allocation, site, ordinal: Some(ordinal), request }
    }

    /// Best-effort public adaptation, called only after the reservation helper
    /// returns. Any future qualification guard must already have been dropped.
    pub(super) fn diagnostic(self) -> Box<Diagnostic> {
        match self.kind {
            FailureKind::Overflow => {
                Diagnostic::new("E0700", "native-admission", "native owned count overflow", None)
            }
            FailureKind::Allocation => {
                let message = match self.site.family {
                    GraphFamily::Callers => "native owned callers storage allocation failed",
                    GraphFamily::Remaining => "native owned remaining storage allocation failed",
                    GraphFamily::CallerEdges => "native owned caller edges storage allocation failed",
                    GraphFamily::CallReady => "native owned call ready storage allocation failed",
                    GraphFamily::Bounds => "native owned bounds storage allocation failed",
                    GraphFamily::Incoming => "native owned CFG incoming storage allocation failed",
                    GraphFamily::CfgReady => "native owned CFG ready storage allocation failed",
                };
                Diagnostic::new("E0700", "native-admission", message, Some(self.site.provenance.span))
            }
            FailureKind::Precondition(
                Precondition::ExactRequiresUnusedVector | Precondition::UnsupportedZeroSizedElement,
            ) => {
                // Internal precondition failures are separate from allocation
                // refusal and deliberately have no source primary span.
                Diagnostic::new(
                    "E0500",
                    "native-admission",
                    "internal compiler error: native graph reservation precondition",
                    None,
                )
            }
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct GraphAllocator {
    attempts: usize,
}

impl GraphAllocator {
    #[cfg(test)]
    pub(super) fn attempts(&self) -> usize {
        self.attempts
    }

    fn begin_request(&mut self, site: Site, request: RequestFacts) -> Result<(usize, RequestFacts), Failure> {
        let request = request.checked(site)?;
        let Some(ordinal) = self.attempts.checked_add(1) else {
            return Err(Failure::before_reserve(FailureKind::Overflow, site, request));
        };
        self.attempts = ordinal;
        Ok((ordinal, request))
    }

    pub(super) fn reserve_exact_empty<T>(
        &mut self,
        vector: &mut Vec<T>,
        count: usize,
        site: Site,
    ) -> Result<(), Failure> {
        let request = RequestFacts::for_vector(vector, count);
        if request.element_bytes == 0 {
            return Err(Failure::before_reserve(
                FailureKind::Precondition(Precondition::UnsupportedZeroSizedElement), site, request,
            ));
        }
        // Both conditions matter: a cleared or preallocated Vec is unsupported,
        // even for count zero. Reject before arithmetic, ordinal, or reserve.
        if request.len != 0 || request.capacity != 0 {
            return Err(Failure::before_reserve(
                FailureKind::Precondition(Precondition::ExactRequiresUnusedVector), site, request,
            ));
        }
        if count == 0 {
            return Ok(());
        }
        let (ordinal, request) = self.begin_request(site, request)?;
        // Exactly one fallible expression. A future qualification guard would
        // wrap only this expression, dropping before inline failure construction.
        let result = vector.try_reserve_exact(count);
        result.map_err(|_| Failure::allocation(site, ordinal, request))
    }

    pub(super) fn push<T>(
        &mut self,
        vector: &mut Vec<T>,
        value: T,
        site: Site,
    ) -> Result<(), Failure> {
        // Rust evaluates the caller's value before entering this helper.
        let request = RequestFacts::for_vector(vector, 1);
        if request.element_bytes == 0 {
            return Err(Failure::before_reserve(
                FailureKind::Precondition(Precondition::UnsupportedZeroSizedElement), site, request,
            ));
        }
        if request.len == request.capacity {
            let (ordinal, request) = self.begin_request(site, request)?;
            // Amortized growth intentionally differs from exact-by-one. The
            // actual allocator request can exceed checked logical len + 1 bytes.
            // A future qualification guard would wrap only this expression.
            let result = vector.try_reserve(1);
            result.map_err(|_| Failure::allocation(site, ordinal, request))?;
        }
        // A successful reserve guarantees spare capacity. On the other path,
        // capacity was already spare and no ordinal or reserve was consumed.
        debug_assert!(vector.len() < vector.capacity());
        vector.push(value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> Span {
        Span { file: crate::frontend::source::SourceFileId(3), start: 7, end: 11 }
    }

    fn exact_site() -> Site {
        Site::new(GraphFamily::Remaining, GraphOperation::InitExact, span()).function(2)
    }

    fn push_site() -> Site {
        Site::new(GraphFamily::CallerEdges, GraphOperation::EdgePush, span())
            .function(2).block(4).target(6).caller(2)
    }

    #[test]
    fn copy_carriers_retain_typed_site_and_request_facts() {
        fn is_copy<T: Copy>() {}
        is_copy::<Site>();
        is_copy::<Provenance>();
        is_copy::<RequestFacts>();
        is_copy::<Failure>();
        let site = push_site();
        assert_eq!(site.family, GraphFamily::CallerEdges);
        assert_eq!(site.operation, GraphOperation::EdgePush);
        assert_eq!(site.provenance.function, Some(2));
        assert_eq!(site.provenance.block, Some(4));
        assert_eq!(site.provenance.target, Some(6));
        assert_eq!(site.provenance.caller, Some(2));
        assert_eq!(site.provenance.span, span());
    }

    #[test]
    fn exact_zero_is_no_attempt_and_nonzero_retains_actual_capacity() {
        let mut allocator = GraphAllocator::default();
        let mut values = Vec::<usize>::new();
        allocator.reserve_exact_empty(&mut values, 0, exact_site()).unwrap();
        assert_eq!((values.len(), values.capacity(), allocator.attempts()), (0, 0, 0));
        allocator.reserve_exact_empty(&mut values, 3, exact_site()).unwrap();
        assert_eq!(allocator.attempts(), 1);
        assert!(values.capacity() >= 3);
        let capacity = values.capacity();
        values.resize(3, 0);
        assert_eq!(values.capacity(), capacity);
        assert_eq!(values.as_slice(), &[0, 0, 0]);
    }

    #[test]
    fn exact_rejects_both_preallocated_empty_and_nonempty_vectors() {
        for nonempty in [false, true] {
            for count in [0, 1] {
                let mut allocator = GraphAllocator::default();
                let mut values = Vec::<usize>::new();
                values.try_reserve_exact(1).unwrap();
                if nonempty { values.push(9); }
                let before = (values.as_ptr(), values.len(), values.capacity());
                let error = allocator.reserve_exact_empty(&mut values, count, exact_site()).unwrap_err();
                assert_eq!(error.kind, FailureKind::Precondition(Precondition::ExactRequiresUnusedVector));
                assert_eq!(error.ordinal, None);
                assert_eq!(allocator.attempts(), 0);
                assert_eq!((values.as_ptr(), values.len(), values.capacity()), before);
                if nonempty { assert_eq!(values.as_slice(), &[9]); }
            }
        }
    }

    #[test]
    fn unsupported_zst_never_consumes_an_ordinal_or_changes_length() {
        let mut allocator = GraphAllocator::default();
        let mut values = Vec::<()>::new();
        let exact = allocator.reserve_exact_empty(&mut values, 0, exact_site()).unwrap_err();
        let push = allocator.push(&mut values, (), push_site()).unwrap_err();
        for error in [exact, push] {
            assert_eq!(error.kind, FailureKind::Precondition(Precondition::UnsupportedZeroSizedElement));
            assert_eq!(error.ordinal, None);
        }
        assert_eq!((allocator.attempts(), values.len()), (0, 0));
    }

    #[test]
    fn push_counts_only_actual_full_vector_reserve_calls() {
        let mut allocator = GraphAllocator::default();
        let mut values = Vec::<usize>::new();
        allocator.push(&mut values, 9, push_site()).unwrap();
        assert_eq!(allocator.attempts(), 1);
        let capacity = values.capacity();
        // Fixed test-setup admission before capacity-dependent filling. Refuse
        // unexpectedly large capacity; never adapt the input or retry setup.
        let bytes = capacity.checked_mul(size_of::<usize>()).expect("test capacity overflow");
        assert!(bytes <= 64 * size_of::<usize>(), "test capacity exceeds 64 slots");
        // Do not assume capacity four, exact returned size, or a growth factor.
        for _ in values.len()..capacity {
            allocator.push(&mut values, 9, push_site()).unwrap();
            assert_eq!(allocator.attempts(), 1);
            assert_eq!(values.capacity(), capacity);
        }
        allocator.push(&mut values, 7, push_site()).unwrap();
        assert_eq!(allocator.attempts(), 2);
        assert_eq!(values.len(), capacity + 1);
        assert!(values[..capacity].iter().all(|value| *value == 9));
        assert_eq!(values[capacity], 7);
    }

    #[test]
    fn spare_push_still_succeeds_when_the_attempt_counter_is_exhausted() {
        let mut allocator = GraphAllocator { attempts: usize::MAX };
        let mut values = Vec::<usize>::new();
        values.try_reserve_exact(1).unwrap();
        let capacity = values.capacity();
        allocator.push(&mut values, 5, push_site()).unwrap();
        assert_eq!(allocator.attempts(), usize::MAX);
        assert_eq!(values.capacity(), capacity);
        assert_eq!(values.as_slice(), &[5]);
    }

    #[test]
    fn graph_contexts_have_independent_ordinals() {
        let mut first = GraphAllocator::default();
        let mut second = GraphAllocator::default();
        first.reserve_exact_empty(&mut Vec::<usize>::new(), 1, exact_site()).unwrap();
        assert_eq!((first.attempts(), second.attempts()), (1, 0));
        second.reserve_exact_empty(&mut Vec::<usize>::new(), 1, exact_site()).unwrap();
        assert_eq!((first.attempts(), second.attempts()), (1, 1));
    }

    #[test]
    fn checked_length_and_byte_overflow_precede_ordinal_consumption() {
        let mut allocator = GraphAllocator::default();
        // Pure arithmetic carrier: constructing an invalid Vec is unnecessary.
        let request = RequestFacts {
            len: usize::MAX, capacity: usize::MAX, additional: 1, element_bytes: 1,
            requested_len: None, requested_bytes: None,
        };
        let length_error = allocator.begin_request(push_site(), request).unwrap_err();
        assert_eq!(length_error.kind, FailureKind::Overflow);
        assert_eq!(length_error.ordinal, None);
        assert_eq!(length_error.request.requested_len, None);
        assert_eq!(length_error.request.requested_bytes, None);
        assert_eq!(allocator.attempts(), 0);
        let mut values = Vec::<usize>::new();
        let bytes_error = allocator.reserve_exact_empty(&mut values, usize::MAX, exact_site()).unwrap_err();
        assert_eq!(bytes_error.kind, FailureKind::Overflow);
        assert_eq!(bytes_error.ordinal, None);
        assert_eq!(bytes_error.request.requested_len, Some(usize::MAX));
        assert_eq!(bytes_error.request.requested_bytes, None);
        assert_eq!((allocator.attempts(), values.len(), values.capacity()), (0, 0, 0));
    }

    #[test]
    fn ordinal_overflow_is_pre_reserve_and_preserves_vector() {
        let mut allocator = GraphAllocator { attempts: usize::MAX };
        let mut values = Vec::<u8>::new();
        let error = allocator.reserve_exact_empty(&mut values, 1, exact_site()).unwrap_err();
        assert_eq!(error.kind, FailureKind::Overflow);
        assert_eq!(error.ordinal, None);
        assert_eq!(error.request.requested_len, Some(1));
        assert_eq!(error.request.requested_bytes, Some(1));
        assert_eq!(allocator.attempts(), usize::MAX);
        assert_eq!((values.len(), values.capacity()), (0, 0));
    }

    #[test]
    fn full_push_ordinal_overflow_preserves_existing_ownership_and_contents() {
        let mut allocator = GraphAllocator { attempts: usize::MAX };
        let mut values = Vec::<usize>::new();
        values.try_reserve_exact(2).unwrap();
        let capacity = values.capacity();
        let bytes = capacity.checked_mul(size_of::<usize>()).expect("test capacity overflow");
        assert!(bytes <= 64 * size_of::<usize>(), "test capacity exceeds 64 slots");
        values.resize(capacity, 9);
        let before = (values.as_ptr(), values.len(), values.capacity());
        let error = allocator.push(&mut values, 7, push_site()).unwrap_err();
        assert_eq!(error.kind, FailureKind::Overflow);
        assert_eq!(error.ordinal, None);
        assert_eq!(allocator.attempts(), usize::MAX);
        assert_eq!((values.as_ptr(), values.len(), values.capacity()), before);
        assert!(values.iter().all(|value| *value == 9));
    }

    #[test]
    fn returned_vec_capacity_error_is_allocation_with_consumed_ordinal() {
        let mut allocator = GraphAllocator::default();
        let mut values = Vec::<u8>::new();
        let before = (values.as_ptr(), values.len(), values.capacity());
        // Logical usize arithmetic succeeds. Vec itself rejects the capacity.
        // This is neither a fault injector nor evidence of a global allocation.
        let error = allocator.reserve_exact_empty(&mut values, usize::MAX, exact_site()).unwrap_err();
        assert_eq!(error.kind, FailureKind::Allocation);
        assert_eq!(error.ordinal, Some(1));
        assert_eq!(error.request.requested_len, Some(usize::MAX));
        assert_eq!(error.request.requested_bytes, Some(usize::MAX));
        assert_eq!(allocator.attempts(), 1);
        assert_eq!((values.as_ptr(), values.len(), values.capacity()), before);
    }

    #[test]
    fn allocation_diagnostics_use_exact_family_literals_and_site_spans() {
        let families = [
            (GraphFamily::Callers, "native owned callers storage allocation failed"),
            (GraphFamily::Remaining, "native owned remaining storage allocation failed"),
            (GraphFamily::CallerEdges, "native owned caller edges storage allocation failed"),
            (GraphFamily::CallReady, "native owned call ready storage allocation failed"),
            (GraphFamily::Bounds, "native owned bounds storage allocation failed"),
            (GraphFamily::Incoming, "native owned CFG incoming storage allocation failed"),
            (GraphFamily::CfgReady, "native owned CFG ready storage allocation failed"),
        ];
        for (family, message) in families {
            let site = Site::new(family, GraphOperation::InitExact, span());
            let request = RequestFacts::for_vector(&Vec::<u8>::new(), 1).checked(site).unwrap();
            let diagnostic = Failure::allocation(site, 1, request).diagnostic();
            assert_eq!((diagnostic.code, diagnostic.stage), ("E0700", "native-admission"));
            assert_eq!(diagnostic.message, message);
            assert_eq!(diagnostic.primary, Some(span()));
            assert!(diagnostic.secondary.is_empty());
            assert!(diagnostic.notes.is_empty());
        }
    }

    #[test]
    fn overflow_and_internal_precondition_diagnostics_remain_distinct() {
        let request = RequestFacts::for_vector(&Vec::<u8>::new(), 1);
        for (kind, code, message) in [
            (FailureKind::Overflow, "E0700", "native owned count overflow"),
            (FailureKind::Precondition(Precondition::ExactRequiresUnusedVector),
             "E0500", "internal compiler error: native graph reservation precondition"),
        ] {
            let diagnostic = Failure::before_reserve(kind, exact_site(), request).diagnostic();
            assert_eq!((diagnostic.code, diagnostic.stage), (code, "native-admission"));
            assert_eq!(diagnostic.message, message);
            assert_eq!(diagnostic.primary, None);
            assert!(diagnostic.secondary.is_empty());
            assert!(diagnostic.notes.is_empty());
        }
    }
}
