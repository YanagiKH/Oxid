//! Additional logical request for the real extended plan. The legacy zero-
//! extension path retains its old formula and creates no extension table.
use super::*;
use crate::frontend::oir::owned::owned_record_codec::{bounds::CheckedElementView, Element};
use crate::frontend::oir::owned::verified::VerifiedOwnedVectorProgram;

#[derive(Clone, Copy)]
pub(super) struct Request {
    pub(super) metadata_bytes: usize,
    pub(super) work: usize,
}

struct PlanControls {
    witness_argument: &'static VerifiedOwnedVectorProgram,
    functions_argument: &'static [RawOwnedFunction],
    selected_function: &'static RawOwnedFunction,
    function_option: Option<&'static RawOwnedFunction>,
    selected_owner: &'static OwnerDecl,
    selected_call: &'static CallDecl,
    selected_argument: &'static ArgumentSlot,
    selected_statement: &'static OwnedStatement,
    functions: std::slice::Iter<'static, RawOwnedFunction>,
    owners: std::slice::Iter<'static, OwnerDecl>,
    calls: std::slice::Iter<'static, CallDecl>,
    arguments: std::slice::Iter<'static, ArgumentSlot>,
    statements: std::slice::Iter<'static, OwnedStatement>,
    view: CheckedElementView<'static>,
    caller_view: CheckedElementView<'static>,
    element: Element<'static>,
    element_return:
        Result<Element<'static>, super::super::owned_record_codec::bounds::ShapeFailure>,
    layout_return: Result<Layout, DeclarationError>,
    caller_layout_return: Result<Layout, DeclarationError>,
    width_return: Result<usize, DeclarationError>,
    caller_width_return: Result<usize, DeclarationError>,
    request: Request,
    optional_request: Option<Request>,
    request_return: Result<Option<Request>, AdmissionFailure>,
    caller_request_return: Result<Option<Request>, AdmissionFailure>,
    plan: ExecutionPlan<'static, VerifiedOwnedVectorProgram>,
    plan_return: Result<ExecutionPlan<'static, VerifiedOwnedVectorProgram>, AdmissionFailure>,
    caller_plan_return:
        Result<ExecutionPlan<'static, VerifiedOwnedVectorProgram>, AdmissionFailure>,
    function_plans: Vec<FunctionPlan>,
    owner_offsets: Vec<usize>,
    calls_vector: Vec<CallPlan>,
    owned_stages: Vec<OwnerPlaceId>,
    borrowed_loans: Vec<LoanId>,
    plans_allocation_return: Result<Vec<FunctionPlan>, AdmissionFailure>,
    offsets_allocation_return: Result<Vec<usize>, AdmissionFailure>,
    call_allocation_return: Result<Vec<CallPlan>, AdmissionFailure>,
    owned_allocation_return: Result<Vec<OwnerPlaceId>, AdmissionFailure>,
    borrowed_allocation_return: Result<Vec<LoanId>, AdmissionFailure>,
    reserve_return: Result<(), std::collections::TryReserveError>,
    frame_usage: [FrameUsage; 2],
    frame_return: Result<FrameUsage, AdmissionFailure>,
    statement_cost: usize,
    terminator_cost: usize,
    delivery_return: Option<usize>,
    caller_delivery_return: Option<usize>,
    key: Option<crate::frontend::builtin_catalog::owned_vector::Function>,
    key_return: Option<crate::frontend::builtin_catalog::owned_vector::Function>,
    capacity_rows: [(usize, usize, usize); 3],
    arithmetic: [Result<usize, AdmissionFailure>; 8],
    counters: [usize; 24],
}

pub(super) fn requested<W: PlanWitness>(
    witness: &W,
    work_limit: usize,
) -> Result<Option<Request>, AdmissionFailure> {
    if !witness.uses_owned_vectors() {
        return Ok(None);
    }
    // The shared builder performs at most sixteen raw/storage scans. Every
    // selected row pays twelve fixed checked bound/identity/query visits.
    // This includes unused and unreachable declarations, not only live calls.
    let mut rows = 1usize;
    rows = add(rows, witness.declarations().records().len())?;
    rows = add(rows, witness.declarations().enums().enums().len())?;
    for function in witness.functions() {
        rows = add(rows, 1)?;
        for count in [
            function.locals.len(),
            function.places.len(),
            function.owners.len(),
            function.references.len(),
            function.loans.len(),
            function.calls.len(),
            function.blocks.len(),
        ] {
            rows = add(rows, count)?;
        }
        for call in &function.calls {
            rows = add(rows, call.arguments.len())?;
        }
        for block in &function.blocks {
            rows = add(rows, block.statements.len())?;
        }
    }
    let work = add(witness.usage().work, add(mul(rows, 16 * 12)?, 32)?)?;
    if work > work_limit.min(budget::Limits::DEFAULT.work) {
        return Err(AdmissionFailure::new("owned vector plan work", None));
    }
    Ok(Some(Request {
        metadata_bytes: add(
            witness.usage().metadata_bytes,
            add(
                size_of::<PlanControls>(),
                super::super::owned_record_codec::bounds::element_control_bytes(),
            )?,
        )?,
        work,
    }))
}

pub(super) fn admit_capacity<T>(
    extended: bool,
    total: &mut usize,
    capacity: usize,
    requested: usize,
    limit: usize,
) -> Result<(), AdmissionFailure> {
    admit_capacity_bytes(extended, total, capacity, requested, size_of::<T>(), limit)
}

pub(super) fn admit_capacity_bytes(
    extended: bool,
    total: &mut usize,
    capacity: usize,
    requested: usize,
    bytes: usize,
    limit: usize,
) -> Result<(), AdmissionFailure> {
    if !extended {
        return Ok(());
    }
    let excess = capacity
        .checked_sub(requested)
        .ok_or_else(|| AdmissionFailure::new("owned vector plan capacity", None))?;
    *total = add(*total, mul(excess, bytes)?)?;
    if *total > limit.min(MAX_PLAN_BYTES) {
        return Err(AdmissionFailure::new("owned plan bytes", None));
    }
    Ok(())
}
