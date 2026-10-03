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
]
PROBES.append(("array-reference-probe-absent-in-production", False, ("E0425",), """
    fn probe(raw: RawOwnedProgram, sources: &SourceMap) {
        let _ = verified::probe_array_reference(raw, sources, budget::Limits::DEFAULT,
            Some(hir::DefId(0)), execute::Limits::default(), Default::default());
    }
"""))


def main():
    root = Path(__file__).resolve().parent.parent
    with tempfile.TemporaryDirectory(prefix="oxid-owned-privacy-") as directory:
        checkout = Path(directory) / "checkout"
        checkout.mkdir()
        for name in ["Cargo.toml", "Cargo.lock", "build.rs"]:
            shutil.copy2(root / name, checkout / name)
        for name in ["src", "native", "compiler", "stdlib", "rfcs", "fixtures"]:
            shutil.copytree(root / name, checkout / name)
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
