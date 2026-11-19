//! Paid construction of an immutable LoanId-indexed acquisition relation.
//! Runtime view validation does not revisit raw blocks or statements.
use super::*;

#[allow(dead_code)]
struct BuilderCarriers<'a> {
    functions_argument: &'a [RawOwnedFunction],
    selected_function: &'a RawOwnedFunction,
    selected_loan: &'a LoanDecl,
    selected_statement: &'a OwnedStatement,
    functions: std::slice::Iter<'a, RawOwnedFunction>,
    loans: std::iter::Enumerate<std::slice::Iter<'a, LoanDecl>>,
    blocks: std::slice::Iter<'a, OwnedBlock>,
    statements: std::slice::Iter<'a, OwnedStatement>,
    table: Vec<Option<Operand>>,
    allocation_return: Result<Vec<Option<Operand>>, AdmissionFailure>,
    caller_table_return: Result<Vec<Option<Operand>>, AdmissionFailure>,
    selected_slot: Option<&'a mut Option<Operand>>,
    selected_slot_result: Result<&'a mut Option<Operand>, AdmissionFailure>,
    slot_argument: &'a mut Option<Operand>,
    selected_record: Option<RecordId>,
    selected_owner: Option<&'a OwnerDecl>,
    selected_reference: Option<&'a ReferenceDecl>,
    selected_loan_option: Option<&'a LoanDecl>,
    base_argument: AccessBase,
    base_return: std::result::Result<(BorrowedTy, bool), OwnedFailure>,
    source_type: BorrowedTy,
    target_type: BorrowedTy,
    candidate_loan: LoanId,
    candidate_index: Operand,
    local_lookup: Option<&'a LocalDecl>,
    type_query: bool,
    slot_query: bool,
    discovery_return: usize,
    caller_discovery_return: usize,
    table_length: usize,
    metadata_value: usize,
    metadata_argument: &'a mut usize,
    actual_capacity: usize,
    requested_capacity: usize,
    metadata_limit: usize,
    requested_work: usize,
    legacy_work: usize,
    work_limit: usize,
    work_subtotal: usize,
    byte_arithmetic: Result<usize, AdmissionFailure>,
    work_arithmetic: Result<usize, AdmissionFailure>,
    additional_work_return: Result<usize, AdmissionFailure>,
    caller_additional_work_return: Result<usize, AdmissionFailure>,
    work_return: Result<(), AdmissionFailure>,
    caller_work_return: Result<(), AdmissionFailure>,
    capacity_return: Result<(), AdmissionFailure>,
    // The complete constructed plan value grows even for an empty sidecar.
    constructed_function: FunctionPlan,
}

#[allow(dead_code)]
struct QueryCarriers<'a, W: PlanWitness> {
    plan_argument: &'a ExecutionPlan<'a, W>,
    function_argument: &'a FunctionPlan,
    table_argument: &'a Vec<Option<Operand>>,
    slice_argument: &'a [Option<Operand>],
    function_id: hir::DefId,
    loan_argument: LoanId,
    lookup_return: Option<&'a Option<Operand>>,
    copied_return: Option<Option<Operand>>,
    selected_index: Option<Operand>,
    caller_index: Option<Operand>,
    acquisition_index: Operand,
    acquisition_comparison: bool,
    feature_return: bool,
    caller_feature_return: bool,
    feature_mode: u8,
    masked_mode: u8,
}

pub(super) const fn builder_control_bytes() -> usize {
    size_of::<BuilderCarriers<'_>>()
}

/// Shared selector/getter/return roles exist before the feature-bit branch.
/// VM and native callers pay these roles even for ordinary owning programs.
pub(in crate::frontend::oir::owned) const fn query_control_bytes<W: PlanWitness>() -> usize {
    size_of::<QueryCarriers<'_, W>>()
}

pub(super) fn requested_len(function: &RawOwnedFunction) -> usize {
    if function
        .loans
        .iter()
        .enumerate()
        .any(|(index, _)| shape::local_record_element(function, LoanId(index)).is_some())
    {
        function.loans.len()
    } else {
        0
    }
}

pub(super) fn additional_work(functions: &[RawOwnedFunction]) -> Result<usize, AdmissionFailure> {
    // Count logical metadata operations. A checked row lookup includes its
    // presence/error handling, and a compact referent decode is one operation.
    // The relation needs the loan lookup, region and projection tests (3),
    // authority tag, lookup, class, decode and permission (5), source tags (2),
    // target decode and tags (3), nominal equality (1) and return tags (2).
    const RELATION_QUERY: usize = 16;
    // Two discovery passes and one completion pass each visit a loan and
    // inspect the relation result. Initialization, slot lookup and presence
    // add three further operations per loan.
    const LOAN_VISITS: usize = 3 * RELATION_QUERY + 9;
    // Population uses up to Q+8. Three native scratch passes and the final
    // policy pass each inspect the new tag. Policy selection can write once
    // per statement.
    const STATEMENT_VISITS: usize = RELATION_QUERY + 8 + 4 + 1;
    // Up to one successful scratch selection per function per usage pass.
    const FUNCTION_VISITS: usize = 32 + 3 * (RELATION_QUERY + 2);
    // Counting selects a block, reads its length and checks multiply/add.
    // Population selects that block once more.
    const BLOCK_VISITS: usize = 5;
    let mut work = add(32, mul(functions.len(), FUNCTION_VISITS)?)?;
    for function in functions {
        work = add(work, mul(function.loans.len(), LOAN_VISITS)?)?;
        work = add(work, mul(function.blocks.len(), BLOCK_VISITS)?)?;
        for block in &function.blocks {
            work = add(work, mul(block.statements.len(), STATEMENT_VISITS)?)?;
        }
    }
    // Checked before table discovery or any plan allocation, also absent.
    Ok(work)
}

pub(super) fn admit_work(
    functions: &[RawOwnedFunction],
    legacy_work: usize,
    limit: usize,
) -> Result<(), AdmissionFailure> {
    let total = add(legacy_work, additional_work(functions)?)?;
    if total > limit.min(budget::Limits::DEFAULT.work) {
        return Err(AdmissionFailure::new("owned element loan plan work", None));
    }
    Ok(())
}

pub(super) fn build(
    function: &RawOwnedFunction,
    metadata_bytes: &mut usize,
    limit: usize,
) -> Result<Vec<Option<Operand>>, AdmissionFailure> {
    if *metadata_bytes > limit.min(MAX_PLAN_BYTES) {
        return Err(AdmissionFailure::new("owned plan bytes", None));
    }
    let requested = requested_len(function);
    let mut table = if requested == 0 {
        Vec::new()
    } else {
        reserve(requested)?
    };
    // The outer complete metadata preflight already included every requested
    // cell. Charge any excess actual capacity before initialization/publication.
    owned_vector_accounting::admit_capacity::<Option<Operand>>(
        true,
        metadata_bytes,
        table.capacity(),
        requested,
        limit,
    )?;
    table.resize(requested, None);
    for block in &function.blocks {
        for statement in &block.statements {
            let OwnedInstruction::AcquireLocalElementBorrow { loan, index } = statement.kind else {
                continue;
            };
            if shape::local_record_element(function, loan).is_none()
                || function
                    .locals
                    .get(index.local.0)
                    .is_none_or(|local| local.ty != hir::Ty::I32)
            {
                return Err(relation_failure(function));
            }
            let slot = table
                .get_mut(loan.0)
                .ok_or_else(|| relation_failure(function))?;
            if slot.is_some() {
                return Err(relation_failure(function));
            }
            *slot = Some(index);
        }
    }
    for (index, _) in function.loans.iter().enumerate() {
        if shape::local_record_element(function, LoanId(index)).is_some()
            && table.get(index).is_none_or(Option::is_none)
        {
            return Err(relation_failure(function));
        }
    }
    Ok(table)
}

fn relation_failure(function: &RawOwnedFunction) -> AdmissionFailure {
    AdmissionFailure::new("owned element loan plan relation", Some(function.span))
}

#[cfg(test)]
#[path = "element_loan_plan_tests.rs"]
mod tests;
