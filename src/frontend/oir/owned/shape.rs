use super::*;
use crate::frontend::builtin_catalog::{BuiltinEnum, BuiltinFunction};
use budget::{add, filled, reserve};
const NONE: usize = usize::MAX;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Site {
    pub block: usize,
    pub statement: usize,
}
#[derive(Clone, Default)]
pub(super) struct OwnerSites {
    live: Option<Site>,
    initialize: Option<Site>,
}
#[derive(Clone, Default)]
pub(super) struct CallSites {
    pub open: Option<Site>,
    pub invoke: Option<usize>,
    pub start: usize,
    pub enter: usize,
    pub exit: usize,
}
pub(super) struct Shape {
    pub calls: Vec<CallSites>,
    acquisitions: Vec<Option<Site>>,
}
impl Shape {
    pub(super) fn acquisition_cause(
        &self,
        f: &RawOwnedFunction,
        loan: LoanId,
    ) -> Result<Span, OwnedFailure> {
        let instruction = self.acquisitions.get(loan.0).and_then(|site| *site)
            .and_then(|site| f.blocks.get(site.block)?.statements.get(site.statement))
            .filter(|i| matches!(i.kind, OwnedInstruction::PrepareBorrow { loan: actual, .. } | OwnedInstruction::PrepareBorrowRange { loan: actual, .. } | OwnedInstruction::AcquireLocalBorrow(actual) if actual == loan))
            .ok_or_else(|| bad(Malformed::CanonicalSite, f.span))?;
        Ok(instruction.cause_span())
    }
}
fn bad(kind: Malformed, s: Span) -> OwnedFailure {
    OwnedFailure::malformed(kind, s)
}
pub(super) fn span(sources: &SourceMap, s: Span) -> Result<(), OwnedFailure> {
    if sources.is_valid_span(s) {
        Ok(())
    } else {
        Err(bad(Malformed::Span, s))
    }
}
pub(super) fn owner(
    f: &RawOwnedFunction,
    id: OwnerPlaceId,
    s: Span,
) -> Result<&OwnerDecl, OwnedFailure> {
    f.owners.get(id.0).ok_or_else(|| bad(Malformed::Id, s))
}
pub(super) fn reference(
    f: &RawOwnedFunction,
    id: ReferenceParamId,
    s: Span,
) -> Result<&ReferenceDecl, OwnedFailure> {
    f.references.get(id.0).ok_or_else(|| bad(Malformed::Id, s))
}
pub(super) fn call(
    f: &RawOwnedFunction,
    id: CallSiteId,
    s: Span,
) -> Result<&CallDecl, OwnedFailure> {
    f.calls.get(id.0).ok_or_else(|| bad(Malformed::Id, s))
}
pub(super) fn loan(f: &RawOwnedFunction, id: LoanId, s: Span) -> Result<&LoanDecl, OwnedFailure> {
    f.loans.get(id.0).ok_or_else(|| bad(Malformed::Id, s))
}
fn local(f: &RawOwnedFunction, id: LocalId, s: Span) -> Result<hir::Ty, OwnedFailure> {
    f.locals
        .get(id.0)
        .map(|x| x.ty)
        .ok_or_else(|| bad(Malformed::Id, s))
}
fn operand(f: &RawOwnedFunction, v: Operand, sources: &SourceMap) -> Result<hir::Ty, OwnedFailure> {
    span(sources, v.span)?;
    local(f, v.local, v.span)
}
fn equal<T: PartialEq>(a: T, b: T, s: Span) -> Result<(), OwnedFailure> {
    if a == b {
        Ok(())
    } else {
        Err(bad(Malformed::Type, s))
    }
}
/// Equality validates actual then expected, with the historical site's error
/// category and origin. Resource failures must never become malformed types.
fn same_aggregate(
    d: &Declarations,
    actual: AggregateTy,
    expected: AggregateTy,
    kind: Malformed,
    s: Span,
) -> Result<(), OwnedFailure> {
    d.same_aggregate_type(actual, expected)
        .map_err(|error| match error {
            DeclarationError::InvalidRecordId(_) | DeclarationError::TypeMismatch => bad(kind, s),
            other => other.into(),
        })
}
fn record(aggregate: AggregateTy, s: Span) -> Result<RecordId, OwnedFailure> {
    match aggregate {
        AggregateTy::Record(record) => Ok(record),
        AggregateTy::FixedArray(_)
        | AggregateTy::Enum(_)
        | AggregateTy::ByteBuf
        | AggregateTy::Vector(_)
        | AggregateTy::File => Err(bad(Malformed::Type, s)),
    }
}
fn array(aggregate: AggregateTy, s: Span) -> Result<FixedArrayTy, OwnedFailure> {
    match aggregate {
        AggregateTy::FixedArray(array) => Ok(array),
        AggregateTy::Record(_)
        | AggregateTy::Enum(_)
        | AggregateTy::ByteBuf
        | AggregateTy::Vector(_)
        | AggregateTy::File => Err(bad(Malformed::Type, s)),
    }
}
fn borrowed_record(ty: BorrowedTy, s: Span) -> Result<RecordId, OwnedFailure> {
    match ty {
        BorrowedTy::Exact(aggregate) => record(aggregate, s),
        BorrowedTy::ScalarSlice(_) => Err(bad(Malformed::Type, s)),
    }
}
fn array_element(ty: BorrowedTy, s: Span) -> Result<hir::Ty, OwnedFailure> {
    if ty == BorrowedTy::Exact(AggregateTy::ByteBuf) {
        return Err(bad(Malformed::Type, s));
    }
    ty.element().ok_or_else(|| bad(Malformed::Type, s))
}
pub(super) fn base(
    f: &RawOwnedFunction,
    b: AccessBase,
    s: Span,
) -> Result<(BorrowedTy, bool), OwnedFailure> {
    match b {
        AccessBase::Owner(id) => {
            let o = owner(f, id, s)?;
            if matches!(o.kind, OwnerKind::StagedArgument { .. }) {
                return Err(bad(Malformed::OwnerClass, s));
            }
            Ok((
                BorrowedTy::Exact(o.aggregate()),
                matches!(o.kind, OwnerKind::Local { mutable: true }),
            ))
        }
        AccessBase::Parameter(id) => {
            let r = reference(f, id, s)?;
            Ok((r.referent(), r.kind == BorrowKind::Exclusive))
        }
        AccessBase::Loan(id) => {
            let loan = loan(f, id, s)?;
            if loan.region != LoanRegion::Local {
                return Err(bad(Malformed::Binding, s));
            }
            Ok((loan.referent(), loan.kind == BorrowKind::Exclusive))
        }
    }
}
fn ordinary(f: &RawOwnedFunction, id: OwnerPlaceId, s: Span) -> Result<&OwnerDecl, OwnedFailure> {
    let o = owner(f, id, s)?;
    if matches!(o.kind, OwnerKind::StagedArgument { .. }) {
        Err(bad(Malformed::OwnerClass, s))
    } else {
        Ok(o)
    }
}
fn slot<T>(values: &mut [Option<T>], index: usize, value: T, s: Span) -> Result<(), OwnedFailure> {
    let v = values.get_mut(index).ok_or_else(|| bad(Malformed::Id, s))?;
    if v.is_some() {
        return Err(bad(Malformed::CanonicalSite, s));
    }
    *v = Some(value);
    Ok(())
}
pub(super) fn parameter(
    f: &RawOwnedFunction,
    p: ParameterBinding,
    s: Span,
) -> Result<ParameterTy, OwnedFailure> {
    Ok(match p {
        ParameterBinding::Scalar(id) => ParameterTy::Value(ValueTy::Scalar(local(f, id, s)?)),
        ParameterBinding::Owned(id) => {
            ParameterTy::Value(ValueTy::Owned(owner(f, id, s)?.aggregate()))
        }
        ParameterBinding::Reference(id) => {
            let r = reference(f, id, s)?;
            ParameterTy::Reference {
                referent: r.referent(),
                kind: r.kind,
            }
        }
    })
}
pub(super) fn signatures(
    raw: &RawOwnedProgram,
    d: &Declarations,
    sources: &SourceMap,
) -> Result<(), OwnedFailure> {
    // Resource authority comes from the closed canonical inventory, never from
    // a producer's nominal spelling or a resource-bearing declaration alone.
    let resource_authority = raw
        .builtins
        .experimental()
        .contains(crate::frontend::builtin_catalog::experimental::Item::ByteBuf);
    let file_authority = raw
        .builtins
        .experimental()
        .contains(crate::frontend::builtin_catalog::experimental::Item::File);
    let vector_authority = raw
        .builtins
        .contains_vector(crate::frontend::builtin_catalog::vector::Item::Vec);
    let permitted = |value| match value {
        ValueTy::Owned(AggregateTy::ByteBuf) => resource_authority,
        ValueTy::Owned(AggregateTy::File) => file_authority,
        ValueTy::Owned(AggregateTy::Vector(_)) => vector_authority,
        _ => true,
    };
    if d.has_resource_declarations() && !resource_authority && !file_authority && !vector_authority
    {
        return Err(DeclarationError::TypeMismatch.into());
    }
    // A catalog for one resource class cannot authorize a different class in
    // otherwise ordinary nominal declarations. Inspect direct leaves only.
    for record in &raw.records {
        for field in &record.fields {
            if let ParameterTy::Value(value) = field.ty {
                if !permitted(value) {
                    return Err(bad(Malformed::Type, field.span));
                }
            }
        }
    }
    for enumeration in &raw.enums {
        for variant in &enumeration.variants {
            if let Some(ParameterTy::Value(value)) = variant.payload {
                if !permitted(value) {
                    return Err(bad(Malformed::Type, variant.span));
                }
            }
        }
    }
    for (index, f) in raw.functions.iter().enumerate() {
        span(sources, f.span)?;
        if f.id != hir::DefId(index) {
            return Err(bad(Malformed::Id, f.span));
        }
        d.check_value_type(f.result)?;
        if !permitted(f.result) || f.loans.iter().any(|loan| {
            matches!(loan.referent(), BorrowedTy::Exact(aggregate) if !permitted(ValueTy::Owned(aggregate)))
        })
        {
            return Err(bad(Malformed::Type, f.span));
        }
        if f.blocks.is_empty() || f.entry.0 >= f.blocks.len() {
            return Err(bad(Malformed::Id, f.span));
        }
        let mut scalar = 0;
        let (mut owns, mut refs) = (0, 0);
        for (position, p) in f.parameters.iter().enumerate() {
            match p {
                ParameterBinding::Scalar(id) => {
                    if id.0 != scalar {
                        return Err(bad(Malformed::Binding, f.span));
                    }
                    local(f, *id, f.span)?;
                    scalar += 1;
                }
                ParameterBinding::Owned(id) => {
                    let o = owner(f, *id, f.span)?;
                    if o.kind != (OwnerKind::Parameter { position }) {
                        return Err(bad(Malformed::Binding, f.span));
                    }
                    owns += 1;
                }
                ParameterBinding::Reference(id) => {
                    if reference(f, *id, f.span)?.position != position {
                        return Err(bad(Malformed::Binding, f.span));
                    }
                    refs += 1;
                }
            }
        }
        for (i, l) in f.locals.iter().enumerate() {
            span(sources, l.span)?;
            if (i < scalar) != (l.kind == LocalKind::Parameter) {
                return Err(bad(Malformed::Binding, l.span));
            }
        }
        for p in &f.places {
            span(sources, p.span)?;
        }
        let mut parameter_owners = 0;
        for (id, o) in f.owners.iter().enumerate() {
            span(sources, o.span)?;
            d.check_aggregate_type(o.aggregate())?;
            if !permitted(ValueTy::Owned(o.aggregate())) {
                return Err(bad(Malformed::Type, o.span));
            }
            match o.kind {
                OwnerKind::Parameter { position } => {
                    parameter_owners += 1;
                    if !matches!(f.parameters.get(position),Some(ParameterBinding::Owned(p)) if p.0==id)
                    {
                        return Err(bad(Malformed::Binding, o.span));
                    }
                }
                OwnerKind::StagedArgument { call: c, argument } => {
                    if call(f, c, o.span)?.arguments.get(argument)
                        != Some(&ArgumentSlot::Owned(OwnerPlaceId(id)))
                    {
                        return Err(bad(Malformed::Binding, o.span));
                    }
                }
                OwnerKind::CallResult { call: c }
                    if call(f, c, o.span)?.result != CallResult::Owned(OwnerPlaceId(id)) =>
                {
                    return Err(bad(Malformed::Binding, o.span));
                }
                _ => {}
            }
        }
        if owns != parameter_owners || refs != f.references.len() {
            return Err(bad(Malformed::Binding, f.span));
        }
        for (id, r) in f.references.iter().enumerate() {
            span(sources, r.span)?;
            d.check_borrowed_type(r.referent())?;
            if matches!(r.referent(), BorrowedTy::Exact(aggregate) if !permitted(ValueTy::Owned(aggregate)))
            {
                return Err(bad(Malformed::Type, r.span));
            }
            if !matches!(f.parameters.get(r.position),Some(ParameterBinding::Reference(p)) if p.0==id)
            {
                return Err(bad(Malformed::Binding, r.span));
            }
        }
    }
    Ok(())
}
pub(super) fn check(
    f: &RawOwnedFunction,
    raw: &RawOwnedProgram,
    d: &Declarations,
    sources: &SourceMap,
    meter: &mut budget::Meter,
) -> Result<Shape, OwnedFailure> {
    check_with_conversion_authority(f, raw, d, sources, meter, false)
}
pub(super) fn check_with_conversion_authority(
    f: &RawOwnedFunction,
    raw: &RawOwnedProgram,
    d: &Declarations,
    sources: &SourceMap,
    meter: &mut budget::Meter,
    source_associated: bool,
) -> Result<Shape, OwnedFailure> {
    check_matches(f, d, sources, meter)?;
    let mut owners = filled(f.owners.len(), OwnerSites::default())?;
    let mut calls = filled(f.calls.len(), CallSites::default())?;
    let mut total_args = 0;
    for (i, c) in f.calls.iter().enumerate() {
        calls[i].start = total_args;
        total_args = add(total_args, c.arguments.len())?;
    }
    let mut preparations = filled(total_args, None)?;
    let mut acquisitions = filled(f.loans.len(), None)?;
    // Raw malformed constructor lengths are capped before any reservation.
    // Zero-constructor/fieldless functions request no field-seen cells.
    let field_capacity = f
        .blocks
        .iter()
        .flat_map(|b| &b.statements)
        .filter_map(|i| match &i.kind {
            OwnedInstruction::Construct { fields, .. } => Some(fields.len().min(1024)),
            OwnedInstruction::ConstructComposite { fields, .. } => Some(fields.len().min(1024)),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let mut field_seen = filled(field_capacity, 0u8)?;
    for (id, c) in f.calls.iter().enumerate() {
        span(sources, c.span)?;
        let target = raw
            .functions
            .get(c.target.0)
            .ok_or_else(|| bad(Malformed::Id, c.span))?;
        if c.arguments.len() != target.parameters.len() {
            return Err(bad(Malformed::Binding, c.span));
        }
        if let Some((parent, argument)) = c.parent {
            if parent.0 == id || argument >= call(f, parent, c.span)?.arguments.len() {
                return Err(bad(Malformed::CallParent, c.span));
            }
        }
        for (arg, descriptor) in c.arguments.iter().enumerate() {
            let ty = parameter(target, target.parameters[arg], c.span)?;
            match (descriptor, ty) {
                (ArgumentSlot::Scalar, ParameterTy::Value(ValueTy::Scalar(_))) => {}
                (ArgumentSlot::Owned(o), ParameterTy::Value(ValueTy::Owned(r))) => {
                    let owner = owner(f, *o, c.span)?;
                    same_aggregate(d, owner.aggregate(), r, Malformed::Type, c.span)?;
                    if owner.kind
                        != (OwnerKind::StagedArgument {
                            call: CallSiteId(id),
                            argument: arg,
                        })
                    {
                        return Err(bad(Malformed::Binding, c.span));
                    }
                }
                (
                    ArgumentSlot::Borrow(l),
                    ParameterTy::Reference {
                        referent: record,
                        kind,
                    },
                ) => {
                    let loan = loan(f, *l, c.span)?;
                    if loan.call_binding() != Some((CallSiteId(id), arg)) {
                        return Err(bad(Malformed::Binding, c.span));
                    }
                    d.same_borrowed_type(loan.referent(), record)
                        .map_err(|_| bad(Malformed::Binding, c.span))?;
                    if loan.kind != kind {
                        return Err(bad(Malformed::Binding, c.span));
                    }
                }
                _ => return Err(bad(Malformed::Type, c.span)),
            }
        }
        match (c.result, target.result) {
            (CallResult::Scalar(v), ValueTy::Scalar(t)) => equal(local(f, v, c.span)?, t, c.span)?,
            (CallResult::Owned(v), ValueTy::Owned(r)) => {
                let o = owner(f, v, c.span)?;
                same_aggregate(d, o.aggregate(), r, Malformed::Type, c.span)?;
                if o.kind
                    != (OwnerKind::CallResult {
                        call: CallSiteId(id),
                    })
                {
                    return Err(bad(Malformed::Binding, c.span));
                }
            }
            _ => return Err(bad(Malformed::Type, c.span)),
        }
    }
    for (id, l) in f.loans.iter().enumerate() {
        span(sources, l.span)?;
        if let Some((binding, argument)) = l.call_binding() {
            let c = call(f, binding, l.span)?;
            if c.arguments.get(argument) != Some(&ArgumentSlot::Borrow(LoanId(id))) {
                return Err(bad(Malformed::Binding, l.span));
            }
        } else if !l.projection.is_empty() || !matches!(l.referent(), BorrowedTy::Exact(_)) {
            return Err(bad(Malformed::Type, l.span));
        }
        let mut authority = l.authority;
        let mut depth = 0;
        while let AccessBase::Loan(parent) = authority {
            meter.visit()?;
            depth += 1;
            if depth > MAX_LOCAL_REFERENCE_DEPTH || depth > f.loans.len() {
                return Err(OwnedFailure::resource("local reference ancestry"));
            }
            let parent = loan(f, parent, l.span)?;
            if parent.region != LoanRegion::Local {
                return Err(bad(Malformed::Binding, l.span));
            }
            authority = parent.authority;
        }
        let (record, _) = base(f, l.authority, l.span)?;
        let record = if l.projection.is_empty() {
            record
        } else {
            let BorrowedTy::Exact(root @ AggregateTy::Record(_)) = record else {
                return Err(bad(Malformed::Type, l.span));
            };
            for _ in &l.projection {
                meter.visit()?;
            }
            match d.projection(root, &l.projection)?.0 {
                ValueTy::Owned(AggregateTy::FixedArray(array)) => {
                    if l.referent() != BorrowedTy::ScalarSlice(array.element()) {
                        return Err(bad(Malformed::Type, l.span));
                    }
                    BorrowedTy::Exact(AggregateTy::FixedArray(array))
                }
                ValueTy::Owned(AggregateTy::Vector(element)) => {
                    BorrowedTy::Exact(AggregateTy::Vector(element))
                }
                ValueTy::Owned(AggregateTy::ByteBuf) => BorrowedTy::Exact(AggregateTy::ByteBuf),
                ValueTy::Owned(AggregateTy::File) => BorrowedTy::Exact(AggregateTy::File),
                ValueTy::Owned(record @ AggregateTy::Record(_)) => BorrowedTy::Exact(record),
                _ => return Err(bad(Malformed::Type, l.span)),
            }
        };
        d.check_borrowed_view(record, l.referent())
            .map_err(|_| bad(Malformed::Type, l.span))?;
        if let AccessBase::Owner(o) = l.authority {
            if !matches!(
                owner(f, o, l.span)?.kind,
                OwnerKind::Local { .. } | OwnerKind::Parameter { .. }
            ) {
                return Err(bad(Malformed::OwnerClass, l.span));
            }
        }
    }
    for (bi, b) in f.blocks.iter().enumerate() {
        span(sources, b.span)?;
        if let Some(m) = &b.merge {
            span(sources, m.span)?;
            span(sources, m.operator_span)?;
            equal(local(f, m.destination, m.span)?, hir::Ty::Bool, m.span)?;
            for i in &m.incoming {
                if i.predecessor.0 >= f.blocks.len() {
                    return Err(bad(Malformed::Id, m.span));
                }
                equal(operand(f, i.value, sources)?, hir::Ty::Bool, i.value.span)?;
            }
        }
        for (si, instruction) in b.statements.iter().enumerate() {
            let s = instruction.span;
            span(sources, s)?;
            diagnostic_origins(sources, instruction.diagnostic_origins, meter)?;
            let site = Site {
                block: bi,
                statement: si,
            };
            match &instruction.kind {
                OwnedInstruction::Experimental {
                    operation,
                    destination,
                } => {
                    let expected = raw.fixed_builtin_base().and_then(|base| {
                        base.checked_add(raw.builtins.experimental_function_rank(*operation)?)
                    });
                    if expected != Some(f.id.0) {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    match *destination {
                        CallResult::Owned(destination) => {
                            let o = ordinary(f, destination, s)?;
                            if o.kind != OwnerKind::Temporary
                                || ValueTy::Owned(o.aggregate()) != f.result
                            {
                                return Err(bad(Malformed::Binding, s));
                            }
                            let previous = &mut owners[destination.0].initialize;
                            if previous.is_some() {
                                return Err(bad(Malformed::CanonicalSite, s));
                            }
                            *previous = Some(site);
                        }
                        CallResult::Scalar(destination) => {
                            equal(ValueTy::Scalar(local(f, destination, s)?), f.result, s)?
                        }
                    }
                }
                OwnedInstruction::Vector {
                    operation,
                    destination,
                } => {
                    if raw.vector_operation_claim(f.id) != Some(*operation) {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    match *destination {
                        CallResult::Owned(destination) => {
                            let o = ordinary(f, destination, s)?;
                            if o.kind != OwnerKind::Temporary
                                || ValueTy::Owned(o.aggregate()) != f.result
                            {
                                return Err(bad(Malformed::Binding, s));
                            }
                            let previous = &mut owners[destination.0].initialize;
                            if previous.is_some() {
                                return Err(bad(Malformed::CanonicalSite, s));
                            }
                            *previous = Some(site);
                        }
                        CallResult::Scalar(destination) => {
                            equal(ValueTy::Scalar(local(f, destination, s)?), f.result, s)?;
                        }
                    }
                }
                OwnedInstruction::ReadStdin {
                    buffer,
                    destination,
                }
                | OwnedInstruction::WriteStdout {
                    buffer,
                    destination,
                } => {
                    // Full canonical suffix validation precedes ordinary shape.
                    // Recheck the claimed ordinal here without repeating that
                    // descriptor walk in each ownership pass.
                    let (function_kind, enum_kind, borrow_kind) = match instruction.kind {
                        OwnedInstruction::ReadStdin { .. } => (
                            BuiltinFunction::ReadStdin,
                            BuiltinEnum::ReadStatus,
                            BorrowKind::Exclusive,
                        ),
                        OwnedInstruction::WriteStdout { .. } => (
                            BuiltinFunction::WriteStdout,
                            BuiltinEnum::WriteStatus,
                            BorrowKind::Shared,
                        ),
                        _ => unreachable!("matched builtin operation"),
                    };
                    let expected_function = raw.fixed_builtin_base().and_then(|base| {
                        base.checked_add(raw.builtins.function_rank(function_kind)?)
                    });
                    if expected_function != Some(f.id.0) {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    let expected_enum = raw
                        .enums
                        .len()
                        .checked_sub(raw.builtins.extra_enums())
                        .and_then(|base| base.checked_add(raw.builtins.enum_rank(enum_kind)?))
                        .map(EnumId)
                        .ok_or_else(|| bad(Malformed::Binding, s))?;
                    let reference = reference(f, *buffer, s)?;
                    let o = ordinary(f, *destination, s)?;
                    if reference.kind != borrow_kind
                        || reference.referent() != BorrowedTy::ScalarSlice(hir::Ty::I32)
                        || o.kind != OwnerKind::Temporary
                        || enumeration(o.aggregate(), s)? != expected_enum
                    {
                        return Err(bad(Malformed::Binding, s));
                    }
                    let prev = &mut owners[destination.0].initialize;
                    if prev.is_some() {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    *prev = Some(site);
                }
                OwnedInstruction::ConstructOwnedEnum {
                    destination,
                    variant,
                    payload,
                } => {
                    let o = ordinary(f, *destination, s)?;
                    let source = ordinary(f, *payload, s)?;
                    if !matches!(o.kind, OwnerKind::Local { .. } | OwnerKind::Temporary)
                        || source.kind != OwnerKind::Temporary
                        || destination == payload
                    {
                        return Err(bad(Malformed::OwnerClass, s));
                    }
                    let prev = &mut owners[destination.0].initialize;
                    if prev.is_some() {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    *prev = Some(site);
                    d.enums()
                        .check_value_payload(
                            enumeration(o.aggregate(), s)?,
                            *variant,
                            Some(ValueTy::Owned(source.aggregate())),
                        )
                        .map_err(DeclarationError::from)?;
                }
                OwnedInstruction::ConsumeOwnedVariant {
                    match_id,
                    arm,
                    destination,
                } => {
                    let (descriptor, selected) = match_arm(f, *match_id, *arm, s)?;
                    let s = descriptor.span;
                    if selected.entry != BlockId(bi)
                        || si != 0
                        || b.merge.is_some()
                        || instruction.span != s
                        || descriptor.source == *destination
                        || instruction
                            .diagnostic_origins
                            .is_some_and(|o| o.primary != s || o.cause != s)
                    {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    let target = ordinary(f, *destination, s)?;
                    if target.kind != (OwnerKind::Local { mutable: false }) {
                        return Err(bad(Malformed::Binding, s));
                    }
                    let sites = &mut owners[destination.0];
                    if sites.live.is_some() || sites.initialize.is_some() {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    sites.live = Some(site);
                    sites.initialize = Some(site);
                    d.enums()
                        .check_value_payload(
                            enumeration(owner(f, descriptor.source, s)?.aggregate(), s)?,
                            selected.variant,
                            Some(ValueTy::Owned(target.aggregate())),
                        )
                        .map_err(DeclarationError::from)?;
                }
                OwnedInstruction::ConstructEnum {
                    destination,
                    variant,
                    payload,
                } => {
                    let o = ordinary(f, *destination, s)?;
                    if !matches!(o.kind, OwnerKind::Local { .. } | OwnerKind::Temporary) {
                        return Err(bad(Malformed::OwnerClass, s));
                    }
                    let prev = &mut owners[destination.0].initialize;
                    if prev.is_some() {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    *prev = Some(site);
                    d.enums()
                        .check_payload(
                            enumeration(o.aggregate(), s)?,
                            *variant,
                            payload.map(|v| operand(f, v, sources)).transpose()?,
                        )
                        .map_err(DeclarationError::from)?;
                }
                OwnedInstruction::ConsumeVariant {
                    match_id,
                    arm,
                    destination,
                } => {
                    let (descriptor, selected) = match_arm(f, *match_id, *arm, s)?;
                    let s = descriptor.span;
                    if selected.entry != BlockId(bi)
                        || si != 0
                        || b.merge.is_some()
                        || instruction.span != s
                    {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    // Origins are descriptor-authoritative, even in a raw fixture.
                    if instruction
                        .diagnostic_origins
                        .is_some_and(|o| o.primary != s || o.cause != s)
                    {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    let actual = destination
                        .map(|id| {
                            let l = f.locals.get(id.0).ok_or_else(|| bad(Malformed::Id, s))?;
                            if l.kind != LocalKind::Binding {
                                return Err(bad(Malformed::Binding, s));
                            }
                            Ok(l.ty)
                        })
                        .transpose()?;
                    d.enums()
                        .check_payload(
                            enumeration(owner(f, descriptor.source, s)?.aggregate(), s)?,
                            selected.variant,
                            actual,
                        )
                        .map_err(DeclarationError::from)?;
                }
                OwnedInstruction::Scalar(i) => {
                    if !source_associated
                        && matches!(
                            i,
                            Statement::Assign(Assign {
                                value: Rvalue::CharacterConvert { .. }
                                    | Rvalue::FloatConvert { .. }
                                    | Rvalue::CheckedI32ToU8 { .. }
                                    | Rvalue::U8ToI32 { .. }
                                    | Rvalue::WidenI64 { .. }
                                    | Rvalue::CheckedI64ToI32 { .. }
                                    | Rvalue::CheckedI64ToU8 { .. },
                                ..
                            })
                        )
                    {
                        return Err(OirFailure {
                            kind: FailureKind::UnauthenticatedConversion,
                            stage: "oir-verify",
                            span: None,
                        }
                        .into());
                    }
                    super::super::verify::scalar_statement_shape(&f.locals, &f.places, i, sources)?
                }
                OwnedInstruction::StorageLive(id) => {
                    let o = owner(f, *id, s)?;
                    if !matches!(o.kind, OwnerKind::Local { .. } | OwnerKind::Temporary) {
                        return Err(bad(Malformed::OwnerClass, s));
                    }
                    let prev = &mut owners[id.0].live;
                    if prev.is_some() {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    *prev = Some(site);
                }
                OwnedInstruction::StorageEnd(id) | OwnedInstruction::Discard(id) => {
                    ordinary(f, *id, s)?;
                }
                OwnedInstruction::Construct {
                    destination,
                    fields,
                } => {
                    let o = ordinary(f, *destination, s)?;
                    if !matches!(o.kind, OwnerKind::Local { .. } | OwnerKind::Temporary) {
                        return Err(bad(Malformed::OwnerClass, s));
                    }
                    let prev = &mut owners[destination.0].initialize;
                    if prev.is_some() {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    *prev = Some(site);
                    let declared = d.fields(record(o.aggregate(), s)?)?;
                    if fields.len() != declared.len() {
                        return Err(bad(Malformed::Type, s));
                    }
                    field_seen[..declared.len()].fill(0);
                    for (id, value) in fields {
                        let field = d.field(record(o.aggregate(), s)?, *id)?;
                        if field_seen[id.index] != 0 {
                            return Err(bad(Malformed::Type, s));
                        }
                        field_seen[id.index] = 1;
                        equal(
                            ValueTy::Scalar(operand(f, *value, sources)?),
                            field.value_ty(),
                            value.span,
                        )?;
                    }
                }
                OwnedInstruction::DestructureRecord { source, fields } => {
                    let source_decl = ordinary(f, *source, s)?;
                    let record = record(source_decl.aggregate(), s)?;
                    let expected = d.record(record)?.owned_field_count();
                    equal(fields.len(), expected, s)?;
                    for (position, (field, destination)) in fields.iter().enumerate() {
                        meter.visit()?;
                        let member = d.field(record, *field)?;
                        if source == destination {
                            return Err(bad(Malformed::Binding, s));
                        }
                        for (earlier, _) in &fields[..position] {
                            meter.visit()?;
                            if earlier == field {
                                return Err(bad(Malformed::Binding, s));
                            }
                        }
                        let target = ordinary(f, *destination, s)?;
                        if target.kind != (OwnerKind::Local { mutable: false }) {
                            return Err(bad(Malformed::OwnerClass, s));
                        }
                        d.same_value_type(member.value_ty(), ValueTy::Owned(target.aggregate()))?;
                        let previous = &mut owners[destination.0].initialize;
                        if previous.is_some() {
                            return Err(bad(Malformed::CanonicalSite, s));
                        }
                        *previous = Some(site);
                    }
                }
                OwnedInstruction::ConstructComposite {
                    destination,
                    fields,
                } => {
                    let target = ordinary(f, *destination, s)?;
                    if !matches!(target.kind, OwnerKind::Local { .. } | OwnerKind::Temporary) {
                        return Err(bad(Malformed::OwnerClass, s));
                    }
                    let prev = &mut owners[destination.0].initialize;
                    if prev.is_some() {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    *prev = Some(site);
                    let record = record(target.aggregate(), s)?;
                    let declared = d.fields(record)?;
                    equal(fields.len(), declared.len(), s)?;
                    field_seen[..declared.len()].fill(0);
                    for (position, (id, value)) in fields.iter().enumerate() {
                        meter.visit()?;
                        let field = d.field(record, *id)?;
                        if field_seen[id.index] != 0 {
                            return Err(bad(Malformed::Type, s));
                        }
                        field_seen[id.index] = 1;
                        match value {
                            FieldInitializer::Scalar(value) => equal(
                                field.value_ty(),
                                ValueTy::Scalar(operand(f, *value, sources)?),
                                value.span,
                            )?,
                            FieldInitializer::Owned(source) => {
                                let source_decl = ordinary(f, *source, s)?;
                                if source == destination || source_decl.kind != OwnerKind::Temporary
                                {
                                    return Err(bad(Malformed::OwnerClass, s));
                                }
                                d.same_value_type(
                                    field.value_ty(),
                                    ValueTy::Owned(source_decl.aggregate()),
                                )?;
                                for (_, earlier) in &fields[..position] {
                                    meter.visit()?;
                                    if matches!(earlier, FieldInitializer::Owned(other) if other == source)
                                    {
                                        return Err(bad(Malformed::Binding, s));
                                    }
                                }
                            }
                        }
                    }
                }
                OwnedInstruction::ReadProjection {
                    destination,
                    base: b,
                    path,
                    index,
                } => {
                    let ty = projection_type(f, d, *b, path, *index, sources, meter, s)?;
                    equal(local(f, *destination, s)?, ty, s)?;
                }
                OwnedInstruction::WriteProjection {
                    base: b,
                    path,
                    index,
                    value,
                } => {
                    let ty = projection_type(f, d, *b, path, *index, sources, meter, s)?;
                    equal(operand(f, *value, sources)?, ty, value.span)?;
                }
                OwnedInstruction::ProjectionLength {
                    destination,
                    base: b,
                    path,
                } => {
                    let (referent, _) = base(f, *b, s)?;
                    let BorrowedTy::Exact(root) = referent else {
                        return Err(bad(Malformed::Type, s));
                    };
                    for _ in path {
                        meter.visit()?;
                    }
                    let (ValueTy::Owned(AggregateTy::FixedArray(_) | AggregateTy::Vector(_)), _) =
                        d.projection(root, path)?
                    else {
                        return Err(bad(Malformed::Type, s));
                    };
                    equal(local(f, *destination, s)?, hir::Ty::I32, s)?;
                }
                OwnedInstruction::ConstructArray {
                    destination,
                    elements,
                } => {
                    let o = ordinary(f, *destination, s)?;
                    if !matches!(o.kind, OwnerKind::Local { .. } | OwnerKind::Temporary) {
                        return Err(bad(Malformed::OwnerClass, s));
                    }
                    let prev = &mut owners[destination.0].initialize;
                    if prev.is_some() {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    *prev = Some(site);
                    let declared = array(o.aggregate(), s)?;
                    if elements.len() != declared.length() {
                        return Err(bad(Malformed::Type, s));
                    }
                    // Both all-shape passes charge actual admitted operands;
                    // arrays need no element-sized scratch or field-ID table.
                    for value in elements {
                        meter.visit()?;
                        equal(operand(f, *value, sources)?, declared.element(), value.span)?;
                    }
                }
                OwnedInstruction::CopyInitialize { destination, source }
                | OwnedInstruction::MoveInitialize {
                    destination,
                    source,
                }
                | OwnedInstruction::Replace {
                    destination,
                    source,
                } => {
                    let dst = ordinary(f, *destination, s)?;
                    let src = ordinary(f, *source, s)?;
                    same_aggregate(d, dst.aggregate(), src.aggregate(), Malformed::Type, s)?;
                    if matches!(instruction.kind, OwnedInstruction::CopyInitialize { .. })
                        && !d.is_copy(src.aggregate())
                    {
                        return Err(bad(Malformed::Type, s));
                    }
                    if destination == source {
                        return Err(bad(Malformed::OwnerClass, s));
                    }
                    if matches!(instruction.kind, OwnedInstruction::Replace { .. }) {
                        if !matches!(dst.kind, OwnerKind::Local { .. }) {
                            return Err(bad(Malformed::OwnerClass, s));
                        }
                    } else {
                        if !matches!(dst.kind, OwnerKind::Local { .. } | OwnerKind::Temporary) {
                            return Err(bad(Malformed::OwnerClass, s));
                        }
                        let prev = &mut owners[destination.0].initialize;
                        if prev.is_some() {
                            return Err(bad(Malformed::CanonicalSite, s));
                        }
                        *prev = Some(site);
                    }
                }
                OwnedInstruction::ReadField {
                    destination,
                    base: b,
                    field,
                } => {
                    let (r, _) = base(f, *b, s)?;
                    equal(
                        ValueTy::Scalar(local(f, *destination, s)?),
                        d.field(borrowed_record(r, s)?, *field)?.value_ty(),
                        s,
                    )?;
                }
                OwnedInstruction::WriteField {
                    base: b,
                    field,
                    value,
                } => {
                    let (r, _) = base(f, *b, s)?;
                    equal(
                        ValueTy::Scalar(operand(f, *value, sources)?),
                        d.field(borrowed_record(r, s)?, *field)?.value_ty(),
                        value.span,
                    )?;
                }
                OwnedInstruction::ReadIndex {
                    destination,
                    base: b,
                    index,
                } => {
                    let (aggregate, _) = base(f, *b, s)?;
                    let element = array_element(aggregate, s)?;
                    equal(operand(f, *index, sources)?, hir::Ty::I32, index.span)?;
                    equal(local(f, *destination, s)?, element, s)?;
                }
                OwnedInstruction::WriteIndex {
                    base: b,
                    index,
                    value,
                } => {
                    let (aggregate, _) = base(f, *b, s)?;
                    let element = array_element(aggregate, s)?;
                    equal(operand(f, *value, sources)?, element, value.span)?;
                    equal(operand(f, *index, sources)?, hir::Ty::I32, index.span)?;
                }
                OwnedInstruction::ArrayLength {
                    destination,
                    base: b,
                } => {
                    let (aggregate, _) = base(f, *b, s)?;
                    array_element(aggregate, s)?;
                    equal(local(f, *destination, s)?, hir::Ty::I32, s)?;
                }
                OwnedInstruction::OpenCall(c) => {
                    call(f, *c, s)?;
                    let prev = &mut calls[c.0].open;
                    if prev.is_some() {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    *prev = Some(site);
                }
                OwnedInstruction::PrepareScalar {
                    call: c,
                    argument,
                    value,
                } => {
                    let decl = call(f, *c, s)?;
                    let target = &raw.functions[decl.target.0];
                    if decl.arguments.get(*argument) != Some(&ArgumentSlot::Scalar) {
                        return Err(bad(Malformed::Binding, s));
                    }
                    equal(
                        ParameterTy::Value(ValueTy::Scalar(operand(f, *value, sources)?)),
                        parameter(target, target.parameters[*argument], s)?,
                        s,
                    )?;
                    slot(
                        &mut preparations,
                        add(calls[c.0].start, *argument)?,
                        site,
                        s,
                    )?;
                }
                OwnedInstruction::PrepareOwned {
                    call: c,
                    argument,
                    source,
                } => {
                    let decl = call(f, *c, s)?;
                    let Some(ArgumentSlot::Owned(staged)) = decl.arguments.get(*argument) else {
                        return Err(bad(Malformed::Binding, s));
                    };
                    same_aggregate(
                        d,
                        ordinary(f, *source, s)?.aggregate(),
                        owner(f, *staged, s)?.aggregate(),
                        Malformed::Type,
                        s,
                    )?;
                    slot(
                        &mut preparations,
                        add(calls[c.0].start, *argument)?,
                        site,
                        s,
                    )?;
                }
                OwnedInstruction::AcquireLocalBorrow(l) => {
                    let declaration = loan(f, *l, s)?;
                    if declaration.region != LoanRegion::Local {
                        return Err(bad(Malformed::Binding, s));
                    }
                    if declaration.span != s {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    slot(&mut acquisitions, l.0, site, s)?;
                }
                OwnedInstruction::ReleaseLocalBorrow(l) => {
                    if loan(f, *l, s)?.region != LoanRegion::Local {
                        return Err(bad(Malformed::Binding, s));
                    }
                }
                OwnedInstruction::PrepareBorrow {
                    call: c,
                    argument,
                    loan: l,
                }
                | OwnedInstruction::PrepareBorrowRange {
                    call: c,
                    argument,
                    loan: l,
                    ..
                } => {
                    if let OwnedInstruction::PrepareBorrowRange { start, end, .. } =
                        instruction.kind
                    {
                        equal(operand(f, start, sources)?, hir::Ty::I32, s)?;
                        equal(operand(f, end, sources)?, hir::Ty::I32, s)?;
                        // The loan loop independently checks the authority,
                        // projection and exact element type. Ranges only select
                        // a narrower extent of that same scalar view; imports
                        // cannot authorize a different type or byte reinterpretation.
                        let BorrowedTy::ScalarSlice(element) = loan(f, *l, s)?.referent() else {
                            return Err(bad(Malformed::Type, s));
                        };
                        d.check_borrowed_type(BorrowedTy::ScalarSlice(element))?;
                    }
                    let decl = call(f, *c, s)?;
                    if decl.arguments.get(*argument) != Some(&ArgumentSlot::Borrow(*l)) {
                        return Err(bad(Malformed::Binding, s));
                    }
                    if loan(f, *l, s)?.span != s {
                        return Err(bad(Malformed::CanonicalSite, s));
                    }
                    slot(
                        &mut preparations,
                        add(calls[c.0].start, *argument)?,
                        site,
                        s,
                    )?;
                    slot(&mut acquisitions, l.0, site, s)?;
                }
            }
        }
        let end = b
            .terminator
            .as_ref()
            .ok_or_else(|| bad(Malformed::MissingTerminator, b.span))?;
        span(sources, end.span)?;
        diagnostic_origins(sources, end.diagnostic_origins, meter)?;
        let target = |id: BlockId| {
            if id.0 < f.blocks.len() {
                Ok(())
            } else {
                Err(bad(Malformed::Id, end.span))
            }
        };
        match end.kind {
            OwnedTerminatorKind::MatchDispatch { match_id, arm } => {
                let (descriptor, selected) = match_arm(f, match_id, arm, end.span)?;
                if selected.dispatch != BlockId(bi)
                    || end.span != descriptor.span
                    || end
                        .diagnostic_origins
                        .is_some_and(|o| o.primary != descriptor.span || o.cause != descriptor.span)
                {
                    return Err(bad(Malformed::CanonicalSite, descriptor.span));
                }
            }
            OwnedTerminatorKind::Branch {
                condition,
                then_block,
                else_block,
            } => {
                equal(
                    operand(f, condition, sources)?,
                    hir::Ty::Bool,
                    condition.span,
                )?;
                target(then_block)?;
                target(else_block)?;
            }
            OwnedTerminatorKind::Goto(id) => target(id)?,
            OwnedTerminatorKind::Invoke {
                call: c,
                continuation,
            } => {
                call(f, c, end.span)?;
                target(continuation)?;
                let prev = &mut calls[c.0].invoke;
                if prev.is_some() {
                    return Err(bad(Malformed::CanonicalSite, end.span));
                }
                *prev = Some(bi);
            }
            OwnedTerminatorKind::ReturnScalar(v) => {
                equal(ValueTy::Scalar(operand(f, v, sources)?), f.result, v.span)?
            }
            OwnedTerminatorKind::ReturnOwned(o) => {
                let actual = ordinary(f, o, end.span)?.aggregate();
                let ValueTy::Owned(expected) = f.result else {
                    return Err(bad(Malformed::Type, end.span));
                };
                same_aggregate(d, actual, expected, Malformed::Type, end.span)?;
            }
        }
    }
    if calls.iter().any(|c| c.open.is_none() || c.invoke.is_none())
        || preparations.iter().any(Option::is_none)
        || acquisitions.iter().any(Option::is_none)
    {
        return Err(bad(Malformed::CanonicalSite, f.span));
    }
    // A raw lifetime needs one canonical live site, and at most one canonical
    // initializer (enforced above). U may end without reading any payload;
    // this does not introduce source-level uninitialized declarations.
    for (o, sites) in f.owners.iter().zip(owners) {
        if matches!(o.kind, OwnerKind::Local { .. } | OwnerKind::Temporary)
            && (sites.live.is_none())
        {
            return Err(bad(Malformed::CanonicalSite, o.span));
        }
    }
    // Build a parent forest with linear adjacency and iterative DFS. Unvisited
    // vertices after visiting every root are precisely cyclic components.
    let mut first = filled(calls.len(), NONE)?;
    let mut next = filled(calls.len(), NONE)?;
    let mut stack = reserve(calls.len())?;
    for (i, c) in f.calls.iter().enumerate() {
        if let Some((p, _)) = c.parent {
            next[i] = first[p.0];
            first[p.0] = i;
        }
    }
    let (mut clock, mut visited) = (0, 0);
    for root in 0..calls.len() {
        if f.calls[root].parent.is_some() {
            continue;
        }
        stack.push((root, false));
        while let Some((id, exit)) = stack.pop() {
            if exit {
                calls[id].exit = clock;
                clock += 1;
                continue;
            }
            calls[id].enter = clock;
            clock += 1;
            visited += 1;
            stack.push((id, true));
            let mut child = first[id];
            while child != NONE {
                stack.push((child, false));
                child = next[child];
            }
        }
    }
    if visited != calls.len() {
        return Err(bad(Malformed::CallParent, f.span));
    }
    Ok(Shape {
        calls,
        acquisitions,
    })
}

fn diagnostic_origins(
    sources: &SourceMap,
    origins: Option<DiagnosticOrigins>,
    meter: &mut budget::Meter,
) -> Result<(), OwnedFailure> {
    if let Some(origins) = origins {
        meter.visit()?;
        span(sources, origins.primary)?;
        meter.visit()?;
        span(sources, origins.cause)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // Same independently checked source/type/work context as instruction shape.
fn projection_type(
    f: &RawOwnedFunction,
    d: &Declarations,
    b: AccessBase,
    path: &[FieldId],
    index: Option<Operand>,
    sources: &SourceMap,
    meter: &mut budget::Meter,
    s: Span,
) -> Result<hir::Ty, OwnedFailure> {
    let (referent, _) = base(f, b, s)?;
    let BorrowedTy::Exact(root) = referent else {
        return Err(bad(Malformed::Type, s));
    };
    for _ in path {
        meter.visit()?;
    }
    let (ty, _) = d.projection(root, path)?;
    match (ty, index) {
        (ValueTy::Scalar(ty), None) => Ok(ty),
        (ValueTy::Owned(AggregateTy::FixedArray(array)), Some(index)) => {
            equal(operand(f, index, sources)?, hir::Ty::I32, index.span)?;
            Ok(array.element())
        }
        (ValueTy::Owned(AggregateTy::Vector(element)), Some(index)) => {
            equal(operand(f, index, sources)?, hir::Ty::I32, index.span)?;
            Ok(element)
        }
        _ => Err(bad(Malformed::Type, s)),
    }
}

fn enumeration(aggregate: AggregateTy, s: Span) -> Result<EnumId, OwnedFailure> {
    match aggregate {
        AggregateTy::Enum(id) => Ok(id),
        _ => Err(bad(Malformed::Type, s)),
    }
}
pub(super) fn match_arm(
    f: &RawOwnedFunction,
    id: MatchId,
    arm: usize,
    s: Span,
) -> Result<(&MatchDecl, &MatchArm), OwnedFailure> {
    let descriptor = f.matches.get(id.0).ok_or_else(|| bad(Malformed::Id, s))?;
    let selected = descriptor
        .arms
        .get(arm)
        .ok_or_else(|| bad(Malformed::Id, descriptor.span))?;
    Ok((descriptor, selected))
}
/// Temporary global role and edge-multiplicity proof. Kept separate from the
/// retained call/owner sites and dropped before their allocation.
#[derive(Clone, Copy)]
pub(super) struct MatchBlockRole {
    role: u8,
    predecessor: usize,
    incoming: usize,
}
fn check_matches(
    f: &RawOwnedFunction,
    d: &Declarations,
    sources: &SourceMap,
    meter: &mut budget::Meter,
) -> Result<(), OwnedFailure> {
    if f.matches.is_empty() {
        return Ok(());
    }
    use super::super::verify::CfgView;
    let mut capacity = 0;
    for descriptor in &f.matches {
        meter.visit()?;
        if descriptor.arms.len() > 256 {
            return Err(bad(Malformed::Type, descriptor.span));
        }
        capacity = capacity.max(descriptor.arms.len());
    }
    let mut roles = filled(
        f.blocks.len(),
        MatchBlockRole {
            role: 0,
            predecessor: NONE,
            incoming: 0,
        },
    )?;
    let mut seen = filled(capacity, false)?;
    for (id, descriptor) in f.matches.iter().enumerate() {
        meter.visit()?;
        let s = descriptor.span;
        span(sources, s)?;
        let source = owner(f, descriptor.source, s)?;
        if !matches!(
            source.kind,
            OwnerKind::Local { .. } | OwnerKind::Parameter { .. }
        ) {
            return Err(bad(Malformed::OwnerClass, s));
        }
        let enumeration = enumeration(source.aggregate(), s)?;
        let variants = d
            .enums()
            .variants(enumeration)
            .map_err(DeclarationError::from)?;
        if descriptor.arms.is_empty()
            || descriptor.arms.len() > 256
            || descriptor.arms.len() != variants.len()
        {
            return Err(bad(Malformed::Type, s));
        }
        seen[..descriptor.arms.len()].fill(false);
        for (arm, selected) in descriptor.arms.iter().enumerate() {
            meter.visit()?;
            d.enums()
                .variant(enumeration, selected.variant)
                .map_err(DeclarationError::from)?;
            if seen[selected.variant.index] {
                return Err(bad(Malformed::Type, s));
            }
            seen[selected.variant.index] = true;
            for (block, role, predecessor) in [
                (
                    selected.dispatch,
                    1,
                    if arm == 0 {
                        NONE
                    } else {
                        descriptor.arms[arm - 1].dispatch.0
                    },
                ),
                (selected.entry, 2, selected.dispatch.0),
            ] {
                meter.visit()?;
                let slot = roles
                    .get_mut(block.0)
                    .ok_or_else(|| bad(Malformed::Id, s))?;
                if slot.role != 0 || (predecessor != NONE && block == f.entry) {
                    return Err(bad(Malformed::CanonicalSite, s));
                }
                *slot = MatchBlockRole {
                    role,
                    predecessor,
                    incoming: 0,
                };
            }
            let dispatch = &f.blocks[selected.dispatch.0];
            if arm != 0 && (!dispatch.statements.is_empty() || dispatch.merge.is_some()) {
                return Err(bad(Malformed::CanonicalSite, s));
            }
            if !matches!(dispatch.terminator.as_ref().map(|e| &e.kind),
                Some(OwnedTerminatorKind::MatchDispatch { match_id, arm: actual }) if *match_id == MatchId(id) && *actual == arm)
            {
                return Err(bad(Malformed::CanonicalSite, s));
            }
            let entry = &f.blocks[selected.entry.0];
            if entry.merge.is_some()
                || !matches!(entry.statements.first().map(|i| &i.kind),
                Some(OwnedInstruction::ConsumeVariant { match_id, arm: actual, .. }
                    | OwnedInstruction::ConsumeOwnedVariant { match_id, arm: actual, .. }) if *match_id == MatchId(id) && *actual == arm)
            {
                return Err(bad(Malformed::CanonicalSite, s));
            }
        }
    }
    // Count edges, never distinct predecessor blocks. The implicit entry edge
    // was excluded above because it is not present in this explicit CFG scan.
    for (block, _) in f.blocks.iter().enumerate() {
        meter.visit()?;
        for target in f.successors(block)?.into_iter().flatten() {
            meter.visit()?;
            let slot = roles
                .get_mut(target.0)
                .ok_or_else(|| bad(Malformed::Id, f.span))?;
            if slot.predecessor != NONE {
                if slot.predecessor != block || slot.incoming != 0 {
                    return Err(bad(Malformed::CanonicalSite, f.blocks[target.0].span));
                }
                slot.incoming += 1;
            }
        }
    }
    for (block, role) in roles.iter().enumerate() {
        meter.visit()?;
        if role.predecessor != NONE && role.incoming != 1 {
            return Err(bad(Malformed::CanonicalSite, f.blocks[block].span));
        }
    }
    Ok(())
}
