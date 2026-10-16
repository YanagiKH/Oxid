"""Explicit outer alias/Unicode capture, retaining the complete formatter stage."""
import copy
import json
from pathlib import Path
import sys
import types

import generation as g
import predecessor
import published_current as previous_current
import current as retained_current
import adapters

same = previous_current.same
_verify = previous_current._verify
_regular = previous_current._regular
_extend_receipt = previous_current._extend_receipt
RETAINED_ACCOUNTING_KEY = previous_current.RETAINED_ACCOUNTING_KEY
LEXER_ACCOUNTING_KEY = previous_current.LEXER_ACCOUNTING_KEY
PUBLISHED_ACCOUNTING_KEY = previous_current.CURRENT_ACCOUNTING_KEY
CURRENT_ACCOUNTING_KEY = "package_json_unit2_accounting_source_binding"

PACKAGE_PATH = g.PACKAGE.rstrip('/')
KEY = 'package_json_unit2_accounting_source_binding'


def admit(repo, seal):
    repo = Path(repo)
    previous = g.regular(repo, g.PACKAGE + g.PREDECESSOR_NAME)
    rows = json.loads(previous)['files']
    inputs = {r['path']: g.regular(repo, r['path']) for r in rows}
    retained_current.source_transition.complete_compiler_members(repo, inputs)
    runner = g.regular(repo, 'tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py')
    evidence = {n: g.regular(repo, g.ROOT + 'source-proof/' + n + '.json') for n in g.READBACKS}
    patch = g.regular(repo, g.ROOT + 'generated/' + g.PATCH_NAME)
    artifacts, restored = g.generate(previous, inputs, patch, evidence, runner)
    for name, raw in artifacts.items():
        actual = g.regular(repo, g.ROOT + 'generated/' + name)
        g.need(raw == actual, 'source generation differs from named artifact: ' + name)
    g.need(seal['source_manifest'] == g.identity(g.CURRENT_NAME, artifacts[g.CURRENT_NAME]),
           'fixed successor manifest seal')
    return {'current': json.loads(artifacts[g.CURRENT_NAME]), 'inputs': inputs,
            'predecessor_inputs': restored, 'artifacts': artifacts,
            'source_only': True, 'execution_qualified': False}


def predecessor_capture(repo, admitted, seal):
    with predecessor.view(repo, admitted['predecessor_inputs'], seal['predecessor_view']) as view:
        runtime = predecessor.load(view, g.PACKAGE + 'run.py', 'package_json_formatter_runtime')
        captured = runtime.preflight(view, view / PACKAGE_PATH)
        g.need(captured['inputs'] == admitted['predecessor_inputs'], 'complete accepted predecessor inputs')
        previous_current._pure(captured, str(view))
        return copy.deepcopy(captured)


def verify_capture(captured, runner, evidence):
    """Exact pure association check, including the genuine complete predecessor."""
    historical = captured['package_json_predecessor']
    g.need(set(captured) == set(historical) | {'package_json_predecessor', 'package_json_seal', CURRENT_ACCOUNTING_KEY},
           'complete successor capture fields')
    generated, restored = g.generate(historical['package_bytes']['current-source.json'],
        captured['inputs'], captured['package_bytes'][g.PATCH_NAME], evidence, runner)
    g.need(restored == historical['inputs'], 'complete predecessor capture inverse')
    g.need(same(captured['current'], json.loads(generated[g.CURRENT_NAME])), 'current capture identity')
    for name, raw in generated.items():
        g.need(captured['package_bytes'][name] == raw, 'current captured artifact')
    for name, value in historical.items():
        if name not in {'current', 'inputs', 'package_bytes', 'package_manifest'}:
            g.need(same(captured[name], value), 'retained capture field changed: ' + name)
    package_rows = g.rows_map(json.loads(captured['package_manifest'])['files'])
    g.need(set(package_rows) == set(captured['package_bytes']) == set(historical['package_bytes']) | set(generated),
           'complete captured current package')
    for name, raw in captured['package_bytes'].items():
        g.need(same(g.identity(name, raw), package_rows[name]), 'captured package identity')
        if name not in set(generated) | {'run.py', 'current-source.json'}:
            g.need(raw == historical['package_bytes'][name], 'captured historical package body')
    g.need(captured['package_bytes']['current-source.json'] == generated[g.CURRENT_NAME], 'captured current alias')
    return generated


def _fixed_seal(seal):
    g.need(set(seal) == {'source_manifest', 'predecessor_view'}, 'fixed package JSON seal')
    for row in seal.values():
        g.need(set(row) == {'path', 'bytes', 'sha256'}, 'exact seal descriptor')


def preflight(repo, package, seal):
    g.need(sys.flags.optimize == 0 and __debug__, 'optimized Python unsupported')
    _fixed_seal(seal)
    repo, package = Path(repo), Path(package)
    admitted = admit(repo, seal)
    old = predecessor_capture(repo, admitted, seal)
    raw = g.regular(package, 'package-manifest.json')
    manifest = json.loads(raw)
    old_manifest = json.loads(old['package_manifest'])
    rows = g.rows_map(manifest['files'])
    g.need(set(rows) == set(old['package_bytes']) | set(admitted['artifacts']),
           'exact previous package plus four named successor artifacts')
    g.need({k:v for k,v in manifest.items() if k != 'files'}
           == {k:v for k,v in old_manifest.items() if k != 'files'}, 'package metadata changed')
    retained_current.closed_package(package, set(rows) | {'package-manifest.json'})
    bodies = {n: g.regular(package, n) for n in rows}
    for n, body in bodies.items():
        g.need(g.identity(n, body) == rows[n], 'current package identity')
        if n in admitted['artifacts']:
            g.need(body == admitted['artifacts'][n], 'named generation body')
        elif n == 'current-source.json':
            g.need(body == admitted['artifacts'][g.CURRENT_NAME], 'current source alias')
        elif n != 'run.py':
            g.need(body == old['package_bytes'][n], 'historical package artifact changed: ' + n)
    capture = {**copy.deepcopy(old), 'current': admitted['current'], 'inputs': admitted['inputs'],
               'package_bytes': bodies, 'package_manifest': raw, 'package_json_predecessor': old,
               'package_json_seal': copy.deepcopy(seal), CURRENT_ACCOUNTING_KEY: None}
    capture[CURRENT_ACCOUNTING_KEY] = unit2_accounting_binding(capture, package, seal)
    return capture


def unit2_accounting_binding(captured, package, seal):
    g.need(same(captured['package_json_seal'], seal), 'source seal capture changed')
    historical = captured['package_json_predecessor']
    previous = previous_current.validate_unit2_accounting(
        historical, package, historical['published_source_seal'])
    g.need(same(captured[PUBLISHED_ACCOUNTING_KEY], previous), 'formatter accounting binding changed')
    repo = Path(package).parents[2]
    runner = g.regular(repo, 'tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py')
    evidence = {n: g.regular(repo, g.ROOT + 'source-proof/' + n + '.json') for n in g.READBACKS}
    verify_capture(captured, runner, evidence)
    for name in g.ACCOUNTING_PATHS:
        g.need(captured['inputs'][name] == historical['inputs'][name], 'immutable accounting body changed')
    return {**copy.deepcopy(previous), 'version': 'unit2-package-json-identical-accounting-source-v1',
            'current_source': g.identity('current-source.json', captured['package_bytes']['current-source.json']),
            'reviewed_source_head': g.CHECKPOINT['head'], 'source_only_tree': g.CHECKPOINT['tree'],
            'predecessor_accounting_source_binding': previous,
            'package_json_provenance': copy.deepcopy(captured['current']['package_json_source'])}


def validate_unit2_accounting(captured, package, seal):
    expected = unit2_accounting_binding(captured, package, seal)
    g.need(same(captured[CURRENT_ACCOUNTING_KEY], expected), 'new accounting binding changed')
    return expected


def outer_receipt(captured):
    return {'accepted_formatter_receipt': previous_current.outer_receipt(captured['package_json_predecessor']),
            'package_json_source_provenance': copy.deepcopy(captured['current']['package_json_source']),
            CURRENT_ACCOUNTING_KEY: copy.deepcopy(captured[CURRENT_ACCOUNTING_KEY])}


def install(runtime, package, seal):
    """Wrap a fresh pinned byte-storage runner, never retained_current.install().

    Original preparation, verifier, main, observer controls, child commands,
    semantic/resource predicates, and result/failure accounting remain their
    unchanged functions. Only admission and explicit outer associations change.
    """
    _fixed_seal(seal)
    package = Path(package)
    saved = retained_current.retained_package(package)
    adapters.need(isinstance(runtime, types.ModuleType)
                  and runtime.PACKAGE == saved
                  and runtime.CURRENT_SOURCE_SHA
                  == retained_current.source_transition.PREDECESSOR_SHA
                  and same(runtime.CURRENT_SOURCE_BYTES,
                           retained_current.source_transition.PREDECESSOR_BYTES),
                  "published runtime requires the fresh byte-storage predecessor")
    adapters.need(adapters.sha(_regular(saved, "run.py").read_bytes())
                  == retained_current.source_transition.RETAINED_RUNNER_SHA,
                  "changed immutable byte-storage runtime body")
    for name in ("preflight", "prepare_archived", "prepare_unit2", "verify_unit2_result",
                 "assert_unchanged", "write_json", "main", "run_unit2_observer_controls"):
        function = getattr(runtime, name)
        adapters.need(isinstance(function, types.FunctionType)
                      and function.__globals__ is vars(runtime),
                      "wrapped or foreign byte-storage runtime function: " + name)
    fixed_seal = copy.deepcopy(seal)
    old_archived = runtime.prepare_archived
    old_prepare_unit2 = runtime.prepare_unit2
    old_verify_unit2_result = runtime.verify_unit2_result
    old_write_json = runtime.write_json
    runtime.PACKAGE = package
    runtime.__file__ = str(package / "run.py")
    runtime.CURRENT_SOURCE_SHA = fixed_seal["source_manifest"]["sha256"]
    runtime.CURRENT_SOURCE_BYTES = fixed_seal["source_manifest"]["bytes"]
    last = {}

    def admitted(repo, package=package):
        last.pop("captured", None)
        result = preflight(repo, package, fixed_seal)
        last["captured"] = result
        return result

    def assert_unchanged(repo, captured, package=package):
        fresh = admitted(repo, package)
        runtime.require(same(fresh, captured), "input identity changed during operation")

    def archived(output, captured):
        validate_unit2_accounting(captured, package, fixed_seal)
        return _extend_receipt(old_archived(output, captured), outer_receipt(captured))

    def prepare_unit2(output, captured):
        current_binding = validate_unit2_accounting(captured, package, fixed_seal)
        result = old_prepare_unit2(output, captured)
        adapters.need(same(result[RETAINED_ACCOUNTING_KEY], captured[RETAINED_ACCOUNTING_KEY]),
                      "changed prepared byte-storage accounting binding")
        return _extend_receipt(result, {
            PUBLISHED_ACCOUNTING_KEY: captured[PUBLISHED_ACCOUNTING_KEY],
            LEXER_ACCOUNTING_KEY: captured[LEXER_ACCOUNTING_KEY],
            CURRENT_ACCOUNTING_KEY: current_binding,
        })

    def verify_unit2_result(output, captured, seam, prepare_only):
        current_binding = validate_unit2_accounting(captured, package, fixed_seal)
        adapters.need(same(seam[PUBLISHED_ACCOUNTING_KEY], captured[PUBLISHED_ACCOUNTING_KEY])
                      and same(seam[RETAINED_ACCOUNTING_KEY], captured[RETAINED_ACCOUNTING_KEY])
                      and same(seam[LEXER_ACCOUNTING_KEY], captured[LEXER_ACCOUNTING_KEY])
                      and same(seam[CURRENT_ACCOUNTING_KEY], current_binding),
                      "changed materialized accounting associations")
        # No retained verifier result or predicate is replaced, suppressed, or
        # interpreted as a fresh result without the original verifier succeeding.
        result = old_verify_unit2_result(output, captured, seam, prepare_only)
        selected = runtime.regular(output / "unit2/derived-package",
                                   "source-inputs.json").read_bytes()
        adapters.need(selected == captured["package_bytes"]["current-source.json"]
                      and same(adapters.binding("current-source.json", selected),
                               current_binding["current_source"]),
                      "wrong materialized published current manifest")
        source = output / "unit2/run/source"
        assembly = runtime.read_json(output / "unit2/run/assembly.json")
        adapters.need(assembly["source_inputs_sha256"]
                      == current_binding["current_source"]["sha256"],
                      "wrong current source assembly association")
        # Keep the actual admitted instrumentation map. Its five modified source
        # bodies are not incorrectly compared with uninstrumented C.
        runtime.check_entries(source, assembly["files"], exact=True)
        historical = captured["package_json_predecessor"]
        g.verify_materialized(source, captured["inputs"], historical["inputs"])
        main = "src/main.rs"
        adapters.need(runtime.regular(source, main).read_bytes() == captured["inputs"][main]
                      and captured["inputs"][main] != historical["inputs"][main],
                      "wrong materialized published main source")
        lexer = "src/frontend/lexer.rs"
        adapters.need(runtime.regular(source, lexer).read_bytes()
                      == captured["inputs"][lexer] == historical["inputs"][lexer]
                      and captured["inputs"][lexer]
                      != historical["byte_storage_inputs"][lexer],
                      "wrong materialized current lexer")
        for row in current_binding["source_dependencies"]:
            _verify(runtime.regular(source, row["path"]).read_bytes(), row, row["path"])
        return _extend_receipt(result, {
            PUBLISHED_ACCOUNTING_KEY: captured[PUBLISHED_ACCOUNTING_KEY],
            LEXER_ACCOUNTING_KEY: captured[LEXER_ACCOUNTING_KEY],
            CURRENT_ACCOUNTING_KEY: current_binding,
        })

    def failure_provenance(path, value, extension=None, error=None):
        """Optional metadata must never replace an already-written failure."""
        try:
            sidecar = Path(path).with_name(Path(path).name + ".package-json-provenance.json")
            record = {
                "schema": "oxid-package-json-failure-provenance-v1",
                "primary_receipt": Path(path).name,
                "primary_status": "failed",
                "primary_error": value.get("error"),
                "provenance_status": "rejected" if error is not None else "validated",
                "provenance_error": error,
                "provenance": extension,
            }
            old_write_json(sidecar, record)
        except (Exception, KeyboardInterrupt) as exc:
            # The primary failure is already preserved through the original
            # writer. Even reporting this secondary I/O failure cannot mask it.
            try:
                print("Could not record secondary published provenance: "
                      + type(exc).__name__ + ": " + str(exc), file=sys.stderr)
            except (Exception, KeyboardInterrupt):
                pass

    def write_json(path, value):
        if not (isinstance(value, dict)
                and value.get("schema") == "oxid-current-archive-binding-v1"
                and "captured" in last):
            old_write_json(path, value)
            return
        primary_failed = value.get("status") == "failed"
        if primary_failed:
            # Original main writes failure.json in finally, outside its try.
            # Preserve the original failure and its exact error BEFORE any
            # package reopen, capture validation, or optional provenance work.
            old_write_json(path, value)
        try:
            validate_unit2_accounting(last["captured"], package, fixed_seal)
            extension = outer_receipt(last["captured"])
            extended = _extend_receipt(value, extension)
        except (Exception, KeyboardInterrupt) as exc:
            error = type(exc).__name__ + ": " + str(exc)
            if primary_failed:
                failure_provenance(path, value, error=error)
                return
            if Path(path).name in ("result.json", "prepared.json"):
                # Terminal writes are also outside original main's try. Never
                # emit success/preparation without its required provenance, and
                # retain the attempted result separately from the failed verdict.
                failed = {
                    **value,
                    "status": "failed",
                    "semantic_pass": False,
                    "error": "published source provenance extension rejected: " + error,
                    "published_source_provenance_error": error,
                    "attempted_terminal_receipt_name": Path(path).name,
                    "attempted_terminal_receipt": copy.deepcopy(value),
                }
                old_write_json(Path(path).with_name("failure.json"), failed)
            # In-try plan writes keep original main's normal failure handling;
            # terminal rejection also exits unsuccessfully after its failure write.
            raise
        if primary_failed:
            failure_provenance(path, value, extension=extension)
            return
        old_write_json(path, extended)

    runtime.preflight = admitted
    runtime.prepare_archived = archived
    runtime.prepare_unit2 = prepare_unit2
    runtime.verify_unit2_result = verify_unit2_result
    runtime.assert_unchanged = assert_unchanged
    runtime.write_json = write_json
    return runtime
