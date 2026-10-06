//! The owned source association walk; raw ownership/shape verification is unchanged.
use super::super::*;
use crate::frontend::{
    declaration_index::DeclarationIndex,
    oir::source::association::{bad, BindUsage, Visitor},
};

fn enumeration(
    enumeration: &RawEnumDecl,
    visitor: &mut Visitor<'_>,
) -> Result<(), Box<Diagnostic>> {
    visitor.declaration()?;
    visitor.span(enumeration.span)?;
    for variant in &enumeration.variants {
        visitor.declaration()?;
        visitor.span(variant.span)?;
    }
    Ok(())
}
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
fn function(
    function: &RawOwnedFunction,
    visitor: &mut Visitor<'_>,
    allow_enums: bool,
) -> Result<(), Box<Diagnostic>> {
    if !allow_enums && !function.matches.is_empty() {
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
    for descriptor in &function.matches {
        visitor.declaration()?;
        visitor.span(descriptor.span)?;
        for _ in &descriptor.arms {
            // Raw arm rows have no independent source span.
            visitor.declaration()?;
        }
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
                OwnedInstruction::ConstructEnum { payload, .. } => {
                    if !allow_enums {
                        return Err(bad());
                    }
                    if let Some(payload) = payload {
                        visitor.span(payload.span)?;
                    }
                }
                OwnedInstruction::ConsumeVariant { .. } => {
                    if !allow_enums {
                        return Err(bad());
                    }
                }
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
                OwnedTerminatorKind::MatchDispatch { .. } => {
                    if !allow_enums {
                        return Err(bad());
                    }
                }
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
    check_impl(raw, index, sources, false)
}

/// Kept separate from the production association gate until source activation.
#[cfg(test)]
pub(super) fn check_enum_candidate(
    raw: &RawOwnedProgram,
    index: &DeclarationIndex<'_>,
    sources: &SourceMap,
) -> Result<BindUsage, Box<Diagnostic>> {
    check_impl(raw, index, sources, true)
}

fn check_impl(
    raw: &RawOwnedProgram,
    index: &DeclarationIndex<'_>,
    sources: &SourceMap,
    allow_enums: bool,
) -> Result<BindUsage, Box<Diagnostic>> {
    if !allow_enums && (!raw.enums.is_empty() || index.enum_count() != 0) {
        return Err(bad());
    }
    let mut count = Visitor::count();
    for declaration in &raw.enums {
        enumeration(declaration, &mut count)?;
    }
    for declaration in &raw.records {
        record(declaration, &mut count)?;
    }
    for declaration in &raw.functions {
        function(declaration, &mut count, allow_enums)?;
    }
    let mut visitor = Visitor::validate(sources);
    // The ordinary zero-enum path keeps its historical dimension/work count.
    if allow_enums {
        visitor.dimension(raw.enums.len(), index.enum_count())?;
    }
    for (ordinal, declaration) in raw.enums.iter().enumerate() {
        let id = EnumId(ordinal);
        let original = index.enum_view(id).map_err(|_| bad())?;
        if declaration.id != id || declaration.span != original.name_span() {
            return Err(bad());
        }
        visitor.dimension(declaration.variants.len(), original.variant_count())?;
        for (index, variant) in declaration.variants.iter().enumerate() {
            let id = VariantId {
                enumeration: id,
                index,
            };
            let expected = original.variant(id).map_err(|_| bad())?;
            if variant.id != id
                || variant.span != expected.name_span()
                || variant.payload
                    != expected
                        .payload()
                        .map(|ty| ParameterTy::Value(ValueTy::Scalar(ty)))
            {
                return Err(bad());
            }
        }
        visitor.file(original.name_span().file);
        enumeration(declaration, &mut visitor)?;
    }
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
        function(declaration, &mut visitor, allow_enums)?;
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

#[cfg(test)]
mod enum_tests {
    use super::*;
    use crate::frontend::{
        declaration_index::{self, IndexLimits, SourceOwner, WorkMeter},
        lexer, parser,
        project::budget::Allocator,
        source::SourceView,
    };

    #[test]
    fn enum_association_checks_unused_payload_identity_and_every_new_span() {
        let text = "enum E{N,V(i32),U(())} fn main()->(){return;}";
        let mut sources = SourceMap::new();
        let file = sources.add("enum-association.ox".into(), text.into());
        let foreign = sources.add("other-enum-association.ox".into(), text.into());
        let source = sources.get(file);
        let (ast, _) = parser::parse_enum_candidate_counted(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::ProjectCandidate,
            parser::MAX_NODES,
            &mut Allocator::default(),
            &mut Default::default(),
        )
        .unwrap();
        let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let index = declaration_index::collect_enum_candidate(
            owner,
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
        let declaration = &ast.enums[0];
        let span = ast.functions[0].name;
        let mut raw = RawOwnedProgram {
            enums: vec![RawEnumDecl {
                id: EnumId(0),
                span: declaration.name,
                variants: [None, Some(hir::Ty::I32), Some(hir::Ty::Unit)]
                    .into_iter()
                    .enumerate()
                    .map(|(index, payload)| RawVariantDecl {
                        id: VariantId {
                            enumeration: EnumId(0),
                            index,
                        },
                        payload: payload.map(|ty| ParameterTy::Value(ValueTy::Scalar(ty))),
                        span: declaration.variants[index].name,
                    })
                    .collect(),
            }],
            records: vec![],
            // Deliberately an association-only carrier fixture. Association
            // certifies provenance, not arbitrary expression/arm equivalence.
            functions: vec![RawOwnedFunction {
                id: hir::DefId(0),
                span,
                result: ValueTy::Scalar(hir::Ty::Unit),
                parameters: vec![],
                locals: vec![],
                places: vec![],
                owners: vec![],
                references: vec![],
                calls: vec![],
                loans: vec![],
                matches: vec![MatchDecl {
                    source: OwnerPlaceId(0),
                    span,
                    arms: vec![MatchArm {
                        variant: VariantId {
                            enumeration: EnumId(0),
                            index: 0,
                        },
                        dispatch: BlockId(0),
                        entry: BlockId(1),
                    }],
                }],
                entry: BlockId(0),
                blocks: vec![OwnedBlock {
                    merge: None,
                    span,
                    statements: vec![
                        OwnedStatement {
                            span,
                            diagnostic_origins: Some(DiagnosticOrigins {
                                primary: span,
                                cause: span,
                            }),
                            kind: OwnedInstruction::ConstructEnum {
                                destination: OwnerPlaceId(0),
                                variant: VariantId {
                                    enumeration: EnumId(0),
                                    index: 1,
                                },
                                payload: Some(Operand {
                                    local: LocalId(0),
                                    span,
                                }),
                            },
                        },
                        OwnedStatement {
                            span,
                            diagnostic_origins: Some(DiagnosticOrigins {
                                primary: span,
                                cause: span,
                            }),
                            kind: OwnedInstruction::ConsumeVariant {
                                match_id: MatchId(0),
                                arm: 0,
                                destination: None,
                            },
                        },
                    ],
                    terminator: Some(OwnedTerminator {
                        span,
                        diagnostic_origins: Some(DiagnosticOrigins {
                            primary: span,
                            cause: span,
                        }),
                        kind: OwnedTerminatorKind::MatchDispatch {
                            match_id: MatchId(0),
                            arm: 0,
                        },
                    }),
                }],
            }],
        };
        let (result, allocations) =
            super::super::super::reviewer_origins::integration_counted(|| {
                check_enum_candidate(&raw, &index, &sources)
            });
        assert_eq!(allocations, 0);
        let usage = result.unwrap();
        assert_eq!(usage.count, usage.validation);
        assert_eq!((usage.count.declarations, usage.count.spans), (7, 17));
        assert_eq!(usage.dimensions, 4);
        assert!(check(&raw, &index, &sources).is_err());

        // A scalar type mutation is still a valid independent raw declaration,
        // but no longer the enum declaration in the original source index.
        for payload in [
            Some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::Bool))),
            None,
        ] {
            let original = raw.enums[0].variants[1].payload;
            raw.enums[0].variants[1].payload = payload;
            EnumDeclarations::prepare(&raw.enums, &sources, DeclarationUsage::default()).unwrap();
            assert!(check_enum_candidate(&raw, &index, &sources).is_err());
            raw.enums[0].variants[1].payload = original;
        }
        let original = raw.enums[0].variants[2].payload;
        raw.enums[0].variants[2].payload = None;
        assert!(check_enum_candidate(&raw, &index, &sources).is_err());
        raw.enums[0].variants[2].payload = original;

        // Every new source carrier, optional scalar operand, and both origin
        // roles must remain in the indexed declaration/function source file.
        fn target_span(raw: &mut RawOwnedProgram, target: usize) -> &mut Span {
            match target {
                0 => &mut raw.enums[0].span,
                1 => &mut raw.enums[0].variants[1].span,
                2 => &mut raw.functions[0].matches[0].span,
                3 => &mut raw.functions[0].blocks[0].statements[0].span,
                4 => {
                    &mut raw.functions[0].blocks[0].statements[0]
                        .diagnostic_origins
                        .as_mut()
                        .unwrap()
                        .primary
                }
                5 => {
                    &mut raw.functions[0].blocks[0].statements[0]
                        .diagnostic_origins
                        .as_mut()
                        .unwrap()
                        .cause
                }
                6 => match &mut raw.functions[0].blocks[0].statements[0].kind {
                    OwnedInstruction::ConstructEnum {
                        payload: Some(payload),
                        ..
                    } => &mut payload.span,
                    _ => unreachable!(),
                },
                7 => &mut raw.functions[0].blocks[0].statements[1].span,
                8 => {
                    &mut raw.functions[0].blocks[0].statements[1]
                        .diagnostic_origins
                        .as_mut()
                        .unwrap()
                        .primary
                }
                9 => {
                    &mut raw.functions[0].blocks[0].statements[1]
                        .diagnostic_origins
                        .as_mut()
                        .unwrap()
                        .cause
                }
                10 => &mut raw.functions[0].blocks[0].terminator.as_mut().unwrap().span,
                11 => {
                    &mut raw.functions[0].blocks[0]
                        .terminator
                        .as_mut()
                        .unwrap()
                        .diagnostic_origins
                        .as_mut()
                        .unwrap()
                        .primary
                }
                _ => {
                    &mut raw.functions[0].blocks[0]
                        .terminator
                        .as_mut()
                        .unwrap()
                        .diagnostic_origins
                        .as_mut()
                        .unwrap()
                        .cause
                }
            }
        }
        for target in 0..13 {
            let wrong = Span {
                file: foreign,
                ..span
            };
            assert!(sources.is_valid_span(wrong));
            let saved = std::mem::replace(target_span(&mut raw, target), wrong);
            assert!(check_enum_candidate(&raw, &index, &sources).is_err());
            *target_span(&mut raw, target) = saved;
        }
        assert_eq!(check_enum_candidate(&raw, &index, &sources).unwrap(), usage);
    }
}
