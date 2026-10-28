//! Joint, bounded layout analysis for records containing enums.
use super::*;

pub(super) struct JointLayouts {
    summaries: Vec<ContainmentSummary>,
    shapes: Vec<u8>,
    records: usize,
}
impl JointLayouts {
    pub(super) fn records(&self) -> &[ContainmentSummary] {
        &self.summaries[..self.records]
    }
    pub(super) fn enums(&self) -> &[ContainmentSummary] {
        &self.summaries[self.records..]
    }
    pub(super) fn record_has_enum_nesting(&self, record: RecordId) -> bool {
        self.shapes[record.0] & 4 != 0
    }
    pub(super) fn enum_shapes(&self) -> &[u8] {
        &self.shapes[self.records..]
    }
}

fn ordinal(
    aggregate: AggregateTy,
    records: usize,
    enums: usize,
) -> Result<usize, DeclarationError> {
    match aggregate {
        AggregateTy::Record(id) if id.0 < records => Ok(id.0),
        AggregateTy::Record(id) => Err(DeclarationError::InvalidRecordId(id)),
        AggregateTy::Enum(id) if id.0 < enums => records
            .checked_add(id.0)
            .ok_or(DeclarationError::LayoutOverflow),
        AggregateTy::Enum(id) => Err(DeclarationError::InvalidEnumId(id)),
        _ => Err(DeclarationError::TypeMismatch),
    }
}

fn child(
    records: &[RawRecordDecl],
    enums: &[RawEnumDecl],
    node: usize,
    member: usize,
) -> Result<Option<ValueTy>, DeclarationError> {
    let value = if node < records.len() {
        let Some(field) = records[node].fields.get(member) else {
            return Ok(None);
        };
        match field.ty {
            ParameterTy::Value(value) => Some(value),
            _ => return Err(DeclarationError::NonScalarField(field.id)),
        }
    } else {
        match enums
            .get(node - records.len())
            .and_then(|declaration| declaration.variants.get(member))
        {
            Some(variant) => match variant.payload {
                Some(ParameterTy::Value(value)) => Some(value),
                Some(_) => return Err(DeclarationError::TypeMismatch),
                None => None,
            },
            None => None,
        }
    };
    Ok(value)
}

pub(super) fn value_summary(
    value: ValueTy,
    records: &[ContainmentSummary],
    enums: &[ContainmentSummary],
) -> Result<ContainmentSummary, DeclarationError> {
    match value {
        ValueTy::Owned(AggregateTy::Enum(id)) => enums
            .get(id.0)
            .copied()
            .ok_or(DeclarationError::InvalidEnumId(id)),
        other => super::value_summary(other, records),
    }
}

pub(super) fn layouts(
    records: &[RawRecordDecl],
    enums: &[RawEnumDecl],
    table_bytes: usize,
    limits: Limits,
    allocator: &mut Allocator,
) -> Result<JointLayouts, DeclarationError> {
    let count = limited_add(
        records.len(),
        enums.len(),
        limits.records,
        "aggregate declarations",
    )?;
    let mut members = 0usize;
    for record in records {
        limited_add(
            0,
            record.fields.len(),
            limits.fields_per_record,
            "fields per record",
        )?;
        members = limited_add(
            members,
            record.fields.len(),
            limits.fields,
            "aggregate members",
        )?;
    }
    for enumeration in enums {
        if enumeration.variants.is_empty() || enumeration.variants.len() > 256 {
            return Err(DeclarationError::ResourceLimit("variants per enum"));
        }
        members = limited_add(
            members,
            enumeration.variants.len(),
            limits.fields,
            "aggregate members",
        )?;
    }
    // Two exact heap requests plus the complete bounded traversal and return
    // controls. This is a requested metadata envelope, not an RSS measurement.
    let payload = count
        .checked_mul(size_of::<ContainmentSummary>() + size_of::<u8>())
        .ok_or(DeclarationError::LayoutOverflow)?;
    let controls = 16 * 1024;
    let scratch = limited_add(
        payload,
        controls,
        limits.table_bytes,
        "combined layout scratch bytes",
    )?;
    limited_add(
        table_bytes,
        scratch,
        limits.table_bytes,
        "combined layout scratch bytes",
    )?;
    let mut summaries = Vec::new();
    allocator.vector_exact(&mut summaries, count, "combined layout summaries")?;
    summaries.resize(count, ContainmentSummary::default());
    let mut state = Vec::new();
    allocator.vector_exact(&mut state, count, "combined layout visit states")?;
    state.resize(count, 0u8);
    let mut stack = [None; MAX_CONTAINMENT_DEPTH];
    for root in 0..count {
        if state[root] & 2 != 0 {
            continue;
        }
        state[root] = 1;
        stack[0] = Some((root, 0usize));
        let mut length = 1;
        while length != 0 {
            let (node, next) = stack[length - 1].expect("active combined layout frame");
            let member_count = if node < records.len() {
                records[node].fields.len()
            } else {
                enums[node - records.len()].variants.len()
            };
            if next < member_count {
                stack[length - 1] = Some((node, next + 1));
                if let Some(ValueTy::Owned(
                    aggregate @ (AggregateTy::Record(_) | AggregateTy::Enum(_)),
                )) = child(records, enums, node, next)?
                {
                    let nested = ordinal(aggregate, records.len(), enums.len())?;
                    match state[nested] {
                        1 => {
                            let record = stack[..length]
                                .iter()
                                .flatten()
                                .find_map(|(id, _)| (*id < records.len()).then_some(RecordId(*id)));
                            return Err(record.map_or(
                                DeclarationError::TypeMismatch,
                                DeclarationError::ContainmentCycle,
                            ));
                        }
                        0 => {
                            if length == stack.len() {
                                return Err(DeclarationError::ResourceLimit(
                                    "combined containment depth",
                                ));
                            }
                            state[nested] = 1;
                            stack[length] = Some((nested, 0));
                            length += 1;
                        }
                        _ => (),
                    }
                }
                continue;
            }
            let mut width = if node < records.len() { 0usize } else { 1usize };
            let mut depth = 1usize;
            let mut packed = 0usize;
            let mut record_layout = LayoutCursor::default();
            let mut payload_size = 4usize;
            let mut payload_align = 4usize;
            let mut enum_nesting = false;
            for member in 0..member_count {
                let Some(value) = child(records, enums, node, member)? else {
                    continue;
                };
                if let ValueTy::Owned(aggregate @ (AggregateTy::Record(_) | AggregateTy::Enum(_))) =
                    value
                {
                    let nested = ordinal(aggregate, records.len(), enums.len())?;
                    enum_nesting |= state[nested] & 4 != 0
                        || (node < records.len() && matches!(aggregate, AggregateTy::Enum(_)));
                }
                let child = value_summary(
                    value,
                    &summaries[..records.len()],
                    &summaries[records.len()..],
                )?;
                depth = depth.max(
                    child
                        .depth()
                        .checked_add(1)
                        .ok_or(DeclarationError::LayoutOverflow)?,
                );
                if depth > MAX_CONTAINMENT_DEPTH {
                    return Err(DeclarationError::ResourceLimit(
                        "combined containment depth",
                    ));
                }
                if node < records.len() {
                    record_layout.push(child.layout)?;
                    packed = limited_add(
                        packed,
                        child.packed_bytes(),
                        limits.layout_bytes,
                        "packed record bytes",
                    )?;
                    width = limited_add(
                        width,
                        child.width,
                        limits.layout_bytes,
                        "record expanded width",
                    )?;
                } else {
                    payload_size = payload_size.max(child.layout.size);
                    payload_align = payload_align.max(child.layout.align);
                    width = width.max(child.width);
                }
            }
            let layout = if node < records.len() {
                width = width.max(1);
                record_layout.finish()?
            } else {
                width = limited_add(width, 1, limits.layout_bytes, "enum expanded width")?;
                let end = payload_size
                    .checked_add(align_up(4, payload_align)?)
                    .ok_or(DeclarationError::LayoutOverflow)?;
                Layout {
                    size: align_up(end, payload_align)?,
                    align: payload_align,
                }
            };
            limited_add(
                0,
                layout.size,
                limits.layout_bytes,
                "declaration layout bytes",
            )?;
            summaries[node] = ContainmentSummary {
                layout,
                width,
                depth_and_packed: ContainmentSummary::pack(
                    depth,
                    if node < records.len() {
                        packed.max(1)
                    } else {
                        layout.size
                    },
                )?,
            };
            state[node] = 2 | (u8::from(enum_nesting) << 2);
            stack[length - 1] = None;
            length -= 1;
        }
    }
    let mut total = 0usize;
    for summary in &summaries {
        total = limited_add(
            total,
            summary.layout.size,
            limits.layout_bytes,
            "declaration layout bytes",
        )?;
    }
    Ok(JointLayouts {
        summaries,
        shapes: state,
        records: records.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::{record, source};
    use super::*;

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
                    payload: payload.map(ParameterTy::Value),
                    span,
                })
                .collect(),
        }
    }
    fn record_with(id: usize, values: &[ValueTy], span: Span) -> RawRecordDecl {
        RawRecordDecl {
            is_copy: false,
            id: RecordId(id),
            span,
            fields: values
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
        }
    }
    #[test]
    fn joint_layouts_follow_alternating_nominal_edges_and_payload_alignment() {
        let (_, span) = source();
        let records = [
            record(0, &[hir::Ty::Bool], span),
            record_with(1, &[ValueTy::Owned(AggregateTy::Enum(EnumId(0)))], span),
            record_with(
                2,
                &[
                    ValueTy::Scalar(hir::Ty::U8),
                    ValueTy::Owned(AggregateTy::Enum(EnumId(0))),
                    ValueTy::Scalar(hir::Ty::U128),
                ],
                span,
            ),
        ];
        let enums = [
            enumeration(
                0,
                &[None, Some(ValueTy::Owned(AggregateTy::Record(RecordId(0))))],
                span,
            ),
            enumeration(
                1,
                &[
                    None,
                    Some(ValueTy::Owned(AggregateTy::Record(RecordId(2)))),
                    Some(ValueTy::Scalar(hir::Ty::U128)),
                ],
                span,
            ),
        ];
        let layouts = layouts(
            &records,
            &enums,
            0,
            Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        assert_eq!(layouts.records()[0].layout, Layout { size: 1, align: 1 });
        assert_eq!(layouts.records()[1].layout, Layout { size: 8, align: 4 });
        assert_eq!(
            layouts.records()[2].layout,
            Layout {
                size: 32,
                align: 16
            }
        );
        assert_eq!(layouts.enums()[0].layout, Layout { size: 8, align: 4 });
        assert_eq!(
            layouts.enums()[1].layout,
            Layout {
                size: 48,
                align: 16
            }
        );
        assert_eq!(
            (layouts.records()[2].width, layouts.enums()[1].width),
            (4, 5)
        );
        assert_eq!(
            (layouts.records()[2].depth(), layouts.enums()[1].depth()),
            (3, 4)
        );
    }
    #[test]
    fn joint_layouts_reject_mixed_cycles_and_invalid_enum_targets() {
        let (_, span) = source();
        let records = [record_with(
            0,
            &[ValueTy::Owned(AggregateTy::Enum(EnumId(0)))],
            span,
        )];
        let enums = [enumeration(
            0,
            &[Some(ValueTy::Owned(AggregateTy::Record(RecordId(0))))],
            span,
        )];
        assert!(matches!(
            layouts(
                &records,
                &enums,
                0,
                Limits::DEFAULT,
                &mut Allocator::default()
            ),
            Err(DeclarationError::ContainmentCycle(RecordId(0)))
        ));
        assert!(matches!(
            layouts(&records, &[], 0, Limits::DEFAULT, &mut Allocator::default()),
            Err(DeclarationError::InvalidEnumId(EnumId(0)))
        ));
    }
    #[test]
    fn joint_layout_depth_counts_both_nominal_families() {
        let (_, span) = source();
        for extra in [false, true] {
            let mut records = (0..32)
                .map(|id| record_with(id, &[ValueTy::Owned(AggregateTy::Enum(EnumId(id)))], span))
                .collect::<Vec<_>>();
            let enums = (0..32)
                .map(|id| {
                    enumeration(
                        id,
                        &[Some(if id < 31 || extra {
                            ValueTy::Owned(AggregateTy::Record(RecordId(id + 1)))
                        } else {
                            ValueTy::Scalar(hir::Ty::I32)
                        })],
                        span,
                    )
                })
                .collect::<Vec<_>>();
            if extra {
                records.push(record(32, &[hir::Ty::I32], span));
            }
            let result = layouts(
                &records,
                &enums,
                0,
                Limits::DEFAULT,
                &mut Allocator::default(),
            );
            if extra {
                assert!(matches!(
                    result,
                    Err(DeclarationError::ResourceLimit(
                        "combined containment depth"
                    ))
                ));
            } else {
                assert_eq!(result.unwrap().records()[0].depth(), 64);
            }
        }
    }
    #[test]
    fn joint_layout_summary_reservations_are_fallible_and_prepaid() {
        let (_, span) = source();
        let records = [record(0, &[hir::Ty::I32], span)];
        for failure in [1, 2] {
            let mut allocator = Allocator {
                fail_at: Some(failure),
                ..Default::default()
            };
            assert!(matches!(
                layouts(&records, &[], 0, Limits::DEFAULT, &mut allocator),
                Err(DeclarationError::Allocation)
            ));
        }
        let mut allocator = Allocator::default();
        assert!(matches!(
            layouts(
                &records,
                &[],
                Limits::DEFAULT.table_bytes,
                Limits::DEFAULT,
                &mut allocator
            ),
            Err(DeclarationError::ResourceLimit(
                "combined layout scratch bytes"
            ))
        ));
        assert_eq!(allocator.attempts, 0);
    }

    #[test]
    fn checked_tables_cache_enum_record_nesting_without_changing_type_identity() {
        let (sources, span) = source();
        let records = [
            record(0, &[hir::Ty::Bool], span),
            record_with(1, &[ValueTy::Owned(AggregateTy::Enum(EnumId(0)))], span),
        ];
        let enums = [
            enumeration(
                0,
                &[None, Some(ValueTy::Owned(AggregateTy::Record(RecordId(0))))],
                span,
            ),
            enumeration(
                1,
                &[Some(ValueTy::Owned(AggregateTy::Record(RecordId(1))))],
                span,
            ),
        ];
        let declarations = Declarations::check_combined(&records, &enums, &sources).unwrap();
        for (aggregate, expected) in [
            (AggregateTy::Record(RecordId(0)), false),
            (AggregateTy::Record(RecordId(1)), true),
            (AggregateTy::Enum(EnumId(0)), false),
            (AggregateTy::Enum(EnumId(1)), true),
        ] {
            assert_eq!(
                declarations.has_enum_record_nesting(aggregate).unwrap(),
                expected
            );
            assert!(!declarations.contains_resources(aggregate).unwrap());
        }
        assert_eq!(
            declarations
                .record(RecordId(1))
                .unwrap()
                .owned_field_count(),
            1
        );
        assert!(declarations
            .leaves(AggregateTy::Record(RecordId(1)))
            .is_err());
        assert_eq!(
            declarations
                .enums()
                .enumeration(EnumId(1))
                .unwrap()
                .layout(),
            Layout { size: 12, align: 4 }
        );
        assert_eq!(declarations.usage().layout_bytes, 29);
    }

    #[test]
    fn joint_record_summary_word_keeps_independent_count_and_nesting_bits() {
        let (_, span) = source();
        for width in [1usize, 1 << 20] {
            for owned in [0usize, 1024] {
                for tagged in [false, true] {
                    let record = RecordDecl {
                        is_copy: false,
                        packed_bytes: 0,
                        id: RecordId(0),
                        span,
                        field_start: 0,
                        field_end: 0,
                        layout: Layout { size: 1, align: 1 },
                        width_and_owned_fields: (width - 1)
                            | (owned << RecordDecl::WIDTH_BITS)
                            | (usize::from(tagged) << 31),
                    };
                    assert_eq!(record.width(), width);
                    assert_eq!(record.owned_field_count(), owned);
                    assert_eq!(record.has_enum_record_nesting(), tagged);
                }
            }
        }
    }
}
