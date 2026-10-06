use super::super::verify::{CfgView, ScalarUse};
use super::*;
fn error(span: Span) -> OirFailure {
    OirFailure::new(FailureKind::InvalidBlock, "oir-owned-cfg", Some(span))
}
impl RawOwnedFunction {
    fn block(&self, b: usize) -> Result<&OwnedBlock, OirFailure> {
        self.blocks.get(b).ok_or_else(|| error(self.span))
    }
    fn instruction(&self, b: usize, s: usize) -> Result<&OwnedStatement, OirFailure> {
        self.block(b)?
            .statements
            .get(s)
            .ok_or_else(|| error(self.span))
    }
    fn end(&self, b: usize) -> Result<&OwnedTerminator, OirFailure> {
        self.block(b)?
            .terminator
            .as_ref()
            .ok_or_else(|| error(self.span))
    }
}
impl CfgView for RawOwnedFunction {
    fn span(&self) -> Span {
        self.span
    }
    fn entry(&self) -> BlockId {
        self.entry
    }
    fn locals(&self) -> &[LocalDecl] {
        &self.locals
    }
    fn places(&self) -> &[PlaceDecl] {
        &self.places
    }
    fn param_count(&self) -> usize {
        self.parameters
            .iter()
            .filter(|p| matches!(p, ParameterBinding::Scalar(_)))
            .count()
    }
    fn block_count(&self) -> usize {
        self.blocks.len()
    }
    fn block_span(&self, b: usize) -> Result<Span, OirFailure> {
        Ok(self.block(b)?.span)
    }
    fn merge(&self, b: usize) -> Result<Option<&BoolMerge>, OirFailure> {
        Ok(self.block(b)?.merge.as_ref())
    }
    fn statement_count(&self, b: usize) -> Result<usize, OirFailure> {
        Ok(self.block(b)?.statements.len())
    }
    fn statement_definition(
        &self,
        b: usize,
        s: usize,
    ) -> Result<Option<(LocalId, Span)>, OirFailure> {
        let i = self.instruction(b, s)?;
        Ok(match &i.kind {
            OwnedInstruction::Scalar(Statement::Assign(a)) => Some((a.destination, a.span)),
            OwnedInstruction::ReadField { destination, .. }
            | OwnedInstruction::ReadIndex { destination, .. }
            | OwnedInstruction::ArrayLength { destination, .. }
            | OwnedInstruction::ReadProjection { destination, .. }
            | OwnedInstruction::ProjectionLength { destination, .. } => {
                Some((*destination, i.span))
            }
            OwnedInstruction::ConsumeVariant {
                match_id,
                destination,
                ..
            } => {
                let descriptor = self.matches.get(match_id.0).ok_or_else(|| error(i.span))?;
                destination.map(|id| (id, descriptor.span))
            }
            _ => None,
        })
    }
    fn statement_initialization(
        &self,
        b: usize,
        s: usize,
    ) -> Result<Option<(PlaceId, Span)>, OirFailure> {
        Ok(match &self.instruction(b, s)?.kind {
            OwnedInstruction::Scalar(Statement::Initialize { place, span, .. }) => {
                Some((place.id, *span))
            }
            _ => None,
        })
    }
    fn statement_uses(
        &self,
        b: usize,
        s: usize,
        visit: &mut dyn FnMut(ScalarUse) -> Result<(), OirFailure>,
    ) -> Result<(), OirFailure> {
        match &self.instruction(b, s)?.kind {
            OwnedInstruction::Scalar(i) => super::super::verify::scalar_statement_uses(i, visit)?,
            OwnedInstruction::ConstructEnum {
                payload: Some(value),
                ..
            } => {
                visit(ScalarUse::Operand(*value))?;
            }
            OwnedInstruction::Construct { fields, .. } => {
                for (_, v) in fields {
                    visit(ScalarUse::Operand(*v))?;
                }
            }
            OwnedInstruction::ConstructComposite { fields, .. } => {
                for (_, value) in fields {
                    if let FieldInitializer::Scalar(value) = value {
                        visit(ScalarUse::Operand(*value))?;
                    }
                }
            }
            OwnedInstruction::ReadProjection {
                index: Some(index), ..
            } => visit(ScalarUse::Operand(*index))?,
            OwnedInstruction::WriteProjection { index, value, .. } => {
                visit(ScalarUse::Operand(*value))?;
                if let Some(index) = index {
                    visit(ScalarUse::Operand(*index))?;
                }
            }
            OwnedInstruction::ConstructArray { elements, .. } => {
                for value in elements {
                    visit(ScalarUse::Operand(*value))?;
                }
            }
            OwnedInstruction::ReadIndex { index, .. } => visit(ScalarUse::Operand(*index))?,
            OwnedInstruction::WriteIndex { index, value, .. } => {
                visit(ScalarUse::Operand(*value))?;
                visit(ScalarUse::Operand(*index))?;
            }
            OwnedInstruction::WriteField { value, .. }
            | OwnedInstruction::PrepareScalar { value, .. } => visit(ScalarUse::Operand(*value))?,
            _ => {}
        }
        Ok(())
    }
    fn call_result(&self, b: usize) -> Result<Option<(LocalId, BlockId, Span)>, OirFailure> {
        let end = self.end(b)?;
        Ok(match end.kind {
            OwnedTerminatorKind::Invoke { call, continuation } => match self
                .calls
                .get(call.0)
                .ok_or_else(|| error(end.span))?
                .result
            {
                CallResult::Scalar(id) => Some((id, continuation, end.span)),
                CallResult::Owned(_) => None,
            },
            _ => None,
        })
    }
    fn terminator_uses(
        &self,
        b: usize,
        visit: &mut dyn FnMut(ScalarUse) -> Result<(), OirFailure>,
    ) -> Result<(), OirFailure> {
        match self.end(b)?.kind {
            OwnedTerminatorKind::Branch { condition, .. }
            | OwnedTerminatorKind::ReturnScalar(condition) => visit(ScalarUse::Operand(condition))?,
            _ => {}
        }
        Ok(())
    }
    fn successors(&self, b: usize) -> Result<[Option<BlockId>; 2], OirFailure> {
        Ok(match self.end(b)?.kind {
            OwnedTerminatorKind::MatchDispatch { match_id, arm } => {
                let descriptor = self
                    .matches
                    .get(match_id.0)
                    .ok_or_else(|| error(self.span))?;
                let current = descriptor
                    .arms
                    .get(arm)
                    .ok_or_else(|| error(descriptor.span))?;
                let next = arm.checked_add(1).and_then(|i| descriptor.arms.get(i));
                [Some(current.entry), next.map(|a| a.dispatch)]
            }
            OwnedTerminatorKind::Branch {
                then_block,
                else_block,
                ..
            } => [Some(then_block), Some(else_block)],
            OwnedTerminatorKind::Goto(t)
            | OwnedTerminatorKind::Invoke {
                continuation: t, ..
            } => [Some(t), None],
            _ => [None, None],
        })
    }
}
