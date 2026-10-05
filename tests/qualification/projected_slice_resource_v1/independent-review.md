# Independent projected-slice resource successor review

Verdict: PASS for reviewed production resource implementation and exact current-source boundary fixtures. No material defect found. Qualification successor publication remains a separate gate: checkpoint01 explicitly says RECOVERABLE_INCOMPLETE_CHECKPOINT_NOT_QUALIFIED. This review is the independent ledger/evidence input for that successor, not permission to relabel an incomplete package or bypass exact-head hosted CI.

## Immutable scope and evidence identity

Production commit 052ad52ac876c01b91701132cffb466689b24d01; tree a573ca3d279bc3e14ad6da1bd84cae9917d0fe50; predecessor f052fc27b068042ef7c5835e4be001e5a5e49123. Reviewed read-only snapshot: oxid-projected-array-slices-safety-snapshot. Independently SHA256-checked every one of 1498 tracked files against checkpoint02 manifest before execution. Manifest SHA256 b4a59a88e49abb13ab79cb050533935f542acda10fd88d2c2e1a18a671c42879. Production patch SHA256 ef88a69447bf85e41f176b481666d3ab3a074787aedafad11e2945ff6fea87bc.

Qualification checkpoint01 commit ac2a27fa44a013575ad96c9d082c6fa18466b664/tree26fc48fc9d0a3765da4300462380cba1d9f27a6e has no src delta against052ad52 (git diff --name-only 052ad52 ac2a27f -- src is empty). No source, test, or oracle changes were made by this reviewer. Existing executables, not a duplicate build, were used. All six executable identities in final-executables.json were independently SHA256-verified. Exact test binaries:
- debug/deps/oxid-f0c68182be132477: cb33da1c3c7797b37a88c460012cc881375dad67add1885124c62ae1cde82678
- release/deps/oxid-41e287c6d9c7ce6d: 74af1887bb376eece15b971df897dd459d70e5a805dd18ea6ab823aac4639659

## Independently derived physical ledger

Notation: S scalar locals/places, A retained argument snapshots, P owner scalar cells, B aligned payload bytes, O owner identities, R reference parameters, L loans, C calls.

storage.rs is repr(C). OwnerKey and LoanKey are each four u64 =32 bytes. BorrowView is u64 offset plus Option<AggregateSlot>, measured16 bytes. Therefore ReferenceHandle is32+32+16=80 bytes/10 cells. LoanRuntime is instance/state16 + root32 + parent32 + view16 + child counts16=112 bytes/14 cells. OwnerRuntime32/4 and CallRuntime16/2 remain unchanged. Actual exact-source runtime layout tests independently print/assert those numbers in both profiles.

plan.rs computes X=S+A+P+4O+10R+14L+2C; Dref=8(S+A)+B+32O+80R+112L+16C. Activation fuel explicitly subtracts2(R+L), yielding logical reference8/loan12 with no changed language work. This deduction is correct for every FrameUsage constructed by the closed planner; count products/sums preflight with checked arithmetic. Native physical storage remains8(S+A)+8(R+L)+align4(B)+4*(slice references+slice loans). Native cells use physical X, while fuel uses the logical activation function.

### Runtime 200000/200001

The recursive function retains S=9+k,A2,R1,L1,C1, so X=37+k; main retains S=3+m,A2,P2,O2,L1,C1, so X=31+m. Countdown998 creates999 recursive activations plus main. Choosing k163 and m169 gives each frame200 cells, total200000. All1000 frames have172 scalar slots, total172000, below the200000 scalar cap;1000<1024 frame cap. Changing only m to170 makes the last activation require200001 and it is rejected before install, with999 Enter events and no Return.

Unpadded source's stable logical schedule is50*999+45=49995. Each padding literal contributes one activation slot and one executed scalar instruction, hence fuel49995+2*169+999*2*163=376007, below1000000. Fresh tests assert exact charge total, not just success. These are amended fixture paddings under unchanged ceilings, not relaxed admission.

### Native 8192/8193

Main padding226 yields S229,A1,P2,O2,L1,C1: X229+1+2+8+14+2=256; Dref2040; Dnative8*230+8+8=1856.
Each of30 middle functions padding227 yields S229,A1,R1,L1,C1: X229+1+10+14+2=256; Dref2048; Dnative8*230+16=1856.
Leaf padding245 yields S246,R1: X246+10=256; Dref2048; Dnative8*246+8=1976.
Thus32*256=8192, scalar total229*31+246=7345, native bytes1856*31+1976=59512, depth32 and63 blocks. Each function's S+O is below256. One extra leaf literal gives8193; aggregate cap rejects it. Lowered live cap8191 separately exercises path rejection, avoiding the aggregate gate masking it. Native slot256/257 is separately tested.

### OWNER_CLASSES peak and header capacity

Main owns constructor, staged argument, call result and binding: S2,A2,P4,O4,B16,L1,C2. X=42; Dref=8*4+16+128+112+32=320; Dnative56. Relay has two owners with P2/B8: X10,Dref72,Dnative8. Read has S1,R1: X11,Dref88,Dnative16. Main+read therefore peaks at53 cells and408 active bytes, versus main+relay52/392. Failure origin at read(&x) is correct. Frame is272 bytes and Scalar8; two reserved headers produce960 requested bytes; three headers produce1232 despite only two live frames. Tests check exact-minus-one for cells/bytes and capacity rather than peak header accounting.

Frozen batch source stays unchanged. Main S26,A6,P32,O8,L3,C5 =>X148,D1056; dispatch S4,A2,R1,L1,C1 =>X32,D256; commit S10,R1=>X20,D160. Peak200 cells/1472 active bytes, plus3*272+8=2296 requested bytes. Exact-minus-one seams fail at commit.

## Retained payload and ceilings

- AST borrow/argument56/88 bytes and resolved HIR borrow/argument16/104 retain spans; no new path vector per syntax argument. Resolved program512 and typed-function view40 are measured. TypedBody is144, an extra24-byte Vec header. Sparse entry is72 and FieldId16.
- resolve.rs/typeck.rs charge24 per typed body plus72 per newly requested sparse capacity plus16 per retained path field into one cumulative64MiB projection budget before allocation. Sparse capacity growth is geometric and fallible; sorting work n*(ilog2(n)+1) is debited before sort. Only actual projected borrow sites get sparse entries. Binary lookup is keyed by expression+argument, so nested/interleaved source order cannot select another call's path. Path depth stays64;65 fails. Existing whole loans get no additional path walk.
- OIR LoanDecl grows to96 including its Vec header; actual path fields are separate16-byte payload. lower::projection_path adds exactly path.len() to projection_fields in count and emit passes, calls check_counts before reserving, and emitted counts must equal preflight. source::budget::function_bytes uses size_of<LoanDecl> plus16*projection_fields under existing64MiB raw cap.
- Raw verifier independently recounts loan paths, caps64, and includes projection_fields in ownership events/work/metadata; it does not trust source counts. Ownership metadata/scratch32MiB and work100000000 remain unchanged. Shape rederives nominal field identities and array element types, refusing projected exact-array/record authority. Forged/omitted/65-hop paths are rejected by actual tests.
- Plan32MiB cap uses measured FunctionPlan184 and CallPlan48; new runtime views live in fixed per-frame R/L arrays, already charged at80/112, with no additional dynamic path allocation. Runtime caps stay1024 frames,200000 scalar slots,200000 expanded cells,16MiB requested bytes,1000000 fuel. Dref<=8X; production bytes are masked by the preceding cell cap (upper1024*272+8+8*200000=1878536<16MiB), so lowered byte tests correctly claim only lowered seams.
- Native retains no field-path clone or new element buffer; PrepareBorrow resolves the checked path and stages the field pointer and actual length. Existing slice length sidecars are charged4 bytes each. Count and render path visits must match; exact text cap and one-under are tested.64-hop fixture measures128 visits on each pass. Existing32KiB emitter transient bound,64MiB IR,16MiB diagnostic inventory,1MiB aggregate/path native storage,8192 physical cells,256 function slots and32 depth remain. Native byte ceiling is likewise masked (8*8192+8=65544<1MiB).

## Authority and count mismatch risks reviewed

Projected fields change view only. Authority remains original owner/reference root; flow conflicts remain whole-root. Runtime validates handle/loan view equality and root/generation, checks view extent, and rederives projected offset/type from immutable loan-path ancestry (bounded by frame limit). Nested slice forwarding preserves parent view; exact record references have offset0. Index arithmetic is checked before leaf access. Mutant tests forge both handle and loan view and cover forwarding; successful writes preserve neighboring sentinel/padding. No retained view can authorize an independent sibling-field borrow.

Count/render equality and lower preflight/emission equality explicitly cover newly introduced path counts. A declaration-layout-only extractor is insufficient for the changed runtime/typed-body layouts; this evidence includes the exact-source measured runtime and retained-source tests as well as code-derived formulas.

## Fresh commands and results

Run from the immutable snapshot with SHA-verified debug and release unit binaries, each with --nocapture --test-threads=1:
- source_resources:8 passed,1 ignored in each profile (includes runtime200000/200001, frames1024/1025, owner/batch seams, native8192/8193 and slots256/257)
- reviewer_source_resource_reserved_headers:1 passed each
- projected_slice_retained_source_layouts:1 passed each
- aggregate_seam_retains_raw:1 passed each
- projection_payload_admission:1 passed each
- projected_:14 passed,4 ignored each (overlap with above measured layout; includes poisoned views, forged paths, native output cap/count-render and64-hop controls)

Then source toolchains/llvm19/env.sh, verify llvm-as reports19.1.7, set OXID_OWNED_NATIVE_EVIDENCE to native-debug or native-release, and run source_native_actual_slot_and_cell_boundaries_use_real_llvm --ignored --nocapture --test-threads=1. Both profiles:1 passed,0 failed. Each generated exact-source slots256 and cells8192 LLVM, assembled it with19.1.7, compiled ELF, removed intermediates, ran with empty environment/no tools and only the ELF present: stdout(), empty stderr,status0. Four LLVM/ELF pairs retained. No heavy duplicate builds.

Logs and artifact hashes are in this evidence directory; evidence-index.json binds all deliverables other than itself. Residual: no hosted CI or cross-platform execution here, and final immutable qualification/resource appendix must be reviewed after incorporation. Scope is current production source and explicit resource successor, not unrelated unpublished packaging.
