# Experimental V3 producer closures

These parser and static-analysis closures implement the V3 wire format for the
experimental checked-HIR host routes. The host supports explicit V3 producer
bundles and supplied observations; these fixtures do not change default provider
selection. V1/V2 behavior and their historical source bindings remain separate.
Source-bound integration and full hosted qualification are not established by
the finite local results described below.

Materialize independent closures from the repository root:

    python scripts/build_v3_parser_feasibility.py --output <fresh-parser-directory>
    python scripts/materialize_v3_static.py <fresh-static-directory>

Compile parser_main.ox and ast_static_main.ox respectively using the existing
compiler with --edition=typed-preview --backend=llvm --entry-mode=process and
--output <fresh-executable>. Set OXID_LLVM_BIN to the supported LLVM19 bin directory.
Runtime input/output uses stdin/stdout; no other files are consulted.

The parser reads at most511 ASCII bytes, retains128 non-EOF tokens including trivia,
128 AST rows,129 continuation slots and depth64. Source storage is128 i32 cells,
four validated base128 ASCII bytes per cell. source_bytes::at(codes,offset) requires
caller-proved0<=offset<used<=511; put additionally requires ASCII and a fresh lane.
The separate used length preserves embedded NUL and exact full source spellings.

OPA3 header: magic[4],error:u8,detail:u8,start:u16LE,end:u16LE,rows:u8,items:u8,used:u16LE.
Success adds five129-cell u16 columns, serialized low-byte plane then high-byte plane:
kind+64*next, start, end, a+256*b, c+256*d. Total success1304 bytes; error14 bytes.
AST3: magic[4],used:u16LE,exact source bytes,successful OPA3. Maximum1821 bytes.
Static rows normalize to header=kind+64*(start+512*end), ab=a+256*b+65536*next, cd=c+256*d.
Raw inactive cells and every wire field are checked before normalization.
Static input validation retains the entire source/grammar/graph/allocation closure,
then echoes exact OPA3 and runs complete resolution, typing and flow validation.

STF3 header: magic[4],tag:u8,rows:u8,kind:u8,start:u16LE,end:u16LE,
secondary_start:u16LE,secondary_end:u16LE,label:u8,expected_type:u8,actual_type:u8,
expected_count:u8,actual_count:u8. Success appends the unchanged two129-cell i32
fact columns with four byte planes. Success1052 bytes; diagnostic20 bytes.
Combined static success2356 bytes; static diagnostic1324 bytes.

The static closure uses its own static_span module. It does not import the parser's
parser_state; the unused parser-only release helper is omitted. Actual static
validation owner clearing and phase checks remain intact. All link traversals and
claim signatures use the moved link in ab. Missing-return spans use nine-bit stride.

Measured unchanged-budget admission: baseline static I/W8010/4416. Current optimized
V3 static I/W8192/4713; current optimized V3 parser I/W6847/3909, all against8192 ceilings.
Historical unoptimized V3 inventories were static7958/4317 and parser6777/3769. Canonical authority remains the host's
unchanged frontend; these standalone observations confer no compiler authority.

## Bounded batching and failure precedence

The V3 static producer reads each high-byte plane as one exact 129-byte batch before validating its fields. Source input similarly uses exact 14-byte batches and a bounded scalar tail. A malformed early cell can therefore wait for the remainder of its current batch. If a later read in that batch returns an I/O error, the result is exit74 rather than the predecessor's immediate malformed-input exit64; both produce no output. A short batch is rejected before any stale or unfilled cells are used. This is bounded fail-closed batching, not universal equivalence of I/O failure timing or error priority.

Every batch stays within its declared source segment or current 129-byte plane. Each request is limited to the remaining declared batch length, without reading past that boundary; exact EOF, trailing-byte, mixed-version, raw-field, and inactive-cell checks remain. The existing host-supervisor/test-harness deadline and fuel/admission limits remain unchanged. A raw standalone blocking read does not itself impose a five-second wall-clock deadline; external supervision is required to bound elapsed waiting. Batching bounds the requested byte count, not standalone wall-clock waiting. V1 and V2 input behavior is unchanged.

The batching contract deliberately permits a 64-to-74 error-priority difference with empty output when a later read in the same batch fails. Regression qualification must cover partial high-plane lengths 0..128 versus exact length 129, source truncation, fragmented writes and injected I/O errors. Successful fail-closed checks do not imply identical error timing or priority.

## Exact name comparison capacity correction

An earlier optimized revision exhausted fuel on legal common-prefix-name workloads. The correction uses exact chunked name comparison in static_common and exposes the existing source_bytes::factor helper to that implementation. Parser and static materializers use the same shared source file. The earlier failures remain regression cases; their expected results are not rewritten to match this revision.

Current static inventory is I8192/W4713: exactly the unchanged instruction ceiling, with ZERO instruction headroom. Parser inventory remains I6847/W3909. The prior optimized static I8144/W4713 result is historical. No resource limit, input/token/row/depth bound, scope rule, or diagnostic ordering is changed.

After the existing nonempty, length and source-end checks, name comparison consumes exact four-byte base128 chunks and compares the remaining1–3 bytes individually. The private word helper is called only when four bytes remain. Because end<=used<=511, chunk start<=507, and both packed-bank indexes are valid. For factor=128^(start%4), extraction is current/factor+(next%factor)*(268435456/factor). Modulo precedes multiplication, and the result is at most268435455. At lane0 the neighboring packed-cell value is masked to zero. This is a bounded storage read, not another stream read; adjacent bytes, embedded NUL and exact full spelling remain significant only within the requested name span. The public factor helper requires a caller-proved valid source offset. Lookup order and scope traversal are unchanged.

The previously frozen finite tests missed legal common-prefix-name workloads. Those failures are retained as regression evidence. The replacement passes all64 supplied stress-packet cases (62 legal successes and2 canonical-invalid refusals), with exact native/reference hashes and no strict/advisory/parser differences, plus prior regressions and alignment/NUL/tail/scope controls. Worst measured retained stress fuel is989408 of1000000. These finite observations do not establish a global fuel guarantee for every admitted input. These local finite results do not replace qualification against the eventual source-bound integration checkpoint and its supported host matrix.
