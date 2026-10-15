"""INACTIVE deterministic, pure public-observer byte derivation. No CLI or I/O.

A separately reviewed caller authenticates this exact generator before loading
it and supplies the exact eight byte buffers below. derive() neither imports the
published dispatcher nor loads a runtime/target helper. It extracts only the
explicit pure declarations from the independently pinned transport source.
It returns five byte buffers; a separately authorized caller owns all writes.
Generation is not an original lifecycle admission, materialization, runtime
qualification, compiler invocation, source execution or 173-consumer audit.

Pin DAG: immutable inputs -> helper -> pure generator; proof pins helper and
immutable sources, authority pins proof/helper, build/test consume those, and
the derivation receipt pins the four outputs. No proof pins a consumer that
contains its digest. Final exact-head CI must independently pin generator,
helper, proof, consumers, unchanged runtime/gate closure and the existing
173-consumer inventory. This inactive recipe does not activate that CI gate.
"""
import ast
import hashlib
import json
from pathlib import PurePosixPath
import re
import types

TRANSPORT_PIN = {'bytes': 45254,
    'sha256': '8958281fad4bd60e581f8642f6098dde76dcf2a354d8493d02d62bc837bff599'}
INPUT_NAMES = (
    'published_lifecycle.py', 'source_dispatcher.py', 'source-package-manifest.json',
    'published-formatter-source-v1.json', 'lexer-reservation-lifecycle-v2.json',
    'authority.py', 'build.py', 'test_lexer_reservation.py')
PURE_CONSTANTS = (
    'PUBLIC', 'PACKAGE', 'SAVED', 'PROOF_NAME', 'HELPER_NAME', 'MAIN',
    'DISPATCHER_PIN', 'PACKAGE_PIN', 'OLD_HELPER_PIN', 'OLD_PROOF_PIN', 'PATCH_PIN',
    'PREVIOUS_PATCH_PIN', 'PREVIOUS_AUTHORITY_PIN', 'OLD_AUTHORITY_PIN',
    'OLD_BUILD_PIN', 'OLD_TEST_PIN', 'RETAINED_PINS', 'HISTORICAL_PACKAGE_PIN',
    'PUBLISHED_MAIN', 'CHECKPOINT', 'HISTORICAL', 'OLD_CURRENT_VALUES',
    'HELPER_ORDER', 'RETAINED_ALIASES', 'SEAL_KEYS', 'PROOF_KEYS', 'MAX_BODY')
PURE_FUNCTIONS = (
    '_need', '_digest', '_binding', '_same', '_json', '_canonical', '_relative',
    '_descriptor', '_verify', '_rows_schema', '_literal', '_dispatcher',
    '_proof_schema', 'make_proof', '_authority_values', '_authority_suffix',
    'render_authority', 'render_build', 'render_tests')
ORIGINAL_TESTS = (
    'test_complete_maps_and_exact_inverse', 'test_nonlexer_insertions_unchanged',
    'test_each_exact_hunk_context_is_checked',
    'test_reordered_and_duplicate_patch_sections_rejected',
    'test_missing_duplicate_and_wrapper_hooks_reject',
    'test_stale_predecessor_and_tampered_patch_reject')
OUTPUT_LIMITS = {'published-lifecycle-v1.json': 524288, 'authority.py': 8192,
                 'build.py': 32768, 'test_lexer_reservation.py': 16384,
                 'derivation-v1.json': 32768}


class DerivationRejected(ValueError):
    pass


def need(ok, message):
    if not ok:
        raise DerivationRejected(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def binding(name, raw):
    return {'path': name, 'bytes': len(raw), 'sha256': sha(raw)}


def encode(value):
    return (json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True,
                       allow_nan=False) + '\n').encode()


def _constant_expression(node, assigned):
    """Only literal construction and concatenation of earlier fixed constants."""
    if isinstance(node, ast.Constant):
        return type(node.value) in (str, int, bool, bytes, type(None))
    if isinstance(node, ast.Name):
        return node.id in assigned
    if isinstance(node, (ast.List, ast.Tuple, ast.Set)):
        return all(_constant_expression(item, assigned) for item in node.elts)
    if isinstance(node, ast.Dict):
        return all(key is not None and _constant_expression(key, assigned)
                   and _constant_expression(value, assigned)
                   for key, value in zip(node.keys, node.values))
    if isinstance(node, ast.BinOp) and isinstance(node.op, (ast.Add, ast.Mult)):
        return _constant_expression(node.left, assigned) and _constant_expression(node.right, assigned)
    return False


def _pure_recipe(raw):
    need(type(raw) is bytes and len(raw) == TRANSPORT_PIN['bytes']
         and sha(raw) == TRANSPORT_PIN['sha256'], 'changed independently pinned transport recipe')
    tree = ast.parse(raw)
    selected, assigned, functions = [], [], []
    for node in tree.body:
        if isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name):
            name = node.targets[0].id
            if name in PURE_CONSTANTS:
                need(name not in assigned and _constant_expression(node.value, set(assigned)),
                     'unsafe or duplicate pure constant: ' + name)
                assigned.append(name)
                selected.append(node)
        elif isinstance(node, ast.FunctionDef) and node.name in PURE_FUNCTIONS:
            need(node.name not in functions and not node.decorator_list,
                 'duplicate or decorated pure function')
            functions.append(node.name)
            selected.append(node)
    need(tuple(assigned) == PURE_CONSTANTS and tuple(functions) == PURE_FUNCTIONS,
         'pure declaration closure changed')
    # Only these standard-library modules/types and the declared functions are
    # available. The authenticated helper's bootstrap, paths, file readers,
    # original helper execution, aliases, lock and admission are never selected.
    module = types.ModuleType('published_observer_pure_recipe')
    module.__dict__.update(ast=ast, hashlib=hashlib, json=json, re=re,
                           PurePosixPath=PurePosixPath, Rejected=DerivationRejected)
    compiled = ast.Module(body=selected, type_ignores=[])
    ast.fix_missing_locations(compiled)
    exec(compile(compiled, '<authenticated-pure-public-observer-recipe>', 'exec'), module.__dict__)
    return module


def _assert_original_tests(original, transported):
    def methods(raw):
        tree = ast.parse(raw)
        matches = [node for node in tree.body if isinstance(node, ast.ClassDef)
                   and node.name == 'CurrentLifecycle']
        need(len(matches) == 1, 'original lifecycle class changed')
        return {node.name: node for node in matches[0].body
                if isinstance(node, ast.FunctionDef) and node.name.startswith('test_')}
    before, after = methods(original), methods(transported)
    need(tuple(before) == tuple(after) == ORIGINAL_TESTS, 'six original tests not preserved')
    rows = []
    for name in ORIGINAL_TESTS:
        old = ast.dump(before[name], annotate_fields=True, include_attributes=False)
        new = ast.dump(after[name], annotate_fields=True, include_attributes=False)
        need(old == new, 'original lifecycle assertion body changed: ' + name)
        rows.append({'name': name, 'unchanged_ast_sha256': sha(old.encode())})
    return rows


def _check_dag():
    graph = {'helper': (), 'generator': ('helper',), 'proof': ('helper',),
             'authority': ('proof', 'helper'), 'build': ('authority', 'helper'),
             'tests': ('proof', 'helper'), 'receipt': ('proof', 'authority', 'build', 'tests')}
    def visit(node, active, seen):
        need(node not in active, 'cyclic generated consumer pin')
        if node in seen:
            return
        for child in graph[node]:
            visit(child, active | {node}, seen)
        seen.add(node)
    seen = set()
    for node in graph:
        visit(node, set(), seen)
    return {name: list(children) for name, children in graph.items()}


def derive(inputs):
    """Return proof/authority/build/test/receipt bytes; perform no filesystem I/O.

    Inputs contain original immutable authority/build/test buffers, never the
    proposed generated consumers. The helper body is pinned above independently
    of every generated proof and authority. The caller must pin this generator
    externally before calling; no generator self-pin is inferred here.
    """
    need(type(inputs) is dict and set(inputs) == set(INPUT_NAMES)
         and all(type(name) is str and type(raw) is bytes for name, raw in inputs.items()),
         'exact eight input byte buffers required')
    need(all(0 < len(raw) <= 524288 for raw in inputs.values()), 'oversized pure recipe input')
    recipe = _pure_recipe(inputs['published_lifecycle.py'])
    try:
        recipe._verify(inputs['authority.py'], recipe.OLD_AUTHORITY_PIN)
        recipe._verify(inputs['build.py'], recipe.OLD_BUILD_PIN)
        recipe._verify(inputs['test_lexer_reservation.py'], recipe.OLD_TEST_PIN)
        proof = recipe.make_proof(
            inputs['lexer-reservation-lifecycle-v2.json'],
            inputs['published-formatter-source-v1.json'], inputs['source_dispatcher.py'],
            inputs['source-package-manifest.json'], inputs['published_lifecycle.py'])
        raw = encode(proof)
        outputs = {
            'published-lifecycle-v1.json': raw,
            'authority.py': recipe.render_authority(inputs['authority.py'], raw,
                                                      inputs['published_lifecycle.py']),
            'build.py': recipe.render_build(inputs['build.py']),
            'test_lexer_reservation.py': recipe.render_tests(inputs['test_lexer_reservation.py'])}
        assertions = _assert_original_tests(inputs['test_lexer_reservation.py'],
                                            outputs['test_lexer_reservation.py'])
        # Exact replacements preserve every other byte, including all original
        # public runtime/gate functions, 16 context controls and two sequential
        # admission calls. No replacement fixture or weakened assertion is added.
        receipt = {
            'schema': 'oxid-published-public-observer-derivation-v1',
            'recipe': 'authenticated-pure-declarations-and-exact-consumer-seams-v1',
            'inputs': [binding(name, inputs[name]) for name in INPUT_NAMES],
            'outputs': [binding(name, outputs[name]) for name in sorted(outputs)],
            'six_original_assertion_bodies': assertions,
            'build_import_replacements': 2, 'test_import_replacements': 1,
            'test_proof_name_replacements': 1,
            'changed_current_authority_constants': [
                'CURRENT_SOURCE_SHA', 'CURRENT_FILES_SHA', 'OBSERVER_FILES_SHA'],
            'historical_authority_constants_unchanged': [
                'LEXER_LIFECYCLE_HELPER_SHA', 'LEXER_LIFECYCLE_AUTHORITY_SHA',
                'LIFECYCLE_PATCH_SHA'],
            'new_named_descriptors': ['PUBLISHED_LIFECYCLE_HELPER', 'PUBLISHED_LIFECYCLE_PROOF'],
            'generated_pin_dag': _check_dag(),
            'base_members': 376, 'observer_members': 377,
            'transport_paths': ['src/main.rs'],
            'compiler_invocations': 0, 'materializations': 0,
            'original_lifecycle_admissions': 0,
            'source_only': True, 'execution_qualified': False,
            'consumer_inventory_modified': False,
            'exact_head_ci_activation': False,
            'runtime_or_host_results_relabelled': False,
            'external_review_required': [
                'exact helper and generator bytes', 'all five generated output bytes',
                'unchanged public runtime and gate closure',
                'unchanged 173-consumer inventory and complete final CI input closure',
                'actual caller exclusion, effects and C/M/package/compiled-global overlap',
                'fresh materialization and control-batch gates']}
        outputs['derivation-v1.json'] = encode(receipt)
        need(set(outputs) == set(OUTPUT_LIMITS), 'unexpected generated output')
        for name, body in outputs.items():
            need(type(body) is bytes and len(body) <= OUTPUT_LIMITS[name],
                 'reserved output slot exceeded: ' + name)
        return outputs
    finally:
        recipe.__dict__.clear()
