//! Denied-Run controls. No test bypasses RUN_ADMITTED or executes imported Run.
use super::*;

#[test]
fn checked_hir_import_run_work_and_complete_carriers_only() {
    let counts = super::super::Counts([2, 2, 1, 2, 11, 5, 7, 2]);
    let verify = verify_terminal::WorkPlan::calculate_request(counts, 29, Request::Verify).unwrap();
    let run = verify_terminal::WorkPlan::calculate_request(counts, 29, Request::Run).unwrap();
    assert_eq!(verify.entry_work, 0);
    assert_eq!(verify.total, 1_009_440);
    assert_eq!(run.entry_work, 256 * (2 + MAX_ROWS as u64 + 2));
    assert_eq!(run.total, verify.total + run.entry_work);
    assert!(verify_terminal::WorkPlan::calculate_request(
        super::super::Counts([MAX_ROWS + 1; 8]),
        MAX_ROWS,
        Request::Run
    )
    .is_err());
    assert!(
        verify_terminal::WorkPlan::calculate_request(counts, MAX_ROWS + 1, Request::Run).is_err()
    );
    println!("HIR_IMPORT_RUN_CARRIERS request={} canonical_input={} completion={} completion_result={} context={} plan={} terminal_facts={} terminal_rejected={} terminal_result={} builder_named={} terminal_named={} candidate_named={}",
        size_of::<Request>(), size_of::<CanonicalInput<'_, '_>>(), size_of::<Completion>(), size_of::<Result<Completion, VerifyRejected>>(),
        size_of::<verify_terminal::Context<'_>>(), size_of::<verify_terminal::WorkPlan>(), size_of::<VerifyFacts>(), size_of::<VerifyRejected>(), size_of::<Result<VerifyFacts, VerifyRejected>>(),
        builder_named_bytes().unwrap(), verify_terminal::named_bytes().unwrap(), verify_named_bytes().unwrap());
}
