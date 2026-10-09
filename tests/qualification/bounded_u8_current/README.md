# RFC0030 prescribed current-source qualification

This package adds a gate; it does not replace enum, stdin/stdout, producer,
parser, Unit1/2/3/4 or general repository gates.

The `oracle/` directory preserves 67 independent design inputs byte-for-byte:
64 pair fixtures, two roundtrip fixtures and their original coverage manifest.
The manifest's `proposed-not-compiler-executed` status is intentionally retained:
it describes the original input generation, not a current execution result.
The runner pins its digest and the two roundtrip digests, and independently
reconstructs every original partition's Boolean expectations from Python integers.
It never derives expected predicates from the implementation under test.

Each route covers all 393,216 unique `(left, right, operator)` keys (256 squared
ordered pairs times six operators), with an explicit coverage bitmap and digest.
The immutable fixtures run through reference execution. Their original native
batch-zero rejection at the unchanged 256-slot/local limit is retained. For
native admission, byte-identical loop partition bodies are moved into bounded,
nonrecursive helpers. A failure-preserving combiner prevents negative mismatch
results from being concealed by positive counts. Adapted reference execution and
source-free native execution must both equal the independent expected count.
All 256 narrow/widen roundtrips run through reference and native execution too.

`tests.json` is the exact 115-test u8 roster from source commit
`5e4875d19961b4eba8e465c915ac676c54a9926e`, tree
`4c687ed5ead4786e34cea154e3b263c00bd1fd1b`. The gate requires exact discovery,
runs the 111 ordinary resource/semantic tests with uncaptured measurement output,
and explicitly executes all four ignored tests. The ignored tests must retain
16 scalar and 75 owned real ELF/IR/stream/status cases. The three owned tests
use the existing unary-native retention key, `OXID_UNARY_NATIVE_EVIDENCE`.

The prescribed workflow runs `scripts/verify_bounded_u8_native.py` in the
existing pinned Linux x86_64 LLVM 19.1.7 / Rust 1.99.0 job, in both ordinary
Cargo profiles. It admits actual Cargo build receipts from the predecessor
bounded-enum gate, exact clean CI head/tree and the reviewed source manifest;
it does not infer executable identity from a filename. It invokes the existing
unaltered `stage_llvm_runtime.py` with real dpkg ownership/version/hash checks.
No extracted-package or hand-written stage receipt substitute is accepted.
Frozen v1/v2 producer compatibility is separately executed in each profile.

Every command retains its exact arguments, status and original streams, including
failure/timeout evidence. The final result remains failed unless every stage and
before/after identity check succeeds. CI uploads the evidence even on failure;
staged library copies are omitted from upload, but the genuine staging receipt
and all required executable validation artifacts remain. Source-free means an
empty execution working directory except for the ELF, with a minimal environment,
not filesystem or operating-system isolation. Resource traces do not claim RSS.

Standalone local corpus rehearsal (qualifies only the supplied executable):

    PYTHONDONTWRITEBYTECODE=1 python3 -B scripts/run_bounded_u8_corpus.py \
      --compiler /absolute/path/to/oxid --route scalar --output /fresh/evidence

Repeat with `--route owned`. This standalone invocation does not claim the
hosted exact-head/ordinary-profile/dpkg-staging gate passed.
