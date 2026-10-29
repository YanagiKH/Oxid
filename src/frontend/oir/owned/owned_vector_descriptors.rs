//! Owning inert extension claims and independent canonical suffix checks.
//! No successful query here constructs an ownership or source witness.
use super::*;
use crate::frontend::{
    builtin_catalog::{
        experimental::Enumeration,
        owned_vector::{Function, Item, Operation, Signature},
        vector,
    },
    oir::owned_types::{
        checked_declaration_byte_limit, checked_declaration_temporary_request,
        owned_vector::OwnedRecordVectorType, retained_declaration_control_bytes,
    },
    project::budget::{Allocator, ReserveFailure},
};
use std::mem::size_of;

super::vector_descriptors::canonical_function!(owned_function, Function, Signature, OwnedVector);

#[path = "owned_vector_context.rs"]
pub(super) mod context;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct OwnedVectorImportClaims(u8);
impl OwnedVectorImportClaims {
    pub(super) const fn empty() -> Self {
        Self(0)
    }
    pub(super) const fn admit(self, item: Item) -> Self {
        Self(
            self.0
                | match item {
                    Item::PushReserved => 1,
                    Item::PopLast => 2,
                },
        )
    }
    fn valid(self) -> bool {
        self.0 & !3 == 0
    }
    fn contains(self, operation: Operation, origin: BuiltinOrigins) -> bool {
        match operation.shared_template() {
            Some(template) => origin.contains_vector(template.item()),
            None => {
                self.0
                    & match operation {
                        Operation::PushReserved => 1,
                        Operation::PopLast => 2,
                        Operation::New
                        | Operation::Len
                        | Operation::Capacity
                        | Operation::TryReserve
                        | Operation::Truncate
                        | Operation::Take => return false,
                    }
                    != 0
            }
        }
    }
}

#[derive(Debug)]
struct RawOwnedVectorClaims {
    imports: OwnedVectorImportClaims,
    functions: Vec<Function>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FunctionDomains {
    fixed_start: usize,
    old_start: usize,
    owned_start: usize,
    end: usize,
}
impl FunctionDomains {
    fn check(raw: &RawOwnedProgram, owned: usize) -> Result<Self, OwnedFailure> {
        let end = raw.functions.len();
        let owned_start = end.checked_sub(owned).ok_or_else(bad)?;
        let old_start = owned_start
            .checked_sub(raw.vector_functions.len())
            .ok_or_else(bad)?;
        let fixed_start = old_start
            .checked_sub(raw.builtins.extra_functions())
            .ok_or_else(bad)?;
        Ok(Self {
            fixed_start,
            old_start,
            owned_start,
            end,
        })
    }
    fn owned_at(self, rank: usize) -> Option<hir::DefId> {
        let id = self.owned_start.checked_add(rank)?;
        (id < self.end).then_some(hir::DefId(id))
    }
    fn old_at(self, rank: usize) -> Option<hir::DefId> {
        let id = self.old_start.checked_add(rank)?;
        (id < self.owned_start).then_some(hir::DefId(id))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CandidateUsage {
    pub(super) metadata_bytes: usize,
    pub(super) work: usize,
    pub(super) owned_functions: usize,
    pub(super) domains: FunctionDomains,
}

/// The core carrier and its key Vec remain unchanged. The wrapper owns the
/// same-input checked declarations, so no reusable detached permit exists.
#[derive(Debug)]
pub(super) struct RawOwnedVectorProgram {
    program: RawOwnedProgram,
    claims: RawOwnedVectorClaims,
    declarations: Option<Declarations>,
    usage: CandidateUsage,
}

fn bad() -> OwnedFailure {
    OwnedFailure::malformed(Malformed::Binding, Origin::NONE.0)
}
fn resource(name: &'static str) -> OwnedFailure {
    OwnedFailure::resource(name)
}
fn add(a: usize, b: usize) -> Result<usize, OwnedFailure> {
    budget::add(a, b)
}
fn mul(a: usize, b: usize) -> Result<usize, OwnedFailure> {
    budget::mul(a, b)
}

/// Real callee/caller/return and construction roles for the new wrapper.
/// The opaque old declaration helper gets its full original allowance before
/// it runs. This bank is additional to the unchanged core raw accounting.
struct FactoryControls {
    program_argument: RawOwnedProgram,
    input: &'static [Function],
    imports: OwnedVectorImportClaims,
    limits: [budget::Limits; 2],
    allocator: &'static mut Allocator,
    vector: Vec<Function>,
    reservation: Result<(), ReserveFailure>,
    source_argument: &'static SourceMap,
    declaration_return: Result<Declarations, DeclarationError>,
    selected_declarations: Option<Declarations>,
    constructed: RawOwnedVectorProgram,
    returned: Result<RawOwnedVectorProgram, OwnedFailure>,
    caller_return: Result<RawOwnedVectorProgram, OwnedFailure>,
    retained_limit: usize,
    retained_result: Result<usize, DeclarationError>,
    mapped_retained_result: Result<usize, OwnedFailure>,
    selected_retained: usize,
    prepaid_retained: Result<usize, OwnedFailure>,
    type_inventory: [Result<(bool, usize), OwnedFailure>; 2],
    needs_facade: bool,
    type_visits: usize,
    core_usage: OwnershipUsage,
    core_return: Result<OwnershipUsage, OwnedFailure>,
    domains: FunctionDomains,
    domains_return: Result<FunctionDomains, OwnedFailure>,
    usage: CandidateUsage,
    usage_returns: [Result<CandidateUsage, OwnedFailure>; 2],
    checked_counts: [Option<usize>; 8],
    size_results: [Result<usize, OwnedFailure>; 8],
    selected_counts: [usize; 12],
}

/// The public internal factory delegates to the independently bounded
/// retention helper. These are that real caller's argument and return roles.
struct PreparationCallControls {
    program: RawOwnedProgram,
    imports: OwnedVectorImportClaims,
    keys: &'static [Function],
    sources: &'static SourceMap,
    limits: budget::Limits,
    allocator: &'static mut Allocator,
    requested_limit: Result<usize, OwnedFailure>,
    selected_limit: usize,
    helper_return: Result<RawOwnedVectorProgram, OwnedFailure>,
    returned: Result<RawOwnedVectorProgram, OwnedFailure>,
}

struct DeclarationRequestControls {
    raw: &'static RawOwnedProgram,
    records: std::slice::Iter<'static, RawRecordDecl>,
    enumerations: std::slice::Iter<'static, RawEnumDecl>,
    record: &'static RawRecordDecl,
    enumeration: &'static RawEnumDecl,
    declarations: usize,
    members: usize,
    temporary_result: Result<usize, DeclarationError>,
    mapped_temporary: Result<usize, OwnedFailure>,
    selected_temporary: usize,
    member_sums: [Result<usize, OwnedFailure>; 2],
    work_products: [Result<usize, OwnedFailure>; 3],
    work_sums: [Result<usize, OwnedFailure>; 5],
}

struct ImportControls {
    receiver: OwnedVectorImportClaims,
    operation: Operation,
    origin: BuiltinOrigins,
    shared_template: Option<vector::Operation>,
    selected_template: vector::Operation,
    selected_item: vector::Item,
    selected_bit: u8,
    validity_return: bool,
    contains_return: bool,
    caller_return: bool,
}

struct DomainControls {
    receiver: FunctionDomains,
    raw: &'static RawOwnedProgram,
    counts: [usize; 5],
    checked_arithmetic: [Option<usize>; 4],
    constructed: FunctionDomains,
    result: Result<FunctionDomains, OwnedFailure>,
    caller_result: Result<FunctionDomains, OwnedFailure>,
    selected_id: Option<hir::DefId>,
    caller_id: Option<hir::DefId>,
}

/// Actual synchronous canonical-check roles, including the shared predicate
/// instantiation with this domain's real key and concrete signature types.
struct CheckControls {
    wrapper: &'static RawOwnedVectorProgram,
    sources: &'static SourceMap,
    limits: budget::Limits,
    ids: [builtins::BuiltinIds; 2],
    ids_results: [Result<builtins::BuiltinIds, OwnedFailure>; 2],
    keys: std::iter::Enumerate<std::slice::Iter<'static, Function>>,
    duplicate_prefix: std::slice::Iter<'static, Function>,
    key: [Function; 2],
    operation: Operation,
    domains: FunctionDomains,
    selected_id: Option<hir::DefId>,
    status: Option<EnumId>,
    signatures: [Signature; 2],
    parameters: std::iter::Enumerate<
        std::iter::Zip<
            std::slice::Iter<'static, ParameterBinding>,
            std::slice::Iter<'static, ParameterTy>,
        >,
    >,
    parameter_values: [ParameterTy; 2],
    parameter_bindings: [ParameterBinding; 2],
    selected_function: &'static RawOwnedFunction,
    function_argument: &'static RawOwnedFunction,
    signature_argument: &'static Signature,
    operation_argument: Function,
    function_option: Option<&'static RawOwnedFunction>,
    selected_owner: &'static OwnerDecl,
    owner_option: Option<&'static OwnerDecl>,
    selected_reference: &'static ReferenceDecl,
    reference_option: Option<&'static ReferenceDecl>,
    expected_locals: [LocalDecl; 2],
    expected_owner: OwnerKind,
    selected_block: &'static OwnedBlock,
    statements: std::slice::Iter<'static, OwnedStatement>,
    selected_terminator: &'static OwnedTerminator,
    terminator_option: Option<&'static OwnedTerminator>,
    destinations: [(CallResult, bool); 2],
    descriptor_results: [Result<(), OwnedFailure>; 3],
    failure_origin_capture: Span,
    selected_failure: OwnedFailure,
    failure_result: Result<(), OwnedFailure>,
    nominal: OwnedRecordVectorType<'static>,
    nominal_results: [Result<OwnedRecordVectorType<'static>, DeclarationError>; 2],
    declaration: &'static Declarations,
    query_work: usize,
    function_iteration: std::iter::Enumerate<std::slice::Iter<'static, RawOwnedFunction>>,
    block_iteration: std::slice::Iter<'static, OwnedBlock>,
    statement_iteration: std::slice::Iter<'static, OwnedStatement>,
    usage: CandidateUsage,
    usage_return: Result<CandidateUsage, OwnedFailure>,
    caller_return: Result<CandidateUsage, OwnedFailure>,
    checked_values: [Option<usize>; 8],
    selected_counts: [usize; 12],
}

fn control_bytes() -> Result<usize, OwnedFailure> {
    add(
        add(
            add(
                add(size_of::<FactoryControls>(), size_of::<CheckControls>())?,
                add(
                    size_of::<PreparationCallControls>(),
                    size_of::<DeclarationRequestControls>(),
                )?,
            )?,
            add(size_of::<ImportControls>(), size_of::<DomainControls>())?,
        )?,
        add(
            OwnedRecordVectorType::requested_query_bytes(),
            add(
                retained_declaration_control_bytes(),
                context::raw_type_usage_control_bytes(),
            )?,
        )?,
    )
}

fn requested(
    raw: &RawOwnedProgram,
    keys: usize,
    capacity: usize,
    base: OwnershipUsage,
) -> Result<CandidateUsage, OwnedFailure> {
    if raw.records.len() > 4096 || keys > mul(raw.records.len(), 8)? || capacity < keys {
        return Err(bad());
    }
    let domains = FunctionDomains::check(raw, keys)?;
    let (type_mentions, type_visits) = context::raw_type_usage(raw)?;
    let needs_facade = keys != 0 || type_mentions;
    let mut metadata = add(base.metadata_bytes, size_of::<RawOwnedVectorClaims>())?;
    metadata = add(metadata, mul(capacity, size_of::<Function>())?)?;
    metadata = add(metadata, control_bytes()?)?;
    if needs_facade {
        metadata = add(metadata, checked_declaration_byte_limit())?;
        metadata = add(metadata, size_of::<Declarations>())?;
        metadata = add(
            metadata,
            checked_declaration_temporary_request(raw.records.len(), raw.enums.len())?,
        )?;
    }
    // All raw nested lengths were independently bounded by core preflight.
    // Count the added placement scan and its bounded header preflight twice.
    let mut placements = mul(raw.functions.len(), 2)?;
    for function in &raw.functions {
        placements = add(placements, mul(function.blocks.len(), 2)?)?;
        for block in &function.blocks {
            placements = add(placements, block.statements.len())?;
        }
    }
    // At most two parameters, two canonical statements, a terminator and the
    // three nominal queries fit these independently stated 32 visits/key.
    let duplicates = mul(keys, keys.saturating_sub(1))? / 2;
    let mut work = add(
        add(add(base.work, placements)?, duplicates)?,
        add(mul(keys, 32)?, 8)?,
    )?;
    work = add(work, mul(type_visits, 3)?)?;
    if needs_facade {
        let declarations = add(raw.records.len(), raw.enums.len())?;
        let mut members = 0usize;
        for record in &raw.records {
            members = add(members, record.fields.len())?;
        }
        for enumeration in &raw.enums {
            members = add(members, enumeration.variants.len())?;
        }
        // Additional logical declaration validation/layout visits, distinct
        // from the existing raw row work. No expanded containment is walked.
        work = add(
            work,
            add(
                mul(add(add(declarations, members)?, 1)?, 32)?,
                add(mul(declarations, 2)?, 64)?,
            )?,
        )?;
    }
    Ok(CandidateUsage {
        metadata_bytes: metadata,
        work,
        owned_functions: keys,
        domains,
    })
}

fn admit(usage: CandidateUsage, limits: budget::Limits) -> Result<(), OwnedFailure> {
    if usage.metadata_bytes > limits.metadata.min(budget::Limits::DEFAULT.metadata) {
        return Err(resource("owned vector candidate metadata"));
    }
    if usage.work > limits.work.min(budget::Limits::DEFAULT.work) {
        return Err(resource("owned vector candidate work"));
    }
    Ok(())
}

impl RawOwnedVectorProgram {
    /// A zero extension keeps the original raw carrier and verifier. Only the
    /// private preparation factory can leave declarations absent, and it does
    /// so only after its complete type inventory found no new-domain usage.
    pub(in crate::frontend::oir::owned) fn try_into_legacy(self) -> Result<RawOwnedProgram, Self> {
        if self.declarations.is_none() && self.claims.functions.is_empty() {
            Ok(self.program)
        } else {
            Err(self)
        }
    }
    pub(super) fn requested(
        program: &RawOwnedProgram,
        keys: usize,
        limits: budget::Limits,
    ) -> Result<CandidateUsage, OwnedFailure> {
        let base = budget::preflight(program, limits)?;
        requested(program, keys, keys, base)
    }

    pub(super) fn prepare(
        program: RawOwnedProgram,
        imports: OwnedVectorImportClaims,
        keys: &[Function],
        sources: &SourceMap,
        limits: budget::Limits,
        allocator: &mut Allocator,
    ) -> Result<Self, OwnedFailure> {
        Self::prepare_with_retained_limit(
            program,
            imports,
            keys,
            sources,
            limits,
            add(checked_declaration_byte_limit(), size_of::<Declarations>())?,
            allocator,
        )
    }

    fn prepare_with_retained_limit(
        program: RawOwnedProgram,
        imports: OwnedVectorImportClaims,
        keys: &[Function],
        sources: &SourceMap,
        limits: budget::Limits,
        retained_limit: usize,
        allocator: &mut Allocator,
    ) -> Result<Self, OwnedFailure> {
        if !imports.valid() {
            return Err(bad());
        }
        let base = budget::preflight(&program, limits)?;
        let initial = requested(&program, keys.len(), keys.len(), base)?;
        admit(initial, limits)?;
        let mut functions = Vec::new();
        if !keys.is_empty() {
            allocator
                .vector_exact(&mut functions, keys.len(), "owned vector raw key claims")
                .map_err(|_| resource("owned vector raw key allocation"))?;
        }
        let usage = requested(&program, keys.len(), functions.capacity(), base)?;
        admit(usage, limits)?;
        functions.extend_from_slice(keys);
        let needs_facade = !keys.is_empty() || context::raw_type_usage(&program)?.0;
        let declarations = if !needs_facade {
            None
        } else {
            let declarations = Declarations::check_combined_owned_vectors(
                &program.records,
                &program.enums,
                sources,
            )?;
            let prepaid = add(checked_declaration_byte_limit(), size_of::<Declarations>())?;
            if declarations.retained_storage_bytes()? > retained_limit.min(prepaid) {
                return Err(resource("owned vector returned declaration storage"));
            }
            Some(declarations)
        };
        Ok(Self {
            program,
            claims: RawOwnedVectorClaims { imports, functions },
            declarations,
            usage,
        })
    }

    /// Matching canonical rows yield inert usage only. Ordinary declaration,
    /// shape/flow, source association and both consumers remain separate gates.
    pub(super) fn check(
        &self,
        sources: &SourceMap,
        limits: budget::Limits,
    ) -> Result<CandidateUsage, OwnedFailure> {
        admit(self.usage, limits)?;
        let raw = &self.program;
        let ids = builtins::check_candidate_prefix(raw, self.usage.domains.owned_start)?;
        if !self.claims.imports.valid()
            || (!self.claims.functions.is_empty()
                && !raw.builtins.contains_vector(vector::Item::Vec))
        {
            return Err(bad());
        }
        for (rank, operation) in self.claims.functions.iter().copied().enumerate() {
            if self.claims.functions[..rank].contains(&operation)
                || !self
                    .claims
                    .imports
                    .contains(operation.operation(), raw.builtins)
            {
                return Err(bad());
            }
            let declarations = self.declarations.as_ref().ok_or_else(bad)?;
            let mut query_work = 3;
            let nominal = OwnedRecordVectorType::check(
                declarations,
                operation.record_id(),
                OwnedRecordVectorType::requested_query_bytes(),
                &mut query_work,
            )?;
            if nominal.aggregate() != AggregateTy::OwnedRecordVector(operation.record_id())
                || !nominal.belongs_to(declarations)
            {
                return Err(bad());
            }
            let grow = if operation.operation().requires_grow_status() {
                ids.experimental_enum(Enumeration::Grow).ok_or_else(bad)?
            } else {
                EnumId(0)
            };
            let id = self.usage.domains.owned_at(rank).ok_or_else(bad)?;
            let function = raw.functions.get(id.0).ok_or_else(bad)?;
            if !sources.is_valid_span(function.span) {
                return Err(bad());
            }
            owned_function(function, id, operation, &operation.concrete_signature(grow))?;
        }
        for (index, function) in raw.functions.iter().enumerate() {
            for block in &function.blocks {
                for statement in &block.statements {
                    if let OwnedInstruction::OwnedVector { operation, .. } = statement.kind {
                        let rank = index
                            .checked_sub(self.usage.domains.owned_start)
                            .ok_or_else(bad)?;
                        if self.claims.functions.get(rank) != Some(&operation) {
                            return Err(bad());
                        }
                    }
                }
            }
        }
        Ok(self.usage)
    }
}

#[cfg(test)]
#[path = "owned_vector_descriptor_tests.rs"]
pub(in crate::frontend::oir::owned) mod tests;
