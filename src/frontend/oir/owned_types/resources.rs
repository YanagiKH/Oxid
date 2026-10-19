//! Allocation-free traversal of resource leaves from checked declarations.
//! No raw layout, pointer carrier, or producer-provided drop plan is accepted.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend::oir) enum ResourceKind {
    ByteBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend::oir) struct ResourceLeaf {
    pub(in crate::frontend::oir) offset: usize,
    pub(in crate::frontend::oir) kind: ResourceKind,
}

pub(in crate::frontend::oir) struct ResourceLeaves<'a, F> {
    declarations: &'a Declarations,
    read_tag: F,
    stack: [Option<(ValueTy, usize, usize)>; MAX_CONTAINMENT_DEPTH + 1],
    length: usize,
}

impl Declarations {
    /// Static resource presence, from checked nominal identities. The bounded
    /// stack traverses alternatives without manufacturing a runtime drop plan.
    pub(in crate::frontend::oir) fn contains_resources(
        &self,
        aggregate: AggregateTy,
    ) -> Result<bool, DeclarationError> {
        self.check_aggregate_type(aggregate)?;
        let mut stack = [None; MAX_CONTAINMENT_DEPTH + 1];
        stack[0] = Some((aggregate, 0));
        let mut length = 1;
        while length != 0 {
            let (aggregate, next) = stack[length - 1].expect("checked resource presence stack");
            let child = match aggregate {
                AggregateTy::ByteBuf => return Ok(true),
                AggregateTy::FixedArray(_) => None,
                AggregateTy::Record(record) => {
                    self.fields(record)?.get(next).map(FieldDecl::value_ty)
                }
                AggregateTy::Enum(enumeration) => {
                    self.enums.variants(enumeration)?.get(next).map(|variant| {
                        variant
                            .value_payload()
                            .unwrap_or(ValueTy::Scalar(hir::Ty::Unit))
                    })
                }
            };
            match child {
                None => length -= 1,
                Some(ValueTy::Scalar(_)) => stack[length - 1] = Some((aggregate, next + 1)),
                Some(ValueTy::Owned(child)) => {
                    stack[length - 1] = Some((aggregate, next + 1));
                    if length == stack.len() {
                        return Err(DeclarationError::ResourceLimit(
                            "resource containment depth",
                        ));
                    }
                    stack[length] = Some((child, 0));
                    length += 1;
                }
            }
        }
        Ok(false)
    }

    /// Traverse only the active alternative. The tag reader supplies bytes,
    /// never layout/type/drop authority. Fields are visited in declaration order.
    pub(in crate::frontend::oir) fn resource_leaves<F>(
        &self,
        aggregate: AggregateTy,
        read_tag: F,
    ) -> Result<ResourceLeaves<'_, F>, DeclarationError>
    where
        F: FnMut(usize) -> Result<usize, DeclarationError>,
    {
        self.check_aggregate_type(aggregate)?;
        let mut stack = [None; MAX_CONTAINMENT_DEPTH + 1];
        stack[0] = Some((ValueTy::Owned(aggregate), 0, 0));
        Ok(ResourceLeaves {
            declarations: self,
            read_tag,
            stack,
            length: 1,
        })
    }

    /// Declaration-only resource detection for the temporary executable gate.
    /// Every record/enum is present, so a direct leaf anywhere also detects
    /// arbitrarily nested containment without expanding the declaration DAG.
    pub(in crate::frontend::oir) fn has_resource_declarations(&self) -> bool {
        self.fields
            .iter()
            .any(|field| field.ty == ValueTy::Owned(AggregateTy::ByteBuf))
            || self.enums.enums().iter().any(|enumeration| {
                self.enums
                    .variants(enumeration.id())
                    .expect("checked enum")
                    .iter()
                    .any(|variant| {
                        variant.value_payload() == Some(ValueTy::Owned(AggregateTy::ByteBuf))
                    })
            })
    }
}

impl<F> ResourceLeaves<'_, F>
where
    F: FnMut(usize) -> Result<usize, DeclarationError>,
{
    fn push(&mut self, ty: ValueTy, offset: usize) -> Result<(), DeclarationError> {
        if self.length == self.stack.len() {
            return Err(DeclarationError::ResourceLimit(
                "resource containment depth",
            ));
        }
        self.stack[self.length] = Some((ty, offset, 0));
        self.length += 1;
        Ok(())
    }
    fn advance(&mut self) -> Result<Option<ResourceLeaf>, DeclarationError> {
        while self.length != 0 {
            let position = self.length - 1;
            let (ty, offset, next) = self.stack[position].expect("checked traversal stack");
            match ty {
                ValueTy::Scalar(_) | ValueTy::Owned(AggregateTy::FixedArray(_)) => self.length -= 1,
                ValueTy::Owned(AggregateTy::ByteBuf) => {
                    self.length -= 1;
                    return Ok(Some(ResourceLeaf {
                        offset,
                        kind: ResourceKind::ByteBuf,
                    }));
                }
                ValueTy::Owned(AggregateTy::Record(record)) => {
                    if let Some(field) = self.declarations.fields(record)?.get(next) {
                        self.stack[position] = Some((ty, offset, next + 1));
                        let field_offset = offset
                            .checked_add(field.offset())
                            .ok_or(DeclarationError::LayoutOverflow)?;
                        self.push(field.value_ty(), field_offset)?;
                    } else {
                        self.length -= 1;
                    }
                }
                ValueTy::Owned(AggregateTy::Enum(enumeration)) => {
                    self.length -= 1;
                    let tag = (self.read_tag)(offset)?;
                    let variant = self.declarations.enums.variant_for_tag(enumeration, tag)?;
                    if let Some(payload) = variant.value_payload() {
                        let payload_offset = offset
                            .checked_add(variant.payload_offset().expect("checked active payload"))
                            .ok_or(DeclarationError::LayoutOverflow)?;
                        self.push(payload, payload_offset)?;
                    }
                }
            }
        }
        Ok(None)
    }
}
impl<F> Iterator for ResourceLeaves<'_, F>
where
    F: FnMut(usize) -> Result<usize, DeclarationError>,
{
    type Item = Result<ResourceLeaf, DeclarationError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.advance() {
            Ok(value) => value.map(Ok),
            Err(error) => {
                self.length = 0;
                Some(Err(error))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{record, source};
    use super::*;

    fn owned(aggregate: AggregateTy) -> ValueTy {
        ValueTy::Owned(aggregate)
    }
    fn enumeration(id: usize, payloads: &[Option<ValueTy>], span: Span) -> RawEnumDecl {
        RawEnumDecl {
            id: EnumId(id),
            span,
            variants: payloads
                .iter()
                .enumerate()
                .map(|(index, payload)| RawVariantDecl {
                    id: VariantId {
                        enumeration: EnumId(id),
                        index,
                    },
                    span,
                    payload: payload.map(ParameterTy::Value),
                })
                .collect(),
        }
    }
    fn declarations() -> Declarations {
        let (sources, span) = source();
        let mut document = record(0, &[hir::Ty::I32, hir::Ty::Unit, hir::Ty::I32], span);
        document.fields[1].ty = ParameterTy::Value(owned(AggregateTy::ByteBuf));
        let mut nested = record(1, &[hir::Ty::Unit, hir::Ty::Unit], span);
        nested.fields[0].ty = ParameterTy::Value(owned(AggregateTy::Record(RecordId(0))));
        nested.fields[1].ty = ParameterTy::Value(owned(AggregateTy::ByteBuf));
        let enums = [
            enumeration(
                0,
                &[
                    Some(owned(AggregateTy::ByteBuf)),
                    Some(ValueTy::Scalar(hir::Ty::I32)),
                    None,
                ],
                span,
            ),
            enumeration(
                1,
                &[Some(owned(AggregateTy::Record(RecordId(0)))), None],
                span,
            ),
            enumeration(2, &[Some(owned(AggregateTy::Enum(EnumId(0)))), None], span),
            enumeration(
                3,
                &[Some(owned(AggregateTy::Record(RecordId(1)))), None],
                span,
            ),
        ];
        Declarations::check_combined(&[document, nested], &enums, &sources).unwrap()
    }
    #[test]
    fn bytebuf_layout_resource_width_and_enum_alignment_are_derived() {
        let d = declarations();
        for (ty, size, width) in [
            (AggregateTy::ByteBuf, 16, 3),
            (AggregateTy::Record(RecordId(0)), 32, 5),
            (AggregateTy::Record(RecordId(1)), 48, 8),
            (AggregateTy::Enum(EnumId(0)), 24, 4),
            (AggregateTy::Enum(EnumId(1)), 40, 6),
            (AggregateTy::Enum(EnumId(2)), 32, 5),
            (AggregateTy::Enum(EnumId(3)), 56, 9),
        ] {
            assert_eq!(d.aggregate_layout(ty).unwrap(), Layout { size, align: 8 });
            assert_eq!(d.aggregate_width(ty).unwrap(), width);
        }
        for enumeration in d.enums.enums() {
            for variant in d.enums.variants(enumeration.id()).unwrap() {
                assert_eq!(variant.payload_offset(), variant.value_payload().map(|_| 8));
            }
        }
        assert_eq!(
            d.fields(RecordId(0))
                .unwrap()
                .iter()
                .map(FieldDecl::offset)
                .collect::<Vec<_>>(),
            [0, 8, 24]
        );
        assert!(d.has_resource_declarations());
        assert!(d.leaves(AggregateTy::ByteBuf).is_err());
        assert_eq!(
            d.leaves(AggregateTy::Record(RecordId(0)))
                .unwrap()
                .map(|leaf| (leaf.offset, leaf.ty))
                .collect::<Vec<_>>(),
            [(0, hir::Ty::I32), (24, hir::Ty::I32)]
        );
        assert_eq!(size_of::<AggregateSlot>(), 8);
        assert_eq!(size_of::<BorrowedSlot>(), 8);
        assert_eq!(size_of::<enums::EnumDecl>(), 48);
        assert_eq!(size_of::<enums::VariantDecl>(), 48);
    }
    #[test]
    fn bytebuf_resource_walk_uses_field_order_and_only_active_alternatives() {
        let d = declarations();
        let offsets = |ty, tags: &[(usize, usize)]| {
            let mut read = Vec::new();
            let leaves = d
                .resource_leaves(ty, |offset| {
                    read.push(offset);
                    Ok(tags
                        .iter()
                        .find(|(at, _)| *at == offset)
                        .expect("unexpected tag read")
                        .1)
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            (
                leaves.iter().map(|leaf| leaf.offset).collect::<Vec<_>>(),
                read,
            )
        };
        assert_eq!(
            offsets(AggregateTy::Record(RecordId(1)), &[]),
            (vec![8, 32], vec![])
        );
        assert_eq!(
            offsets(AggregateTy::Enum(EnumId(2)), &[(0, 0), (8, 0)]),
            (vec![16], vec![0, 8])
        );
        assert_eq!(
            offsets(AggregateTy::Enum(EnumId(2)), &[(0, 0), (8, 1)]),
            (vec![], vec![0, 8])
        );
        assert_eq!(
            offsets(AggregateTy::Enum(EnumId(2)), &[(0, 1)]),
            (vec![], vec![0])
        );
        assert_eq!(
            offsets(AggregateTy::Enum(EnumId(3)), &[(0, 0)]),
            (vec![16, 40], vec![0])
        );
        let mut invalid = d
            .resource_leaves(AggregateTy::Enum(EnumId(0)), |_| Ok(99))
            .unwrap();
        assert!(invalid.next().unwrap().is_err());
        assert!(invalid.next().is_none());
    }
    #[test]
    fn bytebuf_containment_admits_byte_array_fields_but_rejects_enum_fields() {
        let (sources, span) = source();
        let mut raw = record(0, &[hir::Ty::Unit], span);
        raw.fields[0].ty = ParameterTy::Value(owned(AggregateTy::Enum(EnumId(0))));
        let field = raw.fields[0].id;
        let enums = [enumeration(0, &[None], span)];
        assert!(matches!(
            Declarations::check_combined(&[raw], &enums, &sources),
            Err(DeclarationError::NonScalarField(id)) if id == field
        ));

        // Byte-array fields are ordinary values, not ByteBuf resources.
        let array = AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::U8, 1).unwrap());
        let mut raw = record(0, &[hir::Ty::Unit], span);
        raw.fields[0].ty = ParameterTy::Value(owned(array));
        let declarations = Declarations::check_combined(&[raw], &[], &sources).unwrap();
        assert_eq!(
            declarations.fields(RecordId(0)).unwrap()[0].value_ty(),
            owned(array)
        );
        assert!(!declarations.has_resource_declarations());
        assert_eq!(
            declarations
                .leaves(AggregateTy::Record(RecordId(0)))
                .unwrap()
                .map(|leaf| (leaf.offset, leaf.ty))
                .collect::<Vec<_>>(),
            [(0, hir::Ty::U8)]
        );
        assert!(
            BorrowedTy::ScalarSlice(hir::Ty::U8).accepts(BorrowedTy::Exact(AggregateTy::ByteBuf))
        );
        assert!(
            !BorrowedTy::ScalarSlice(hir::Ty::I32).accepts(BorrowedTy::Exact(AggregateTy::ByteBuf))
        );
        assert!(
            !BorrowedTy::Exact(AggregateTy::ByteBuf).accepts(BorrowedTy::ScalarSlice(hir::Ty::U8))
        );
    }
}
