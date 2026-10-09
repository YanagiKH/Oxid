# U8-INDEX-INTEGRATION-1: local review checkpoint

This checkpoint integrates RFC0030's namespace reservation into the real index.
It is not complete byte-scalar implementation, qualification or publication.
Base is `5fecd59d1e304a194cae033749dc329481795738`; contract docs come from
`c3920d9bb720d4bb11d0b2a17893e8de7db18c7d`. Historical blocked `8839afa` and
compact test-only `7afd173` trees and receipts remain unchanged. No alternate
production freezer/calculator is transferred.

## Production seam

The single real freeze calls the compact scanner after all original-conflict,
enum-variant, import and final original-validation phases. A complete paid
cross-kind item-source-order pass precedes reservation. Modules use numeric
preorder; items use source order. Imports require both type_target and
current-id type_first_import, preserving function-only aliases.

The core returns reason plus Span without constructing diagnostics. After its
frame returns, the adapter directly calls the same existing constructor with
unchanged allocation/fallback behavior. Borrowed source accesses preserve source
ownership, provenance, bounds and UTF-8 checks. Owner/item lookup failures use
EOF; order/import metadata failures use the current item. Compact debits preserve
existing overflow/limit/mutation/event rules.

## Named resource successor

The single IndexPlan calculator adds checked U before all limits/allocation:

    U = 2M + 3(O+I) + 4(R+E+I)

Exact complete scan work is:

    S = 2M + 3(O+I) + (R+E+I) + sum(candidate comparison costs)

Each candidate comparison costs1..3; only type-owning import aliases are compared.
The checked helper borrows Counts and returns before overflow diagnostics. No
predecessor work cushion is credited; failed prefixes pay only reached work.

The only fixed-ledger substitution is max(old prepared-name pair, complete new
repository-owned phase envelope), computed from actual types in a production
const expression. Measured peak912 <= pair928 preserves fixed4096. All other
terms, row layouts, retained payloads and hard limits stay unchanged. No inherited
counter-bank credit is taken. Core, diagnostic, final-vector and preflight
lifetimes are explicit. This is not a machine-stack, allocator-internals or RSS
claim: unchanged opaque stdlib mechanisms and existing test-observer instrumentation
retain the predecessor exclusion. The private formatting descriptor witness has
an isolated rustc layout probe; RUSTC_BOOTSTRAP is used only for that probe.

Independent endpoint controls preserve historical arithmetic:

- No-u8 Zebra/function fixture: M1/O2/R1, U12, exact S11; every failure prefix.
- Synthetic M1/O2/R1 with zero name bytes: predecessor184, successor196,
  retained552, scratch4108; old/exact/one-less work endpoints.
- Existing enum identity fixture: M3/O15/I2/R3/E5 gives U=6+51+40=97;
  predecessor1586, successor1683. Existing preflight visits are separately paid
  at the same collection gate. Old budget refuses; successor exact/one-less tested.
- Old enum duplicate comparison census remains10 at its original phase boundary.
  New reservation comparisons are separately observed.

## Evidence and remaining gates

Fresh local receipts are retained separately under integration-* in the u8
resource evidence directory. The focused index run passes104 tests, zero ignored;
production check passes. Formatting, Clippy, fresh identity/checksums and independent
review are separate explicit checks. Failed earlier iterations remain preserved.

Tests use the real collection/freeze path. They cover old-domain duplicate,
variant, missing/inaccessible target and alias-conflict vectors; function/type/dual
alias provenance; both nominal kinds and sibling preorder; forged item order,
owner/import metadata; exact work/admission/allocator boundaries; finite comparison,
complete diagnostic/fallback and meter parity. Full parser/provider/runtime/native
qualification remains outstanding. No u8 query, scalar or conversion carrier has
been enabled by this checkpoint. Independent review precedes broader semantics.
