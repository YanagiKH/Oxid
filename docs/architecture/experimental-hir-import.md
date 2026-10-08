# Experimental checked-HIR artifact import

Explicit typed-preview `check`, `run` and `compile` accept
`--experimental-hir-import <observation-file>`. This is an opt-in bridge from the
existing bounded Oxid frontend component. It does not switch default compiler
providers or claim self-hosting. The parser/static producer is not launched by the
compiler. Without the option, every existing route is unchanged.

```sh
oxid check program.ox --edition typed-preview --experimental-hir-import program.hir
oxid run program.ox --edition typed-preview --experimental-hir-import program.hir
oxid compile program.ox --edition typed-preview --experimental-hir-import program.hir \
  --backend llvm --output program
```

Existing text/JSON summaries, source diagnostics, runtime failures, native-tool
requirements and no-replace executable publication apply. Result entry mode is
required; Process mode, format commands and legacy edition reject the option.
Native compilation remains Linux x86_64 with LLVM/Clang/LLD 19.1.7 at O0.

## Accepted boundary

The source is one genuine root-only `ProjectSources` input in the existing scalar
grammar, at most 128 ASCII bytes. Public functions are supported. No multi-file
imports, aggregates, broader grammar or language-limit increase is introduced.
The observation must be the exact 2,607-byte successful OPA1+STF1 form, read from a
regular file with a fixed 2,608-byte buffer (one extra byte detects trailing data).
Directories, devices, partial outputs, pending/diagnostic forms, larger files and
malformed/corrupt/stale observations cannot trigger fallback or publication.
Artifact file handling uses the existing stable-filesystem trust model; it is not
a TOCTOU/filesystem sandbox or an I/O wall-clock guarantee.

Loading and parsing source happens first and retains ordinary source diagnostics.
The artifact is untrusted. The bridge compares complete source/AST correspondence,
reconstructs candidate-owned resolved HIR, compares canonical resolution, runs the
genuine checker, compares every supplied type/flow fact, lowers, associates source
and independently verifies OIR. A producer's first-diagnostic record is not a
replacement for the compiler's full diagnostic list and is not importable success.
The separate [live producer route](experimental-hir-producers.md#source-validated-static-diagnostics)
can validate that first diagnostic against genuine source and return the complete
canonical error vector; this does not admit diagnostic artifact files.
Authentic compiler/runtime/native diagnostics retain their normal fields; new
transport/domain/budget/mismatch refusals use E0702 at stage `hir-import`.

The opt-in wrapper debits its named carrier/buffer storage and 4,096 fixed setup
work units before entering the already bounded leaf. It uses unchanged ceilings,
so a boundary source may be refused by import while ordinary compilation succeeds.
This is affected-importer accounting, not total process/allocator/stack memory.
Existing source/AST and argv-string ownership remain the ordinary loader/CLI
baseline. The bridge-specific option box has one fallible exact reservation;
its actual payload, transport carriers and fixed artifact buffer are accounted.
The default Route carrier remains 56 bytes on the qualified 64-bit host. External
LLVM, file I/O, diagnostic rendering and executable publication retain their
existing contracts outside the compiler's private work tariff.

## Produce a real observation without editing transport

The following uses the already implemented Oxid parser and AST static consumer.
Run from the repository root after building the compiler. Both output executables
and artifacts are new files under a fresh directory. The sample's original bytes
are preserved; it returns the Result value `1`.

```sh
cargo build --locked
export OXID_LLVM_BIN=/usr/lib/llvm-19/bin
oxid="$PWD/target/debug/oxid"
work="$(mktemp -d)"

"$oxid" compile fixtures/typed-lexer-samples/parser_main.ox \
  --edition typed-preview --backend llvm --entry-mode process \
  --output "$work/parser"
"$oxid" compile fixtures/typed-lexer-samples/ast_static_main.ox \
  --edition typed-preview --backend llvm --entry-mode process \
  --output "$work/static"
cp tests/fixtures/checked_hir_import/rich-source.txt "$work/program.ox"

python3 - "$work" <<'PY'
from pathlib import Path
import subprocess
import sys
work = Path(sys.argv[1])
source = (work / "program.ox").read_bytes()
assert len(source) <= 128 and source.isascii()
parser = subprocess.run([str(work / "parser")], input=source,
                        capture_output=True, check=True, timeout=30)
opa = parser.stdout
assert not parser.stderr
assert len(opa) == 1559 and opa[:8] == b"OPA1\0\0\0\0"
assert opa[10] == len(source)
# This host only frames original bytes. It derives no AST, name, type or flow fact.
ast1 = b"AST1" + bytes([len(source)]) + source + opa
static = subprocess.run([str(work / "static")], input=ast1,
                        capture_output=True, check=True, timeout=30)
artifact = static.stdout
assert not static.stderr
assert len(artifact) == 2607 and artifact[:1559] == opa
assert artifact[1559:1564] == b"STF1\0"
with (work / "program.hir").open("xb") as output:
    output.write(artifact)
PY

cmp "$work/program.hir" tests/fixtures/checked_hir_import/rich-success.bin
"$oxid" check "$work/program.ox" --edition typed-preview \
  --experimental-hir-import "$work/program.hir"
"$oxid" run "$work/program.ox" --edition typed-preview \
  --experimental-hir-import "$work/program.hir"
"$oxid" compile "$work/program.ox" --edition typed-preview \
  --experimental-hir-import "$work/program.hir" --backend llvm \
  --output "$work/program"
"$work/program"
```

The final artifact is OPA1+STF1, not the intermediate AST1 input. Exit status 0
alone does not mean producer success: parsers can emit diagnostics and the static
consumer can emit a 1,575-byte diagnostic record. Do not trim source, normalize
line endings or pass binary transport through shell command substitution. Use
`ast_static_main.ox`, not the permanent synthetic `static_main.ox` probe or the
inadmissible combined `typed_main.ox`. The current example makes no claim that
an arbitrary producer binary is trusted; the compiler still validates its output.

Current-source seals, independent review and exact-head hosted results are
separate qualification evidence. Older private/native receipts retain their
original identities and do not by themselves certify this public CLI route.
