"""Inactive, pure, finite source recipe for published parser controls.

No inputs are discovered, no file is read/written, and no module/test is loaded
by derive(). Its caller supplies the three exact predecessor sources, final
independently authenticated descriptors, and the pinned current-fixture map.
Returned prospective sources/receipt are not execution or qualification evidence.
"""
import ast
import copy
import hashlib
import json
from pathlib import PurePosixPath
import textwrap

CURRENT_SHA = "0271a71075e3561ff84bbe5820d5560f82c57e4aa82a0d7812411348abb9425b"
LEXER_SHA = "f4fada86d3088cdf6fb8cb7fd54f0fd36cbe54ed71f58536cc0bcffd155056a9"
PORTABLE_SHA = "3366c49f267ab98bd792a1cd1b01ebbaaca548bc19685d841eb688cf39f99162"
SUPPORT_SHA = "415cb1b9391cd330388dd4f80c62f4057209d75ec710348d726c572bc795b424"
DERIVATION_SHA = "2d5427b6f3bd4dfa0cfec6ad7723725c0122a70ccc033a34c41d728470d76bf6"
DIRECTORY = "tests/qualification/unit4_parser_current/"
SUPPORT = "tests/qualification/published_formatter_current/"
RETAINED = SUPPORT + "predecessor-v1/"
FIXTURE_NAME = SUPPORT + "parser-control-fixture-v1.json"
FIXTURE_EXCLUDED = frozenset((FIXTURE_NAME,
    DIRECTORY + "test_current.py", DIRECTORY + "test_lexer_reservation.py",
    DIRECTORY + "test_current_successor.py", DIRECTORY + "test_lexer_reservation_successor.py",
    RETAINED + DIRECTORY + "test_current.py", RETAINED + DIRECTORY + "test_lexer_reservation.py"))
PIN_FIELDS = frozenset(("schema", "authority", "portable", "support", "derivation", "predecessor_view",
                       "retained_portable", "retained_current_tests", "retained_lexer_tests",
                       "byte_storage_portable", "lexer_prephase_portable", "fixture"))
ARTIFACT_FIELDS = PIN_FIELDS - {"schema"}
FIXTURE_FIELDS = frozenset(("schema", "files", "file_count", "directories", "logical_bytes",
                           "file_rounded_bytes", "allocated_bytes", "other_live_allocated_bytes",
                           "maximum_lifetime_bytes", "predecessor_view_sha256"))

HISTORICAL_CLASSES = frozenset((
    "DivisionCompositionControls", "SlicesCompositionControls", "RecordCompositionControls",
    "UnaryCompositionControls", "ProjectedCompositionControls", "StdinCompositionControls",
    "HirProducerTransitionControls", "LexicalProviderTransitionControls",
    "ProducerDiagnosticTransitionControls", "FrontendV2TransitionControls",
    "PackageIntegrityTransitionControls", "CachePreservationTransitionControls",
    "CacheAdmissionTransitionControls", "ByteStorageTransitionControls"))
HISTORICAL_METHODS = {
    "CurrentAuthorityControls": (
        "test_native_storage_transition_rejects_parser_instrumentation_overlap",
        "test_native_inventory_transition_rejects_parser_instrumentation_overlap",
        "test_native_inventory_retains_separate_storage_source_authority"),
    "SourceReadCompositionControls": (
        "test_every_other_instrumentation_overlap_remains_rejected",
        "test_uninstrumented_or_stale_composed_map_rejects"),
    "ArrayCompositionControls": ("test_plain_or_stale_array_derived_maps_are_never_accepted",),
    "StdoutCompositionControls": (
        "test_exact_stdout_overlap_restores_retained_stdin_before_enum",
        "test_changed_and_coherently_rehashed_stdout_source_rejects",
        "test_double_inverse_and_wrong_paths_reject",
        "test_transition_predecessor_and_runner_drift_reject_before_transform",
        "test_forged_stdin_predecessor_identity_rejects",
        "test_missing_duplicate_and_wrong_patch_section_reject"),
    "EnumCompositionControls": (
        "test_exact_enum_overlap_roster_and_projected_predecessors",
        "test_changed_and_coherently_rehashed_enum_source_rejects",
        "test_enum_transition_predecessor_and_runner_identity_reject_before_transform",
        "test_unknown_enum_instrumentation_path_rejects"),
    "HirImportOverlapControls": (
        "test_changed_and_coherently_rehashed_tail_rejects", "test_double_inverse_and_wrong_path_reject",
        "test_predecessor_pin_cannot_be_rebased", "test_unexpected_observer_and_control_overlap_reject",
        "test_transition_predecessor_and_runner_drift_reject"),
    "U8TransitionControls": (
        "test_cross_host_inverse_preserves_complete_initial_u8_source",
        "test_complete_exact_source_inverse", "test_missing_extra_modified_source_rejects",
        "test_every_u8_binding_is_pinned", "test_unrelated_parser_authority_cannot_drift",
        "test_u8_overlap_rosters_cannot_expand", "test_prior_policy_and_predicates_are_immutable"),
    "CurrentLexer": (
        "test_complete_outer_inverse_and_wrong_stage",
        "test_coherent_manifest_row_rejects_before_preflight_or_materialization",
        "test_stale_and_uninstrumented_current_hook_maps_reject"),
}
# The C versions keep testing the current public boundary. Exact old inner-stage
# assertions are additionally retained under explicitly historical test names.
HISTORICAL_CLONES = {
    "CurrentAuthorityControls": (
        "test_historical_and_current_authorities_remain_distinct",
        "test_coherently_rewritten_authority_rejects_at_trusted_pin",
        "test_duplicate_current_member_rejects", "test_reordered_transition_rejects",
        "test_deleted_historical_compiler_member_rejects_even_with_same_count",
        "test_every_added_and_changed_source_identity_is_bound"),
    "SourceReadCompositionControls": (
        "test_changed_current_source_rejects_before_composition",
        "test_coherently_rehashed_source_cannot_change_historical_body",
        "test_changed_formatter_patch_rejects_before_composition"),
    "ArrayCompositionControls": (
        "test_every_composition_rejects_changed_source",
        "test_unknown_path_and_unapproved_control_overlap_reject",
        "test_coherent_source_rehash_cannot_change_historical_array_body",
        "test_changed_inverse_patch_and_runner_reject_before_transform"),
    "EnumCompositionControls": ("test_namespace_composition_preserves_both_hooks_and_current_body",),
    "CurrentLexer": ("test_successful_append_hook_and_exact_inverse",),
}
LOCAL_FIELDS = frozenset(("a", "active", "inputs", "raw", "name", "outer", "helper", "hooks"))


class Rejected(ValueError):
    pass


def need(value, message):
    if not value:
        raise Rejected(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def encoded(value):
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def descriptor(raw, row):
    need(type(row) is dict and set(row) == {"path", "bytes", "sha256"}, "closed descriptor")
    name = row["path"]
    need(type(name) is str and name not in ("", ".", "..") and "\\" not in name and not PurePosixPath(name).is_absolute()
         and str(PurePosixPath(name)) == name and ".." not in PurePosixPath(name).parts,
         "safe finite descriptor path")
    need(type(row["bytes"]) is int and row["bytes"] >= 0 and type(row["sha256"]) is str
         and len(row["sha256"]) == 64 and all(c in "0123456789abcdef" for c in row["sha256"]),
         "typed independently reviewed descriptor")
    if raw is not None:
        need(type(raw) is bytes and len(raw) == row["bytes"] and sha(raw) == row["sha256"],
             "exact independently pinned artifact")


def _fixture(raw, fixed):
    descriptor(raw, fixed["fixture"])
    # Match the runtime p.load rejection of duplicate keys at every nesting
    # depth and of nonfinite JSON; a pin alone does not make ambiguous JSON valid.
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, "duplicate JSON key: " + key)
            result[key] = value
        return result
    value = json.loads(raw, object_pairs_hook=pairs,
                       parse_constant=lambda x: (_ for _ in ()).throw(Rejected("nonfinite JSON")))
    need(type(value) is dict and set(value) == FIXTURE_FIELDS
         and value["schema"] == "oxid-published-parser-control-fixture-v1", "closed fixture map")
    rows = value["files"]
    need(type(rows) is list and len(rows) == 700, "exact complete 700-file current fixture map")
    for row in rows:
        descriptor(None, row)
    names = [row["path"] for row in rows]
    need(names == sorted(set(names)), "exact sorted fixture file membership")
    need(not set(names).intersection(FIXTURE_EXCLUDED), "acyclic fixture excludes its map and test consumers")
    need(all(not {".git", "target", "__pycache__"}.intersection(PurePosixPath(name).parts)
             for name in names), "source-only current fixture excludes Git/build/cache trees")
    directories = sorted({str(parent) for name in names for parent in PurePosixPath(name).parents
                          if str(parent) != "."})
    need(len(directories) == 119, "exact 119-directory current fixture map")
    numbers = ("file_count", "logical_bytes", "file_rounded_bytes", "allocated_bytes",
               "other_live_allocated_bytes", "maximum_lifetime_bytes")
    need(all(type(value[key]) is int and value[key] > 0 for key in numbers), "typed fixture accounting")
    logical = sum(row["bytes"] for row in rows)
    rounded = sum(((row["bytes"] + 4095) // 4096) * 4096 for row in rows)
    need(value["file_count"] == len(rows) and value["directories"] == directories
         and value["logical_bytes"] == logical and value["file_rounded_bytes"] == rounded
         and value["allocated_bytes"] == rounded + (len(directories) + 1) * 4096,
         "exact fixture file/directory accounting")
    need(value["other_live_allocated_bytes"] >= 53827644
         and value["maximum_lifetime_bytes"] ==
             value["allocated_bytes"] + value["other_live_allocated_bytes"]
         and value["maximum_lifetime_bytes"] <= 128 * 1024 * 1024,
         "independently reviewed full live fixture/view bound")
    need(value["predecessor_view_sha256"] == fixed["predecessor_view"]["sha256"],
         "fixed exact M681 association")
    by_name = {r["path"]: r for r in rows}
    for key in ("authority", "portable", "support", "predecessor_view"):
        need(by_name.get(fixed[key]["path"]) == fixed[key], "fixture includes current parser entrypoints")
    return value


def _pins(fixed, current, lexer, portable, fixture_raw):
    need(type(fixed) is dict and set(fixed) == PIN_FIELDS
         and fixed["schema"] == "oxid-published-parser-control-pins-v1", "fixed final control pins required")
    for key in ARTIFACT_FIELDS:
        descriptor(None, fixed[key])
        need(fixed[key]["bytes"] > 0, "nonempty final control artifact")
    paths = {
        "authority": DIRECTORY + "authority.json", "portable": DIRECTORY + "portable.py",
        "support": SUPPORT + "published_parser.py", "derivation": SUPPORT + "derive_parser_successor.py",
        "predecessor_view": SUPPORT + "predecessor-view-v1.json",
        "retained_portable": RETAINED + DIRECTORY + "portable.py",
        "retained_current_tests": RETAINED + DIRECTORY + "test_current.py",
        "retained_lexer_tests": RETAINED + DIRECTORY + "test_lexer_reservation.py",
        "byte_storage_portable": DIRECTORY + "byte_storage_portable_v1.py",
        "lexer_prephase_portable":
            "scripts/predecessors/lexer_reservation_source_v1/" + DIRECTORY + "portable.py",
        "fixture": FIXTURE_NAME,
    }
    for key, path in paths.items():
        need(fixed[key]["path"] == path, "fixed control dependency path: " + key)
    for key, raw, expected in (("retained_current_tests", current, CURRENT_SHA),
                               ("retained_lexer_tests", lexer, LEXER_SHA),
                               ("retained_portable", portable, PORTABLE_SHA)):
        descriptor(raw, fixed[key])
        need(sha(raw) == expected, "immutable complete predecessor source")
    for key, expected in (
            ("support", SUPPORT_SHA), ("derivation", DERIVATION_SHA),
            ("byte_storage_portable", "2efcbd2f11ee0b9fee041c19b362459b15f7d843b16bc55f8743a938c58210cd"),
            ("lexer_prephase_portable", "86577f7fb47c9514c55adb12900953ffd6d329d2666228aae40ec3d9cd8f1de2")):
        need(fixed[key]["sha256"] == expected, "independently accepted dependency: " + key)
    need(fixed["portable"]["sha256"] != PORTABLE_SHA
         and fixed["authority"]["sha256"] !=
             "d8a02a6e970417b2108fcb8ff5e9eb2b32b068d562a7042d73eabaf504dae35c",
         "distinct final C parser pins required")
    return _fixture(fixture_raw, fixed)


def _inventory(text):
    tree = ast.parse(text)
    classes, methods = {}, {}
    for node in tree.body:
        if isinstance(node, ast.ClassDef):
            need(node.name not in classes, "unique fixed test class")
            classes[node.name] = node
            for method in node.body:
                if isinstance(method, ast.FunctionDef):
                    key = (node.name, method.name)
                    need(key not in methods, "unique fixed test method")
                    methods[key] = method
    return classes, methods


def _span(text, node, decorators=True):
    lines = text.splitlines(keepends=True)
    first = min([node.lineno] + ([d.lineno for d in node.decorator_list]
                                if decorators and hasattr(node, "decorator_list") else []))
    return first - 1, node.end_lineno, "".join(lines[first - 1:node.end_lineno])


def _body(text, node):
    _, _, complete = _span(text, node, False)
    lines = complete.splitlines(keepends=True)
    need(lines[0].rstrip().endswith(":"), "fixed single-line method signature")
    return textwrap.dedent("".join(lines[1:]))


def _method(name, body, args="self"):
    return "    def " + name + "(" + args + "):\n" + textwrap.indent(body.rstrip() + "\n", "        ")


def _historical_name(name):
    need(name.startswith("test_"), "historical case has explicit test name")
    return "test_historical_" + name[5:]


def _localize(body):
    """Only p and the eight explicitly listed fixture attributes are renamed.

    AST source spans avoid modifying strings/comments or mutating module globals.
    This is a generation-time operation over independently pinned source only.
    """
    need(body.isascii(), "fixed predecessor controls are ASCII")
    tree = ast.parse(body)
    lines = body.splitlines(keepends=True)
    offsets, total = [], 0
    for line in lines:
        offsets.append(total)
        total += len(line)
    edits = []
    for node in ast.walk(tree):
        replacement = None
        if isinstance(node, ast.Name) and node.id == "p":
            replacement = "old"
        elif (isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name)
              and node.value.id in ("self", "cls") and node.attr in LOCAL_FIELDS):
            replacement = "historical_" + node.attr
        if replacement is not None:
            edits.append((offsets[node.lineno - 1] + node.col_offset,
                          offsets[node.end_lineno - 1] + node.end_col_offset, replacement))
    for first, last, replacement in sorted(edits, reverse=True):
        body = body[:first] + replacement + body[last:]
    body = body.replace("self.mutated_current_rejects(",
                        "_historical_mutated_current_rejects(self, old, ")
    return body


def _assertions(text):
    tree = ast.parse(text)
    return sum(isinstance(n, ast.Assert) or
               (isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)
                and isinstance(n.func.value, ast.Name) and n.func.value.id == "self"
                and n.func.attr.startswith("assert")) for n in ast.walk(tree))


def _historical_method(text, cls, node, methods, name=None):
    setup = next((methods[(cls, candidate)] for candidate in ("setUp", "setUpClass")
                  if (cls, candidate) in methods), None)
    local_setup = _localize(_body(text, setup)) if setup is not None else ""
    local_body = _localize(_body(text, node))
    if "stage_lexer_dispatcher_dependencies(" in local_body:
        local_body = ("def stage_lexer_dispatcher_dependencies(root, active):\n"
                      "    return _historical_stage_dependencies(old, root, active)\n" + local_body)
    body = ("with _historical_parser() as (old, historical_outer, historical_a):\n" +
            textwrap.indent(local_setup + local_body, "    "))
    rendered = _method(name or _historical_name(node.name), body)
    need(textwrap.indent(_localize(_body(text, node)).rstrip(), "            ") in rendered,
         "complete historical assertion body retained")
    return rendered


def _once(source, before, after):
    need(source.count(before) == 1, "exact reviewed current-test edit seam: " + before[:80])
    return source.replace(before, after, 1)


# Emitted only into the two explicit current control entrypoints. These helpers
# neither discover dependencies nor accept a callback/operation selected at runtime.
RUNTIME = r'''
from contextlib import contextmanager as _control_contextmanager
import tempfile as _control_tempfile
from pathlib import PurePosixPath as _ControlPurePath


def _control_read(key):
    p.require(key in _PARSER_CONTROL_PINS and key != 'schema', 'closed control artifact key')
    row = _PARSER_CONTROL_PINS[key]
    p.verify_map(p.REPOSITORY, [row])
    raw = (p.REPOSITORY / row['path']).read_bytes()
    p.same({'path': row['path'], 'bytes': len(raw), 'sha256': p.sha(raw)}, row,
           'exact second-read parser control artifact')
    return raw


def _control_bindings():
    p.same(p._PUBLISHED_PARSER_SEAL['authority'], _PARSER_CONTROL_PINS['authority'],
           'independently pinned final C parser authority')
    p.same(p.AUTHORITY_SHA, _PARSER_CONTROL_PINS['authority']['sha256'], 'current C authority constant')
    p.same(p._PUBLISHED_PARSER_SEAL['support'], _PARSER_CONTROL_PINS['support'], 'accepted parser support')
    p.same(p._PUBLISHED_PARSER_SEAL['source_seal']['predecessor_view'],
           _PARSER_CONTROL_PINS['predecessor_view'], 'independently pinned M681 descriptor')
    p.same(Path(p.__file__).absolute(), p.REPOSITORY / _PARSER_CONTROL_PINS['portable']['path'],
           'actual current control adapter path')
    # Complete old test/portable texts are authenticated outside every M view.
    for key in ('portable', 'retained_portable', 'retained_current_tests', 'retained_lexer_tests',
                'byte_storage_portable', 'lexer_prephase_portable', 'support', 'derivation',
                'predecessor_view', 'fixture'):
        _control_read(key)


@_control_contextmanager
def _historical_parser(expected_prephase=None):
    _control_bindings()
    support, admitted, api, _ = p._published_parser_context()
    # Source preflight has closed before the only M scope below begins.
    with support._retained_scope(api, p._PUBLISHED_PARSER_SEAL, admitted) as (old, view):
        full = old.authority()
        prior = old.phase_previous(full)
        if expected_prephase is not None:
            support.same(expected_prephase,
                support._transport(prior, p._PUBLISHED_PARSER_SEAL, admitted, 'prephase'),
                'historical test uses the matching complete C prephase')
        yield old, full, prior
    # No old module/helper/callback is stored on a test class or returned.


def _historical_source_projection(a, name, raw):
    p.same(name, 'src/frontend/source.rs', 'closed source-read intermediate control')
    with _historical_parser(a) as (old, full, prior):
        old.same(raw, (old.REPOSITORY / name).read_bytes(), 'unchanged current/M source-read input')
        result = old.restore_enum_source(prior, name, raw)
    return result


def _historical_budget_projection(a, name, raw):
    p.same(name, 'src/frontend/project/budget.rs', 'closed budget intermediate control')
    with _historical_parser(a) as (old, full, prior):
        old.same(raw, (old.REPOSITORY / name).read_bytes(), 'unchanged current/M budget input')
        result = old.restore_hir_import_source(prior, name, raw)
    return result


def _historical_lexer_inverse(a, name, derived, raw):
    p.require(name in ('src/frontend/lexer.rs', 'src/frontend/project/budget.rs'),
              'closed lexer/reserve inverse controls')
    with _historical_parser(a) as (old, full, prior):
        old.same(raw, (old.REPOSITORY / name).read_bytes(), 'unchanged current/M lexer input')
        helper = old.lexer_module(prior['current'])
        hooks = helper.adapter(old.lexer_api(), prior['current'])
        expected = (old.compose_current_lexer(prior, raw) if name == 'src/frontend/lexer.rs' else
                    old.compose_array_instrumentation(prior, name, raw))
        old.same(derived, expected, 'current wrapper equals complete historical composer output')
        seams = hooks.TOKEN_SEAMS if name == 'src/frontend/lexer.rs' else hooks.BUDGET_SEAMS
        result = hooks.inverse_exact(derived, seams)
    return result


def _current_fixture_map():
    value = p.load(_control_read('fixture'))
    p.same(value['schema'], 'oxid-published-parser-control-fixture-v1', 'fixed current control fixture')
    p.same(value['predecessor_view_sha256'],
           p._PUBLISHED_PARSER_SEAL['source_seal']['predecessor_view']['sha256'], 'exact M681 association')
    rows = value['files']
    names = [row['path'] for row in rows]
    p.same(names, sorted(set(names)), 'exact current fixture roster')
    directories = sorted({str(parent) for name in names for parent in _ControlPurePath(name).parents
                          if str(parent) != '.'})
    p.same(value['directories'], directories, 'exact current fixture directories')
    p.same(value['file_count'], len(rows), 'exact current fixture count')
    p.same(value['logical_bytes'], sum(row['bytes'] for row in rows), 'exact fixture logical bytes')
    rounded = sum(((row['bytes'] + 4095) // 4096) * 4096 for row in rows)
    p.same(value['file_rounded_bytes'], rounded, 'exact fixture file allocation')
    p.same(value['allocated_bytes'], rounded + (len(directories) + 1) * 4096, 'exact fixture allocation')
    p.same(value['maximum_lifetime_bytes'], value['allocated_bytes'] + value['other_live_allocated_bytes'],
           'exact independently reviewed full lifetime bound')
    p.require(value['maximum_lifetime_bytes'] <= 128 * 1024 * 1024, 'reviewed source-test lifetime ceiling')
    p.verify_map(p.REPOSITORY, rows)
    return value


def _assert_current_capture_rejects(case, transform, historical_label):
    support, admitted, api, _ = p._published_parser_context()
    changed = copy.deepcopy(admitted)
    transform(changed['current']['files'])
    if historical_label == 'duplicate current member':
        expected = 'duplicate map member or invalid byte count'
    elif historical_label in ('unexpected current transition scope',
                              'current transition deletes historical compiler input'):
        expected = 'exact published-main source membership and one-file delta'
    else:
        p.same(historical_label, 'current transition before/after identities', 'closed capture mutation case')
        before = {row['path']: row for row in admitted['current']['files']}
        names = [row['path'] for row in changed['current']['files'] if row != before[row['path']]]
        p.same(len(names), 1, 'exact one-row current capture mutation')
        expected = ('captured source body identity' if names == ['src/main.rs'] else
                    'exact published-main source membership and one-file delta')
    with patch.object(_control_tempfile, 'TemporaryDirectory',
                      side_effect=AssertionError('new view reached after forged capture')):
        with case.assertRaisesRegex(support.Rejected, expected):
            support._source(api, p._PUBLISHED_PARSER_SEAL, changed)


def _assert_current_artifact_rejects(case, a, field, operation):
    # Added C injected-read coverage, not physical file-drift coverage. The
    # complete original physical controls also run under explicit historical names.
    p.require((field, operation) in {
        ('formatter_transition_patch', 'source'),
        ('combined_transition_patch', 'array'), ('source_binding_runner', 'array')},
        'closed current artifact-corruption controls')
    fixture = _current_fixture_map()
    positive = p.authority()
    p.same(p.phase_previous(positive), a, 'genuine positive C authority/prephase before injection')
    relative = a['current'][field]['path']
    p.same(next(row for row in fixture['files'] if row['path'] == relative), a['current'][field],
           'injected artifact belongs to the independently pinned complete C fixture')
    target = p.REPOSITORY / relative
    original_read_bytes = Path.read_bytes
    raw = original_read_bytes(target)
    p.same({'path': relative, 'bytes': len(raw), 'sha256': p.sha(raw)}, a['current'][field],
           'exact second-read current artifact before injection')
    name = 'src/frontend/source.rs' if operation == 'source' else 'src/frontend/ast.rs'
    source = (p.REPOSITORY / name).read_bytes()
    package = 'tests/fixtures/typed_project_source_binding/'
    p.require(relative.startswith(package), 'fixed current source-package artifact')
    expected = ('file bytes differ: ' + relative if field == 'source_binding_runner' else
                'unapproved complete body: ' + relative[len(package):])
    matching_reads = 0
    def read_bytes(path):
        nonlocal matching_reads
        if path == target:
            matching_reads += 1
            return raw + b'\n'
        return original_read_bytes(path)
    read_patch = patch.object(Path, 'read_bytes', read_bytes)
    read_patch.start()
    try:
        with patch.object(_control_tempfile, 'TemporaryDirectory',
                          side_effect=AssertionError('M view reached after injected current artifact')) as materialization:
            with case.assertRaises(p.Rejected) as caught:
                try:
                    if operation == 'source':
                        p.compose_source_read(a, source)
                    else:
                        p.compose_array_instrumentation(a, name, source)
                except p.Rejected as failure:
                    # unittest clears the caught outer traceback on context exit.
                    frames = set()
                    traceback = (failure.__cause__ or failure).__traceback__
                    while traceback is not None:
                        frames.add(traceback.tb_frame.f_code.co_name)
                        traceback = traceback.tb_next
                    raise
            materialization.assert_not_called()
    finally:
        read_patch.stop()
        case.assertIs(Path.read_bytes, original_read_bytes)
    # A no-match interception, missing helper or unrelated rejection cannot pass.
    case.assertEqual(matching_reads, 1)
    case.assertIs(type(caught.exception), p.Rejected)
    case.assertEqual(str(caught.exception), expected)
    if field == 'source_binding_runner':
        case.assertIsNone(caught.exception.__cause__)
        required_frames = {'_seal', 'verify_map'}
    else:
        boundary = caught.exception.__cause__
        current = p.sys.modules['oxid_published_current']
        case.assertIs(type(boundary), current.adapters.Reject)
        case.assertEqual(str(boundary), expected)
        case.assertIsNone(boundary.__cause__)
        required_frames = {'_current_package', '_verify'}
    case.assertTrue(required_frames <= frames)
    case.assertNotIn('predecessor_view', frames)


def _assert_current_parser_pin_rejects(case):
    original_read_bytes = Path.read_bytes
    target = p.REPOSITORY / _PARSER_CONTROL_PINS['authority']['path']
    original = original_read_bytes(target)
    changed = p.load(original)
    changed['source_delta'] = changed['source_delta'][1:]
    changed_raw = (json.dumps(changed, sort_keys=True, indent=2) + '\n').encode()
    def read_bytes(path):
        return changed_raw if path == target else original_read_bytes(path)
    with patch.object(Path, 'read_bytes', read_bytes), \
         patch.object(_control_tempfile, 'TemporaryDirectory',
                      side_effect=AssertionError('source preflight/view reached before parser pin rejection')):
        with case.assertRaisesRegex(p.Rejected, 'file bytes differ: .*unit4_parser_current/authority.json'):
            p.authority()


_control_bindings()
'''

CURRENT_EXTRA = r'''


class PublishedCurrentBridgeControls(unittest.TestCase):
    def test_exact_two_edge_portable_source_derivation(self):
        original = _control_read('retained_portable')
        support, admitted, api, _ = p._published_parser_context()
        recipe = support._module(api, _PARSER_CONTROL_PINS['derivation'], '_published_parser_control_derivation')
        generated, receipt = recipe.derive(original, p._PUBLISHED_PARSER_SEAL)
        self.assertEqual(generated, _control_read('portable'))
        self.assertEqual({row['name'] for row in receipt['changed_functions']}, {
            'authority', 'phase_previous', 'current_candidate', 'compose_current_lexer',
            'compose_array_instrumentation', 'compose_namespace_resource', 'compose_source_read'})
        self.assertEqual(receipt['added_functions'], ['_published_parser_context', '_published_parser_modules'])
        self.assertFalse(receipt['execution_qualified'])

    def test_all_eight_current_composer_roles_equal_the_retained_outputs(self):
        outer = p.authority()
        current = p.phase_previous(outer)
        outputs = {}
        names = ('src/frontend/lexer.rs', 'src/frontend/ast.rs', 'src/frontend/parser.rs',
                 'src/frontend/project/budget.rs', 'src/frontend/declaration_index/resource.rs',
                 'src/frontend/source.rs')
        for name in names:
            raw = (p.REPOSITORY / name).read_bytes()
            controls = (False, True) if name in ('src/frontend/ast.rs', 'src/frontend/parser.rs') else (False,)
            for control in controls:
                if name == 'src/frontend/lexer.rs': body = p.compose_current_lexer(current, raw)
                elif name == 'src/frontend/declaration_index/resource.rs': body = p.compose_namespace_resource(current, raw)
                elif name == 'src/frontend/source.rs': body = p.compose_source_read(current, raw)
                else: body = p.compose_array_instrumentation(current, name, raw, control)
                outputs[(name, control)] = body
        historical_outputs = {}
        with _historical_parser(current) as (old, full, prior):
            for (name, control), current_body in outputs.items():
                raw = (old.REPOSITORY / name).read_bytes()
                self.assertEqual(raw, (p.REPOSITORY / name).read_bytes())
                if name == 'src/frontend/lexer.rs': body = old.compose_current_lexer(prior, raw)
                elif name == 'src/frontend/declaration_index/resource.rs': body = old.compose_namespace_resource(prior, raw)
                elif name == 'src/frontend/source.rs': body = old.compose_source_read(prior, raw)
                else: body = old.compose_array_instrumentation(prior, name, raw, control)
                historical_outputs[(name, control)] = body
        self.assertEqual(len(outputs), 8)
        self.assertEqual(outputs, historical_outputs)
        for (name, control), raw in outputs.items():
            field = 'current_control_derived_files' if control else 'current_derived_files'
            self.assertEqual(next(row for row in current['current'][field] if row['path'] == name),
                             {'path': name, 'bytes': len(raw), 'sha256': p.sha(raw)})

    def test_current_outer_stage_and_complete_maps_reject_stale_inputs(self):
        outer = p.authority()
        previous = p.phase_previous(outer)
        raw = (p.REPOSITORY / 'src/frontend/lexer.rs').read_bytes()
        for wrong in (outer, p.read(p.REPOSITORY / _PARSER_CONTROL_PINS['authority']['path'])):
            with self.assertRaisesRegex(p.Rejected, 'composer requires exact current prephase stage'):
                p.compose_current_lexer(wrong, raw)
        with self.assertRaisesRegex(p.Rejected, 'exact phase input; no double inversion'):
            p.phase_previous(previous)
        for field in ('current_base_files', 'current_derived_files', 'current_control_derived_files'):
            altered = copy.deepcopy(previous)
            next(row for row in altered['current'][field] if row['path'] == 'src/main.rs')['sha256'] = '0' * 64
            with self.subTest(field=field), self.assertRaisesRegex(p.Rejected, 'composer requires exact current prephase stage'):
                p.compose_current_lexer(altered, raw)
'''

LEXER_EXTRA = r'''


class PublishedCurrentLexerControls(unittest.TestCase):
    def test_current_provenance_inverse_is_distinct_from_historical_lexer_inverse(self):
        outer = p.authority()
        support, admitted, api, _ = p._published_parser_context()
        prior = support.provenance_previous(api, outer, p._PUBLISHED_PARSER_SEAL, admitted)
        support.same(prior['current_source'], admitted['published_predecessor']['current'], 'exact M source association')
        support.same(outer['current_source'], admitted['current'], 'exact C source association')
        before, after = admitted['published_predecessor']['inputs'], admitted['inputs']
        self.assertEqual(len(before), 376)
        self.assertEqual(set(before), set(after))
        self.assertEqual([name for name in sorted(before) if before[name] != after[name]], ['src/main.rs'])
        self.assertEqual(len(outer['current']['published_main_parser']['source_provenance']['main_delta']), 1)
        self.assertNotEqual(p.current_candidate(outer)['current_source_manifest_sha256'],
                            prior['current']['current_source_manifest']['sha256'])
        with self.assertRaisesRegex(support.Rejected, 'exact full outer inverse input'):
            support.provenance_previous(api, prior, p._PUBLISHED_PARSER_SEAL, admitted)

    def test_coherent_current_lexer_row_rejects_before_new_view(self):
        support, admitted, api, _ = p._published_parser_context()
        changed = copy.deepcopy(admitted)
        name = 'src/frontend/lexer.rs'
        raw = changed['inputs'][name] + b'\n'
        changed['inputs'][name] = raw
        next(row for row in changed['current']['files'] if row['path'] == name).update(bytes=len(raw), sha256=p.sha(raw))
        with patch.object(_control_tempfile, 'TemporaryDirectory', side_effect=AssertionError('view reached')):
            with self.assertRaisesRegex(support.Rejected, 'exact published-main source membership and one-file delta'):
                support._source(api, p._PUBLISHED_PARSER_SEAL, changed)

    def test_current_lexer_and_budget_stale_maps_reject_at_outer_boundary(self):
        current = p.phase_previous(p.authority())
        replacements = {}
        with _historical_parser(current) as (old, full, prior):
            before = old.lexer_predecessor_active(prior['current'])
            for name in ('src/frontend/lexer.rs', 'src/frontend/project/budget.rs'):
                replacements[name] = next(row for row in before['current_derived_files'] if row['path'] == name)
        for name in replacements:
            for source in ('current_base_files', 'historical_derived'):
                wrong = copy.deepcopy(current)
                replacement = (next(row for row in current['current']['current_base_files'] if row['path'] == name)
                               if source == 'current_base_files' else replacements[name])
                next(row for row in wrong['current']['current_derived_files'] if row['path'] == name).update(replacement)
                raw = (p.REPOSITORY / name).read_bytes()
                with self.subTest(name=name, source=source), self.assertRaisesRegex(p.Rejected, 'composer requires exact current prephase stage'):
                    if name == 'src/frontend/lexer.rs': p.compose_current_lexer(wrong, raw)
                    else: p.compose_array_instrumentation(wrong, name, raw)
'''


def _current_changes(text, methods):
    changed = {}
    def edit(cls, name, before, after):
        key = (cls, name)
        original = changed.get(key, _span(text, methods[key])[2])
        changed[key] = _once(original, before, after)

    edit("CurrentAuthorityControls", "test_historical_and_current_authorities_remain_distinct",
         "self.assertEqual(a['current']['reviewed_source_head'], 'b3abc9f0dda99d6d8fe65d9c3a9ed31dedcbd489')",
         "self.assertEqual(a['current']['reviewed_source_head'], 'fbff527c895e6cd4ec9350bd8b5d460bc20bb80d')")
    edit("CurrentAuthorityControls", "test_historical_and_current_authorities_remain_distinct",
         "self.assertEqual(a['current']['source_only_tree'], '7f5c9aa08c569c4d0b5a27391d8fc68337075d36')",
         "self.assertEqual(a['current']['source_only_tree'], '89e90c3ec83d0712d0037a0407fc6571883c5e9c')")
    edit("CurrentAuthorityControls", "test_copied_algorithms_have_only_reviewed_change_boundaries",
         "new_text = Path(p.__file__).read_text()", "new_text = _control_read('retained_portable').decode()")
    changed[("CurrentAuthorityControls", "test_coherently_rewritten_authority_rejects_at_trusted_pin")] = _method(
        "test_coherently_rewritten_authority_rejects_at_trusted_pin", "_assert_current_parser_pin_rejects(self)\n")
    changed[("CurrentAuthorityControls", "mutated_current_rejects")] = _method(
        "mutated_current_rejects", "_assert_current_capture_rejects(self, transform, message)\n", "self, transform, message")
    edit("SourceReadCompositionControls", "test_exact_composition_preserves_both_hooks_and_only_adds_accessor",
         "p.restore_enum_source(self.a, 'src/frontend/source.rs', self.raw)",
         "_historical_source_projection(self.a, 'src/frontend/source.rs', self.raw)")
    edit("SourceReadCompositionControls", "test_changed_current_source_rejects_before_composition",
         "'composition current source identity'", "'unchanged complete M/current composer input'")
    edit("SourceReadCompositionControls", "test_coherently_rehashed_source_cannot_change_historical_body",
         "'current lexer composition identity'", "'composer requires exact current prephase stage'")
    changed[("SourceReadCompositionControls", "test_changed_formatter_patch_rejects_before_composition")] = _method(
        "test_changed_formatter_patch_rejects_before_composition",
        "_assert_current_artifact_rejects(self, self.a, 'formatter_transition_patch', 'source')\n")
    edit("ArrayCompositionControls", "test_every_composition_rejects_changed_source",
         "'current lexer composition identity' if name.endswith('/budget.rs') else 'composition current array identity'",
         "'unchanged complete M/current composer input'")
    for old in ("'unapproved array instrumentation path'", "'unapproved control composition'"):
        edit("ArrayCompositionControls", "test_unknown_path_and_unapproved_control_overlap_reject",
             old, "'closed published composer target/role'")
    edit("ArrayCompositionControls", "test_coherent_source_rehash_cannot_change_historical_array_body",
         "'current lexer composition identity'", "'composer requires exact current prephase stage'")
    changed[("ArrayCompositionControls", "test_changed_inverse_patch_and_runner_reject_before_transform")] = _method(
        "test_changed_inverse_patch_and_runner_reject_before_transform",
        "for field in ('combined_transition_patch', 'source_binding_runner'):\n"
        "    with self.subTest(field=field):\n"
        "        _assert_current_artifact_rejects(self, self.a, field, 'array')\n")
    edit("EnumCompositionControls", "test_namespace_composition_preserves_both_hooks_and_current_body",
         "'current lexer composition identity'", "'unchanged complete M/current composer input'")
    edit("HirImportOverlapControls", "test_exact_inverse_and_current_reserve_hook",
         "p.restore_hir_import_source(self.a, self.name, self.raw)",
         "_historical_budget_projection(self.a, self.name, self.raw)")
    key = ("CheckoutControls", "historical_ast_from_retained_transitions")
    body = _localize(_body(text, methods[key]))
    body = _once(body, "prephase = old.phase_previous(historical_a)", "prephase = historical_a")
    changed[key] = _method(key[1],
        "current_prephase = p.phase_previous(self.a)\n"
        "with _historical_parser(current_prephase) as (old, historical_outer, historical_a):\n"
        "    with patch.object(old, 'git', side_effect=AssertionError('no historical Git lookup')):\n" +
        textwrap.indent(body, "        "))
    return changed


def _lexer_changes(text, methods):
    changed = {}
    key = ("CurrentLexer", "setUpClass")
    source = _span(text, methods[key])[2]
    source = _once(source, "        cls.helper = p.lexer_module(cls.a['current'])\n", "")
    source = _once(source, "        cls.hooks = cls.helper.adapter(p.lexer_api(), cls.a['current'])\n", "")
    changed[key] = source
    for name, seam in (("test_successful_append_hook_and_exact_inverse", "TOKEN_SEAMS"),
                       ("test_reserve_hook_and_control_are_preserved", "BUDGET_SEAMS")):
        key = ("CurrentLexer", name)
        source = _span(text, methods[key])[2]
        source = _once(source, "self.hooks.inverse_exact(derived, self.hooks." + seam + ")",
                       "_historical_lexer_inverse(self.a, name, derived, raw)")
        if seam == "TOKEN_SEAMS":
            source = _once(source, "self.assertRaises((p.Rejected, self.hooks.Reject))",
                           "self.assertRaises(p.Rejected)")
        changed[key] = source
    return changed


def _replace_source(text, replacements):
    _, methods = _inventory(text)
    lines = text.splitlines(keepends=True)
    edits = []
    for key, new in replacements.items():
        first, last, _ = _span(text, methods[key])
        edits.append((first, last, new))
    for first, last, new in sorted(edits, reverse=True):
        lines[first:last] = [new]
    return "".join(lines)


def _derive_one(raw, fixed, kind):
    text = raw.decode("utf-8")
    classes, methods = _inventory(text)
    need(sum(name.startswith("test_") for cls, name in methods) == (143 if kind == "current" else 6),
         "exact original control-case count")
    replacements = _current_changes(text, methods) if kind == "current" else _lexer_changes(text, methods)
    classification = {}
    for key, node in methods.items():
        cls, name = key
        historical = (name.startswith("test_") and
                      (cls in HISTORICAL_CLASSES or name in HISTORICAL_METHODS.get(cls, ())))
        if historical:
            new_name = name if cls in HISTORICAL_CLASSES else _historical_name(name)
            replacements[key] = _historical_method(text, cls, node, methods, new_name)
            classification[key] = ("historical", ("Historical" + cls if cls in HISTORICAL_CLASSES else cls) + "." + new_name)
        elif name.startswith("test_"):
            classification[key] = ("current", cls + "." + name)
    for cls in HISTORICAL_CLASSES.intersection(classes):
        for name in ("setUp", "setUpClass"):
            key = (cls, name)
            if key in methods:
                need(_assertions(_body(text, methods[key])) == 0, "historical setup contains no lost assertion")
                decorator = "    @classmethod\n" if name == "setUpClass" else ""
                replacements[key] = decorator + _method(name,
                    "# Exact old setup executes locally within each historical test's M scope.\npass\n",
                    "cls" if name == "setUpClass" else "self")
    clones = []
    for cls, names in HISTORICAL_CLONES.items():
        if cls not in classes:
            continue
        for name in names:
            key = (cls, name)
            clone = _historical_method(text, cls, methods[key], methods)
            # Append to the same class by inserting after its final original method.
            tail = max((key for key in methods if key[0] == cls), key=lambda key: methods[key].end_lineno)
            replacements[tail] = replacements.get(tail, _span(text, methods[tail])[2]) + "\n" + clone
            clones.append(cls + "." + _historical_name(name))
    result = _replace_source(text, replacements)
    for cls in sorted(HISTORICAL_CLASSES.intersection(classes)):
        result = _once(result, "class " + cls + "(unittest.TestCase):", "class Historical" + cls + "(unittest.TestCase):")
    helpers = RUNTIME
    if kind == "current":
        top = next(node for node in ast.parse(text).body if isinstance(node, ast.FunctionDef)
                   and node.name == "stage_lexer_dispatcher_dependencies")
        body = _localize(_body(text, top))
        helpers += "\n\ndef _historical_stage_dependencies(old, root, active):\n" + textwrap.indent(body, "    ")
        body = _localize(_body(text, methods[("CurrentAuthorityControls", "mutated_current_rejects")]))
        helpers += "\n\ndef _historical_mutated_current_rejects(self, old, transform, message):\n" + textwrap.indent(body, "    ")
    result = _once(result, "import portable as p\n", "import portable as p\n\n_PARSER_CONTROL_PINS = " +
                   repr(json.loads(encoded(fixed))) + "\n" + helpers + "\n")
    anchor = "if __name__ == '__main__':"
    result = _once(result, anchor, (CURRENT_EXTRA if kind == "current" else LEXER_EXTRA) + "\n\n" + anchor)
    out = result.encode()
    new_classes, new_methods = _inventory(result)
    preserved = []
    changed_methods = []
    for key, node in methods.items():
        if key[0] not in HISTORICAL_CLASSES and key in new_methods:
            old_body = _span(text, node)[2]
            if _span(result, new_methods[key])[2] == old_body:
                preserved.append([".".join(key), sha(old_body.encode())])
                continue
            need(key in replacements, "unreviewed change to complete test body: " + ".".join(key))
        changed_methods.append(".".join(key))
    cases = sorted(cls + "." + name for cls, name in new_methods if name.startswith("test_"))
    added_cases = ([("PublishedCurrentBridgeControls." + name) for name in (
            "test_exact_two_edge_portable_source_derivation",
            "test_all_eight_current_composer_roles_equal_the_retained_outputs",
            "test_current_outer_stage_and_complete_maps_reject_stale_inputs")] if kind == "current" else
        [("PublishedCurrentLexerControls." + name) for name in (
            "test_current_provenance_inverse_is_distinct_from_historical_lexer_inverse",
            "test_coherent_current_lexer_row_rejects_before_new_view",
            "test_current_lexer_and_budget_stale_maps_reject_at_outer_boundary")])
    expected_cases = sorted([entry[1] for entry in classification.values()] + clones + added_cases)
    need(cases == expected_cases, "closed successor test-case roster; no omitted or hidden cases")
    # Each complete original name appears once. Indexing avoids an otherwise
    # >45 KiB minimum receipt without dropping a route, method or body digest.
    method_names = sorted(".".join(key) for key in methods)
    method_index = {name: index for index, name in enumerate(method_names)}
    routes = []
    for key in sorted(classification):
        route, target = classification[key]
        cls, name = key
        rule = ("identity" if route == "current" else
                "class_prefix" if cls in HISTORICAL_CLASSES else "method_prefix")
        reconstructed = (".".join(key) if rule == "identity" else
                         "Historical" + cls + "." + name if rule == "class_prefix" else
                         cls + ".test_historical_" + name[len("test_"):])
        need(reconstructed == target, "lossless compact exact case route")
        routes.append([method_index[".".join(key)], route, rule])
    receipt = {"predecessor_sha256": sha(raw), "derived_sha256": sha(out), "derived_bytes": len(out),
               "original_cases": len(classification), "derived_cases": len(cases),
               "current_original_cases": sum(route == "current" for route, target in classification.values()),
               "historical_original_cases": sum(route == "historical" for route, target in classification.values()),
               "method_names": method_names,
               "case_name_rules": {"identity": ["", ""], "class_prefix": ["Historical", ""],
                                   "method_prefix": ["", "historical_"]},
               "case_routes": routes, "additional_historical_cases": clones,
               "additional_current_cases": added_cases, "derived_case_roster_sha256": sha(encoded(cases)),
               "changed_original_methods": [method_index[name] for name in sorted(changed_methods)],
               "preserved_complete_bodies": [[method_index[name], digest] for name, digest in preserved],
               "original_assertion_sites": _assertions(text), "derived_assertion_sites": _assertions(result),
               "source_only": True, "execution_qualified": False}
    need(len(out) <= (262144 if kind == "current" else 131072), "reserved prospective source ceiling")
    return out, receipt


def derive(current_source, lexer_source, portable_source, fixed, fixture_manifest):
    """Return two prospective source bodies and a compact source-only receipt.

    fixed and fixture_manifest must be independently reviewed final data. There
    are deliberately no defaults, fake pins, environment inputs or output writes.
    """
    _pins(fixed, current_source, lexer_source, portable_source, fixture_manifest)
    current, current_receipt = _derive_one(current_source, fixed, "current")
    lexer, lexer_receipt = _derive_one(lexer_source, fixed, "lexer")
    receipt = {"schema": "oxid-published-parser-control-transport-v1", "current": current_receipt,
               "lexer": lexer_receipt, "fixed_descriptor_sha256": sha(encoded(fixed)),
               "fixture_manifest_sha256": sha(fixture_manifest), "production_changed_functions": 7,
               "production_api_members": 6, "predecessor_view_files": 681,
               "source_only": True, "execution_qualified": False,
               "not_supplied": ["effects-audited execution", "genuine historical Git preparation",
                                "compiler/native runtime gates", "four-host exact-head CI"]}
    receipt_raw = encoded(receipt)
    need(len(receipt_raw) <= 32768, "reserved compact receipt ceiling")
    return {"test_current_successor.py": current, "test_lexer_reservation_successor.py": lexer,
            "parser-control-transport-receipt-v1.json": receipt_raw}
