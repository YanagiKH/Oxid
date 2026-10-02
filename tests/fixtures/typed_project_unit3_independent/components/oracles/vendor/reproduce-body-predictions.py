#!/usr/bin/env python3
"""Planning body-model calculation only. Does not build or invoke Oxid/LLVM.

Usage: python3 -B reproduce-body-predictions.py /path/to/frozen/source/scripts
Prints a receipt; the caller chooses any receipt output path.
This is not a project resolver and never emits a substitute compiler input.
"""
import hashlib
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
model_dir = Path(sys.argv[1]).resolve()
sys.path.insert(0, str(model_dir))
import owned_source_model as m

old = m.batch_program()
old_by_name = {f.name: f for f in old.functions}
b = m.Builder()
b.serial = 10000  # Disjoint labels from the retained predecessor body nodes.
make = m.Function("make", (), "Batch", b.block(b.ret(b.lit("Batch",
    ("completed", b.i(0)), ("retries", b.i(0)),
    ("checksum", b.i(0)), ("active", b.b(True))))))
next_job = m.Function("next_job", (("state", "&Batch"),), "i32",
    b.block(b.ret(b.op(b.f("state", "completed"), "+", b.i(1)))))
is_active = m.Function("is_active", (("state", "&Batch"),), "bool",
    b.block(b.ret(b.f("state", "active"))))
stop = m.Function("stop", (("state", "&mut Batch"),), "()",
    b.block(b.write("state", "active", b.b(False)), b.ret()))
main = m.Function("main", (), "i32", b.block(
    b.let("state", b.call("make"), True),
    b.loop(b.call("is_active", b.borrow("state")), b.block(
        b.let("attempt", b.i(0), True),
        b.loop(b.op(b.v("attempt"), "<", b.i(3)), b.block(
            b.store("attempt", b.op(b.v("attempt"), "+", b.i(1))),
            b.iff(b.op(b.v("attempt"), "<", b.i(2)), b.block(
                b.discard(b.call("retry", b.borrow("state", True))), b.cont())),
            b.let("job", b.call("next_job", b.borrow("state"))),
            b.discard(b.call("dispatch", b.borrow("state", True), b.v("job"))), b.brk())),
        b.iff(b.call("done", b.borrow("state")), b.block(
            b.discard(b.call("stop", b.borrow("state", True))), b.brk())), b.cont())),
    b.let("completed", b.call("relay", b.v("state"))),
    b.ret(b.call("finish", b.v("completed")))))
opaque = m.Program(old.records, (
    main, make, old_by_name["retry"], old_by_name["commit"], old_by_name["done"],
    next_job, is_active, stop, old_by_name["relay"], old_by_name["finish"],
    old_by_name["dispatch"]))

# One separate entry-binding control, not an enlargement of the 24/48 families.
# child_main is a unique body-model label for source state::main, not a source
# rewrite or an assertion that the model implements project name resolution.
e = m.Builder()
entry_helper = m.Function("helper", (), "i32", e.block(e.ret(e.i(91))))
entry_main = m.Function("main", (), "i32", e.block(
    e.let("value", e.call("make", e.i(4)), True, "Counter"),
    e.discard(e.call("bump", e.borrow("value", True))),
    e.ret(e.call("finish", e.v("value")))))
entry_make = m.Function("make", (("value", "i32"),), "Counter",
    e.block(e.ret(e.lit("Counter", ("value", e.v("value"))))))
entry_bump = m.Function("bump", (("value", "&mut Counter"),), "()",
    e.block(e.write("value", "value", e.op(e.f("value", "value"), "+", e.i(1))), e.ret()))
entry_finish = m.Function("finish", (("value", "Counter"),), "i32",
    e.block(e.ret(e.f("value", "value"))))
entry_child_main = m.Function("child_main", (("value", "Counter"),), "Counter",
    e.block(e.ret(e.v("value"))))
owned_entry = m.Program((m.Record("Counter", (("value", "i32"),)),), (
    entry_helper, entry_main, entry_make, entry_bump, entry_finish, entry_child_main))

def calculate(identifier, program):
    checked = m.NamesTypes(program).check()
    dynamic = m.Machine(checked).run()
    schedule = m.TemplateScheduler(checked).schedule(dynamic)
    if dynamic["failure"] is not None:
        raise AssertionError(dynamic["failure"])
    return {
        "id": identifier,
        "symbolic_functions": [f.name for f in program.functions],
        "result": dynamic["result"],
        "result_type": dynamic["result_type"],
        "total_fuel": schedule["total_fuel"],
        "frame_counts": {name: {key: value for key, value in counts.items()
            if isinstance(value, int)} for name, counts in schedule["frames"].items()},
        "dynamic_events": dynamic["events"],
        "schedule": schedule["items"],
        "exact_budget": m.TemplateScheduler.at_budget(schedule, dynamic, schedule["total_fuel"]),
        "one_below": m.TemplateScheduler.at_budget(schedule, dynamic, schedule["total_fuel"] - 1),
    }

receipt = {
    "schema": "oxid-unit3-planning-body-predictions-v1",
    "evidence_kind": "MODEL_ONLY_BODY_SEMANTICS_NO_MODULE_RESOLUTION",
    "actual_cli_invocations": 0,
    "actual_native_executions": 0,
    "candidate_v4_manifest_sha256": "9aaadaf0567378a862ddfdbcf163045ba6fb696af88e0eac624b63f3a77abdfb",
    "model_file_sha256": hashlib.sha256((model_dir / "owned_source_model.py").read_bytes()).hexdigest(),
    "calculation_file_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    "scope": "Published independent ownership/scalar template semantics over explicitly authored bodies. Names are symbolic unique body labels. File/module resolution, visibility, byte-origin relocation, compiler and native behavior are not qualified.",
    "cases": [calculate("original-batch-body", old), calculate("opaque-batch-body", opaque),
              calculate("owned-entry-id1-body", owned_entry)],
}
receipt["cases"][-1]["planned_project_mapping"] = {
    "files": ["main.ox", "state.ox"],
    "functions_by_global_id": ["root::helper", "root::main", "state::make", "state::bump", "state::finish", "state::main"],
    "model_child_main_means": "state::main",
    "source_alias_increment_means": "state::bump",
    "root_original_main": 1,
    "record_0": "state::Counter",
    "field_0_0": "state::Counter.value",
    "qualification": "Authored symbolic mapping only; production project selection is not observed by this calculation.",
}
assert [(x["result"], x["total_fuel"]) for x in receipt["cases"]] == [(816, 1297), (816, 1577), (5, 126)]
print(json.dumps(receipt, indent=2))
