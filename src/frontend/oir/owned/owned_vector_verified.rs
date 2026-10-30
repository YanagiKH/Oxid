//! Owning proof for the separate nominal vector domain. This type cannot be
//! passed to either legacy consumer or a source-association constructor.
use super::*;
use crate::frontend::oir::owned::owned_record_codec::bounds::{
    CheckedElementView, OwnedElementBounds,
};

#[derive(Debug)]
pub(in crate::frontend::oir::owned) struct VerifiedOwnedVectorProgram {
    wrapper: RawOwnedVectorProgram,
    bounds: OwnedElementBounds,
    usage: OwnershipUsage,
    seal: OwnedVectorSeal,
}

#[derive(Debug)]
struct OwnedVectorSeal;

/// Actual move, callee, allocator, result and caller transports. The ordinary
/// helper keeps its inherited logical request scope, not an allocator/RSS claim.
struct OwningProofControls {
    wrapper_argument: RawOwnedVectorProgram,
    sources_argument: &'static SourceMap,
    limits: budget::Limits,
    allocator_argument: &'static mut Allocator,
    remaining: RemainingContextBudget<'static>,
    remaining_return: Result<RemainingContextBudget<'static>, OwnedFailure>,
    context: OwnedVectorContext<'static>,
    context_return: Result<OwnedVectorContext<'static>, OwnedFailure>,
    bounds_request: Result<usize, ShapeFailure>,
    mapped_bounds_request: Result<usize, OwnedFailure>,
    usage: OwnershipUsage,
    usage_return: Result<OwnershipUsage, OwnedFailure>,
    selected_elements: CheckedElements<'static>,
    optional_elements: Option<CheckedElements<'static>>,
    retained_bounds: OwnedElementBounds,
    retained_return: Result<usize, ShapeFailure>,
    borrowed_parts: (&'static Declarations, &'static OwnedElementBounds),
    view: CheckedElementView<'static>,
    element: Element<'static>,
    element_return: Result<Element<'static>, ShapeFailure>,
    aggregate: AggregateTy,
    layout_return: Result<Layout, DeclarationError>,
    width_return: Result<usize, DeclarationError>,
    selected_width: usize,
    constructed: VerifiedOwnedVectorProgram,
    returned: Result<VerifiedOwnedVectorProgram, OwnedFailure>,
    caller_return: Result<VerifiedOwnedVectorProgram, OwnedFailure>,
    charged: [Option<usize>; 4],
    selected_bytes: [usize; 4],
}

struct ProofDispatchControls {
    wrapper: RawOwnedVectorProgram,
    sources: &'static SourceMap,
    limits: budget::Limits,
    allocator: &'static mut Allocator,
    returned: Result<VerifiedOwnedVectorProgram, OwnedFailure>,
}
struct AssociatedProofControls {
    associated: crate::frontend::oir::owned::source::association::AssociatedOwnedVector<'static>,
    parts: (RawOwnedVectorProgram, &'static SourceMap),
    returned: Result<VerifiedOwnedVectorProgram, OwnedFailure>,
}

impl VerifiedOwnedVectorProgram {
    pub(in crate::frontend::oir::owned) fn verify(
        wrapper: RawOwnedVectorProgram,
        sources: &SourceMap,
        limits: budget::Limits,
        allocator: &mut Allocator,
    ) -> Result<Self, OwnedFailure> {
        Self::verify_core::<false>(wrapper, sources, limits, allocator)
    }

    pub(in crate::frontend::oir::owned) fn verify_associated(
        associated: crate::frontend::oir::owned::source::association::AssociatedOwnedVector<'_>,
        limits: budget::Limits,
        allocator: &mut Allocator,
    ) -> Result<Self, OwnedFailure> {
        let (wrapper, sources) = associated.into_parts();
        Self::verify_core::<true>(wrapper, sources, limits, allocator)
    }

    // Only the two factories above can select this continuation. A raw caller
    // cannot choose source conversion authority by changing claims or keys.
    fn verify_core<const SOURCE_ASSOCIATED: bool>(
        wrapper: RawOwnedVectorProgram,
        sources: &SourceMap,
        limits: budget::Limits,
        allocator: &mut Allocator,
    ) -> Result<Self, OwnedFailure> {
        // End every borrow of the wrapper before moving it into the owner.
        let (bounds, usage) = {
            let mut remaining = wrapper.remaining_context_budget(limits)?;
            let metadata = remaining
                .metadata
                .checked_sub(size_of::<OwningProofControls>())
                .and_then(|n| {
                    n.checked_sub(
                        super::super::super::owned_record_codec::bounds::element_control_bytes(),
                    )
                })
                .and_then(|n| n.checked_sub(size_of::<ProofDispatchControls>()))
                .and_then(|n| {
                    n.checked_sub(if SOURCE_ASSOCIATED {
                        size_of::<AssociatedProofControls>()
                    } else {
                        0
                    })
                })
                .ok_or_else(|| resource("owned vector owning proof metadata"))?;
            let work = remaining
                .work
                .checked_sub(24)
                .ok_or_else(|| resource("owned vector owning proof work"))?;
            remaining.metadata = metadata;
            remaining.work = work;
            let before_context = limits
                .metadata
                .min(budget::Limits::DEFAULT.metadata)
                .checked_sub(remaining.metadata)
                .ok_or_else(|| resource("owned vector owning context request"))?;
            let context = wrapper.checked_context(sources, &mut remaining, allocator)?;
            // Genuine zero-extension programs retain the original factory and
            // witness. A zero-key program mentioning the type is not legacy.
            if context.is_legacy() {
                return Err(bad());
            }
            // Recover the earlier request from its actual returned control
            // charge and an O(1) finite-table size request. Do not repeat the
            // context request's complete raw type inventory just to observe
            // metadata. The fixed24 visits include these checked transports.
            let controls = context
                .charged_metadata
                .checked_sub(context.bounds_retained)
                .ok_or_else(bad)?;
            let bounds_request =
                CheckedElements::requested_bytes(wrapper.declarations.as_ref().ok_or_else(bad)?)
                    .map_err(bounds_failure)?;
            let context_peak = add(before_context, add(controls, bounds_request)?)?;
            let mut usage = context.probe_ordinary::<SOURCE_ASSOCIATED>(sources, &mut remaining)?;
            let retained = context.bounds_retained;
            let bounds = context.elements.ok_or_else(bad)?.into_owned();
            if bounds.retained_bytes().map_err(bounds_failure)? > retained {
                return Err(resource("owned vector owning retained bounds"));
            }
            // Preserve the larger earlier logical request as well as the
            // retained proof scope. The opaque declaration helper's allowance
            // remains a logical contract, not measured allocator/RSS peak.
            usage.metadata_bytes = usage.metadata_bytes.max(context_peak);
            (bounds, usage)
        };
        Ok(Self {
            wrapper,
            bounds,
            usage,
            seal: OwnedVectorSeal,
        })
    }

    pub(in crate::frontend::oir::owned) fn functions(&self) -> &[RawOwnedFunction] {
        &self.wrapper.program.functions
    }

    pub(in crate::frontend::oir::owned) fn declarations(&self) -> &Declarations {
        self.wrapper
            .declarations
            .as_ref()
            .expect("sealed owned vector facade")
    }

    pub(in crate::frontend::oir::owned) fn elements(&self) -> CheckedElementView<'_> {
        CheckedElementView::for_program(self)
    }

    /// Only the bounds module uses these same-owner parts to issue a view.
    pub(in crate::frontend::oir::owned) fn checked_element_parts(
        &self,
    ) -> (&Declarations, &OwnedElementBounds) {
        (self.declarations(), &self.bounds)
    }

    pub(in crate::frontend::oir::owned) fn raw_program(&self) -> &RawOwnedProgram {
        &self.wrapper.program
    }

    pub(in crate::frontend::oir::owned) fn fixed_builtin_base(&self) -> Option<usize> {
        self.wrapper.fixed_builtin_base()
    }

    pub(in crate::frontend::oir::owned) fn vector_operation(
        &self,
        id: hir::DefId,
    ) -> Option<vector::Function> {
        self.wrapper.old_vector_operation(id)
    }

    pub(in crate::frontend::oir::owned) fn owned_vector_operation(
        &self,
        id: hir::DefId,
    ) -> Option<Function> {
        self.wrapper.owned_vector_operation(id)
    }

    pub(in crate::frontend::oir::owned) fn usage(&self) -> OwnershipUsage {
        self.usage
    }

    /// These allocation-free finite lookups are priced by the consuming plan,
    /// emitter or activation scope, rather than inventing a fresh query budget.
    pub(in crate::frontend::oir::owned) fn aggregate_layout(
        &self,
        aggregate: AggregateTy,
    ) -> Result<Layout, DeclarationError> {
        if let Some(root) = OwnedVectorRoot::from_vector(aggregate) {
            self.elements()
                .element_root(root)
                .map_err(|_| DeclarationError::TypeMismatch)?;
            Ok(OwnedVectorType::storage_layout())
        } else {
            self.declarations().aggregate_layout(aggregate)
        }
    }

    pub(in crate::frontend::oir::owned) fn aggregate_width(
        &self,
        aggregate: AggregateTy,
    ) -> Result<usize, DeclarationError> {
        if let Some(root) = OwnedVectorRoot::from_vector(aggregate) {
            self.elements()
                .element_root(root)
                .map_err(|_| DeclarationError::TypeMismatch)?;
            Ok(OwnedVectorType::storage_width())
        } else {
            self.declarations().aggregate_width(aggregate)
        }
    }
}
