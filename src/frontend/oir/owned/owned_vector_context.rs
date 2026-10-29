//! Inert, borrowed program/facade/bounds association. No proof or consumer
//! accepts this context as an ownership seal or source association.
use super::*;
use crate::frontend::oir::owned::owned_record_codec::{
    bounds::{CheckedElements, ShapeFailure},
    Element,
};

#[path = "owned_vector_queries.rs"]
pub(super) mod queries;

#[path = "shape_declaration_queries.rs"]
pub(in crate::frontend::oir::owned) mod signature_queries;

#[path = "owned_vector_proof.rs"]
mod ordinary;

#[path = "owned_vector_verified.rs"]
pub(in crate::frontend::oir::owned) mod owning;

/// Static identity queries shared by the two concrete key domains. Raw
/// implementations report claims; the context authenticates canonical rows.
/// Neither supplies ordinary ownership proof. Callers price these allocation-
/// free lookups in their own scope.
pub(in crate::frontend::oir::owned) trait ProgramDomainQueries {
    fn fixed_builtin_base(&self) -> Option<usize>;
    fn old_vector_operation(&self, id: hir::DefId) -> Option<vector::Function>;
    fn owned_vector_operation(&self, id: hir::DefId) -> Option<Function>;
}

impl ProgramDomainQueries for RawOwnedProgram {
    fn fixed_builtin_base(&self) -> Option<usize> {
        RawOwnedProgram::fixed_builtin_base(self)
    }
    fn old_vector_operation(&self, id: hir::DefId) -> Option<vector::Function> {
        self.vector_operation_claim(id)
    }
    fn owned_vector_operation(&self, _: hir::DefId) -> Option<Function> {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend::oir::owned) struct ContextRequest {
    pub(in crate::frontend::oir::owned) metadata_bytes: usize,
    pub(in crate::frontend::oir::owned) work: usize,
    bounds_bytes: usize,
}

/// A single mutable remaining ledger, tied to its owning wrapper. Scope
/// admission subtracts from this ledger rather than inventing a fresh limit.
pub(in crate::frontend::oir::owned) struct RemainingContextBudget<'a> {
    wrapper: &'a RawOwnedVectorProgram,
    limits: budget::Limits,
    metadata: usize,
    work: usize,
}

pub(in crate::frontend::oir::owned) struct OwnedVectorContext<'a> {
    wrapper: &'a RawOwnedVectorProgram,
    elements: Option<CheckedElements<'a>>,
    charged_metadata: usize,
    bounds_retained: usize,
}

/// These are actual factory, scope-helper, query and caller return roles.
/// The bounds factory's own controls are added only for a nonempty extension.
struct ContextControls {
    wrapper_arguments: [&'static RawOwnedVectorProgram; 3],
    source_arguments: [&'static SourceMap; 2],
    budget_argument: &'static mut RemainingContextBudget<'static>,
    budget_constructed: RemainingContextBudget<'static>,
    budget_return: Result<RemainingContextBudget<'static>, OwnedFailure>,
    budget_caller_return: Result<RemainingContextBudget<'static>, OwnedFailure>,
    limits: [budget::Limits; 3],
    allocator_arguments: [&'static mut Allocator; 2],
    requests: [ContextRequest; 2],
    request_returns: [Result<ContextRequest, OwnedFailure>; 2],
    checked_wrapper: Result<CandidateUsage, OwnedFailure>,
    facade: &'static Declarations,
    facade_option: Option<&'static Declarations>,
    facade_result: Result<&'static Declarations, OwnedFailure>,
    bounds_request: Result<usize, ShapeFailure>,
    mapped_bounds_request: Result<usize, OwnedFailure>,
    bounds_result: Result<CheckedElements<'static>, ShapeFailure>,
    mapped_bounds_result: Result<CheckedElements<'static>, OwnedFailure>,
    selected_bounds: CheckedElements<'static>,
    optional_bounds: Option<CheckedElements<'static>>,
    borrowed_bounds: Option<&'static CheckedElements<'static>>,
    checked_borrowed_bounds: Result<&'static CheckedElements<'static>, OwnedFailure>,
    retained_result: Result<usize, ShapeFailure>,
    mapped_retained_result: Result<usize, OwnedFailure>,
    constructed: OwnedVectorContext<'static>,
    scope_return: Result<OwnedVectorContext<'static>, OwnedFailure>,
    selected_scope_return: Result<OwnedVectorContext<'static>, OwnedFailure>,
    factory_return: Result<OwnedVectorContext<'static>, OwnedFailure>,
    caller_return: Result<OwnedVectorContext<'static>, OwnedFailure>,
    remaining_result: (usize, usize),
    remaining_caller_result: (usize, usize),
    selected_failure: OwnedFailure,
    key_iteration: std::slice::Iter<'static, Function>,
    key: Function,
    function: hir::DefId,
    selected_key: Option<Function>,
    key_result: Result<Function, OwnedFailure>,
    element: Element<'static>,
    element_result: Result<Element<'static>, ShapeFailure>,
    mapped_element_result: Result<Element<'static>, OwnedFailure>,
    caller_element_result: Result<Element<'static>, OwnedFailure>,
    identity_comparisons: [bool; 3],
    inventory: Inventory,
    inventory_result: Result<Inventory, OwnedFailure>,
    type_validator: &'static CheckedElements<'static>,
    type_validator_facade: &'static Declarations,
    type_validator_capture: (&'static CheckedElements<'static>, &'static Declarations),
    type_validation_result: Result<(), OwnedFailure>,
    type_validation_caller_result: Result<(), OwnedFailure>,
    checked_counts: [Option<usize>; 8],
    checked_results: [Result<usize, OwnedFailure>; 8],
    sizes: [usize; 16],
}

struct InventoryControls {
    raw: &'static RawOwnedProgram,
    records: std::slice::Iter<'static, RawRecordDecl>,
    record: &'static RawRecordDecl,
    fields: std::slice::Iter<'static, RawFieldDecl>,
    field: &'static RawFieldDecl,
    enumerations: std::slice::Iter<'static, RawEnumDecl>,
    enumeration: &'static RawEnumDecl,
    variants: std::slice::Iter<'static, RawVariantDecl>,
    variant: &'static RawVariantDecl,
    payload: Option<ParameterTy>,
    functions: std::slice::Iter<'static, RawOwnedFunction>,
    function: &'static RawOwnedFunction,
    owners: std::slice::Iter<'static, OwnerDecl>,
    owner: &'static OwnerDecl,
    references: std::slice::Iter<'static, ReferenceDecl>,
    reference: &'static ReferenceDecl,
    loans: std::slice::Iter<'static, LoanDecl>,
    loan: &'static LoanDecl,
    blocks: std::slice::Iter<'static, OwnedBlock>,
    block: &'static OwnedBlock,
    statements: std::slice::Iter<'static, OwnedStatement>,
    statement: &'static OwnedStatement,
    value: ValueTy,
    parameter: ParameterTy,
    borrowed: BorrowedTy,
    aggregate: AggregateTy,
    mentions: [bool; 4],
    result: Result<Inventory, OwnedFailure>,
    caller_result: Result<Inventory, OwnedFailure>,
    selected_record: Option<RecordId>,
    record_result: Result<(), OwnedFailure>,
    visitor_return: Result<(), OwnedFailure>,
    use_count: Option<usize>,
    type_usage: (bool, usize),
    type_usage_return: Result<(bool, usize), OwnedFailure>,
    type_usage_caller_return: Result<(bool, usize), OwnedFailure>,
}

struct DomainQueryControls {
    raw: &'static RawOwnedProgram,
    context: &'static OwnedVectorContext<'static>,
    function: hir::DefId,
    domains: FunctionDomains,
    checked_offset: Option<usize>,
    selected_offset: usize,
    old_key: Option<vector::Function>,
    old_caller_key: Option<vector::Function>,
    new_key: Option<Function>,
    new_caller_key: Option<Function>,
    base: Option<usize>,
    caller_base: Option<usize>,
}

fn control_bytes() -> Result<usize, OwnedFailure> {
    add(
        add(size_of::<ContextControls>(), size_of::<InventoryControls>())?,
        size_of::<DomainQueryControls>(),
    )
}

fn bounds_failure(failure: ShapeFailure) -> OwnedFailure {
    match failure {
        ShapeFailure::MetadataLimit => resource("owned vector context bounds metadata"),
        ShapeFailure::Allocation => resource("owned vector context bounds allocation"),
        ShapeFailure::Type => bad(),
    }
}

#[derive(Clone, Copy)]
struct Inventory {
    declarations: usize,
    members: usize,
    visits: usize,
    mentions_extension: bool,
    uses: usize,
}

fn vector_record(ty: AggregateTy) -> Option<RecordId> {
    match ty {
        AggregateTy::OwnedRecordVector(record) => Some(record),
        _ => None,
    }
}
fn value_record(ty: ValueTy) -> Option<RecordId> {
    match ty {
        ValueTy::Owned(aggregate) => vector_record(aggregate),
        ValueTy::Scalar(_) => None,
    }
}
fn borrowed_record(ty: BorrowedTy) -> Option<RecordId> {
    match ty {
        BorrowedTy::Exact(aggregate) => vector_record(aggregate),
        BorrowedTy::ScalarSlice(_) | BorrowedTy::RecordSlice(_) => None,
    }
}
fn parameter_record(ty: ParameterTy) -> Option<RecordId> {
    match ty {
        ParameterTy::Value(value) => value_record(value),
        ParameterTy::Reference { referent, .. } => borrowed_record(referent),
    }
}

impl Inventory {
    fn note(
        &mut self,
        record: Option<RecordId>,
        visit: &mut impl FnMut(RecordId) -> Result<(), OwnedFailure>,
    ) -> Result<(), OwnedFailure> {
        if let Some(record) = record {
            self.mentions_extension = true;
            self.uses = add(self.uses, 1)?;
            visit(record)?;
        }
        Ok(())
    }
}

pub(super) fn raw_type_usage(raw: &RawOwnedProgram) -> Result<(bool, usize), OwnedFailure> {
    let inventory = inventory(raw)?;
    Ok((inventory.mentions_extension, inventory.visits))
}

pub(super) const fn raw_type_usage_control_bytes() -> usize {
    size_of::<InventoryControls>()
}

/// No allocation or expansion. These are the complete raw carriers that can
/// contain an owned type; scalar locals/places and call IDs contain no types.
fn inventory(raw: &RawOwnedProgram) -> Result<Inventory, OwnedFailure> {
    inventory_with(raw, |_| Ok(()))
}

fn inventory_with(
    raw: &RawOwnedProgram,
    mut visit: impl FnMut(RecordId) -> Result<(), OwnedFailure>,
) -> Result<Inventory, OwnedFailure> {
    let mut inventory = Inventory {
        declarations: add(raw.records.len(), raw.enums.len())?,
        members: 0,
        visits: 0,
        mentions_extension: false,
        uses: 0,
    };
    for record in &raw.records {
        inventory.visits = add(inventory.visits, 1)?;
        inventory.members = add(inventory.members, record.fields.len())?;
        for field in &record.fields {
            inventory.visits = add(inventory.visits, 1)?;
            inventory.note(parameter_record(field.ty), &mut visit)?;
        }
    }
    for enumeration in &raw.enums {
        inventory.visits = add(inventory.visits, 1)?;
        inventory.members = add(inventory.members, enumeration.variants.len())?;
        for variant in &enumeration.variants {
            inventory.visits = add(inventory.visits, 1)?;
            inventory.note(variant.payload.and_then(parameter_record), &mut visit)?;
        }
    }
    for function in &raw.functions {
        inventory.visits = add(inventory.visits, 1)?;
        inventory.note(value_record(function.result), &mut visit)?;
        for owner in &function.owners {
            inventory.visits = add(inventory.visits, 1)?;
            inventory.note(vector_record(owner.aggregate()), &mut visit)?;
        }
        for reference in &function.references {
            inventory.visits = add(inventory.visits, 1)?;
            inventory.note(borrowed_record(reference.referent()), &mut visit)?;
        }
        for loan in &function.loans {
            inventory.visits = add(inventory.visits, 1)?;
            inventory.note(borrowed_record(loan.referent.referent()), &mut visit)?;
        }
        for block in &function.blocks {
            inventory.visits = add(inventory.visits, 1)?;
            for statement in &block.statements {
                inventory.visits = add(inventory.visits, 1)?;
                if let OwnedInstruction::OwnedVector { operation, .. } = statement.kind {
                    inventory.note(Some(operation.record_id()), &mut visit)?;
                }
            }
        }
    }
    Ok(inventory)
}

impl RawOwnedVectorProgram {
    pub(in crate::frontend::oir::owned) fn remaining_context_budget(
        &self,
        limits: budget::Limits,
    ) -> Result<RemainingContextBudget<'_>, OwnedFailure> {
        admit(self.usage, limits)?;
        Ok(RemainingContextBudget {
            wrapper: self,
            limits,
            metadata: limits
                .metadata
                .min(budget::Limits::DEFAULT.metadata)
                .checked_sub(self.usage.metadata_bytes)
                .ok_or_else(bad)?,
            work: limits
                .work
                .min(budget::Limits::DEFAULT.work)
                .checked_sub(self.usage.work)
                .ok_or_else(bad)?,
        })
    }

    pub(in crate::frontend::oir::owned) fn context_request(
        &self,
    ) -> Result<ContextRequest, OwnedFailure> {
        let inventory = inventory(&self.program)?;
        // A type-only extension needs its real Vec binding, independently of
        // whether this program calls or registers any intrinsic operation.
        if inventory.mentions_extension && !self.program.builtins.contains_vector(vector::Item::Vec)
        {
            return Err(OwnedFailure::malformed(Malformed::Type, Origin::NONE.0));
        }
        let mut request = ContextRequest {
            metadata_bytes: control_bytes()?,
            work: add(add(self.usage.work, mul(inventory.visits, 2)?)?, 16)?,
            bounds_bytes: 0,
        };
        if !self.claims.functions.is_empty() || inventory.mentions_extension {
            let declarations = self.declarations.as_ref().ok_or_else(bad)?;
            request.bounds_bytes =
                CheckedElements::requested_bytes(declarations).map_err(bounds_failure)?;
            request.metadata_bytes = add(request.metadata_bytes, request.bounds_bytes)?;
            // Roots, two initialized table cells/nominal, bounded stack-cell
            // initialization and up to nine logical sites per transition.
            // A direct member is revisited at most once after descending into
            // a new child; no expanded containment graph is traversed.
            let graph = add(
                mul(inventory.declarations, MAX_CONTAINMENT_DEPTH + 13)?,
                mul(inventory.members, 18)?,
            )?;
            request.work = add(
                request.work,
                add(
                    graph,
                    mul(add(self.claims.functions.len(), inventory.uses)?, 4)?,
                )?,
            )?;
        }
        Ok(request)
    }

    pub(in crate::frontend::oir::owned) fn checked_context<'a>(
        &'a self,
        sources: &SourceMap,
        remaining: &mut RemainingContextBudget<'a>,
        allocator: &mut Allocator,
    ) -> Result<OwnedVectorContext<'a>, OwnedFailure> {
        if !std::ptr::eq(self, remaining.wrapper) {
            return Err(bad());
        }
        let request = self.context_request()?;
        if request.metadata_bytes > remaining.metadata {
            return Err(resource("owned vector context metadata"));
        }
        let work = remaining
            .work
            .checked_sub(request.work)
            .ok_or_else(|| resource("owned vector context work"))?;
        let metadata_scope = remaining.metadata;
        // Exclusive prepaid scope: while the helper runs, no remaining
        // metadata can be reused. Its own actual capacity gates see this same
        // scope after the complete caller/control bank has been subtracted.
        remaining.metadata = 0;
        remaining.work = work;
        let result = self.context_in_scope(
            sources,
            remaining.limits,
            request,
            metadata_scope,
            allocator,
        );
        match result {
            Ok(context) => {
                remaining.metadata = metadata_scope
                    .checked_sub(context.charged_metadata)
                    .ok_or_else(bad)?;
                Ok(context)
            }
            Err(failure) => {
                // No returned table survives; attempted work remains charged.
                remaining.metadata = metadata_scope;
                Err(failure)
            }
        }
    }

    fn context_in_scope<'a>(
        &'a self,
        sources: &SourceMap,
        limits: budget::Limits,
        request: ContextRequest,
        metadata_scope: usize,
        allocator: &mut Allocator,
    ) -> Result<OwnedVectorContext<'a>, OwnedFailure> {
        self.check(sources, limits)?;
        let controls = control_bytes()?;
        let (elements, bounds_retained) = if self.declarations.is_none() {
            (None, 0)
        } else {
            let declarations = self.declarations.as_ref().ok_or_else(bad)?;
            let allowance = metadata_scope.checked_sub(controls).ok_or_else(bad)?;
            if request.bounds_bytes > allowance {
                return Err(resource("owned vector context bounds metadata"));
            }
            let elements =
                CheckedElements::check_with_allocator(declarations, allowance, allocator)
                    .map_err(bounds_failure)?;
            let retained = elements.retained_bytes().map_err(bounds_failure)?;
            if retained > allowance {
                return Err(resource("owned vector context retained bounds"));
            }
            for key in &self.claims.functions {
                let element = elements.element(key.record_id()).map_err(bounds_failure)?;
                if !std::ptr::eq(element.declarations(), declarations) {
                    return Err(bad());
                }
            }
            inventory_with(&self.program, |record| {
                let element = elements.element(record).map_err(bounds_failure)?;
                if !std::ptr::eq(element.declarations(), declarations) {
                    return Err(bad());
                }
                Ok(())
            })?;
            (Some(elements), retained)
        };
        let charged_metadata = add(controls, bounds_retained)?;
        if charged_metadata > metadata_scope {
            return Err(resource("owned vector context retained bounds"));
        }
        Ok(OwnedVectorContext {
            wrapper: self,
            elements,
            charged_metadata,
            bounds_retained,
        })
    }
}

impl RemainingContextBudget<'_> {
    pub(in crate::frontend::oir::owned) const fn remaining(&self) -> (usize, usize) {
        (self.metadata, self.work)
    }
}

impl<'a> OwnedVectorContext<'a> {
    pub(in crate::frontend::oir::owned) fn belongs_to(
        &self,
        wrapper: &RawOwnedVectorProgram,
    ) -> bool {
        std::ptr::eq(self.wrapper, wrapper)
    }

    pub(in crate::frontend::oir::owned) const fn is_legacy(&self) -> bool {
        self.elements.is_none()
    }

    pub(in crate::frontend::oir::owned) const fn retained_bounds_bytes(&self) -> usize {
        self.bounds_retained
    }

    pub(in crate::frontend::oir::owned) fn element_for(
        &self,
        function: hir::DefId,
        remaining: &mut RemainingContextBudget<'a>,
    ) -> Result<Element<'a>, OwnedFailure> {
        if !std::ptr::eq(self.wrapper, remaining.wrapper) {
            return Err(bad());
        }
        let work = remaining
            .work
            .checked_sub(4)
            .ok_or_else(|| resource("owned vector context element work"))?;
        remaining.work = work;
        let key = self.owned_vector_operation(function).ok_or_else(bad)?;
        self.elements
            .as_ref()
            .ok_or_else(bad)?
            .element(key.record_id())
            .map_err(bounds_failure)
    }
}

impl ProgramDomainQueries for RawOwnedVectorProgram {
    fn fixed_builtin_base(&self) -> Option<usize> {
        Some(self.usage.domains.fixed_start)
    }
    fn old_vector_operation(&self, id: hir::DefId) -> Option<vector::Function> {
        let domains = self.usage.domains;
        if id.0 >= domains.owned_start {
            return None;
        }
        self.program
            .vector_functions
            .get(id.0.checked_sub(domains.old_start)?)
            .copied()
    }
    fn owned_vector_operation(&self, id: hir::DefId) -> Option<Function> {
        let domains = self.usage.domains;
        self.claims
            .functions
            .get(id.0.checked_sub(domains.owned_start)?)
            .copied()
    }
}

impl ProgramDomainQueries for OwnedVectorContext<'_> {
    fn fixed_builtin_base(&self) -> Option<usize> {
        self.wrapper.fixed_builtin_base()
    }
    fn old_vector_operation(&self, id: hir::DefId) -> Option<vector::Function> {
        self.wrapper.old_vector_operation(id)
    }
    fn owned_vector_operation(&self, id: hir::DefId) -> Option<Function> {
        self.wrapper.owned_vector_operation(id)
    }
}
