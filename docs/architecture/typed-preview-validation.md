# Typed-preview local validation

Date: 2026-10-01. Scope: experimental single-file bool/unit checking plus the
legacy syntax extraction and edition gate. This does not validate a memory
model, execution engine, native backend, or completed roadmap milestone.

## Snapshot and host

- Base commit: `eeaebebdfae11312414557168ca7583697497ea9`
- Snapshot: base plus the uncommitted typed-preview implementation reviewed with
  this change; no publication commit is claimed by this local report
- Code/test manifest SHA-256:
  `d1092c05f0d60432ff50a2ded0fa01d484db2a471c51169a2aa2350d9b6fb21d`
- The manifest maps all `src/**/*.rs` and `tests/*.rs` relative paths, in sorted
  order, to their SHA-256 hashes. Its encoding is Python `json.dumps(map,
  indent=2) + "\n"`. Documentation is outside that code/test digest
- Host: Linux x86_64, `x86_64-unknown-linux-gnu`
- Rust: `rustc 1.98.1 (48a229cea 2026-09-01)`; Python 3.12.14
- No new dependencies or toolchain configuration changes

## Fresh integrated checks

| Command | Exit | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | Formatting passes |
| `rustfmt --check --edition 2021 src/legacy/syntax.rs` | 0 | Included legacy module explicitly checked |
| `cargo clippy --all-targets --all-features -- -D warnings` | 0 | No warnings |
| `cargo test --all-targets --all-features --locked` | 0 | 117 Rust tests pass |
| `python3 -m unittest discover -s scripts -p 'test_*.py' -v` | 0 | 7 metadata tests pass |
| `cargo build --release --locked` | 0 | Release executable builds |
| `python3 scripts/verify_repo.py target/release/oxid` | 0 | 121 sources, 67 runnable programs; feature inventory passes |
| `git diff --check` | 0 | No whitespace errors |

Rust counts: 73 unit tests, 12 edition-boundary tests, 15 typed-frontend tests,
1 generated-document test, 8 legacy-module tests, and 8 legacy-semantic tests.
The entire previous 61-test Rust suite remains represented and passing.

The source/diagnostic unit tests cover immutable source identity, invalid internal
spans, CRLF/UTF-8/EOF positions, deterministic text/JSON fields and escaping.
A separate Python decoder check round-tripped every Unicode scalar through the
JSON string encoder. Actual CLI JSON was also decoded independently for paths
containing quotes, backslashes and non-ASCII characters and for exact error ranges.

## Functional evidence

The [specified multi-function example](../../spec/typed-preview.md#grammar)
passed through the release CLI with exit 0 and this JSON terminal record:

```json
{"schema_version":1,"edition":"typed-preview","kind":"check-summary","success":true,"errors":0,"functions":2}
```

For the following source, the same CLI exited 1 with E0300, stage `type`,
message `type mismatch: expected bool, found ()`, and primary bytes `[34, 36)`,
line 2 columns 12–14:

```text
fn example() -> bool {
    return ();
}
```

That result checks return types; it does not execute either example.

Additional tests check forward/recursive direct-call resolution, bool/unit
inference, optional local annotations, function/local IDs, complete expression
and local type tables, duplicate/unknown names, signature/arity mismatches,
terminal returns, unsupported constructs, malformed UTF-8/artifacts, source I/O,
resource boundaries and capped deterministic parser recovery.

The edition suite makes real binary invocations across unsupported operations
and flag placements. It uses a working legacy `write_text` sentinel and compares
entire temporary-project filesystem snapshots. Rejected typed commands do not
execute that sentinel, remove caches, scaffold projects, or create artifacts.
Explicit legacy execution/check/compile/ast retain matching outputs/artifact bytes.
Script payload and separator handling have separate regressions.

## Legacy extraction evidence

The syntax move was verified separately from new semantics. After normalizing
only visibility and rustfmt, the moved AST/parser/lexer/helpers match the original
code exactly. Remaining legacy code changed only at the module/import seam in
that extraction snapshot. All previous 61 tests, formatting, Clippy and release
build passed for the extraction alone.

Six representative programs (`hello`, `oxid_shortcuts`, `arrays`, `records_json`,
`async`, `modules`) were compiled before and after from the same source paths.
All six OXBC artifacts were byte-identical; source/artifact stdout and stderr also
matched. The later edition gate is independently covered by the integrated tests.

## Red/green and limits

The initial eight typed-frontend tests all failed against the legacy path before
implementation. The initial edition suite observed nine failures and one pass;
in particular, a typed `run` request incorrectly executed legacy code. The source,
diagnostic and option helpers also had recorded failing behavior tests before
implementation. A later diagnostic-cap regression reproduced 101 emitted errors
against the specified maximum of 100, then passed after its fix. Independent
review found that a third duplicate function labeled the second declaration as
"first declared here". A regression reproduced the incorrect secondary range,
then passed after retaining the original name-table entry. Both duplicate errors
now point back to the actual first declaration; the full suite was rerun.

Supplemental structural/boundary tests and two extra public CLI edge cases first
ran after integration; their successful first runs are not labeled red/green.

This report covers one local Linux host. Windows, macOS Intel/ARM64, Linux ARM64,
installation and Docker were not rerun for this increment here. Existing CI
configuration is not new cross-platform evidence. No performance or memory-safety
certification, OIR, ownership checking, numeric execution, LLVM output, or typed
artifact support is claimed. See the [resource/trust limitations](../../spec/typed-preview.md#resource-and-trust-bounds).
