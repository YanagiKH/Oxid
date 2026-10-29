//! Paid declaration facts for the exact borrowed context. These facts do not
//! grant ordinary ownership, a plan, a source type or resource operations.
use super::*;

/// Existing resource identities retain their exact old class. The new vector
/// is a distinct class, never a ByteBuf or a scalar/Copy-record vector tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend::oir::owned) enum DirectResourceKind {
    Existing(ResourceKind),
    OwnedRecordVector(RecordId),
}

/// Facts retain the exact context borrow; a nominal spelling cannot substitute
/// a different wrapper, facade or checked finite bounds table.
#[derive(Clone, Copy)]
pub(in crate::frontend::oir::owned) struct AggregateFacts<'context, 'program> {
    context: &'context OwnedVectorContext<'program>,
    aggregate: AggregateTy,
    layout: Layout,
    width: usize,
    copy: bool,
    contains_resources: bool,
    direct_resource: Option<DirectResourceKind>,
}

struct QueryControls {
    context: &'static OwnedVectorContext<'static>,
    budget: &'static mut RemainingContextBudget<'static>,
    aggregate: AggregateTy,
    facade: &'static Declarations,
    facade_option: Option<&'static Declarations>,
    facade_result: Result<&'static Declarations, OwnedFailure>,
    bounds: &'static CheckedElements<'static>,
    bounds_option: Option<&'static CheckedElements<'static>>,
    bounds_reference_result: Result<&'static CheckedElements<'static>, OwnedFailure>,
    nominal: OwnedRecordVectorType<'static>,
    nominal_result: Result<OwnedRecordVectorType<'static>, DeclarationError>,
    mapped_nominal_result: Result<OwnedRecordVectorType<'static>, OwnedFailure>,
    nominal_work: usize,
    element: Element<'static>,
    element_result: Result<Element<'static>, ShapeFailure>,
    mapped_element_result: Result<Element<'static>, OwnedFailure>,
    layout: Layout,
    layout_result: Result<Layout, DeclarationError>,
    mapped_layout_result: Result<Layout, OwnedFailure>,
    width: usize,
    width_result: Result<usize, DeclarationError>,
    mapped_width_result: Result<usize, OwnedFailure>,
    resource_result: Result<bool, DeclarationError>,
    mapped_resource_result: Result<bool, OwnedFailure>,
    copy: bool,
    contains_resources: bool,
    resource_kind: DirectResourceKind,
    optional_resource: Option<DirectResourceKind>,
    ordinal: Result<u32, std::num::TryFromIntError>,
    mapped_ordinal: Result<u32, DeclarationError>,
    owned_ordinal_result: Result<u32, OwnedFailure>,
    selected_ordinal: u32,
    request_result: Result<(usize, usize), OwnedFailure>,
    request_caller_result: Result<(usize, usize), OwnedFailure>,
    request: (usize, usize),
    checked_work: Option<usize>,
    checked_metadata: Option<usize>,
    selected_remaining: [usize; 2],
    constructed: AggregateFacts<'static, 'static>,
    callee_return: Result<AggregateFacts<'static, 'static>, OwnedFailure>,
    caller_return: Result<AggregateFacts<'static, 'static>, OwnedFailure>,
    identity: [bool; 3],
}

impl<'program> OwnedVectorContext<'program> {
    pub(in crate::frontend::oir::owned) fn aggregate_query_request(
    ) -> Result<(usize, usize), OwnedFailure> {
        // Three nominal visits, four finite-bound lookups and five direct
        // identity/resource/construction visits cover either concrete path.
        Ok((
            add(
                size_of::<QueryControls>(),
                OwnedRecordVectorType::requested_query_bytes(),
            )?,
            12,
        ))
    }

    /// One concrete paid helper. Caller-owned returned facts and complete
    /// synchronous query controls remain charged to the shared ledger; no
    /// allocation, active-value walk or implicit fresh budget occurs here.
    pub(in crate::frontend::oir::owned) fn aggregate_facts<'context>(
        &'context self,
        aggregate: AggregateTy,
        remaining: &mut RemainingContextBudget<'program>,
    ) -> Result<AggregateFacts<'context, 'program>, OwnedFailure> {
        if !std::ptr::eq(self.wrapper, remaining.wrapper) {
            return Err(bad());
        }
        let (metadata, work) = Self::aggregate_query_request()?;
        let metadata = remaining
            .metadata
            .checked_sub(metadata)
            .ok_or_else(|| resource("owned vector aggregate query metadata"))?;
        let work = remaining
            .work
            .checked_sub(work)
            .ok_or_else(|| resource("owned vector aggregate query work"))?;
        remaining.metadata = metadata;
        remaining.work = work;
        let declarations = self.wrapper.declarations.as_ref().ok_or_else(bad)?;
        let direct_resource = match aggregate {
            AggregateTy::ByteBuf => Some(DirectResourceKind::Existing(ResourceKind::ByteBuf)),
            AggregateTy::File => Some(DirectResourceKind::Existing(ResourceKind::File)),
            AggregateTy::Vector(element) => {
                Some(DirectResourceKind::Existing(ResourceKind::Vector(element)))
            }
            AggregateTy::RecordVector(record) => {
                Some(DirectResourceKind::Existing(ResourceKind::RecordVector(
                    u32::try_from(record.0)
                        .map_err(|_| DeclarationError::InvalidRecordId(record))?,
                )))
            }
            AggregateTy::OwnedRecordVector(record) => {
                Some(DirectResourceKind::OwnedRecordVector(record))
            }
            AggregateTy::Record(_) | AggregateTy::Enum(_) | AggregateTy::FixedArray(_) => None,
        };
        let (layout, width, copy, contains_resources) =
            if let Some(DirectResourceKind::OwnedRecordVector(record)) = direct_resource {
                let mut nominal_work = 3;
                let nominal = OwnedRecordVectorType::check(
                    declarations,
                    record,
                    OwnedRecordVectorType::requested_query_bytes(),
                    &mut nominal_work,
                )?;
                let element = self
                    .elements
                    .as_ref()
                    .ok_or_else(bad)?
                    .element(record)
                    .map_err(bounds_failure)?;
                if nominal.aggregate() != aggregate
                    || !nominal.belongs_to(declarations)
                    || !std::ptr::eq(element.declarations(), declarations)
                {
                    return Err(bad());
                }
                (
                    nominal.descriptor_layout(),
                    nominal.descriptor_width(),
                    false,
                    true,
                )
            } else {
                (
                    declarations.aggregate_layout(aggregate)?,
                    declarations.aggregate_width(aggregate)?,
                    declarations.is_copy(aggregate),
                    declarations.contains_resources(aggregate)?,
                )
            };
        Ok(AggregateFacts {
            context: self,
            aggregate,
            layout,
            width,
            copy,
            contains_resources,
            direct_resource,
        })
    }
}

impl AggregateFacts<'_, '_> {
    pub(in crate::frontend::oir::owned) fn belongs_to(
        &self,
        context: &OwnedVectorContext<'_>,
    ) -> bool {
        std::ptr::eq(self.context, context)
    }
    pub(in crate::frontend::oir::owned) const fn aggregate(self) -> AggregateTy {
        self.aggregate
    }
    pub(in crate::frontend::oir::owned) const fn layout(self) -> Layout {
        self.layout
    }
    pub(in crate::frontend::oir::owned) const fn width(self) -> usize {
        self.width
    }
    pub(in crate::frontend::oir::owned) const fn is_copy(self) -> bool {
        self.copy
    }
    pub(in crate::frontend::oir::owned) const fn contains_resources(self) -> bool {
        self.contains_resources
    }
    /// This is the class of one direct descriptor, not an active-leaf iterator
    /// for a record or enum containing resource fields/payloads.
    pub(in crate::frontend::oir::owned) const fn direct_resource(
        self,
    ) -> Option<DirectResourceKind> {
        self.direct_resource
    }
}
