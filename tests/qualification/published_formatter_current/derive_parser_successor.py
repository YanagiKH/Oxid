"""Inactive exact source recipe. No seal, derived adapter or output is bundled.

derive() is pure: it accepts the preserved complete portable body and a reviewed
fixed seal, and returns bytes plus a source-difference receipt. It does not load
the adapter, run predicates, materialize files, or write an activation artifact.
"""
import ast
import hashlib
import json

PREDECESSOR_SHA = "3366c49f267ab98bd792a1cd1b01ebbaaca548bc19685d841eb688cf39f99162"
FUNCTIONS = (
    "authority", "phase_previous", "current_candidate",
    "compose_current_lexer", "compose_array_instrumentation",
    "compose_namespace_resource", "compose_source_read",
)
IMPORT_ORDER = ("adapters", "source_transition", "seal", "current",
                "published_transition", "published_view")
SEAL_FIELDS = frozenset(("schema", "authority", "support", "view",
                         "source_runtime", "source_runtime_dependencies",
                         "source_dispatcher", "source_manifest", "source_seal"))


class Rejected(ValueError):
    pass


def need(value, message):
    if not value:
        raise Rejected(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def _functions(raw):
    text = raw.decode("utf-8")
    lines = text.splitlines(keepends=True)
    found = {}
    for node in ast.parse(text).body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            need(node.name not in found and not node.decorator_list,
                 "unique undecorated top-level function")
            found[node.name] = (node.lineno, node.end_lineno,
                                "".join(lines[node.lineno - 1:node.end_lineno]).encode())
    return found


LOADER = '''
# Explicit source-derived published-main transport. No activation data fallback.
def _published_parser_modules():
    seal = _PUBLISHED_PARSER_SEAL
    rows = seal['source_runtime_dependencies']
    same([row['module'] for row in rows],
         ['adapters', 'source_transition', 'seal', 'current',
          'published_transition', 'published_view'],
         'closed published source import roster/order')
    same(seal['view'], rows[-1]['artifact'], 'one authenticated view implementation')
    named = [(row['module'], row['artifact']) for row in rows]
    named += [('oxid_published_current', seal['source_runtime']),
              ('oxid_published_parser_support', seal['support'])]
    bodies = []
    for name, row in named:
        verify_map(REPOSITORY, [row])
        path = REPOSITORY / row['path']
        raw = path.read_bytes()
        same({'path': row['path'], 'bytes': len(raw), 'sha256': sha(raw)}, row,
             'exact publication import body')
        bodies.append((name, path, raw))
    # Every dependency is authenticated before executing the first one.
    modules = {}
    for name, path, raw in bodies:
        module = types.ModuleType(name)
        module.__file__ = str(path)
        sys.modules[name] = module
        exec(compile(raw, str(path), 'exec'), module.__dict__)
        modules[name] = module
    return modules['oxid_published_parser_support'], modules['oxid_published_current']


def _published_parser_context():
    support, current = _published_parser_modules()
    # Existing CI loaders do not necessarily register this module in sys.modules.
    # Expose only the six current-module members consumed by parser support.
    api = types.SimpleNamespace(REPOSITORY=REPOSITORY, AUTHORITY_SHA=AUTHORITY_SHA,
                                verify_map=verify_map, regular=regular,
                                read=read, load=load)
    source_rejected = current.adapters.Reject
    try:
        # Preserve the current authority pin-before-preflight/materialization gate.
        support._seal(api, _PUBLISHED_PARSER_SEAL)
        admitted = current.preflight(
            REPOSITORY, REPOSITORY / 'tests/fixtures/typed_project_source_binding',
            _PUBLISHED_PARSER_SEAL['source_seal'])
    except (support.Rejected, source_rejected) as error:
        raise Rejected(str(error)) from error
    return support, admitted, api, source_rejected

'''


REPLACEMENTS = {
    "authority": '''def authority():
    support, admitted, api, source_rejected = _published_parser_context()
    try:
        return support.authority(api, _PUBLISHED_PARSER_SEAL, admitted)
    except (support.Rejected, source_rejected) as error:
        raise Rejected(str(error)) from error
''',
    "phase_previous": '''def phase_previous(a):
    support, admitted, api, source_rejected = _published_parser_context()
    try:
        return support.phase_previous(api, a, _PUBLISHED_PARSER_SEAL, admitted)
    except (support.Rejected, source_rejected) as error:
        raise Rejected(str(error)) from error
''',
    "current_candidate": '''def current_candidate(a):
    support, _ = _published_parser_modules()
    try:
        return support.current_candidate(a)
    except support.Rejected as error:
        raise Rejected(str(error)) from error
''',
    "compose_current_lexer": '''def compose_current_lexer(a, raw):
    support, admitted, api, source_rejected = _published_parser_context()
    try:
        return support.compose(api, a, 'src/frontend/lexer.rs', raw,
                               False, 'lexer', _PUBLISHED_PARSER_SEAL, admitted)
    except (support.Rejected, source_rejected) as error:
        raise Rejected(str(error)) from error
''',
    "compose_array_instrumentation": '''def compose_array_instrumentation(a, name, raw, control=False):
    support, admitted, api, source_rejected = _published_parser_context()
    try:
        return support.compose(api, a, name, raw, control, 'array',
                               _PUBLISHED_PARSER_SEAL, admitted)
    except (support.Rejected, source_rejected) as error:
        raise Rejected(str(error)) from error
''',
    "compose_namespace_resource": '''def compose_namespace_resource(a, raw):
    support, admitted, api, source_rejected = _published_parser_context()
    try:
        return support.compose(api, a, 'src/frontend/declaration_index/resource.rs',
                               raw, False, 'namespace', _PUBLISHED_PARSER_SEAL, admitted)
    except (support.Rejected, source_rejected) as error:
        raise Rejected(str(error)) from error
''',
    "compose_source_read": '''def compose_source_read(a, raw):
    support, admitted, api, source_rejected = _published_parser_context()
    try:
        return support.compose(api, a, 'src/frontend/source.rs', raw,
                               False, 'source', _PUBLISHED_PARSER_SEAL, admitted)
    except (support.Rejected, source_rejected) as error:
        raise Rejected(str(error)) from error
''',
}


def derive(original, seal):
    need(type(original) is bytes and sha(original) == PREDECESSOR_SHA,
         "exact independently pinned complete predecessor body")
    need(type(seal) is dict and set(seal) == SEAL_FIELDS
         and seal.get("schema") == "oxid-unit4-published-main-parser-seal-v1",
         "reviewed fixed parser seal required; draft has no default")
    pin = seal["authority"]["sha256"]
    need(type(pin) is str and len(pin) == 64
         and all(c in "0123456789abcdef" for c in pin)
         and pin != "d8a02a6e970417b2108fcb8ff5e9eb2b32b068d562a7042d73eabaf504dae35c",
         "distinct current authority digest")
    need([r["module"] for r in seal["source_runtime_dependencies"]] == list(IMPORT_ORDER),
         "finite source dependency closure")
    need(seal["view"] == seal["source_runtime_dependencies"][-1]["artifact"],
         "exact shared predecessor view")
    old_functions = _functions(original)
    need(set(FUNCTIONS) <= set(old_functions), "all seven exact callable seams")
    lines = original.decode().splitlines(keepends=True)
    for name in sorted(FUNCTIONS, key=lambda name: old_functions[name][0], reverse=True):
        first, last, _ = old_functions[name]
        lines[first - 1:last] = [REPLACEMENTS[name]]
    result = "".join(lines)
    before = "AUTHORITY_SHA = 'd8a02a6e970417b2108fcb8ff5e9eb2b32b068d562a7042d73eabaf504dae35c'"
    need(result.count(before) == 1, "exact authority constant seam")
    result = result.replace(before, "AUTHORITY_SHA = " + repr(pin), 1)
    anchor = '\n\nif __name__ == "__main__":\n'
    need(result.count(anchor) == 1 and "_PUBLISHED_PARSER_SEAL" not in original.decode(),
         "fresh explicit pinned-loader insertion")
    # JSON round-trip excludes executable objects and normalizes only seal data.
    literal = repr(json.loads(json.dumps(seal, sort_keys=True, allow_nan=False)))
    result = result.replace(anchor, "\n\n_PUBLISHED_PARSER_SEAL = " + literal + "\n" + LOADER + anchor, 1)
    derived = result.encode()
    new_functions = _functions(derived)
    extra = {"_published_parser_modules", "_published_parser_context"}
    need(set(new_functions) == set(old_functions) | extra, "closed derived function roster")
    preserved = []
    for name, (_, _, body) in old_functions.items():
        if name not in FUNCTIONS:
            need(new_functions[name][2] == body, "changed preserved function: " + name)
            preserved.append({"name": name, "bytes": len(body), "sha256": sha(body)})
    receipt = {
        "schema": "oxid-unit4-published-parser-source-derivation-v1",
        "predecessor": {"bytes": len(original), "sha256": sha(original)},
        "derived": {"bytes": len(derived), "sha256": sha(derived)},
        "changed_functions": [{"name": name,
                               "before_sha256": sha(old_functions[name][2]),
                               "after_sha256": sha(new_functions[name][2])} for name in FUNCTIONS],
        "added_functions": sorted(extra), "preserved_functions": preserved,
        "changed_constants": ["AUTHORITY_SHA"], "added_constants": ["_PUBLISHED_PARSER_SEAL"],
        "source_only": True, "execution_qualified": False,
    }
    return derived, receipt
