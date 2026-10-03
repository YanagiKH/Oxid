#!/usr/bin/env python3
"""Integrated synthetic receipt negatives through unmocked portable admission."""
import copy
import json
import os
from pathlib import Path
import tempfile
import unittest

import portable as p
from synthetic_receipts import build_fixture


class ReceiptControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory(prefix='oxid-portable-integrated-')
        cls.f = build_fixture(Path(cls.tmp.name) / 'synthetic-debug')
        cls.records = []
        cls.saved = {}
        for folder in ('build-debug', 'collect-debug'):
            for path in (cls.f['root'] / folder).rglob('*'):
                if path.is_file() and not path.is_symlink() and 'cargo-home' not in path.parts:
                    cls.saved[path] = (path.read_bytes(), path.stat().st_atime_ns, path.stat().st_mtime_ns)
        cls.session = cls.f['session_path'].read_bytes()

    @classmethod
    def tearDownClass(cls):
        report = {'schema': 'oxid-portable-integrated-synthetic-controls-v2',
                  'compiler_build_executions': 0, 'candidate_executions': 0,
                  'native_and_rust_identity_probes': True, 'records': cls.records,
                  'status': 'pass' if all(x['status'] == 'pass' for x in cls.records) else 'fail'}
        if os.environ.get('UNIT4_PORTABLE_RECEIPT_REPORT'):
            p.write(os.environ['UNIT4_PORTABLE_RECEIPT_REPORT'], report)
        cls.tmp.cleanup()

    def restore(self):
        self.f['session_path'].write_bytes(self.session)
        for path, (raw, atime, mtime) in self.saved.items():
            if not path.exists() or path.read_bytes() != raw:
                path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(raw)
                os.utime(path, ns=(atime, mtime))
        result = self.f['root'] / 'collect-debug/result'
        for path in result.rglob('*'):
            if path.is_file() and path not in self.saved:
                path.unlink()

    def rewrite(self, path, change):
        value = p.read(path); change(value); p.write(path, value)

    def refresh_envelopes(self):
        root = self.f['root']; build_dir = root / 'build-debug'; collect_dir = root / 'collect-debug'
        build_path = build_dir / 'result/build-receipt.json'
        build = p.read(build_path)
        for key in ('stdout', 'stderr', 'binary'):
            if Path(build[key]['path']).is_file(): build[key] = p.identity(build[key]['path'])
        p.write(build_path, build)
        envelope = p.read(build_dir / 'portable-build.json')
        envelope['receipt'] = p.identity(build_path); envelope['invocation'] = p.identity(build_dir / 'invocation.json')
        p.write(build_dir / 'portable-build.json', envelope)
        manifest_path = self.f['manifest_path']; manifest = p.read(manifest_path)
        manifest['build_receipt'] = p.identity(build_path)
        for receipt in manifest['case_receipts']:
            for key in ('stdout', 'stderr', 'request', 'raw'):
                if Path(receipt[key]['path']).is_file(): receipt[key] = p.identity(receipt[key]['path'])
            p.write(collect_dir / 'result' / receipt['case_id'] / 'receipt.json', receipt)
        if Path(manifest['observations']['path']).is_file(): manifest['observations'] = p.identity(manifest['observations']['path'])
        p.write(manifest_path, manifest)
        envelope = p.read(collect_dir / 'portable-collection.json')
        envelope['manifest'] = p.identity(manifest_path); envelope['invocation'] = p.identity(collect_dir / 'invocation.json')
        p.write(collect_dir / 'portable-collection.json', envelope)

    def admit(self, full=False, nonces=None):
        f = self.f
        contract = f['c'].load_contract(f['CONTRACT']) if full else {'cases': [f['case']]}
        return f['c'].execution_manifest(f['manifest_path'], contract,
               {'authority': f['A'], 'session_path': f['session_path'], 'nonces': set() if nonces is None else nonces})

    def rejection(self, name, action, refresh=True, full=False, nonces=None):
        self.restore()
        action()
        if refresh: self.refresh_envelopes()
        try:
            self.admit(full=full, nonces=nonces)
        except (ValueError, OSError, KeyError, TypeError, IndexError) as error:
            self.records.append({'name': name, 'status': 'pass', 'rejection': str(error)})
        else:
            self.records.append({'name': name, 'status': 'fail', 'rejection': None})
            self.fail('invalid synthetic receipt admitted: ' + name)

    def test_integrated_positive_and_negative_controls(self):
        self.restore(); profile, rows, _ = self.admit()
        self.assertEqual((profile, len(rows)), ('debug', 1))
        self.records.append({'name': 'single_synthetic_case_admitted', 'status': 'pass', 'scope': 'not the full contract'})
        root = self.f['root']; mp = self.f['manifest_path']; case = self.f['case']['id']
        case_dir = root / 'collect-debug/result' / case
        build_inv = root / 'build-debug/invocation.json'; collect_inv = root / 'collect-debug/invocation.json'
        collection = root / 'collect-debug/portable-collection.json'; build_envelope = root / 'build-debug/portable-build.json'
        self.rejection('full638_contract_rejects_one_case', lambda: None, full=True)
        self.rejection('cross_profile_nonce_reuse', lambda: None, nonces={self.f['nonce']})
        for where, path in (('build', build_inv), ('collection', collect_inv)):
            for field in ('release', 'version', 'machine', 'system'):
                self.rejection(where + '_host_' + field, lambda path=path, field=field: self.rewrite(path, lambda v: v['host']['uname'].__setitem__(field, 'SYNTHETIC-DIFFERENT')))
            self.rejection(where + '_python_identity', lambda path=path: self.rewrite(path, lambda v: v['host'].__setitem__('python_version', 'SYNTHETIC-DIFFERENT')))
        for name, field, value in (('wrong_profile', 'profile', 'release'), ('running_status', 'status', 'running'),
                                   ('missing_roster', 'requested_case_ids', []), ('missing_receipts', 'case_receipts', []),
                                   ('wrong_case_count', 'case_count', 0), ('wrong_mode_count', 'observation_count', 0)):
            self.rejection(name, lambda field=field, value=value: self.rewrite(mp, lambda v: v.__setitem__(field, value)))
        self.rejection('duplicate_receipt', lambda: self.rewrite(mp, lambda v: v['case_receipts'].append(copy.deepcopy(v['case_receipts'][0]))))
        self.rejection('zero_execution_stdout', lambda: (case_dir / 'stdout.txt').write_bytes(b'test result: ok. 0 passed; 0 failed; 0 ignored;\n'))
        self.rejection('missing_raw_file', lambda: (case_dir / 'raw.json').unlink())
        self.rejection('extra_unreported_evidence', lambda: (root / 'collect-debug/result/EXTRA').write_bytes(b'SYNTHETIC extra'))
        self.rejection('actual_rust_abi_mismatch', lambda: self.rewrite(case_dir / 'raw.json', lambda v: v['observations'][0].__setitem__('pointer_width', 32)))
        self.rejection('actual_rust_arch_mismatch', lambda: self.rewrite(case_dir / 'raw.json', lambda v: v['observations'][0].__setitem__('runtime_architecture', 'aarch64')))
        self.rejection('stale_raw_nonce', lambda: self.rewrite(case_dir / 'raw.json', lambda v: v.__setitem__('nonce', '2'*32)))
        self.rejection('forged_normalized_row', lambda: (root / 'collect-debug/result/observations.jsonl').write_text('{}\n'))
        self.rejection('zero_raw_observations', lambda: self.rewrite(case_dir / 'raw.json', lambda v: v.__setitem__('observations', [])))
        self.rejection('collection_precedes_build', lambda: self.rewrite(collect_inv, lambda v: v.__setitem__('started_ns', 0)))
        self.rejection('build_precedes_session', lambda: self.rewrite(build_inv, lambda v: v.__setitem__('started_ns', 0)))
        self.rejection('ambient_environment_added_to_build', lambda: self.rewrite(build_inv, lambda v: v['environment'].__setitem__('RUSTC_WRAPPER', '/unreviewed')))
        self.rejection('stale_session_bytes', lambda: self.rewrite(self.f['session_path'], lambda v: v.__setitem__('run_id', '9'*32)), refresh=False)
        self.rejection('wrong_collection_session_path', lambda: self.rewrite(collection, lambda v: v['session'].__setitem__('path', str(root / 'foreign-session.json'))), refresh=False)
        self.rejection('wrong_build_receipt_path', lambda: self.rewrite(build_envelope, lambda v: v['receipt'].__setitem__('path', str(root / 'foreign-build.json'))), refresh=False)
        self.rejection('wrong_collection_manifest_path', lambda: self.rewrite(collection, lambda v: v['manifest'].__setitem__('path', str(root / 'foreign-manifest.json'))), refresh=False)
        self.restore()

    def test_relocated_release_session(self):
        fixture = build_fixture(Path(self.tmp.name) / 'unrelated-release-root', 'release')
        profile, rows, _ = fixture['c'].execution_manifest(fixture['manifest_path'], {'cases': [fixture['case']]},
                            {'authority': fixture['A'], 'session_path': fixture['session_path'], 'nonces': set()})
        self.assertEqual((profile, len(rows)), ('release', 1))
        self.records.append({'name': 'relocated_synthetic_release_admitted', 'status': 'pass', 'scope': 'not the full contract'})


if __name__ == '__main__':
    unittest.main(verbosity=2)
