//! Allocation-free traversal of resource leaves from checked declarations.
//! No raw layout, pointer carrier, or producer-provided drop plan is accepted.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend::oir) enum ResourceKind {
    ByteBuf,
    File,
    Vector(hir::Ty),
    RecordVector(u32),
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

type WalkStack = [Option<(ValueTy, usize, usize)>; MAX_CONTAINMENT_DEPTH + 1];

/// Complete synchronous factory/iterator transports. This is a conservative
/// sum of sequential roles, shared by validation, transfer, and release. It is
/// a logical request envelope, not a bound on native stack use or process RSS.
#[allow(dead_code)]
struct WalkCarriers<F> {
    factory_stack: WalkStack,
    factory_reader: F,
    factory_aggregate: AggregateTy,
    constructed: ResourceLeaves<'static, F>,
    factory_result: Result<ResourceLeaves<'static, F>, DeclarationError>,
    caller_walker: ResourceLeaves<'static, F>,
    into_iterator: ResourceLeaves<'static, F>,
    iterator_receiver: *mut ResourceLeaves<'static, F>,
    advance_return: Result<Option<ResourceLeaf>, DeclarationError>,
    next_return: Option<Result<ResourceLeaf, DeclarationError>>,
    caller_option: Option<Result<ResourceLeaf, DeclarationError>>,
    caller_result: Result<ResourceLeaf, DeclarationError>,
    leaf: ResourceLeaf,
    kind: ResourceKind,
    position: usize,
    selected_stack_entry: (ValueTy, usize, usize),
    fields_return: Result<&'static [FieldDecl], DeclarationError>,
    field_option: Option<&'static FieldDecl>,
    selected_field: &'static FieldDecl,
    variant_return: Result<&'static enums::VariantDecl, EnumDeclarationError>,
    selected_variant: &'static enums::VariantDecl,
    checked_offsets: [Option<usize>; 2],
    offset_returns: [Result<usize, DeclarationError>; 2],
    selected_offsets: [usize; 2],
    push_return: Result<(), DeclarationError>,
    tag_input: usize,
    tag_return: Result<usize, DeclarationError>,
    selected_tag: usize,
    optional_payload: Option<ValueTy>,
    selected_payload: ValueTy,
}

pub(in crate::frontend::oir) fn resource_walk_control_bytes<F>(_: &F) -> usize {
    size_of::<WalkCarriers<F>>()
}

pub(in crate::frontend::oir) fn enumerated_resource_walk_bytes<F>(_: &F) -> usize
where
    F: FnMut(usize) -> Result<usize, DeclarationError>,
{
    // Enumerate embeds the entire walker, rather than only an index word.
    size_of::<std::iter::Enumerate<ResourceLeaves<'static, F>>>()
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Presence {
    Unseen,
    Visiting,
    Absent,
    Present,
}

fn ordinal(declarations: &Declarations, aggregate: AggregateTy) -> Result<usize, DeclarationError> {
    declarations.check_aggregate_type(aggregate)?;
    match aggregate {
        AggregateTy::Record(record) => Ok(record.0),
        AggregateTy::Enum(enumeration) => declarations
            .records
            .len()
            .checked_add(enumeration.0)
            .ok_or(DeclarationError::LayoutOverflow),
        _ => Err(DeclarationError::TypeMismatch),
    }
}

#[derive(Clone, Copy)]
struct PresenceFrame {
    aggregate: AggregateTy,
    next: usize,
    contains: bool,
}

/// One summary per checked nominal declaration. Shared suffixes are evaluated
/// once, with no expanded alternative tree. This allocation exists only when
/// the independently admitted declarations contain a direct resource leaf.
pub(super) fn summaries(
    declarations: &Declarations,
    byte_limit: usize,
    allocator: &mut Allocator,
) -> Result<(Vec<Presence>, usize), DeclarationError> {
    let count = declarations
        .records
        .len()
        .checked_add(declarations.enums.enums().len())
        .ok_or(DeclarationError::LayoutOverflow)?;
    let base = declarations.storage.table_bytes as usize;
    limited_add(
        base,
        count,
        byte_limit,
        "resource containment summary bytes",
    )?;
    let mut states = Vec::new();
    if count != 0 {
        allocator.vector_exact(&mut states, count, "resource containment summaries")?;
    }
    let table_bytes = limited_add(
        base,
        states.capacity(),
        byte_limit,
        "resource containment summary bytes",
    )?;
    states.resize(count, Presence::Unseen);
    for root in 0..count {
        if states[root] != Presence::Unseen {
            continue;
        }
        let aggregate = if root < declarations.records.len() {
            AggregateTy::Record(RecordId(root))
        } else {
            AggregateTy::Enum(EnumId(root - declarations.records.len()))
        };
        let mut stack = [None; MAX_CONTAINMENT_DEPTH + 1];
        stack[0] = Some(PresenceFrame {
            aggregate,
            next: 0,
            contains: false,
        });
        states[root] = Presence::Visiting;
        let mut length = 1;
        while length != 0 {
            let frame = stack[length - 1].expect("bounded presence stack");
            let row = ordinal(declarations, frame.aggregate)?;
            let child = match frame.aggregate {
                AggregateTy::Record(record) => declarations
                    .fields(record)?
                    .get(frame.next)
                    .map(FieldDecl::value_ty),
                AggregateTy::Enum(enumeration) => declarations
                    .enums
                    .variants(enumeration)?
                    .get(frame.next)
                    .map(|variant| {
                        variant
                            .value_payload()
                            .unwrap_or(ValueTy::Scalar(hir::Ty::Unit))
                    }),
                _ => return Err(DeclarationError::TypeMismatch),
            };
            match child {
                None => {
                    states[row] = if frame.contains {
                        Presence::Present
                    } else {
                        Presence::Absent
                    };
                    length -= 1;
                }
                Some(ValueTy::Owned(child @ (AggregateTy::Record(_) | AggregateTy::Enum(_)))) => {
                    let child_row = ordinal(declarations, child)?;
                    match states[child_row] {
                        Presence::Present | Presence::Absent => {
                            let top = stack[length - 1].as_mut().unwrap();
                            top.contains |= states[child_row] == Presence::Present;
                            top.next += 1;
                        }
                        Presence::Unseen => {
                            if length == stack.len() {
                                return Err(DeclarationError::ResourceLimit(
                                    "resource containment depth",
                                ));
                            }
                            states[child_row] = Presence::Visiting;
                            stack[length] = Some(PresenceFrame {
                                aggregate: child,
                                next: 0,
                                contains: false,
                            });
                            length += 1;
                        }
                        Presence::Visiting => return Err(DeclarationError::TypeMismatch),
                    }
                }
                Some(ty) => {
                    let top = stack[length - 1].as_mut().unwrap();
                    top.contains |= matches!(
                        ty,
                        ValueTy::Owned(
                            AggregateTy::ByteBuf
                                | AggregateTy::File
                                | AggregateTy::Vector(_)
                                | AggregateTy::RecordVector(_)
                        )
                    );
                    top.next += 1;
                }
            }
        }
    }
    Ok((states, table_bytes))
}

impl Declarations {
    /// Constant-time lookup from independently checked, memoized containment.
    pub(in crate::frontend::oir) fn contains_resources(
        &self,
        aggregate: AggregateTy,
    ) -> Result<bool, DeclarationError> {
        self.check_aggregate_type(aggregate)?;
        match aggregate {
            AggregateTy::ByteBuf
            | AggregateTy::File
            | AggregateTy::Vector(_)
            | AggregateTy::RecordVector(_) => return Ok(true),
            AggregateTy::FixedArray(_) => return Ok(false),
            _ => {}
        }
        if self.storage.resources.is_empty() {
            return Ok(false);
        }
        match self.storage.resources.get(ordinal(self, aggregate)?) {
            Some(Presence::Present) => Ok(true),
            Some(Presence::Absent) => Ok(false),
            _ => Err(DeclarationError::TypeMismatch),
        }
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

    /// Inspect each direct checked field/payload once. Nested vectors must have
    /// a direct declaration somewhere in these already bounded tables.
    pub(in crate::frontend::oir) fn has_record_vector_declarations(&self) -> bool {
        self.fields
            .iter()
            .any(|field| matches!(field.ty, ValueTy::Owned(AggregateTy::RecordVector(_))))
            || self.enums.enums().iter().any(|enumeration| {
                self.enums
                    .variants(enumeration.id())
                    .expect("checked enum")
                    .iter()
                    .any(|variant| {
                        matches!(
                            variant.value_payload(),
                            Some(ValueTy::Owned(AggregateTy::RecordVector(_)))
                        )
                    })
            })
    }
    pub(in crate::frontend::oir) fn has_vector_declarations(&self) -> bool {
        self.fields.iter().any(|field| {
            matches!(
                field.ty,
                ValueTy::Owned(AggregateTy::Vector(_) | AggregateTy::RecordVector(_))
            )
        }) || self.enums.enums().iter().any(|enumeration| {
            self.enums
                .variants(enumeration.id())
                .expect("checked enumeration")
                .iter()
                .any(|variant| {
                    matches!(
                        variant.value_payload(),
                        Some(ValueTy::Owned(
                            AggregateTy::Vector(_) | AggregateTy::RecordVector(_)
                        ))
                    )
                })
        })
    }

    /// The cache is populated only after a direct resource declaration is found.
    pub(in crate::frontend::oir) fn has_resource_declarations(&self) -> bool {
        !self.storage.resources.is_empty()
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
                ValueTy::Owned(AggregateTy::File) => {
                    self.length -= 1;
                    return Ok(Some(ResourceLeaf {
                        offset,
                        kind: ResourceKind::File,
                    }));
                }
                ValueTy::Owned(AggregateTy::ByteBuf) => {
                    self.length -= 1;
                    return Ok(Some(ResourceLeaf {
                        offset,
                        kind: ResourceKind::ByteBuf,
                    }));
                }
                ValueTy::Owned(AggregateTy::RecordVector(record)) => {
                    self.length -= 1;
                    return Ok(Some(ResourceLeaf {
                        offset,
                        kind: ResourceKind::RecordVector(
                            u32::try_from(record.0)
                                .map_err(|_| DeclarationError::LayoutOverflow)?,
                        ),
                    }));
                }
                ValueTy::Owned(AggregateTy::Vector(element)) => {
                    self.length -= 1;
                    return Ok(Some(ResourceLeaf {
                        offset,
                        kind: ResourceKind::Vector(element),
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
    fn vector_resource_walk_keeps_element_identity_through_active_enum_payloads() {
        let (sources, span) = source();
        let mut holder = record(0, &[hir::Ty::I32, hir::Ty::Unit, hir::Ty::Unit], span);
        holder.fields[1].ty = ParameterTy::Value(owned(AggregateTy::Vector(hir::Ty::I128)));
        holder.fields[2].ty = ParameterTy::Value(owned(AggregateTy::ByteBuf));
        let alternatives = [enumeration(
            0,
            &[
                None,
                Some(owned(AggregateTy::Vector(hir::Ty::U8))),
                Some(owned(AggregateTy::Record(RecordId(0)))),
            ],
            span,
        )];
        let d = Declarations::check_combined(&[holder], &alternatives, &sources).unwrap();
        let collect = |tag| {
            d.resource_leaves(AggregateTy::Enum(EnumId(0)), |offset| {
                assert_eq!(offset, 0);
                Ok(tag)
            })
            .unwrap()
            .map(|leaf| leaf.map(|leaf| (leaf.offset, leaf.kind)))
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
        };
        assert!(collect(0).is_empty());
        assert_eq!(collect(1), [(8, ResourceKind::Vector(hir::Ty::U8))]);
        assert_eq!(
            collect(2),
            [
                (16, ResourceKind::Vector(hir::Ty::I128)),
                (32, ResourceKind::ByteBuf),
            ]
        );
        assert!(d
            .contains_resources(AggregateTy::Record(RecordId(0)))
            .unwrap());
        assert!(d.contains_resources(AggregateTy::Enum(EnumId(0))).unwrap());
        assert_eq!(
            d.leaves(AggregateTy::Record(RecordId(0)))
                .unwrap()
                .map(|leaf| (leaf.offset, leaf.ty))
                .collect::<Vec<_>>(),
            [(0, hir::Ty::I32)]
        );
    }

    #[test]
    fn vector_slice_authority_cannot_change_element_type_or_become_a_byte_buffer_owner() {
        let vector = BorrowedTy::Exact(AggregateTy::Vector(hir::Ty::U8));
        assert!(BorrowedTy::ScalarSlice(hir::Ty::U8).accepts(vector));
        assert!(!BorrowedTy::ScalarSlice(hir::Ty::I8).accepts(vector));
        assert!(!BorrowedTy::ScalarSlice(hir::Ty::I128).accepts(vector));
        assert!(!BorrowedTy::Exact(AggregateTy::ByteBuf).accepts(vector));
        assert!(!vector.accepts(BorrowedTy::ScalarSlice(hir::Ty::U8)));
        assert!(!vector.accepts(BorrowedTy::Exact(AggregateTy::ByteBuf)));
        let wide = BorrowedTy::Exact(AggregateTy::Vector(hir::Ty::I128));
        assert!(BorrowedTy::ScalarSlice(hir::Ty::I128).accepts(wide));
        assert!(!BorrowedTy::ScalarSlice(hir::Ty::U128).accepts(wide));
    }

    #[test]
    fn vector_enum_payload_retains_element_identity_when_another_variant_requires_alignment_16() {
        let (sources, span) = source();
        let alternatives = [enumeration(
            0,
            &[
                Some(owned(AggregateTy::Vector(hir::Ty::F64))),
                Some(ValueTy::Scalar(hir::Ty::I128)),
            ],
            span,
        )];
        let d = Declarations::check_combined(&[], &alternatives, &sources).unwrap();
        let variant = d
            .enums
            .variant(
                EnumId(0),
                VariantId {
                    enumeration: EnumId(0),
                    index: 0,
                },
            )
            .unwrap();
        assert_eq!(
            variant.value_payload(),
            Some(owned(AggregateTy::Vector(hir::Ty::F64)))
        );
        assert_eq!(variant.payload_offset(), Some(16));
        let leaves = d
            .resource_leaves(AggregateTy::Enum(EnumId(0)), |_| Ok(0))
            .unwrap()
            .map(|leaf| leaf.map(|leaf| (leaf.offset, leaf.kind)))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(leaves, [(16, ResourceKind::Vector(hir::Ty::F64))]);
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
    fn bytebuf_containment_accepts_declared_enums_and_keeps_reference_fields_closed() {
        let (sources, span) = source();
        let mut valid = record(0, &[hir::Ty::Unit], span);
        valid.fields[0].ty = ParameterTy::Value(owned(AggregateTy::Enum(EnumId(0))));
        let enums = [enumeration(0, &[None], span)];
        let checked = Declarations::check_combined(&[valid], &enums, &sources).unwrap();
        assert!(checked
            .has_enum_record_nesting(AggregateTy::Record(RecordId(0)))
            .unwrap());
        assert!(!checked
            .contains_resources(AggregateTy::Record(RecordId(0)))
            .unwrap());
        for ty in [
            ParameterTy::Value(owned(AggregateTy::Enum(EnumId(1)))),
            ParameterTy::Reference {
                referent: BorrowedTy::Exact(AggregateTy::ByteBuf),
                kind: BorrowKind::Shared,
            },
        ] {
            let mut raw = record(0, &[hir::Ty::Unit], span);
            raw.fields[0].ty = ty;
            let enums = [enumeration(0, &[None], span)];
            assert!(Declarations::check_combined(&[raw], &enums, &sources).is_err());
        }
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

    #[test]
    fn resource_presence_shared_enum_suffixes_are_memoized_in_both_orders() {
        for forward in [false, true] {
            for leaf_resource in [false, true] {
                for unused_resource in [false, true] {
                    let (sources, span) = source();
                    let mut extra = record(0, &[hir::Ty::Unit], span);
                    if unused_resource {
                        extra.fields[0].ty = ParameterTy::Value(owned(AggregateTy::ByteBuf));
                    }
                    let depth = 50;
                    let raw: Vec<_> = (0..=depth)
                        .map(|id| {
                            let leaf = if forward { id == depth } else { id == 0 };
                            let payload = if leaf {
                                if leaf_resource {
                                    owned(AggregateTy::ByteBuf)
                                } else {
                                    ValueTy::Scalar(hir::Ty::I32)
                                }
                            } else {
                                owned(AggregateTy::Enum(EnumId(if forward {
                                    id + 1
                                } else {
                                    id - 1
                                })))
                            };
                            enumeration(id, &[Some(payload), Some(payload), None], span)
                        })
                        .collect();
                    let checked = Declarations::check_combined(&[extra], &raw, &sources).unwrap();
                    assert_eq!(
                        checked.has_resource_declarations(),
                        leaf_resource || unused_resource
                    );
                    assert_eq!(
                        checked.storage.resources.len(),
                        if leaf_resource || unused_resource {
                            depth + 2
                        } else {
                            0
                        }
                    );
                    for _ in 0..100 {
                        for id in 0..=depth {
                            assert_eq!(
                                checked.contains_resources(AggregateTy::Enum(EnumId(id))),
                                Ok(leaf_resource)
                            );
                        }
                        assert_eq!(
                            checked.contains_resources(AggregateTy::Record(RecordId(0))),
                            Ok(unused_resource)
                        );
                    }
                    assert!(checked
                        .contains_resources(AggregateTy::Enum(EnumId(depth + 1)))
                        .is_err());
                }
            }
        }
    }

    #[test]
    fn resource_cache_capacity_and_each_allocation_failure_are_checked() {
        let (sources, span) = source();
        let mut raw = record(0, &[hir::Ty::Unit], span);
        raw.fields[0].ty = ParameterTy::Value(owned(AggregateTy::ByteBuf));
        let mut allocator = Allocator::default();
        let checked = Declarations::check_combined_with_limits(
            std::slice::from_ref(&raw),
            &[],
            &sources,
            Limits::DEFAULT,
            &mut allocator,
        )
        .unwrap();
        let required = checked.usage().table_bytes;
        let events: Vec<_> = allocator
            .trace
            .iter()
            .filter(|event| event.kind == "resource containment summaries")
            .collect();
        assert_eq!(events.len(), 1);
        assert_eq!(
            (events[0].length, events[0].element_bytes, events[0].success),
            (1, 1, true)
        );
        for table_bytes in [required, required - 1] {
            let mut allocation = Allocator::default();
            let result = Declarations::check_combined_with_limits(
                std::slice::from_ref(&raw),
                &[],
                &sources,
                Limits {
                    table_bytes,
                    ..Limits::DEFAULT
                },
                &mut allocation,
            );
            if table_bytes == required {
                assert!(result.is_ok());
            } else {
                assert!(matches!(
                    result,
                    Err(DeclarationError::ResourceLimit(
                        "resource containment summary bytes"
                    ))
                ));
                assert_eq!(allocation.attempts, 0);
            }
        }
        for attempt in 1..=allocator.attempts {
            let mut allocation = Allocator {
                fail_at: Some(attempt),
                ..Allocator::default()
            };
            let result = Declarations::check_combined_with_limits(
                std::slice::from_ref(&raw),
                &[],
                &sources,
                Limits::DEFAULT,
                &mut allocation,
            );
            assert!(
                matches!(result, Err(DeclarationError::Allocation)),
                "attempt={attempt} {result:?}"
            );
            assert_eq!(allocation.attempts, attempt);
        }
    }
}
