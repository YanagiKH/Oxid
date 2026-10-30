//! Memoized finite type bounds for the separate owned-record vector lane.
//! Every nominal is evaluated once; shared enum alternatives do not expand a
//! declaration DAG into an exponential tree. Heap operations allocate nothing.
use super::{AggregateTy, Declarations, Element, RecordId, ValueTy, MAX_CONTAINMENT_DEPTH};
use crate::frontend::oir::owned_types::{EnumId, OwnedVectorRoot, OwnedVectorRootSlot};
use crate::frontend::project::budget::{Allocator, ReserveFailure};
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Bounds {
    visits: usize,
    resources: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend::oir::owned) enum ShapeFailure {
    MetadataLimit,
    Allocation,
    Type,
    UnsupportedElement,
}
type Result<T> = std::result::Result<T, ShapeFailure>;

/// A separate checked table; it borrows the exact declaration facade whose
/// identities/layouts it authenticated. Its retained capacity and temporary
/// state/control requests must be admitted by the new lane's shared caller.
#[derive(Debug)]
pub(in crate::frontend::oir::owned) struct CheckedElements<'a> {
    declarations: &'a Declarations,
    bounds: Vec<Bounds>,
}

/// Independently owned finite data, containing no borrowed facade.
#[derive(Debug)]
pub(in crate::frontend::oir::owned) struct OwnedElementBounds {
    bounds: Vec<Bounds>,
}

/// No constructor accepts a caller-selected facade/table pair.
#[derive(Clone, Copy)]
pub(in crate::frontend::oir::owned) struct CheckedElementView<'a> {
    declarations: &'a Declarations,
    bounds: &'a [Bounds],
}

#[derive(Clone, Copy)]
struct Frame {
    aggregate: AggregateTy,
    next: usize,
    bounds: Bounds,
}
type Stack = [Option<Frame>; MAX_CONTAINMENT_DEPTH + 1];

struct FactoryControls {
    arguments: [&'static Declarations; 2],
    output: CheckedElements<'static>,
    output_result: Result<CheckedElements<'static>>,
    state: Vec<u8>,
    stacks: [Stack; 2],
    selected_frames: [Frame; 3],
    selected_bounds: [Bounds; 3],
    child: Option<ValueTy>,
    child_result: Result<Option<Bounds>>,
    layout_result: std::result::Result<super::super::Layout, super::super::DeclarationError>,
    checked_products: [Option<usize>; 8],
    selected_sizes: [usize; 12],
    allocator: Allocator,
    reserve_result: std::result::Result<(), ReserveFailure>,
}

/// Complete borrowed lookup transports shared by the authenticated root
/// selector. Callers prepay this bank; no lookup allocates a new table.
#[repr(C)]
struct ElementQueryCarriers {
    declarations: &'static Declarations,
    bounds: &'static [Bounds],
    argument: OwnedVectorRoot,
    caller_root: OwnedVectorRoot,
    aggregate: AggregateTy,
    checked_slot: Option<OwnedVectorRootSlot>,
    slot: OwnedVectorRootSlot,
    ordinal: usize,
    ordinal_result: Result<usize>,
    validation: std::result::Result<(), super::super::DeclarationError>,
    layout: super::super::Layout,
    layout_return: std::result::Result<super::super::Layout, super::super::DeclarationError>,
    selected: &'static Bounds,
    selected_return: Option<&'static Bounds>,
    bytes: usize,
    copy_exclusion: bool,
    constructed: Element<'static>,
    returned: Result<Element<'static>>,
    caller_return: Result<Element<'static>>,
}
pub(in crate::frontend::oir::owned) const fn element_control_bytes() -> usize {
    size_of::<ElementQueryCarriers>()
}

pub(in crate::frontend::oir::owned) const fn control_bytes() -> usize {
    size_of::<FactoryControls>() + element_control_bytes()
}

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(ShapeFailure::MetadataLimit)
}

impl<'a> CheckedElements<'a> {
    /// Complete request before either vector allocation or declaration walks.
    pub(in crate::frontend::oir::owned) fn requested_bytes(
        declarations: &Declarations,
    ) -> Result<usize> {
        let count = add(
            declarations.records().len(),
            declarations.enums().enums().len(),
        )?;
        add(
            count
                .checked_mul(size_of::<Bounds>() + size_of::<u8>())
                .ok_or(ShapeFailure::MetadataLimit)?,
            control_bytes(),
        )
    }

    pub(in crate::frontend::oir::owned) fn check(
        declarations: &'a Declarations,
        limit: usize,
    ) -> Result<Self> {
        Self::check_with_allocator(declarations, limit, &mut Allocator::default())
    }

    pub(in crate::frontend::oir::owned) fn check_with_allocator(
        declarations: &'a Declarations,
        limit: usize,
        allocator: &mut Allocator,
    ) -> Result<Self> {
        let limit = limit.min(super::super::plan::MAX_PLAN_BYTES);
        if Self::requested_bytes(declarations)? > limit {
            return Err(ShapeFailure::MetadataLimit);
        }
        let count = add(
            declarations.records().len(),
            declarations.enums().enums().len(),
        )?;
        let mut bounds = Vec::new();
        if count != 0 {
            allocator
                .vector_exact(&mut bounds, count, "owned vector type bounds")
                .map_err(|_| ShapeFailure::Allocation)?;
        }
        let bounds_bytes = bounds
            .capacity()
            .checked_mul(size_of::<Bounds>())
            .ok_or(ShapeFailure::MetadataLimit)?;
        if add(add(bounds_bytes, count)?, control_bytes())? > limit {
            return Err(ShapeFailure::MetadataLimit);
        }
        let mut states = Vec::new();
        if count != 0 {
            allocator
                .vector_exact(&mut states, count, "owned vector type states")
                .map_err(|_| ShapeFailure::Allocation)?;
        }
        if add(add(bounds_bytes, states.capacity())?, control_bytes())? > limit {
            return Err(ShapeFailure::MetadataLimit);
        }
        bounds.resize(count, Bounds::default());
        states.resize(count, 0u8);
        let mut table = Self {
            declarations,
            bounds,
        };
        for root in 0..count {
            if states[root] == 2 {
                continue;
            }
            let aggregate = table.aggregate(root)?;
            let mut stack = [None; MAX_CONTAINMENT_DEPTH + 1];
            stack[0] = Some(Frame::new(aggregate)?);
            states[root] = 1;
            let mut length = 1;
            while length != 0 {
                let position = length - 1;
                let mut frame = stack[position].ok_or(ShapeFailure::Type)?;
                let row = table.ordinal(frame.aggregate)?;
                let child = match frame.aggregate {
                    AggregateTy::Record(record) => table
                        .declarations
                        .fields(record)
                        .map_err(|_| ShapeFailure::Type)?
                        .get(frame.next)
                        .map(|field| Some(field.value_ty())),
                    AggregateTy::Enum(enumeration) => table
                        .declarations
                        .enums()
                        .variants(enumeration)
                        .map_err(|_| ShapeFailure::Type)?
                        .get(frame.next)
                        .map(|variant| variant.value_payload()),
                    _ => return Err(ShapeFailure::Type),
                };
                let Some(child) = child else {
                    // A finite ordinary nominal may contain an owned vector
                    // while being ineligible as a nested vector element. The
                    // active exclusion bit is sticky across every enum arm.
                    table.bounds[row] = if states[row] == 3 {
                        Bounds::default()
                    } else {
                        frame.bounds
                    };
                    states[row] = 2;
                    length -= 1;
                    continue;
                };
                if let Some(child) = child {
                    let bound = match table.leaf(child)? {
                        Some(bound) => bound,
                        None => {
                            let ValueTy::Owned(aggregate) = child else {
                                return Err(ShapeFailure::Type);
                            };
                            let child_row = table.ordinal(aggregate)?;
                            match states[child_row] {
                                2 => table.bounds[child_row],
                                0 => {
                                    if length == stack.len() {
                                        return Err(ShapeFailure::Type);
                                    }
                                    states[child_row] = 1;
                                    stack[length] = Some(Frame::new(aggregate)?);
                                    length += 1;
                                    continue;
                                }
                                _ => return Err(ShapeFailure::Type),
                            }
                        }
                    };
                    if bound.visits == 0 {
                        states[row] = 3;
                    }
                    match frame.aggregate {
                        AggregateTy::Record(_) => {
                            frame.bounds.visits = add(frame.bounds.visits, add(1, bound.visits)?)?;
                            frame.bounds.resources = add(frame.bounds.resources, bound.resources)?;
                        }
                        AggregateTy::Enum(_) => {
                            frame.bounds.visits = frame.bounds.visits.max(add(2, bound.visits)?);
                            frame.bounds.resources = frame.bounds.resources.max(bound.resources);
                        }
                        _ => return Err(ShapeFailure::Type),
                    }
                }
                frame.next = add(frame.next, 1)?;
                stack[position] = Some(frame);
            }
        }
        Ok(table)
    }

    pub(in crate::frontend::oir::owned) fn retained_bytes(&self) -> Result<usize> {
        add(
            size_of::<Self>(),
            self.bounds
                .capacity()
                .checked_mul(size_of::<Bounds>())
                .ok_or(ShapeFailure::MetadataLimit)?,
        )
    }

    pub(in crate::frontend::oir::owned) fn belongs_to(&self, declarations: &Declarations) -> bool {
        std::ptr::eq(self.declarations, declarations)
    }

    pub(in crate::frontend::oir::owned) fn element(&self, record: RecordId) -> Result<Element<'a>> {
        element(self.declarations, &self.bounds, record)
    }

    pub(in crate::frontend::oir::owned) fn element_root(
        &self,
        root: OwnedVectorRoot,
    ) -> Result<Element<'a>> {
        element_root(self.declarations, &self.bounds, root)
    }

    pub(in crate::frontend::oir::owned) fn view(&self) -> CheckedElementView<'_> {
        CheckedElementView {
            declarations: self.declarations,
            bounds: &self.bounds,
        }
    }

    pub(in crate::frontend::oir::owned) fn into_owned(self) -> OwnedElementBounds {
        OwnedElementBounds {
            bounds: self.bounds,
        }
    }

    fn ordinal(&self, aggregate: AggregateTy) -> Result<usize> {
        match aggregate {
            AggregateTy::Record(record) if record.0 < self.declarations.records().len() => {
                Ok(record.0)
            }
            AggregateTy::Enum(enumeration)
                if enumeration.0 < self.declarations.enums().enums().len() =>
            {
                add(self.declarations.records().len(), enumeration.0)
            }
            _ => Err(ShapeFailure::Type),
        }
    }

    fn aggregate(&self, ordinal: usize) -> Result<AggregateTy> {
        if ordinal < self.declarations.records().len() {
            Ok(AggregateTy::Record(RecordId(ordinal)))
        } else if ordinal < self.bounds.len() {
            Ok(AggregateTy::Enum(EnumId(
                ordinal - self.declarations.records().len(),
            )))
        } else {
            Err(ShapeFailure::Type)
        }
    }

    fn leaf(&self, value: ValueTy) -> Result<Option<Bounds>> {
        Ok(Some(match value {
            // Zero visits is an ineligible element subtree, not a malformed
            // ordinary declaration. Parents propagate it through DAG edges.
            ValueTy::Owned(AggregateTy::OwnedRecordVector(_) | AggregateTy::OwnedEnumVector(_)) => {
                Bounds::default()
            }
            ValueTy::Scalar(_) => Bounds {
                visits: 1,
                resources: 0,
            },
            ValueTy::Owned(
                AggregateTy::ByteBuf
                | AggregateTy::File
                | AggregateTy::Vector(_)
                | AggregateTy::RecordVector(_),
            ) => Bounds {
                visits: 1,
                resources: 1,
            },
            ValueTy::Owned(AggregateTy::FixedArray(array)) => Bounds {
                visits: add(array.length().max(1), 1)?,
                resources: 0,
            },
            ValueTy::Owned(AggregateTy::Record(_) | AggregateTy::Enum(_)) => return Ok(None),
        }))
    }
}

impl OwnedElementBounds {
    pub(in crate::frontend::oir::owned) fn retained_bytes(&self) -> Result<usize> {
        add(
            size_of::<Self>(),
            self.bounds
                .capacity()
                .checked_mul(size_of::<Bounds>())
                .ok_or(ShapeFailure::MetadataLimit)?,
        )
    }
}

impl<'a> CheckedElementView<'a> {
    /// The proof owner is the only source of newly bound owned-data views.
    pub(in crate::frontend::oir::owned) fn for_program(
        owner: &'a super::super::verified::VerifiedOwnedVectorProgram,
    ) -> Self {
        let (declarations, bounds) = owner.checked_element_parts();
        Self {
            declarations,
            bounds: &bounds.bounds,
        }
    }

    pub(in crate::frontend::oir::owned) fn belongs_to(self, declarations: &Declarations) -> bool {
        std::ptr::eq(self.declarations, declarations)
    }

    pub(in crate::frontend::oir::owned) fn element(self, record: RecordId) -> Result<Element<'a>> {
        element(self.declarations, self.bounds, record)
    }

    pub(in crate::frontend::oir::owned) fn element_root(
        self,
        root: OwnedVectorRoot,
    ) -> Result<Element<'a>> {
        element_root(self.declarations, self.bounds, root)
    }
}

fn element<'a>(
    declarations: &'a Declarations,
    bounds: &[Bounds],
    record: RecordId,
) -> Result<Element<'a>> {
    element_root(declarations, bounds, OwnedVectorRoot::Record(record))
}

fn element_root<'a>(
    declarations: &'a Declarations,
    bounds: &[Bounds],
    root: OwnedVectorRoot,
) -> Result<Element<'a>> {
    let aggregate = root.aggregate();
    let slot = OwnedVectorRootSlot::check(root).ok_or(ShapeFailure::Type)?;
    let ordinal = match root {
        OwnedVectorRoot::Record(id) => id.0,
        OwnedVectorRoot::Enum(id) => add(declarations.records().len(), id.0)?,
    };
    declarations
        .check_aggregate_type(aggregate)
        .map_err(|_| ShapeFailure::Type)?;
    if declarations.is_copy(aggregate) {
        return Err(ShapeFailure::Type);
    }
    let bytes = declarations
        .aggregate_layout(aggregate)
        .map_err(|_| ShapeFailure::Type)?
        .size();
    let bound = bounds.get(ordinal).ok_or(ShapeFailure::Type)?;
    if bytes == 0 || bound.resources > bytes / 16 {
        return Err(ShapeFailure::Type);
    }
    if bound.visits == 0 {
        return Err(ShapeFailure::UnsupportedElement);
    }
    Ok(Element {
        declarations,
        root: slot,
        bytes,
        visits: bound.visits,
        resources: bound.resources,
    })
}

impl Frame {
    fn new(aggregate: AggregateTy) -> Result<Self> {
        Ok(Self {
            aggregate,
            next: 0,
            bounds: Bounds {
                visits: match aggregate {
                    AggregateTy::Record(_) => 1,
                    AggregateTy::Enum(_) => 2,
                    _ => return Err(ShapeFailure::Type),
                },
                resources: 0,
            },
        })
    }
}

#[cfg(test)]
#[path = "owned_enum_vector_bounds_tests.rs"]
mod enum_tests;
