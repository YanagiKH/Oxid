//! File primitives prepare their complete return and loan releases before I/O.
use super::*;

#[derive(Clone, Copy)]
struct Release {
    loan: LoanKey,
    frame: usize,
    index: usize,
    owner: bool,
    kind: BorrowKind,
}
struct Delivery {
    local: ResultPlace,
    caller_result: ResultPlace,
    from: OwnerKey,
    to: OwnerKey,
    moved_generation: u64,
    caller: usize,
    resume: Resume,
    releases: [Option<Release>; 2],
    cost: usize,
}
struct Value {
    tag: u32,
    payload: [u8; 16],
    offset: usize,
    length: usize,
}
impl Value {
    fn status(tag: u32, payload: Option<i32>, offset: usize) -> Self {
        let mut bytes = [0; 16];
        if let Some(value) = payload {
            bytes[..4].copy_from_slice(&value.to_le_bytes());
        }
        Self {
            tag,
            payload: bytes,
            offset,
            length: if payload.is_some() { 4 } else { 0 },
        }
    }
}
#[allow(dead_code)]
struct ControlCarriers {
    delivery: Delivery,
    preparation: Result<Delivery>,
    value: Value,
    open_result: Result<byte_buffer::OpenValue>,
    raw_open_result: std::result::Result<byte_buffer::OpenValue, byte_buffer::Failure>,
    file_descriptor: byte_buffer::FileDescriptor,
    file_parameter_return: Result<(BufferPlace, byte_buffer::FileDescriptor)>,
    file_access: byte_buffer::FileAccess,
    file_access_return: std::result::Result<byte_buffer::FileAccess, byte_buffer::Failure>,
    mapped_file_access_return: Result<byte_buffer::FileAccess>,
    byte_attempt_return: std::result::Result<std::result::Result<usize, i32>, byte_buffer::Failure>,
    mapped_byte_attempt_return: Result<std::result::Result<usize, i32>>,
    file_status_return: std::result::Result<std::result::Result<(), i32>, byte_buffer::Failure>,
    mapped_file_status_return: Result<std::result::Result<(), i32>>,
    selected_open_value: byte_buffer::OpenValue,
    selected_byte_attempt: std::result::Result<usize, i32>,
    selected_file_status: std::result::Result<(), i32>,
    byte_region: Result<ByteRegion>,
    resource_walk: Result<u64>,
    path_copy: [u8; 4096],
    terminated_path: [u8; 4096],
}
pub(in super::super) fn control_bytes() -> usize {
    size_of::<ControlCarriers>() + byte_buffer::file_helper_control_bytes()
}

impl<V: StoredValue> ValueMachine<'_, '_, V> {
    fn prepare_file_delivery(
        &self,
        frame: usize,
        operation: Function,
        destination: CallResult,
        span: Span,
    ) -> Result<Delivery> {
        let id = self.frames[frame].function;
        if self.plan.witness().experimental_function(operation) != Some(id) || !operation.is_file()
        {
            return Err(bad("file primitive identity", span));
        }
        let local = self.prepare_experimental_result(frame, destination, span)?;
        let ResultPlace::Owner {
            owner, generation, ..
        } = local
        else {
            return Err(bad("file result owner", span));
        };
        let from = OwnerKey {
            generation,
            ..self.raw_key(frame, owner)
        };
        let moved_generation = epoch(generation, span)?;
        let resume = self.frames[frame]
            .return_to
            .ok_or_else(|| bad("file caller", span))?;
        let caller = frame
            .checked_sub(1)
            .ok_or_else(|| bad("file caller", span))?;
        let parent = self.function(self.frames[caller].function);
        let call = parent
            .calls
            .get(resume.call.0)
            .ok_or_else(|| bad("file call", span))?;
        if call.target != id
            || self.frames[caller].calls[resume.call.0].phase != IN_FLIGHT
            || resume.continuation.0 >= parent.blocks.len()
        {
            return Err(bad("file call state", span));
        }
        let CallResult::Owned(target) = call.result else {
            return Err(bad("file caller result", span));
        };
        self.expect_owner(caller, target, &[DEAD], span)?;
        let mut to = self.raw_key(caller, target);
        let bytes = self.owner_extent(to, span)?;
        to.generation = epoch(to.generation, span)?;
        if self.function(id).result != ValueTy::Owned(parent.owners[target.0].aggregate()) {
            return Err(bad("file result identity", span));
        }
        let ResultPlace::Owner {
            bytes: local_bytes, ..
        } = &local
        else {
            unreachable!()
        };
        if bytes.len() != local_bytes.len() {
            return Err(bad("file caller result extent", span));
        }
        let loans = self.plan.function(parent.id).borrowed_loans(resume.call);
        if loans.is_empty() || loans.len() > 2 {
            return Err(bad("file loan inventory", span));
        }
        let mut releases = [None; 2];
        for (position, loan) in loans.iter().enumerate() {
            let loan = self.handle(caller, *loan, span)?.permission;
            self.release_preflight(loan, span)?;
            let row = self.loan(loan, span)?;
            let (release_frame, release_index, owner) = if row.parent.instance == 0 {
                (
                    index(row.root.frame, span)?,
                    index(row.root.owner, span)?,
                    true,
                )
            } else {
                (
                    index(row.parent.frame, span)?,
                    index(row.parent.loan, span)?,
                    false,
                )
            };
            releases[position] = Some(Release {
                loan,
                frame: release_frame,
                index: release_index,
                owner,
                kind: self.loan_kind(loan, span)?,
            });
        }
        Ok(Delivery {
            local,
            caller_result: ResultPlace::Owner {
                owner: target,
                bytes,
                generation: to.generation,
            },
            from,
            to,
            moved_generation,
            caller,
            resume,
            releases,
            cost: self
                .plan
                .file_delivery_cost(id)
                .ok_or_else(|| bad("file return cost", span))?,
        })
    }
    fn file_parameter(
        &self,
        frame: usize,
        span: Span,
    ) -> Result<(BufferPlace, byte_buffer::FileDescriptor)> {
        let reference = ReferenceParamId(0);
        if self
            .function(self.frames[frame].function)
            .references
            .first()
            .is_none_or(|row| {
                row.referent() != BorrowedTy::Exact(AggregateTy::File)
                    || row.kind != BorrowKind::Exclusive
            })
        {
            return Err(bad("file parameter", span));
        }
        let root = self.base(frame, AccessBase::Parameter(reference), Access::Write, span)?;
        let view = self.base_view(frame, AccessBase::Parameter(reference), root, span)?;
        let owner = self.owner_extent(root, span)?;
        let start = owner
            .start
            .checked_add(index(view.offset, span)?)
            .ok_or_else(|| bad("file offset", span))?;
        let end = start
            .checked_add(16)
            .filter(|end| *end <= owner.end)
            .ok_or_else(|| bad("file extent", span))?;
        let frame = index(root.frame, span)?;
        let bytes = start..end;
        let value = self
            .heap
            .file_descriptor(&self.frames[frame].payload[bytes.clone()])
            .map_err(|failure| heap_failure(failure, span))?;
        Ok((BufferPlace { frame, bytes }, value))
    }
    fn commit_file_result(&mut self, frame: usize, place: ResultPlace, value: &Value) {
        let ResultPlace::Owner {
            owner,
            bytes,
            generation,
        } = place
        else {
            unreachable!("prepared file owner")
        };
        let target = &mut self.frames[frame];
        target.payload[bytes.start..bytes.start + 4].copy_from_slice(&value.tag.to_le_bytes());
        let start = bytes.start + value.offset;
        target.payload[start..start + value.length].copy_from_slice(&value.payload[..value.length]);
        target.owners[owner.0].generation = generation;
        target.owners[owner.0].state = AVAILABLE;
    }
    pub(in super::super) fn file_delivery(
        &mut self,
        frame: usize,
        operation: Function,
        destination: CallResult,
        span: Span,
    ) -> Result<()> {
        let delivery = self.prepare_file_delivery(frame, operation, destination, span)?;
        // Prepay the return before even a successful open can change a file.
        let mut fuel = self.fuel;
        if fuel != crate::frontend::build_config::HOSTED_FUEL {
            fuel = fuel
                .checked_sub(delivery.cost)
                .ok_or(RunFailure::Fuel(span))?;
        }
        let value = match operation {
            Function::OpenRead | Function::Create => {
                let region = self.byte_region(
                    frame,
                    AccessBase::Parameter(ReferenceParamId(0)),
                    Access::Read,
                    span,
                )?;
                // The extra source pass is prepaid before copying a heap path.
                // Activation paths pay the same public work model.
                let length = match &region {
                    ByteRegion::Activation { bytes, .. } => bytes.len(),
                    ByteRegion::Buffer { start, end, .. } => (*end - *start) as usize,
                };
                if length < 4096 {
                    fuel = fuel.checked_sub(length).ok_or(RunFailure::Fuel(span))?;
                }
                let mut copied = [0u8; 4096];
                let result = match region {
                    ByteRegion::Activation { frame, bytes } => self.heap.file_open(
                        &self.frames[frame].payload[bytes],
                        operation == Function::Create,
                        &mut fuel,
                    ),
                    ByteRegion::Buffer { value, start, end } => {
                        let bytes = self
                            .heap
                            .range(&value, start, end)
                            .map_err(|failure| heap_failure(failure, span))?;
                        if bytes.len() >= copied.len() {
                            Ok(byte_buffer::OpenValue::Error(36))
                        } else {
                            let length = bytes.len();
                            copied[..length].copy_from_slice(bytes);
                            self.heap.file_open(
                                &copied[..length],
                                operation == Function::Create,
                                &mut fuel,
                            )
                        }
                    }
                }
                .map_err(|failure| heap_failure(failure, span))?;
                match result {
                    byte_buffer::OpenValue::Opened(file) => Value {
                        tag: 0,
                        payload: file.encoding(),
                        offset: 8,
                        length: 16,
                    },
                    byte_buffer::OpenValue::Error(code) => Value::status(1, Some(code), 8),
                    byte_buffer::OpenValue::MemoryLimit => Value::status(2, None, 0),
                    byte_buffer::OpenValue::AllocationFailed => Value::status(3, None, 0),
                }
            }
            Function::FileReadSome | Function::FileWriteSome => {
                let (_, file) = self.file_parameter(frame, span)?;
                let access = self
                    .heap
                    .file_access(&file)
                    .map_err(|failure| heap_failure(failure, span))?;
                let reading = operation == Function::FileReadSome;
                let region = self.byte_region(
                    frame,
                    AccessBase::Parameter(ReferenceParamId(1)),
                    if reading { Access::Write } else { Access::Read },
                    span,
                )?;
                let bytes = match region {
                    ByteRegion::Activation { frame, bytes } => {
                        &mut self.frames[frame].payload[bytes]
                    }
                    ByteRegion::Buffer { value, start, end } => self
                        .heap
                        .range_mut(&value, start, end)
                        .map_err(|failure| heap_failure(failure, span))?,
                };
                let empty = bytes.is_empty();
                let result = if reading {
                    access.read(bytes, &mut fuel)
                } else {
                    access.write(bytes, &mut fuel)
                }
                .map_err(|failure| heap_failure(failure, span))?;
                match result {
                    Ok(0) if reading && !empty => Value::status(1, None, 0),
                    Ok(count) => Value::status(0, Some(count as i32), 4),
                    Err(code) => Value::status(if reading { 2 } else { 1 }, Some(code), 4),
                }
            }
            Function::SyncAll | Function::Close => {
                let (place, mut file) = self.file_parameter(frame, span)?;
                let result = if operation == Function::Close {
                    let result = self
                        .heap
                        .file_close(&mut file)
                        .map_err(|failure| heap_failure(failure, span))?;
                    self.frames[place.frame].payload[place.bytes].copy_from_slice(&file.encoding());
                    result
                } else {
                    self.heap
                        .file_sync(&file)
                        .map_err(|failure| heap_failure(failure, span))?
                };
                match result {
                    Ok(()) => Value::status(0, None, 0),
                    Err(code) => Value::status(1, Some(code), 4),
                }
            }
            _ => return Err(bad("file operation", span)),
        };
        if self.fuel != crate::frontend::build_config::HOSTED_FUEL {
            self.fuel = fuel;
        }
        let id = self.frames[frame].function;
        self.commit_file_result(frame, delivery.local, &value);
        self.commit_file_result(delivery.caller, delivery.caller_result, &value);
        self.frames[frame].owners[delivery.from.owner as usize].generation =
            delivery.moved_generation;
        self.frames[frame].owners[delivery.from.owner as usize].state = MOVED;
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
            self.record_event(Event::Charge(span, delivery.cost));
            self.record_event(Event::Transfer(delivery.from, delivery.to));
            self.record_event(Event::Return(id));
        }
        let _ = delivery.to;
        let usage = self.plan.function(id).usage();
        self.frames.pop();
        self.live_slots -= usage.scalar_slots;
        self.live_cells -= usage.expanded_cells;
        self.live_bytes -= usage.reference_bytes;
        Ok(())
    }
}
