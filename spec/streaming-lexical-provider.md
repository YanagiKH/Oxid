# Experimental streaming lexical provider

This explicit Linux x86_64 route selects an Oxid-authored lexical component for
`check`, `run` and LLVM `compile` with `--edition=typed-preview
--experimental-lexical-provider BUNDLE`. Process entry remains supported;
Process Run still forbids JSON mode. HIR import/producer selectors cannot be
combined with this selector. Default routing is unchanged.

The bundle contains exactly selected `lexer` and `manifest.txt` inputs. Manifest
bytes are `oxid-lexical-provider-v1\nlexer_sha256=` followed by 64 lowercase
hex digits and a final newline. The digest is a local identity assertion, not
a signature or provenance authority. Only a verified sealed executable snapshot
is executed. Never select an untrusted executable: supervision controls bounded
transport/lifetime, not operating-system capabilities.

Scalar/array/Process semantics, one invocation per complete source, 1,000,000 fuel
and all compiler limits remain unchanged. The 1 MiB source ceiling is admission,
not a promise that every admitted source finishes within fuel. Failure is closed;
the compiler never falls back to canonical tokens.

## Input LXI1

Exactly 12 header bytes: ASCII LXI1, u32LE source length N, u32LE remaining non-EOF token limit L. Then exactly N raw ASCII bytes and EOF. N <= 1,048,576; L <= 100,000. Invalid header/range, missing or extra source, non-ASCII, and I/O failure must refuse without a successful terminal.

## Output LXS1

Exactly four magic bytes LXS1 followed by records below. Source position fields are full-width u32LE. Token endpoints use the most recent explicit full-width source chunk base plus a checked 0..128 relative endpoint. Every reconstructed start/end is checked against the actual retained SourceFile. This is not byte-wide source admission.

### Echo E (69)

69, used u8, base u32LE, buffer[128]. Extent 134 bytes. Each base equals the cumulative number of prior used echo bytes. A normal read assembles the next128 bytes. Every full source block has used128. The final read is Eof(remainder); when N is a multiple of128 (including empty source), there is one final used0 echo at baseN. Otherwise final used is N%128 and no additional empty echo occurs. No subsequent echo is allowed after that final read.

The used prefix equals source[base..base+used] exactly. Unused tail equals the previous echo buffer's same tail, with an initially zero buffer. This deterministic carry rule is checked exactly; padding is not arbitrary or ignored. It follows existing read_stdin destination semantics and removes needless zeroing loops. Input length/EOF is validated independently of source-byte echo.

### Token pair

kind u8 (1..47), relative_end u8 (0..128). Kind IDs follow the existing unchanged Rust lexer declaration order. global_end=checked(base+relative_end), global_start=previous global_end, initially zero. Non-EOF tokens must advance, have width at most 65,536, and remain within already echoed source; this width cap does not restrict diagnostic spans. EOF is the sole kind47 at [N,N). A token may span any number of chunks and may end at relative0 in a later chunk. No token precedes the first echo; EOF cannot precede the final echo. Full token buffer output consists of exactly 50 such pairs (100 bytes), without a tag.

### Partial token buffer B (66)

66, used u8 (even2..98), buffer[100]. The used prefix contains token pairs under the current base. Unused tail equals the previous complete or partial token buffer's same tail; initial token buffer is all zero. Reject any deviation. Raw token pairs must form complete100-byte groups; B carries the only permitted partial group. No raw partial group can be interrupted by echo or terminal. Full 50-pair groups emit immediately. At most one B partial is allowed per echo base, as its last token record immediately before the next echo or terminal; no raw pair or second B may follow it under that base. On the final read, finish the carried token and append EOF (or determine the diagnostic) before this single partial flush. Pending token bytes are always flushed before the next echo changes the base.

### Success S (83)

83, token count INCLUDING EOF u32LE, source length u32LE. Extent 9 bytes. Requires full exact source echo, sole final EOF, matching count and source extent. One terminal only; any suffix is rejected.

### Lexical diagnostic D (68)

68, diagnostic tag u8, start u32LE, end u32LE. Extent 10 bytes. Tag1=E0100 unterminated string literal; tag2=E0100 unterminated block comment; tag3=E0400 token resource limit exceeded. Stage lex, error severity, no secondary spans/notes. Requires full exact source echo, no EOF token and no success terminal. Diagnostic start equals the end of the complete emitted token prefix. All prior token data is discarded. Tags 1/2 end exactly at N. Terminal diagnostic is compared exactly with unchanged canonical Rust lexing of the same retained source and the same L.

Token width/count checks occur after scanning the complete token, exactly like the canonical lexer; unterminated string/comment therefore takes precedence. EOF is not charged against L. On a resource failure, preserve the first complete offending token's span, flush prior pending tokens before changing echo base, and drain/echo remaining source without further lexing. On an unterminated EOF token, report its start and N. A complete valid diagnostic terminal uses process exit 0, just like a successful token observation. Domain, framing, transport, I/O or runtime failures use nonzero status and never produce an accepted terminal; there is no fallback.

## Host binding and authority

Provider output is untrusted. Strictly reject malformed magic/tags, extent errors, invalid contextual bases, noncanonical chunk or group sizes, bad deterministic padding, noncontiguous/reversed/oversized spans, invalid EOF/count, mismatched source, truncation, terminal suffixes, and diagnostic mismatch. Allocate returned Token spans only against the already-retained SourceFile and retain its identity in the checked carrier. The actual returned producer Vec is moved into the existing parser after comparison; canonical Vec is comparison-only and dropped. Provider failures never select canonical tokens or replace a native output.

Accept either S or D only after complete input write plus stdin EOF, process exit 0, empty stderr, reaped leader, and stdout transport EOF without suffix.

Default route and all v1/v2 HIR, parser/resolver/ownership/OIR/backend authority remain unchanged. Per-file receipts show source identity/path/hash, executable/input/output identities, transport completion, canonical comparison attempted/matched, producer tokens selected for parser, and no fallback. Keep ProjectSources/LoadFailure/builder retained layouts unchanged by passing transient provider state through loading calls. Drop transport/receipt state before ordinary checking.

## Checked transport bound

Let chunks=floor(N/128)+1, max_tokens=min(N,L)+1. A conservative output ceiling is 4 +134*chunks +2*max_tokens +102*(chunks+1) +10, plus one overflow witness. This safely includes every complete/partial token buffer, final flush and diagnostic/success terminal; overcharging is deliberate. Input ceiling12+N; stderr and deadline stay bounded. Use fallible exact Rust vectors and explicit loader-phase storage/work accounting under existing ceilings. Historical HIR transport is untouched. Execution uses a sealed immutable local executable and bounded nonblocking supervisor, not an OS sandbox/RSS promise.

## Qualification boundary

Measure complete framing and exact diagnostics on all 31 real v2 sources, plus a real multi-module source closure for the new lexer itself. Retain exact source/protocol/binary identities. Require complete token/span/echo parity and genuine diagnostic fields, not status alone, under unchanged limits with measured remaining fuel. Component-use, corruption/failure/no-fallback/output-preservation, ordinary debug/release producer replay, current-source binding and full regression gates are separate evidence requirements. See [the qualification ledger](../docs/architecture/streaming-lexical-provider.md) for the current result. No self-hosting claim.


### Platform setup and private-directory cleanup

Named loader accounting covers admitted provider carriers, retained workspace
paths and the explicit source/stream/token allocations. Standard-library
`temp_dir` lookup and path canonicalization can allocate before their resulting
path is checked; those platform-setup internals are outside this named model.
Neither whole-process OOM recovery nor an operating-system allocation/time bound
is claimed.

The owned private cwd is removed only when empty and still the same directory.
Cleanup is nonrecursive: executable-created contents leave that directory in
place rather than triggering an unbounded host traversal. A selected executable
is trusted local code, and may have filesystem capabilities outside that cwd;
process supervision is not a sandbox.
