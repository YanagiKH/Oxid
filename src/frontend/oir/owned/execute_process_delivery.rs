//! Scalar-only process effects prepay their complete immediate return.
use super::*;

struct Delivery {
    local: ResultPlace,
    caller_result: ResultPlace,
    caller: usize,
    resume: Resume,
    cost: usize,
}
#[allow(dead_code)]
struct Control {
    prepared: Delivery,
    preparation: Result<Delivery>,
    resume_return: Option<Resume>,
    selected_call: &'static CallDecl,
    selected_local: &'static LocalDecl,
    scalar: OperationValue,
    signal_result: bool,
    cost: usize,
    caller: usize,
    operation: Function,
}
pub(in super::super) const fn control_bytes() -> usize {
    size_of::<Control>()
}

impl<V: StoredValue, W: PlanWitness> ValueMachine<'_, '_, V, W> {
    fn prepare_process_delivery(
        &self,
        frame: usize,
        operation: Function,
        destination: CallResult,
        span: Span,
    ) -> Result<Delivery> {
        let f = self.function(self.frames[frame].function);
        if operation.process().is_none()
            || self.plan.witness().experimental_function(operation) != Some(f.id)
            || !f.parameters.is_empty()
            || !f.owners.is_empty()
            || !f.references.is_empty()
            || !f.calls.is_empty()
            || !f.loans.is_empty()
        {
            return Err(bad("process delivery identity", span));
        }
        let local = self.prepare_experimental_result(frame, destination, span)?;
        if !matches!(local, ResultPlace::Scalar(_)) {
            return Err(bad("process scalar result", span));
        }
        let resume = self.frames[frame]
            .return_to
            .ok_or_else(|| bad("process missing caller", span))?;
        let caller = frame
            .checked_sub(1)
            .ok_or_else(|| bad("process caller frame", span))?;
        let parent = self.function(self.frames[caller].function);
        let call = parent
            .calls
            .get(resume.call.0)
            .ok_or_else(|| bad("process caller identity", span))?;
        let CallResult::Scalar(target) = call.result else {
            return Err(bad("process caller scalar", span));
        };
        let declaration = parent
            .locals
            .get(target.0)
            .ok_or_else(|| bad("process caller result", span))?;
        if declaration.ty != hir::Ty::Bool
            || declaration.kind == LocalKind::Parameter
            || self.frames[caller].slots.get(target.0).is_none()
            || self.frames[caller]
                .calls
                .get(resume.call.0)
                .is_none_or(|call| call.phase != IN_FLIGHT)
            || parent.blocks.get(resume.continuation.0).is_none()
            || !self
                .plan
                .function(parent.id)
                .borrowed_loans(resume.call)
                .is_empty()
            || !self
                .plan
                .function(parent.id)
                .owned_stages(resume.call)
                .is_empty()
        {
            return Err(bad("process prepared return", span));
        }
        let cost = self
            .plan
            .process_delivery_cost(f.id)
            .ok_or_else(|| bad("process return cost", span))?;
        Ok(Delivery {
            local,
            caller_result: ResultPlace::Scalar(target),
            caller,
            resume,
            cost,
        })
    }

    pub(in super::super) fn process_delivery(
        &mut self,
        frame: usize,
        operation: Function,
        destination: CallResult,
        span: Span,
    ) -> Result<()> {
        let delivery = self.prepare_process_delivery(frame, operation, destination, span)?;
        self.charge(delivery.cost, span)?;
        let id = self.frames[frame].function;
        let usage = self.plan.function(id).usage();
        let value = match operation {
            Function::EnableTermination => crate::program_termination::enable(),
            Function::TerminationRequested => crate::program_termination::requested(),
            _ => unreachable!("prepared process operation"),
        };
        // Every possible failure, source charge, caller range and return check
        // precedes installation. Completion below has no allocation or failure.
        let value = OperationValue::Scalar(Scalar::Bool(value));
        self.commit_experimental_result(frame, delivery.local, value);
        self.commit_experimental_result(delivery.caller, delivery.caller_result, value);
        self.frames[delivery.caller].calls[delivery.resume.call.0] = CallRuntime::default();
        self.branch(delivery.caller, delivery.resume.continuation);
        #[cfg(test)]
        {
            self.record_event(Event::Return(id));
            crate::program_termination::delivered();
        }
        self.frames.pop();
        self.live_slots -= usage.scalar_slots;
        self.live_cells -= usage.expanded_cells;
        self.live_bytes -= usage.reference_bytes;
        Ok(())
    }
}
