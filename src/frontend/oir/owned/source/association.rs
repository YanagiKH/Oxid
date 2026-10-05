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
    if !function.matches.is_empty() {
        return Err(bad());
    }
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
                OwnedInstruction::ConstructEnum { .. }
                | OwnedInstruction::ConsumeVariant { .. } => return Err(bad()),
                OwnedInstruction::Scalar(statement) => visitor.statement(statement)?,
                OwnedInstruction::Construct { fields, .. } => {
                    for (_, operand) in fields {
                        visitor.span(operand.span)?;
                    }
                }
                OwnedInstruction::ConstructComposite { fields, .. } => {
                    for (_, value) in fields {
                        if let FieldInitializer::Scalar(operand) = value {
                            visitor.span(operand.span)?;
                        }
                    }
                }
                OwnedInstruction::ReadProjection { index, .. } => {
                    if let Some(index) = index {
                        visitor.span(index.span)?;
                    }
                }
                OwnedInstruction::WriteProjection { index, value, .. } => {
                    if let Some(index) = index {
                        visitor.span(index.span)?;
                    }
                    visitor.span(value.span)?;
                }
                OwnedInstruction::ProjectionLength { .. } => (),
                OwnedInstruction::ConstructArray { elements, .. } => {
                    for operand in elements {
                        visitor.span(operand.span)?;
                    }
                }
                OwnedInstruction::ReadIndex { index, .. } => visitor.span(index.span)?,
                OwnedInstruction::WriteIndex { value, index, .. } => {
                    visitor.span(value.span)?;
                    visitor.span(index.span)?;
                }
                OwnedInstruction::WriteField { value, .. }
                | OwnedInstruction::PrepareScalar { value, .. } => visitor.span(value.span)?,
                OwnedInstruction::StorageLive(_)
                | OwnedInstruction::StorageEnd(_)
                | OwnedInstruction::MoveInitialize { .. }
                | OwnedInstruction::Replace { .. }
                | OwnedInstruction::Discard(_)
                | OwnedInstruction::ReadField { .. }
                | OwnedInstruction::ArrayLength { .. }
                | OwnedInstruction::OpenCall(_)
                | OwnedInstruction::PrepareOwned { .. }
                | OwnedInstruction::PrepareBorrow { .. } => (),
            }
        }
        if let Some(terminator) = &block.terminator {
            visitor.span(terminator.span)?;
            origins(terminator.diagnostic_origins, visitor)?;
            match &terminator.kind {
                OwnedTerminatorKind::MatchDispatch { .. } => return Err(bad()),
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
    if !raw.enums.is_empty() {
        return Err(bad());
    }
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

#[cfg(test)]
mod array_tests {
    use super::*;
    use crate::frontend::{lexer, parser};

    #[test]
    fn unit2b_association_walks_every_array_operand_in_both_passes() {
        let mut sources = SourceMap::new();
        let text = "struct C {} fn main()->(){let a=C{};return;}";
        let file = sources.add("array-association.ox".into(), text.into());
        let foreign = sources.add("unrelated.ox".into(), text.into());
        let source = sources.get(file);
        let ast = parser::parse_with_mode(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::OwnedCandidate,
        )
        .unwrap();
        let typed =
            super::super::typeck::check(super::super::resolve::resolve(source, &ast).unwrap())
                .unwrap();
        let mut raw = super::super::lower::lower(&typed).unwrap();
        let baseline = check(&raw, typed.index(), &sources).unwrap();
        let span = raw.functions[0].span;
        let operand = Operand {
            local: LocalId(0),
            span,
        };
        let base = AccessBase::Owner(OwnerPlaceId(0));
        let kinds = [
            (
                OwnedInstruction::ConstructArray {
                    destination: OwnerPlaceId(0),
                    elements: vec![operand; 1024],
                },
                1025,
            ),
            (
                OwnedInstruction::ReadIndex {
                    destination: LocalId(0),
                    base,
                    index: operand,
                },
                2,
            ),
            (
                OwnedInstruction::WriteIndex {
                    base,
                    index: operand,
                    value: operand,
                },
                3,
            ),
            (
                OwnedInstruction::ArrayLength {
                    destination: LocalId(0),
                    base,
                },
                1,
            ),
        ];
        for (kind, span_count) in kinds {
            raw.functions[0].blocks[0].statements.push(OwnedStatement {
                kind,
                span,
                diagnostic_origins: None,
            });
            let (result, allocations) =
                super::super::super::reviewer_origins::integration_counted(|| {
                    check(&raw, typed.index(), &sources)
                });
            assert_eq!(allocations, 0);
            let usage = result.unwrap();
            assert_eq!(usage.count.spans, baseline.count.spans + span_count);
            assert_eq!(
                usage.validation.spans,
                baseline.validation.spans + span_count
            );
            assert_eq!(usage.count.declarations, baseline.count.declarations);
            assert_eq!(usage.dimensions, baseline.dimensions);
            let original = raw.functions[0].blocks[0].statements.pop().unwrap();
            // One mutation per actual operand, plus the instruction and both
            // diagnostic origins. These spans are valid in SourceMap, but do
            // not belong to the indexed source function.
            let operand_count = span_count - 1;
            for location in 0..operand_count + 3 {
                let mut changed = original.clone();
                let wrong = Span {
                    file: foreign,
                    ..span
                };
                assert!(sources.is_valid_span(wrong));
                if location == operand_count {
                    changed.span = wrong;
                } else if location == operand_count + 1 {
                    changed.diagnostic_origins = Some(DiagnosticOrigins {
                        primary: wrong,
                        cause: span,
                    });
                } else if location == operand_count + 2 {
                    changed.diagnostic_origins = Some(DiagnosticOrigins {
                        primary: span,
                        cause: wrong,
                    });
                } else {
                    match &mut changed.kind {
                        OwnedInstruction::ConstructArray { elements, .. } => {
                            elements[location].span = wrong
                        }
                        OwnedInstruction::ReadIndex { index, .. } => index.span = wrong,
                        OwnedInstruction::WriteIndex { value, index, .. } => {
                            if location == 0 {
                                value.span = wrong;
                            } else {
                                index.span = wrong;
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                raw.functions[0].blocks[0].statements.push(changed);
                let e = check(&raw, typed.index(), &sources).unwrap_err();
                assert_eq!(e.code, "E0500");
                assert_eq!(e.stage, "oir-project-bind");
                raw.functions[0].blocks[0].statements.pop();
            }
        }
    }
}
