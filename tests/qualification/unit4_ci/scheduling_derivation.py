"""Named unit4-class-scheduling-v1 derivation; historical bytes are inputs only."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
BASE = '31f49568b1142dd58d80eef8f71faeeb99f32ba5'
BASE_TREE = 'c31f377ccca7ea34b8f741e8ce725975108e481d'
WORKFLOW_SHA = 'b800c47f9b387f94fbd60f3c2e203bae14f71334f57e9369bf50ae5dd67e2b3b'
NAME = 'unit4-class-scheduling-v1'
ROSTER_SHA = 'c2b9aacf2382dfccd88e34850e75cc54c6bb251063960c9b1a7c732d6f6e4eb9'
COMMAND = 'python -B tests/qualification/unit4_ci/scheduling.py run --repo "$GITHUB_WORKSPACE" --head "$UNIT4_EXPECTED_HEAD" --event-sha "$UNIT4_EVENT_SHA" --host "$UNIT4_HOST" --shard "$UNIT4_SHARD" --output "$RUNNER_TEMP/unit4-controls"'


DEPENDENCY_STEPS = '''      - name: Download this-attempt integration approval
        uses: actions/download-artifact@v4
        with:
          name: unit4-integration-join-${{ github.run_id }}-${{ github.run_attempt }}
          path: ${{ runner.temp }}/unit4-approved-controls
      - name: Require successful controls from this exact run and attempt
        shell: bash
        run: PYTHON -B tests/qualification/unit4_ci/scheduling.py verify --repo . --head "$UNIT4_EXPECTED_HEAD" --event-sha "$UNIT4_EVENT_SHA" --artifacts "$RUNNER_TEMP/unit4-approved-controls"
'''

def exact(raw, before, after):
    if raw.count(before) != 1:
        raise ValueError('scheduling anchor missing/duplicate: ' + before[:80])
    return raw.replace(before, after)


def workflow(raw):
    if hashlib.sha256(raw).hexdigest() != WORKFLOW_SHA:
        raise ValueError('unreviewed scheduling predecessor workflow')
    text = raw.decode()
    text = exact(text, '      - name: Test current parser authority and prescribed CI mutation controls\n',
                 '      - name: Fetch and verify fixed scheduling predecessor\n'
                 '        run: python3 -B tests/qualification/unit4_ci/derive_scheduling.py --fetch-base\n\n'
                 '      - name: Test current parser authority and prescribed CI mutation controls\n')
    text = exact(text, '          python3 -B tests/qualification/unit4_ci/test_integration.py\n',
                 '          python3 -B tests/qualification/unit4_ci/test_scheduling.py\n')
    text = exact(text, '      - name: Verify Unit4 integration failure controls\n        run: python3 -B tests/qualification/unit4_ci/test_integration.py\n', '')
    text = exact(text, '      - name: Verify actual-host integration controls\n        run: python -B tests/qualification/unit4_ci/test_integration.py\n', '')
    text = exact(text, '      - name: Execute complete current-source public, hosted-root and parser gates\n',
                 DEPENDENCY_STEPS.replace('PYTHON', 'python3') + '      - name: Execute complete current-source public, hosted-root and parser gates\n')
    text = exact(text, '      - name: Execute ordinary, lifecycle and actual no-native trap gates\n',
                 DEPENDENCY_STEPS.replace('PYTHON', 'python') + '      - name: Execute ordinary, lifecycle and actual no-native trap gates\n')
    text = exact(text, 'python3 -B tests/qualification/unit4_ci/gate.py host',
                 'python3 -B tests/qualification/unit4_ci/gate_scheduling.py host')
    for job in ('unit4-linux', 'unit4-portable-hosts'):
        text = exact(text, '  ' + job + ':\n', '  ' + job + ':\n    needs: unit4-integration-join\n')
    roster_raw = (HERE / 'integration-roster-v1.json').read_bytes()
    if hashlib.sha256(roster_raw).hexdigest() != ROSTER_SHA:
        raise ValueError('unreviewed scheduling roster')
    classes = [{'name': 'comparison-seal', 'minutes': 150},
               {'name': 'remaining-controls', 'minutes': 100}]
    jobs = '''  unit4-integration:
    name: Unit4 controls (${{ matrix.host.name }} / ${{ matrix.shard.name }})
    strategy:
      fail-fast: false
      matrix:
        host:
          - {name: Linux x86_64, os: ubuntu-latest, artifact: linux-x86_64}
          - {name: Windows x86_64, os: windows-latest, artifact: windows-x86_64}
          - {name: macOS x86_64, os: macos-15-intel, artifact: macos-x86_64}
          - {name: macOS arm64, os: macos-14, artifact: macos-arm64}
        shard: CLASSES
    runs-on: ${{ matrix.host.os }}
    timeout-minutes: ${{ matrix.shard.minutes }}
    env:
      UNIT4_EXPECTED_HEAD: ${{ github.event.pull_request.head.sha || github.sha }}
      UNIT4_EVENT_SHA: ${{ github.sha }}
      UNIT4_HOST: ${{ matrix.host.name }}
      UNIT4_SHARD: ${{ matrix.shard.name }}
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ github.event.pull_request.head.sha || github.sha }}
      - uses: actions/setup-python@v5
        with:
          python-version: '3.12'
      - name: Restore committed checkout bytes
        shell: bash
        run: python -B tests/qualification/unit4_ci/common.py --restore-checkout --repo "$GITHUB_WORKSPACE" --output "$RUNNER_TEMP/unit4-controls-checkout" --expected-head "$UNIT4_EXPECTED_HEAD"
      - uses: dtolnay/rust-toolchain@1.99.0
      - run: cargo fetch --locked
      - name: Run complete classes serially
        shell: bash
        run: COMMAND
      - name: Preserve all test failures and receipt
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: unit4-controls-${{ github.run_id }}-${{ github.run_attempt }}-${{ matrix.host.artifact }}-${{ matrix.shard.name }}
          if-no-files-found: error
          retention-days: 14
          path: ${{ runner.temp }}/unit4-controls

  unit4-integration-join:
    needs: unit4-integration
    if: always()
    runs-on: ubuntu-latest
    timeout-minutes: 30
    env:
      UNIT4_EXPECTED_HEAD: ${{ github.event.pull_request.head.sha || github.sha }}
      UNIT4_EVENT_SHA: ${{ github.sha }}
      UNIT4_JOB_RESULTS: ${{ toJSON(needs) }}
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ github.event.pull_request.head.sha || github.sha }}
      - uses: actions/setup-python@v5
        with:
          python-version: '3.12'
      - uses: actions/download-artifact@v4
        with:
          pattern: unit4-controls-${{ github.run_id }}-${{ github.run_attempt }}-*
          path: ${{ runner.temp }}/unit4-controls-artifacts
      - name: Join complete four-host controls
        if: always()
        shell: bash
        run: python -B tests/qualification/unit4_ci/scheduling.py join --repo "$GITHUB_WORKSPACE" --head "$UNIT4_EXPECTED_HEAD" --event-sha "$UNIT4_EVENT_SHA" --artifacts "$RUNNER_TEMP/unit4-controls-artifacts" --job-results "$UNIT4_JOB_RESULTS" --output "$RUNNER_TEMP/unit4-controls-join"
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: unit4-integration-join-${{ github.run_id }}-${{ github.run_attempt }}
          if-no-files-found: error
          retention-days: 14
          path: ${{ runner.temp }}/unit4-controls-join

'''.replace('CLASSES', json.dumps(classes)).replace('COMMAND', COMMAND)
    return exact(text, '  unit4-linux:\n', jobs + '  unit4-linux:\n').encode()


def disposition(plan):
    """Keep all original obligation fields; relocate only three active commands."""
    moved = {'ci.yml:L62', 'ci.yml:L988', 'ci.yml:L1071'}
    if len(plan['entries']) != 173 or len({row['id'] for row in plan['entries']}) != 173:
        raise ValueError('incomplete consumer obligations')
    found = set()
    for row in plan['entries']:
        if row['id'] == 'ci.yml:L997':
            row['active_command'] = exact(row['active_command'], 'unit4_ci/gate.py host', 'unit4_ci/gate_scheduling.py host')
            row['active_command_sha256'] = hashlib.sha256(row['active_command'].encode()).hexdigest()
            row['activation'] = 'unit4-prepare-budget-600-v1'
        if row['id'] in moved:
            found.add(row['id'])
            row.update(activation=NAME, active_command=COMMAND,
                       active_command_sha256=hashlib.sha256(COMMAND.encode()).hexdigest(),
                       scheduling={'job': 'unit4-integration', 'join': 'unit4-integration-join',
                                   'roster_sha256': hashlib.sha256((HERE/'integration-roster-v1.json').read_bytes()).hexdigest()})
    if found != moved:
        raise ValueError('missing moved consumer')
    plan['scheduling_derivation'] = NAME
    return plan
