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


def owned_upload_test(raw):
    """Adapt the upload fixture to adjacent scheduling jobs; preserve every assertion."""
    if hashlib.sha256(raw).hexdigest() != '8084114db22898b6003a9cb1c286e42ee449f8bd8cdf7a5bfbd17c97d37ad073':
        raise ValueError("unreviewed owned upload fixture predecessor")
    before = r'''    def test_container_upload_paths_cover_all_members_after_early_failure(self):
        # Observed on runner 2.337.0, runs 37169581641 and 37169578747.
        # ContainerInfo.TranslateToContainerPath translates one leading prefix
        # per whole INPUT_PATH value, not each line of that value.
        workflow = (Path(__file__).resolve().parents[1] / '.github/workflows/ci.yml').read_text()
        package = workflow.split('      - name: Index and preserve combined Unit2E evidence on every terminal state\n', 1)[1].split('      - name:', 1)[0]
        self.assertIn('        id: unit2e_preservation\n', package)
        self.assertIn('        if: always()\n', package)
        shell = '\n'.join(line[10:] for line in package.split('        run: |\n', 1)[1].splitlines())
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / 'step-output'
            environment = dict(os.environ, RUNNER_TEMP='/__w/_temp', GITHUB_OUTPUT=str(output))
            process = subprocess.run(['/bin/bash', '-e', '-o', 'pipefail', '-c', 'python3() { return 7; }\n' + shell],
                                     env=environment, capture_output=True)
            self.assertEqual(process.returncode, 7)
            self.assertEqual(output.read_text(), 'upload-root=/__w/_temp/fixed-array-unit2e-upload\n')
        def translate(value):
            prefix = '/home/runner/work'
            return '/__w' + value[len(prefix):] if value.startswith(prefix + '/') else value
        uploads = [('Retain compact Unit2E evidence and original receipt bodies',
                    [admission.INDEX_NAME, admission.COMPACT_NAME, admission.COMPACT_NAME + '.sha256']),
                   ('Retain complete or explicitly incomplete combined Unit2E evidence',
                    [admission.ARCHIVE_NAME, admission.ARCHIVE_NAME + '.sha256'])]
        for title, names in uploads:
            block = workflow.split('      - name: ' + title + '\n', 1)[1].split('\n      - name:', 1)[0].split('\n  unit4-linux:', 1)[0]
            self.assertIn('        if: always()\n', block)
            paths = [line.strip() for line in block.split('          path: |\n', 1)[1].splitlines() if line.strip()]
            self.assertEqual(paths, ['${{ steps.unit2e_preservation.outputs.upload-root }}/' + name for name in names])
            actual = translate('\n'.join(paths).replace('${{ steps.unit2e_preservation.outputs.upload-root }}',
                                                    '/__w/_temp/fixed-array-unit2e-upload')).splitlines()
            wanted = ['/__w/_temp/fixed-array-unit2e-upload/' + name for name in names]
            self.assertEqual(actual, wanted)
            old = translate('\n'.join('/home/runner/work/_temp/fixed-array-unit2e-upload/' + name for name in names)).splitlines()
            self.assertEqual([path for path in old if path in wanted], wanted[:1])
'''
    after = r'''    def test_container_upload_paths_cover_all_members_after_early_failure(self):
        # Observed on runner 2.337.0, runs 37169581641 and 37169578747.
        # ContainerInfo.TranslateToContainerPath translates one leading prefix
        # per whole INPUT_PATH value, not each line of that value.
        def indented_block(text, marker, indentation):
            self.assertEqual(text.count(marker), 1)
            lines = []
            for line in text.split(marker, 1)[1].splitlines(True):
                if line.strip() and (len(line) - len(line.lstrip(' '))) <= indentation:
                    break
                lines.append(line)
            return ''.join(lines)
        # A following step or arbitrarily named job cannot become upload paths.
        marker = '          path: |\n'
        body = marker + '            first\n            second\n'
        for adjacent in ('      - name: next step\n        run: poison\n',
                         '  arbitrary-next-job:\n    steps:\n      - run: poison\n',
                         '          retention-days: 7\n'):
            self.assertEqual(indented_block(body + adjacent, marker, 10),
                             '            first\n            second\n')
        self.assertIn('            unexpected\n',
                      indented_block(body + '            unexpected\n', marker, 10))
        workflow = (Path(__file__).resolve().parents[1] / '.github/workflows/ci.yml').read_text()
        package = indented_block(workflow, '      - name: Index and preserve combined Unit2E evidence on every terminal state\n', 6)
        self.assertIn('        id: unit2e_preservation\n', package)
        self.assertIn('        if: always()\n', package)
        shell = '\n'.join(line[10:] for line in package.split('        run: |\n', 1)[1].splitlines())
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / 'step-output'
            environment = dict(os.environ, RUNNER_TEMP='/__w/_temp', GITHUB_OUTPUT=str(output))
            process = subprocess.run(['/bin/bash', '-e', '-o', 'pipefail', '-c', 'python3() { return 7; }\n' + shell],
                                     env=environment, capture_output=True)
            self.assertEqual(process.returncode, 7)
            self.assertEqual(output.read_text(), 'upload-root=/__w/_temp/fixed-array-unit2e-upload\n')
        def translate(value):
            prefix = '/home/runner/work'
            return '/__w' + value[len(prefix):] if value.startswith(prefix + '/') else value
        uploads = [('Retain compact Unit2E evidence and original receipt bodies',
                    [admission.INDEX_NAME, admission.COMPACT_NAME, admission.COMPACT_NAME + '.sha256']),
                   ('Retain complete or explicitly incomplete combined Unit2E evidence',
                    [admission.ARCHIVE_NAME, admission.ARCHIVE_NAME + '.sha256'])]
        for title, names in uploads:
            block = indented_block(workflow, '      - name: ' + title + '\n', 6)
            self.assertIn('        if: always()\n', block)
            paths = [line.strip() for line in indented_block(block, '          path: |\n', 10).splitlines() if line.strip()]
            self.assertEqual(paths, ['${{ steps.unit2e_preservation.outputs.upload-root }}/' + name for name in names])
            actual = translate('\n'.join(paths).replace('${{ steps.unit2e_preservation.outputs.upload-root }}',
                                                    '/__w/_temp/fixed-array-unit2e-upload')).splitlines()
            wanted = ['/__w/_temp/fixed-array-unit2e-upload/' + name for name in names]
            self.assertEqual(actual, wanted)
            old = translate('\n'.join('/home/runner/work/_temp/fixed-array-unit2e-upload/' + name for name in names)).splitlines()
            self.assertEqual([path for path in old if path in wanted], wanted[:1])
'''
    return exact(raw.decode(), before, after).encode()


# Exact current facade identities after relocating one immutable accepted-main body.
# Retained manifests and receipts remain byte-identical; source semantics are unchanged.
IDENTITY_FIXTURE_RELOCATION = {'scripts/test_published_formatter_source.py': {'after': {'bytes': 2177,
                                                          'path': 'scripts/test_published_formatter_source.py',
                                                          'sha256': '13b6042582d900792557d3acad06e6590840881c5fc0f191ce5010647c71506c'},
                                                'before': {'bytes': 2177,
                                                           'path': 'scripts/test_published_formatter_source.py',
                                                           'sha256': 'ad88ecb150e0d72f8909d53693f28ef251da87f2a02c4f6af038049e1820ae5c'}},
 'scripts/verify_owned_array_native.py': {'after': {'bytes': 153138,
                                                    'path': 'scripts/verify_owned_array_native.py',
                                                    'sha256': '5dcf73f1f8c4af03cd62e5342d90d978da1711a62b6a8609937c578d1fb2f8cf'},
                                          'before': {'bytes': 153138,
                                                     'path': 'scripts/verify_owned_array_native.py',
                                                     'sha256': 'c749381f65a366e7dac356be7b316e771397036a9550f69a1b2671a82d281753'}},
 'tests/fixtures/typed_project_source_binding/package-manifest.json': {'after': {'bytes': 16690,
                                                                                 'path': 'tests/fixtures/typed_project_source_binding/package-manifest.json',
                                                                                 'sha256': '87d3ab489ecf0e52913ca79d81a126eaa1c04e703467d04bb546c854aa65d315'},
                                                                       'before': {'bytes': 16690,
                                                                                  'path': 'tests/fixtures/typed_project_source_binding/package-manifest.json',
                                                                                  'sha256': 'db16441e82f0d090b3237a355a4ba8669b1f0858cf3871a3d38e9e51cfe2c5ad'}},
 'tests/fixtures/typed_project_source_binding/run.py': {'after': {'bytes': 6774,
                                                                  'path': 'tests/fixtures/typed_project_source_binding/run.py',
                                                                  'sha256': '581c9b8949b9725cffc4931504ae59c7c4e98312e629234d16ea6c0018e6ee9d'},
                                                        'before': {'bytes': 6774,
                                                                   'path': 'tests/fixtures/typed_project_source_binding/run.py',
                                                                   'sha256': 'd7f81190dde89901808ffcd9cb769a8fd4728ad6ff7f1b26d694340da9e17a4e'}},
 'tests/qualification/lexer_reservation_current/test_current_source.py': {'after': {'bytes': 3493,
                                                                                    'path': 'tests/qualification/lexer_reservation_current/test_current_source.py',
                                                                                    'sha256': 'd3304050c16659d906e8a71f92dd340412bce589f2dd87b77c3ab35e00b352ca'},
                                                                          'before': {'bytes': 3493,
                                                                                     'path': 'tests/qualification/lexer_reservation_current/test_current_source.py',
                                                                                     'sha256': 'bf1cd17e3f6a5f864ce70e21c3d99dd7c0c94936c1e4dccac0b9f80a03007f05'}},
 'tests/qualification/lexer_reservation_current/unit1_current_support.py': {'after': {'bytes': 13746,
                                                                                      'path': 'tests/qualification/lexer_reservation_current/unit1_current_support.py',
                                                                                      'sha256': '23daad2a3bc89f33da509c957e37651fcbc6be6124b56bf5da6d31409eedbf31'},
                                                                            'before': {'bytes': 13746,
                                                                                       'path': 'tests/qualification/lexer_reservation_current/unit1_current_support.py',
                                                                                       'sha256': '62729d359f7da44b119707f416cc90a1a2867f955fd40f9a4784c9146fb743e4'}},
 'tests/qualification/package_json_current_v1/accepted-view-v1.json': {'after': {'bytes': 1023905,
                                                                                 'path': 'tests/qualification/package_json_current_v1/accepted-view-v1.json',
                                                                                 'sha256': 'ddcf5e3c09cda020aca4f1b639d2a66d7357f843f130054800d20228606ad76a'},
                                                                       'before': {'bytes': 1023846,
                                                                                  'path': 'tests/qualification/package_json_current_v1/accepted-view-v1.json',
                                                                                  'sha256': 'bdaf84866568205c978062beea8c7cd70a9e8b40e24568de15d5127b407d1976'}},
 'tests/qualification/package_json_current_v1/generated-parser-authority-v1.json': {'after': {'bytes': 529359,
                                                                                              'path': 'tests/qualification/package_json_current_v1/generated-parser-authority-v1.json',
                                                                                              'sha256': '266c1078275213018f3344cfa94c83ed4ef2b119fa3d8b3f23fb36d93cd85576'},
                                                                                    'before': {'bytes': 529359,
                                                                                               'path': 'tests/qualification/package_json_current_v1/generated-parser-authority-v1.json',
                                                                                               'sha256': '969f9b642c057b2e392621b212033c1a0a842a47f6de6dcf5b22dcd8e88a2c52'}},
 'tests/qualification/package_json_current_v1/hosted_controls.py': {'after': {'bytes': 184125,
                                                                              'path': 'tests/qualification/package_json_current_v1/hosted_controls.py',
                                                                              'sha256': '4c781bef120ac64550aaf0bfdd5826ffcb1ddd3bd5f70acbf468f130999672c5'},
                                                                    'before': {'bytes': 184125,
                                                                               'path': 'tests/qualification/package_json_current_v1/hosted_controls.py',
                                                                               'sha256': '249c64a6d4b963054f91367b81b9bd1eb8f825299c4cd5dc3a3e97add7b8f48c'}},
 'tests/qualification/package_json_current_v1/predecessor-v1/tests/qualification/unit4_ci/test_source_identity.py': {'after': {'bytes': 19556,
                                                                                                                               'path': 'tests/qualification/package_json_current_v1/predecessor-v1/tests/qualification/unit4_ci/test_source_identity.py',
                                                                                                                               'sha256': '52b324aafe927d1fb9d552e726a022ba919e53b73a644f3313ff188d7b150436'},
                                                                                                                     'before': None},
 'tests/qualification/package_json_current_v1/source-proof/hosted-control-derivation-v1.json': {'after': {'bytes': 900,
                                                                                                          'path': 'tests/qualification/package_json_current_v1/source-proof/hosted-control-derivation-v1.json',
                                                                                                          'sha256': 'ec0fdcfe93773ab1a8cbe6e91a06982258ee3ad562f2a8e29d456565cdff6352'},
                                                                                                'before': {'bytes': 900,
                                                                                                           'path': 'tests/qualification/package_json_current_v1/source-proof/hosted-control-derivation-v1.json',
                                                                                                           'sha256': '1c2cbff8023cab1190301f5fd0a4581f275342b0345aef77617f171bf281c59a'}},
 'tests/qualification/package_json_current_v1/source-seal-v1.json': {'after': {'bytes': 388,
                                                                               'path': 'tests/qualification/package_json_current_v1/source-seal-v1.json',
                                                                               'sha256': '937e727b74d5813c5f7d5f2370c2b2f52476adf0e19053ba3b17506e3821dbd0'},
                                                                     'before': {'bytes': 388,
                                                                                'path': 'tests/qualification/package_json_current_v1/source-seal-v1.json',
                                                                                'sha256': '285ef0359e2ccd78a0753ba6d0210d4c119ffa253210c1bf77898a02511f6810'}},
 'tests/qualification/unit4_parser_current/authority.json': {'after': {'bytes': 529359,
                                                                       'path': 'tests/qualification/unit4_parser_current/authority.json',
                                                                       'sha256': '266c1078275213018f3344cfa94c83ed4ef2b119fa3d8b3f23fb36d93cd85576'},
                                                             'before': {'bytes': 529359,
                                                                        'path': 'tests/qualification/unit4_parser_current/authority.json',
                                                                        'sha256': '969f9b642c057b2e392621b212033c1a0a842a47f6de6dcf5b22dcd8e88a2c52'}},
 'tests/qualification/unit4_parser_current/portable.py': {'after': {'bytes': 238733,
                                                                    'path': 'tests/qualification/unit4_parser_current/portable.py',
                                                                    'sha256': 'e513d06fd3eaeb272facbe074d19b0fca6afb946e7a67e5a2203dff183f6ed76'},
                                                          'before': {'bytes': 238733,
                                                                     'path': 'tests/qualification/unit4_parser_current/portable.py',
                                                                     'sha256': '5cbdf39dc143d9180da01d2cb6b9b2a3b52b478ed714d9f3cbf45508ba02c4ab'}},
 'tests/qualification/unit4_parser_current/test_current.py': {'after': {'bytes': 3482,
                                                                        'path': 'tests/qualification/unit4_parser_current/test_current.py',
                                                                        'sha256': '93d9636b6f3f104a7c1c35f6d6681237005f8cb8dbffda256a9b94d2bd682062'},
                                                              'before': {'bytes': 3482,
                                                                         'path': 'tests/qualification/unit4_parser_current/test_current.py',
                                                                         'sha256': '4a2a32813e1992d213fc3e505175b1ac4d7bdd797ec02ba17c6434ece8d772d8'}},
 'tests/qualification/unit4_parser_current/test_lexer_reservation.py': {'after': {'bytes': 3491,
                                                                                  'path': 'tests/qualification/unit4_parser_current/test_lexer_reservation.py',
                                                                                  'sha256': '3f4cf499ba26e7946122239d9c37744bb2222734abbcff09e0b555dbb8363191'},
                                                                        'before': {'bytes': 3491,
                                                                                   'path': 'tests/qualification/unit4_parser_current/test_lexer_reservation.py',
                                                                                   'sha256': '8dfc4dbf41dac6c54b8eebf6d47e25d4c403e0d94546b286eb9e7398b79ed730'}},
 'tests/qualification/unit4_public_v3/authority.py': {'after': {'bytes': 2331,
                                                                'path': 'tests/qualification/unit4_public_v3/authority.py',
                                                                'sha256': 'ec97b2b6982630482f87d68eec6909c9598071f8ad2002fc3455e245aca27507'},
                                                      'before': {'bytes': 2331,
                                                                 'path': 'tests/qualification/unit4_public_v3/authority.py',
                                                                 'sha256': '21b65a2f1f0857ed5d0860a449aea667be65fb969f2d51630d8708d306284878'}},
 'tests/qualification/unit4_public_v3/package-json-lifecycle-v1.json': {'after': {'bytes': 145624,
                                                                                  'path': 'tests/qualification/unit4_public_v3/package-json-lifecycle-v1.json',
                                                                                  'sha256': '61f52bc2700268d57ecbef8865e296cd3dcf3264a480357b775712fd83d95bb3'},
                                                                        'before': {'bytes': 145624,
                                                                                   'path': 'tests/qualification/unit4_public_v3/package-json-lifecycle-v1.json',
                                                                                   'sha256': '4b46d20a52e92dd2e1f5732969ce23c871fd5f89b10b96b2cb7b7d89389ff491'}},
 'tests/qualification/unit4_public_v3/package_json_lifecycle.py': {'after': {'bytes': 5647,
                                                                             'path': 'tests/qualification/unit4_public_v3/package_json_lifecycle.py',
                                                                             'sha256': '286bceed1d1a44c55376a809d6eb903c4155c6e200e3b754602a30f241986873'},
                                                                   'before': {'bytes': 5647,
                                                                              'path': 'tests/qualification/unit4_public_v3/package_json_lifecycle.py',
                                                                              'sha256': 'fcab606f3e039f13d366fbe9d6d6fcdf0259a44f8d6c2087696d97b2e0e747f8'}}}
