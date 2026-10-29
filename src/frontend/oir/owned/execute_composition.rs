//! Active scalar cells in alternating record/enum layouts.
use super::*;

type Stack = [Option<(ValueTy, usize, usize)>; MAX_CONTAINMENT_DEPTH + 1];
pub(super) struct Cells<'a, V> {
    declarations: &'a Declarations,
    bytes: &'a [u8],
    extent: std::ops::Range<usize>,
    stack: Stack,
    length: usize,
    span: Span,
    value: std::marker::PhantomData<V>,
}
impl<'a, V: StoredValue> Cells<'a, V> {
    pub(super) fn new(
        declarations: &'a Declarations,
        bytes: &'a [u8],
        extent: std::ops::Range<usize>,
        aggregate: AggregateTy,
        span: Span,
    ) -> Self {
        let mut stack = [None; MAX_CONTAINMENT_DEPTH + 1];
        stack[0] = Some((ValueTy::Owned(aggregate), 0, 0));
        Self {
            declarations,
            bytes,
            extent,
            stack,
            length: 1,
            span,
            value: std::marker::PhantomData,
        }
    }
    fn read(&self, leaf: ScalarLeaf) -> Result<(ScalarLeaf, V)> {
        let offset = scalar_offset(self.extent.clone(), leaf.offset, leaf.ty, self.span)?;
        Ok((leaf, V::decode(self.bytes, offset, leaf.ty, self.span)?))
    }
    fn push(&mut self, value: ValueTy, relative: usize) -> Result<()> {
        if self.length == self.stack.len() {
            return Err(bad("composed value depth", self.span));
        }
        self.stack[self.length] = Some((value, relative, 0));
        self.length += 1;
        Ok(())
    }
    fn advance(&mut self) -> Result<Option<(ScalarLeaf, V)>> {
        while self.length != 0 {
            let position = self.length - 1;
            let (value, relative, next) =
                self.stack[position].ok_or_else(|| bad("composed value cursor", self.span))?;
            match value {
                ValueTy::Scalar(ty) => {
                    self.length -= 1;
                    return self
                        .read(ScalarLeaf {
                            offset: relative,
                            ty,
                        })
                        .map(Some);
                }
                ValueTy::Owned(
                    AggregateTy::ByteBuf
                    | AggregateTy::File
                    | AggregateTy::Vector(_)
                    | AggregateTy::RecordVector(_)
                    | AggregateTy::OwnedRecordVector(_),
                ) => self.length -= 1,
                ValueTy::Owned(AggregateTy::FixedArray(array)) => {
                    if next < array.length().max(1) {
                        self.stack[position] = Some((value, relative, next + 1));
                        let offset = next
                            .checked_mul(array.stride())
                            .and_then(|offset| relative.checked_add(offset))
                            .ok_or_else(|| bad("composed array offset", self.span))?;
                        return self
                            .read(ScalarLeaf {
                                offset,
                                ty: array.element(),
                            })
                            .map(Some);
                    }
                    self.length -= 1;
                }
                ValueTy::Owned(AggregateTy::Record(record)) => {
                    let fields = self
                        .declarations
                        .fields(record)
                        .map_err(|_| bad("composed record identity", self.span))?;
                    if fields.is_empty() && next == 0 {
                        self.length -= 1;
                        return self
                            .read(ScalarLeaf {
                                offset: relative,
                                ty: hir::Ty::Unit,
                            })
                            .map(Some);
                    }
                    if let Some(field) = fields.get(next) {
                        self.stack[position] = Some((value, relative, next + 1));
                        let offset = relative
                            .checked_add(field.offset())
                            .ok_or_else(|| bad("composed record offset", self.span))?;
                        self.push(field.value_ty(), offset)?;
                    } else {
                        self.length -= 1;
                    }
                }
                ValueTy::Owned(AggregateTy::Enum(enumeration)) => {
                    if next != 0 {
                        self.length -= 1;
                        continue;
                    }
                    let leaf = ScalarLeaf {
                        offset: relative,
                        ty: hir::Ty::I32,
                    };
                    let (_, tag) = self.read(leaf)?;
                    let Some(Scalar::I32(tag)) = tag.narrow() else {
                        return Err(bad("composed enum tag type", self.span));
                    };
                    let variant = self
                        .declarations
                        .enums()
                        .variant_for_tag(enumeration, tag as u32 as usize)
                        .map_err(|_| bad("composed enum tag", self.span))?;
                    self.stack[position] = Some((value, relative, 1));
                    if let Some(payload) = variant.value_payload() {
                        let offset = variant
                            .payload_offset()
                            .and_then(|offset| relative.checked_add(offset))
                            .ok_or_else(|| bad("composed enum payload offset", self.span))?;
                        self.push(payload, offset)?;
                    }
                    return Ok(Some((leaf, V::from(Scalar::I32(tag)))));
                }
            }
        }
        Ok(None)
    }
}
impl<V: StoredValue> Iterator for Cells<'_, V> {
    type Item = Result<(ScalarLeaf, V)>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.advance() {
            Ok(Some(value)) => Some(Ok(value)),
            Ok(None) => None,
            Err(error) => {
                self.length = 0;
                Some(Err(error))
            }
        }
    }
}

pub(super) fn control_bytes<V: StoredValue>() -> usize {
    // Factory, return, caller, iterator and loop-result transports are distinct.
    5 * size_of::<Cells<'static, V>>()
        + 2 * size_of::<Stack>()
        + 8 * size_of::<Result<(ScalarLeaf, V)>>()
        + 16 * size_of::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn declarations() -> (Declarations, Span) {
        let mut sources = SourceMap::new();
        let id = sources.add(
            "composition-cells.ox".into(),
            "records enums scalar cells".into(),
        );
        let span = sources.get(id).span(0, 6);
        let values = [
            vec![
                ValueTy::Owned(AggregateTy::Enum(EnumId(0))),
                ValueTy::Scalar(hir::Ty::U8),
            ],
            vec![ValueTy::Scalar(hir::Ty::Char)],
        ];
        let records = values
            .iter()
            .enumerate()
            .map(|(id, fields)| RawRecordDecl {
                is_copy: false,
                id: RecordId(id),
                span,
                fields: fields
                    .iter()
                    .enumerate()
                    .map(|(index, ty)| RawFieldDecl {
                        id: FieldId {
                            record: RecordId(id),
                            index,
                        },
                        ty: ParameterTy::Value(*ty),
                        span,
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        let enums = [RawEnumDecl {
            id: EnumId(0),
            span,
            variants: vec![
                RawVariantDecl {
                    id: VariantId {
                        enumeration: EnumId(0),
                        index: 0,
                    },
                    payload: None,
                    span,
                },
                RawVariantDecl {
                    id: VariantId {
                        enumeration: EnumId(0),
                        index: 1,
                    },
                    payload: Some(ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(
                        RecordId(1),
                    )))),
                    span,
                },
            ],
        }];
        (
            Declarations::check_combined(&records, &enums, &sources).unwrap(),
            span,
        )
    }
    #[test]
    fn composed_cells_visit_only_active_payloads_with_relative_offsets() {
        let (declarations, span) = declarations();
        let aggregate = AggregateTy::Record(RecordId(0));
        assert_eq!(declarations.aggregate_layout(aggregate).unwrap().size(), 12);
        for base in [0, 7] {
            for tag in [0u32, 1] {
                let mut bytes = vec![0xa5; base + 12];
                bytes[base..base + 4].copy_from_slice(&tag.to_le_bytes());
                if tag == 1 {
                    bytes[base + 4..base + 8].copy_from_slice(&u32::from('雪').to_le_bytes());
                }
                bytes[base + 8] = 255;
                let cells = Cells::<RuntimeScalar>::new(
                    &declarations,
                    &bytes,
                    base..base + 12,
                    aggregate,
                    span,
                )
                .collect::<Result<Vec<_>>>()
                .unwrap();
                let offsets = cells
                    .iter()
                    .map(|(leaf, _)| (leaf.offset, leaf.ty))
                    .collect::<Vec<_>>();
                assert_eq!(
                    offsets,
                    if tag == 0 {
                        vec![(0, hir::Ty::I32), (8, hir::Ty::U8)]
                    } else {
                        vec![(0, hir::Ty::I32), (4, hir::Ty::Char), (8, hir::Ty::U8)]
                    }
                );
                assert_eq!(cells.last().unwrap().1.narrow(), Some(Scalar::U8(255)));
                if tag == 1 {
                    assert_eq!(cells[1].1.character(), Some('雪'));
                }
            }
        }
    }
    #[test]
    fn composed_transfer_preserves_inert_bytes_and_refuses_invalid_active_data() {
        let (declarations, span) = declarations();
        let aggregate = AggregateTy::Record(RecordId(0));
        let mut source = vec![0xa5; 12];
        source[..4].copy_from_slice(&0u32.to_le_bytes());
        source[8] = 255;
        let mut destination = vec![0x5a; 12];
        copy_active_bytes_for::<RuntimeScalar, _>(
            &declarations,
            aggregate,
            &source,
            &mut destination,
            span,
        )
        .unwrap();
        assert_eq!(&destination[..4], &[0; 4]);
        assert_eq!(destination[8], 255);
        assert_eq!(&destination[4..8], &[0x5a; 4]);
        assert_eq!(&destination[9..], &[0x5a; 3]);
        for tag in [1u32, 255] {
            source[..4].copy_from_slice(&tag.to_le_bytes());
            source[4..8].copy_from_slice(&0xd800u32.to_le_bytes());
            assert!(
                Cells::<RuntimeScalar>::new(&declarations, &source, 0..12, aggregate, span)
                    .collect::<Result<Vec<_>>>()
                    .is_err()
            );
        }
    }
}
