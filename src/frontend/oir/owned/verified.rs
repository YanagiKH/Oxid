//! The sealed boundary. Sibling consumers cannot construct or mutate fields.
use super::*;
use crate::frontend::builtin_catalog::{BuiltinEnum, BuiltinFunction};

#[derive(Debug)]
pub(super) struct VerifiedOwnedProgram {
    program: RawOwnedProgram,
    declarations: Declarations,
    usage: OwnershipUsage,
    seal: OwnershipSeal,
}
#[derive(Debug)]
struct OwnershipSeal;

/// Requests identify the exact immutable instruction whose region was checked.
/// A consumer can request a permission, but cannot manufacture its proof.
pub(super) enum ProjectionRequest<'a> {
    Field {
        base: AccessBase,
        field: FieldId,
        write: bool,
    },
    Path {
        base: AccessBase,
        path: &'a [FieldId],
        write: bool,
    },
    Borrow(LoanId),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ProjectionAccess {
    Read,
    Write,
    Borrow(BorrowKind),
}

pub(super) struct ProjectionPermission<'a> {
    witness: &'a VerifiedOwnedProgram,
    function: hir::DefId,
    block: BlockId,
    statement: usize,
    base: AccessBase,
    access: ProjectionAccess,
}

impl ProjectionPermission<'_> {
    pub(super) fn permits(
        &self,
        witness: &VerifiedOwnedProgram,
        function: hir::DefId,
        block: BlockId,
        statement: usize,
        base: AccessBase,
        access: ProjectionAccess,
    ) -> bool {
        std::ptr::eq(self.witness, witness)
            && self.function == function
            && self.block == block
            && self.statement == statement
            && self.base == base
            && self.access == access
    }
}

#[cfg(test)]
mod projection_permission_tests {
    use super::*;

    fn field(index: usize) -> FieldId {
        FieldId {
            record: RecordId(1),
            index,
        }
    }

    fn sibling_loans() -> (SourceMap, RawOwnedProgram) {
        use consumer_fixtures as fixture;
        let (sources, mut raw, _) = fixture::shared_read();
        let span = raw.functions[0].span;
        let inner = AggregateTy::Record(RecordId(0));
        raw.records.push(RawRecordDecl {
            is_copy: false,
            id: RecordId(1),
            span,
            fields: (0..2)
                .map(|index| RawFieldDecl {
                    id: field(index),
                    ty: ParameterTy::Value(ValueTy::Owned(inner)),
                    span,
                })
                .collect(),
        });
        let main = &mut raw.functions[0];
        main.owners[0].aggregate =
            AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(1))).unwrap();
        main.owners[0].kind = OwnerKind::Local { mutable: true };
        for _ in 0..2 {
            main.owners.push(fixture::owner(OwnerKind::Temporary, span));
        }
        let initial = main.blocks[0].statements.remove(2);
        assert!(matches!(initial.kind, OwnedInstruction::Construct { .. }));
        let mut construction = Vec::new();
        for owner in 1..=2 {
            construction.push(fixture::instruction(
                OwnedInstruction::StorageLive(OwnerPlaceId(owner)),
                span,
            ));
            construction.push(fixture::instruction(
                OwnedInstruction::Construct {
                    destination: OwnerPlaceId(owner),
                    fields: vec![(
                        FieldId {
                            record: RecordId(0),
                            index: 0,
                        },
                        fixture::operand(0, span),
                    )],
                },
                span,
            ));
        }
        construction.push(fixture::instruction(
            OwnedInstruction::ConstructComposite {
                destination: OwnerPlaceId(0),
                fields: vec![
                    (field(0), FieldInitializer::Owned(OwnerPlaceId(1))),
                    (field(1), FieldInitializer::Owned(OwnerPlaceId(2))),
                ],
            },
            span,
        ));
        for owner in 1..=2 {
            construction.push(fixture::instruction(
                OwnedInstruction::StorageEnd(OwnerPlaceId(owner)),
                span,
            ));
        }
        main.blocks[0].statements.splice(2..2, construction);
        main.loans[0].projection = vec![field(0)];
        main.loans[0].kind = BorrowKind::Exclusive;
        main.loans.push(LoanDecl {
            projection: vec![field(1)],
            region: LoanRegion::Call {
                call: CallSiteId(0),
                argument: 1,
            },
            authority: AccessBase::Owner(OwnerPlaceId(0)),
            kind: BorrowKind::Exclusive,
            referent: BorrowedSlot::check(BorrowedTy::Exact(inner)).unwrap(),
            span,
        });
        main.calls[0]
            .arguments
            .push(ArgumentSlot::Borrow(LoanId(1)));
        main.blocks[0].statements.push(fixture::instruction(
            OwnedInstruction::PrepareBorrow {
                call: CallSiteId(0),
                argument: 1,
                loan: LoanId(1),
            },
            span,
        ));
        let callee = &mut raw.functions[1];
        callee.references[0].kind = BorrowKind::Exclusive;
        callee.references.push(ReferenceDecl {
            referent: BorrowedSlot::check(BorrowedTy::Exact(inner)).unwrap(),
            kind: BorrowKind::Exclusive,
            position: 1,
            span,
        });
        callee
            .parameters
            .push(ParameterBinding::Reference(ReferenceParamId(1)));
        (sources, raw)
    }

    #[test]
    fn independent_raw_sibling_loans_require_nominal_paths_and_sufficient_work() {
        let (sources, raw) = sibling_loans();
        let witness = verify_owned(raw, &sources).unwrap();
        assert_eq!(
            execute::run(&witness, Some(hir::DefId(0))),
            Ok(Scalar::I32(-47))
        );
        for (pc, statement) in witness.functions()[0].blocks[0]
            .statements
            .iter()
            .enumerate()
        {
            if let OwnedInstruction::PrepareBorrow { loan, .. } = statement.kind {
                let proof = witness
                    .projection_permission(
                        hir::DefId(0),
                        BlockId(0),
                        pc,
                        ProjectionRequest::Borrow(loan),
                    )
                    .unwrap();
                assert!(proof.permits(
                    &witness,
                    hir::DefId(0),
                    BlockId(0),
                    pc,
                    AccessBase::Owner(OwnerPlaceId(0)),
                    ProjectionAccess::Borrow(BorrowKind::Exclusive)
                ));
                assert!(witness
                    .projection_permission(
                        hir::DefId(0),
                        BlockId(0),
                        pc,
                        ProjectionRequest::Borrow(LoanId(1 - loan.0))
                    )
                    .is_none());
            }
        }
        let (sources, raw) = sibling_loans();
        let failure = verify_with_limits(
            raw,
            &sources,
            budget::Limits {
                work: witness.usage().work - 1,
                ..budget::Limits::DEFAULT
            },
        )
        .unwrap_err();
        assert!(matches!(failure.kind, OwnedFailureKind::Resource(_)));
        for mutation in 0..5 {
            let (sources, mut raw) = sibling_loans();
            match mutation {
                0 => raw.functions[0].loans[1].projection = vec![field(0)],
                1 => raw.functions[0].loans[1].projection[0].record = RecordId(0),
                2 => raw.functions[0].loans[1].projection[0].index = usize::MAX,
                3 => raw.functions[0].owners[0].kind = OwnerKind::Local { mutable: false },
                _ => raw.functions[0].loans[1].projection.clear(),
            }
            let failure = verify_owned(raw, &sources).unwrap_err();
            if mutation == 0 {
                assert_eq!(
                    failure.kind,
                    OwnedFailureKind::Ownership(Violation::LoanConflict)
                );
            } else if mutation == 3 {
                assert_eq!(
                    failure.kind,
                    OwnedFailureKind::Ownership(Violation::Permission)
                );
            } else {
                assert!(matches!(failure.kind, OwnedFailureKind::Malformed(_)));
            }
        }
    }

    #[test]
    fn projected_permission_is_bound_to_witness_instruction_base_and_mode() {
        let (sources, raw, _) = consumer_fixtures::shared_read();
        let first = verify_owned(raw, &sources).unwrap();
        let (sources, raw, _) = consumer_fixtures::shared_read();
        let other = verify_owned(raw, &sources).unwrap();
        let function = hir::DefId(1);
        let block = BlockId(0);
        let base = AccessBase::Parameter(ReferenceParamId(0));
        let field = FieldId {
            record: RecordId(0),
            index: 0,
        };
        let request = || ProjectionRequest::Field {
            base,
            field,
            write: false,
        };
        let proof = first
            .projection_permission(function, block, 0, request())
            .unwrap();
        assert!(proof.permits(&first, function, block, 0, base, ProjectionAccess::Read));
        assert!(!proof.permits(&other, function, block, 0, base, ProjectionAccess::Read));
        assert!(!proof.permits(
            &first,
            hir::DefId(0),
            block,
            0,
            base,
            ProjectionAccess::Read
        ));
        assert!(!proof.permits(
            &first,
            function,
            BlockId(1),
            0,
            base,
            ProjectionAccess::Read
        ));
        assert!(!proof.permits(&first, function, block, 1, base, ProjectionAccess::Read));
        assert!(!proof.permits(
            &first,
            function,
            block,
            0,
            AccessBase::Owner(OwnerPlaceId(0)),
            ProjectionAccess::Read
        ));
        for access in [
            ProjectionAccess::Write,
            ProjectionAccess::Borrow(BorrowKind::Shared),
            ProjectionAccess::Borrow(BorrowKind::Exclusive),
        ] {
            assert!(!proof.permits(&first, function, block, 0, base, access));
        }
        assert!(first
            .projection_permission(hir::DefId(usize::MAX), block, 0, request())
            .is_none());
        assert!(first
            .projection_permission(function, BlockId(usize::MAX), 0, request())
            .is_none());
        assert!(first
            .projection_permission(function, block, usize::MAX, request())
            .is_none());
        for request in [
            ProjectionRequest::Field {
                base,
                field,
                write: true,
            },
            ProjectionRequest::Field {
                base,
                field: FieldId { index: 1, ..field },
                write: false,
            },
            ProjectionRequest::Borrow(LoanId(0)),
        ] {
            assert!(first
                .projection_permission(function, block, 0, request)
                .is_none());
        }
        // The caller's whole-record loan cannot suspend the whole-root checks.
        assert!(first
            .projection_permission(
                hir::DefId(0),
                block,
                4,
                ProjectionRequest::Borrow(LoanId(0))
            )
            .is_none());
    }

    #[test]
    fn projection_paths_require_the_actual_immutable_instruction_slice() {
        let (sources, raw) = composition_reference_tests::batch();
        let witness = verify_owned(raw, &sources).unwrap();
        let mut checked = 0;
        for f in witness.functions() {
            for (block, body) in f.blocks.iter().enumerate() {
                for (pc, statement) in body.statements.iter().enumerate() {
                    if let OwnedInstruction::ReadProjection { base, path, .. } = &statement.kind {
                        let request = |path| ProjectionRequest::Path {
                            base: *base,
                            path,
                            write: false,
                        };
                        assert!(witness
                            .projection_permission(f.id, BlockId(block), pc, request(path))
                            .is_some());
                        let copied = path.clone();
                        assert!(witness
                            .projection_permission(f.id, BlockId(block), pc, request(&copied))
                            .is_none());
                        assert!(witness
                            .projection_permission(f.id, BlockId(block), pc, request(&[]))
                            .is_none());
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 0);
    }
}

impl VerifiedOwnedProgram {
    pub(super) fn projection_permission(
        &self,
        function: hir::DefId,
        block: BlockId,
        statement: usize,
        request: ProjectionRequest<'_>,
    ) -> Option<ProjectionPermission<'_>> {
        let f = self
            .program
            .functions
            .get(function.0)
            .filter(|f| f.id == function)?;
        let instruction = &f.blocks.get(block.0)?.statements.get(statement)?.kind;
        let (base, access) = match (instruction, request) {
            (
                OwnedInstruction::ReadField { base, field, .. },
                ProjectionRequest::Field {
                    base: requested,
                    field: target,
                    write: false,
                },
            ) if *base == requested && *field == target => (*base, ProjectionAccess::Read),
            (
                OwnedInstruction::WriteField { base, field, .. },
                ProjectionRequest::Field {
                    base: requested,
                    field: target,
                    write: true,
                },
            ) if *base == requested && *field == target => (*base, ProjectionAccess::Write),
            (
                OwnedInstruction::ReadProjection { base, path, .. }
                | OwnedInstruction::CopyIndexInitialize { base, path, .. }
                | OwnedInstruction::ProjectionLength { base, path, .. },
                ProjectionRequest::Path {
                    base: requested,
                    path: target,
                    write: false,
                },
            ) if *base == requested
                && !path.is_empty()
                && std::ptr::eq(path.as_slice(), target) =>
            {
                (*base, ProjectionAccess::Read)
            }
            (
                OwnedInstruction::WriteProjection { base, path, .. }
                | OwnedInstruction::CopyIndexWrite { base, path, .. },
                ProjectionRequest::Path {
                    base: requested,
                    path: target,
                    write: true,
                },
            ) if *base == requested
                && !path.is_empty()
                && std::ptr::eq(path.as_slice(), target) =>
            {
                (*base, ProjectionAccess::Write)
            }
            (
                OwnedInstruction::PrepareBorrow { loan, .. }
                | OwnedInstruction::PrepareBorrowRange { loan, .. },
                ProjectionRequest::Borrow(requested),
            ) if *loan == requested => {
                let loan = f.loans.get(loan.0)?;
                if loan.projection.is_empty() {
                    return None;
                }
                (loan.authority, ProjectionAccess::Borrow(loan.kind))
            }
            _ => return None,
        };
        Some(ProjectionPermission {
            witness: self,
            function,
            block,
            statement,
            base,
            access,
        })
    }
    pub(super) fn experimental_function(
        &self,
        item: crate::frontend::builtin_catalog::experimental::Function,
    ) -> Option<hir::DefId> {
        let rank = self.program.builtins.experimental_function_rank(item)?;
        self.program
            .fixed_builtin_base()?
            .checked_add(rank)
            .map(hir::DefId)
    }
    /// Only the exact concrete suffix survives both descriptor and ownership
    /// proof. A matching ordinary body never acquires vector identity.
    pub(super) fn vector_operation(
        &self,
        id: hir::DefId,
    ) -> Option<crate::frontend::builtin_catalog::vector::Function> {
        self.program.vector_operation_claim(id)
    }
    pub(super) fn has_experimental_operations(&self) -> bool {
        self.program.builtins.experimental().functions() != 0
            || !self.program.vector_functions.is_empty()
    }
    pub(super) fn builtin_function(&self) -> Option<hir::DefId> {
        let rank = self
            .program
            .builtins
            .function_rank(BuiltinFunction::ReadStdin)?;
        self.program
            .fixed_builtin_base()?
            .checked_add(rank)
            .map(hir::DefId)
    }
    pub(super) fn builtin_enumeration(&self) -> Option<EnumId> {
        let rank = self.program.builtins.enum_rank(BuiltinEnum::ReadStatus)?;
        self.program
            .enums
            .len()
            .checked_sub(self.program.builtins.extra_enums())?
            .checked_add(rank)
            .map(EnumId)
    }
    pub(super) fn builtin_output_function(&self) -> Option<hir::DefId> {
        let rank = self
            .program
            .builtins
            .function_rank(BuiltinFunction::WriteStdout)?;
        self.program
            .fixed_builtin_base()?
            .checked_add(rank)
            .map(hir::DefId)
    }
    pub(super) fn builtin_output_enumeration(&self) -> Option<EnumId> {
        let rank = self.program.builtins.enum_rank(BuiltinEnum::WriteStatus)?;
        self.program
            .enums
            .len()
            .checked_sub(self.program.builtins.extra_enums())?
            .checked_add(rank)
            .map(EnumId)
    }
    pub(super) fn has_builtin_origins(&self) -> bool {
        self.program.builtins != BuiltinOrigins::None
    }
    pub(super) fn functions(&self) -> &[RawOwnedFunction] {
        &self.program.functions
    }
    pub(super) fn declarations(&self) -> &Declarations {
        &self.declarations
    }
    pub(super) fn usage(&self) -> OwnershipUsage {
        self.usage
    }
}
pub(super) fn verify_owned(
    raw: RawOwnedProgram,
    sources: &SourceMap,
) -> Result<VerifiedOwnedProgram, OwnedFailure> {
    verify_with_limits(raw, sources, budget::Limits::DEFAULT)
}
pub(super) fn verify_with_limits(
    raw: RawOwnedProgram,
    sources: &SourceMap,
    limits: budget::Limits,
) -> Result<VerifiedOwnedProgram, OwnedFailure> {
    let (mut usage, declarations, mut meter) = prepare(&raw, sources, limits)?;
    // Retain the pre-existing fixed carrier-inventory work charges for all
    // programs. Array admission still requires every authoritative check below.
    inventory_carriers(&raw, &mut meter)?;
    validate(&raw, &declarations, sources, &mut usage, &mut meter)?;
    Ok(VerifiedOwnedProgram {
        program: raw,
        declarations,
        usage,
        seal: OwnershipSeal,
    })
}

/// Only the consuming source-association wrapper can select conversion authority.
pub(super) fn verify_associated(
    associated: super::source::association::AssociatedOwned<'_>,
) -> Result<VerifiedOwnedProgram, OwnedFailure> {
    let (raw, sources) = associated.into_parts();
    let (mut usage, declarations, mut meter) = prepare(&raw, sources, budget::Limits::DEFAULT)?;
    inventory_carriers(&raw, &mut meter)?;
    validate_proof_impl(&raw, &declarations, sources, &mut usage, &mut meter, true)?;
    Ok(VerifiedOwnedProgram {
        program: raw,
        declarations,
        usage,
        seal: OwnershipSeal,
    })
}

fn prepare(
    raw: &RawOwnedProgram,
    sources: &SourceMap,
    limits: budget::Limits,
) -> Result<(OwnershipUsage, Declarations, budget::Meter), OwnedFailure> {
    let usage = budget::preflight(raw, limits)?;
    // Canonical builtin descriptors are an additional untrusted-raw check;
    // declarations and every ordinary shape/CFG/ownership pass still follow.
    builtins::check(raw)?;
    let declarations = Declarations::check_combined(&raw.records, &raw.enums, sources)?;
    let mut meter = budget::Meter {
        visits: 0,
        ceiling: usage.work,
    };
    for _ in 0..builtins::descriptor_visits(raw.builtins) {
        meter.visit()?;
    }
    for _ in 0..builtins::vector_descriptor_visits(&raw.vector_functions)? {
        meter.visit()?;
    }
    Ok((usage, declarations, meter))
}

/// One authoritative continuation for production and the non-executable test
/// probe. It never constructs a seal and cannot return an executable value.
fn validate(
    raw: &RawOwnedProgram,
    declarations: &Declarations,
    sources: &SourceMap,
    usage: &mut OwnershipUsage,
    meter: &mut budget::Meter,
) -> Result<(), OwnedFailure> {
    validate_proof(raw, declarations, sources, usage, meter)
}

// Enum consumers share this authoritative proof. Source production still has
// no enum syntax/HIR/lowering route; no alternate witness path is introduced.
fn validate_proof(
    raw: &RawOwnedProgram,
    declarations: &Declarations,
    sources: &SourceMap,
    usage: &mut OwnershipUsage,
    meter: &mut budget::Meter,
) -> Result<(), OwnedFailure> {
    validate_proof_impl(raw, declarations, sources, usage, meter, false)
}
fn validate_proof_impl(
    raw: &RawOwnedProgram,
    declarations: &Declarations,
    sources: &SourceMap,
    usage: &mut OwnershipUsage,
    meter: &mut budget::Meter,
    source_associated: bool,
) -> Result<(), OwnedFailure> {
    shape::signatures(raw, declarations, sources)?;
    // Check every instruction in every function before accepting reachability
    // or any ownership result, including malformed unreachable operations.
    for f in &raw.functions {
        shape::check_with_conversion_authority(
            f,
            raw,
            declarations,
            sources,
            meter,
            source_associated,
        )?;
    }
    for f in &raw.functions {
        super::super::verify::cfg(f)?;
    }
    for f in &raw.functions {
        for owner in &f.owners {
            usage.owner_cells = budget::add(
                usage.owner_cells,
                declarations.aggregate_width(owner.aggregate())?,
            )?;
            usage.owner_layout_bytes = budget::add(
                usage.owner_layout_bytes,
                declarations.aggregate_layout(owner.aggregate())?.size(),
            )?;
        }
        if budget::active(f) {
            let checked = shape::check_with_conversion_authority(
                f,
                raw,
                declarations,
                sources,
                meter,
                source_associated,
            )?;
            flow::check(f, &checked, meter)?;
        }
    }
    Ok(())
}

/// Non-executable enum observation: only inert usage or denial escapes. This
/// uses the exact production proof, with no enum witness, plan, raw data,
/// declarations or consumer callback exposed.
#[cfg(test)]
pub(super) fn probe_enum_validation(
    raw: &RawOwnedProgram,
    sources: &SourceMap,
    limits: budget::Limits,
) -> Result<OwnershipUsage, OwnedFailure> {
    let (mut usage, declarations, mut meter) = prepare(raw, sources, limits)?;
    inventory_carriers(raw, &mut meter)?;
    validate_proof(raw, &declarations, sources, &mut usage, &mut meter)?;
    Ok(usage)
}

/// Closed output observation shares the authoritative proof, but returns only
/// inert usage. It cannot construct a witness or enable an effect consumer.
#[cfg(test)]
pub(super) fn probe_output_validation(
    raw: &RawOwnedProgram,
    sources: &SourceMap,
    limits: budget::Limits,
) -> Result<OwnershipUsage, OwnedFailure> {
    let (mut usage, declarations, mut meter) = prepare(raw, sources, limits)?;
    inventory_carriers(raw, &mut meter)?;
    validate_proof(raw, &declarations, sources, &mut usage, &mut meter)?;
    Ok(usage)
}

/// Observe the authoritative checks without acquiring execution authority.
/// Only unprivileged usage or a failure escapes; neither raw data, declarations,
/// a plan nor an executable witness is returned, even under cfg(test).
#[cfg(test)]
pub(super) fn probe_array_validation(
    raw: &RawOwnedProgram,
    sources: &SourceMap,
    limits: budget::Limits,
) -> Result<OwnershipUsage, OwnedFailure> {
    let (mut usage, declarations, mut meter) = prepare(raw, sources, limits)?;
    validate(raw, &declarations, sources, &mut usage, &mut meter)?;
    Ok(usage)
}

/// Qualification runs the real reference consumer while the witness remains
/// sealed inside this trusted boundary. No privileged object can escape.
#[cfg(test)]
pub(super) fn probe_array_reference(
    raw: RawOwnedProgram,
    sources: &SourceMap,
    verification_limits: budget::Limits,
    entry: Option<hir::DefId>,
    execution_limits: execute::Limits,
    observation: execute::ObservationControl,
) -> Result<execute::ReferenceObservation, OwnedFailure> {
    let (mut usage, declarations, mut meter) = prepare(&raw, sources, verification_limits)?;
    validate(&raw, &declarations, sources, &mut usage, &mut meter)?;
    let witness = VerifiedOwnedProgram {
        program: raw,
        declarations,
        usage,
        seal: OwnershipSeal,
    };
    Ok(execute::run_array_observed(
        &witness,
        entry,
        execution_limits,
        observation,
    ))
}

/// Qualification invokes the complete real native consumer synchronously while
/// the authoritative witness stays inside this boundary. Only bounded emitted
/// text/error and inert accounting escape; no callback receives privileged data.
#[cfg(test)]
pub(super) fn probe_array_native(
    raw: RawOwnedProgram,
    verification_sources: &SourceMap,
    verification_limits: budget::Limits,
    entry: Option<hir::DefId>,
    rendering_sources: &SourceMap,
    control: native::NativeControl,
) -> Result<native::NativeObservation, OwnedFailure> {
    let (mut usage, declarations, mut meter) =
        prepare(&raw, verification_sources, verification_limits)?;
    validate(
        &raw,
        &declarations,
        verification_sources,
        &mut usage,
        &mut meter,
    )?;
    let witness = VerifiedOwnedProgram {
        program: raw,
        declarations,
        usage,
        seal: OwnershipSeal,
    };
    Ok(native::run_array_observed(
        &witness,
        entry,
        rendering_sources,
        control,
    ))
}

/// Preserve the fixed carrier-inventory charge used before array activation.
/// Shape, identity, availability and loan checks remain mandatory in `validate`,
/// including every malformed instruction in unreachable blocks.
fn inventory_carriers(
    raw: &RawOwnedProgram,
    meter: &mut budget::Meter,
) -> Result<(), OwnedFailure> {
    for enumeration in &raw.enums {
        meter.visit()?;
        for _ in &enumeration.variants {
            meter.visit()?;
        }
    }
    for f in &raw.functions {
        for descriptor in &f.matches {
            meter.visit()?;
            for _ in &descriptor.arms {
                meter.visit()?;
            }
        }
        for _ in &f.owners {
            meter.visit()?;
        }
        for _ in &f.references {
            meter.visit()?;
        }
        for _ in &f.loans {
            meter.visit()?;
        }
    }
    Ok(())
}

/// Named transport inventory for the new proof-dispatch layer on both bare and
/// associated routes. The inherited raw `Limits::scratch` and `metadata` model
/// requested vector/table payload, not inline Rust call stacks. Accordingly this
/// measurement is separate; the source HirPlan is not claimed to cover bare raw
/// verification. There are no new raw-route heap allocations or retained rows.
#[allow(dead_code)]
struct ConversionProofDispatchCarriers {
    raw: &'static RawOwnedProgram,
    declarations: &'static Declarations,
    sources: &'static SourceMap,
    usage: &'static mut OwnershipUsage,
    meter: &'static mut budget::Meter,
    selected: bool,
    validation_return: Result<(), OwnedFailure>,
    function: &'static RawOwnedFunction,
    shape_raw: &'static RawOwnedProgram,
    shape_declarations: &'static Declarations,
    shape_sources: &'static SourceMap,
    shape_meter: &'static mut budget::Meter,
    shape_selected: bool,
    shape_return: Result<shape::Shape, OwnedFailure>,
    rejected: OirFailure,
    normalized: OwnedFailure,
}
#[test]
fn owned_u8_raw_proof_dispatch_carriers_are_separate_from_payload_limits() {
    println!("RFC0030 owned raw proof dispatch inline_bytes={} align={} raw_scratch_scope=requested_vector_payload raw_metadata_scope=requested_table_payload",
        std::mem::size_of::<ConversionProofDispatchCarriers>(), std::mem::align_of::<ConversionProofDispatchCarriers>());
    assert!(std::mem::size_of::<ConversionProofDispatchCarriers>() > 0);
}

#[test]
fn vector_descriptor_preflight_visits_are_debited_by_the_actual_meter() {
    let (sources, raw) = super::vector_raw_tests::one_element_program(hir::Ty::I64);
    let (_, _, meter) = prepare(&raw, &sources, budget::Limits::DEFAULT).unwrap();
    // Nine independently authored vector bodies: 106 descriptor visits and
    // 36 prior-key comparisons. GrowStatus adds a header plus its four variants
    // and the existing fixed-inventory header: six more visits.
    assert_eq!(meter.visits, 148);
}
