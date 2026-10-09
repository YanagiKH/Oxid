# Bounded standalone byte storage validation

Status: experimental current-source implementation; final integrated admission,
source-only authority seal, full repository verification and exact-head hosted
CI remain pending. [RFC0031](../../rfcs/0031-bounded-standalone-byte-storage.md)
defines the reviewed scope and records the accepted source-authority amendment.
This does not requalify predecessor artifacts or complete a roadmap milestone.

Dependency: merged main b5455ad07081590aeffdec54a927c9095e870fb0, tree
cb53acc04ac88538a74410bc0a3c037da369c02e. Implementation lanes use Rust1.99 and
LLVM/Clang/LLD19.1.7, Linux x86_64 O0; other-host native execution is not claimed.

## Evidence established before the final seal

- Exact-base probe:39 shared syntax/type/query/error/frame carriers measured;
  66 existing carrier controls,22 native-array resource controls and396 owned
  source controls passed (9 native controls remained ignored in that run).
- Same predecessor-source parser capacity probe matched current bool/i32/unit:
  8 nodes,2400 retained bytes,2432 peak requested bytes,7 allocation attempts,
  function/parameter/expression capacities4/4/4. Current u8 has the same measured
  shape. These are explicit payload/capacity observations, not stack/RSS claims.
- Syntax/declaration lane:15 focused,87 declaration,54 parser and3 diagnostic
  controls passed.26 common measured enclosing carriers have unchanged size and
  alignment. Original superseded test sources are retained byte-for-byte under
  tests/qualification/byte_storage_current/predecessor.
- Reference/source-authority lane:35 combined focused tests,38 predecessor
  reference controls and5 production privacy compile probes passed. Source
  authority is minted through trusted lower_and_associate only; cfg(test) raw
  mutation helpers do not create SourceProgram. The measured relocation role
  bound is1336 bytes versus the1576-byte predecessor, with no new retained table.
- Native lane cf24c90a8e02cd9c24a5c1b87a6508065d474f21, tree
  efec421967539d1cdda8166d1132a2997a3e398c:8 new normal native controls,3 public
  CLI boundary controls,103 native regression controls passed. Four new ignored
  gates separately ran93 source-free ELF cases:22 successes and71 expected
  failures. This includes all256 bytes,638 pilot, sentinels/neighbors, RHS128
  snapshot, guarded/acyclic phi/bounds/conversion paths, every fuel0..51 and
  fuel83 E0601 versus84 E0606. Source, IR, ELF, stdout, stderr and status were
  retained for every case. The lane predates the new source-fence fixed-bank
  successor and is not final-head evidence.

The independently frozen v3 corpus (manifest
`de6d63aba53edfe1af802536d044ffbad345509556e804b293381ea5c39ed3a7`)
passed all323 source/reference cases against the preserved pre-resource native
lane CLI. This includes all21 diagnostics independently strengthened from
immutable predecessor templates and validated predecessor analogues. It is not
final-head or all323 native-execution evidence. The v1 failed fixture run remains
preserved; the v2/v3 corrections did not change numeric expectations.

## Resource and authority boundaries

The new record-fence guard captures/receivers/returns are explicitly inventoried
in a separately named fixed HIR successor. Existing banks and the64MiB cap are
unchanged; current exact/one-short endpoints must be verified against independently
executed baseline totals. Declaration table_bytes continues to mean requested
retained output payload; separately measured raw transient guards are not
misreported as output rows. Independent integrated replay on 72e6022 measured
14 bytes (alignment2) per complete source guard role, summed to28 bytes. Default
fixed-plus-dynamic admission moves157896 to157924; unchanged64MiB cap leaves
66950940 bytes. The ordinary source seed moves169210 to169238 and its exact
association endpoint171238 to171266. Both the old endpoint and current one-short
endpoint reject before tracker reservation. Raw guards measure12 bytes each,
summed to24 transient bytes, with zero retained output-table delta. All11
independently selected resource controls passed; final sealed replay is pending.

Native independent tiny examples demand12/92/100 arena bytes and3/13/14
instructions under unchanged caps. Exact/one-short checks passed. Existing i8
transport/copy/formatting callsites are reused unchanged; fresh MIR/LLVM probes
measured complete formatting capture/backing roles. The inherited256 function
slot cap still rejects a1024-element byte constructor before tools/output.

All historical source/protocol authority remains immutable. A new current-source
inverse/membership/include closure and public provider-refusal recipe are being
qualified separately. Neither an old diagnostic artifact nor a valid i32
observation can grant authority for newly valid byte-array source.


## Integrated checks before final test-only closure

Production source at e7b7f97eab586fde6a3d66c67052fdb6042d9d1a passed
fmt and all-target/all-feature clippy with warnings denied. The preceding
source-content-equivalent production revision passed 2185 Rust tests across
31 suites (73 ignored), and the four new native gates separately passed all
93 retained ELF cases. All five source privacy probes passed. The release CLI
SHA-256 is `b83d76e227fa0b19d0d88fbe84d4ad1d3e0ca41eb30fa4b5f763d1e43dac60f6`.

That CLI passed the independent 323-case v3 source/reference corpus, including
49 exact diagnostics. Independent native replay passed all 256 byte transport
cases. Additional independent boundaries accepted a 248-element native owner
and rejected 249 and 1024 under the inherited per-function limits before tools
or output; a 1024-element reference owner retained every 255 and summed to
261120. Repository verification passed 210 language sources, 146 checks and
76 runnable programs. These observations are tied to the preserved CLI, not a
later unbuilt head.

The separate provider recipe now distinguishes 12 unchanged-invalid parity
checks, 12 canonical enum grammar refusals before provider dispatch, and 204
protocol/import refusals across the unchanged v1/v2 builders. It includes an
unused byte helper alongside main, unchanged root bytes with a bool-to-u8 child
module change, and wrong integer/old-error authorities. All 216 refusal checks
preserved existing output, created no new output and invoked no LLVM tools.
The initial enum expectation mistake and later harness-only validation mistake
remain retained failed attempts; immutable baseline probes independently fix
the enum E0100 and multi-file E0703/E0702 precedence.

Test-only closure subsequently adds independently priced byte fuel schedules,
malformed IDs/owner roles, exact store first-error precedence, excluded literal
and enum grammar, the unchanged i32 stdout signature, array-entry check/run
separation, and identical-text cross-file runtime origin checks. Final-source
replay, regenerated current source seals and independent final review remain
required before claiming those later bytes qualified.

## Reproduction and outstanding gates

Use the ordinary repository fmt, clippy, all-target Rust tests, Python discovery
and verify_repo commands in CONTRIBUTING.md. In the pinned native environment,
run the byte_storage native tests with --include-ignored and --test-threads=1,
setting OXID_OWNED_NATIVE_EVIDENCE to an external evidence directory.
The separate scripts/qualify_hir_byte_storage_compatibility.py reuses unchanged
v1/v2 provider builders and writes its own report; RFC0030's recipe is unchanged.

Final source-authority successor controls and every direct CI suite must run,
including Unit2/Unit4 and archived observer staging closures. The actual system
dpkg LLVM stager is unavailable in the local environment; genuine hosted staging
and all applicable exact-head CI jobs are mandatory. No local shim, old CI result,
or successful compilation substitutes for those gates.
## Historical test-only checkpoint and local qualification

The earlier, now superseded compiler-input checkpoint is
`cd5c0e47cd0d40179b916758068ddb320a52a6fb`, with source-only tree
`d29049905ca3a8b9401d40650eeb7d63b841d87c`. The reversible current-source
manifest SHA-256 is
`d5d1492a4873a40a53867d81062eec81f35aeecfa3468b70ceb8ce1f353fbadb`.
It closes 375 inputs, including 287 src/native files, and preserves the original
membership/inverse authorities. The source-qualification integration at a3f1244
has the same compiler inputs. All 694 admission controls passed.

At that historical checkpoint, the ordinary all-target/all-feature Rust run passed 2,197 tests across
31 suites, with 73 ignored; fmt and clippy with warnings denied passed. The
release byte filter passed 63 tests, with four native gates separately executed.
Those four gates passed 93 source-free ELF cases in each ordinary profile.
The final eight public integration tests also passed through Cargo. The later
public-test receipt is separate from the earlier main-unit source receipt.

Independent resource review passed the final source: 11 focused controls,
28 HIR budget controls, 22 inherited native-resource controls and eight trust
controls. The measured 28-byte source successor and exact admission endpoints
above are unchanged. Production authority files and privacy-probe source are
byte-identical to the previously executed five compile-time privacy controls.

Independent semantic review found no production defect. The source/raw local
move and replacement schedules, including N=1024, and 18 borrow schedules cover
every integer fuel budget. Owned by-value calls and results cover every budget
at N=0/1; N=1024 checks 1,070 source and 22 raw exact/one-short debit boundaries,
not every integer budget. Exact malformed IDs/owner roles and a real prepared
owner escaping on a reachable branch are rejected before sealing. Full frozen
scalar-u8 reference/native compatibility was also executed in both scalar and
owned source routes: 393,216 pair/operator keys and 256 round trips per route.

Actual current Unit2 debug/release qualification passed 3,603 semantic cases per
profile, plus its resource and observer controls. The public ordinary/observer
builds passed the original 248 and predecessor 7,814 projections and all 284
explicitly selected non-native lifecycle pairs. The other 34 native lifecycle
pairs remain a hosted gate; this is not a full local public qualification claim.

All four legacy observer builds and their parser/static projection tests passed.
The direct typed lexer, parser and static reference/native proof controllers
passed, including both ordinary parser/static profiles. Streaming component-use
qualification passed 623 cases, 69 exact diagnostics, 82 module invocations and
111 failure-control receipts per profile. Its separate exact hosted CI wrapper
remains required.

Portable parser qualification completed four real compiler builds, both
ordinary passivity controls and both collection profiles. The current comparison
passed all 638 observations with no issues. Historical unadapted/frozen failures
remain retained under their original predicates; the reviewed current amendment
is applied explicitly, without rewriting raw observations or old expectations.

The separately registered RFC0031 checker passed actual 323 reference and
256 native transport cases in each ordinary profile against the preserved public
qualification CLIs. This rehearses its execution and exact comparison paths;
it does not claim the full genuine-dpkg-staged controller has run locally.

Resource approval is specifically incremental accounting, source authority and
the executed boundaries above. It does not turn the independent oracle's 25
compound resource obligations into complete coverage. Remaining distinctions
include work versus byte endpoints, frame count one versus the frame ceiling,
measured native text demand versus independently derived text demand, and
fail-first allocation rejection versus every allocation site. Those broader
claims remain explicitly partial; no production defect was inferred from the
coverage gaps.

### Explicit inherited lexer limitation

Parent approved the RFC0031 B11 clarification recorded in the RFC after reviewing
exact predecessor/current lexer identity and route evidence. The ordinary lexer
already processed the same byte spellings before the later grammar decision;
this increment changes none of its allocation topology. Its infallible token
pushes have no recoverable allocation-failure proof. That subclaim remains
UNPROVED, and the original compound oracle rows remain unchanged rather than
being marked empirical passes. All new/changed byte allocation claims still
require the full applicable controls. Separate lexer hardening is tracked in
the implementation plan and has not been implemented.

### Explicit inherited native limitations

Parent separately approved the RFC's enumerated seven graph-vector families and
call Vec/String/format/join temporaries as unchanged predecessor limitations.
Their allocator recovery/per-site injection remains UNPROVED. Early adjacency
allocation is bounded by the verified raw 300,000-block ceiling, not the later
native 4,096-block check or post-allocation metadata metrics. Logical graph and
call-text bounds are not capacity/RSS guarantees. Frozen independent N0/N1
demands cover their acyclic fixtures; guarded template/topology inheritance and
actual count/render controls remain separately labeled. The original stronger
corpus is unchanged, and separate fallible-preflight hardening is recorded in
the plan rather than marked completed.

## Historical corrected Linux test-source checkpoint

The corrected compiler-input checkpoint is
`b3371e6a383b526d25409f2d4b308b4993739466`, tree
`c168a3d6d6c3e3ba3d6b71b981319cbed9497525`. Its source manifest is
`9e9e65c9ba3b034074ca22cb0967d6820ff5b5f8907613fcb36720b97519b0f8`,
closing 376 inputs, including 288 src/native files. The adapter/CI integration
`b8780569e81959a6200c13444cc5850d0ab2286e` has identical compiler inputs.
Fresh ordinary builds, with recorded source and executable identities, passed
2,210 Rust tests across 31 suites (73 ignored), all-target/all-feature clippy
with warnings denied, and formatting checks. The release byte roster passed
73 tests with four separately registered ignored native gates; all ten public
integration tests passed. All 694 final source-adapter admission controls passed.
The complete closed CI registry contains 77 unit tests, ten public tests,
11 privacy probes and 93 ELF cases from the four ignored native gates.

Targeted resource closure adds independently enumerated HIR/parser/query work,
full frame-ceiling and allocation-topology controls, maximum argument scratch,
raw count/fill reservation controls and independent acyclic N0/N1 native text,
diagnostic and plan demands. Its scoped admission was reviewed with the two
explicit inherited allocator-recovery limitations above. Authentication has no
numeric WORK gate; its checked-count and bounded-traversal proof is separate.
Guarded native topology/count-render evidence does not become an independently
derived guarded text-demand claim. Original compound oracle obligations remain
unchanged; these are explicit applicability dispositions, not 145 empirical
whole-row passes.

Fresh direct-source executions and the final per-profile native/oracle/provider
replays are tracked separately. Genuine hosted staging, the full public native
lifecycle gate and all applicable exact-PR-head CI remain required. Historical
observations above are preserved under their actual source identities; they are
not relabeled as executions of this corrected checkpoint.

## Cross-host test-fixture successor

The reviewed cross-host test checkpoint is
`e3c1b4a1a3ef457326f11a802896c125202fe797`, tree
`5fdb4f06a8fcbc61724676df55a4c3bee130eaf7`. The current-source manifest is
`402db5018af489c30b2a57ed3ef558c055013af2b727a3ad0eb39ffc42125efa`.
Membership remains 376 inputs, 288 src/native files, 78 compile-time fixture
bodies and 136 includes. The closed byte roster is now 79 unit tests: 75 normal
and four ignored native gates. Public tests remain ten, privacy probes eleven,
and the native gate artifact roster remains 93.

The first PR57 hosted run correctly refused filesystem module loading on
unqualified hosts, exposing a test fixture that incorrectly required that route.
The successor changes three test files only. Portable tests use authentic parsed
module fixtures with source/AST ownership checks and exact runtime/parser origins;
Linux also executes actual filesystem-loader parity. Public non-Linux paths
assert the unchanged exact E0005 source refusal. Production host admission,
language semantics and caps are unchanged. The two new portable semantic controls
remain active on every host; no test body is skipped to hide the refusal.

Fresh focused artifacts passed 19 exact unit controls and all ten public tests,
with formatting and all-target/all-feature clippy warnings denied. The workflow
also separates toolchain lookup from export so failures retain their status.
The previous 9e9-source full Linux receipts remain historical under their actual
identities. Fresh closed authority/registry controls and all applicable hosted
direct, foreign-host and native qualification must pass on this successor before
final acceptance; focused tests do not replace those gates.
