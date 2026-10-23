//! The closed binary primitive completes its direct return in one prepared
//! operation. No source wrapper, persistent frame field or raw-OIR flag can
//! select this path. No allocation or fallible check follows the transport.
use super::*;

struct ReleaseTarget {
    frame: usize,
    index: usize,
    owner: bool,
    kind: BorrowKind,
}
struct Delivery {
    local: ResultPlace,
    caller_result: ResultPlace,
    from: OwnerKey,
    _to: OwnerKey,
    moved_generation: u64,
    caller: usize,
    resume: Resume,
    loan: LoanKey,
    release: ReleaseTarget,
    cost: usize,
}

// Supplemental fixed control roles, admitted only when the sealed binary
// catalog is present. No new Frame/Machine fields or logical fuel cells.
#[allow(dead_code)]
struct ControlCarriers {
    prepared: Delivery,
    preparation: Result<Delivery>,
    region: Result<ByteRegion>,
    value: OperationValue,
    release_loan: LoanRuntime,
}
pub(in super::super) fn control_bytes() -> usize {
    size_of::<ControlCarriers>()
}

impl Machine<'_, '_> {
    fn prepare_binary_delivery(
        &self,
        frame: usize,
        operation: Function,
        destination: CallResult,
        span: Span,
    ) -> Result<Delivery> {
        let id = self.frames[frame].function;
        if self.plan.witness().experimental_function(operation) != Some(id) {
            return Err(bad("binary delivery identity", span));
        }
        let cost = self
            .plan
            .binary_delivery_cost(id)
            .ok_or_else(|| bad("binary delivery function", span))?;
        let local = self.prepare_experimental_result(frame, destination, span)?;
        let ResultPlace::Owner {
            owner, generation, ..
        } = local
        else {
            return Err(bad("binary delivery result", span));
        };
        let from = OwnerKey {
            generation,
            ..self.raw_key(frame, owner)
        };
        let moved_generation = epoch(generation, span)?;
        let resume = self.frames[frame]
            .return_to
            .ok_or_else(|| bad("binary delivery caller", span))?;
        let caller = frame
            .checked_sub(1)
            .ok_or_else(|| bad("binary delivery caller", span))?;
        let parent = self.function(self.frames[caller].function);
        let call = parent
            .calls
            .get(resume.call.0)
            .ok_or_else(|| bad("binary delivery call", span))?;
        if call.target != id
            || self.frames[caller].calls[resume.call.0].phase != IN_FLIGHT
            || resume.continuation.0 >= parent.blocks.len()
        {
            return Err(bad("binary delivery call state", span));
        }
        let CallResult::Owned(target) = call.result else {
            return Err(bad("binary delivery caller result", span));
        };
        self.expect_owner(caller, target, &[DEAD], span)?;
        let mut to = self.raw_key(caller, target);
        let bytes = self.owner_extent(to, span)?;
        to.generation = epoch(to.generation, span)?;
        let aggregate = parent.owners[target.0].aggregate();
        if self.function(id).result != ValueTy::Owned(aggregate) {
            return Err(bad("binary delivery nominal result", span));
        }
        let AggregateTy::Enum(enumeration) = aggregate else {
            return Err(bad("binary delivery status", span));
        };
        // Prepare every possible closed status before the attempt determines
        // which one it returns. Padding/inactive payload bytes are not copied.
        for variant in self
            .plan
            .witness()
            .declarations()
            .enums()
            .variants(enumeration)
            .map_err(|_| bad("binary delivery layout", span))?
        {
            enum_offsets(
                bytes.clone(),
                EnumValue {
                    tag: variant.tag(),
                    payload: variant.payload().map(|_| Scalar::I32(0)),
                    payload_offset: variant.payload_offset().unwrap_or(0) as u8,
                },
                span,
            )?;
        }
        let [loan] = self.plan.function(parent.id).borrowed_loans(resume.call) else {
            return Err(bad("binary delivery loan inventory", span));
        };
        let loan = self.handle(caller, *loan, span)?.permission;
        self.release_preflight(loan, span)?;
        let row = self.loan(loan, span)?;
        let kind = self.loan_kind(loan, span)?;
        let release = if row.parent.instance == 0 {
            ReleaseTarget {
                frame: index(row.root.frame, span)?,
                index: index(row.root.owner, span)?,
                owner: true,
                kind,
            }
        } else {
            ReleaseTarget {
                frame: index(row.parent.frame, span)?,
                index: index(row.parent.loan, span)?,
                owner: false,
                kind,
            }
        };
        Ok(Delivery {
            local,
            caller_result: ResultPlace::Owner {
                owner: target,
                bytes,
                generation: to.generation,
            },
            from,
            _to: to,
            moved_generation,
            caller,
            resume,
            loan,
            release,
            cost,
        })
    }

    pub(in super::super) fn binary_delivery(
        &mut self,
        frame: usize,
        operation: Function,
        destination: CallResult,
        span: Span,
    ) -> Result<()> {
        #[cfg(test)]
        self.inject_binary_delivery(frame, span)?;
        let delivery = self.prepare_binary_delivery(frame, operation, destination, span)?;
        let region = self.byte_region(
            frame,
            AccessBase::Parameter(ReferenceParamId(0)),
            if operation == Function::ReadSome {
                Access::Write
            } else {
                Access::Read
            },
            span,
        )?;
        // Resolve the final slice and all resource validation before admission.
        // No user instruction or nested call can run between here and delivery.
        let bytes = match region {
            ByteRegion::Activation { frame, bytes } => &mut self.frames[frame].payload[bytes],
            ByteRegion::Buffer { value, start, end } => self
                .heap
                .range_mut(&value, start, end)
                .map_err(|failure| heap_failure(failure, span))?,
        };
        binary_io::admit_delivery(bytes.len(), delivery.cost, &mut self.fuel)
            .map_err(|failure| io_failure(failure, span))?;
        let value = if operation == Function::ReadSome {
            match binary_io::read_admitted(bytes) {
                binary_io::ReadStatus::Data(value) => OperationValue::Status {
                    tag: 0,
                    payload: Some(value),
                },
                binary_io::ReadStatus::Eof => OperationValue::Status {
                    tag: 1,
                    payload: None,
                },
                binary_io::ReadStatus::Error(value) => OperationValue::Status {
                    tag: 2,
                    payload: Some(value),
                },
            }
        } else {
            match binary_io::write_admitted(bytes) {
                binary_io::WriteStatus::Written(value) => OperationValue::Status {
                    tag: 0,
                    payload: Some(value),
                },
                binary_io::WriteStatus::Error(value) => OperationValue::Status {
                    tag: 1,
                    payload: Some(value),
                },
            }
        };
        self.complete_binary_delivery(frame, delivery, value, span);
        Ok(())
    }

    fn complete_binary_delivery(
        &mut self,
        frame: usize,
        delivery: Delivery,
        value: OperationValue,
        _span: Span,
    ) {
        let id = self.frames[frame].function;
        self.commit_experimental_result(frame, delivery.local, value);
        self.commit_experimental_result(delivery.caller, delivery.caller_result, value);
        self.frames[frame].owners[delivery.from.owner as usize].generation =
            delivery.moved_generation;
        self.frames[frame].owners[delivery.from.owner as usize].state = MOVED;
        let release = delivery.release;
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
        self.frames[delivery.loan.frame as usize].loans[delivery.loan.loan as usize].state = 0;
        self.frames[delivery.caller].calls[delivery.resume.call.0] = CallRuntime::default();
        self.branch(delivery.caller, delivery.resume.continuation);
        #[cfg(test)]
        {
            // Process observation is silent; these calls allocate no storage.
            self.record_event(Event::Charge(_span, delivery.cost));
            self.record_event(Event::Transfer(delivery.from, delivery._to));
            self.record_event(Event::Release(delivery.loan));
            self.record_event(Event::Return(id));
            binary_io::testing::delivered();
        }
        // The sealed primitive has one scalar-only status owner and no owned
        // parameters. Its result was transferred; there is nothing to release.
        let usage = self.plan.function(id).usage();
        self.frames.pop();
        self.live_slots -= usage.scalar_slots;
        self.live_cells -= usage.expanded_cells;
        self.live_bytes -= usage.reference_bytes;
    }

    #[cfg(test)]
    fn inject_binary_delivery(&mut self, frame: usize, _span: Span) -> Result<()> {
        use binary_io::testing::DeliveryFault;
        let Some(fault) = binary_io::testing::delivery_fault() else {
            return Ok(());
        };
        let caller = frame - 1;
        let call = self.frames[frame].return_to.unwrap().call;
        let parent = self.function(self.frames[caller].function);
        let CallResult::Owned(owner) = parent.calls[call.0].result else {
            panic!("closed binary result")
        };
        match fault {
            DeliveryFault::LocalGeneration => self.frames[frame].owners[0].generation = u64::MAX,
            DeliveryFault::DestinationState => {
                self.frames[caller].owners[owner.0].state = AVAILABLE
            }
            DeliveryFault::DestinationGeneration => {
                self.frames[caller].owners[owner.0].generation = u64::MAX;
            }
            DeliveryFault::LoanRelease => {
                let loan = self.plan.function(parent.id).borrowed_loans(call)[0];
                self.frames[caller].loans[loan.0].shared_children = 1;
            }
            DeliveryFault::Admission => unreachable!("injected by binary admission"),
        }
        Ok(())
    }
}
