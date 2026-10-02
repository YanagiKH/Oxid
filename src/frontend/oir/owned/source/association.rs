//! The owned source association walk; raw ownership/shape verification is unchanged.
use super::super::*;
use crate::frontend::{
    declaration_index::DeclarationIndex,
    oir::source::association::{bad, BindUsage, Visitor},
};

fn record(record: &RawRecordDecl, visitor: &mut Visitor<'_>) -> Result<(), Box<Diagnostic>> {
    visitor.declaration()?;
    visitor.span(record.span)?;
    for field in &record.fields {
        visitor.declaration()?;
        visitor.span(field.span)?;
    }
    Ok(())
}
fn origins(
    origins: Option<DiagnosticOrigins>,
    visitor: &mut Visitor<'_>,
) -> Result<(), Box<Diagnostic>> {
    if let Some(origins) = origins {
        visitor.span(origins.primary)?;
        visitor.span(origins.cause)?;
    }
    Ok(())
}
fn function(function: &RawOwnedFunction, visitor: &mut Visitor<'_>) -> Result<(), Box<Diagnostic>> {
    visitor.declaration()?;
    visitor.span(function.span)?;
    for local in &function.locals {
        visitor.span(local.span)?;
    }
    for place in &function.places {
        visitor.span(place.span)?;
    }
    for owner in &function.owners {
        visitor.span(owner.span)?;
    }
    for reference in &function.references {
        visitor.span(reference.span)?;
    }
    for call in &function.calls {
        visitor.span(call.span)?;
    }
    for loan in &function.loans {
        visitor.span(loan.span)?;
    }
    for block in &function.blocks {
        visitor.span(block.span)?;
        if let Some(merge) = &block.merge {
            visitor.merge(merge)?;
        }
        for statement in &block.statements {
            visitor.span(statement.span)?;
            origins(statement.diagnostic_origins, visitor)?;
            match &statement.kind {
                OwnedInstruction::Scalar(statement) => visitor.statement(statement)?,
                OwnedInstruction::Construct { fields, .. } => {
                    for (_, operand) in fields {
                        visitor.span(operand.span)?;
                    }
                }
                OwnedInstruction::WriteField { value, .. }
                | OwnedInstruction::PrepareScalar { value, .. } => visitor.span(value.span)?,
                OwnedInstruction::StorageLive(_)
                | OwnedInstruction::StorageEnd(_)
                | OwnedInstruction::MoveInitialize { .. }
                | OwnedInstruction::Replace { .. }
                | OwnedInstruction::Discard(_)
                | OwnedInstruction::ReadField { .. }
                | OwnedInstruction::OpenCall(_)
                | OwnedInstruction::PrepareOwned { .. }
                | OwnedInstruction::PrepareBorrow { .. } => (),
            }
        }
        if let Some(terminator) = &block.terminator {
            visitor.span(terminator.span)?;
            origins(terminator.diagnostic_origins, visitor)?;
            match &terminator.kind {
                OwnedTerminatorKind::Branch { condition, .. } => visitor.span(condition.span)?,
                OwnedTerminatorKind::ReturnScalar(operand) => visitor.span(operand.span)?,
                OwnedTerminatorKind::Goto(_)
                | OwnedTerminatorKind::Invoke { .. }
                | OwnedTerminatorKind::ReturnOwned(_) => (),
            }
        }
    }
    Ok(())
}
pub(super) fn check(
    raw: &RawOwnedProgram,
    index: &DeclarationIndex<'_>,
    sources: &SourceMap,
) -> Result<BindUsage, Box<Diagnostic>> {
    let mut count = Visitor::count();
    for declaration in &raw.records {
        record(declaration, &mut count)?;
    }
    for declaration in &raw.functions {
        function(declaration, &mut count)?;
    }
    let mut visitor = Visitor::validate(sources);
    visitor.dimension(raw.records.len(), index.record_count())?;
    visitor.dimension(raw.functions.len(), index.function_count())?;
    for (ordinal, declaration) in raw.records.iter().enumerate() {
        let id = RecordId(ordinal);
        let (key, module) = index.record(id).map_err(|_| bad())?;
        let ast = index.sources().ast(module).map_err(|_| bad())?;
        let original = ast.records.get(key.index).ok_or_else(bad)?;
        if declaration.id != id || declaration.span != original.name {
            return Err(bad());
        }
        visitor.dimension(declaration.fields.len(), original.fields.len())?;
        for (field_index, (field, original)) in
            declaration.fields.iter().zip(&original.fields).enumerate()
        {
            if field.id
                != (FieldId {
                    record: id,
                    index: field_index,
                })
                || field.span != original.name
            {
                return Err(bad());
            }
        }
        visitor.file(original.name.file);
        record(declaration, &mut visitor)?;
    }
    for (ordinal, declaration) in raw.functions.iter().enumerate() {
        let id = hir::DefId(ordinal);
        let (key, module) = index.function(id).map_err(|_| bad())?;
        let ast = index.sources().ast(module).map_err(|_| bad())?;
        let original = ast.functions.get(key.index).ok_or_else(bad)?;
        if declaration.id != id || declaration.span != original.name {
            return Err(bad());
        }
        visitor.file(original.name.file);
        function(declaration, &mut visitor)?;
    }
    visitor.finish(count)
}
