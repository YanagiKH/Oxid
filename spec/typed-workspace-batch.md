# Explicit typed workspace CHECK and TEST

This partial R1/R6 workflow adds sequential operations over declared workspace
members. It does not complete either roadmap milestone or qualify source/native
resource policies.

```sh
oxid check --manifest-path workspace/oxid.toml --workspace
oxid test --manifest-path workspace/oxid.toml --workspace
oxid test --manifest-path workspace/oxid.toml --workspace --fail-fast
```

`--workspace` is a valueless command-local flag for explicit manifest CHECK and
TEST. It occurs at most once before `--`, requires `--manifest-path` after the
command, and conflicts with `--package`. Only a virtual workspace is accepted,
even if it has one member. A direct project manifest must use the ordinary
single-package command instead. The existing `new --workspace` command remains
independent and unchanged.

The batch is text-only. TEST defaults to the interpreter and accepts explicit
`--engine=interpreter`; CHECK accepts no engine option. Native execution, JSON,
profiles, test filters, entry-mode and experimental selectors are rejected before
manifest I/O. Use an ordinary selected `--package` command for those existing
features. Source operands are not accepted. Global-option error priority, script
argument ownership and the `--` separator retain their existing behavior.
`--fail-fast` is supported by both batch commands.

## Membership and selected sessions

The initial catalog uses the existing virtual-workspace parser, canonical-member
checks, unique project names, manifest snapshots and allocation/budget checks.
All listed member manifests must be valid before any member operation begins.
This does not check every dependency graph, source body or test suite up front.
No alternate parser, implicit member discovery or member skipping is used.

Members run in declared order. For each name, the command creates an ordinary
selected-workspace request using the original lexical manifest path and
`--package` semantics. CHECK reuses the normal selected-package session,
diagnostics and check summary. TEST reuses the existing interpreter test runner:
the selected root supplies test declarations; dependency/child-module tests
are not discovered; all bodies and discovered signatures are checked before
that member's cases run. Missing suites and invalid signatures remain failures,
including for a member described informally as a library.

Each member session revalidates the workspace and checks its dependency closure.
A later member's preparation can therefore fail after earlier tests ran.
The filesystem is assumed stable during the invocation; neither the catalog nor
the sequence is an atomic snapshot or an all-workspace transaction.

## Output and failure behavior

Before each attempted member, stdout receives and flushes:

```text
workspace package: NAME
```

The existing member operation then emits its normal diagnostics and output.
By default, failed members do not prevent subsequent members from running.
`--fail-fast` stops after the first failed member and, for TEST, also enables
the existing within-suite fail-fast behavior.

After the attempted operations, stdout receives and flushes:

```text
workspace result: P passed; F failed; U unrun; T total packages
```

These are package counts, not combined test-case counts. `P + F + U == T`.
Exit status is 0 only if every member passed and required output succeeded;
otherwise it is 1. A batch heading or summary write/flush failure stops further
work and can prevent the final summary. Initial catalog failures report their
ordinary manifest diagnostic classification without a package heading or summary.

## Limits and exclusions

The initial manifest catalog retains the existing 16,384-byte aggregate manifest
allowance, member/edge envelope, path allowance and member-count limit. It borrows
validated names without copying a second package/name catalog. At most one
ordinary selected-package session is live alongside this bounded catalog.

Each selected session independently uses the unchanged existing manifest,
source, module, token, node, index and interpreter limits for its entire
dependency closure. Those limits are not raised or pooled across members.
Total time and work can grow with the number of declared members, and shared
dependencies can be checked repeatedly. This feature does not claim a single
workspace-wide resource/fuel budget, a memory bound equal to one session alone,
parallel execution or performance improvement.

There are no native compiles, managed artifacts, receipt/cache changes, registry
operations, dependency edits or lockfiles. Runtime behavior remains that of the
existing interpreter test mode. This feature does not add Process-mode effects.
