#!/usr/bin/env python3
"""Compile actual sibling consumers against the private owned witness boundary.

Positive siblings can inspect immutable witness/plan accessors. Negative siblings must fail
for private fields or mutability, not because a marker/type/import is missing.
A temporary source checkout and target directory leave the candidate untouched.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


PROBES = [
    ("compact-slot-checked-construction", True, (), """
        fn construct() {
            let slot = AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap();
            let _ = slot.aggregate();
        }
    """),
    ("compact-slot-private-representation", False, ("E0616",), """
        fn inspect(slot: AggregateSlot) { let _ = slot.0; }
    """),
    ("compact-slot-no-infallible-record-conversion", False, ("E0277",), """
        fn construct(record: RecordId) -> AggregateSlot { record.into() }
    """),
    ("array-validation-probe-absent-in-production", False, ("E0425",), """
        fn probe(raw: &RawOwnedProgram, sources: &SourceMap) {
            let _ = verified::probe_array_validation(raw, sources, budget::Limits::DEFAULT);
        }
    """),
    ("source-facade-immutable-methods", True, (), """
        fn inspect(w: &SourceProgram, sources: &SourceMap) {
            let _ = w.function_count();
            let _ = w.run(None, sources);
        }
    """),
    ("source-facade-cannot-be-forged", False, ("E0451",), """
        fn forge(witness: VerifiedOwnedProgram) -> SourceProgram {
            SourceProgram { witness }
        }
    """),
    ("source-facade-cannot-be-rebound", False, ("E0451",), """
        fn rebind(base: SourceProgram, replacement: VerifiedOwnedProgram) -> SourceProgram {
            SourceProgram { witness: replacement, ..base }
        }
    """),
    ("source-facade-private-witness", False, ("E0616",), """
        fn mutate(w: &mut SourceProgram, replacement: VerifiedOwnedProgram) {
            w.witness = replacement;
        }
    """),
    ("raw-program-cannot-enter-source-facade", False, ("E0308",), """
        fn run(raw: &RawOwnedProgram, sources: &SourceMap) {
            let _ = SourceProgram::run(raw, None, sources);
        }
    """),
    ("checked-source-cannot-rebind-entry", False, ("E0616",), """
        fn mutate(w: &mut crate::frontend::oir::CheckedSourceProgram) {
            w.entry = None;
        }
    """),
    ("checked-source-cannot-use-struct-update", False, ("E0451",), """
        fn rebind<'s>(base: crate::frontend::oir::CheckedSourceProgram<'s>)
            -> crate::frontend::oir::CheckedSourceProgram<'s> {
            crate::frontend::oir::CheckedSourceProgram { entry: None, ..base }
        }
    """),
    ("raw-program-cannot-enter-checked-source", False, ("E0308",), """
        fn run(raw: &RawOwnedProgram, sources: &SourceMap) {
            let _ = crate::frontend::oir::CheckedSourceProgram::run(raw);
        }
    """),
    ("denial-context-immutable-inspection", True, (), """
        fn inspect(context: &flow::DenialContext) {
            let facts = context.facts();
            let _ = (facts.operation, facts.role, facts.subject, facts.state,
                     facts.counterpart, facts.requested_borrow);
        }
    """),
    ("denial-context-cannot-be-forged", False, ("E0451",), """
        fn forge(facts: flow::DenialFacts) -> flow::DenialContext {
            flow::DenialContext { facts }
        }
    """),
    ("denial-context-cannot-mutate-facts", False, ("E0616",), """
        fn mutate(context: &mut flow::DenialContext, replacement: flow::DenialFacts) {
            context.facts = replacement;
        }
    """),
    ("denial-context-constructor-is-private", False, ("E0624",), """
        fn construct(raw: &RawOwnedFunction) {
            let _ = flow::DenialContext::owner(raw, OwnerPlaceId(0));
        }
    """),
    ("raw-program-cannot-enter-reference", False, ("E0308",), """
        fn run_raw(raw: &RawOwnedProgram) {
            let _ = execute::run(raw, Some(hir::DefId(0)));
        }
    """),
    ("raw-program-cannot-enter-native", False, ("E0308",), """
        fn compile_raw(raw: &RawOwnedProgram, sources: &SourceMap) {
            let _ = native::native_module(raw, Some(hir::DefId(0)), sources);
        }
    """),
    ("immutable-plan-access", True, (), """
        fn inspect_plan(w: &VerifiedOwnedProgram) {
            let plan = plan::ExecutionPlan::build(w).unwrap();
            let _ = (plan.witness(), plan.functions(), plan.metadata_bytes());
        }
    """),
    ("rebind-plan-using-struct-update", False, ("E0451",), """
        fn rebind<'a>(base: plan::ExecutionPlan<'a>, replacement: &'a VerifiedOwnedProgram)
            -> plan::ExecutionPlan<'a> {
            plan::ExecutionPlan { witness: replacement, ..base }
        }
    """),
    ("mutate-private-plan-witness", False, ("E0616",), """
        fn rebind<'a>(p: &mut plan::ExecutionPlan<'a>, replacement: &'a VerifiedOwnedProgram) {
            p.witness = replacement;
        }
    """),
    ("mutate-through-immutable-plan-accessor", False, ("E0596",), """
        fn mutate(p: &mut plan::ExecutionPlan<'_>) {
            p.functions().swap(0, 0);
        }
    """),
    ("immutable-access", True, (), """
        fn inspect(w: &VerifiedOwnedProgram) {
            let _ = (w.functions(), w.declarations(), w.usage());
        }
    """),
    ("replace-program-using-struct-update", False, ("E0451",), """
        fn forge(base: VerifiedOwnedProgram, replacement: RawOwnedProgram) -> VerifiedOwnedProgram {
            VerifiedOwnedProgram { program: replacement, ..base }
        }
    """),
    ("mutate-private-program-field", False, ("E0616",), """
        fn mutate(w: &mut VerifiedOwnedProgram, replacement: RawOwnedProgram) {
            w.program = replacement;
        }
    """),
    ("mutate-through-immutable-accessor", False, ("E0596", "E0594"), """
        fn mutate(w: &mut VerifiedOwnedProgram) {
            w.functions()[0].blocks.clear();
        }
    """),
]


TEST_PROBES = [
    ("array-reference-observation-is-unprivileged", True, (), """
        fn inspect(raw: RawOwnedProgram, sources: &SourceMap) {
            let observation = verified::probe_array_reference(raw, sources,
                budget::Limits::DEFAULT, Some(hir::DefId(0)), execute::Limits::default(),
                execute::ObservationControl::default()).unwrap();
            let _ = (observation.result, observation.events, observation.storage,
                     observation.remaining_fuel, observation.truncated);
        }
    """),
    ("array-reference-observation-cannot-be-witness", False, ("E0308",), """
        fn forge(observation: execute::ReferenceObservation) -> VerifiedOwnedProgram { observation }
    """),
    ("array-reference-observation-cannot-build-plan", False, ("E0308",), """
        fn forge(observation: &execute::ReferenceObservation) {
            let _ = plan::ExecutionPlan::build(observation);
        }
    """),
    ("array-reference-observation-has-no-executable-accessor", False, ("E0599",), """
        fn forge(observation: &execute::ReferenceObservation) { let _ = observation.witness(); }
    """),
    ("array-reference-observation-has-no-raw-body", False, ("E0609",), """
        fn forge(observation: &execute::ReferenceObservation) { let _ = &observation.program; }
    """),
    ("array-native-observation-is-unprivileged", True, (), """
        fn inspect(raw: RawOwnedProgram, verification_sources: &SourceMap,
                   rendering_sources: &SourceMap) {
            let native::NativeObservation { result, metrics } = verified::probe_array_native(
                raw, verification_sources, budget::Limits::DEFAULT, Some(hir::DefId(0)),
                rendering_sources, native::NativeControl::default()).unwrap();
            let _: Result<String, Box<Diagnostic>> = result;
            let _: (usize, usize, usize, usize, Option<&'static str>) = (
                metrics.occurrences, metrics.unique, metrics.count_bytes,
                metrics.render_bytes, metrics.failed_allocation);
        }
    """),
    ("array-native-observation-cannot-be-witness", False, ("E0308",), """
        fn forge(observation: native::NativeObservation) -> VerifiedOwnedProgram { observation }
    """),
    ("array-native-observation-cannot-build-plan", False, ("E0308",), """
        fn forge(observation: &native::NativeObservation) {
            let _ = plan::ExecutionPlan::build(observation);
        }
    """),
    ("array-native-observation-has-no-raw-body", False, ("E0609",), """
        fn forge(observation: &native::NativeObservation) { let _ = &observation.program; }
    """),
    ("array-native-observation-has-no-declarations", False, ("E0609",), """
        fn forge(observation: &native::NativeObservation) { let _ = &observation.declarations; }
    """),
    ("array-native-observation-has-no-executable-accessor", False, ("E0599",), """
        fn forge(observation: &native::NativeObservation) { let _ = observation.witness(); }
    """),
    ("array-native-observation-cannot-enter-native", False, ("E0308",), """
        fn compile(observation: &native::NativeObservation, sources: &SourceMap) {
            let _ = native::native_module(observation, Some(hir::DefId(0)), sources);
        }
    """),
    ("raw-program-cannot-enter-observed-native", False, ("E0308",), """
        fn compile(raw: &RawOwnedProgram, sources: &SourceMap) {
            let _ = native::run_array_observed(raw, Some(hir::DefId(0)), sources,
                native::NativeControl::default());
        }
    """),
    ("array-native-probe-cannot-accept-witness-callback", False, ("E0308",), """
        fn probe(raw: RawOwnedProgram, sources: &SourceMap) {
            let _ = verified::probe_array_native(raw, sources, budget::Limits::DEFAULT,
                Some(hir::DefId(0)), sources, |_: &VerifiedOwnedProgram| ());
        }
    """),
]
PROBES.append(("array-reference-probe-absent-in-production", False, ("E0425",), """
    fn probe(raw: RawOwnedProgram, sources: &SourceMap) {
        let _ = verified::probe_array_reference(raw, sources, budget::Limits::DEFAULT,
            Some(hir::DefId(0)), execute::Limits::default(), Default::default());
    }
"""))
PROBES.append(("array-native-probe-absent-in-production", False, ("E0425",), """
    fn probe(raw: RawOwnedProgram, sources: &SourceMap) {
        let _ = verified::probe_array_native(raw, sources, budget::Limits::DEFAULT,
            Some(hir::DefId(0)), sources, Default::default());
    }
"""))


# Exact compile-time data dependencies of array and enum source tests.
# These are copied inputs for cfg(test) compilation, not additional privacy probes.
ARRAY_COMPILE_TIME_FIXTURES = (
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-call-context-excluded/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-no-context/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-nonzero-annotation/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-reassignment-context-excluded/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-return-context-excluded/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-scalar-context/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/grouped-complete-access-and-index/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-first-heterogeneous-element/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-length-max-trailing-comma/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-nested-nonempty-is-nonscalar/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/nested-empty-does-not-inherit-context/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-array-bad-index/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-base-kind-before-index-kind/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-internal-type-first/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-literal-range-before-base-type/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-resolution-before-base-type/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-unknown-base-first/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-bool-length-0/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-bool-length-1/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-i32-length-0/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-i32-length-1/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-0/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-1/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/structural-identities-pairwise-distinct/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/utf8-read-primary/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/whole-program-resolution-before-earlier-function-type/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-base-kind-before-index-kind/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-both-subtree-errors-rhs-wins/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-element-before-mutability/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-index-internal-type-before-base/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-index-kind-before-rhs-element/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-one-conflict-element/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-one-conflict-index/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-owner-mutability-last/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-resolve-base-before-both-operands/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-resolve-rhs-before-index/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-rhs-internal-type-first/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-whole-resolution-before-rhs-typing/main.ox',
    'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-array-free/main.ox',
    'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-empty/main.ox',
    'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-record-only/main.ox',
    'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/reference-access-modes/main.ox',
)


ENUM_COMPILE_TIME_FIXTURES = (
    'tests/fixtures/bounded_enum_scanner/main.ox',
    'tests/fixtures/bounded_enum_scanner/scanner.ox',
)
COMPILE_TIME_FIXTURES = (*ARRAY_COMPILE_TIME_FIXTURES, *ENUM_COMPILE_TIME_FIXTURES)


def materialize_checkout(root, checkout):
    checkout.mkdir()
    for name in ["Cargo.toml", "Cargo.lock", "build.rs"]:
        shutil.copy2(root / name, checkout / name)
    for name in ["src", "native", "compiler", "stdlib", "rfcs", "fixtures"]:
        shutil.copytree(root / name, checkout / name)
    for name in COMPILE_TIME_FIXTURES:
        destination = checkout / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / name, destination)


def main():
    root = Path(__file__).resolve().parent.parent
    with tempfile.TemporaryDirectory(prefix="oxid-owned-privacy-") as directory:
        checkout = Path(directory) / "checkout"
        materialize_checkout(root, checkout)
        module = checkout / "src/frontend/oir/owned/mod.rs"
        original = module.read_text()
        env = dict(os.environ, CARGO_TARGET_DIR=str(Path(directory) / "target"))
        reports = []
        for (name, succeeds, codes, body), test in [(probe, False) for probe in PROBES] + [(probe, True) for probe in TEST_PROBES]:
            module.write_text(original + ("\n#[cfg(test)]" if test else "") + "\nmod consumer_privacy_probe {\n"
                              "use super::*;\n"
                              "use super::verified::VerifiedOwnedProgram;\n"
                              + body + "\n}\n")
            command = ["cargo", "check", "--locked", "--bin", "oxid", "--message-format=json"]
            if test:
                command.append("--tests")
            result = subprocess.run(command, cwd=checkout, env=env,
                                    text=True, capture_output=True, timeout=180)
            errors = []
            for line in result.stdout.splitlines():
                item = json.loads(line)
                if item.get("reason") == "compiler-message":
                    message = item["message"]
                    if message["level"] == "error":
                        errors.append(message)
            actual_codes = [e["code"]["code"] for e in errors if e.get("code")]
            if succeeds:
                assert result.returncode == 0, (name, result.stdout, result.stderr)
            else:
                assert result.returncode != 0 and any(code in actual_codes for code in codes), (
                    name, result.returncode, actual_codes, result.stdout, result.stderr)
                assert not any(code in actual_codes for code in ["E0412", "E0422", "E0432", "E0433"]), (
                    "probe failed on a missing import/type instead of witness privacy", name, actual_codes)
            reports.append({"probe": name, "expected_success": succeeds,
                            "exit_code": result.returncode, "error_codes": actual_codes})
        print(json.dumps({"owned_witness_privacy": "passed", "probes": reports}, indent=2))


if __name__ == "__main__":
    main()
