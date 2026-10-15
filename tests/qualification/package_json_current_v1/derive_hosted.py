"""Pure source derivation of the hosted predecessor-control / current-export bridge.

All old test bodies, preparation commands, failures, evidence caps, exclusive
writes and full-run predicates survive. The new explicit seams separate the
actual successor checkout/receipt from the accepted formatter test fixture.
"""
import ast
import hashlib
import json

import generation as g

ORIGINAL = (180318, '312e99afb23cddf1af59f3f101b98ef21223308a36dccbfd39dc52de79145d39')
NAME = g.ROOT + 'hosted_controls.py'
FACADE = 'scripts/test_published_formatter_source.py'


def replace(raw, before, after, count=1):
    g.need(raw.count(before) == count, 'exact hosted source seam: ' + before[:60])
    return raw.replace(before, after)


BOOTSTRAP = '''
# This configuration is independent of the retained v1 test-fixture roster.
ACTUAL_REPOSITORY = Path(__file__).absolute().parents[3]


def bootstrap_successor(repo):
    row = _SUCCESSOR['dispatcher']
    raw = read_regular(repo / row['path'])
    need(descriptor(row['path'], raw) == row, 'independently pinned successor dispatcher')
    runtime = load_buffer('package_json_hosted_current', repo / row['path'], raw)
    need(runtime._SOURCE_SEAL == _SUCCESSOR['source_seal'], 'successor source seal')
    captured = runtime.preflight(repo)
    need(captured['current']['reviewed_source_head'] == '858c70d862587b6a8b5d59a70afe4b8334b1f150'
         and captured['current']['source_only_tree'] == 'c44b7dfb2938dc510016ff4939104a2236ba2e9c',
         'actual published successor source association')
    return runtime, captured


def verify_successor_receipt(value, expected):
    # Canonical bytes distinguish bool/int and retain every nested predecessor.
    need(type(value) is dict and type(expected) is dict, 'complete source receipt dictionaries')
    for key, body in expected.items():
        need(key in value and encoded(value[key]) == encoded(body),
             'current execution outer provenance differs: ' + key)

'''


def derive(original, dispatcher, source_seal):
    g.pin(original, ORIGINAL, 'unchanged complete hosted v1 controller')
    text = original.decode()
    text = replace(text, '    def __init__(self, repo, configuration, bodies):',
                   '    def __init__(self, repo, configuration, bodies, current_runtime, current_capture):')
    before = '''            proof = value["published_source_provenance"]
            need(proof["source_checkpoint"] == {"head": SOURCE_HEAD, "tree": SOURCE_TREE, "parent": M_HEAD}
                 and proof["published_main"]["head"] == M_HEAD
                 and proof["retained_historical_source"]["head"] == HISTORICAL_HEAD
                 and proof["source_manifest"] == configuration["source_seal"]["source_manifest"]
                 and proof["package_manifest"] == descriptor("package-manifest.json", bodies[PACKAGE + "/package-manifest.json"]),
                 "old execution outer provenance differs")'''
    after = '''            expected = current_runtime._modules['package_current'].outer_receipt(current_capture)
            verify_successor_receipt(value, expected)'''
    text = replace(text, before, after)
    text = replace(text, '        self.attempted, self.written_bytes, self.written = False, 0, []',
                   '''        # Verify the actual new full Unit2 execution with the unchanged original
        # verifier and new ordinary-source checks before exporting any evidence.
        current_runtime.verify_unit2_result(self.root, current_capture, seam, False)
        self.attempted, self.written_bytes, self.written = False, 0, []''')
    text = replace(text, 'return "source-controls-v1-" + name.replace("/", "--")',
                   'return "source-controls-package-json-v1-" + name.replace("/", "--")')
    text = replace(text, '"schema": "oxid-source-controls-flat-evidence-v1"',
                   '"schema": "oxid-package-json-source-controls-flat-evidence-v1"')
    text = replace(text, '            self.repo = Path(__file__).absolute().parents[1]',
                   '''            self.actual_repo = ACTUAL_REPOSITORY
            self.current_runtime, self.current_capture = bootstrap_successor(self.actual_repo)
            view_module = self.current_runtime._modules['predecessor']
            self.accepted_scope = view_module.view(
                self.actual_repo, self.current_capture['package_json_predecessor']['inputs'],
                self.current_capture['package_json_seal']['predecessor_view'])
            self.repo = self.accepted_scope.__enter__()
            self.accepted_entered = True''')
    text = replace(text, '            self.exporter = ExistingUnit2Evidence(self.repo, _CONFIG, self.bodies)',
                   '''            actual_bodies = {
                PACKAGE + '/run.py': self.current_capture['package_bytes']['run.py'],
                PACKAGE + '/package-manifest.json': self.current_capture['package_manifest']}
            self.exporter = ExistingUnit2Evidence(
                self.actual_repo, {'source_seal': self.current_capture['package_json_seal']},
                actual_bodies, self.current_runtime, self.current_capture)''')
    text = replace(text, 'self.exporter.root.parent / "oxid-published-source-ci-v1"',
                   'self.exporter.root.parent / "oxid-package-json-source-ci-v1"')
    text = replace(text, '            checkout = checkout_identity(self.repo)',
                   '''            checkout = checkout_identity(self.actual_repo)
            checkout['controls_stage'] = 'accepted-formatter-predecessor'
            checkout['actual_current_source'] = self.current_capture['current']['package_json_source']''')
    text = replace(text, '            write_new(self.evidence / "complete.json", encoded({"status": "passed-source-controls-only",',
                   '''            # Verify and close the complete outer fixture BEFORE committing
            # a successful completion/index. Preservation failure exports failure.
            self.modules = None
            self.fixture = None
            self.accepted_entered = False
            self.accepted_scope.__exit__(None, None, None)
            write_new(self.evidence / "complete.json", encoded({"status": "passed-source-controls-only",''')
    text = replace(text, '"compiler_executions": 0, "execution_qualified": False, "checkout": checkout}), 16384)',
                   '"compiler_executions": 0, "execution_qualified": False, "checkout": checkout, '
                   '"accepted_fixture_view": {"files":2724,"logical_bytes":78507317,'
                   '"scope":"entire predecessor control batch; closed before terminal export",'
                   '"separate_from_nested_view_effect_audit":True}}), 16384)')
    # On failures before the terminal boundary, close the same accepted scope.

    # Cleanup failures remain original errors or explicit groups, never success.
    before = '''        if failure is not None:
            result.startTest(case)'''
    after = '''        if getattr(self, 'accepted_entered', False):
            self.modules = None
            self.fixture = None
            try:
                self.accepted_scope.__exit__(None, None, None)
            except BaseException as cleanup:
                self.failed = True
                if failure is None:
                    failure = sys.exc_info()
                else:
                    combined = BaseExceptionGroup('controls and accepted-view closure failed', [failure[1], cleanup])
                    failure = (type(combined), combined, combined.__traceback__)
            finally:
                self.accepted_entered = False
        if failure is not None:
            result.startTest(case)'''
    text = replace(text, before, after)
    # Helpers are defined before unittest invokes the unchanged load_tests API.
    text += '\n_SUCCESSOR = ' + repr({'dispatcher': dispatcher, 'source_seal': source_seal}) + '\n' + BOOTSTRAP
    ast.parse(text)
    result = text.encode()
    receipt = {'schema': 'oxid-package-json-hosted-control-derivation-v1',
               'predecessor': g.identity(FACADE, original), 'successor': g.identity(NAME, result),
               'preserved_test_roster': {'unit':37,'integration':15,'view':14,'ci':20,'public':6},
               'accepted_fixture_view': {'files':2724,'logical_bytes':78507317,'closes_before_terminal_export':True},
               'fresh_preparations':3, 'new_export_namespace':'source-controls-package-json-v1-',
               'source_only':True,'execution_qualified':False,
               'full_execution_receipt_required_for_hosted_route':True}
    return result, receipt


def facade(body):
    row = g.identity(NAME, body)
    return ('''"""Pinned hosted successor entry; retained v1 controls run in their genuine view."""
from pathlib import Path
import hashlib
import os
import stat
import sys
import types
sys.dont_write_bytecode = True
BODY = ''' + repr(row) + '''


def load_tests(loader, tests, pattern):
    root = Path(__file__).absolute().parents[1]
    path = root / BODY['path']
    for parent in (path, *path.parents):
        if parent.is_symlink():
            raise ValueError('aliased hosted successor entry')
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or before.st_mode & 0o111:
        raise ValueError('unsafe hosted successor entry')
    fields = ('st_dev','st_ino','st_mode','st_nlink','st_size','st_mtime_ns','st_ctime_ns')
    stamp = lambda value: tuple(getattr(value,field) for field in fields)
    fd = os.open(path,os.O_RDONLY | getattr(os,'O_NOFOLLOW',0) | getattr(os,'O_NONBLOCK',0) | getattr(os,'O_BINARY',0))
    try:
        opened = os.fstat(fd)
        left, right = stamp(before), stamp(opened)
        matches = left == right
        if os.name == 'nt' and hasattr(before,'st_birthtime_ns') and hasattr(opened,'st_birthtime_ns'):
            matches = left[:-1] == right[:-1] and before.st_birthtime_ns == opened.st_birthtime_ns
        if not matches:
            raise ValueError('hosted successor entry changed before read')
        with os.fdopen(fd,'rb',closefd=False) as stream:
            raw = stream.read(BODY['bytes']+1)
        if stamp(os.fstat(fd)) != stamp(opened):
            raise ValueError('hosted successor entry changed during read')
    finally:
        os.close(fd)
    if stamp(path.lstat()) != stamp(before) or len(raw)!=BODY['bytes'] or hashlib.sha256(raw).hexdigest()!=BODY['sha256']:
        raise ValueError('unapproved complete hosted successor body')
    module = types.ModuleType('package_json_hosted_controls')
    module.__file__ = str(path)
    exec(compile(raw,str(path),'exec'),module.__dict__)
    return module.load_tests(loader,tests,pattern)
''').encode()
