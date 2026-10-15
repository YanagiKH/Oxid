"""Inactive, pure-byte derivation of the one existing-discovery CI entrypoint.

No filesystem access, target imports, subprocesses, tests or materialization.
The caller supplies exact independently reviewed final pins and bodies. This
recipe cannot invent unknown final consumer/helper identities or approve itself.
"""
import ast
import hashlib
import json
import re
from pathlib import PurePosixPath

ENTRY = "scripts/test_published_formatter_source.py"
SLOT = b'_CONFIG = None\n_CONFIG_SLOT = "CI_SOURCE_CONTROLS_FINAL_CONFIGURATION_V1"\n'
PACKAGE = "tests/fixtures/typed_project_source_binding/"
CURRENT = "tests/qualification/published_formatter_current/"
CONSUMER = "tests/qualification/lexer_reservation_current/unit1_current_support.py"
PUBLIC = "tests/qualification/unit4_public_v3/"
PUBLIC_REQUIRED = frozenset(PUBLIC + name for name in (
    "published_lifecycle.py", "published-lifecycle-v1.json", "authority.py", "build.py",
    "test_lexer_reservation.py", "lexer_reservation_lifecycle.py", "lexer-reservation-lifecycle-v2.json",
    "observer-lexer-reservation-v2.patch", "observer-u8-v1.patch", "byte_storage_source_authority.py"))
RETAINED = "tests/fixtures/typed_project_source_binding_byte_storage_v1/"
DOWNSTREAM_CLOSURE = frozenset((
    "tests/qualification/unit4_ci/inputs.json",
    "tests/qualification/unit4_ci/inputs-published-formatter-v1.json",
    "tests/qualification/unit4_ci/common.py",
))
SOURCE_HEAD = "fbff527c895e6cd4ec9350bd8b5d460bc20bb80d"
SOURCE_TREE = "89e90c3ec83d0712d0037a0407fc6571883c5e9c"
M_HEAD = "15eac633e167c261ba0246ced49cfdc1c6cef434"
HISTORICAL_HEAD = "b3abc9f0dda99d6d8fe65d9c3a9ed31dedcbd489"
ACCEPTED = {
    CURRENT + "published_transition.py": (12447, "069f7610fe2ffd3e51c47b1d2ed8f6db61f774dccb1dabf6977b24472e973a45"),
    CURRENT + "published_current.py": (32290, "11f96d500198406d29e1a1e0880ade13dd8e2afd799315a08d5febcf5e889393"),
    CURRENT + "published_view.py": (11861, "2009903d72ea6e55fbfb10aaa00848397ec45281b5c47b2b9cd1677a6697609f"),
    CURRENT + "published_ci_inventory.py": (8640, "b4ceba16b7cd03893035e6fe63c2ef70740ccba2704277237fe6909b1d939e2e"),
    CURRENT + "test_published_view.py": (11345, "b876b6d58fe5b40e274168117188e34236cf80d8304cc17eff9f2fddb9b54bd0"),
    CURRENT + "test_published_ci_inventory.py": (12577, "aa86d277358e586030bf6a1c8d6ec6e8823512e99a736704f28c1c6d167b01c1"),
    PACKAGE + "run.py": (5824, "695ea921fa6c263fd22bb8bd11a8ac01046002ab2feb8943c3831e289ac78b98"),
    PACKAGE + "package-manifest.json": (16025, "9a36d668a53e763bc06cb8e38b32d9808152537bafe14c2dcd0fd4d9a9b64ec0"),
    RETAINED + "run.py": (216279, "584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76"),
}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def row(name, raw):
    return {"path": name, "bytes": len(raw), "sha256": sha(raw)}


def relative(name):
    need(type(name) is str and name and not name.startswith("/") and "\\" not in name
         and str(PurePosixPath(name)) == name
         and all(part not in (".", "..") for part in name.split("/")), "unsafe recipe path")
    return name


def verify_pin(raw, pin, name):
    need(type(raw) is bytes and type(pin) is dict and set(pin) == {"path", "bytes", "sha256"}
         and pin["path"] == name and type(pin["bytes"]) is int and 0 <= pin["bytes"] <= 2097152
         and type(pin["sha256"]) is str and re.fullmatch("[0-9a-f]{64}", pin["sha256"])
         and row(name, raw) == pin, "missing or changed externally reviewed final pin: " + name)


def literal(raw, name):
    found = [node.value for node in ast.parse(raw).body if isinstance(node, ast.Assign)
             and any(isinstance(target, ast.Name) and target.id == name for target in node.targets)]
    need(len(found) == 1, "missing exact declaration: " + name)
    return ast.literal_eval(found[0])


def named_test_roster(raw, factory, class_name):
    tree = ast.parse(raw)
    functions = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == factory]
    need(len(functions) == 1, "missing explicit control factory")
    classes = [node for node in functions[0].body if isinstance(node, ast.ClassDef) and node.name == class_name]
    need(len(classes) == 1, "missing explicit control class")
    names = [node.name for node in classes[0].body
             if isinstance(node, ast.FunctionDef) and node.name.startswith("test_")]
    need(len(names) == len(set(names)), "duplicate accepted control names")
    return sorted(names)


def package_closure(inputs, manifest_path, expected_count):
    raw = inputs[manifest_path]
    value = json.loads(raw)
    need(type(value) is dict and type(value.get("files")) is list
         and len(value["files"]) == expected_count, "wrong genuine package cardinality")
    prefix = str(PurePosixPath(manifest_path).parent) + "/"
    names = []
    for item in value["files"]:
        name = relative(item["path"])
        need(name != "package-manifest.json", "self-referential source package manifest")
        full = prefix + name
        verify_pin(inputs[full], {**item, "path": full}, full)
        names.append(name)
    need(names == sorted(set(names)), "changed package row ordering or duplicates")


def verify_consumer(raw, dispatcher_raw, manifest_raw):
    """Static linkage check supplements, never replaces, external consumer pin."""
    tree = ast.parse(raw)
    by_name = {node.name: node for node in tree.body if isinstance(node, ast.FunctionDef)}
    need("verify_source_entrypoint" in by_name and "admit" in by_name, "real consumer extraction is absent")
    method = by_name["verify_source_entrypoint"]
    need([arg.arg for arg in method.args.args] == ["dispatcher_raw", "manifest_raw"]
         and not method.args.vararg and not method.args.kwarg and not method.args.defaults,
         "changed reviewed consumer buffer interface")
    messages = [node.value for node in ast.walk(method) if isinstance(node, ast.Constant) and type(node.value) is str]
    need("Changed trusted current source dispatcher" in messages
         and "Changed trusted current source-binding package manifest" in messages,
         "consumer predicate messages changed")
    need(literal(raw, "DISPATCHER_SHA") == sha(dispatcher_raw)
         and literal(raw, "SOURCE_PACKAGE_SHA") == sha(manifest_raw), "consumer final fixed pins disagree")
    calls = [node for node in ast.walk(by_name["admit"]) if isinstance(node, ast.Call)
             and isinstance(node.func, ast.Name) and node.func.id in ("verify_source_entrypoint", "load_module")]
    verify_calls = [node for node in calls if node.func.id == "verify_source_entrypoint"]
    import_calls = [node for node in calls if node.func.id == "load_module"]
    need(len(verify_calls) == 1 and len(import_calls) == 1
         and verify_calls[0].lineno < import_calls[0].lineno, "real consumer does not use boundary before import")


TRANSITION_TEST = CURRENT + "test_published_transition.py"
ACCEPTED_TRANSITION_TEST = {"path": TRANSITION_TEST, "bytes": 63339,
    "sha256": "98c7b9ce73173f822971dd7e63e5dbd6db8b1dc2660e4949d7b9e29e3e61f158"}
# Exact accepted text, including its original wording and indentation.
OLD_FACTORY_DOC = (b"    pinned byte runtime, output, full capture, and seam. It must use genuine\n"
                   b"    unchanged preparation/assembly code with compiler/network effects blocked.\n")
NEW_FACTORY_DOC = (b"    pinned byte runtime, output, full capture, and seam. It must use genuine\n"
                   b"    unchanged preparation/assembly code. The pinned --prepare-only target path\n"
                   b"    contains no compiler or network calls. This is structural control-flow\n"
                   b"    exclusion in ordinary trusted CI, not an OS sandbox or startup attestation.\n")


def derive_transition_test_docstring(accepted_raw):
    """Return a comment-only successor; never overwrite the accepted old file."""
    verify_pin(accepted_raw, ACCEPTED_TRANSITION_TEST, TRANSITION_TEST)
    need(accepted_raw.count(OLD_FACTORY_DOC) == 1 and NEW_FACTORY_DOC not in accepted_raw,
         "missing unique accepted factory docstring seam")
    successor = accepted_raw.replace(OLD_FACTORY_DOC, NEW_FACTORY_DOC, 1)
    need(successor.count(NEW_FACTORY_DOC) == 1
         and successor.replace(NEW_FACTORY_DOC, OLD_FACTORY_DOC, 1) == accepted_raw,
         "factory docstring inverse does not recover every accepted byte")
    before, after = ast.parse(accepted_raw), ast.parse(successor)
    old_factory = next(node for node in before.body
                       if isinstance(node, ast.FunctionDef) and node.name == "build_integration_suite")
    new_factory = next(node for node in after.body
                       if isinstance(node, ast.FunctionDef) and node.name == "build_integration_suite")
    need(isinstance(old_factory.body[0], ast.Expr) and isinstance(new_factory.body[0], ast.Expr),
         "factory seam is not a docstring")
    new_factory.body[0] = old_factory.body[0]
    need(ast.dump(before, include_attributes=False) == ast.dump(after, include_attributes=False),
         "factory successor changed executable syntax or an assertion")
    return successor


def derive(template_raw, *, reviewed_template_pin, final_inputs, final_input_pins,
           final_consumer_pin, final_source_seal, historical_package_manifest,
           ci_data, final_public_pins, accepted_transition_test_raw,
           final_transition_test_pin):
    """Return final entry/test buffers and a source-only derivation description.

    Every keyword is mandatory. No final hash is taken from ambient files, an
    environment variable, a coherent self-manifest or a default placeholder.
    The outer reviewer must authenticate the parameters independently and review
    the generated buffer before the final closure seal is constructed.
    """
    verify_pin(template_raw, reviewed_template_pin, "draft/test_published_formatter_source.py")
    need(len(template_raw) <= 65536 and template_raw.count(SLOT) == 1, "changed unique entrypoint configuration seam")
    need(type(final_inputs) is dict and type(final_input_pins) is list
         and 0 < len(final_input_pins) <= 2048, "missing complete finite input authentication roster")
    names = [pin["path"] for pin in final_input_pins]
    need(names == sorted(set(names)) == sorted(final_inputs), "final pin/body roster mismatch")
    need(ENTRY not in final_inputs and not DOWNSTREAM_CLOSURE.intersection(final_inputs),
         "entrypoint dependencies would create a CI-input/common self-hash cycle")
    for pin in final_input_pins:
        name = relative(pin["path"])
        verify_pin(final_inputs[name], pin, name)
    successor_test = derive_transition_test_docstring(accepted_transition_test_raw)
    verify_pin(successor_test, final_transition_test_pin, TRANSITION_TEST)
    need(final_inputs[TRANSITION_TEST] == successor_test,
         "final source controls are not the independently reviewed comment-only successor")
    for name, (size, digest) in ACCEPTED.items():
        need(name in final_inputs and (len(final_inputs[name]), sha(final_inputs[name])) == (size, digest),
             "changed accepted source/CI dependency: " + name)
    modules = literal(template_raw, "MODULES")
    need(type(modules) is tuple and len(modules) == 15
         and all(path in final_inputs for _, path in modules), "missing final executable helper/test buffer")
    verify_pin(final_inputs[CONSUMER], final_consumer_pin, CONSUMER)
    verify_consumer(final_inputs[CONSUMER], final_inputs[PACKAGE + "run.py"],
                    final_inputs[PACKAGE + "package-manifest.json"])
    need(type(final_public_pins) is list, "missing separately reviewed public consumer closure")
    public_names = [item["path"] for item in final_public_pins]
    need(public_names == sorted(set(public_names)) and PUBLIC_REQUIRED <= set(public_names),
         "incomplete final public original-six closure")
    for item in final_public_pins:
        verify_pin(final_inputs[item["path"]], item, item["path"])
    public_raw = final_inputs[PUBLIC + "test_lexer_reservation.py"]
    old_public = public_raw.replace(b"import published_lifecycle as m\n", b"import lexer_reservation_lifecycle as m\n").replace(
        b"'published-lifecycle-v1.json'", b"'lexer-reservation-lifecycle-v2.json'")
    need(len(old_public) == 4442 and sha(old_public) == "5afea775ea089e577487c3de71ed5a803707d50d176a3d8e52a99aa4a7e1e132"
         and old_public.count(b"import lexer_reservation_lifecycle as m\n") == 1
         and old_public.count(b"'lexer-reservation-lifecycle-v2.json'") == 1
         and old_public.replace(b"import lexer_reservation_lifecycle as m\n", b"import published_lifecycle as m\n").replace(
             b"'lexer-reservation-lifecycle-v2.json'", b"'published-lifecycle-v1.json'") == public_raw,
         "original public six assertion bodies changed outside exact transport seams")
    need(type(final_source_seal) is dict
         and set(final_source_seal) == {"schema", "source_manifest", "authority", "patch", "source_proof", "predecessor_view"}
         and final_source_seal["schema"] == "oxid-published-main-source-seal-v1", "missing final noncyclic source seal")
    need(literal(final_inputs[PACKAGE + "run.py"], "_SOURCE_SEAL") == final_source_seal,
         "entrypoint source seal differs from accepted real dispatcher")
    for field in ("source_manifest", "authority", "patch", "source_proof", "predecessor_view"):
        item = final_source_seal[field]
        name = item["path"] if field == "predecessor_view" else PACKAGE + item["path"]
        verify_pin(final_inputs[name], {**item, "path": name}, name)
    source = json.loads(final_inputs[PACKAGE + final_source_seal["source_manifest"]["path"]])
    need(source["reviewed_source_head"] == SOURCE_HEAD and source["source_only_tree"] == SOURCE_TREE
         and source["published_main_head"] == M_HEAD and len(source["files"]) == 376,
         "wrong current source checkpoint or selected closure")
    for item in source["files"]:
        verify_pin(final_inputs[item["path"]], item, item["path"])
    package_closure(final_inputs, PACKAGE + "package-manifest.json", 99)
    package_closure(final_inputs, RETAINED + "package-manifest.json", 88)
    # Old M package is read through its preserved exact alias; that location is
    # mandatory rather than guessed from the current 99-member package.
    relative(historical_package_manifest)
    need(historical_package_manifest in final_inputs, "missing genuine old95 package manifest")
    old = json.loads(final_inputs[historical_package_manifest])
    need(len(old["files"]) == 95, "wrong genuine historical package roster")
    historical_descriptor = {"bytes": 15335, "sha256": "5fb5ee78a42609dab51f8e8bb23c325305954f49c52f0d18df6bbe47e57548bc"}
    need(len(final_inputs[historical_package_manifest]) == historical_descriptor["bytes"]
         and sha(final_inputs[historical_package_manifest]) == historical_descriptor["sha256"],
         "changed independently frozen M package manifest")
    view = json.loads(final_inputs[final_source_seal["predecessor_view"]["path"]])
    need(len(view["files"]) == 681 and len(view["directories"]) == 111
         and view["logical_bytes"] == 38003640 and view["published_main"]["head"] == M_HEAD,
         "wrong preserved complete M681 view")
    selected = {item["path"] for item in source["files"]}
    for item in view["files"]:
        if item["read_from"] is None:
            need(item["path"] in selected, "unexpected inverse-only M member")
        else:
            name = relative(item["read_from"])
            verify_pin(final_inputs[name], {key: name if key == "path" else item[key]
                                          for key in ("path", "bytes", "sha256")}, name)
    need(type(ci_data) is dict and set(ci_data) == {"audit", "oldest", "middle", "old_dispositions"}
         and all(type(name) is str and name in final_inputs for name in ci_data.values()),
         "missing genuine unchanged 173-consumer inventory inputs")
    transition_tests = final_inputs[CURRENT + "test_published_transition.py"]
    rosters = {"source_unit": list(literal(transition_tests, "UNIT_CONTROLS")),
               "source_integration": list(literal(transition_tests, "INTEGRATION_CONTROLS")),
               "view": named_test_roster(final_inputs[CURRENT + "test_published_view.py"], "build_suite", "GenuinePredecessorView"),
               "ci": named_test_roster(final_inputs[CURRENT + "test_published_ci_inventory.py"], "build_suite", "ExactWorkflowTransport")}
    public_classes = [node for node in ast.parse(public_raw).body
                      if isinstance(node, ast.ClassDef) and node.name == "CurrentLifecycle"]
    need(len(public_classes) == 1, "missing original public CurrentLifecycle class")
    rosters["public_original"] = sorted(node.name for node in public_classes[0].body
                                        if isinstance(node, ast.FunctionDef) and node.name.startswith("test_"))
    need({name: len(value) for name, value in rosters.items()}
         == {"source_unit": 37, "source_integration": 15, "view": 14, "ci": 20, "public_original": 6}, "changed accepted test counts")
    configuration = {"schema": "oxid-published-source-ci-entry-v1", "input_roster": final_input_pins,
                     "source_seal": final_source_seal, "historical_package_manifest": historical_package_manifest,
                     "ci_data": ci_data, "test_rosters": rosters}
    # Python literal rendering is deterministic for the caller's normalized
    # canonical JSON value; no execution or template-defined eval is used.
    canonical = json.loads(json.dumps(configuration, sort_keys=True, allow_nan=False))
    replacement = ("_CONFIG = " + repr(canonical) + "\n").encode()
    generated = template_raw.replace(SLOT, replacement)
    need(SLOT not in generated and b"_CONFIG = None" not in generated
         and len(generated) <= 262144, "unresolved or oversized generated CI entrypoint")
    parsed = ast.parse(generated)
    need(literal(generated, "_CONFIG") == canonical, "generated configuration changed")
    report = {"schema": "oxid-published-source-ci-derivation-v1", "entrypoint": row(ENTRY, generated),
              "template": reviewed_template_pin,
              "transition_test_predecessor": ACCEPTED_TRANSITION_TEST,
              "transition_test_successor": final_transition_test_pin,
              "transition_test_change": "one factory docstring only; exact inverse and executable AST preserved",
              "real_consumer": final_consumer_pin, "input_count": len(final_input_pins),
              "original_public_consumer_closure": final_public_pins,
              "additional_public_controls_pending": 36,
              "control_counts": {key: len(value) for key, value in rosters.items()},
              "workflow_commands_added": 0, "workflow_jobs_added": 0,
              "tests_executed": 0, "children_launched": 0, "views_materialized": 0,
              "source_only": True, "execution_qualified": False,
              "self_pinned_entrypoint": False, "depends_on_final_ci_inputs_or_common": False,
              "source_checkpoint_head": SOURCE_HEAD, "published_main_head": M_HEAD,
              "historical_source_head": HISTORICAL_HEAD}
    report_raw = (json.dumps(report, sort_keys=True, indent=2, allow_nan=False) + "\n").encode()
    need(len(successor_test) <= 65536 and len(report_raw) <= 65536,
         "generated test or canonical derivation report exceeds its reserved output slot")
    return {ENTRY: generated, TRANSITION_TEST: successor_test}, report
