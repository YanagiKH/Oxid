"""Closed overlay, real repository discovery, and CI admission controls."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import build_wide_span_parser as wide
import run_wide_span_parser_ci as ci
import verify_repo


class WideSpanOverlayControls(unittest.TestCase):
    def test_complete_materialization_and_source_identity(self):
        sources = wide.sources()
        self.assertTrue(set((*wide.MEMBERS, 'control_main')) <= set(sources))
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / 'candidate'
            entry = wide.materialize(output)
            self.assertEqual(entry, output / 'parser_main.ox')
            receipt = json.loads((output / 'materialization.json').read_bytes())
            self.assertEqual(set(receipt), set(sources))
            for name, (origin, raw) in sources.items():
                self.assertEqual((output / (name + '.ox')).read_bytes(), raw)
                self.assertEqual(receipt[name]['sha256'], hashlib.sha256(raw).hexdigest())
                self.assertEqual(receipt[name]['origin'], origin.relative_to(wide.ROOT).as_posix())
            with self.assertRaises(FileExistsError):
                wide.materialize(output)

    def test_manifest_unknown_fields_versions_and_changed_hash_fail(self):
        read = wide.regular
        original = json.loads(read(wide.ROOT, wide.BASELINE_MANIFEST))
        changed = [dict(original, extra=True), dict(original, schema_version=True)]
        record = copy.deepcopy(original)
        record['sources']['parser_main']['extra'] = True
        changed.append(record)
        record = copy.deepcopy(original)
        record['sources']['parser_main']['path'] = '../escape'
        changed.append(record)
        for value in changed:
            def replacement(root, name):
                return json.dumps(value).encode() if name == wide.BASELINE_MANIFEST else read(root, name)
            with self.subTest(value=value), patch.object(wide, 'regular', side_effect=replacement):
                with self.assertRaises(ValueError):
                    wide.sources()

    def test_changed_original_and_unresolved_module_fail(self):
        original = wide.regular
        for target, suffix in [('fixtures/typed-frontend-v2/source_buffer.ox', b'\n'),
                               (wide.OVERLAY_PATH + '/parser_main.ox', b'\nmod unlisted;\n')]:
            def replacement(root, name):
                raw = original(root, name)
                return raw + suffix if name == target else raw
            with self.subTest(target=target), patch.object(wide, 'regular', side_effect=replacement):
                with self.assertRaises(ValueError):
                    wide.sources()

    def test_exact_overlay_membership_and_symlink_refusal(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            manifest = root / wide.BASELINE_MANIFEST
            manifest.parent.mkdir(parents=True)
            manifest.write_bytes((wide.ROOT / wide.BASELINE_MANIFEST).read_bytes())
            baseline = json.loads(manifest.read_bytes())
            for record in baseline['sources'].values():
                p = root / record['path'];p.parent.mkdir(parents=True, exist_ok=True)
                p.write_bytes((wide.ROOT / record['path']).read_bytes())
            overlay = root / wide.OVERLAY_PATH;overlay.mkdir()
            for p in wide.OVERLAY.iterdir(): (overlay / p.name).write_bytes(p.read_bytes())
            wide.sources(root)
            extra = overlay / 'unexpected.ox';extra.write_text('')
            with self.assertRaises(ValueError): wide.sources(root)
            extra.unlink()
            victim = overlay / 'parser_main.ox';victim.unlink()
            with self.assertRaises(ValueError): wide.sources(root)
            try: victim.symlink_to(wide.OVERLAY / 'parser_main.ox')
            except (OSError, NotImplementedError): return
            with self.assertRaises(ValueError): wide.sources(root)

    def test_real_discovery_routes_overlay_only_to_typed_materialization(self):
        sources = sorted(wide.ROOT.rglob('*.ox'))
        checks, _, _ = verify_repo.source_plan(sources, wide.ROOT)
        selected = {wide.ROOT / name for name in wide.OVERLAY_FILES}
        self.assertFalse(selected & {p for p, _ in checks})
        with tempfile.TemporaryDirectory() as temp, patch.object(verify_repo, 'run') as run:
            verify_repo.check_wide_overlays(Path('/compiler'), wide.sources(), Path(temp) / 'project')
            self.assertEqual(len(run.call_args_list), 2)
            for call, name in zip(run.call_args_list, ('parser_main', 'control_main')):
                command = call.args[0]
                self.assertEqual(command[:2], ['/compiler', 'check'])
                self.assertTrue(command[2].endswith('/' + name + '.ox'))
                self.assertEqual(command[-1], '--edition=typed-preview')


class WideSpanCIControls(unittest.TestCase):
    def test_compiler_receipt_requires_both_exact_profiles(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); receipt = root / 'receipt.json'
            source = {'head': 'a'*40, 'tree': 'b'*40}
            data = {'complete': True, 'source': source, 'expected_head': source['head'], 'profiles': {}}
            for name, opt in [('debug','0'), ('release','3')]:
                p=root/(name+'-compiler');p.write_bytes(name.encode())
                data['profiles'][name]={'complete':True,'source':source,'retained_executable':str(p),
                    'executable_sha256':ci.sha(p),'cargo_artifact':{'target':{'name':'oxid','kind':['bin']},
                    'profile':{'test':False,'opt_level':opt}}}
            receipt.write_text(json.dumps(data))
            self.assertEqual(set(ci.admitted_profiles(receipt, **source)), {'debug','release'})
            for change in ('complete','head','profile','hash','opt'):
                bad=copy.deepcopy(data)
                if change=='complete':bad['complete']=False
                elif change=='head':bad['expected_head']='c'*40
                elif change=='profile':del bad['profiles']['release']
                elif change=='hash':bad['profiles']['debug']['executable_sha256']='0'*64
                else:bad['profiles']['release']['cargo_artifact']['profile']['opt_level']='0'
                receipt.write_text(json.dumps(bad))
                with self.subTest(change=change), self.assertRaises(ValueError):
                    ci.admitted_profiles(receipt, **source)

    def test_report_requires_real_native_reference_and_complete_controls(self):
        with tempfile.TemporaryDirectory() as temp:
            p=Path(temp)/'binary';p.write_bytes(b'compiler')
            roster_path=ci.ROOT/'tests/fixtures/bounded_typed_parser/cases.json'
            names=[row['name'] for row in json.loads(roster_path.read_bytes())['cases']]
            names += [label+'-'+str(n) for n in (254,255,256) for label in ('function','diagnostic')]
            names += ['empty','non-ascii','token-capacity']
            report={'passed':True,'roster_sha256':ci.sha(roster_path),'source_head':'a'*40,'compiler_sha256':ci.sha(p),'harness_sha256':ci.sha(p),
                'comparisons':[{'name':name,'modes':['baseline-reference','candidate-reference','baseline-native','candidate-native']} for name in names],
                'controls':[{'mode':i,'execution':mode} for i in range(22) for mode in ('reference','native')],
                'binary_sha256':dict.fromkeys(('baseline','candidate','controls'),'b'*64),
                'canonical_observers':dict.fromkeys(('parser','lexer'),'b'*64)}
            ci.validate_report(report,p,p,'a'*40)
            for key in ('passed','controls','comparisons','binary_sha256','canonical_observers','source_head'):
                bad=copy.deepcopy(report);bad[key]=False if key=='passed' else []
                with self.subTest(key=key), self.assertRaises(ValueError):ci.validate_report(bad,p,p,'a'*40)


if __name__ == '__main__':
    unittest.main()
