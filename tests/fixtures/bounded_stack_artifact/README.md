# Saved bounded stack artifacts (OXS1)

The producer at `../../../fixtures/typed-expression-samples/artifact_main.ox`
parses the existing bounded expression language and lowers it to the existing
stack Code. Its `artifact_writer.ox` validates all used rows before emitting one
80-byte artifact. The independent loader root `artifact_load.ox` imports only
`artifact_reader.ox` and bounded stdin; it receives saved bytes and evaluates
them without expression text or producer modules. Both reuse the existing
compiler limits. The six existing producer dependencies are unchanged.

## Exact wire contract

Every file is exactly 80 bytes, without a newline or scalar trailer:

- Bytes 0 through 3: ASCII `OXS1`, hexadecimal `4f585331`
- Byte 4: count of used rows, from 1 through 15
- Fifteen rows, each five bytes, starting at byte 5
- A row contains one opcode byte and a four-byte unsigned little-endian operand
- Used opcode 1 pushes a nonnegative i32 operand; 2 adds and 3 multiplies
- Operators require operand zero and at least two existing stack values
- Every operand is at most 2147483647, including the most significant byte <= 127
- The complete used prefix must finish with exactly one stack value
- Every byte after the used rows is zero, including byte 79

Both consumers validate the complete file before any expression arithmetic.
Evaluation then uses checked i32 operations. A later malformed row, residual
stack, or bad unused byte therefore takes priority over an earlier arithmetic
overflow. Multiplication by zero does not suppress an earlier overflow.
The producer does not evaluate the expression before serialization, so a valid
program that overflows still produces a structurally valid artifact.

The original grammar, 128-byte expression limit, 15 arena nodes, and 15-entry
parser stacks are unchanged. Producer stdin uses 129 cells to witness excess
input; loader stdin uses 81 cells to witness any byte after the artifact.
An 82-byte artifact leaves its final byte unread. Zero is a real trailing byte.
No filesystem API, ISA instruction, optimizer, capacity increase, or durability
promise is introduced.

## Status and failure contract

The producer requires process entry. It returns 0 after a complete write, 64
for syntax or input/component capacity rejection, 65 for invalid arena/Code,
70 for an impossible encoded byte-domain failure, and 74 for I/O failure.
Decimal scanner overflow remains E0604 and happens before any bytes are emitted.

The loader uses ordinary default result entry. Valid values print as full i32
with a newline, including 2147483647. Length/trailing failures print `-4\n`,
stdin I/O failure prints `-5\n`, and invalid framing/Code prints `-6\n`, all with
process status 0. Arithmetic overflow reports E0604 from `artifact_reader.ox`,
prints no scalar, and has runtime status 1. The loader never imports stdout.

The standalone Python decoder accepts only a saved artifact path, reads at most
81 bytes, validates with `struct` little-endian decoding, and interprets with
its own checked stack. It imports neither Oxid code nor fixture expectations.
It exits 0 with JSON rows/value, 65 for malformed framing/Code, 1 for arithmetic
overflow, or 74 for file I/O error. Invalid byte positions and overflow rows are
part of the authored expectations in `artifact-cases.json`.

## Independent fixtures and execution

The 28 artifacts and nine expression inputs were hand-authored independently of
Oxid execution. `artifact-cases.json` contains their complete bytes, lengths,
hashes, rows, and expected outcomes. It is test authority, never decoder input.
The corpus covers 39/63, zero, the i32 maximum, mixed-byte little-endian order,
all 15 rows, empty/truncated/trailing files, bad magic/version/count/opcode,
operand high bit, nonzero operator operands, underflow/residual stack, unused
bytes, validation-before-arithmetic, and checked addition/multiplication overflow.

The two acceptance examples are:

- `12 + 3 * (4 + 5)`: seven rows, result 39, artifact SHA-256
  `7ec41c2ad48697ba7ec6cde11158b78ff571d588f9a0bbcda416879b5580e5a8`
- `7*(8+1)`: five rows, result 63, artifact SHA-256
  `7994c12777bb488a93f245aeee39c55d64b3ad6479c0667cbe5b6e2a70bee854`

Run the focused controller with an already built, input/output-enabled compiler:

```sh
python3 -B scripts/verify_bounded_stack_artifact.py --oxid target/release/oxid \
  --output /tmp/bounded-stack-artifact-evidence --native
```

The output directory must be new. The controller checks each root once and
compiles each application exactly once. It reuses the producer and loader ELFs
across all cases, checks their hashes, preserves the compiler/source hashes,
commands, byte inputs, stdout, stderr, statuses and unread input. The loader and
external decoder receive only saved byte input; native/decoder processes run
from an empty cwd with a cleared environment. This is not filesystem isolation.
Reference/native diagnostics must match exactly. Omit `--native` for explicitly
reference-only verification; that mode does not prove native compilation.

A producer's stdout is written directly into a new `stdout.partial` evidence
file. After it exits, the controller requires status 0, no stderr, exact authored
bytes, external decoding and loader results before atomic rename into `published`.
Malformed, partial, failed, or timed-out output is retained and never published.
Even a complete 80-byte output followed by failure is ineligible. Atomic rename
controls visibility; neither this component nor the controller promises fsync or
crash durability. Overflowing programs may be published only after both consumers
confirm their independently specified checked-overflow outcome.

Python controller tests exercise all decoder expectations, unread-byte controls,
successful publication, decoder/loader failure, trailing output, stderr, partial
failure, complete-output failure, existing destinations and timeout preservation:

```sh
python3 -B -m unittest discover -s scripts -p 'test_bounded_stack_artifact.py'
```

Those tests alone do not establish Oxid compilation or execution. Compiler
stdout/fuel/provenance/process-policy/SIGPIPE qualification and exact-head CI
remain separate gates.
