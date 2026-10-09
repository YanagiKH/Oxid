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
