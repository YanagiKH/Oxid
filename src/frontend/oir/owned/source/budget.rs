//! Source output inventory only; neither a declaration nor an executable witness.
use super::super::{budget as raw_budget, *};
use super::{lower, typeck::TypedOwnedProgram};
use std::mem::size_of;

pub(super) const MAX_RAW_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub raw_bytes: usize,
}
impl Limits {
    pub const DEFAULT: Self = Self {
        raw_bytes: MAX_RAW_BYTES,
    };
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Usage {
    pub raw_bytes: usize,
    pub scratch_bytes: usize,
    pub analysis: OwnershipUsage,
}
pub(super) fn add(a: usize, b: usize) -> Result<usize, OwnedFailure> {
    raw_budget::add(a, b)
}
pub(super) fn mul(a: usize, b: usize) -> Result<usize, OwnedFailure> {
    raw_budget::mul(a, b)
}
pub(super) fn cap(value: usize, ceiling: usize, name: &'static str) -> Result<usize, OwnedFailure> {
    if value <= ceiling {
        Ok(value)
    } else {
        Err(OwnedFailure::resource(name))
    }
}
fn at(mut error: OwnedFailure, span: Span) -> OwnedFailure {
    if error.primary.get().is_none() {
        error.primary = Origin(span);
    }
    error
}
pub(super) fn reserve<T>(count: usize) -> Result<Vec<T>, OwnedFailure> {
    #[cfg(test)]
    guard_event(GuardEvent::RawReservation);
    #[cfg(test)]
    allocation_test_point()?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| OwnedFailure::resource("source allocation"))?;
    Ok(values)
}
/// The only new literal payload request. Complete source preflight and the
/// emission count have admitted full S/Q/payload before output allocation.
/// The call site also checks its current partial counts before this request;
/// ConstructArray itself increments S when the completed instruction is emitted.
/// Even a zero-length literal performs one logical fallible reservation.
pub(super) fn reserve_array_operands(count: usize) -> Result<Vec<Operand>, OwnedFailure> {
    #[cfg(test)]
    guard_event(GuardEvent::RawReservation);
    #[cfg(test)]
    allocation_test_point()?;
    let requested = count;
    #[cfg(test)]
    let requested = ARRAY_OPERAND_FAILURE.with(|point| match point.get() {
        Some(0) => usize::MAX,
        Some(n) => {
            point.set(Some(n - 1));
            requested
        }
        None => requested,
    });
    let mut values = Vec::new();
    let result = values.try_reserve_exact(requested);
    #[cfg(test)]
    super::array_pipeline::literal_reserved(count, requested == usize::MAX, result.is_ok());
    result.map_err(|_| OwnedFailure::resource("source allocation"))?;
    Ok(values)
}
pub(super) fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>, OwnedFailure> {
    let mut values = reserve(count)?;
    values.resize(count, value);
    Ok(values)
}
pub(super) fn append<T>(
    values: &mut Vec<T>,
    value: T,
    count: usize,
    span: Span,
) -> Result<(), OwnedFailure> {
    if values.len() >= count || values.len() == values.capacity() {
        return Err(OwnedFailure::malformed(Malformed::CanonicalSite, span));
    }
    values.push(value);
    Ok(())
}
pub(super) fn function_bytes(c: raw_budget::FunctionCounts) -> Result<usize, OwnedFailure> {
    let mut bytes = size_of::<RawOwnedFunction>();
    for (n, width) in [
        (c.parameters, size_of::<ParameterBinding>()),
        (c.locals, size_of::<LocalDecl>()),
        (c.places, size_of::<PlaceDecl>()),
        (c.owners, size_of::<OwnerDecl>()),
        (c.references, size_of::<ReferenceDecl>()),
        (c.calls, size_of::<CallDecl>()),
        (c.loans, size_of::<LoanDecl>()),
        (c.matches, size_of::<MatchDecl>()),
        (c.match_arms, size_of::<MatchArm>()),
        (c.descriptor_arguments, size_of::<ArgumentSlot>()),
        (c.blocks, size_of::<OwnedBlock>()),
        (c.statements, size_of::<OwnedStatement>()),
        (c.constructed_fields, size_of::<(FieldId, Operand)>()),
        (c.constructed_elements, size_of::<Operand>()),
        (c.composite_fields, size_of::<(FieldId, FieldInitializer)>()),
        (c.projection_fields, size_of::<FieldId>()),
    ] {
        bytes = add(bytes, mul(n, width)?)?;
    }
    Ok(bytes)
}

#[test]
fn unit2b_q_adds_only_actual_operand_payload_to_source_inventory() {
    let without = raw_budget::FunctionCounts::default();
    for elements in [0, 1, 1024] {
        let with = raw_budget::FunctionCounts {
            constructed_elements: elements,
            ..without
        };
        assert_eq!(
            function_bytes(with).unwrap() - function_bytes(without).unwrap(),
            elements * size_of::<Operand>()
        );
    }
    #[cfg(target_pointer_width = "64")]
    assert_eq!(size_of::<raw_budget::FunctionCounts>(), 184);
}
pub(super) fn preflight(
    typed: &TypedOwnedProgram<'_>,
    limits: Limits,
) -> Result<Usage, OwnedFailure> {
    if (typed.index().builtin_set() != BuiltinOrigins::None || {
        #[cfg(test)]
        {
            typed.index().is_output_candidate_pipeline()
        }
        #[cfg(not(test))]
        {
            false
        }
    }) && !typed.admission().allows_paid_source(typed.index())
    {
        return Err(OwnedFailure::malformed(
            Malformed::Binding,
            typed.index().sources().eof(),
        ));
    }
    if !typed.admission().allows_lowering() {
        return Err(OwnedFailure::malformed(
            Malformed::CanonicalSite,
            typed.index().sources().eof(),
        ));
    }
    if typed.index().builtin_set() != BuiltinOrigins::None {
        typed.validate_function_signatures().map_err(|_| {
            OwnedFailure::malformed(Malformed::Binding, typed.index().sources().eof())
        })?;
    }
    #[cfg(test)]
    if typed.index().builtin_set() == BuiltinOrigins::None
        && typed.admission() == super::resolve::SourceAdmission::BuiltinPipeline
    {
        typed.validate_function_signatures().map_err(|_| {
            OwnedFailure::malformed(Malformed::Binding, typed.index().sources().eof())
        })?;
    }
    #[cfg(test)]
    guard_event(GuardEvent::DeclarationAdmission);
    let mut declarations =
        admit_declaration_counts(typed.records().iter().map(|r| r.fields.len()))?;
    declarations.layout_bytes = if typed
        .records()
        .iter()
        .all(|r| r.fields.iter().all(|f| matches!(f.ty, ValueTy::Scalar(_))))
    {
        admit_scalar_layouts(typed.records().iter().map(|r| {
            r.fields.iter().map(|f| {
                let ValueTy::Scalar(ty) = f.ty else {
                    unreachable!("scalar-only declaration inventory")
                };
                ty
            })
        }))?
    } else {
        admit_value_layouts(
            typed
                .records()
                .iter()
                .map(|r| r.fields.iter().map(|f| f.ty)),
        )?
    };
    // Both aggregate counts and the actual padded record layout sum participate
    // in the shared record/enum gate, including unused enum declarations.
    let enums = admit_enum_counts(typed.index().enum_variant_counts(), declarations)
        .map_err(DeclarationError::from)?;
    let ceiling = limits.raw_bytes.min(MAX_RAW_BYTES);
    let mut bytes = size_of::<RawOwnedProgram>();
    bytes = add(
        bytes,
        mul(declarations.records, size_of::<RawRecordDecl>())?,
    )?;
    bytes = add(bytes, mul(declarations.fields, size_of::<RawFieldDecl>())?)?;
    // RawOwnedProgram and RawEnumDecl already contain the inline Vec headers.
    bytes = add(bytes, mul(enums.enums, size_of::<RawEnumDecl>())?)?;
    bytes = add(bytes, mul(enums.variants, size_of::<RawVariantDecl>())?)?;
    cap(bytes, ceiling, "source raw payload")?;
    let mut scratch = 0;
    let mut counts = raw_budget::ProgramCounts::default();
    raw_budget::account_enum_declarations(enums, raw_budget::Limits::DEFAULT, &mut counts)?;
    raw_budget::account_builtin_descriptors(
        typed.index().builtin_set(),
        raw_budget::Limits::DEFAULT,
        &mut counts,
    )?;
    #[cfg(test)]
    guard_event(GuardEvent::FunctionIteration);
    for view in typed.functions() {
        let span = view.signature().span;
        let count = lower::count_preflight_function(&view).map_err(|error| at(error, span))?;
        raw_budget::account_function(count, raw_budget::Limits::DEFAULT, &mut counts)
            .map_err(|error| at(error, span))?;
        bytes = cap(
            add(bytes, function_bytes(count)?)?,
            ceiling,
            "source raw payload",
        )
        .map_err(|error| at(error, span))?;
        scratch = scratch.max(lower::scratch_bytes(&view, count)?);
    }
    if typed.index().builtin_set().extra_functions() != 0 {
        for kind in crate::frontend::builtin_catalog::BuiltinFunction::ALL {
            if typed.index().builtin_set().contains_function(kind) {
                let count = super::builtin_lower::counts();
                raw_budget::account_function(count, raw_budget::Limits::DEFAULT, &mut counts)?;
                bytes = cap(
                    add(bytes, function_bytes(count)?)?,
                    ceiling,
                    "source raw payload",
                )?;
            }
        }
    }
    // Status-only imports use the same fixed capacity-check roles for the new
    // enum suffix; conservatively admit the bounded builtin producer envelope.
    if typed.index().builtin_set() != BuiltinOrigins::None {
        scratch = scratch.max(super::builtin_lower::carrier_bytes());
    }
    if let Some(seed) = typed.source_storage_bytes() {
        admit_lower_scratch(seed, scratch)?;
    }
    Ok(Usage {
        raw_bytes: bytes,
        scratch_bytes: scratch,
        analysis: counts.usage(),
    })
}

/// Invocation-local overlay. The affected-source owner already paid the fixed
/// Walk/stacks/views and typed projection payload. Function caches are sequential
/// and their maximum is charged once; no persistent source cell is ever changed.
/// Source association follows lowering and releases its tracker before returning
/// the consuming wrapper. Its paid seed already includes fixed RFC0030 roles.
pub(super) fn admit_conversion_scratch(seed: usize, scratch: usize) -> Result<usize, OwnedFailure> {
    let ceiling = super::hir_budget::MAX_HIR_BYTES;
    #[cfg(test)]
    let ceiling =
        CONVERSION_LIMIT.with(|limit| limit.get().map_or(ceiling, |limit| limit.min(ceiling)));
    cap(
        add(seed, scratch)?,
        ceiling,
        "conversion association scratch",
    )
}
pub(super) fn admit_lower_scratch(seed: usize, scratch: usize) -> Result<usize, OwnedFailure> {
    cap(
        add(seed, add(scratch, lower::invocation_control_bytes())?)?,
        super::hir_budget::MAX_HIR_BYTES,
        "source HIR scratch",
    )
}

#[cfg(test)]
thread_local! { static ALLOCATION_FAILURE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) }; }
#[cfg(test)]
thread_local! { static ARRAY_OPERAND_FAILURE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) }; }
/// Zero-based literal emission request; forcing usize::MAX exercises the real
/// fallible capacity-error path rather than the older synthetic failpoint.
#[cfg(test)]
pub(super) fn fail_array_operand_after<T>(count: usize, operation: impl FnOnce() -> T) -> T {
    struct Reset(Option<usize>);
    impl Drop for Reset {
        fn drop(&mut self) {
            ARRAY_OPERAND_FAILURE.with(|point| point.set(self.0));
        }
    }
    let previous = ARRAY_OPERAND_FAILURE.with(|point| point.replace(Some(count)));
    let _reset = Reset(previous);
    operation()
}

#[test]
fn unit3b2_operand_failure_restores_outer_state_and_unwind() {
    fail_array_operand_after(1, || {
        assert!(reserve_array_operands(0).is_ok());
        assert!(fail_array_operand_after(0, || reserve_array_operands(0)).is_err());
        assert!(reserve_array_operands(1).is_err());
        let panic = std::panic::catch_unwind(|| {
            fail_array_operand_after(9, || panic!("restore the enclosing failure control"));
        });
        assert!(panic.is_err());
        assert!(reserve_array_operands(0).is_err());
    });
    assert!(reserve_array_operands(0).is_ok());
}
#[cfg(test)]
fn allocation_test_point() -> Result<(), OwnedFailure> {
    ALLOCATION_FAILURE.with(|point| match point.get() {
        Some(0) => Err(OwnedFailure::resource("injected source allocation failure")),
        Some(n) => {
            point.set(Some(n - 1));
            Ok(())
        }
        None => Ok(()),
    })
}
#[cfg(test)]
pub(super) fn fail_allocation_after<T>(count: usize, operation: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ALLOCATION_FAILURE.with(|point| point.set(None));
        }
    }
    ALLOCATION_FAILURE.with(|point| point.set(Some(count)));
    let _reset = Reset;
    operation()
}

/// Fixed-size passive sentinels for the private types-only entry fences. No
/// source payload or unbounded observation log is retained.
#[cfg(test)]
#[derive(Clone, Copy)]
pub(super) enum GuardEvent {
    DeclarationAdmission,
    FunctionIteration,
    CountEntry,
    InventoryEntry,
    BlockAllocation,
    RawReservation,
    EmitStep,
}
#[cfg(test)]
thread_local! { static GUARD_COUNTS: std::cell::Cell<[usize; 7]> = const { std::cell::Cell::new([0; 7]) }; }
#[cfg(test)]
pub(super) fn guard_event(event: GuardEvent) {
    GUARD_COUNTS.with(|counts| {
        let mut next = counts.get();
        next[event as usize] = next[event as usize].saturating_add(1);
        counts.set(next);
    });
}
#[cfg(test)]
pub(super) fn reset_guard_counts() {
    GUARD_COUNTS.with(|counts| counts.set([0; 7]));
}
#[cfg(test)]
pub(super) fn guard_counts() -> [usize; 7] {
    GUARD_COUNTS.with(|counts| counts.get())
}

#[test]
fn enum_lower_scratch_overlay_has_exact_unchanged_boundary_and_overflow() {
    let controls = lower::invocation_control_bytes();
    let limit = super::hir_budget::MAX_HIR_BYTES;
    for scratch in [0, 1, 1024] {
        let available = limit - controls - scratch;
        assert_eq!(
            admit_lower_scratch(available - 1, scratch).unwrap(),
            limit - 1
        );
        assert_eq!(admit_lower_scratch(available, scratch).unwrap(), limit);
        assert!(admit_lower_scratch(available + 1, scratch).is_err());
        // The helper receives a read-only value; repeated successful/failed
        // invocations cannot debit or replace the owner's persistent seed.
        assert_eq!(admit_lower_scratch(available, scratch).unwrap(), limit);
        assert_eq!(available, limit - controls - scratch);
    }
    assert!(admit_lower_scratch(usize::MAX, 0).is_err());
    assert!(admit_lower_scratch(0, usize::MAX).is_err());
}

#[cfg(test)]
thread_local! {
    static CONVERSION_LIMIT: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
pub(super) fn with_conversion_limit<T>(limit: usize, action: impl FnOnce() -> T) -> T {
    struct Reset(Option<usize>);
    impl Drop for Reset {
        fn drop(&mut self) {
            CONVERSION_LIMIT.with(|limit| limit.set(self.0));
        }
    }
    let _reset = Reset(CONVERSION_LIMIT.with(|value| value.replace(Some(limit))));
    action()
}
