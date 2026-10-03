# Typed project index: independent resource and ownership proof

Reviewed candidate-v4: `9aaadaf0567378a862ddfdbcf163045ba6fb696af88e0eac624b63f3a77abdfb` (423 source files). Qualification is local Linux x86_64, Rust1.98.1. This is the private Unit2 index/resource gate; public project syntax and linked execution remain closed.

## Representation and requested space

Let A=F+R+M-1, where F/R/K are original functions, records and record fields, M loaded modules and U imports. Eight repr(C) u32-only row sizes were independently measured in both profiles: Original36, Function12, Record28, Field16, Module60, Import20, Alias16, Seen8 bytes. The immutable index header is280; Facts472, Plan96, Counts72, Scratch96, SourceOwner32 and a prepared nominal name576 bytes.

The ten retained vector lengths give the exact requested index formula:

    I =40A+12F+28R+16K+60M+4(M-1)+40U+280

No row owns a nested allocation or copied identifier. Four scratch vectors have lengths max(A,U), U, U and M, with elements4/4/8/4 bytes. Their conservative charged bound is:

    J =4max(A,U)+12U+4M+3528

The3528-byte fixed envelope explicitly includes builder/table/plan headers, counters, traversal state, transaction state, two prepared-name objects, source/query handles and copied row/span state. It conservatively sums some disjoint lifetimes; it is not a measured Rust call-stack frame or RSS.

For OriginalSingleFile, M=1, U=0 and A+K<=N<=100,000, so:

    I <=68N+60+280 <=6,800,340 <33,554,432
    J <=4N+4+3528 <=403,532 <16,777,216

Sources/ASTs have a separate inventory. Fresh measured SourceFile/Program/Function sizes are88/248/200. Function+outer block+ItemId costs288 bytes per function node; statements including up to two child blocks cost280; other disjoint charged categories are at most176. Value-argument wrappers attach to distinct expression roots, and absolute-path rows have at least two charged segments. Therefore AST payload<=288N, with Program headers counted separately. One hundred empty functions actually retain28,800 AST payload bytes. The earlier280N envelope is not reused.

## Original-input W theorem

B<=1,048,576 is source bytes; T,N<=100,000 are non-EOF tokens (including trivia) and charged syntax nodes. H=17 bounds both merge levels and three-way binary-search probes. For real parser-produced trees, including semantic failures:

| Source-derived component | Upper bound |
| --- | ---: |
| Complete validation and collection inventory |13N+2T+5 |
| New complete route scan, if used |6N+2B+1 |
| All admitted row/sort initialization |4N+2 |
| Fill/domain/group/conflict/freeze visits |8N+3 |
| Original sort, adjacent comparisons, reserved/main classification |(H+3)A+(H+4)B |
| Namespace requests, including facade and builtin work |160T+76B |
| Additional construction/field/exposure checks |2N |
| At most100 errors, two prepared original names each |13,000 |

Validation charges every callback: two per token including EOF, two fixed visits, and at most11 per charged node. This establishes V<=11N+2T+4 before the separate collector inventory.

The stable-merge byte bound charges each comparison's inspected paired bytes to its emitted operand, once per merge level. The original name-byte total is at most B. Searches use one three-way comparator per probe, including the first unequal byte. Each physical name occurrence has at most two actual namespace requests; four facade events and4B spelling bytes provide conservative allowance for nested facade calls. Scalar parameter types and owned explicit local annotations are the repeated sites. Permission/domain and formatting work are accounted separately.

Using A<=N, the table sums to:

    W <=53N+162T+99B+13,011
      <=64N+168T+104B+32,768
      <=132,284,672 <256,000,000

This replaces the incomplete121,960,576 proposal. Actual validation, route scans, row initialization and nominal formatting are included. The mandatory build formula is checked against already charged work before any index reserve; it is a reservation check, not an extra debit. Its remaining build component is:

    16(A+K+M+U)+128
    + sum[(h(n)+1)(n+L)] for (n,L)=(A,Lo),(U,La),(U,Lp)
    +2A+4Lo

There is one nonresetting meter through signatures, bodies and owned typing. Checked arithmetic precedes I, J, mandatory W and allocation. Old aggregate declaration admission retains its first position. Every new table/scratch vector uses a real fallible reserve, then fixed admitted length without hidden growth.

## Independent execution and ownership evidence

Fresh candidate-v4 resource runs pass **21/21 in debug and release**. They include:

- Nine independently prescribed space cases; exact/one-below I/J/W and zero-reserve rejection; old4097-record/1025-field admission winning over I=J=W=0
- All14 actual index reserve-failure positions and all5 parser positions, checked overflow, Q/N precedence and cleanup/no final index escape
- Hand-cost controls:202-unit mandatory admission;10/7-unit successful searches;17/7/24-unit missing endpoint/intermediate/private endpoint decisions
- Every remaining budget0..46 for a paired import whose finish cost is46, confirming no partial alias/seen commit on failure
-19,531 production stable-merge sequences, plus65,536-byte keys (65,537 exact work) and normalized long path comparisons (65,549 exact work)
- Malformed source/type/span/arena owners, nonzero original file IDs, source generation exhaustion and8,000 concurrent unique assignments
- Four unqualified/qualified reference-exposure origins and their0/1-unit work controls

**18 genuine sibling privacy/lifetime probes** and the **unchanged22-probe ownership gate** passed on v3. They are explicitly rebound to v4: all sealed/source-owner/resource files, relevant witness files and the owned carrier/constructor region are byte-identical. The only production delta selects the checked nominal referent span for E0207, without an extra namespace query or debit. Fresh v4 compilation/runtime controls exercise that delta.

Opaque SourceOwner construction binds actual map membership and non-Clone parser provenance. Full source/AST validation runs once before immutable facts. A dedicated child owns CleanOriginals/facts/index construction, and callers cannot replace sources, tables or domains. The owned HIR owns or borrows the same index without a self-reference or second namespace. The seal certifies original conflicts and atomic imports, not later typing or ownership.

The source generation counter is a checked, nonwrapping process-local AtomicU64. Successful identities are never reused, including after later allocation failure. Exhaustion fails closed. Relaxed ordering provides uniqueness; Rust ownership/borrowing supplies data synchronization. Generation assignment does not affect language IDs or normal diagnostics.

## Trace binding and limitations

The seven frozen failing legacy schedules are separate from nine added complete original-source controls (seven successful type checks and two intentional E0300 mismatches). Independent v3 analysis passed16 cases/profile: at most4 facade events/span, total<=4T and queried bytes<=4B; debit sums equal final counters; full traces match across profiles. V4 independently reran all nine added controls in both debug and release: the profiles match exactly, every query tuple and operation/unit sequence is unchanged from v3, and exactly the three prescribed reference-exposure origins change. Fresh receipts and analysis are under `v4-query-controls/`; the seven v3 failing-schedule controls remain separately identified.

These are requested frontend-storage and explicit-work guarantees. They exclude Vec capacity/allocator rounding, fragmentation, compiler-generated stack frames, total RSS, process OOM recovery, diagnostics rendering, OS blocking and unrelated compiler/LLVM work. Test trace storage is separate. Fallible fault tests induce actual capacity errors, not system-wide OOM. Internal/reduced seams are not claimed as reachable default-source cap maxima. Local Rust1.98.1 layouts do not establish hosted Rust1.99.0 layouts; no new linked/native execution is claimed.

## Reproduction artifacts

`run-resource-review.py` verifies the complete supplied freeze and manifest hash, copies it into a new caller-selected output directory, installs only the independent tests, and runs the21 controls in debug/release with at most two Cargo jobs. Its generated426-file tree was verified byte-for-byte against the actually tested tree. `run-privacy-probes.py` preserves individual compiler diagnostics for the18 sibling probes. The unmodified predecessor script supplies its separate22-probe gate. `analyze-query-traces.py` performs the independent physical-span and source-byte aggregation; `v4-query-controls/` contains the fresh observer receipts and v3-to-v4 comparisons.

Full derivation and coverage are in `resource-ownership-review.md`; exact source, instrumentation, toolchain, scripts and result hashes are bound by `artifact-manifest.json`. No production change is authorized or made by these review tools.
