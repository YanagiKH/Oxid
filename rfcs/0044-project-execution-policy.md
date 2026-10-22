# Project execution policy

Status: experimental. Typed projects can choose how execution work is limited.
Reference execution supports both policies; native compilation remains limited
to Linux x86_64 with LLVM/Clang/LLD 19.1.7.

## Project setting

```toml
[execution]
policy = "hosted"
```

`hosted` lets a program continue until it finishes or encounters an ordinary
runtime error. It has no cumulative instruction or byte-work budget. Use it for
input processing and other programs whose running time depends on their input.

`bounded` retains the existing execution budget and exact charging order. It is
the default when the setting is absent. Explicit source-file commands retain
their existing bounded behavior.

The setting applies to `run`, native `build`, and the project's declared tests.
Each workspace binary uses its own manifest. Dependency settings do not change
the importing binary's policy. `dev` and `release` select optimization separately;
the build-profile alias `bounded` does not select an execution policy.

The policy must be a quoted `bounded` or `hosted` name in an ordinary
`[execution]` table. Duplicate, unknown, malformed, nested, inline and dotted
execution settings fail before source execution or native tools. A failed build
preserves the previous executable.

## Checks and resources

Both policies use the same typed source, ownership verification and runtime
value checks. Checked arithmetic, array bounds, initialized views, fallible
allocation, live heap limits and resource cleanup retain their contracts.

Hosted execution removes cumulative fuel debits and the native static fuel
upper-bound check. Source-size, compiler memory, layout, call-depth and live
storage limits remain. This change does not add recursive native calls,
cancellation or additional native targets.

Binary I/O still validates the entire input range and reserves direct status
delivery before transport. A partial read/write, EOF or OS error is returned to
the caller through the existing status type. Hosted execution cannot authorize
I/O from a result-mode entry or bypass descriptor checks.

## Native builds and caching

The emitted module records the chosen policy. Native object reuse follows the
generated module and tool settings; changing policy cannot reuse code with
different execution semantics. Runtime compilation and linking still occur on
every build. Reproducible builds retain their existing source-path rules.

The scalar emitter omits runtime fuel guards for hosted programs. The owned
emitter preserves its control-flow continuation identities while omitting the
counter debit and exhaustion edge. Byte primitives retain their checked local
admission calculations without accumulating their debits across the program.

## Validation

- Long scalar loops and repeated buffer growth distinguish bounded exhaustion
  from hosted completion through ordinary project commands.
- Arithmetic, bounds, process-entry errors and live-heap refusal remain checked.
- Binary output exceeds the previous program budget without losing direct
  status delivery; native checks run at both optimization levels.
- Cache tests switch policy, verify warm reuse and execute the resulting binary.
- C component tests cover repeated growth, transport, partial progress, EOF,
  interruptions, invalid descriptors and delivery refusal at O0/O2, including
  ASan/UBSan runs.
- Retained scalar/record compatibility remains a separate bounded assertion
  suite. Its results do not qualify the new policy's execution semantics.
