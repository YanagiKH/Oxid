//! Typed scalar vector execution below the immutable canonical-suffix witness.
//! Descriptor coordinates and element ranges are distinct; payloads never use a
//! fixed-array carrier or an owned host Vec.
use super::super::vector_type::{self, VectorTy};
use super::experimental::{heap_failure, OperationValue, ResultPlace};
use super::*;
use crate::frontend::builtin_catalog::vector::{Function, Operation};
use std::ops::Range;

struct VectorPlace {
    frame: usize,
    bytes: Range<usize>,
}

/// A scalar slice retains its real backing resource class. Vec<u8> is not a
/// ByteBuf, although an admitted &[u8] may borrow either class as a source.
enum ScalarRegion {
    Activation {
        frame: usize,
        bytes: Range<usize>,
        element: VectorTy,
    },
    Buffer {
        value: byte_buffer::Descriptor,
        start: i32,
        end: i32,
    },
    Vector {
        value: byte_buffer::VectorDescriptor,
        start: i32,
        end: i32,
        element: VectorTy,
    },
}

#[derive(Clone, Copy)]
struct PreparedRelease {
    loan: LoanKey,
    frame: usize,
    index: usize,
    owner: bool,
    kind: BorrowKind,
}
struct Delivery {
    local: ResultPlace,
    caller_result: ResultPlace,
    from: Option<(OwnerKey, u64)>,
    to: Option<OwnerKey>,
    releases: [Option<PreparedRelease>; 2],
    caller: usize,
    resume: Resume,
    function: hir::DefId,
    cost: usize,
}
enum Returned {
    Scalar(Scalar),
    Descriptor([u8; 16]),
    Status(u32),
}
enum Footprint {
    Activation(Range<u64>),
    Vector {
        origin: u64,
        element: VectorTy,
        start: i32,
        end: i32,
        whole: bool,
    },
}

/// Complete named synchronous roles; no new persistent frame or machine field.
/// The caller must charge this actual representation only for vector plans.
#[allow(dead_code)]
struct ControlCarriers<V> {
    delivery: Delivery,
    delivery_return: Result<Delivery>,
    region: ScalarRegion,
    region_return: Result<ScalarRegion>,
    place: VectorPlace,
    place_return: Result<(VectorPlace, byte_buffer::VectorDescriptor)>,
    descriptor: byte_buffer::VectorDescriptor,
    descriptor_return: std::result::Result<byte_buffer::VectorDescriptor, byte_buffer::Failure>,
    handles: [ReferenceHandle; 3],
    views: [BorrowView; 3],
    owner_keys: [OwnerKey; 3],
    loan_keys: [LoanKey; 3],
    elements: [byte_buffer::CheckedVectorElement<'static>; 3],
    types: [VectorTy; 3],
    ranges: [Range<usize>; 4],
    footprints: [Footprint; 2],
    footprint_return: Result<Footprint>,
    scalar_scratch: [u8; 16],
    record_leaves: [ScalarLeaves<'static>; 4],
    record_ids: [RecordId; 3],
    packed_cursors: [usize; 4],
    packed_ranges: [Range<usize>; 2],
    scalar: V,
    decoded: Result<V>,
    encoded: Result<()>,
    returned: Returned,
    region_pointer: *const u8,
    region_length: usize,
    byte_region_return: Result<experimental::ByteRegion>,
    vector_bytes_return: std::result::Result<&'static [u8], byte_buffer::Failure>,
    vector_bytes_mut_return: std::result::Result<&'static mut [u8], byte_buffer::Failure>,
    operation_fuel: usize,
    status: std::result::Result<byte_buffer::GrowStatus, byte_buffer::Failure>,
    scalar_results: [Result<i32>; 2],
    range_results: [Result<(i32, i32)>; 2],
    selected_ranges: [(i32, i32); 3],
}
pub(super) const fn control_bytes<V: StoredValue>() -> usize {
    size_of::<ControlCarriers<V>>() + range_control_bytes()
}
#[allow(dead_code)]
struct RangeCarriers {
    arrays: [FixedArrayTy; 2],
    view: BorrowView,
    view_return: Result<BorrowView>,
    range_argument: Option<(i32, i32)>,
    ranges: [(i32, i32); 3],
    range_return: Result<(i32, i32)>,
    offset: Option<u64>,
    descriptor: std::result::Result<AggregateSlot, DeclarationError>,
}
pub(super) const fn range_control_bytes() -> usize {
    size_of::<RangeCarriers>()
}

fn selected_range(
    start: i32,
    end: i32,
    range: Option<(i32, i32)>,
    span: Span,
) -> Result<(i32, i32)> {
    let length = end
        .checked_sub(start)
        .filter(|length| *length >= 0)
        .ok_or(OwnedRunFailure::Bounds(span))?;
    let (first, last) = range.unwrap_or((0, length));
    if start < 0 || first < 0 || last < first || last > length {
        return Err(OwnedRunFailure::Bounds(span));
    }
    Ok((
        start
            .checked_add(first)
            .ok_or(OwnedRunFailure::Bounds(span))?,
        start
            .checked_add(last)
            .ok_or(OwnedRunFailure::Bounds(span))?,
    ))
}
fn packed_range(start: i32, end: i32, span: Span) -> Result<u64> {
    if start < 0 || end < start {
        return Err(OwnedRunFailure::Bounds(span));
    }
    Ok(((start as u64) << 32) | (end - start) as u64)
}
fn unpacked_range(offset: u64, span: Span) -> Result<(i32, i32)> {
    let start = i32::try_from(offset >> 32).map_err(|_| OwnedRunFailure::Bounds(span))?;
    let length = i32::try_from(offset as u32).map_err(|_| OwnedRunFailure::Bounds(span))?;
    Ok((
        start,
        start
            .checked_add(length)
            .ok_or(OwnedRunFailure::Bounds(span))?,
    ))
}

impl<V: StoredValue> ValueMachine<'_, '_, V> {
    fn vector_at(
        &self,
        root: OwnerKey,
        relative: u64,
        ty: VectorTy,
        span: Span,
    ) -> Result<(VectorPlace, byte_buffer::VectorDescriptor)> {
        self.owner(root, span)?;
        let extent = self.owner_extent(root, span)?;
        let start = extent
            .start
            .checked_add(index(relative, span)?)
            .ok_or_else(|| bad("vector offset", span))?;
        let end = start
            .checked_add(16)
            .filter(|end| *end <= extent.end)
            .ok_or_else(|| bad("vector extent", span))?;
        let frame = index(root.frame, span)?;
        let value = self
            .heap
            .vector_descriptor(
                &self.frames[frame].payload[start..end],
                vector_type::checked(ty, self.plan.witness().declarations())
                    .map_err(|failure| heap_failure(failure, span))?,
            )
            .map_err(|failure| heap_failure(failure, span))?;
        Ok((
            VectorPlace {
                frame,
                bytes: start..end,
            },
            value,
        ))
    }

    /// Packed slice words are never interpreted as an activation offset. The
    /// descriptor address comes exclusively from the checked nominal ancestry.
    fn vector_origin(
        &self,
        permission: LoanKey,
        root: OwnerKey,
        ty: VectorTy,
        span: Span,
    ) -> Result<u64> {
        match self.aggregate(root, span)? {
            actual if actual == ty.aggregate() => Ok(0),
            AggregateTy::Record(_) => {
                let view = self.projected_view(permission, root, span)?;
                if view.aggregate.map(AggregateSlot::aggregate) != Some(ty.aggregate()) {
                    return Err(bad("vector projection type", span));
                }
                Ok(view.offset)
            }
            _ => Err(bad("vector root type", span)),
        }
    }

    fn authority_handle(
        &self,
        frame: usize,
        base: AccessBase,
        span: Span,
    ) -> Result<Option<ReferenceHandle>> {
        match base {
            AccessBase::Owner(_) => Ok(None),
            AccessBase::Parameter(reference) => self.frames[frame]
                .references
                .get(reference.0)
                .copied()
                .map(Some)
                .ok_or_else(|| bad("vector parent reference", span)),
            AccessBase::Loan(loan) => self.reference_from_loan(frame, loan, span).map(Some),
        }
    }
    fn authority_referent(&self, frame: usize, base: AccessBase, span: Span) -> Result<BorrowedTy> {
        let function = self.function(self.frames[frame].function);
        match base {
            AccessBase::Owner(owner) => function
                .owners
                .get(owner.0)
                .map(|row| BorrowedTy::Exact(row.aggregate())),
            AccessBase::Parameter(reference) => function
                .references
                .get(reference.0)
                .map(ReferenceDecl::referent),
            AccessBase::Loan(loan) => function.loans.get(loan.0).map(LoanDecl::referent),
        }
        .ok_or_else(|| bad("vector authority type", span))
    }

    fn vector_slice_proof(
        &self,
        mut handle: ReferenceHandle,
        ty: VectorTy,
        span: Span,
    ) -> Result<(u64, i32, i32)> {
        self.validate_handle_identity(handle, span)?;
        let origin = self.vector_origin(handle.permission, handle.root, ty, span)?;
        let (_, value) = self.vector_at(handle.root, origin, ty, span)?;
        let length = self
            .heap
            .vector_len(
                &value,
                vector_type::checked(ty, self.plan.witness().declarations())
                    .map_err(|failure| heap_failure(failure, span))?,
            )
            .map_err(|failure| heap_failure(failure, span))?;
        let selected = self.vector_handle_range(handle, ty, origin, length, span)?;
        for _ in 0..plan::MAX_FRAMES * (MAX_LOCAL_REFERENCE_DEPTH + 1) {
            self.validate_handle_identity(handle, span)?;
            if handle.view.aggregate.map(AggregateSlot::aggregate) != Some(ty.aggregate()) {
                return Err(bad("vector slice type", span));
            }
            let row = self.loan(handle.permission, span)?;
            let frame = index(handle.permission.frame, span)?;
            let declaration = self
                .function(self.frames[frame].function)
                .loans
                .get(index(handle.permission.loan, span)?)
                .ok_or_else(|| bad("vector slice loan", span))?;
            let child = match declaration.referent() {
                actual if actual == ty.slice() => unpacked_range(handle.view.offset, span)?,
                BorrowedTy::Exact(actual) if actual == ty.aggregate() => {
                    if handle.view.offset != origin {
                        return Err(bad("vector descriptor origin", span));
                    }
                    (0, length)
                }
                _ => return Err(bad("vector slice referent", span)),
            };
            if child.0 < 0 || child.1 < child.0 || child.1 > length {
                return Err(OwnedRunFailure::Bounds(span));
            }
            let parent = self.authority_handle(frame, declaration.authority, span)?;
            let mut base_view = if let Some(parent) = parent {
                self.validate_handle_identity(parent, span)?;
                if row.parent != parent.permission || row.root != parent.root {
                    return Err(bad("vector slice parent", span));
                }
                parent.view
            } else {
                let AccessBase::Owner(owner) = declaration.authority else {
                    unreachable!()
                };
                if row.parent.instance != 0
                    || handle.root.frame != frame as u64
                    || handle.root.activation != self.frames[frame].activation
                    || handle.root.owner != owner.0 as u64
                {
                    return Err(bad("vector slice root identity", span));
                }
                self.whole_view(handle.root, span)?
            };
            let slice_parent = declaration.projection.is_empty()
                && self.authority_referent(frame, declaration.authority, span)? == ty.slice();
            if slice_parent && declaration.referent() == BorrowedTy::Exact(ty.aggregate()) {
                return Err(bad("vector slice cannot restore owner authority", span));
            }
            if !declaration.projection.is_empty() {
                let base = base_view
                    .aggregate
                    .ok_or_else(|| bad("vector projection base", span))?
                    .aggregate();
                let (projected, offset) = self
                    .plan
                    .witness()
                    .declarations()
                    .projection(base, &declaration.projection)
                    .map_err(|_| bad("vector slice projection", span))?;
                if projected != ValueTy::Owned(ty.aggregate()) {
                    return Err(bad("vector slice projection type", span));
                }
                base_view.offset = base_view
                    .offset
                    .checked_add(offset as u64)
                    .ok_or_else(|| bad("vector slice projection offset", span))?;
                base_view.aggregate = Some(
                    AggregateSlot::try_from_aggregate(ty.aggregate())
                        .map_err(|_| bad("vector slice projection descriptor", span))?,
                );
            }
            if base_view.aggregate.map(AggregateSlot::aggregate) != Some(ty.aggregate()) {
                return Err(bad("vector slice base type", span));
            }
            let base_range = if slice_parent {
                self.vector_handle_range(
                    parent.ok_or_else(|| bad("vector slice parent handle", span))?,
                    ty,
                    origin,
                    length,
                    span,
                )?
            } else {
                self.view_extent(handle.root, base_view, span)?;
                if base_view.offset != origin {
                    return Err(bad("vector slice base origin", span));
                }
                (0, length)
            };
            if child.0 < base_range.0 || child.1 > base_range.1 {
                return Err(bad("vector slice parent bounds", span));
            }
            let Some(parent) = parent else {
                return Ok((origin, selected.0, selected.1));
            };
            if parent.view.aggregate.map(AggregateSlot::aggregate) != Some(ty.aggregate()) {
                self.checked_handle_view(parent, span)?;
                return Ok((origin, selected.0, selected.1));
            }
            handle = parent;
        }
        Err(bad("vector slice ancestry", span))
    }
    fn vector_handle_range(
        &self,
        handle: ReferenceHandle,
        ty: VectorTy,
        origin: u64,
        length: i32,
        span: Span,
    ) -> Result<(i32, i32)> {
        let frame = index(handle.permission.frame, span)?;
        let declaration = self
            .function(self.frames[frame].function)
            .loans
            .get(index(handle.permission.loan, span)?)
            .ok_or_else(|| bad("vector handle range loan", span))?;
        match declaration.referent() {
            actual if actual == ty.slice() => unpacked_range(handle.view.offset, span),
            BorrowedTy::Exact(actual)
                if actual == ty.aggregate() && handle.view.offset == origin =>
            {
                Ok((0, length))
            }
            _ => Err(bad("vector handle range type", span)),
        }
    }
    pub(super) fn vector_slice(
        &self,
        handle: ReferenceHandle,
        ty: VectorTy,
        span: Span,
    ) -> Result<()> {
        self.vector_slice_proof(handle, ty, span).map(|_| ())
    }

    pub(super) fn stage_vector_view(
        &self,
        frame: usize,
        declaration: &LoanDecl,
        root: OwnerKey,
        mut view: BorrowView,
        range: Option<(i32, i32)>,
        span: Span,
    ) -> Result<BorrowView> {
        let Some(ty) = view
            .aggregate
            .map(AggregateSlot::aggregate)
            .and_then(VectorTy::from_aggregate)
        else {
            return Err(bad("vector loan view", span));
        };
        if declaration.referent() == BorrowedTy::Exact(ty.aggregate()) {
            if range.is_some() {
                return Err(bad("vector exact range", span));
            }
            if matches!(
                self.authority_referent(frame, declaration.authority, span)?,
                BorrowedTy::ScalarSlice(_) | BorrowedTy::RecordSlice(_)
            ) {
                return Err(bad("vector slice cannot restore owner authority", span));
            }
            self.vector_at(root, view.offset, ty, span)?;
            return Ok(view);
        }
        if declaration.referent() != ty.slice() {
            return Err(bad("vector slice element", span));
        }
        let (start, end) = if declaration.projection.is_empty()
            && self.authority_referent(frame, declaration.authority, span)? == ty.slice()
        {
            let parent = self
                .authority_handle(frame, declaration.authority, span)?
                .ok_or_else(|| bad("vector slice authority", span))?;
            let (_, start, end) = self.vector_slice_proof(parent, ty, span)?;
            (start, end)
        } else {
            let (_, value) = self.vector_at(root, view.offset, ty, span)?;
            (
                0,
                self.heap
                    .vector_len(
                        &value,
                        vector_type::checked(ty, self.plan.witness().declarations())
                            .map_err(|failure| heap_failure(failure, span))?,
                    )
                    .map_err(|failure| heap_failure(failure, span))?,
            )
        };
        let (start, end) = selected_range(start, end, range, span)?;
        view.offset = packed_range(start, end, span)?;
        Ok(view)
    }
    pub(super) fn stage_typed_array_view(
        &self,
        declaration: &LoanDecl,
        mut view: BorrowView,
        range: (i32, i32),
        span: Span,
    ) -> Result<BorrowView> {
        let Some(AggregateTy::FixedArray(array)) = view.aggregate.map(AggregateSlot::aggregate)
        else {
            return Err(bad("typed array range", span));
        };
        if declaration.referent() != BorrowedTy::ScalarSlice(array.element()) {
            return Err(bad("typed slice element", span));
        }
        let (start, end) = selected_range(0, array.length() as i32, Some(range), span)?;
        let offset = (start as u64)
            .checked_mul(array.stride() as u64)
            .ok_or(OwnedRunFailure::Bounds(span))?;
        view.offset = view
            .offset
            .checked_add(offset)
            .ok_or(OwnedRunFailure::Bounds(span))?;
        // This is a genuine fixed array, already constrained by its declaration.
        // Dynamic vector views never enter this branch or inherit its 1024 cap.
        let array = FixedArrayTy::check(array.element(), (end - start) as usize)
            .map_err(|_| bad("typed array range length", span))?;
        view.aggregate = Some(
            AggregateSlot::try_from_aggregate(AggregateTy::FixedArray(array))
                .map_err(|_| bad("typed array range descriptor", span))?,
        );
        Ok(view)
    }

    fn scalar_region(
        &self,
        frame: usize,
        base: AccessBase,
        access: Access,
        ty: VectorTy,
        span: Span,
    ) -> Result<ScalarRegion> {
        let root = self.base(frame, base, access, span)?;
        let view = self.base_view(frame, base, root, span)?;
        match view.aggregate.map(AggregateSlot::aggregate) {
            Some(actual) if actual == ty.aggregate() => {
                let slice = self.authority_referent(frame, base, span)? == ty.slice();
                let (origin, start, end) = if slice {
                    self.vector_slice_proof(
                        self.authority_handle(frame, base, span)?
                            .ok_or_else(|| bad("vector slice handle", span))?,
                        ty,
                        span,
                    )?
                } else {
                    let (_, value) = self.vector_at(root, view.offset, ty, span)?;
                    (
                        view.offset,
                        0,
                        self.heap
                            .vector_len(
                                &value,
                                vector_type::checked(ty, self.plan.witness().declarations())
                                    .map_err(|failure| heap_failure(failure, span))?,
                            )
                            .map_err(|failure| heap_failure(failure, span))?,
                    )
                };
                let (_, value) = self.vector_at(root, origin, ty, span)?;
                Ok(ScalarRegion::Vector {
                    value,
                    start,
                    end,
                    element: ty,
                })
            }
            Some(AggregateTy::FixedArray(array)) if VectorTy::Scalar(array.element()) == ty => {
                let extent = self.owner_extent(root, span)?;
                let start = extent
                    .start
                    .checked_add(index(view.offset, span)?)
                    .ok_or_else(|| bad("scalar region offset", span))?;
                let end = start
                    .checked_add(array.length() * array.stride())
                    .filter(|end| *end <= extent.end)
                    .ok_or_else(|| bad("scalar region extent", span))?;
                Ok(ScalarRegion::Activation {
                    frame: index(root.frame, span)?,
                    bytes: start..end,
                    element: ty,
                })
            }
            Some(AggregateTy::ByteBuf) if ty == VectorTy::Scalar(hir::Ty::U8) => {
                match self.byte_region(frame, base, access, span)? {
                    experimental::ByteRegion::Buffer { value, start, end } => {
                        Ok(ScalarRegion::Buffer { value, start, end })
                    }
                    _ => Err(bad("byte resource region", span)),
                }
            }
            _ => Err(bad("scalar region element", span)),
        }
    }

    pub(super) fn vector_byte_region(
        &self,
        frame: usize,
        base: AccessBase,
        access: Access,
        span: Span,
    ) -> Result<experimental::ByteRegion> {
        match self.scalar_region(frame, base, access, VectorTy::Scalar(hir::Ty::U8), span)? {
            ScalarRegion::Vector {
                value,
                start,
                end,
                element: hir::Ty::U8,
            } => {
                // Establish both U8 element identity and the initialized extent
                // before any byte helper can acquire a raw mutable view.
                self.heap
                    .vector_range(&value, byte_buffer::VectorElement::U8, start, end)
                    .map_err(|failure| heap_failure(failure, span))?;
                Ok(experimental::ByteRegion::Vector { value, start, end })
            }
            _ => Err(bad("vector byte view element", span)),
        }
    }

    pub(super) fn vector_view(&self, frame: usize, base: AccessBase) -> bool {
        let aggregate = match base {
            AccessBase::Owner(owner) => self
                .function(self.frames[frame].function)
                .owners
                .get(owner.0)
                .map(OwnerDecl::aggregate),
            AccessBase::Parameter(reference) => self.frames[frame]
                .references
                .get(reference.0)
                .and_then(|handle| handle.view.aggregate)
                .map(AggregateSlot::aggregate),
            AccessBase::Loan(loan) => self.frames[frame]
                .loans
                .get(loan.0)
                .and_then(|row| row.view.aggregate)
                .map(AggregateSlot::aggregate),
        };
        matches!(
            aggregate,
            Some(AggregateTy::Vector(_) | AggregateTy::RecordVector(_))
        )
    }
    fn vector_view_type(&self, frame: usize, base: AccessBase, span: Span) -> Result<VectorTy> {
        VectorTy::from_borrowed(self.authority_referent(frame, base, span)?)
            .ok_or_else(|| bad("vector indexed type", span))
    }
    fn vector_view_element(&self, frame: usize, base: AccessBase, span: Span) -> Result<hir::Ty> {
        match self.authority_referent(frame, base, span)? {
            BorrowedTy::Exact(AggregateTy::Vector(ty)) | BorrowedTy::ScalarSlice(ty) => Ok(ty),
            _ => Err(bad("vector indexed type", span)),
        }
    }
    pub(super) fn vector_index_read(
        &mut self,
        frame: usize,
        destination: LocalId,
        base: AccessBase,
        operand: Operand,
        span: Span,
    ) -> Result<()> {
        #[cfg(test)]
        let root = self.base(frame, base, Access::Read, span)?;
        let ty = self.vector_view_element(frame, base, span)?;
        let ScalarRegion::Vector {
            value, start, end, ..
        } = self.scalar_region(frame, base, Access::Read, VectorTy::Scalar(ty), span)?
        else {
            return Err(bad("vector indexed region", span));
        };
        let ordinal = self.vector_index(frame, operand, end - start, span)?;
        let bytes = self
            .heap
            .vector_range(
                &value,
                super::super::vector_type::element(ty),
                start + ordinal,
                start + ordinal + 1,
            )
            .map_err(|failure| heap_failure(failure, span))?;
        let value = V::decode(bytes, 0, ty, span)?;
        self.write(frame, destination, value, span)?;
        #[cfg(test)]
        self.record_event(Event::ReadIndex(root, ordinal as usize, value));
        Ok(())
    }
    fn vector_index(&self, frame: usize, operand: Operand, length: i32, span: Span) -> Result<i32> {
        let Some(Scalar::I32(ordinal)) = self.read(frame, operand)?.narrow() else {
            return Err(bad("vector index type", span));
        };
        if ordinal < 0 || ordinal >= length {
            return Err(OwnedRunFailure::Bounds(span));
        }
        Ok(ordinal)
    }
    fn vector_store(
        &mut self,
        descriptor: &byte_buffer::VectorDescriptor,
        ty: hir::Ty,
        ordinal: i32,
        value: V,
        span: Span,
    ) -> Result<()> {
        if value.ty() != ty {
            return Err(bad("vector element type", span));
        }
        let mut scratch = [0u8; 16];
        value.encode(&mut scratch, 0, span)?;
        let element = super::super::vector_type::element(ty);
        // SAFETY: Access::Write already established the exclusive language loan.
        // Only the actual typed StoredValue encoding reaches initialized cells;
        // bool/unit/char invariants and float bit patterns are preserved.
        let bytes = unsafe {
            self.heap
                .vector_range_mut(descriptor, element, ordinal, ordinal + 1)
        }
        .map_err(|failure| heap_failure(failure, span))?;
        bytes.copy_from_slice(&scratch[..element.size()]);
        Ok(())
    }
    pub(super) fn vector_index_write(
        &mut self,
        frame: usize,
        base: AccessBase,
        operand: Operand,
        value: Operand,
        span: Span,
    ) -> Result<()> {
        #[cfg(test)]
        let root = self.base(frame, base, Access::Write, span)?;
        let ty = self.vector_view_element(frame, base, span)?;
        let ScalarRegion::Vector {
            value: descriptor,
            start,
            end,
            ..
        } = self.scalar_region(frame, base, Access::Write, VectorTy::Scalar(ty), span)?
        else {
            return Err(bad("vector indexed region", span));
        };
        let value = self.read(frame, value)?;
        let ordinal = self.vector_index(frame, operand, end - start, span)?;
        self.vector_store(&descriptor, ty, start + ordinal, value, span)?;
        #[cfg(test)]
        self.record_event(Event::WriteIndex(root, ordinal as usize, value));
        Ok(())
    }
    pub(super) fn vector_length(
        &mut self,
        frame: usize,
        destination: LocalId,
        base: AccessBase,
        span: Span,
    ) -> Result<()> {
        #[cfg(test)]
        let root = self.base(frame, base, Access::Read, span)?;
        let ty = self.vector_view_type(frame, base, span)?;
        let ScalarRegion::Vector { start, end, .. } =
            self.scalar_region(frame, base, Access::Read, ty, span)?
        else {
            return Err(bad("vector length region", span));
        };
        self.write(frame, destination, Scalar::I32(end - start), span)?;
        #[cfg(test)]
        self.record_event(Event::ArrayLength(root, (end - start) as usize));
        Ok(())
    }
    // Keep checked projection operands explicit, matching the IR instruction.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn vector_projection_read(
        &mut self,
        frame: usize,
        destination: LocalId,
        root: OwnerKey,
        ty: hir::Ty,
        relative: usize,
        operand: Operand,
        span: Span,
    ) -> Result<()> {
        let (_, descriptor) = self.vector_at(root, relative as u64, VectorTy::Scalar(ty), span)?;
        let length = self
            .heap
            .vector_len(&descriptor, super::super::vector_type::element(ty))
            .map_err(|failure| heap_failure(failure, span))?;
        let ordinal = self.vector_index(frame, operand, length, span)?;
        let bytes = self
            .heap
            .vector_range(
                &descriptor,
                super::super::vector_type::element(ty),
                ordinal,
                ordinal + 1,
            )
            .map_err(|failure| heap_failure(failure, span))?;
        let value = V::decode(bytes, 0, ty, span)?;
        self.write(frame, destination, value, span)?;
        #[cfg(test)]
        self.record_event(Event::ReadIndex(root, ordinal as usize, value));
        Ok(())
    }
    // Keep checked projection operands explicit, matching the IR instruction.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn vector_projection_write(
        &mut self,
        frame: usize,
        root: OwnerKey,
        ty: hir::Ty,
        relative: usize,
        operand: Operand,
        value: Operand,
        span: Span,
    ) -> Result<()> {
        let (_, descriptor) = self.vector_at(root, relative as u64, VectorTy::Scalar(ty), span)?;
        let length = self
            .heap
            .vector_len(&descriptor, super::super::vector_type::element(ty))
            .map_err(|failure| heap_failure(failure, span))?;
        let value = self.read(frame, value)?;
        let ordinal = self.vector_index(frame, operand, length, span)?;
        self.vector_store(&descriptor, ty, ordinal, value, span)?;
        #[cfg(test)]
        self.record_event(Event::WriteIndex(root, ordinal as usize, value));
        Ok(())
    }
    pub(super) fn vector_projection_length(
        &mut self,
        frame: usize,
        destination: LocalId,
        root: OwnerKey,
        ty: VectorTy,
        relative: usize,
        span: Span,
    ) -> Result<()> {
        let (_, descriptor) = self.vector_at(root, relative as u64, ty, span)?;
        let length = self
            .heap
            .vector_len(
                &descriptor,
                vector_type::checked(ty, self.plan.witness().declarations())
                    .map_err(|failure| heap_failure(failure, span))?,
            )
            .map_err(|failure| heap_failure(failure, span))?;
        self.write(frame, destination, Scalar::I32(length), span)?;
        #[cfg(test)]
        self.record_event(Event::ArrayLength(root, length as usize));
        Ok(())
    }

    fn vector_footprint(&self, handle: ReferenceHandle, span: Span) -> Result<Footprint> {
        let actual = self.checked_handle_view(handle, span)?;
        if let Some(ty) = VectorTy::from_aggregate(actual) {
            let frame = index(handle.permission.frame, span)?;
            let declaration = self
                .function(self.frames[frame].function)
                .loans
                .get(index(handle.permission.loan, span)?)
                .ok_or_else(|| bad("vector footprint loan", span))?;
            let whole = declaration.referent() == BorrowedTy::Exact(actual);
            let (origin, start, end) = if whole {
                let origin = self.vector_origin(handle.permission, handle.root, ty, span)?;
                if handle.view.offset != origin {
                    return Err(bad("vector footprint origin", span));
                }
                let (_, value) = self.vector_at(handle.root, origin, ty, span)?;
                (
                    origin,
                    0,
                    self.heap
                        .vector_len(
                            &value,
                            vector_type::checked(ty, self.plan.witness().declarations())
                                .map_err(|failure| heap_failure(failure, span))?,
                        )
                        .map_err(|failure| heap_failure(failure, span))?,
                )
            } else {
                self.vector_slice_proof(handle, ty, span)?
            };
            return Ok(Footprint::Vector {
                origin,
                element: ty,
                start,
                end,
                whole,
            });
        }
        let offset = if actual == AggregateTy::ByteBuf {
            self.resource_origin(handle.permission, handle.root, span)?
        } else {
            handle.view.offset
        };
        let size = match actual {
            AggregateTy::FixedArray(array) => array.length() as u64 * array.stride() as u64,
            _ => self
                .plan
                .witness()
                .declarations()
                .aggregate_layout(actual)
                .map_err(|_| bad("vector alias layout", span))?
                .size() as u64,
        };
        Ok(Footprint::Activation(
            offset
                ..offset
                    .checked_add(size)
                    .ok_or_else(|| bad("vector alias extent", span))?,
        ))
    }
    pub(super) fn vector_reference_overlap(
        &self,
        left: ReferenceHandle,
        right: ReferenceHandle,
        span: Span,
    ) -> Result<bool> {
        if left.root != right.root {
            return Ok(false);
        }
        let left = self.vector_footprint(left, span)?;
        let right = self.vector_footprint(right, span)?;
        match (left, right) {
            (
                Footprint::Vector {
                    origin: a,
                    element: ta,
                    start: sa,
                    end: ea,
                    whole: wa,
                },
                Footprint::Vector {
                    origin: b,
                    element: tb,
                    start: sb,
                    end: eb,
                    whole: wb,
                },
            ) => {
                if a != b {
                    return Ok(false);
                }
                if ta != tb {
                    return Err(bad("vector alias element", span));
                }
                Ok(wa || wb || (sa < ea && sb < eb && sa < eb && sb < ea))
            }
            (Footprint::Vector { origin, .. }, Footprint::Activation(bytes))
            | (Footprint::Activation(bytes), Footprint::Vector { origin, .. }) => Ok(bytes.start
                < bytes.end
                && origin < bytes.end
                && bytes.start
                    < origin
                        .checked_add(16)
                        .ok_or_else(|| bad("vector alias descriptor", span))?),
            (Footprint::Activation(a), Footprint::Activation(b)) => {
                Ok(a.start < a.end && b.start < b.end && a.start < b.end && b.start < a.end)
            }
        }
    }

    fn vector_parameter(
        &self,
        frame: usize,
        key: Function,
        access: Access,
        span: Span,
    ) -> Result<(VectorPlace, byte_buffer::VectorDescriptor)> {
        if self.authority_referent(frame, AccessBase::Parameter(ReferenceParamId(0)), span)?
            != BorrowedTy::Exact(key.aggregate())
        {
            return Err(bad("vector parameter type", span));
        }
        let handle = self
            .authority_handle(frame, AccessBase::Parameter(ReferenceParamId(0)), span)?
            .ok_or_else(|| bad("vector parameter handle", span))?;
        let permission_frame = index(handle.permission.frame, span)?;
        let referent = self
            .function(self.frames[permission_frame].function)
            .loans
            .get(index(handle.permission.loan, span)?)
            .map(LoanDecl::referent)
            .ok_or_else(|| bad("vector parameter loan", span))?;
        if referent != BorrowedTy::Exact(key.aggregate()) {
            return Err(bad("vector parameter requires whole authority", span));
        }
        let root = self.base(
            frame,
            AccessBase::Parameter(ReferenceParamId(0)),
            access,
            span,
        )?;
        let view = self.base_view(
            frame,
            AccessBase::Parameter(ReferenceParamId(0)),
            root,
            span,
        )?;
        self.vector_at(root, view.offset, VectorTy::function(key), span)
    }
    fn vector_i32(&self, frame: usize, local: usize, span: Span) -> Result<i32> {
        match self
            .read(
                frame,
                Operand {
                    local: LocalId(local),
                    span,
                },
            )?
            .narrow()
        {
            Some(Scalar::I32(value)) => Ok(value),
            _ => Err(bad("vector count type", span)),
        }
    }
    fn vector_scalar(
        &self,
        frame: usize,
        local: usize,
        ty: hir::Ty,
        span: Span,
    ) -> Result<[u8; 16]> {
        let value = self.read(
            frame,
            Operand {
                local: LocalId(local),
                span,
            },
        )?;
        if value.ty() != ty {
            return Err(bad("vector scalar type", span));
        }
        let mut scratch = [0u8; 16];
        value.encode(&mut scratch, 0, span)?;
        Ok(scratch)
    }
    fn vector_caller_result(
        &self,
        caller: usize,
        destination: CallResult,
        expected: ValueTy,
        span: Span,
    ) -> Result<ResultPlace> {
        let function = self.function(self.frames[caller].function);
        match (destination, expected) {
            (CallResult::Scalar(local), ValueTy::Scalar(ty)) => {
                let row = function
                    .locals
                    .get(local.0)
                    .ok_or_else(|| bad("vector caller scalar", span))?;
                let slot = self.frames[caller]
                    .slots
                    .get(local.0)
                    .ok_or_else(|| bad("vector caller scalar storage", span))?;
                if row.ty != ty || (row.kind == LocalKind::Parameter && slot.is_some()) {
                    return Err(bad("vector caller scalar type", span));
                }
                Ok(ResultPlace::Scalar(local))
            }
            (CallResult::Owned(owner), ValueTy::Owned(aggregate)) => {
                self.expect_owner(caller, owner, &[DEAD], span)?;
                if function.owners[owner.0].aggregate() != aggregate {
                    return Err(bad("vector caller owner type", span));
                }
                let key = self.raw_key(caller, owner);
                let generation = epoch(key.generation, span)?;
                let bytes = self.owner_extent(key, span)?;
                match aggregate {
                    AggregateTy::Vector(_) | AggregateTy::RecordVector(_) if bytes.len() == 16 => {
                        ()
                    }
                    AggregateTy::Enum(enumeration) => {
                        let variants = self
                            .plan
                            .witness()
                            .declarations()
                            .enums()
                            .variants(enumeration)
                            .map_err(|_| bad("vector status layout", span))?;
                        if bytes.len() < 4
                            || variants.len() != 4
                            || variants
                                .iter()
                                .any(|variant| variant.value_payload().is_some())
                        {
                            return Err(bad("vector status result", span));
                        }
                    }
                    _ => return Err(bad("vector caller result type", span)),
                }
                Ok(ResultPlace::Owner {
                    owner,
                    bytes,
                    generation,
                })
            }
            _ => Err(bad("vector caller result class", span)),
        }
    }
    fn prepare_vector_delivery(
        &self,
        frame: usize,
        key: Function,
        destination: CallResult,
        span: Span,
    ) -> Result<Delivery> {
        let current = &self.frames[frame];
        let function = self.function(current.function);
        if self.plan.witness().vector_operation(function.id) != Some(key) {
            return Err(bad("vector function identity", span));
        }
        let block = function
            .blocks
            .get(current.block.0)
            .ok_or_else(|| bad("vector block", span))?;
        if current.next.checked_add(1) != Some(block.statements.len()) {
            return Err(bad("vector canonical position", span));
        }
        let end = block
            .terminator
            .as_ref()
            .ok_or_else(|| bad("vector return", span))?;
        match (destination, &end.kind) {
            (CallResult::Scalar(local), OwnedTerminatorKind::ReturnScalar(value))
                if local == value.local => {}
            (CallResult::Owned(owner), OwnedTerminatorKind::ReturnOwned(returned))
                if owner == *returned => {}
            _ => return Err(bad("vector canonical return", span)),
        }
        if current.loans.iter().any(|row| row.state != 0)
            || current.calls.iter().any(|row| row.phase != CLOSED)
            || current
                .owners
                .iter()
                .any(|row| row.shared_children != 0 || row.exclusive_children != 0)
        {
            return Err(bad("vector return region", span));
        }
        let input_owners = function
            .parameters
            .iter()
            .filter(|parameter| matches!(parameter, ParameterBinding::Owned(_)))
            .count();
        for parameter in &function.parameters {
            if let ParameterBinding::Owned(owner) = parameter {
                let record = key
                    .record_id()
                    .ok_or_else(|| bad("scalar vector owned input", span))?;
                if function.owners[owner.0].aggregate() != AggregateTy::Record(record) {
                    return Err(bad("record vector input identity", span));
                }
                self.base(frame, AccessBase::Owner(*owner), Access::Read, span)?;
            }
        }
        let local = self.prepare_experimental_result(frame, destination, span)?;
        let from = match &local {
            ResultPlace::Owner {
                owner, generation, ..
            } => {
                if function.owners.len() != input_owners + 1 {
                    return Err(bad("vector return owners", span));
                }
                let key = OwnerKey {
                    generation: *generation,
                    ..self.raw_key(frame, *owner)
                };
                Some((key, epoch(*generation, span)?))
            }
            ResultPlace::Scalar(_) => {
                if function.owners.len() != input_owners {
                    return Err(bad("vector scalar return owners", span));
                }
                None
            }
        };
        let caller = frame
            .checked_sub(1)
            .ok_or_else(|| bad("vector caller", span))?;
        let resume = current
            .return_to
            .ok_or_else(|| bad("vector caller", span))?;
        let parent = self.function(self.frames[caller].function);
        let call = parent
            .calls
            .get(resume.call.0)
            .ok_or_else(|| bad("vector caller call", span))?;
        if call.target != function.id
            || self.frames[caller]
                .calls
                .get(resume.call.0)
                .is_none_or(|row| row.phase != IN_FLIGHT)
            || resume.continuation.0 >= parent.blocks.len()
        {
            return Err(bad("vector caller call state", span));
        }
        let caller_result =
            self.vector_caller_result(caller, call.result, function.result, span)?;
        let to = match &caller_result {
            ResultPlace::Owner {
                owner, generation, ..
            } => Some(OwnerKey {
                generation: *generation,
                ..self.raw_key(caller, *owner)
            }),
            ResultPlace::Scalar(_) => None,
        };
        let borrowed = self.plan.function(parent.id).borrowed_loans(resume.call);
        if borrowed.len() > 2 || borrowed.len() != function.references.len() {
            return Err(bad("vector return loans", span));
        }
        let mut releases: [Option<PreparedRelease>; 2] = [None, None];
        for (position, loan) in borrowed.iter().enumerate() {
            let handle = self.handle(caller, *loan, span)?;
            let key = handle.permission;
            if current.references.get(position).copied() != Some(handle)
                || function.references[position].kind != self.loan_kind(key, span)?
            {
                return Err(bad("vector return reference identity", span));
            }
            self.release_preflight(key, span)?;
            let row = self.loan(key, span)?;
            let kind = self.loan_kind(key, span)?;
            let release = if row.parent.instance == 0 {
                PreparedRelease {
                    loan: key,
                    frame: index(row.root.frame, span)?,
                    index: index(row.root.owner, span)?,
                    owner: true,
                    kind,
                }
            } else {
                PreparedRelease {
                    loan: key,
                    frame: index(row.parent.frame, span)?,
                    index: index(row.parent.loan, span)?,
                    owner: false,
                    kind,
                }
            };
            for previous in releases[..position].iter().flatten() {
                if previous.loan == key {
                    return Err(bad("vector duplicate return loan", span));
                }
                if previous.frame == release.frame
                    && previous.index == release.index
                    && previous.owner == release.owner
                    && previous.kind == release.kind
                {
                    let count = if release.owner {
                        let row = &self.frames[release.frame].owners[release.index];
                        if kind == BorrowKind::Shared {
                            row.shared_children
                        } else {
                            row.exclusive_children
                        }
                    } else {
                        let row = &self.frames[release.frame].loans[release.index];
                        if kind == BorrowKind::Shared {
                            row.shared_children
                        } else {
                            row.exclusive_children
                        }
                    };
                    if count < 2 {
                        return Err(bad("vector return loan counts", span));
                    }
                }
            }
            releases[position] = Some(release);
        }
        Ok(Delivery {
            local,
            caller_result,
            from,
            to,
            releases,
            caller,
            resume,
            function: function.id,
            cost: self.plan.terminator_cost(function.id, &end.kind),
        })
    }

    fn commit_vector_result(&mut self, frame: usize, place: ResultPlace, value: &Returned) {
        let bytes = match &place {
            ResultPlace::Owner { bytes, .. } => Some(bytes.clone()),
            _ => None,
        };
        let legacy = match value {
            Returned::Scalar(value) => OperationValue::Scalar(*value),
            Returned::Descriptor(_) => OperationValue::EmptyBuffer,
            Returned::Status(tag) => OperationValue::Status {
                tag: *tag,
                payload: None,
            },
        };
        self.commit_experimental_result(frame, place, legacy);
        if let Returned::Descriptor(encoded) = value {
            self.frames[frame].payload[bytes.expect("prepared vector result")]
                .copy_from_slice(encoded);
        }
    }
    fn complete_vector_delivery(&mut self, frame: usize, delivery: Delivery, value: Returned) {
        self.commit_vector_result(frame, delivery.local, &value);
        self.commit_vector_result(delivery.caller, delivery.caller_result, &value);
        if let Some((from, generation)) = delivery.from {
            let row = &mut self.frames[frame].owners[from.owner as usize];
            row.generation = generation;
            row.state = MOVED;
        }
        for release in delivery.releases.into_iter().flatten() {
            let (shared, exclusive) = if release.owner {
                let row = &mut self.frames[release.frame].owners[release.index];
                (&mut row.shared_children, &mut row.exclusive_children)
            } else {
                let row = &mut self.frames[release.frame].loans[release.index];
                (&mut row.shared_children, &mut row.exclusive_children)
            };
            match release.kind {
                BorrowKind::Shared => *shared -= 1,
                BorrowKind::Exclusive => *exclusive -= 1,
            }
            self.frames[release.loan.frame as usize].loans[release.loan.loan as usize].state = 0;
            #[cfg(test)]
            self.record_event(Event::Release(release.loan));
        }
        self.frames[delivery.caller].calls[delivery.resume.call.0] = CallRuntime::default();
        self.branch(delivery.caller, delivery.resume.continuation);
        #[cfg(test)]
        {
            if let (Some((from, _)), Some(to)) = (delivery.from, delivery.to) {
                self.record_event(Event::Transfer(from, to));
            }
            self.record_event(Event::Return(delivery.function));
        }
        let usage = self.plan.function(delivery.function).usage();
        self.frames.pop();
        self.live_slots -= usage.scalar_slots;
        self.live_cells -= usage.expanded_cells;
        self.live_bytes -= usage.reference_bytes;
    }
    pub(super) fn vector_delivery(
        &mut self,
        frame: usize,
        key: Function,
        destination: CallResult,
        span: Span,
    ) -> Result<()> {
        let delivery = self.prepare_vector_delivery(frame, key, destination, span)?;
        // Reserve the ACTUAL sealed return work before any heap mutation. The
        // direct completion below never recharges or re-enters fallible return.
        self.charge(delivery.cost, span)?;
        let ty = VectorTy::function(key);
        let element = vector_type::checked(ty, self.plan.witness().declarations())
            .map_err(|failure| heap_failure(failure, span))?;
        let value = match key.operation() {
            Operation::New => {
                Returned::Descriptor(byte_buffer::VectorDescriptor::empty().encoding())
            }
            Operation::Len | Operation::Capacity => {
                let (_, value) = self.vector_parameter(frame, key, Access::Read, span)?;
                let count = if key.operation() == Operation::Len {
                    self.heap.vector_len(&value, element)
                } else {
                    self.heap.vector_capacity(&value, element)
                }
                .map_err(|failure| heap_failure(failure, span))?;
                Returned::Scalar(Scalar::I32(count))
            }
            Operation::Take => {
                let (place, mut value) = self.vector_parameter(frame, key, Access::Write, span)?;
                let moved = self
                    .heap
                    .vector_take(&mut value, element)
                    .map_err(|failure| heap_failure(failure, span))?;
                let encoded = moved.encoding();
                self.frames[place.frame].payload[place.bytes].copy_from_slice(&value.encoding());
                Returned::Descriptor(encoded)
            }
            operation => {
                let (place, mut value) = self.vector_parameter(frame, key, Access::Write, span)?;
                let mut operation_fuel = self.fuel;
                let status = match operation {
                    Operation::TryReserve => {
                        let count = self.vector_i32(frame, 0, span)?;
                        self.heap.vector_try_reserve(
                            &mut value,
                            element,
                            count,
                            &mut operation_fuel,
                        )
                    }
                    Operation::TryPush if key.record_id().is_some() => {
                        let scratch = self.packed_record_value(
                            frame,
                            OwnerPlaceId(0),
                            key.record_id().expect("checked record key"),
                            span,
                        )?;
                        self.heap.vector_try_push(
                            &mut value,
                            element,
                            &self.frames[frame].payload[scratch],
                            &mut operation_fuel,
                        )
                    }
                    Operation::TryResize if key.record_id().is_some() => {
                        let count = self.vector_i32(frame, 0, span)?;
                        let scratch = self.packed_record_value(
                            frame,
                            OwnerPlaceId(0),
                            key.record_id().expect("checked record key"),
                            span,
                        )?;
                        self.heap.vector_try_resize(
                            &mut value,
                            element,
                            count,
                            &self.frames[frame].payload[scratch],
                            &mut operation_fuel,
                        )
                    }
                    Operation::TryPush => {
                        let scratch = self.vector_scalar(frame, 0, key.element(), span)?;
                        self.heap.vector_try_push(
                            &mut value,
                            element,
                            &scratch[..element.size()],
                            &mut operation_fuel,
                        )
                    }
                    Operation::TryResize => {
                        let count = self.vector_i32(frame, 0, span)?;
                        let scratch = self.vector_scalar(frame, 1, key.element(), span)?;
                        self.heap.vector_try_resize(
                            &mut value,
                            element,
                            count,
                            &scratch[..element.size()],
                            &mut operation_fuel,
                        )
                    }
                    Operation::TryExtend => match self.scalar_region(
                        frame,
                        AccessBase::Parameter(ReferenceParamId(1)),
                        Access::Read,
                        ty,
                        span,
                    )? {
                        ScalarRegion::Activation {
                            frame,
                            bytes,
                            element: source_type,
                        } => {
                            if source_type != ty {
                                return Err(bad("vector source element", span));
                            }
                            self.heap.vector_try_extend(
                                &mut value,
                                element,
                                &self.frames[frame].payload[bytes],
                                &mut operation_fuel,
                            )
                        }
                        ScalarRegion::Vector {
                            value: source,
                            start,
                            end,
                            element: source_type,
                        } => {
                            if source_type != ty {
                                return Err(bad("vector source element", span));
                            }
                            self.heap.vector_try_extend_vector(
                                &mut value,
                                element,
                                &source,
                                start,
                                end,
                                &mut operation_fuel,
                            )
                        }
                        ScalarRegion::Buffer {
                            value: source,
                            start,
                            end,
                        } => {
                            if ty != VectorTy::Scalar(hir::Ty::U8) {
                                return Err(bad("vector byte source element", span));
                            }
                            let bytes = self
                                .heap
                                .range(&source, start, end)
                                .map_err(|failure| heap_failure(failure, span))?;
                            let pointer = bytes.as_ptr();
                            let length = bytes.len();
                            // SAFETY: validated ByteBuf and Vector are distinct
                            // resource classes/allocations. Only the target
                            // payload and registry links can change here; the
                            // admitted source loan keeps these bytes allocated.
                            let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
                            self.heap.vector_try_extend(
                                &mut value,
                                element,
                                bytes,
                                &mut operation_fuel,
                            )
                        }
                    },
                    Operation::Truncate => {
                        let count = self.vector_i32(frame, 0, span)?;
                        self.heap
                            .vector_truncate(&mut value, element, count)
                            .map(|_| byte_buffer::GrowStatus::Ok)
                    }
                    _ => unreachable!("closed mutable vector operation"),
                };
                if self.fuel != HOSTED_FUEL {
                    self.fuel = operation_fuel;
                }
                let status = status.map_err(|failure| heap_failure(failure, span))?;
                self.frames[place.frame].payload[place.bytes].copy_from_slice(&value.encoding());
                if operation == Operation::Truncate {
                    Returned::Scalar(Scalar::Unit)
                } else {
                    Returned::Status(status as u32)
                }
            }
        };
        self.complete_vector_delivery(frame, delivery, value);
        Ok(())
    }
    fn packed_record_value(
        &mut self,
        frame: usize,
        source: OwnerPlaceId,
        record: RecordId,
        span: Span,
    ) -> Result<Range<usize>> {
        let from = self.base(frame, AccessBase::Owner(source), Access::Read, span)?;
        let declarations = self.plan.witness().declarations();
        if self.aggregate(from, span)? != AggregateTy::Record(record)
            || !declarations.is_copy(AggregateTy::Record(record))
        {
            return Err(bad("record vector value type", span));
        }
        let bytes = declarations
            .record_packed_bytes(record)
            .map_err(|_| bad("record vector payload extent", span))?;
        let available = self
            .plan
            .record_vector_scratch_range(self.frames[frame].function)
            .ok_or_else(|| bad("record vector staging", span))?;
        let end = available
            .start
            .checked_add(bytes)
            .filter(|end| *end <= available.end)
            .ok_or_else(|| bad("record vector staging extent", span))?;
        let scratch = available.start..end;
        // Validate every initialized leaf before the first staging write. Neither
        // traversal visits activation padding, even for a nested empty record.
        let mut cursor = 0usize;
        for leaf in declarations
            .leaves(AggregateTy::Record(record))
            .map_err(|_| bad("record vector leaves", span))?
        {
            self.load_leaf(from, leaf, span)?;
            cursor = cursor
                .checked_add(vector_type::element(leaf.ty).size())
                .ok_or_else(|| bad("record vector leaf extent", span))?;
        }
        if cursor != bytes {
            return Err(bad("record vector packed layout", span));
        }
        cursor = scratch.start;
        for leaf in declarations
            .leaves(AggregateTy::Record(record))
            .map_err(|_| bad("record vector leaves", span))?
        {
            let value = self.load_leaf(from, leaf, span)?;
            value.encode(&mut self.frames[frame].payload, cursor, span)?;
            cursor += vector_type::element(leaf.ty).size();
        }
        Ok(scratch)
    }
    fn record_vector_region(
        &self,
        frame: usize,
        base: AccessBase,
        path: &[FieldId],
        access: Access,
        record: RecordId,
        span: Span,
    ) -> Result<(byte_buffer::VectorDescriptor, i32, i32)> {
        let ty = VectorTy::Record(record);
        if path.is_empty() {
            let ScalarRegion::Vector {
                value,
                start,
                end,
                element,
            } = self.scalar_region(frame, base, access, ty, span)?
            else {
                return Err(bad("record vector indexed region", span));
            };
            if element != ty {
                return Err(bad("record vector nominal identity", span));
            }
            return Ok((value, start, end));
        }
        let (root, actual, relative) = self.projection_base(frame, base, access, path, span)?;
        if actual != ValueTy::Owned(ty.aggregate()) {
            return Err(bad("record vector projected identity", span));
        }
        let (_, descriptor) = self.vector_at(root, relative as u64, ty, span)?;
        let element = vector_type::checked(ty, self.plan.witness().declarations())
            .map_err(|failure| heap_failure(failure, span))?;
        let length = self
            .heap
            .vector_len(&descriptor, element)
            .map_err(|failure| heap_failure(failure, span))?;
        Ok((descriptor, 0, length))
    }
    pub(super) fn record_vector_index_read(
        &mut self,
        frame: usize,
        destination: OwnerPlaceId,
        base: AccessBase,
        path: &[FieldId],
        index_operand: Operand,
        span: Span,
    ) -> Result<()> {
        self.expect_owner(frame, destination, &[UNINITIALIZED], span)?;
        let to = self.raw_key(frame, destination);
        let AggregateTy::Record(record) = self.aggregate(to, span)? else {
            return Err(bad("record vector copy destination", span));
        };
        let declarations = self.plan.witness().declarations();
        let element = vector_type::checked(VectorTy::Record(record), declarations)
            .map_err(|failure| heap_failure(failure, span))?;
        let (descriptor, start, end) =
            self.record_vector_region(frame, base, path, Access::Read, record, span)?;
        let ordinal = self.vector_index(frame, index_operand, end - start, span)?;
        let bytes = self
            .heap
            .vector_range(&descriptor, element, start + ordinal, start + ordinal + 1)
            .map_err(|failure| heap_failure(failure, span))?;
        // The heap allocation is distinct from activation storage and stays
        // registered for this synchronous copy; no allocation or release occurs.
        let pointer = bytes.as_ptr();
        let length = bytes.len();
        let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
        let mut cursor = 0;
        for leaf in declarations
            .leaves(AggregateTy::Record(record))
            .map_err(|_| bad("record vector copy leaves", span))?
        {
            V::decode(bytes, cursor, leaf.ty, span)?;
            self.leaf_offset(to, leaf.offset, leaf.ty, span)?;
            cursor += vector_type::element(leaf.ty).size();
        }
        if cursor != length {
            return Err(bad("record vector copy extent", span));
        }
        cursor = 0;
        for leaf in declarations
            .leaves(AggregateTy::Record(record))
            .map_err(|_| bad("record vector copy leaves", span))?
        {
            let value = V::decode(bytes, cursor, leaf.ty, span)?;
            self.store_leaf(to, leaf, value, span)?;
            cursor += vector_type::element(leaf.ty).size();
        }
        self.change_owner(frame, destination, AVAILABLE, span)
    }
    pub(super) fn record_vector_index_write(
        &mut self,
        frame: usize,
        base: AccessBase,
        path: &[FieldId],
        index_operand: Operand,
        source: OwnerPlaceId,
        span: Span,
    ) -> Result<()> {
        let from = self.base(frame, AccessBase::Owner(source), Access::Read, span)?;
        let AggregateTy::Record(record) = self.aggregate(from, span)? else {
            return Err(bad("record vector store source", span));
        };
        let (descriptor, start, end) =
            self.record_vector_region(frame, base, path, Access::Write, record, span)?;
        let ordinal = self.vector_index(frame, index_operand, end - start, span)?;
        let scratch = self.packed_record_value(frame, source, record, span)?;
        let element =
            vector_type::checked(VectorTy::Record(record), self.plan.witness().declarations())
                .map_err(|failure| heap_failure(failure, span))?;
        // SAFETY: complete source canonical validation and exclusive language
        // authority precede this single dense initialized-payload write.
        let target = unsafe {
            self.heap
                .vector_range_mut(&descriptor, element, start + ordinal, start + ordinal + 1)
        }
        .map_err(|failure| heap_failure(failure, span))?;
        if target.len() != scratch.len() {
            return Err(bad("record vector store extent", span));
        }
        target.copy_from_slice(&self.frames[frame].payload[scratch]);
        Ok(())
    }
}

#[cfg(test)]
#[path = "execute_vector_tests.rs"]
mod tests;
