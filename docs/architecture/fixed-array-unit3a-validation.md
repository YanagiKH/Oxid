# Dormant fixed-array source frontend validation

Unit3A implements private parser, AST, inventory, selector and structural-query
support for [RFC 0016](../../rfcs/0016-fixed-scalar-arrays.md). The production
parser always selects the Closed policy; Candidate exists only in test builds.
Public array grammar and production raw-array admission remain closed. Local
frontend qualification covers Linux x86_64 with Rust 1.99.0. Expression typing,
HIR production, lowering, ownership execution, source-native support and public
activation remain later work.

## Implemented boundary

The private parser recognizes scalar array types and array-reference parameter
types, lengths 0 through 1024, literals, direct named-base indexing, indexed
assignment and exact named-base `.len()`. It interprets brackets from the
existing Unsupported tokens, preserving the production lexer tape. No new
SourceMode, command-line/environment switch, type interner or general place
expression is introduced.

An indexed assignment retains one direct IndexRead target wrapper in the AST.
That node records syntax and its index child; it is not an emitted read. Actual
literal-vector entries enter project inventory. Counted validation checks all
new spans, descriptors and edges, and both existing source selectors recognize
every new form across the loaded module set. Parser provenance remains private
and non-Clone.

Checked declaration queries return the existing FixedArrayTy/AggregateTy and
ParameterTy representations. Array types do not allocate nominal declarations.
The existing nominal-reference parameter conversion moved into the shared query
facade with its prior order and work debit. Both scalar and owned source
resolvers explicitly fence array syntax before HIR construction. No executable
witness, callback, raw factory or fallback route is exposed.

The [design and resource derivation](fixed-array-unit3a-design.md) records the
exact interfaces and checkpoint boundary.

## Source and authority identities

The tested source starts from commit
`d582d1dca2a26eb3230632a921f9e8166c39d5c0`, tree
`a826c1782dac19b8c34bdafb0202240e5d414719`, plus the 13-file Unit3A patch. Its
1,092-file source archive SHA-256 is
`aace15bc61711ce9f7731c435f5618eb9b6a1118da4f226e9172b4b053891624`;
the file manifest is
`ddd3f8d02053301ff9e56ac550de8874bbed2dcaac632776f1c4efcf6eebc80e`.
The exact implementation files were published at
`48bc2c5f50ace4052a103304b9a80524e5deb4d0`, tree
`4d06adf8feb004d2b39e2b5ff945c0c13606fd42`. That publication also preserves the
disjoint authority and fixture-registration commits. It does not relabel local
source-bound results as hosted results on the publication head.

The [core source authority](../../tests/fixtures/fixed_array_source_unit3/contracts-v2/README.md)
has freeze SHA-256
`45e01b88abfd91a1a30eb80cfa65c033a1b2986f365d05caa44836c8051231bf`:
58 frontend cases, 29 later resolver/type cases and two later effect cases.
The [six-case boundary supplement](../../tests/fixtures/fixed_array_source_unit3/element-boundary-supplement-v1/README.md)
has freeze SHA-256
`a8c8454ac11355708cebbd749ade26335ed477e6649da75089184c5b7162fb00`.
Their source bytes, origins and diagnostic expectations were independently
reviewed before compiler observations. All 120 core and ten supplemental bodies
were rehashed before and after qualification. The 107 published `.ox` inputs
are individually registered prospective/negative data; they are not counted as
successful language examples or runnable programs.

## Observed local qualification

| Gate | Actual result |
| --- | --- |
| Ordinary debug, all targets/features | 900 passed, 0 failed, 27 ignored across 17 suites |
| Formatting and strict Clippy | Pass, including all targets/features with warnings denied |
| Focused Unit3A controls | 17 pass independently in debug and release |
| Frozen frontend comparisons | 58 pass per profile |
| Separate literal-boundary comparisons | 6 pass per profile |
| Lowered observer limits | 4 explicit incomplete outcomes per profile |
| External comparison mutations | 18 scoped mutations reject |
| Profile artifact equality | All 68 frontend/boundary/limit artifacts are byte-identical |

The ordinary debug total contains the 883 predecessor tests and the 17 new
controls; the focused reruns do not add unique tests. The complete ordinary
release suite was not rerun in this checkpoint. Release evidence consists of
the independently built source observer and the stated focused frontend/resource
checks. No LLVM or wider runtime corpus was invoked for this slice.

The uninstrumented copied debug test binary has SHA-256
`ce9c0d2e7f5c7d9e099fb85ac56cab9889aba4f5ed7c398227d0a686a6b0cac6`.
The observer debug and release binaries are respectively
`5a6b591d262e22df842078a86a207d6f76c246ab63a044b9511e0884f9295f8b`
and `4b550d1408df166b90ad2b2a4f00485691fa5db9b41814f761d4d28caf7405b1`.
Their assembled source manifest is
`f091756f3f6c9a38a8896bec6040673a74480d4c1f45bd2e51b09011d6857198`.

## Resource evidence

Measured rows agree in debug and release: Program 248, Function 200, Param 88,
TypeSyntax 64, Expr 88 and Stmt 136 bytes. FixedArraySyntax is four bytes. HIR
rows remain unchanged. The parser-produced-tree 288N derivation charges a
function, outer block and item at 288; a statement with two introduced child
blocks at 280; and an expression with its external call-argument wrapper at
most 176. An array element edge costs eight bytes at its distinct child root,
disjoint from that wrapper. The store-target wrapper is its own charged node.
These are headers and requested logical payload, not vector capacity, allocator
overhead or RSS measurements.

Project inventory counts actual retained literal entries. In-flight literal
vectors contribute at most one unfilled logical edge per active literal plus
headers. Counted validation remains within `11N + 2T + 4` per source. The selector
is a metered row walk; structural queries add no table or spelling rescan.
Decimal length parsing adds at most `2B` parser byte inspections outside index
work accounting. Existing cumulative source/token/node, module, argument and
depth limits remain; 1024-element literals are distinct from 256-argument calls.

The 63,002-byte unsupported-token control measures the actual lexer/parser
allocation path. Candidate formatting uses 12 successful allocations,
1,568 retained and 3,272 peak requested bytes. The unchanged public formatter
uses nine allocations, 126,622 retained and 127,270 peak requested bytes. Source
and token-tape lifetimes prevent an untracked free from masking the measured
peak. Separate helper accounting confirms one bounded message reservation and
at most 1024 copied bytes. This does not establish universal OOM recovery.

## Observation and comparison boundaries

The test-only observer uses the actual loader, parser, provenance validator,
declaration index, selectors and checked queries. It receives source paths and
downward-only limits; expected values never enter the compiler. It exports
actual IDs, requested edges/roles, type-use origins, descriptors, diagnostics
and inventories. It returns no HIR, executable program or witness.

Rows are counted before fallible output reservation. Limits are 200,000 rows
and 16 MiB serialized bytes, with separate bounds for presentation maps, target
flags and stored query values. Request input has one hard-bounded descriptor
read. Fresh absent output directories, create-new writes and an exclusive
one-artifact-per-case roster prevent an old success from surviving a new error.
Incomplete observations never qualify a source case.

The external comparator checks requested source roles and field relationships,
including a literal element versus an identically spelled type annotation.
It checks mandatory source/identity/envelope integrity, exact requested
frontend diagnostics and type uses. Exported generic fields outside those
explicit assertions do not acquire a correctness claim merely by being present.
The temporary pre-HIR fence is covered for code, message and valid origin by
focused tests; its separate future full-origin roster is not credited as an
independent executed corpus.

## Preserved corrections and remaining work

Source-only review corrected a draft qualified module path and a unit-element
origin before observations; original authorities and superseded hashes remain
separate. Static implementation review then corrected the after-comma boundary:
EOF/nonstarter keeps its syntax error, while a possible excess expression
prefix fails before parsing/reservation. A second correction replaced a new
full-token formatter allocation with the existing bounded borrowed formatter.
Public behavior was preserved.

Observer review closed stale-success/error coexistence, supplementary binding,
source-identity and request-read gaps before semantic observations. The first
mutation run had a helper parameter-name collision; the next rejected 17/18
because one mutation targeted an unrequested scalar query. Retargeting only that
control to the frozen requested array use produced 18/18 rejection. Compiler,
authority, observations and comparison acceptance rules did not change. The
original failures are preserved with their own identities.

Independent review of the later replay packaging found that its new input
checker recorded relocated request hashes without checking every request field.
Empty or altered label/mode/limit/path rows could therefore pass that identity
check. The successor binds the exact 58+6+4 tuples to the frozen authorities
and preserves the first failing controls. This packaging correction does not
change the executed observer, comparison rules or historical observations.

The 31 later semantic/effect cases remain deferred. HIR ordering, array typing,
source-to-raw lowering and constructor operands, source association, ownership
behavior, source reference/native execution and the pilot still require their
own evidence. Public activation and exact-head hosted qualification remain
separate gates. The [replay package](../../tests/fixtures/fixed_array_source_unit3/replay-v1/README.md)
preserves the executed observer and external comparison components with explicit
historical versus future-current source identities.
