//! Allocation-free finite owned-record vector delivery in the shared VM.
use super::experimental::{heap_failure, ResultPlace};
use super::vector::{Delivery, Returned, VectorPlace, VectorPrimitive};
use super::*;
use crate::frontend::builtin_catalog::owned_vector::{Function, Operation};
use crate::frontend::oir::owned::owned_record_codec::{
    bounds::{CheckedElementView, ShapeFailure},
    Element,
};
use std::ops::Range;

impl VectorPrimitive for Function {
    fn authenticates<W: PlanWitness>(self, witness: &W, function: hir::DefId) -> bool {
        witness.owned_vector_operation(function) == Some(self)
    }
    fn input_record(self) -> Option<RecordId> {
        Some(self.record_id())
    }
    fn input_access(self) -> Access {
        Access::Consume
    }
}

struct InputMove {
    owner: OwnerPlaceId,
    generation: u64,
    bytes: Range<usize>,
}
struct RecordPublication {
    local: OwnerPlaceId,
    local_generation: u64,
    caller: OwnerPlaceId,
    caller_generation: u64,
    caller_frame: usize,
    bytes: Range<usize>,
}

struct ControlCarriers<'a, V, W: PlanWitness> {
    witness: &'a W,
    view: Option<CheckedElementView<'a>>,
    element: Element<'a>,
    element_return: std::result::Result<Element<'a>, ShapeFailure>,
    mapped_element_return: Result<Element<'a>>,
    key: Function,
    key_return: Option<Function>,
    root: OwnerKey,
    root_return: Result<OwnerKey>,
    handle_return: Result<Option<ReferenceHandle>>,
    view_return: Result<BorrowView>,
    parameter_view_return: Result<(OwnerKey, BorrowView)>,
    types: [AggregateTy; 2],
    offset_returns: [Result<usize>; 3],
    checked_offsets: [Option<usize>; 2],
    place: VectorPlace,
    place_return: Result<(VectorPlace, byte_buffer::OwnedVectorDescriptor)>,
    descriptor: byte_buffer::OwnedVectorDescriptor,
    descriptor_return:
        std::result::Result<byte_buffer::OwnedVectorDescriptor, byte_buffer::Failure>,
    result_descriptor: byte_buffer::OwnedVectorDescriptor,
    encoding: [u8; 16],
    input: Option<InputMove>,
    input_return: Result<Option<InputMove>>,
    publication: Option<RecordPublication>,
    publication_return: Result<Option<RecordPublication>>,
    count: i32,
    count_return: Result<i32>,
    query_result: i32,
    operation_status: std::result::Result<byte_buffer::GrowStatus, byte_buffer::Failure>,
    operation_return: std::result::Result<(), byte_buffer::Failure>,
    returned: Returned,
    scalar: V,
    scalar_return: Result<V>,
    fuel_argument: &'a mut usize,
    source_argument: &'a mut [u8],
    output_argument: &'a mut [u8],
    hosted: bool,
    metadata: [usize; 4],
}
pub(super) const fn control_bytes<V: StoredValue, W: PlanWitness>() -> usize {
    size_of::<ControlCarriers<'_, V, W>>()
        + vector::delivery_control_bytes::<V, Function>()
        + byte_buffer::owned_vector_control_bytes()
}

impl<V: StoredValue, W: PlanWitness> ValueMachine<'_, '_, V, W> {
    /// The general primitive result helper can admit these two new classes
    /// only for the independently authenticated exact new-domain function.
    pub(super) fn owned_vector_result_extent(
        &self,
        function: hir::DefId,
        aggregate: AggregateTy,
    ) -> Option<usize> {
        let key = self.plan.witness().owned_vector_operation(function)?;
        let element = self
            .plan
            .witness()
            .elements()?
            .element(key.record_id())
            .ok()?;
        match key.operation() {
            Operation::New | Operation::Take
                if aggregate == AggregateTy::OwnedRecordVector(key.record_id()) =>
            {
                Some(16)
            }
            Operation::PopLast if aggregate == AggregateTy::Record(key.record_id()) => {
                Some(element.size())
            }
            _ => None,
        }
    }

    fn owned_vector_parameter(
        &self,
        frame: usize,
        key: Function,
        access: Access,
        span: Span,
    ) -> Result<(VectorPlace, byte_buffer::OwnedVectorDescriptor)> {
        let (root, view) = self.vector_parameter_view(
            frame,
            AggregateTy::OwnedRecordVector(key.record_id()),
            access,
            span,
        )?;
        let extent = self.owner_extent(root, span)?;
        let start = extent
            .start
            .checked_add(index(view.offset, span)?)
            .ok_or_else(|| bad("owned vector view offset", span))?;
        let end = start
            .checked_add(16)
            .filter(|end| *end <= extent.end)
            .ok_or_else(|| bad("owned vector view extent", span))?;
        let place = VectorPlace {
            frame: index(root.frame, span)?,
            bytes: start..end,
        };
        let value = byte_buffer::OwnedVectorDescriptor::decode(
            &self.frames[place.frame].payload[place.bytes.clone()],
        )
        .map_err(|failure| heap_failure(failure, span))?;
        Ok((place, value))
    }

    fn owned_vector_input(
        &self,
        frame: usize,
        key: Function,
        element: Element<'_>,
        span: Span,
    ) -> Result<Option<InputMove>> {
        if key.operation() != Operation::PushReserved {
            return Ok(None);
        }
        let function = self.function(self.frames[frame].function);
        let owner = function
            .parameters
            .iter()
            .find_map(|binding| match binding {
                ParameterBinding::Owned(owner) => Some(*owner),
                ParameterBinding::Scalar(_) | ParameterBinding::Reference(_) => None,
            })
            .ok_or_else(|| bad("owned vector input parameter", span))?;
        self.expect_owner(frame, owner, &[AVAILABLE], span)?;
        let root = self.base(frame, AccessBase::Owner(owner), Access::Consume, span)?;
        if self.aggregate(root, span)? != AggregateTy::Record(key.record_id()) {
            return Err(bad("owned vector input identity", span));
        }
        let bytes = self.owner_extent(root, span)?;
        if bytes.len() != element.size() {
            return Err(bad("owned vector input extent", span));
        }
        Ok(Some(InputMove {
            owner,
            generation: epoch(root.generation, span)?,
            bytes,
        }))
    }

    fn owned_vector_record_publication(
        &self,
        key: Function,
        delivery: &Delivery,
        element: Element<'_>,
        span: Span,
    ) -> Result<Option<RecordPublication>> {
        if key.operation() != Operation::PopLast {
            return Ok(None);
        }
        match (&delivery.local, &delivery.caller_result) {
            (
                ResultPlace::Owner {
                    owner: local,
                    generation: local_generation,
                    bytes: local_bytes,
                },
                ResultPlace::Owner {
                    owner: caller,
                    generation: caller_generation,
                    bytes,
                },
            ) if local_bytes.len() == element.size() && bytes.len() == element.size() => {
                Ok(Some(RecordPublication {
                    local: *local,
                    local_generation: *local_generation,
                    caller: *caller,
                    caller_generation: *caller_generation,
                    caller_frame: delivery.caller,
                    bytes: bytes.clone(),
                }))
            }
            _ => Err(bad("owned vector record delivery", span)),
        }
    }

    fn put_owned_vector(&mut self, place: VectorPlace, value: &byte_buffer::OwnedVectorDescriptor) {
        self.frames[place.frame].payload[place.bytes].copy_from_slice(&value.encoding());
    }

    pub(super) fn owned_vector_delivery(
        &mut self,
        frame: usize,
        key: Function,
        destination: CallResult,
        span: Span,
    ) -> Result<()> {
        let delivery = self.prepare_vector_delivery(frame, key, destination, span)?;
        let function = self.frames[frame].function;
        let cost = self
            .plan
            .owned_vector_delivery_cost(function, key)
            .ok_or_else(|| bad("owned vector delivery identity", span))?;
        let element = self
            .plan
            .witness()
            .elements()
            .ok_or_else(|| bad("owned vector finite bounds", span))?
            .element(key.record_id())
            .map_err(|_| bad("owned vector finite element", span))?;
        let input = self.owned_vector_input(frame, key, element, span)?;
        let publication = self.owned_vector_record_publication(key, &delivery, element, span)?;
        // Compiler-owned frame/fuel roles are disjoint. This prepays the exact
        // canonical statement plus immediate return before any heap mutation.
        self.charge(cost, span)?;
        let hosted = self.fuel == HOSTED_FUEL;
        let returned = match key.operation() {
            Operation::New => {
                Returned::Descriptor(byte_buffer::OwnedVectorDescriptor::empty().encoding())
            }
            Operation::Len | Operation::Capacity => {
                let (_, value) = self.owned_vector_parameter(frame, key, Access::Read, span)?;
                let mut result = 0i32;
                if key.operation() == Operation::Len {
                    self.heap
                        .owned_vector_len_into(&value, element, &mut self.fuel, &mut result)
                } else {
                    self.heap.owned_vector_capacity_into(
                        &value,
                        element,
                        &mut self.fuel,
                        &mut result,
                    )
                }
                .map_err(|failure| heap_failure(failure, span))?;
                Returned::Scalar(Scalar::I32(result))
            }
            Operation::TryReserve => {
                let (place, mut value) =
                    self.owned_vector_parameter(frame, key, Access::Write, span)?;
                let count = self.vector_i32(frame, 0, span)?;
                let status = self
                    .heap
                    .owned_vector_try_reserve(&mut value, element, count, &mut self.fuel)
                    .map_err(|failure| heap_failure(failure, span))?;
                self.put_owned_vector(place, &value);
                Returned::Status(status as u32)
            }
            Operation::PushReserved => {
                let (place, mut value) =
                    self.owned_vector_parameter(frame, key, Access::Write, span)?;
                let input = input.ok_or_else(|| bad("owned vector prepared input", span))?;
                self.heap
                    .owned_vector_push_reserved(
                        &mut value,
                        element,
                        &mut self.frames[frame].payload[input.bytes],
                        &mut self.fuel,
                    )
                    .map_err(|failure| heap_failure(failure, span))?;
                self.put_owned_vector(place, &value);
                let row = &mut self.frames[frame].owners[input.owner.0];
                row.generation = input.generation;
                row.state = MOVED;
                Returned::Scalar(Scalar::Unit)
            }
            Operation::PopLast => {
                let (place, mut value) =
                    self.owned_vector_parameter(frame, key, Access::Write, span)?;
                let publication =
                    publication.ok_or_else(|| bad("owned vector prepared output", span))?;
                // The core writes the preadmitted real caller result directly,
                // matching native %result. No record-sized scratch/copy exists.
                self.heap
                    .owned_vector_pop_last(
                        &mut value,
                        element,
                        &mut self.frames[publication.caller_frame].payload[publication.bytes],
                        &mut self.fuel,
                    )
                    .map_err(|failure| heap_failure(failure, span))?;
                self.put_owned_vector(place, &value);
                let local = &mut self.frames[frame].owners[publication.local.0];
                local.generation = publication.local_generation;
                local.state = AVAILABLE;
                let caller =
                    &mut self.frames[publication.caller_frame].owners[publication.caller.0];
                caller.generation = publication.caller_generation;
                caller.state = AVAILABLE;
                if hosted {
                    self.fuel = HOSTED_FUEL;
                }
                self.complete_vector_return(frame, delivery);
                return Ok(());
            }
            Operation::Truncate => {
                let (place, mut value) =
                    self.owned_vector_parameter(frame, key, Access::Write, span)?;
                let count = self.vector_i32(frame, 0, span)?;
                self.heap
                    .owned_vector_truncate(&mut value, element, count, &mut self.fuel)
                    .map_err(|failure| heap_failure(failure, span))?;
                self.put_owned_vector(place, &value);
                Returned::Scalar(Scalar::Unit)
            }
            Operation::Take => {
                let (place, mut value) =
                    self.owned_vector_parameter(frame, key, Access::Write, span)?;
                let mut result = byte_buffer::OwnedVectorDescriptor::empty();
                self.heap
                    .owned_vector_take_into(&mut value, element, &mut self.fuel, &mut result)
                    .map_err(|failure| heap_failure(failure, span))?;
                self.put_owned_vector(place, &value);
                Returned::Descriptor(result.encoding())
            }
        };
        if hosted {
            self.fuel = HOSTED_FUEL;
        }
        self.complete_vector_delivery(frame, delivery, returned);
        Ok(())
    }
}
