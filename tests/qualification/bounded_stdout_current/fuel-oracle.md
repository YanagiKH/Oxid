# Independent raw stdout process fuel oracle

Frozen driver commit: `2932c12ad001cb51b18967dd7138690a232c3eab`.
Prepared from the written fuel contract and the authored raw fixture rows on
2026-10-06. This is an expected-results ledger, not execution evidence.
No compiler build, reference execution, native execution, production cost-helper
query, or consumer-observed fuel result was used to derive these numbers.

## Frozen inputs and authority

Repository root: `/workspace/scratch/8266abf56995/oxid-bounded-output`.

* `src/frontend/oir/owned/builtin_output_process_tests.rs:51` through line 185:
  frozen `candidate(case)` construction. SHA-256 of the function text, including
  its following newline and excluding the preceding comments and next function:
  `f3fc543256a4bcb4bb915b5e35f05c8803274de357d2b85b74ba12577ef1651c`.
  Whole driver file SHA-256:
  `b63bd714e06ca447a20d34df84551f3cff1aa9c6160ef77eb581af0d2a0a8e38`.
  The working file and `git show 2932c12:<path>` matched this whole-file hash.
* `src/frontend/oir/owned/builtin_output_fixtures.rs:25`: canonical output
  callee; line 74: scalar-only inventory; line 124: direct caller;
  lines 216 onward: output transformation of projected/forwarded graphs.
  SHA-256: `ee170ab6e64a7f9f20e8bdfe9f666de04f91d0e91d550dd845e075ba21ac5568`.
* `src/frontend/oir/owned/builtin_input_fixtures.rs:39`: retained base main
  declarations and rows; line 168: checksum declarations retained by projected;
  line 365: projected record construction; line 487: forwarding function.
  SHA-256: `83594ab6226b0dd6a94dc8c1b7a0c79015fa351ff43ea15a3620a5edefbe8176`.
* `spec/typed-preview.md:1206`: complete declared storage and logical activation
  formula; lines 1236–1252: ordinary operation charges; lines 1261–1273:
  return release, failed-charge ordering, and preserved earlier effects.
  SHA-256: `c1c35447c4ff2bfcbffb9b92033f9cfd3fcd6b05d3324b2f898bdcb6e5fe5f05`.
* `rfcs/0023-bounded-enum-match.md:79`: every enum has logical width 2;
  lines 133–147: written-arm-order dispatch and width-based consumption/transfer.
* `rfcs/0022-projected-array-slices.md:54`: projection/reborrow uses existing
  logical activation and preparation charges; extra physical metadata is not fuel.
* `rfcs/0025-bounded-stdout-process-entry.md:71` through line 103: complete
  prevalidation, empty behavior, accepted-byte count, one-byte attempts,
  interruptions, `4 + capacity + attempts`, and no rollback. Lines 17–58:
  process status, signal setup, diagnostic behavior, and pre-activation ordering.
  SHA-256: `76f3d8e0070e00b9a0e3b0e0846b6f54471d99705bb8569e05e77b55d83c5d3c`.

All relative source paths above are under the absolute repository root stated
above. These are raw fixture totals; they are not source-lowering fuel totals.

## Notation and ordinary charges

Let N be output capacity, w = max(1, N), H be attempted one-byte host writes,
and k be the number of those attempts that return EINTR. For frame accounting,
S counts scalar locals plus scalar places, G argument descriptors, P the sum
of every owner's logical width, O owners, R reference parameters, L loans,
and Q calls. Count declarations even if their previous instructions were removed.

`X = S + G + P + 4O + 8R + 12L + 2Q`.

Root costs `1 + X`. Scalar assignment, StorageLive, OpenCall with no owned
arguments, and PrepareBorrow each cost 1. Construction, MoveInitialize,
Discard and StorageEnd each cost `1 + affected width`. Each MatchDispatch costs
1, selected ConsumeVariant costs 3, and written arms are Complete first,
InvalidInput second, IoError third. Scalar return costs `1 + P + L + Q + R`;
owned return adds the returned width. One borrowed argument has zero pairwise
alias-check term, so Invoke costs `2 + X(callee)`.

The builtin has S=0, G=0, P=2, O=1, R=1, L=0, Q=0: X=14.
Its Invoke is 16, StorageLive is 1, and ReturnOwned is `1+2+1+2=6`.
Its operation prepays `4+N`, then charges one before every attempted write.
There is no separate post-write enum-construction debit. Physical view metadata
and the 1024-byte builtin scratch reservation add no logical activation fuel.

## Direct capacity 3 and capacity 0 inventories

The process wrapper adds eight locals to the original one and one match-subject
owner to the two original owners. It prepends six assignments but retains the
original assignment of 37, although that local is no longer used in the array.
Thus S=9, G=1, P=w+4, O=3, R=0, L=1, Q=1, giving X=w+40.
Root activation is w+41. The final scalar return costs w+7.

The following rows are explicitly authored from the frozen raw instructions.
The cumulative columns describe successful debits; an exact cumulative budget
then fails at the next nonzero charge, unless the program has returned.

| Executed event | N=3 cost | N=3 cumulative | N=0 cost | N=0 cumulative |
|---|---:|---:|---:|---:|
| Root activation | 44 | 44 | 42 | 42 |
| Six prepended scalar assignments | 6 | 50 | 6 | 48 |
| Retained assignment of 37 | 1 | 51 | 1 | 49 |
| Buffer StorageLive | 1 | 52 | 1 | 50 |
| ConstructArray | 4 | 56 | 2 | 52 |
| OpenCall | 1 | 57 | 1 | 53 |
| PrepareBorrow | 1 | 58 | 1 | 54 |
| Invoke output builtin | 16 | 74 | 16 | 70 |
| Builtin result StorageLive | 1 | 75 | 1 | 71 |
| Output prevalidation/staging/result reservation | 7 | 82 | 4 | 75 |
| Host accepts A | 1 | 83 | — | 75 |
| Host accepts B | 1 | 84 | — | 75 |
| Host accepts C | 1 | 85 | — | 75 |
| Builtin ReturnOwned | 6 | 91 | 6 | 81 |
| Match subject StorageLive | 1 | 92 | 1 | 82 |
| MoveInitialize subject from call result | 3 | 95 | 3 | 85 |
| Call-result StorageEnd | 3 | 98 | 3 | 88 |
| Complete MatchDispatch | 1 | 99 | 1 | 89 |
| ConsumeVariant Complete | 3 | 102 | 3 | 92 |
| Subject StorageEnd | 3 | 105 | 3 | 95 |
| Buffer Discard | 4 | 109 | 2 | 97 |
| Buffer StorageEnd | 4 | 113 | 2 | 99 |
| Main ReturnScalar | 10 | **123** | 8 | **107** |

Generally, the successful debit before the first possible attempt is
`B = 73 + 2w + N`. Post-operation debit is `29+3w` for Complete,
`30+3w` for InvalidInput, and `32+3w` for IoError. The last includes three
dispatches and the one-unit `payload + 2` checked scalar assignment.

Therefore complete-call totals, including all caller return work, are:

* Complete: `102 + 5w + N + H`
* InvalidInput: `103 + 5w + N`, with H=0
* IoError: `105 + 5w + N + H`

For valid capacity 3, H=3+k on Complete, so the total is **123+k**.
For capacity 0, Complete costs **107** with zero attempts even on an unusable fd 1.
Both invalid-last fixtures cost **121**: the last element is 256 or -1,
InvalidInput maps to status 1, stdout is empty, and stderr is empty.
That semantic status 1 is distinct from a fuel diagnostic with process status 1.

## Exact direct-fixture boundaries

Assume every reached host attempt accepts its requested byte, signal setup
succeeds, and stderr can deliver an eventual diagnostic.

| Budget | Language stdout | Next failed debit or completion | Fuel left at failure |
|---:|---|---|---:|
| 81 | empty | Output core needs 7 after cumulative 75; no scan or attempt | 6 |
| 82 | empty | First one-byte attempt | 0 |
| 83 | A | Second one-byte attempt | 0 |
| 84 | AB | Third one-byte attempt | 0 |
| 85 | ABC | Builtin ReturnOwned needs 6 | 0 |
| 90 | ABC | Builtin ReturnOwned needs 6 | 5 |
| 91 | ABC | Caller subject StorageLive | 0 |
| 122 | ABC | Main ReturnScalar needs 10 after cumulative 113 | 9 |
| 123 | ABC | Complete, process status 0 | 0 remaining |

Every failure row has E0601 and process status 1 if the diagnostic completes;
stderr failure changes the process status to 74 without undoing stdout.
Failed subtraction preserves the listed remainder. Reaching cumulative 85
already completes output and materializes Complete, but does not pay either
normal return. A complete-looking artifact is therefore not a success signal.

Direct WriteStdout and builtin ReturnOwned share the authored builtin origin
`raw-owned-consumers.ox:401:1`; the row ledger distinguishes them. The caller's
subject StorageLive is at line 3609, and final ReturnScalar at line 3618.
These coordinates follow `consumer_fixtures.rs:24` (one `x` per line) and
`enum_consumer_fixtures.rs:12` (ordinal multiplied by two bytes).

For empty: 74 fails the four-unit core with three remaining; 75 through 80
fail the six-unit builtin return; 81 reaches the caller then fails StorageLive;
106 fails the eight-unit main return with seven remaining; 107 completes.
There is no first-byte attempt in any empty path.

For invalid-last: 81 fails core; 82 through 87 fail builtin return; 88 reaches
the caller. After caller transfer cumulative 95, two dispatches end at 97,
ConsumeVariant at 100, subject end at 103, buffer discard/end at 107/111,
and main return at 121. Budget 120 fails main return with nine remaining.
No budget causes output from an invalid-last fixture.

## IoError, partial output, and interruptions

On capacity 3 let n be accepted bytes before a terminal zero-progress/error
attempt. Then 0<=n<=2, H=n+k+1, and complete execution costs
**124+n+k**. It returns status **n+2**, emits the first n bytes of ABC, and
needs no stderr diagnostic. A failed attempt and a zero-progress attempt have
the same fuel formula. An error scripted after three accepted bytes is never
attempted: Complete has already been reached.

For example, accept A then fail B: cumulative 83 accepts A; cumulative 84 pays
the terminal attempt and materializes IoError(1). Builtin return ends at 90;
subject live/move/call-result end at 91/94/97; three dispatches at 98/99/100;
consume at 103; subject end at 106; buffer discard/end at 110/114;
`payload+2` at 115; main return at **125**. Process status is 3.
Budget 124 fails main return with nine remaining and preserves A.

For repeated interruptions or arbitrary mixtures, after the first j paid
attempts the spent amount is `82+j`; stdout is exactly the accepted-success
subsequence before termination. EINTR does not advance the byte index. The
next attempted write always needs a fresh unit. For the sequence
EINTR, accept A, accept B, accept C, budgets 83/84/85/86 leave empty/A/AB/ABC
respectively, then fail the next attempt or six-unit return. The full total is
124. For accept A, EINTR, accept B, accept C, those same budgets leave
A/A/AB/ABC. These are injected-outcome expectations, not real-OS observations.

## Narrow projection and forwarding checks

The frozen projected candidate retains 24 old locals and two scalar places,
adds eight locals, and retains four owners of widths 5,2,2,3. Thus S=34,
G=1, P=12, O=4, R=0, L=1, Q=1 and X=77; root costs 78.
Its prefix costs, in execution order, are:

`78 + 6 + 3 + 1 + 2 + 1 + 4 + 6 + 4 + 1 + 1 + 16 + 1 + 7 = 131`.

The terms are root; new assignments; old assignments; record live; two guard
assignments; temporary array live; array construction; record construction;
temporary end; open; borrow; invoke; builtin live; output core. The record
has width 5 even though only its width-3 middle field is output. Three writes
end at 134, builtin return at 140. The Complete tail including builtin return
costs `6+1+3+3+1+3+3+6+6+15=47`, so **181** returns status 0 with ABC.
Budgets 131/132/133/134 leave empty/A/AB/ABC; 180 fails main return with 14 left.

The frozen forwarded main retains four old locals and adds eight: S=12,
G=1, P=7, O=3, R=0, L=1, Q=1 and X=46; root costs 47.
The forwarding function has S=0, G=1, P=4, O=2, R=1, L=1, Q=1 and X=35.
Main's Invoke therefore costs 37. Forward's own Invoke of the builtin costs 16,
and Forward ReturnOwned costs `1+4+1+1+1+2=10`.
Its prefix is:

`47 + 6 + 3 + 1 + 4 + 1 + 1 + 37 + 1 + 1 + 16 + 1 + 7 = 126`.

After three writes at 129, builtin return ends at 135. The forwarder pays
live/move/end/owned-return of 1+3+3+10, returning to main at 152. Main's ordinary
Complete tail costs 32, so **184** returns status 0 with ABC. Budgets
126/127/128/129 leave empty/A/AB/ABC; 183 fails main return with nine left.
These two fixtures receive only this narrow complete-path accounting here;
no broad process/OS matrix is claimed.

## Scalar process controls and assumptions

Status-only fixtures have S=1 and no owners, calls, loans or references.
They cost root 2 + assignment 1 + return 1 = **4**. Budgets 0/1 fail root,
2 fails assignment, and 3 fails return. At 4, statuses 0/37 return exactly,
while -256/256 produce the specified out-of-range diagnostic and status 1,
or 74 if that diagnostic fails. All have empty stdout; no scalar trailer.

This ledger assumes ordinary verification and physical admission succeed on
the qualified Linux x86_64 host. SIGPIPE setup is outside the source-fuel
ledger and precedes root activation: setup failure is silent status 74 with
no source effects even if fuel is zero. Terminal stderr retries are outside
source fuel. Host calls may block; fuel bounds attempted writes, not elapsed
time or durability. Actual reference/native controls must separately establish
these expectations at the frozen head. No passed execution is asserted here.

## Append-only amendment: corrected raw admission through 3ff96af

Amended 2026-10-06, before receiving any projected exact-fuel observations.
This section supersedes the historical projected declaration count and its
derived totals above for commit
`3ff96afb2acabb41fff770e244986a4d6fd6ac69`. The original ledger text remains
unchanged so its independent pre-execution freeze is preserved. Its original
file SHA-256 was
`2fd35fc91ef2fcc01a4a15c49636e7d5dc52e8f797f69878dc076169e778baa0`.
This amendment derives only from the fixture source diff and written formulas;
execution observations do not supply or calibrate any expectation.

Source-equivalence review of the driver diff from 2932c12 through 3ff96af:

1. `1a0e0d8a3eb86873c2fb1c5b5d692d23b5116944` changes the private driver's
   presentation of raw-verification errors to the existing diagnostic adapter.
   It does not change any candidate declaration, instruction, call or cost.
2. `9b2dce3c026abe95907bab8765f01e762c0681cd` corrects the ConsumeVariant
   instruction origin from `s(12)` to the authoritative match descriptor's
   `s(11)`. All operand IDs, arm choices, declarations and charge-bearing
   operations stay identical. The corrected consume-failure origin is
   `raw-owned-consumers.ox:3612:1`, shared with MatchDispatch, rather than the
   historical candidate's line 3613. This resolves a raw canonical-site mismatch;
   it is not an additional event or a change from the three-unit consume cost.
3. `3ff96afb2acabb41fff770e244986a4d6fd6ac69` adds, immediately after the
   truncation of main's blocks, `main.places.clear()` only for Case::Projected.
   The two removed i32 places belonged exclusively to the discarded checksum
   loop. Their initialization/load/store instructions were already absent from
   the retained construction, projected borrow, output call and new match tail.
   All 32 scalar locals, four owners, call/loan descriptors and retained
   executable rows are unchanged. Debug formatting changes elsewhere in the
   same diff do not execute in an admitted language path.

Consequently ABC/invalid/empty/forwarded/status fuel totals and boundaries above
are unchanged. Their ConsumeVariant failure origin uses the corrected match
anchor. For projected, S changes from 34 to 32; G=1, P=12, O=4, R=0, L=1,
Q=1 stay unchanged. Recomputing from declarations gives:

`X = 32 + 1 + 12 + 4*4 + 8*0 + 12*1 + 2*1 = 75`.

Root costs **76**. Only that root charge loses two units. No operation debit or
return debit changes. The corrected projected preattempt ledger is:

`76 + 6 + 3 + 1 + 2 + 1 + 4 + 6 + 4 + 1 + 1 + 16 + 1 + 7 = 129`.

Three successful one-byte attempts bring the debit to 132. Builtin ReturnOwned
brings it to 138. The unchanged complete-path post-operation cost remains
`6+1+3+3+1+3+3+6+6+15=47`. Therefore the corrected projected total is
**129 + 3 + 47 = 179**, returning status 0 and stdout ABC.

Corrected projected boundary expectations:

| Budget | Language stdout | Next failed debit or completion | Fuel left |
|---:|---|---|---:|
| 128 | empty | Seven-unit output core after cumulative 122 | 6 |
| 129 | empty | First one-byte attempt | 0 |
| 130 | A | Second one-byte attempt | 0 |
| 131 | AB | Third one-byte attempt | 0 |
| 132 | ABC | Six-unit builtin ReturnOwned | 0 |
| 137 | ABC | Six-unit builtin ReturnOwned | 5 |
| 138 | ABC | Main subject StorageLive | 0 |
| 178 | ABC | Fifteen-unit main ReturnScalar after cumulative 164 | 14 |
| 179 | ABC | Complete, process status 0 | 0 remaining |

The projected WriteStdout/builtin return origin is
`raw-owned-consumers.ox:4096:1`; subject StorageLive remains line 3609,
ConsumeVariant now uses line 3612, and main ReturnScalar remains line 3618.
The historical 181 total is retained only as arithmetic for the old two-place
declaration inventory; **179 is the expectation for the corrected fixture**.

Corrected frozen input fingerprints for 3ff96af:

* Whole `src/frontend/oir/owned/builtin_output_process_tests.rs`:
  `acf504e004820ae705ca54e56d00c21b40f77814cecbcfac8a5428e674ade6b1`
* `candidate(case)` text at lines 51–191, using the original extraction rule:
  `e71296e511c0c6ae24005cbaedaa4ba65dbb6e40c806738b0f8dbe1c93a2722e`
