# Private typed-project Unit2 qualification

This package exercises the real private parser/loader/index/type-check pipeline.
It does not activate public project syntax or linked execution.

```
python3 -B tests/fixtures/typed_project_unit2_independent/run.py --repo . --output /tmp/new-unit2-results
python3 -B -m unittest discover -s tests/fixtures/typed_project_unit2_independent -p test_protocol.py -v
```

Use a Linux x86_64 host with the qualified Rust/C/C++ toolchains on PATH and
Cargo dependencies already cached. The runner is offline and locked; it uses
at most two Cargo jobs. `--cargo` and `--rustc` select explicit executable paths.
The output directory must be new and outside the input repository and package.
`--prepare-only` verifies and assembles the inputs without executing a compiler;
it creates a `prepared.json`, never a passing qualification receipt.

The source manifest binds 113 compiler/build inputs from v4, full-freeze SHA-256
`9aaadaf0567378a862ddfdbcf163045ba6fb696af88e0eac624b63f3a77abdfb`.
All files below src/native are checked for exact membership as well as contents.
Additional required inputs are Cargo.toml/Cargo.lock/build.rs and the existing
embedded RFC/fixture/demo sources. Documentation, test packages and CI files
may be integrated without changing those compiler inputs. This is a separate
Unit2 input manifest; historical Unit1 identities are not rewritten.

Assembly copies only those checked files into an isolated candidate, then adds
the reviewed cfg(test) observer, the exact read-source-entry callback and the
unchanged independent resource payloads. The production input tree is never
edited. Assembly records every instrumentation delta and hashes all 117 resulting
files. The original reviewer files are preserved under archive; the portable
semantic files retain the reviewer's explicit path-only export changes.

The static corpus contains 3,603 final cases: 93 original examples, 2,884 bounded
visibility cases, 512 original import cases plus 64 reviewed module-B replacements,
34 parser/loading cases, seven legacy scheduling cases and nine grammar controls.
Its 1,050,383-byte gzip expands to at most the checked 13,747,865-byte payload.
Sources total 366,806 bytes across 12,867 files. Materialization preserves exact
requests and source bytes, 3,539 explicit creation orders and the source_files-order
fallback used for the 64 module-B replacements. It writes sources/requests and a
queue only. Expected values stay in the static corpus, consumed by a separate
comparison process. No replacement semantic model is introduced.

Each profile builds one isolated test executable. The runner first requires
the exact observer test name and all 21 expected resource names in Rust's test
list. It runs the exact observer once, then the resource filter separately.
Every named test must report `ok`; the terminal reports must show one and 21
passes respectively, zero failures/ignored/measured tests. Unrelated filtered
tests are recorded separately. Raw and normalized streams and comparison results
must contain exactly the 3,603 unique prescribed IDs. A match count alone is
insufficient. Debug and release must both complete.

Receipts bind a fresh invocation identity, source/package/queue manifests,
profile, test binary and every prescribed output stream. The runner refuses old
output directories, verifies inputs before and after, and records command argv,
status, timestamps and output hashes. Protocol controls reject zero execution,
missing/duplicate/unexpected IDs, absent terminal success, stale invocation or
binary bindings, changed artifacts, missing profiles and path aliases.

The read callback observes logical Loader::read_source entry, including lossless
native encoded path bytes. It does not trace OS calls or strengthen the existing
filesystem policy. The nine complete original query controls, seven exact
predecessor rendering comparisons, sibling privacy probes, ordinary/public
regressions and source-free native smoke remain separately identified evidence;
they are not added to the portable 3,603-case or 21-test counts.

The independent resource derivation is preserved at
[publication-resource-proof.md](archive/resource/publication-resource-proof.md).
Its original reproduction script and payload hashes are archived as well.
The combined assembly/execution wrapper is a new adapter and needs its own
qualification; inherited source-review results do not establish runner execution.
