# Same-normal-return loan release comparison: source-contract review

Status: proposal only. The original failure, frozen expectations and comparator
v4 are unchanged. Full collection stopped after the first semantic mismatch.

Counterexample: positive-exclusive_alias has two distinct mutable owners passed
to one call. The frozen body model acquires handle0 then1 and releases1 then0.
The unchanged consumer acquires0 then1 and releases0 then1 inside that call's
single normal ReturnScalar operation. The first debug report is
full-debug-incremental-attempt1.json:151 cases match, this one differs only in
that release order. Stopped collection retains152 debug and74 release receipts.

## Existing source authority

RFC0014 §4 states that a loan begins when its argument is evaluated, remains
active during later arguments and nested calls, and ends when its owning direct
call returns normally. Its explicit nested-reborrow rule requires a child loan
to end before its parent call can return. It does not prescribe ordering among
independent sibling argument loans released by the same normal return.

The approved Unit3 design §5 preserves immediate lexical argument preparation,
earlier loans through nested argument evaluation, normal-return release and
reverse lexical StorageEnd cleanup. Reverse lexical order is an owner-storage
cleanup obligation. The document does not assign that order to same-return
loan-release callbacks.

The frozen predecessor source corroborates this distinction without consulting
the Unit3 candidate: owned/execute.rs return_value preflights the complete set
of the resume call's borrowed loans, performs result transfer, and releases that
same set in its declaration order. The independent Python body model instead
iterates reversed(loans) after invoke returns. Its scheduler charges one whole
ReturnScalar/ReturnOwned operation and gives individual borrow-release events
no separate charge. The prior source comparator compared charges rather than
claiming a source-specified order for these internal release callbacks.

All source operations resume only after that normal return completes. This
language has no stored references, user destructors, callback-on-release or
concurrent observation that could execute between two release callbacks in one
already admitted Return operation. Legal exclusive siblings have distinct
roots; shared siblings can decrement the same shared counter. Their complete
normal-return membership determines the resulting capability state. An abrupt
failure before the Return charge commits none of that return; no partial
release/unwind is promised.

Therefore the frozen source contract supports exact release membership at the
normal-return boundary, with exact ordering of acquisitions, invocations,
returns, parent/child lifetimes, charged operations and lexical cleanup. It does
not support treating the Python model's reversed sibling iteration as a new
language-level guarantee.

## Narrow proposed comparison

Keep every actual callback in its recorded physical order. Add the exact return
group identity from actual data: returning activation, its observed Enter-time
resume call, caller activation/function and raw CallSiteId, plus return origin.
At Enter, snapshot that actual call's exact borrowed LoanKey set, including
frame/activation/loan/instance. Every release must match one of those keys once,
while the same actual Return operation remains charged and uncommitted. Normal
return must release the full set; missing/duplicate/wrong-call/moved callbacks
remain failures. Preserve actual frame/epoch/parent-child checks unchanged.

Expected group identity comes solely from the frozen model's return event and
the frozen owning call identity on its subsequent release events. The returning
activation and caller invocation distinguish recursion and repeated call sites.
Do not group by span or target function alone.

For comparison only, compare exact release membership as a multiset within one
such group. Preserve group order and all intervening acquisitions. Never sort
the global loan stream. Do not change original model events, sources, expected
targets, result/fuel data, or original actual traces. StorageEnd order stays
strict. Compare full charged operations and physical effects in their original
order, so no operation or effect can move across a return boundary.

Before acceptance, add controls that swap only independent releases within one
return and controls that drop/duplicate a release or move one across charged
returns, including repeated call-site/recursive identity distinctions. The
former should be equivalent; all latter forms must fail. Any correction must
receive independent review and a new comparator identity before collection
resumes, with the original mismatch still attributed to comparator v4.

