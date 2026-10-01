# Bounded native-oracle dispatch

The native LLVM CI job runs the same seven Python oracle scripts, with unchanged
arguments, corpora, models, diagnostic checks, per-command timeouts, resource
limits, source-absent execution, and debug/release artifact comparisons. The
compiler and oracle implementations are unchanged. The existing eight explicit
raw-LLVM invocations (four tests, debug and release) still finish before dispatch.

## Invocation and isolation

```sh
python3 scripts/verify_native_suite.py target/debug/oxid target/release/oxid --jobs 2
```

`--jobs` accepts only 1 or 2 and defaults to 1. The longest scripts (comparison,
arithmetic, then Boolean logic) are admitted first, followed by preview, mutable
locals, while loops and loop control. Log groups always appear in that declared
order, independent of completion order. `--timeout` is a positive finite
whole-script backstop in seconds (default 1800); no existing inner timeout or
semantic resource limit changes.

Each oracle runs as a separate Python subprocess and POSIX session/process group.
The dispatcher does not import oracle modules or execute them in threads: the
oracles' narrow-stack native executions use `preexec_fn`. The original scripts
use separate `TemporaryDirectory` roots. Their transitive imports define models
or helpers without running another suite; generated sources, binaries, C
harnesses and caches stay under each script's scratch root. Compiler binaries,
LLVM tools and `native/typed_preview.c` are read-only shared inputs. Environment
and working directory inheritance match the old shell invocations.

The dispatcher resolves both compiler paths and checks their SHA256 before and
after the aggregate. Python assertions must be enabled. All seven scripts must
exist before work starts. Missing tools fail inside the unchanged real oracle;
there is no availability-based skip.

## Failure and log contract

- Never run more than two active oracle processes
- Capture combined stdout/stderr directly in separate files to avoid full pipes
  and unbounded in-memory output; replay each file in bounded chunks
- Require both exit zero and the expected final, newline-terminated PASS summary
- On the first observed failure, stop admitting work; report unstarted scripts
  as `NOT RUN` and terminated siblings as `CANCELLED`, never as passes
- Preserve the observed child exit code; map a child signal to `128 + signal`
- Return 124 for a whole-worker timeout and 1 for launch/summary/hash failures
- On failure, timeout, SIGINT or SIGTERM, terminate remaining launched process groups,
  allow one second, then kill remaining group members and reap direct children
- A worker that exits zero while leaving running descendants is a failure
- SIGINT/SIGTERM return their conventional 130/143 codes; even repeated signals
  do not bypass group cleanup

The runner does not change any corpus, profile, expected output, or native gate.
On a failing run, complete validation still requires rerunning the full suite.
Successful aggregate output explicitly accounts for seven of seven scripts.

## Automated dispatcher tests

`python3 -m unittest discover -s scripts -p 'test_*.py' -v` includes real tiny
subprocess fixtures for all seven command/argument shapes, serial execution,
two-worker overlap and concurrency cap, completion-order-independent grouped
logs (including large stdout), nonzero exits, missing tools/workers/scripts,
missing final summaries, executable/compiler checks, post-run compiler mutation,
assertions disabled, child signals, timeout, SIGINT/SIGTERM, TERM-ignoring process
children, successful-worker orphan detection and unstarted-work accounting.

## Qualification (2026-10-01)

All three runs below used the same copied debug/release compiler binaries from
source tree `fdd77bc94431f705c1e6eef3599ea5df4bf1191b`, exactly matching all 293
tracked files of main `2ab82519aaa30932470937ac80db0aca3654212c`. The dispatcher
patch was then rebased onto main `473811e44afb63855599e04d513e447e8c959a58`
(tree `5c8b1638c06b513ce8a5807110298e84dfe48979`), preserving its ownership-witness
privacy gate. The benchmark is explicitly for the earlier pinned compiler tree;
it is not a new-compiler performance claim or a substitute for exact-head CI.

Compiler SHA256, unchanged before and after every run:

- Debug: `679818e2d9860e473aaaad04b6d9bad86ff761f37ce3378aca1e080c54623b77`
- Release: `651b17cbce1874af2ba5d372b68ce3565efec32b47baf16ec5dc36190c256500`

Environment: Debian 13 Linux x86_64, Python 3.12.14, LLVM/Clang/LLD 19.1.7,
GNU readelf 2.44, nine visible logical CPUs. The modes ran consecutively with no
concurrent heavy Rust build/native sweep. Each mode ran the unchanged complete
seven-script workload once; builds and preceding raw-LLVM gates were not timed.

| Mode | Wall seconds | User/system CPU seconds | Sampled peak aggregate RSS | Sampled peak scratch | Oracle workers |
| --- | ---: | ---: | ---: | ---: | ---: |
| Original direct serial commands | 922.05 | 478.95 / 448.19 | 154.6 MiB | 6.82 MiB | 1 |
| Dispatcher `--jobs 1` | 928.06 | 483.91 / 451.03 | 188.4 MiB | 6.66 MiB | 1 |
| Dispatcher `--jobs 2` | 509.45 | 501.31 / 462.69 | 307.1 MiB | 6.83 MiB | 2 |

The two-worker run used **44.75% less wall time** than the original direct serial
run in this sample (15m22s to 8m29s). This is one local sample per mode, not a
variance study, hosted-CI guarantee, compiler optimization, or runtime speedup.
Total user+system CPU increased from 927.13s to 963.99s. The throughput benefit
trades extra concurrent memory and CPU consumption for lower wall time.

An external observer sampled process trees and scratch sizes every 0.2 seconds;
RSS and disk values are sampled observations, not guaranteed absolute peaks.
The maximum sampled descendant count was 3/4/8 for direct/one/two workers;
maximum individual-child RSS was 119652/120000/120228 KiB. All three modes exited
zero, kept both compiler hashes unchanged and left no scratch files. Worker
counts use direct-child topology: narrow-stack `preexec_fn` forks temporarily
retain their parent's Python argv and must not be counted as additional suites.
The original argv-only direct-serial observation was corrected for this known
counting ambiguity; direct execution starts and waits for one script at a time.

### Complete oracle parity

Every oracle's complete emitted output was byte-for-byte identical across all
three modes, including every available count, category, corpus/artifact/model
hash, compiler hash and final PASS summary. The scripts and transitive model
helpers themselves remain byte-identical to both baseline commits.

| Oracle | Source cases | Primary compiled artifacts | Additional resource artifacts | Negative cases | Native resource cases | Reference-only cases |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Preview | 57 | 57 | not separately reported | not separately reported | not separately reported | not separately reported |
| Arithmetic | 1051 | 2102 | not separately reported | not separately reported | not separately reported | not separately reported |
| Comparisons | 1610 | 3220 | not separately reported | 198 | 14 | not separately reported |
| Boolean logic | 525 | 1050 | 14 | 149 | 18 | 7 |
| Mutable locals | 163 | 326 | 10 | 49 | 13 | 7 |
| While loops | 144 | 288 | 8 | 12 | 7 | not separately reported |
| Loop control | 186 | 372 | 8 | 35 | 7 | not separately reported |

All 3736 primary source cases remain, along with unchanged resource, negative,
reference-only, fuel, I/O-failure, adapter, narrow-stack and no-clobber checks. “Not separately reported” is not a claim that a check is absent.

### Additional verification and limits

- All 21 repository Python tests passed after rebase, including 14 dispatcher tests
- Nine independent heldout orchestration tests passed: mixed exits before new
  admission, interruption inside Popen registration, failing launch with active
  sibling, repeated signals and depth-two TERM-ignoring descendants, malformed
  final summaries, binary/large stderr, exact path/argument/environment/cwd/hash
  preservation, duplicate names, and handler restoration on output failure
- A real missing-LLVM aggregate failed with exit 1 and explicit 0/7 accounting
- Repository verification passed on the frozen binary: 121 sources, 67 runnable programs
- Existing oracle bytes, all eight raw-LLVM commands and the new ownership privacy
  gate were checked unchanged in the rebased workflow
- No production compiler, corpus, model, timeout, resource limit or expected-output
  implementation changed; no compiler rebuild was needed for this harness patch
- The raw-LLVM invocations were preserved, not newly executed by this qualification;
  the final published head must still pass the complete configured CI
- Process groups cover the unchanged trusted oracle/tool chain. They are not a
  sandbox for programs that deliberately escape into a new session
