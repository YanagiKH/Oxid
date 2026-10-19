# oxutf8: strict UTF-8 copy Process CLI

Experimental ordinary-source workspace for R2. Both members take a direct path
dependency on the pure Text package `packages/utf8_text` (alias `text`), including
`into_bytes(Text) -> ByteBuf` for the app and
`append_to_bytes(&Text, &mut ByteBuf) -> GrowStatus` for the contracts. The
package does no I/O; only the `oxutf8` app member reads and writes process streams.

```text
oxutf8/
  oxid.toml                 [workspace] members = ["app", "contracts"]
  app/oxid.toml             project "oxutf8", entry src/main.ox,
                            text = "../../../../packages/utf8_text"
  app/src/main.ox           Process-mode CLI; the only member that does I/O
  contracts/oxid.toml       project "oxutf8-contracts", entry src/main.ox,
                            text = "../../../../packages/utf8_text"
  contracts/src/main.ox     pure TEST/Result cases for the Text interface
```

## Text interface consumed

From the root of `text`: `Text`, `FromBytes` (`Ok(Text)` or
`Invalid(ByteBuf)`), `from_bytes(ByteBuf) -> FromBytes`, `Validation`,
`validate(&[u8]) -> Validation`, `equals_utf8(&Text, &[u8]) -> bool`,
`into_bytes(Text) -> ByteBuf`, and
`append_to_bytes(&Text, &mut ByteBuf) -> GrowStatus`. The app consumes a valid
Text with `into_bytes`, transferring its existing allocation without copying
its payload or requesting growth. The unchanged pure contracts use
`append_to_bytes`: it appends exactly the Text's bytes to the destination,
keeps the Text, and on a returned growth failure leaves the destination
unchanged. `GrowStatus` is the canonical `alloc::bytes::GrowStatus`. This
workspace defines no Text copy and extends no package API. The app requires
the Text package's consuming export and its `alloc::bytes::take` dependency.

## Application behavior

```sh
oxid run --manifest-path examples/projects/oxutf8/oxid.toml --package oxutf8 --engine interpreter --entry-mode process < input.txt > output.txt
```

The app reads stdin to EOF with `read_some` into an initialized 1,024-byte
chunk, appending each chunk to one growable `ByteBuf` (the verified oxrev read
pattern; the read-success enum `Loaded::Ok(ByteBuf)` carries the buffer
directly, so no record field is moved). `from_bytes` then validates the
complete input as strict UTF-8. A valid Text is consumed with `into_bytes`,
recovering its existing `ByteBuf`, which the unchanged `write_some` loop
writes to stdout, handling partial progress. Linux EINTR (4) is retried on
both read and write; a write reporting zero bytes is rejected.

| Outcome | Exit | Stdout |
| --- | --- | --- |
| Valid UTF-8 (including empty input) | 0 | exactly the input bytes |
| Invalid UTF-8 anywhere in the input | 65 | empty |
| Capacity overflow (read buffer initialization or growth) | 70 | empty |
| Allocation failure or memory-limit policy refusal during read buffer initialization or growth | 71 | empty |
| Read/write error or zero-progress write | 74 | possibly a prefix |

Validation happens before any output, so invalid input never produces a
prefix. A write failure can occur after part of the output was written; no
rollback is promised. This is a complete-buffer program, not streaming: the
whole input is held. Consuming export eliminates the second payload buffer
and its growth request; it therefore removes only the export-copy paths to
70 and 71. Read buffer initialization and accumulation still allocate and
can return those codes. The 1,024-byte read chunk, per-allocation headers,
capacity slack, and growth's old-plus-new peaks still matter while reading.
Runtime limits and ordinary owner-return checks still apply; no input size
is promised to fit existing fuel or memory policy. This is not a claim of
allocation-free execution or newly qualified failure delivery. Exit codes
70, 71 and 74 are documented but not forced by any test.

## Pure contracts

`oxutf8-contracts` performs no I/O, so its cases run under TEST and Result:

```sh
oxid test --manifest-path examples/projects/oxutf8/oxid.toml --package oxutf8-contracts
oxid run --manifest-path examples/projects/oxutf8/oxid.toml --package oxutf8-contracts --engine interpreter --entry-mode result
oxid test --manifest-path examples/projects/oxutf8/oxid.toml --package oxutf8-contracts --engine native
```

Expected: five PASS lines
(`test_append_keeps_text_and_destination_prefix`, `test_append_twice_is_exact`,
`test_empty_text_appends_nothing`, `test_valid_classes_round_trip`,
`test_invalid_classes_return_original_owner`), `test result: 5 passed; 0
failed; 5 total`, and Result output `42`. The cases check exact append after
an existing destination prefix, repeated append with the Text kept, empty Text,
valid class round-trips, and invalid classes returning the original owner. Both
ordinary interpreter TEST and native TEST must pass. Native TEST requires
LLVM 19.1.7 via `OXID_LLVM_BIN`.

## Fixed corpus

| Name | Class | Exit | Stdout |
| --- | --- | --- | --- |
| valid-empty | empty | 0 | identical |
| valid-ascii | ASCII | 0 | identical |
| valid-nul | embedded NUL | 0 | identical |
| valid-multibyte | 2/3/4-byte edges | 0 | identical |
| valid-combining | combining marks | 0 | identical |
| valid-bom | UTF-8 BOM prefix | 0 | identical |
| valid-cross-1024 | length 1027 crossing the 1,024 chunk | 0 | identical |
| valid-8199 | length 8199 mixed scalars | 0 | identical |
| invalid-overlong | overlong encoding | 65 | empty |
| invalid-surrogate | surrogate code unit | 65 | empty |
| invalid-above-10ffff | above U+10FFFF | 65 | empty |
| invalid-lone-continuation | lone continuation | 65 | empty |
| invalid-truncated | truncated sequence | 65 | empty |
| invalid-late-truncated-after-1024 | truncated after 1,500 ASCII | 65 | empty |

Acceptance: `tests/owned_utf8_package.rs` drives `scripts/accept_oxutf8.py`,
which copies sources to a fresh temp tree (preserving relative package paths),
runs CHECK for both members, contracts TEST/Result, and the Process corpus.
Native (ignored unless `OXID_TEST_LLVM_BIN` is set) builds, packages, and runs
the deployed `bin/oxutf8` through `scripts/cli_driver.py` with segmented stdin
and close-EOF, cwd=`/`, empty env. Lengths and SHA-256 pins match
the fixed `corpus_pins` table in the acceptance script.

## Focused acceptance status

On Linux x86_64 with LLVM 19.1.7, this consuming-export revision passed CHECK
for the app (29 functions) and pure contracts (34 functions), and the app's
format check passed. The ordinary `owned_utf8_package` test run passed its
interpreter acceptance wrapper (one passed, one native test ignored). Running
that target with ignored native coverage enabled passed both wrappers. These
wrappers exercise the unchanged acceptance script, including contracts
TEST/Result and the fixed fourteen-input Process corpus.

The native wrapper ran successfully first, but removes its temporary evidence
on success. The native acceptance script was then invoked directly a second
time with a retained evidence directory. That separate run also passed all
fourteen fixed inputs: valid inputs returned 0 with exact output, and invalid
inputs returned 65 with empty output. The retained records were independently checked against
the fixed corpus for input/output lengths and SHA-256 hashes, status, empty
stderr, complete and closed stdin, stream EOF, clean process cleanup, cwd=`/`,
empty environment, `record_ok`, and an empty rejection list. The retained native
executable hash also matches its receipt.

The Take/Text dependency passed its focused source/reference and native-IR,
catalog, ordinary Text and standalone-native Text regressions, plus scoped
Clippy. These results do not establish a full repository CI pass or completed
R2. Allocation-failure and stale-descriptor injection, failure-transport
qualification, forced short-write or EINTR coverage, unrestricted input
capacity, and other outstanding qualifications remain unverified. Exit codes
70, 71 and 74 were not forced by these tests.
