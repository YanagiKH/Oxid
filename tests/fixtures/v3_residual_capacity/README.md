# V3 residual-capacity regression corpus

Corpus attribution: fufu, as identified in the original supplied stress packet.
No license or copyright notice in the supplied files has been removed.

The corpus contains 58 coverage cases in cases/ and six minimized boundary
probes in probes/. The probes include two previously failing common-prefix-name
workloads and four passing neighboring controls. expected.tsv records all 64
inputs: source length, token/depth metadata, canonical acceptance, parser hashes,
and strict or advisory static reference hashes. There are 62 legal successes
and two canonical-invalid refusals. Keep the exact source and oracle bytes.

The original optimized baseline, identified in the oracle as aedae8, passed
56 of 64 cases; six coverage cases and two minimized probes exhausted fuel.
The corrected exact-name implementation passes this finite packet locally.
Neither result is a worst-case fuel bound for all admitted inputs. Strict hash
mismatches must fail; any advisory or parser difference requires a technical
explanation and separate review, never an edited oracle that absorbs the change.

Run the repository harness scripts/qualify_v3_residual_packet.py with explicit
--compiler, --llvm-bin, --bundle, --old-bundle and a new --output directory.
It checks canonical, live-producer and supplied-import routes, exact producer
invocation bindings, output preservation on refusal and selected frame mutants.
See scripts/build_hir_producers_v3.py for building an explicit local V3 bundle.
The old bundle is an explicitly supplied earlier producer bundle used for
historical failure controls. These scripts
produce local receipts; they do not create a compiler source authority.

ORIGINAL-PACKET-SHA256SUMS preserves the original supplied checksum bytes under
an explicit historical name. It describes the original larger archive, including
11 files not shipped here: six baseline reports, four development/provenance
scripts and the original standalone runner. Its README entry binds the original
packet README, not this rewritten repository README; that original identity is
abe9e4b34b3e16e55be38021f75fd61a7523047f81355c5669cc7e509423656e.
This historical inventory must not be used to verify the current directory.
The original private reports and environment-specific tools are unnecessary
for the repository's canonical harness and are not imported implicitly.
PUBLISHED-SHA256SUMS lists every other shipped regular file in this directory,
including ORIGINAL-PACKET-SHA256SUMS and this README, and excludes itself.
Verify it from this directory with sha256sum --check PUBLISHED-SHA256SUMS.
