//! Canonical dynamic operations. All result locations and loans are checked
//! before mutation or I/O; result commit has no allocation, charge or failure.
use super::*;
use crate::frontend::builtin_catalog::experimental::Function;
use std::ops::Range;

#[path = "execute_io_delivery.rs"]
mod delivery;
#[path = "execute_file_delivery.rs"]
mod file_delivery;
pub(super) use delivery::argument_control_bytes as argument_delivery_control_bytes;
pub(super) use delivery::control_bytes as binary_delivery_control_bytes;
pub(super) use file_delivery::control_bytes as file_delivery_control_bytes;

pub(super) enum ByteRegion {
    Activation {
        frame: usize,
        bytes: Range<usize>,
    },
    Buffer {
        value: byte_buffer::Descriptor,
        start: i32,
        end: i32,
    },
}
struct BufferPlace {
    frame: usize,
    bytes: Range<usize>,
}
enum ResultPlace {
    Scalar(LocalId),
    Owner {
        owner: OwnerPlaceId,
        bytes: Range<usize>,
        generation: u64,
    },
}
#[derive(Clone, Copy)]
enum OperationValue {
    Scalar(Scalar),
    EmptyBuffer,
    Status { tag: u32, payload: Option<i32> },
}
fn heap_failure(failure: byte_buffer::Failure, span: Span) -> OwnedRunFailure {
    match failure {
        byte_buffer::Failure::Bounds => OwnedRunFailure::Bounds(span),
        byte_buffer::Failure::Fuel => RunFailure::Fuel(span).into(),
        byte_buffer::Failure::Descriptor => bad("resource descriptor", span),
        byte_buffer::Failure::IdentityExhausted => bad("allocation identity exhausted", span),
    }
}
fn io_failure(failure: binary_io::Failure, span: Span) -> OwnedRunFailure {
    match failure {
        binary_io::Failure::Bounds => OwnedRunFailure::Bounds(span),
        binary_io::Failure::Fuel => RunFailure::Fuel(span).into(),
        binary_io::Failure::Host => OwnedRunFailure::BinaryHost(span),
    }
}
impl<V: StoredValue> ValueMachine<'_, '_, V> {
    fn buffer_at(
        &self,
        root: OwnerKey,
        relative: u64,
        span: Span,
    ) -> Result<(BufferPlace, byte_buffer::Descriptor)> {
        let owner = self.owner_extent(root, span)?;
        let start = owner
            .start
            .checked_add(index(relative, span)?)
            .ok_or_else(|| bad("buffer offset", span))?;
        let end = start
            .checked_add(16)
            .filter(|end| *end <= owner.end)
            .ok_or_else(|| bad("buffer extent", span))?;
        let frame = index(root.frame, span)?;
        let bytes = start..end;
        let value = self
            .heap
            .descriptor(&self.frames[frame].payload[bytes.clone()])
            .map_err(|failure| heap_failure(failure, span))?;
        Ok((BufferPlace { frame, bytes }, value))
    }
    /// The descriptor address is re-derived from checked nominal paths. A slice
    /// stores only its initialized range and can never manufacture an owner.
    fn resource_origin(&self, mut permission: LoanKey, root: OwnerKey, span: Span) -> Result<u64> {
        if matches!(self.aggregate(root, span)?, AggregateTy::Record(_)) {
            let view = self
                .projected_view(permission, root, span)
                .map_err(|_| bad("resource projection", span))?;
            if view.aggregate.map(AggregateSlot::aggregate) != Some(AggregateTy::ByteBuf) {
                return Err(bad("resource projection type", span));
            }
            return Ok(view.offset);
        }
        for _ in 0..plan::MAX_FRAMES {
            let loan = self.loan(permission, span)?;
            if loan.root != root {
                return Err(bad("resource loan root", span));
            }
            let f = self.function(self.frames[index(permission.frame, span)?].function);
            let declaration = f
                .loans
                .get(index(permission.loan, span)?)
                .ok_or_else(|| bad("resource loan identity", span))?;
            if !declaration.projection.is_empty() {
                let (value, offset) = self
                    .plan
                    .witness()
                    .declarations()
                    .projection(self.aggregate(root, span)?, &declaration.projection)
                    .map_err(|_| bad("resource projection", span))?;
                if value != ValueTy::Owned(AggregateTy::ByteBuf) {
                    return Err(bad("resource projection type", span));
                }
                return Ok(offset as u64);
            }
            if loan.parent.instance == 0 {
                if self.aggregate(root, span)? != AggregateTy::ByteBuf {
                    return Err(bad("resource root type", span));
                }
                return Ok(0);
            }
            permission = loan.parent;
        }
        Err(bad("resource ancestry", span))
    }
    pub(super) fn buffer_slice(&self, handle: ReferenceHandle, span: Span) -> Result<ByteRegion> {
        let offset = self.resource_origin(handle.permission, handle.root, span)?;
        let (_, value) = self.buffer_at(handle.root, offset, span)?;
        let start = (handle.view.offset >> 32) as u32;
        let length = handle.view.offset as u32;
        let end = start
            .checked_add(length)
            .and_then(|end| i32::try_from(end).ok())
            .ok_or(OwnedRunFailure::Bounds(span))?;
        let start = i32::try_from(start).map_err(|_| OwnedRunFailure::Bounds(span))?;
        self.heap
            .range(&value, start, end)
            .map_err(|failure| heap_failure(failure, span))?;
        Ok(ByteRegion::Buffer { value, start, end })
    }
    pub(super) fn byte_region(
        &self,
        frame: usize,
        base: AccessBase,
        access: Access,
        span: Span,
    ) -> Result<ByteRegion> {
        let root = self.base(frame, base, access, span)?;
        let view = self.base_view(frame, base, root, span)?;
        match view.aggregate.map(AggregateSlot::aggregate) {
            Some(AggregateTy::ByteBuf) => {
                if let AccessBase::Parameter(reference) = base {
                    if self.function(self.frames[frame].function).references[reference.0].referent()
                        == BorrowedTy::ScalarSlice(hir::Ty::U8)
                    {
                        return self.buffer_slice(self.frames[frame].references[reference.0], span);
                    }
                }
                let (_, value) = self.buffer_at(root, view.offset, span)?;
                let end = self
                    .heap
                    .len(&value)
                    .map_err(|failure| heap_failure(failure, span))?;
                Ok(ByteRegion::Buffer {
                    value,
                    start: 0,
                    end,
                })
            }
            Some(AggregateTy::FixedArray(array)) if array.element() == hir::Ty::U8 => {
                let extent = self.owner_extent(root, span)?;
                let start = extent
                    .start
                    .checked_add(index(view.offset, span)?)
                    .ok_or_else(|| bad("byte view offset", span))?;
                let end = start
                    .checked_add(array.length())
                    .filter(|end| *end <= extent.end)
                    .ok_or_else(|| bad("byte view extent", span))?;
                Ok(ByteRegion::Activation {
                    frame: index(root.frame, span)?,
                    bytes: start..end,
                })
            }
            _ => Err(bad("byte view type", span)),
        }
    }
    fn buffer_parameter(
        &self,
        frame: usize,
        access: Access,
        span: Span,
    ) -> Result<(BufferPlace, byte_buffer::Descriptor)> {
        let reference = ReferenceParamId(0);
        let f = self.function(self.frames[frame].function);
        if f.references
            .first()
            .is_none_or(|reference| reference.referent() != BorrowedTy::Exact(AggregateTy::ByteBuf))
        {
            return Err(bad("buffer parameter type", span));
        }
        let root = self.base(frame, AccessBase::Parameter(reference), access, span)?;
        let view = self.base_view(frame, AccessBase::Parameter(reference), root, span)?;
        self.buffer_at(root, view.offset, span)
    }
    fn prepare_experimental_result(
        &self,
        frame: usize,
        destination: CallResult,
        span: Span,
    ) -> Result<ResultPlace> {
        let f = self.function(self.frames[frame].function);
        match destination {
            CallResult::Scalar(local) => {
                let row = f
                    .locals
                    .get(local.0)
                    .ok_or_else(|| bad("buffer result scalar", span))?;
                if row.kind != LocalKind::Temporary
                    || f.result != ValueTy::Scalar(row.ty)
                    || local.0 >= self.frames[frame].slots.len()
                {
                    return Err(bad("buffer result scalar type", span));
                }
                Ok(ResultPlace::Scalar(local))
            }
            CallResult::Owned(owner) => {
                self.expect_owner(frame, owner, &[UNINITIALIZED], span)?;
                let key = self.raw_key(frame, owner);
                if key.generation == 0 || f.owners[owner.0].kind != OwnerKind::Temporary {
                    return Err(bad("buffer result owner", span));
                }
                let bytes = self.owner_extent(key, span)?;
                let generation = epoch(key.generation, span)?;
                match f.result {
                    ValueTy::Owned(AggregateTy::ByteBuf) if bytes.len() == 16 => (),
                    ValueTy::Owned(AggregateTy::Enum(enumeration)) => {
                        let variants = self
                            .plan
                            .witness()
                            .declarations()
                            .enums()
                            .variants(enumeration)
                            .map_err(|_| bad("buffer result enum", span))?;
                        for variant in variants {
                            let offset = variant.payload_offset().unwrap_or(0);
                            let length = match variant.value_payload() {
                                None => 0,
                                Some(ValueTy::Scalar(hir::Ty::I32)) => 4,
                                Some(ValueTy::Owned(AggregateTy::File)) => 16,
                                _ => return Err(bad("status payload type", span)),
                            };
                            if bytes.len() < 4
                                || offset
                                    .checked_add(length)
                                    .is_none_or(|end| end > bytes.len())
                            {
                                return Err(bad("status payload extent", span));
                            }
                        }
                    }
                    _ => return Err(bad("buffer result type", span)),
                }
                Ok(ResultPlace::Owner {
                    owner,
                    bytes,
                    generation,
                })
            }
        }
    }
    fn commit_experimental_result(
        &mut self,
        frame: usize,
        place: ResultPlace,
        value: OperationValue,
    ) {
        match (place, value) {
            (ResultPlace::Scalar(local), OperationValue::Scalar(value)) => {
                self.frames[frame].slots[local.0] = Some(value.into())
            }
            (
                ResultPlace::Owner {
                    owner,
                    bytes,
                    generation,
                },
                value,
            ) => {
                let target = &mut self.frames[frame];
                match value {
                    OperationValue::EmptyBuffer => target.payload[bytes].fill(0),
                    OperationValue::Status { tag, payload } => {
                        target.payload[bytes.start..bytes.start + 4]
                            .copy_from_slice(&tag.to_le_bytes());
                        if let Some(value) = payload {
                            target.payload[bytes.start + 4..bytes.start + 8]
                                .copy_from_slice(&value.to_le_bytes());
                        }
                    }
                    OperationValue::Scalar(_) => unreachable!("checked result kind"),
                }
                target.owners[owner.0].generation = generation;
                target.owners[owner.0].state = AVAILABLE;
            }
            _ => unreachable!("checked result kind"),
        }
    }
    fn experimental_scalar(&self, frame: usize, local: usize, span: Span) -> Result<Scalar> {
        self.read(
            frame,
            Operand {
                local: LocalId(local),
                span,
            },
        )
        .and_then(|value| {
            value
                .narrow()
                .ok_or_else(|| bad("builtin scalar type", span))
        })
    }
    fn experimental_i32(&self, frame: usize, local: usize, span: Span) -> Result<i32> {
        match self.experimental_scalar(frame, local, span)? {
            Scalar::I32(value) => Ok(value),
            _ => Err(bad("buffer size type", span)),
        }
    }
    fn experimental_u8(&self, frame: usize, local: usize, span: Span) -> Result<u8> {
        match self.experimental_scalar(frame, local, span)? {
            Scalar::U8(value) => Ok(value),
            _ => Err(bad("buffer byte type", span)),
        }
    }
    pub(super) fn experimental(
        &mut self,
        frame: usize,
        operation: Function,
        destination: CallResult,
        span: Span,
    ) -> Result<()> {
        if self.plan.witness().experimental_function(operation) != Some(self.frames[frame].function)
        {
            return Err(bad("experimental function identity", span));
        }
        let result = self.prepare_experimental_result(frame, destination, span)?;
        let value = match operation {
            Function::New => OperationValue::EmptyBuffer,
            Function::ArgCount => {
                OperationValue::Scalar(Scalar::I32(crate::program_arguments::count()))
            }
            Function::Take => {
                let bytes = match &result {
                    ResultPlace::Owner { bytes, .. }
                        if self.function(self.frames[frame].function).result
                            == ValueTy::Owned(AggregateTy::ByteBuf)
                            && bytes.len() == 16 =>
                    {
                        bytes.clone()
                    }
                    _ => return Err(bad("buffer transfer result", span)),
                };
                let (place, value) = self.buffer_parameter(frame, Access::Write, span)?;
                let encoding = value.encoding();
                // All source/result admission precedes the transfer. The
                // existing allocation stays registered under its old identity.
                self.frames[place.frame].payload[place.bytes].fill(0);
                self.commit_experimental_result(frame, result, OperationValue::EmptyBuffer);
                self.frames[frame].payload[bytes].copy_from_slice(&encoding);
                return Ok(());
            }
            Function::ReadSome | Function::WriteSome | Function::ArgBytes => {
                return Err(bad("binary operation requires prepared delivery", span));
            }
            Function::Len | Function::Capacity => {
                let (_, value) = self.buffer_parameter(frame, Access::Read, span)?;
                let value = if operation == Function::Len {
                    self.heap.len(&value)
                } else {
                    self.heap.capacity(&value)
                }
                .map_err(|failure| heap_failure(failure, span))?;
                OperationValue::Scalar(Scalar::I32(value))
            }
            _ => {
                let (place, mut value) = self.buffer_parameter(frame, Access::Write, span)?;
                // A hosted primitive retains all existing checked cost and
                // failure rules, using a fresh local admission budget. Its
                // debit never becomes an accumulated program counter.
                let mut operation_fuel = self.fuel;
                let status = match operation {
                    Function::Reserve => {
                        let size = self.experimental_i32(frame, 0, span)?;
                        self.heap.try_reserve(&mut value, size, &mut operation_fuel)
                    }
                    Function::Push => {
                        let byte = self.experimental_u8(frame, 0, span)?;
                        self.heap.try_push(&mut value, byte, &mut operation_fuel)
                    }
                    Function::Resize => {
                        let size = self.experimental_i32(frame, 0, span)?;
                        let byte = self.experimental_u8(frame, 1, span)?;
                        self.heap
                            .try_resize(&mut value, size, byte, &mut operation_fuel)
                    }
                    Function::Extend => {
                        match self.byte_region(
                            frame,
                            AccessBase::Parameter(ReferenceParamId(1)),
                            Access::Read,
                            span,
                        )? {
                            ByteRegion::Activation { frame, bytes } => self.heap.try_extend(
                                &mut value,
                                &self.frames[frame].payload[bytes],
                                &mut operation_fuel,
                            ),
                            ByteRegion::Buffer {
                                value: source,
                                start,
                                end,
                            } => self.heap.try_extend_buffer(
                                &mut value,
                                &source,
                                start,
                                end,
                                &mut operation_fuel,
                            ),
                        }
                    }
                    Function::Truncate => {
                        let size = self.experimental_i32(frame, 0, span)?;
                        self.heap
                            .truncate(&mut value, size)
                            .map(|_| byte_buffer::GrowStatus::Ok)
                    }
                    _ => unreachable!("mutable buffer operation"),
                };
                if self.fuel != crate::frontend::build_config::HOSTED_FUEL {
                    self.fuel = operation_fuel;
                }
                let status = status.map_err(|failure| heap_failure(failure, span))?;
                self.frames[place.frame].payload[place.bytes].copy_from_slice(&value.encoding());
                if operation == Function::Truncate {
                    OperationValue::Scalar(Scalar::Unit)
                } else {
                    OperationValue::Status {
                        tag: status as u32,
                        payload: None,
                    }
                }
            }
        };
        self.commit_experimental_result(frame, result, value);
        Ok(())
    }
}

impl<V: StoredValue> ValueMachine<'_, '_, V> {
    pub(super) fn stage_byte_view(
        &self,
        frame: usize,
        declaration: &LoanDecl,
        root: OwnerKey,
        mut view: BorrowView,
        range: Option<(i32, i32)>,
        span: Span,
    ) -> Result<BorrowView> {
        let actual = view
            .aggregate
            .ok_or_else(|| bad("byte view identity", span))?
            .aggregate();
        if actual == AggregateTy::ByteBuf
            && declaration.referent() == BorrowedTy::ScalarSlice(hir::Ty::U8)
        {
            let (base_start, base_length) = if declaration.projection.is_empty()
                && matches!(declaration.authority, AccessBase::Parameter(reference) if self.function(self.frames[frame].function).references[reference.0].referent() == BorrowedTy::ScalarSlice(hir::Ty::U8))
            {
                ((view.offset >> 32) as i32, view.offset as u32 as i32)
            } else {
                let (_, value) = self.buffer_at(root, view.offset, span)?;
                (
                    0,
                    self.heap
                        .len(&value)
                        .map_err(|failure| heap_failure(failure, span))?,
                )
            };
            let (start, end) = range.unwrap_or((0, base_length));
            if start < 0 || start > end || end > base_length {
                return Err(OwnedRunFailure::Bounds(span));
            }
            let length = end - start;
            let start = base_start
                .checked_add(start)
                .ok_or(OwnedRunFailure::Bounds(span))?;
            view.offset = ((start as u64) << 32) | length as u64;
            return Ok(view);
        }
        if let Some((start, end)) = range {
            let AggregateTy::FixedArray(array) = actual else {
                return Err(bad("byte range type", span));
            };
            if array.element() != hir::Ty::U8
                || start < 0
                || start > end
                || end as usize > array.length()
            {
                return Err(OwnedRunFailure::Bounds(span));
            }
            view.offset = view
                .offset
                .checked_add(start as u64)
                .ok_or(OwnedRunFailure::Bounds(span))?;
            let array = FixedArrayTy::check(hir::Ty::U8, (end - start) as usize)
                .map_err(|_| bad("byte range extent", span))?;
            view.aggregate = Some(
                AggregateSlot::try_from_aggregate(AggregateTy::FixedArray(array))
                    .map_err(|_| bad("byte range descriptor", span))?,
            );
        }
        Ok(view)
    }
    pub(super) fn dynamic_byte_view(&self, frame: usize, base: AccessBase) -> bool {
        matches!(base, AccessBase::Parameter(reference) if self.frames[frame].references[reference.0].view.aggregate.map(AggregateSlot::aggregate) == Some(AggregateTy::ByteBuf))
    }
    pub(super) fn dynamic_index_read(
        &mut self,
        frame: usize,
        destination: LocalId,
        base: AccessBase,
        operand: Operand,
        span: Span,
    ) -> Result<()> {
        let root = self.base(frame, base, Access::Read, span)?;
        let ByteRegion::Buffer { value, start, end } =
            self.byte_region(frame, base, Access::Read, span)?
        else {
            return Err(bad("dynamic byte view", span));
        };
        let Some(Scalar::I32(index)) = self.read(frame, operand)?.narrow() else {
            return Err(bad("byte index type", span));
        };
        if index < 0 || index >= end - start {
            return Err(OwnedRunFailure::Bounds(span));
        }
        let byte = self
            .heap
            .range(&value, start, end)
            .map_err(|failure| heap_failure(failure, span))?[index as usize];
        self.write(frame, destination, Scalar::U8(byte), span)?;
        #[cfg(test)]
        self.record_event(Event::ReadIndex(
            root,
            index as usize,
            Scalar::U8(byte).into(),
        ));
        let _ = root;
        Ok(())
    }
    pub(super) fn dynamic_index_write(
        &mut self,
        frame: usize,
        base: AccessBase,
        operand: Operand,
        value: Operand,
        span: Span,
    ) -> Result<()> {
        let root = self.base(frame, base, Access::Write, span)?;
        let ByteRegion::Buffer {
            value: descriptor,
            start,
            end,
        } = self.byte_region(frame, base, Access::Write, span)?
        else {
            return Err(bad("dynamic byte view", span));
        };
        let Some(Scalar::U8(byte)) = self.read(frame, value)?.narrow() else {
            return Err(bad("byte value type", span));
        };
        let Some(Scalar::I32(index)) = self.read(frame, operand)?.narrow() else {
            return Err(bad("byte index type", span));
        };
        if index < 0 || index >= end - start {
            return Err(OwnedRunFailure::Bounds(span));
        }
        self.heap
            .range_mut(&descriptor, start, end)
            .map_err(|failure| heap_failure(failure, span))?[index as usize] = byte;
        #[cfg(test)]
        self.record_event(Event::WriteIndex(
            root,
            index as usize,
            Scalar::U8(byte).into(),
        ));
        let _ = root;
        Ok(())
    }
    pub(super) fn dynamic_length(
        &mut self,
        frame: usize,
        destination: LocalId,
        base: AccessBase,
        span: Span,
    ) -> Result<()> {
        let root = self.base(frame, base, Access::Read, span)?;
        let ByteRegion::Buffer { start, end, .. } =
            self.byte_region(frame, base, Access::Read, span)?
        else {
            return Err(bad("dynamic byte view", span));
        };
        self.write(frame, destination, Scalar::I32(end - start), span)?;
        #[cfg(test)]
        self.record_event(Event::ArrayLength(root, (end - start) as usize));
        let _ = root;
        Ok(())
    }
}
