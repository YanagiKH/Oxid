//! Lexical views into pinned canonical cells, never additional owned values.
use super::experimental::{heap_failure, BufferPlace, ByteRegion};
use super::*;
use crate::frontend::oir::owned::owned_record_codec::Element;

// Only resource_place constructs an address, after exact epoch/nominal/cell
// validation. It is deliberately move-only and has no raw-OIR representation.
// It may live across a child-resource operation, which cannot unlink or move
// the pinned outer vector. No address is retained in a VM frame or language
// reference; child call arguments retain the unchanged logical loan handles.
pub(super) struct CellAddress {
    pointer: std::ptr::NonNull<u8>,
    length: usize,
    permission: BorrowKind,
}
impl CellAddress {
    pub(super) fn len(&self) -> usize {
        self.length
    }
}
struct ElementOrigin<'a> {
    descriptor: byte_buffer::OwnedVectorDescriptor,
    element: Element<'a>,
    cell: usize,
}
// Shared adapters are entered by ordinary owning programs before a Cell
// alternative can be selected. Their argument, construction, complete return
// and caller transports therefore have an unconditional owner. The selector's
// own bool/getter roles remain in plan::element_loan_accounting::QueryCarriers;
// the conditional ControlCarriers below owns the additional Cell computation.
// These are named-value envelopes, not compiler-stack or RSS measurements.
#[allow(dead_code)]
struct SharedBaseTestCarriers<'a, V, W: PlanWitness> {
    receiver: &'a ValueMachine<'a, 'a, V, W>,
    frame_argument: usize,
    base_argument: AccessBase,
    root_argument: OwnerKey,
    span_argument: Span,
    returned: Result<bool>,
    caller_return: Result<bool>,
}
#[allow(dead_code)]
struct SharedExtentCarriers<'a, V, W: PlanWitness> {
    receiver: &'a ValueMachine<'a, 'a, V, W>,
    frame_argument: usize,
    base_argument: AccessBase,
    root_argument: OwnerKey,
    view_argument: BorrowView,
    span_argument: Span,
    returned: Result<Option<AggregateTy>>,
    caller_return: Result<Option<AggregateTy>>,
    selected: Option<AggregateTy>,
    base_test: SharedBaseTestCarriers<'a, V, W>,
}
#[allow(dead_code)]
struct SharedFieldPlaceCarriers<'a, V, W: PlanWitness> {
    receiver: &'a ValueMachine<'a, 'a, V, W>,
    frame_argument: usize,
    base_argument: AccessBase,
    root_argument: OwnerKey,
    field_argument: FieldId,
    span_argument: Span,
    returned: Result<Option<(BufferPlace, hir::Ty)>>,
    caller_return: Result<Option<(BufferPlace, hir::Ty)>>,
    selected: Option<(BufferPlace, hir::Ty)>,
    base_test: SharedBaseTestCarriers<'a, V, W>,
}
#[allow(dead_code)]
struct SharedReadLeafCarriers<'a, V, W: PlanWitness> {
    receiver: &'a ValueMachine<'a, 'a, V, W>,
    frame_argument: usize,
    base_argument: AccessBase,
    root_argument: OwnerKey,
    leaf_argument: ScalarLeaf,
    span_argument: Span,
    returned: Result<V>,
    caller_return: Result<V>,
    selected: V,
    base_test: SharedBaseTestCarriers<'a, V, W>,
}
#[allow(dead_code)]
struct SharedWriteLeafCarriers<'a, V, W: PlanWitness> {
    receiver: &'a mut ValueMachine<'a, 'a, V, W>,
    frame_argument: usize,
    base_argument: AccessBase,
    root_argument: OwnerKey,
    leaf_argument: ScalarLeaf,
    value_argument: V,
    span_argument: Span,
    returned: Result<()>,
    caller_return: Result<()>,
    base_test: SharedBaseTestCarriers<'a, V, W>,
}
#[allow(dead_code)]
struct SharedResourcePlaceCarriers<'a, V, W: PlanWitness> {
    receiver: &'a ValueMachine<'a, 'a, V, W>,
    frame_argument: usize,
    base_argument: AccessBase,
    root_argument: OwnerKey,
    relative_argument: u64,
    bytes_argument: usize,
    span_argument: Span,
    activation_extent: std::ops::Range<usize>,
    extent_return: Result<std::ops::Range<usize>>,
    converted_relative: Result<usize>,
    start: usize,
    checked_start: Option<usize>,
    end: usize,
    checked_end: Option<usize>,
    constructed: BufferPlace,
    returned: Result<BufferPlace>,
    caller_return: Result<BufferPlace>,
    caller_place: BufferPlace,
}
#[allow(dead_code)]
struct SharedBufferAccessCarriers<'a, V, W: PlanWitness> {
    receiver: &'a ValueMachine<'a, 'a, V, W>,
    frame_argument: usize,
    base_argument: AccessBase,
    root_argument: OwnerKey,
    relative_argument: u64,
    span_argument: Span,
    // The inherited buffer_at result remains in its original bank. These are
    // the newly inserted buffer_at_access forwarding and caller transports.
    forwarded: Result<(BufferPlace, byte_buffer::Descriptor)>,
    caller_return: Result<(BufferPlace, byte_buffer::Descriptor)>,
}
#[allow(dead_code)]
struct SharedVectorAccessCarriers<'a, V, W: PlanWitness> {
    receiver: &'a ValueMachine<'a, 'a, V, W>,
    frame_argument: usize,
    base_argument: AccessBase,
    root_argument: OwnerKey,
    relative_argument: u64,
    element_argument: super::super::vector_type::VectorTy,
    span_argument: Span,
    // The existing VectorPlace/descriptor result in vector::ControlCarriers
    // still owns its old operation transport. This wrapper is an added layer.
    forwarded: Result<(super::vector::VectorPlace, byte_buffer::VectorDescriptor)>,
    caller_return: Result<(super::vector::VectorPlace, byte_buffer::VectorDescriptor)>,
}
#[allow(dead_code)]
struct SharedBytesCarriers<'a, V> {
    frames_argument: &'a [ValueFrame<V>],
    place_argument: &'a BufferPlace,
    activation_range: std::ops::Range<usize>,
    returned: &'a [u8],
    caller_return: &'a [u8],
}
#[allow(dead_code)]
struct SharedBytesMutCarriers<'a, V> {
    frames_argument: &'a mut [ValueFrame<V>],
    place_argument: &'a BufferPlace,
    activation_range: std::ops::Range<usize>,
    returned: &'a mut [u8],
    caller_return: &'a mut [u8],
}
#[allow(dead_code)]
struct SharedResourceBytesCarriers<'a, V, W: PlanWitness> {
    receiver: &'a ValueMachine<'a, 'a, V, W>,
    place_argument: &'a BufferPlace,
    forwarded: &'a [u8],
    caller_return: &'a [u8],
    byte_adapter: SharedBytesCarriers<'a, V>,
}
#[allow(dead_code)]
struct SharedResourceBytesMutCarriers<'a, V, W: PlanWitness> {
    receiver: &'a mut ValueMachine<'a, 'a, V, W>,
    place_argument: &'a BufferPlace,
    forwarded: &'a mut [u8],
    caller_return: &'a mut [u8],
    byte_adapter: SharedBytesMutCarriers<'a, V>,
}
#[allow(dead_code)]
struct SharedActivationReadCarriers<'a, V, W: PlanWitness> {
    // Old Activation alternatives now construct this independent place before
    // entering resource_bytes; Cell alternatives keep their conditional bank.
    constructed_place: BufferPlace,
    pointer: *const u8,
    length: usize,
    reconstructed_bytes: &'a [u8],
    resource_bytes: SharedResourceBytesCarriers<'a, V, W>,
}
#[allow(dead_code)]
struct SharedActivationDirectReadCarriers<'a, V> {
    constructed_place: BufferPlace,
    bytes: SharedBytesCarriers<'a, V>,
}
#[allow(dead_code)]
struct SharedActivationDirectWriteCarriers<'a, V> {
    constructed_place: BufferPlace,
    bytes: SharedBytesMutCarriers<'a, V>,
}
#[allow(dead_code)]
struct SharedAdapterCarriers<'a, V, W: PlanWitness> {
    field_write: SharedFieldPlaceCarriers<'a, V, W>,
    handle_view_extent: SharedExtentCarriers<'a, V, W>,
    borrowed_projection_extent: SharedExtentCarriers<'a, V, W>,
    projection_base: SharedBaseTestCarriers<'a, V, W>,
    projection_vector_read: SharedBaseTestCarriers<'a, V, W>,
    projection_vector_write: SharedBaseTestCarriers<'a, V, W>,
    projection_vector_length: SharedBaseTestCarriers<'a, V, W>,
    projection_leaf_read: SharedReadLeafCarriers<'a, V, W>,
    projection_leaf_write: SharedWriteLeafCarriers<'a, V, W>,
    fixed_array_read: SharedReadLeafCarriers<'a, V, W>,
    fixed_array_write: SharedWriteLeafCarriers<'a, V, W>,
    buffer_slice_access: SharedBufferAccessCarriers<'a, V, W>,
    byte_region_buffer_access: SharedBufferAccessCarriers<'a, V, W>,
    byte_buffer_parameter_access: SharedBufferAccessCarriers<'a, V, W>,
    byte_buffer_borrow_access: SharedBufferAccessCarriers<'a, V, W>,
    fixed_array_byte_region: SharedResourcePlaceCarriers<'a, V, W>,
    file_parameter_place: SharedResourcePlaceCarriers<'a, V, W>,
    vector_slice_access: SharedVectorAccessCarriers<'a, V, W>,
    vector_view_access: SharedVectorAccessCarriers<'a, V, W>,
    vector_stage_access: SharedVectorAccessCarriers<'a, V, W>,
    vector_region_access: SharedVectorAccessCarriers<'a, V, W>,
    vector_range_access: SharedVectorAccessCarriers<'a, V, W>,
    vector_capacity_access: SharedVectorAccessCarriers<'a, V, W>,
    vector_overlap_access: SharedVectorAccessCarriers<'a, V, W>,
    vector_projection_access: SharedVectorAccessCarriers<'a, V, W>,
    fixed_array_vector_region: SharedResourcePlaceCarriers<'a, V, W>,
    byte_buffer_take_writeback: SharedResourceBytesMutCarriers<'a, V, W>,
    byte_buffer_extend_activation: SharedActivationReadCarriers<'a, V, W>,
    byte_buffer_operation_writeback: SharedResourceBytesMutCarriers<'a, V, W>,
    file_parameter_descriptor: SharedResourceBytesCarriers<'a, V, W>,
    file_open_activation: SharedActivationReadCarriers<'a, V, W>,
    file_read_activation: SharedActivationDirectWriteCarriers<'a, V>,
    file_write_activation: SharedActivationDirectReadCarriers<'a, V>,
    file_close_writeback: SharedResourceBytesMutCarriers<'a, V, W>,
    binary_read_activation: SharedActivationDirectWriteCarriers<'a, V>,
    binary_write_activation: SharedActivationDirectReadCarriers<'a, V>,
    vector_parameter_descriptor: SharedResourceBytesCarriers<'a, V, W>,
    vector_take_writeback: SharedResourceBytesMutCarriers<'a, V, W>,
    vector_extend_activation: SharedActivationReadCarriers<'a, V, W>,
    vector_operation_writeback: SharedResourceBytesMutCarriers<'a, V, W>,
    owned_vector_parameter_descriptor: SharedResourceBytesCarriers<'a, V, W>,
    owned_vector_operation_writeback: SharedResourceBytesMutCarriers<'a, V, W>,
}
pub(super) const fn shared_adapter_control_bytes<V: StoredValue, W: PlanWitness>() -> usize {
    size_of::<SharedAdapterCarriers<'_, V, W>>()
}

#[allow(dead_code)]
struct ControlCarriers<'a, V, W: PlanWitness> {
    witness: &'a W,
    keys: [LoanKey; 4],
    owner: OwnerKey,
    handles: [ReferenceHandle; 3],
    views: [BorrowView; 3],
    declaration: &'a LoanDecl,
    function: &'a RawOwnedFunction,
    origin: ElementOrigin<'a>,
    origin_return: Result<Option<ElementOrigin<'a>>>,
    caller_origin_return: Result<Option<ElementOrigin<'a>>>,
    place: BufferPlace,
    nested_places: [BufferPlace; 2],
    place_arguments: [&'a BufferPlace; 3],
    projection: (ElementOrigin<'a>, usize, AggregateTy),
    projection_result: Result<Option<(ElementOrigin<'a>, usize, AggregateTy)>>,
    caller_projection_result: Result<Option<(ElementOrigin<'a>, usize, AggregateTy)>>,
    place_return: Result<BufferPlace>,
    // read_field's guarded helper and caller; ordinary WriteField adapters
    // have their own unconditional SharedFieldPlaceCarriers above.
    optional_place_return: Result<Option<(BufferPlace, hir::Ty)>>,
    caller_optional_place_return: Result<Option<(BufferPlace, hir::Ty)>>,
    descriptor_return:
        std::result::Result<byte_buffer::OwnedVectorDescriptor, byte_buffer::Failure>,
    selected_type: Option<ValueTy>,
    projection_return: std::result::Result<(ValueTy, usize), DeclarationError>,
    offsets: [usize; 8],
    arithmetic: [Option<usize>; 4],
    byte_views: [&'a [u8]; 2],
    mutable_bytes: &'a mut [u8],
    pointer: *const u8,
    cell_address: CellAddress,
    raw_address_return: std::result::Result<std::ptr::NonNull<u8>, byte_buffer::Failure>,
    mapped_address_return: Result<std::ptr::NonNull<u8>>,
    permission_counts: [(u64, u64); 2],
    selected_count: u64,
    offset_result: usize,
    fuel: usize,
    scalar: V,
    scalar_arguments: [Operand; 2],
    scalar_returns: [Result<V>; 3],
    decoded_index: Option<Scalar>,
    vector_type: super::super::vector_type::VectorTy,
    vector_descriptor: byte_buffer::VectorDescriptor,
    vector_descriptor_return: Result<byte_buffer::VectorDescriptor>,
    vector_raw_descriptor_return:
        std::result::Result<byte_buffer::VectorDescriptor, byte_buffer::Failure>,
    selected_record: RecordId,
    record_option: Option<RecordId>,
    scalar_type: hir::Ty,
    selected_array: FixedArrayTy,
    granted_kind: BorrowKind,
    element_return:
        std::result::Result<Element<'a>, super::super::owned_record_codec::bounds::ShapeFailure>,
    mapped_element_return: Result<Element<'a>>,
    root_return: Result<OwnerKey>,
    handle_return: Result<Option<ReferenceHandle>>,
    borrowed_view_return: Result<BorrowView>,
    child_ancestry_return: Result<()>,
    selected_extent: std::ops::Range<usize>,
    extent_return: Result<std::ops::Range<usize>>,
    raw_offset: Option<Operand>,
    operand_option: Option<Operand>,
    query_returns: [Result<bool>; 2],
    origin_offset_return: Result<Option<u64>>,
    byte_region: ByteRegion,
    byte_region_return: Result<ByteRegion>,
    selected_runtime: LoanRuntime,
    next_instance: u64,
    nominal: AggregateSlot,
    nominal_return: std::result::Result<AggregateSlot, DeclarationError>,
    acquisition_return: Result<()>,
    raw_acquisition_return: std::result::Result<(), byte_buffer::Failure>,
    mapped_acquisition_return: Result<()>,
    view_result: Result<Option<AggregateTy>>,
}
pub(super) const fn control_bytes<V: StoredValue, W: PlanWitness>() -> usize {
    size_of::<ControlCarriers<'_, V, W>>() + byte_buffer::owned_vector_borrow_control_bytes()
}

impl<V: StoredValue, W: PlanWitness> ValueMachine<'_, '_, V, W> {
    fn element_origin(
        &self,
        mut permission: LoanKey,
        root: OwnerKey,
        span: Span,
    ) -> Result<Option<ElementOrigin<'_>>> {
        if !self.plan.has_local_record_element_loans() {
            return Ok(None);
        }
        for _ in 0..plan::MAX_FRAMES * (MAX_LOCAL_REFERENCE_DEPTH + 1) {
            let state = self.loan(permission, span)?;
            if state.root != root {
                return Err(bad("element root ancestry", span));
            }
            let frame = index(permission.frame, span)?;
            let f = self.function(self.frames[frame].function);
            let id = LoanId(index(permission.loan, span)?);
            if let Some(record) = shape::local_record_element(f, id) {
                let row = &f.loans[id.0];
                let parent = self.element_authority_handle(frame, row.authority, span)?;
                let source = if let Some(parent) = parent {
                    self.validate_handle_identity(parent, span)?;
                    if state.parent != parent.permission || parent.root != root {
                        return Err(bad("element parent authority", span));
                    }
                    parent.view
                } else {
                    if state.parent.instance != 0 {
                        return Err(bad("element root parent", span));
                    }
                    let AccessBase::Owner(owner) = row.authority else {
                        return Err(bad("element root authority", span));
                    };
                    if root.frame != frame as u64
                        || root.activation != self.frames[frame].activation
                        || root.owner != owner.0 as u64
                    {
                        return Err(bad("element root owner identity", span));
                    }
                    self.whole_view(root, span)?
                };
                if source.aggregate.map(AggregateSlot::aggregate)
                    != Some(AggregateTy::OwnedRecordVector(record))
                {
                    return Err(bad("element vector identity", span));
                }
                let element = self
                    .plan
                    .witness()
                    .elements()
                    .ok_or_else(|| bad("element owning witness", span))?
                    .element(record)
                    .map_err(|_| bad("element finite record", span))?;
                let owner = self.owner_extent(root, span)?;
                let start = owner
                    .start
                    .checked_add(index(source.offset, span)?)
                    .ok_or_else(|| bad("element vector offset", span))?;
                let end = start
                    .checked_add(16)
                    .filter(|end| *end <= owner.end)
                    .ok_or_else(|| bad("element vector extent", span))?;
                let descriptor = self
                    .heap
                    .owned_vector_descriptor(
                        &self.frames[index(root.frame, span)?].payload[start..end],
                        element,
                    )
                    .map_err(|failure| heap_failure(failure, span))?;
                if state.view.aggregate.map(AggregateSlot::aggregate)
                    != Some(AggregateTy::Record(record))
                {
                    return Err(bad("element retained nominal", span));
                }
                let cell = index(state.view.offset, span)?;
                if !cell.is_multiple_of(element.size()) {
                    return Err(bad("element retained ordinal", span));
                }
                let operand = self
                    .plan
                    .function(self.frames[frame].function)
                    .element_index(id)
                    .ok_or_else(|| bad("element acquisition site", span))?;
                let Some(Scalar::I32(ordinal)) = self.read(frame, operand)?.narrow() else {
                    return Err(bad("element captured index", span));
                };
                if ordinal < 0 || (ordinal as usize).checked_mul(element.size()) != Some(cell) {
                    return Err(bad("element captured ordinal", span));
                }
                self.heap
                    .owned_vector_cell_bytes(&descriptor, element, cell, element.size())
                    .map_err(|failure| heap_failure(failure, span))?;
                return Ok(Some(ElementOrigin {
                    descriptor,
                    element,
                    cell,
                }));
            }
            if state.parent.instance == 0 {
                return Ok(None);
            }
            permission = state.parent;
        }
        Err(bad("element ancestry bound", span))
    }
    fn element_authority_handle(
        &self,
        frame: usize,
        base: AccessBase,
        span: Span,
    ) -> Result<Option<ReferenceHandle>> {
        match base {
            AccessBase::Owner(_) => Ok(None),
            AccessBase::Parameter(id) => self.frames[frame]
                .references
                .get(id.0)
                .copied()
                .map(Some)
                .ok_or_else(|| bad("element reference identity", span)),
            AccessBase::Loan(id) => self.reference_from_loan(frame, id, span).map(Some),
        }
    }
    fn element_projection(
        &self,
        mut permission: LoanKey,
        root: OwnerKey,
        span: Span,
    ) -> Result<Option<(ElementOrigin<'_>, usize, AggregateTy)>> {
        let Some(origin) = self.element_origin(permission, root, span)? else {
            return Ok(None);
        };
        let mut offset = 0usize;
        let mut target = None;
        for _ in 0..plan::MAX_FRAMES * (MAX_LOCAL_REFERENCE_DEPTH + 1) {
            let state = self.loan(permission, span)?;
            let f = self.function(self.frames[index(permission.frame, span)?].function);
            let id = LoanId(index(permission.loan, span)?);
            let row = f
                .loans
                .get(id.0)
                .ok_or_else(|| bad("element projection loan", span))?;
            if shape::local_record_element(f, id).is_some() {
                let aggregate = target.unwrap_or(origin.element.root().aggregate());
                let relative = origin
                    .cell
                    .checked_add(offset)
                    .ok_or_else(|| bad("element projection offset", span))?;
                return Ok(Some((origin, relative, aggregate)));
            }
            if !row.projection.is_empty() {
                let (BorrowedTy::Exact(base), _) = shape::base(f, row.authority, span)
                    .map_err(|_| bad("element projection authority", span))?
                else {
                    return Err(bad("element projection base", span));
                };
                let (ValueTy::Owned(projected), relative) = self
                    .plan
                    .witness()
                    .declarations()
                    .projection(base, &row.projection)
                    .map_err(|_| bad("element projection path", span))?
                else {
                    return Err(bad("element projected value", span));
                };
                if target.is_none() {
                    target = Some(projected);
                }
                offset = offset
                    .checked_add(relative)
                    .ok_or_else(|| bad("element projection offset", span))?;
            }
            if state.parent.instance == 0 {
                return Err(bad("element projection root", span));
            }
            permission = state.parent;
        }
        Err(bad("element projection ancestry", span))
    }
    pub(super) fn checked_element_view(
        &self,
        handle: ReferenceHandle,
        span: Span,
    ) -> Result<Option<AggregateTy>> {
        let Some((origin, relative, expected)) =
            self.element_projection(handle.permission, handle.root, span)?
        else {
            return Ok(None);
        };
        self.validate_handle_identity(handle, span)?;
        let actual = handle
            .view
            .aggregate
            .ok_or_else(|| bad("element view identity", span))?
            .aggregate();
        let row = self.loan(handle.permission, span)?;
        let f = self.function(self.frames[index(handle.permission.frame, span)?].function);
        let declared = f.loans[index(handle.permission.loan, span)?].referent();
        match declared {
            BorrowedTy::Exact(target) if target == expected && actual == target => {
                if index(handle.view.offset, span)? != relative {
                    return Err(bad("element view offset", span));
                }
            }
            BorrowedTy::ScalarSlice(ty)
                if expected == AggregateTy::ByteBuf && ty == hir::Ty::U8 && actual == expected =>
            {
                let ByteRegion::Buffer { value, .. } = self.buffer_slice(handle, span)? else {
                    return Err(bad("element byte region", span));
                };
                let length = self
                    .heap
                    .len(&value)
                    .map_err(|failure| heap_failure(failure, span))?;
                self.checked_element_buffer_ancestry(handle, length, span)?;
                return Ok(Some(actual));
            }
            BorrowedTy::ScalarSlice(ty)
                if expected == AggregateTy::Vector(ty) && actual == expected =>
            {
                self.vector_slice(
                    handle,
                    super::super::vector_type::VectorTy::Scalar(ty),
                    span,
                )?;
                return Ok(Some(actual));
            }
            BorrowedTy::RecordSlice(record)
                if expected == AggregateTy::RecordVector(record) && actual == expected =>
            {
                self.vector_slice(
                    handle,
                    super::super::vector_type::VectorTy::Record(record),
                    span,
                )?;
                return Ok(Some(actual));
            }
            BorrowedTy::ScalarSlice(ty) => {
                let (AggregateTy::FixedArray(parent), AggregateTy::FixedArray(child)) =
                    (expected, actual)
                else {
                    return Err(bad("element array view", span));
                };
                if parent.element() != ty || child.element() != ty {
                    return Err(bad("element array type", span));
                }
                self.checked_byte_array_provenance(handle, span)?;
                let offset = index(handle.view.offset, span)?;
                if offset < relative
                    || offset
                        .checked_add(child.length() * child.stride())
                        .is_none_or(|end| end > relative + parent.length() * parent.stride())
                {
                    return Err(bad("element array extent", span));
                }
            }
            _ => return Err(bad("element borrowed target", span)),
        }
        let size = match actual {
            AggregateTy::FixedArray(array) if array.length() == 0 => 0,
            _ => self
                .plan
                .witness()
                .aggregate_layout(actual)
                .map_err(|_| bad("element target layout", span))?
                .size(),
        };
        let offset = index(handle.view.offset, span)?;
        if offset < origin.cell
            || offset
                .checked_add(size)
                .is_none_or(|end| end > origin.cell + origin.element.size())
        {
            return Err(bad("element view extent", span));
        }
        let _ = row;
        Ok(Some(actual))
    }
    fn checked_element_buffer_ancestry(
        &self,
        mut handle: ReferenceHandle,
        length: i32,
        span: Span,
    ) -> Result<()> {
        for _ in 0..plan::MAX_FRAMES * (MAX_LOCAL_REFERENCE_DEPTH + 1) {
            self.validate_handle_identity(handle, span)?;
            let frame = index(handle.permission.frame, span)?;
            let id = LoanId(index(handle.permission.loan, span)?);
            let row = self.loan(handle.permission, span)?;
            let declaration = &self.function(self.frames[frame].function).loans[id.0];
            let child = match declaration.referent() {
                BorrowedTy::ScalarSlice(hir::Ty::U8) => {
                    let start = i32::try_from(handle.view.offset >> 32)
                        .map_err(|_| bad("element byte start", span))?;
                    let count = i32::try_from(handle.view.offset as u32)
                        .map_err(|_| bad("element byte length", span))?;
                    (
                        start,
                        start
                            .checked_add(count)
                            .ok_or_else(|| bad("element byte end", span))?,
                    )
                }
                BorrowedTy::Exact(AggregateTy::ByteBuf) => (0, length),
                _ => return Err(bad("element byte ancestry type", span)),
            };
            if child.0 < 0 || child.1 < child.0 || child.1 > length {
                return Err(bad("element byte extent", span));
            }
            let parent = self
                .element_authority_handle(frame, declaration.authority, span)?
                .ok_or_else(|| bad("element byte parent", span))?;
            if row.parent != parent.permission || row.root != parent.root {
                return Err(bad("element byte ancestry identity", span));
            }
            let parent_declared = self
                .function(self.frames[index(parent.permission.frame, span)?].function)
                .loans[index(parent.permission.loan, span)?]
            .referent();
            if declaration.projection.is_empty()
                && parent_declared == BorrowedTy::ScalarSlice(hir::Ty::U8)
            {
                if declaration.referent() != BorrowedTy::ScalarSlice(hir::Ty::U8) {
                    return Err(bad("element byte slice owner escalation", span));
                }
                let start = i32::try_from(parent.view.offset >> 32)
                    .map_err(|_| bad("element byte parent start", span))?;
                let count = i32::try_from(parent.view.offset as u32)
                    .map_err(|_| bad("element byte parent length", span))?;
                let end = start
                    .checked_add(count)
                    .ok_or_else(|| bad("element byte parent end", span))?;
                if child.0 < start || child.1 > end {
                    return Err(bad("element byte parent extent", span));
                }
            }
            if parent.view.aggregate.map(AggregateSlot::aggregate) != Some(AggregateTy::ByteBuf) {
                self.checked_handle_view(parent, span)?;
                return Ok(());
            }
            handle = parent;
        }
        Err(bad("element byte ancestry bound", span))
    }
    pub(super) fn element_resource_offset(
        &self,
        permission: LoanKey,
        root: OwnerKey,
        target: AggregateTy,
        span: Span,
    ) -> Result<Option<u64>> {
        let Some((_, relative, actual)) = self.element_projection(permission, root, span)? else {
            return Ok(None);
        };
        if actual != target {
            return Err(bad("element resource target", span));
        }
        Ok(Some(relative as u64))
    }
    pub(super) fn resource_place(
        &self,
        frame: usize,
        base: AccessBase,
        root: OwnerKey,
        relative: u64,
        bytes: usize,
        span: Span,
    ) -> Result<BufferPlace> {
        if self.plan.has_local_record_element_loans() {
            if let Some(handle) = self.element_authority_handle(frame, base, span)? {
                if let Some(origin) = self.element_origin(handle.permission, root, span)? {
                    let relative = index(relative, span)?;
                    if relative < origin.cell
                        || relative
                            .checked_add(bytes)
                            .is_none_or(|end| end > origin.cell + origin.element.size())
                    {
                        return Err(bad("element memory extent", span));
                    }
                    let pointer = self
                        .heap
                        .owned_vector_cell_address(
                            &origin.descriptor,
                            origin.element,
                            relative,
                            bytes,
                        )
                        .map_err(|failure| heap_failure(failure, span))?;
                    let permission = self.loan_kind(handle.permission, span)?;
                    return Ok(BufferPlace::Cell(CellAddress {
                        pointer,
                        length: bytes,
                        permission,
                    }));
                }
            }
        }
        let owner = self.owner_extent(root, span)?;
        let start = owner
            .start
            .checked_add(index(relative, span)?)
            .ok_or_else(|| bad("resource offset", span))?;
        let end = start
            .checked_add(bytes)
            .filter(|end| *end <= owner.end)
            .ok_or_else(|| bad("resource extent", span))?;
        Ok(BufferPlace::Activation {
            frame: index(root.frame, span)?,
            bytes: start..end,
        })
    }
    pub(super) fn resource_bytes<'a>(&'a self, place: &BufferPlace) -> &'a [u8] {
        bytes(&self.frames, place)
    }
    pub(super) fn resource_bytes_mut<'a>(&'a mut self, place: &BufferPlace) -> &'a mut [u8] {
        bytes_mut(&mut self.frames, place)
    }
    pub(super) fn resource_place_permission(
        &self,
        permission: LoanKey,
        root: OwnerKey,
        relative: u64,
        bytes: usize,
        span: Span,
    ) -> Result<BufferPlace> {
        self.resource_place(
            index(permission.frame, span)?,
            AccessBase::Loan(LoanId(index(permission.loan, span)?)),
            root,
            relative,
            bytes,
            span,
        )
    }
    pub(super) fn has_element_base(
        &self,
        frame: usize,
        base: AccessBase,
        root: OwnerKey,
        span: Span,
    ) -> Result<bool> {
        if !self.plan.has_local_record_element_loans() {
            return Ok(false);
        }
        let Some(handle) = self.element_authority_handle(frame, base, span)? else {
            return Ok(false);
        };
        Ok(self
            .element_origin(handle.permission, root, span)?
            .is_some())
    }
    pub(super) fn element_field_place(
        &self,
        frame: usize,
        base: AccessBase,
        root: OwnerKey,
        field: FieldId,
        span: Span,
    ) -> Result<Option<(BufferPlace, hir::Ty)>> {
        if !self.has_element_base(frame, base, root, span)? {
            return Ok(None);
        }
        let view = self.base_view(frame, base, root, span)?;
        let record = record_type(
            view.aggregate
                .ok_or_else(|| bad("element field view", span))?
                .aggregate(),
            span,
        )?;
        let field = self
            .plan
            .witness()
            .declarations()
            .field(record, field)
            .map_err(|_| bad("element field identity", span))?;
        let ValueTy::Scalar(ty) = field.value_ty() else {
            return Err(bad("element scalar field", span));
        };
        let relative = view
            .offset
            .checked_add(field.offset() as u64)
            .ok_or_else(|| bad("element field offset", span))?;
        Ok(Some((
            self.resource_place(frame, base, root, relative, scalar_size(ty), span)?,
            ty,
        )))
    }
    pub(super) fn read_access_leaf(
        &self,
        frame: usize,
        base: AccessBase,
        root: OwnerKey,
        leaf: ScalarLeaf,
        span: Span,
    ) -> Result<V> {
        if self.has_element_base(frame, base, root, span)? {
            let place = self.resource_place(
                frame,
                base,
                root,
                leaf.offset as u64,
                scalar_size(leaf.ty),
                span,
            )?;
            V::decode(self.resource_bytes(&place), 0, leaf.ty, span)
        } else {
            self.load_leaf(root, leaf, span)
        }
    }
    pub(super) fn write_access_leaf(
        &mut self,
        frame: usize,
        base: AccessBase,
        root: OwnerKey,
        leaf: ScalarLeaf,
        value: V,
        span: Span,
    ) -> Result<()> {
        if self.has_element_base(frame, base, root, span)? {
            if value.ty() != leaf.ty {
                return Err(bad("element scalar leaf type", span));
            }
            let place = self.resource_place(
                frame,
                base,
                root,
                leaf.offset as u64,
                scalar_size(leaf.ty),
                span,
            )?;
            value.encode(self.resource_bytes_mut(&place), 0, span)
        } else {
            self.store_leaf(root, leaf, value, span)
        }
    }
    pub(super) fn staged_element_extent(
        &self,
        frame: usize,
        base: AccessBase,
        root: OwnerKey,
        view: BorrowView,
        span: Span,
    ) -> Result<Option<AggregateTy>> {
        if !self.has_element_base(frame, base, root, span)? {
            return Ok(None);
        }
        let actual = view
            .aggregate
            .ok_or_else(|| bad("element staged view", span))?
            .aggregate();
        let bytes = match actual {
            AggregateTy::FixedArray(array) if array.length() == 0 => 0,
            _ => self
                .plan
                .witness()
                .aggregate_layout(actual)
                .map_err(|_| bad("element staged layout", span))?
                .size(),
        };
        self.resource_place(frame, base, root, view.offset, bytes, span)?;
        Ok(Some(actual))
    }
    fn element_vector_descriptor(
        &self,
        frame: usize,
        base: AccessBase,
        root: OwnerKey,
        relative: usize,
        ty: super::super::vector_type::VectorTy,
        span: Span,
    ) -> Result<byte_buffer::VectorDescriptor> {
        let place = self.resource_place(frame, base, root, relative as u64, 16, span)?;
        let element = super::super::vector_type::checked(ty, self.plan.witness().declarations())
            .map_err(|failure| heap_failure(failure, span))?;
        self.heap
            .vector_descriptor(self.resource_bytes(&place), element)
            .map_err(|failure| heap_failure(failure, span))
    }
    // Keep each opcode operand explicit for the runtime accounting model.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn element_vector_read(
        &mut self,
        frame: usize,
        destination: LocalId,
        base: AccessBase,
        root: OwnerKey,
        ty: hir::Ty,
        relative: usize,
        operand: Operand,
        span: Span,
    ) -> Result<()> {
        let descriptor = self.element_vector_descriptor(
            frame,
            base,
            root,
            relative,
            super::super::vector_type::VectorTy::Scalar(ty),
            span,
        )?;
        let element = super::super::vector_type::element(ty);
        let length = self
            .heap
            .vector_len(&descriptor, element)
            .map_err(|failure| heap_failure(failure, span))?;
        let ordinal = self.vector_index(frame, operand, length, span)?;
        let bytes = self
            .heap
            .vector_range(&descriptor, element, ordinal, ordinal + 1)
            .map_err(|failure| heap_failure(failure, span))?;
        let value = V::decode(bytes, 0, ty, span)?;
        self.write(frame, destination, value, span)
    }
    // Keep each opcode operand explicit for the runtime accounting model.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn element_vector_write(
        &mut self,
        frame: usize,
        base: AccessBase,
        root: OwnerKey,
        ty: hir::Ty,
        relative: usize,
        operand: Operand,
        value: Operand,
        span: Span,
    ) -> Result<()> {
        let descriptor = self.element_vector_descriptor(
            frame,
            base,
            root,
            relative,
            super::super::vector_type::VectorTy::Scalar(ty),
            span,
        )?;
        let length = self
            .heap
            .vector_len(&descriptor, super::super::vector_type::element(ty))
            .map_err(|failure| heap_failure(failure, span))?;
        let ordinal = self.vector_index(frame, operand, length, span)?;
        let value = self.read(frame, value)?;
        self.vector_store(&descriptor, ty, ordinal, value, span)
    }
    // Keep each opcode operand explicit for the runtime accounting model.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn element_vector_length(
        &mut self,
        frame: usize,
        destination: LocalId,
        base: AccessBase,
        root: OwnerKey,
        ty: super::super::vector_type::VectorTy,
        relative: usize,
        span: Span,
    ) -> Result<()> {
        let descriptor = self.element_vector_descriptor(frame, base, root, relative, ty, span)?;
        let element = super::super::vector_type::checked(ty, self.plan.witness().declarations())
            .map_err(|failure| heap_failure(failure, span))?;
        let length = self
            .heap
            .vector_len(&descriptor, element)
            .map_err(|failure| heap_failure(failure, span))?;
        self.write(frame, destination, Scalar::I32(length), span)
    }
    pub(super) fn acquire_element(
        &mut self,
        frame: usize,
        id: LoanId,
        operand: Operand,
        span: Span,
    ) -> Result<()> {
        if self
            .plan
            .function(self.frames[frame].function)
            .element_index(id)
            != Some(operand)
        {
            return Err(bad("element acquisition plan relation", span));
        }
        let f = self.function(self.frames[frame].function);
        let record = shape::local_record_element(f, id)
            .ok_or_else(|| bad("element acquisition declaration", span))?;
        let declaration = &f.loans[id.0];
        let root = self.base(
            frame,
            declaration.authority,
            Access::Borrow(declaration.kind),
            span,
        )?;
        let view = self.base_view(frame, declaration.authority, root, span)?;
        if view.aggregate.map(AggregateSlot::aggregate)
            != Some(AggregateTy::OwnedRecordVector(record))
        {
            return Err(bad("element source type", span));
        }
        let element = self
            .plan
            .witness()
            .elements()
            .ok_or_else(|| bad("element owning witness", span))?
            .element(record)
            .map_err(|_| bad("element checked root", span))?;
        let place =
            self.resource_place(frame, declaration.authority, root, view.offset, 16, span)?;
        let value = self
            .heap
            .owned_vector_descriptor(self.resource_bytes(&place), element)
            .map_err(|failure| heap_failure(failure, span))?;
        let Some(Scalar::I32(ordinal)) = self.read(frame, operand)?.narrow() else {
            return Err(bad("element index type", span));
        };
        let parent = self
            .element_authority_handle(frame, declaration.authority, span)?
            .map_or(LoanKey::default(), |handle| handle.permission);
        let previous = self.frames[frame].loans[id.0];
        if previous.state != 0 {
            return Err(bad("element loan already active", span));
        }
        let instance = epoch(previous.instance, span)?;
        let nominal = AggregateSlot::try_from_aggregate(AggregateTy::Record(record))
            .map_err(|_| bad("element nominal slot", span))?;
        let release_frame = index(
            if parent.instance == 0 {
                root.frame
            } else {
                parent.frame
            },
            span,
        )?;
        let release_index = index(
            if parent.instance == 0 {
                root.owner
            } else {
                parent.loan
            },
            span,
        )?;
        let counts = if parent.instance == 0 {
            let owner = self.owner(root, span)?;
            (owner.shared_children, owner.exclusive_children)
        } else {
            let row = self.loan(parent, span)?;
            (row.shared_children, row.exclusive_children)
        };
        let next = if declaration.kind == BorrowKind::Shared {
            counts.0
        } else {
            counts.1
        }
        .checked_add(1)
        .ok_or_else(|| bad("element loan counter", span))?;
        let mut offset = 0;
        let mut fuel = self.fuel;
        self.heap
            .owned_vector_borrow_record(&value, element, ordinal, &mut fuel, &mut offset)
            .map_err(|failure| heap_failure(failure, span))?;
        self.fuel = fuel;
        if parent.instance == 0 {
            let owner = &mut self.frames[release_frame].owners[release_index];
            if declaration.kind == BorrowKind::Shared {
                owner.shared_children = next;
            } else {
                owner.exclusive_children = next;
            }
        } else {
            let row = &mut self.frames[release_frame].loans[release_index];
            if declaration.kind == BorrowKind::Shared {
                row.shared_children = next;
            } else {
                row.exclusive_children = next;
            }
        }
        self.frames[frame].loans[id.0] = LoanRuntime {
            instance,
            state: 1,
            root,
            parent,
            view: BorrowView {
                offset: offset as u64,
                aggregate: Some(nominal),
            },
            shared_children: 0,
            exclusive_children: 0,
        };
        #[cfg(test)]
        self.record_event(Event::Acquire(
            LoanKey {
                frame: frame as u64,
                activation: self.frames[frame].activation,
                loan: id.0 as u64,
                instance,
            },
            root,
            declaration.kind,
        ));
        Ok(())
    }
}

// Keep the frame borrow separate from heap/fuel control. There is no sentinel
// index and no pointer-valued Range: activation coordinates are always actual
// frame coordinates. CellAddress can only be constructed by resource_place.
pub(super) fn bytes<'a, V>(frames: &'a [ValueFrame<V>], place: &BufferPlace) -> &'a [u8] {
    match place {
        BufferPlace::Activation { frame, bytes } => &frames[*frame].payload[bytes.clone()],
        BufferPlace::Cell(address) => {
            // SAFETY: the checked live outer loan pins this allocation for the
            // enclosing synchronous primitive, including disjoint child growth.
            unsafe { std::slice::from_raw_parts(address.pointer.as_ptr(), address.length) }
        }
    }
}
pub(super) fn bytes_mut<'a, V>(
    frames: &'a mut [ValueFrame<V>],
    place: &BufferPlace,
) -> &'a mut [u8] {
    match place {
        BufferPlace::Activation { frame, bytes } => &mut frames[*frame].payload[bytes.clone()],
        BufferPlace::Cell(address) => {
            // A caller must validate write access before any effect. This
            // assertion also prevents an internal read-only place from being
            // used to manufacture a mutable host slice.
            assert_eq!(address.permission, BorrowKind::Exclusive);
            // SAFETY: exclusive language authority and the pinned outer node
            // remain live until this synchronous operation's final publication.
            unsafe { std::slice::from_raw_parts_mut(address.pointer.as_ptr(), address.length) }
        }
    }
}

#[cfg(test)]
#[path = "execute_element_loans_tests.rs"]
mod tests;
