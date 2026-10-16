"""Review-time mechanical activation. Never regenerates source manifests/checkpoints."""
import copy
import json
from pathlib import Path
import subprocess
import sys
sys.dont_write_bytecode = True
import scheduling_derivation as d

REPO = Path(__file__).resolve().parents[3]
PACKAGE = 'tests/qualification/package_json_current_v1/'
HERE = 'tests/qualification/unit4_ci/'
sys.path.insert(0, str(REPO / PACKAGE))
import ci
import generation as g
import derive_consumers as consumers


def ensure_base(repo=REPO):
    """Fetch only the fixed predecessor, without expanding checkout history."""
    subprocess.run(['git', 'fetch', '--no-tags', '--depth=1', 'origin', d.BASE],
                   cwd=repo, check=True)
    tree = subprocess.check_output(['git', 'rev-parse', d.BASE + '^{tree}'], cwd=repo).decode().strip()
    g.need(tree == d.BASE_TREE, 'unreviewed scheduling predecessor tree')


def base(path):
    return subprocess.check_output(['git', 'show', d.BASE + ':' + path], cwd=REPO)


def generate():
    outputs = {HERE + 'integration-roster-v1.json': (REPO/HERE/'integration-roster-v1.json').read_bytes()}
    workflow = d.workflow(base('.github/workflows/ci.yml'))
    outputs['.github/workflows/ci.yml'] = workflow
    plan = ci.derive((REPO/'tests/qualification/published_formatter_current/published-direct-consumer-dispositions-v1.json').read_bytes(),
                     (REPO/PACKAGE/'predecessor-v1/.github/workflows/ci.yml').read_bytes(), workflow,
                     (REPO/'tests/fixtures/typed_project_source_binding/current-source.json').read_bytes())
    outputs[PACKAGE + ci.NAME] = g.encoded(plan)
    name = 'scripts/test_lexer_current_ci.py'
    raw = base(name)
    for key, value in {'CURRENT_WORKFLOW_DESCRIPTOR': g.identity('.github/workflows/ci.yml', workflow),
                       'CURRENT_DISPOSITIONS_DESCRIPTOR': g.identity(ci.NAME, outputs[PACKAGE + ci.NAME])}.items():
        raw = consumers.assignment(raw, key, value)
    raw = raw.replace(b'package-json-direct-consumer-dispositions-v1.json', ci.NAME.encode())
    raw = consumers.exact(raw, b'        self.assertEqual(current, expected)\n',
        b'        sys.path.insert(0, str(REPO / "tests/qualification/unit4_ci"))\n'
        b'        import scheduling_derivation as scheduling\n'
        b'        self.assertEqual(current, scheduling.workflow(expected))\n', 1)
    raw = consumers.exact(raw, b'self.assertEqual(current.replace(new_argument, old_argument), middle)',
                          b'self.assertEqual(expected.replace(new_argument, old_argument), middle)', 1)
    raw = consumers.exact(raw, b'published_ci_inventory.workflow_successor(raw, middle, CURRENT_SHA), current)',
                          b'published_ci_inventory.workflow_successor(raw, middle, CURRENT_SHA), expected)', 1)
    raw = consumers.exact(raw, b'                         old_workflow.replace(old_sha, CURRENT_SHA.encode()))',
        b'                         __import__("ci").scheduling().workflow(old_workflow.replace(old_sha, CURRENT_SHA.encode())))', 1)
    raw = consumers.exact(raw, b'class CurrentCIActivationControls(unittest.TestCase):',
        b'def scheduled_workflow(raw):\n'
        b'    sys.path.insert(0, str(REPO / "tests/qualification/package_json_current_v1"))\n'
        b'    import ci\n'
        b'    return ci.scheduling().workflow(raw)\n\n\n'
        b'class CurrentCIActivationControls(unittest.TestCase):', 1)
    raw = raw.replace(b'__import__("ci").scheduling().workflow(', b'scheduled_workflow(')
    outputs[name] = raw
    # Preserve inputs-package-json-v1.json byte-for-byte; only the active alias moves.
    old_raw = base(HERE + 'inputs.json')
    manifest = json.loads(old_raw)
    manifest['schema'] = 'oxid-unit4-ci-class-scheduling-inputs-v1'
    manifest['scheduling_predecessor'] = g.identity(HERE + 'inputs-package-json-v1.json', old_raw)
    names = {row['path'] for row in manifest['files']} | {PACKAGE + ci.NAME}
    previous = {row['path']: row for row in manifest['files']}
    changed = {PACKAGE + 'ci.py', PACKAGE + 'derive_consumers.py', 'scripts/test_lexer_current_ci.py'}
    rows = []
    for name in sorted(names):
        raw = outputs[name] if name in outputs else (REPO/name).read_bytes()
        row = g.identity(name, raw)
        if name in previous and name not in changed:
            g.need(row == previous[name], 'unapproved scheduling input change: ' + name)
        rows.append(row)
    manifest['files'] = rows
    outputs[HERE + 'inputs-scheduling-v1.json'] = g.encoded(manifest)
    outputs[HERE + 'inputs.json'] = g.encoded(manifest)
    outputs[HERE + 'common.py'] = consumers.assignment(base(HERE + 'common.py'), 'INPUTS_SHA', g.sha(g.encoded(manifest)))
    return outputs


if __name__ == '__main__':
    if sys.argv[1:] == ['--fetch-base']:
        ensure_base()
        sys.exit(0)
    g.need(not sys.argv[1:], 'unknown derivation argument')
    import scheduling
    roster_raw = (json.dumps(scheduling.roster(scheduling.discover()), indent=2, sort_keys=True)+'\n').encode()
    g.need(g.sha(roster_raw) == d.ROSTER_SHA, 'unreviewed discovered test roster')
    scheduling.ROSTER.write_bytes(roster_raw)
    outputs = generate()
    for name, raw in outputs.items():
        (REPO/name).write_bytes(raw)
    print('Derived', len(outputs), 'outputs; 173 obligations; historical evidence retained')
