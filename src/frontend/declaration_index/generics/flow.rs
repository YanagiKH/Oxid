//! Affine state joins at loop heads and at each distinct transfer edge.
use super::*;

pub(super) struct LoopFrame {
    breaks: Vec<bool>,
    continues: Vec<bool>,
    repeated: bool,
}

impl Walker<'_, '_> {
    pub(super) fn in_repeated_loop(&self) -> bool {
        self.loop_frames.iter().any(|frame| frame.repeated)
    }

    pub(super) fn record_loop_exit(
        &mut self,
        breaks: bool,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        if !self.abstract_body || !self.reachable {
            return Ok(());
        }
        let frame = self.loop_frames.last_mut().ok_or_else(|| bad(at))?;
        let state = if breaks {
            &mut frame.breaks
        } else {
            &mut frame.continues
        };
        self.work
            .debit(state.len() as u64, at, "generic loop transfer")?;
        for (target, binding) in state.iter_mut().zip(&self.bindings) {
            *target |= binding.moved;
        }
        Ok(())
    }

    pub(super) fn check_loop(
        &mut self,
        condition: Option<ExprId>,
        body: ast::BodyBlockId,
        loops: usize,
        at: Span,
    ) -> Result<Flow, Box<Diagnostic>> {
        let mut head = self.snapshot()?;
        let reachable = self.reachable;
        let mut repeated = false;
        loop {
            self.bindings = head;
            let mut next_head = self.snapshot()?;
            let mut frame = LoopFrame {
                breaks: Vec::new(),
                continues: Vec::new(),
                repeated,
            };
            for _ in &next_head {
                self.storage
                    .push(&mut frame.breaks, false, self.allocator, self.work, at)?;
                self.storage
                    .push(&mut frame.continues, false, self.allocator, self.work, at)?;
            }
            self.storage
                .push(&mut self.loop_frames, frame, self.allocator, self.work, at)?;
            self.reachable = reachable;
            if let Some(condition) = condition {
                let condition = self.expression(condition)?;
                self.expect(condition, Type::scalar(Ty::Bool), at)?;
            }
            let mut exit = self.snapshot()?;
            let flow = self.block(body, loops + 1)?;
            let frame = self.loop_frames.pop().ok_or_else(|| bad(at))?;
            let mut changed = false;
            for (position, target) in next_head.iter_mut().enumerate() {
                let moved =
                    frame.continues[position] || (flow.falls && self.bindings[position].moved);
                changed |= moved && !target.moved;
                target.moved |= moved;
                exit[position].moved |= frame.breaks[position];
            }
            if self.abstract_body && reachable && changed {
                // Availability only decreases at a join. Recheck the next
                // iteration, allowing assignments to restore values before use.
                head = next_head;
                repeated = true;
                continue;
            }
            self.bindings = exit;
            self.reachable = reachable;
            return Ok(Flow {
                returns: flow.returns,
                ..Flow::FALL
            });
        }
    }
}
