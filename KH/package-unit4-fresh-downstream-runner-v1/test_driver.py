"""Tiny local driver controls. No producer module, process, build or live owner."""
import hashlib
import json
import linecache
import os
from pathlib import Path
import signal
import sys
import tempfile
import types
import unittest
from unittest.mock import patch


HERE = Path(__file__).resolve().parent
D = types.ModuleType('fresh_live_owner_driver_test')
D.__file__ = str(HERE / 'driver.py')
exec(compile((HERE / 'driver.py').read_bytes(), D.__file__, 'exec'), D.__dict__)


class DriverTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_no_transport_rejects_before_any_producer_load(self):
        with patch.object(D, 'load_bytes', side_effect=AssertionError('effect forbidden')):
            with self.assertRaisesRegex(D.Rejected, 'captured launcher transport'):
                D.main()

    def test_ownership_limits_are_closed_and_checked_before_effects(self):
        manifest = {'schema': D.SCHEMA, 'mode': 'fresh-downstream', 'pins': {},
                    'installed': {}, 'paths': {}, 'limits': D.LIMITS,
                    'capacity': D.CAPACITY, 'environment': {},
                    'ownership_limits': dict(D.OWNERSHIP_LIMITS)}
        transport = {'manifest': manifest, 'request': {}, 'started_monotonic': 1.0}
        with patch.object(D, 'read_pin', side_effect=AssertionError('input read forbidden')):
            for wrong in ({'live_pidfds': 129, 'lifetime_identities': 65536},
                          {'live_pidfds': 128, 'lifetime_identities': 65537},
                          {'live_pidfds': 128},
                          dict(D.OWNERSHIP_LIMITS, unknown=1)):
                manifest['ownership_limits'] = wrong
                with self.assertRaisesRegex(D.Rejected, 'approved diagnostic scope/bounds'):
                    D.validate_transport(transport, b'')
            del manifest['ownership_limits']
            with self.assertRaisesRegex(D.Rejected, 'closed diagnostic input manifest'):
                D.validate_transport(transport, b'')

    def test_stable_pin_reads_exact_bytes(self):
        p = self.root / 'regular'
        p.write_bytes(b'abc')
        pin = {'path': str(p), 'bytes': 3, 'sha256': hashlib.sha256(b'abc').hexdigest()}
        self.assertEqual(D.read_pin(pin), b'abc')
        pin['sha256'] = '0' * 64
        with self.assertRaisesRegex(D.Rejected, 'pin mismatch'):
            D.read_pin(pin)

    def test_read_and_close_failure_keeps_original(self):
        p = self.root / 'regular'
        p.write_bytes(b'abc')
        primary = OSError('primary read')
        actual_close = os.close
        def close(fd):
            actual_close(fd)
            raise RuntimeError('secondary close')
        with patch.object(D.os, 'read', side_effect=primary), patch.object(D.os, 'close', side_effect=close):
            with self.assertRaises(OSError) as caught:
                D.stable_bytes(p, 3)
        self.assertIs(caught.exception, primary)
        self.assertTrue(primary.__notes__)

    def test_captured_module_source_is_exact_and_failed_name_removed(self):
        body = b'raise RuntimeError("fixture")\n'
        name = '_fresh_driver_tiny_fixture'
        pin = {'path': str(self.root / 'never-opened.py'), 'bytes': len(body),
               'sha256': hashlib.sha256(body).hexdigest()}
        with self.assertRaisesRegex(RuntimeError, 'fixture'):
            D.load_bytes(pin, body, name)
        self.assertNotIn(name, sys.modules)
        with self.assertRaisesRegex(D.Rejected, 'captured source bytes'):
            D.load_bytes(pin, body + b' ', name)

    def test_sink_has_distinct_partial_terminal_and_cannot_qualify(self):
        sink = D.Sink(self.root, 2 * D.RESERVE)
        with self.assertRaisesRegex(D.Rejected, 'cannot claim qualification'):
            sink.emit('terminal', {'execution_qualified': True})
        sink.emit('terminal', {'schema': 'tiny-partial-fixture', 'local_complete': False})
        value = json.loads((self.root / 'partial-downstream-diagnostic.json').read_bytes())
        self.assertTrue(value['diagnostic_only'])
        self.assertFalse(value['local_complete'])
        with self.assertRaisesRegex(D.Rejected, 'duplicate partial'):
            sink.emit('terminal', {})

    def test_emergency_reserve_remains_after_ordinary_cap(self):
        sink = D.Sink(self.root, D.RESERVE + 128)
        with self.assertRaisesRegex(D.Rejected, 'output cap'):
            sink.emit('call', {'message': 'x' * 512})
        self.assertTrue(sink.emergency(RuntimeError('primary fixture')))
        self.assertTrue((self.root / 'driver-failure.jsonl').is_file())

    def test_worker_maximum_record_fits_without_truncation(self):
        sink = D.Sink(self.root, D.LIMITS['output_bytes'])
        value = {'payload': 'x' * 131072}
        sink.emit('call', value)
        observed = json.loads((self.root / 'call.jsonl').read_bytes())
        self.assertEqual(observed['payload'], value['payload'])
        self.assertTrue(all(observed[key] is expected for key, expected in D.FLAGS.items()))

    def test_emergency_does_not_lookup_source_and_handles_bad_str(self):
        class BadError(Exception):
            def __str__(self):
                raise RuntimeError('no text')
        sink = D.Sink(self.root, 2 * D.RESERVE)
        with patch.object(linecache, 'getline', side_effect=AssertionError('source lookup forbidden')):
            try:
                raise BadError()
            except BadError as error:
                self.assertTrue(sink.emergency(error))
        value = json.loads((self.root / 'driver-failure.jsonl').read_text())
        self.assertEqual(value['error'], '<exception formatting unavailable>')

    def test_output_write_and_close_failure_keeps_original(self):
        sink = D.Sink(self.root, 2 * D.RESERVE)
        primary = OSError('primary write')
        actual_close = os.close
        def close(fd):
            actual_close(fd)
            raise RuntimeError('secondary close')
        with patch.object(D.os, 'write', side_effect=primary), patch.object(D.os, 'close', side_effect=close):
            with self.assertRaises(OSError) as caught:
                sink.emit('call', {})
        self.assertIs(caught.exception, primary)

    def test_same_path_replacement_and_root_drift_are_rejected(self):
        sink = D.Sink(self.root, 2 * D.RESERVE)
        sink.emit('call', {'value': 1})
        path = self.root / 'call.jsonl'
        body = path.read_bytes()
        path.rename(self.root / 'old-call.jsonl')
        path.write_bytes(body)
        with self.assertRaisesRegex(D.Rejected, 'output changed'):
            sink.emit('call', {'value': 2})
        fake = types.SimpleNamespace(lstat=lambda: types.SimpleNamespace(
            st_dev=sink.identity[0], st_ino=sink.identity[1] + 1))
        with patch.object(D, 'plain', return_value=fake):
            with self.assertRaisesRegex(D.Rejected, 'root identity changed'):
                sink.emit('preservation', {})

    def test_final_pin_error_cannot_replace_worker_error(self):
        primary = RuntimeError('worker fixture')
        cancellation = types.SimpleNamespace(finalizing=False, sealed=False, poison=lambda: None)
        session = types.SimpleNamespace(_Cancellation=lambda: cancellation)
        worker = types.SimpleNamespace(run=lambda *a, **k: (_ for _ in ()).throw(primary))
        manifest = {'installed': {'repository':'/synthetic/repo','head':'a'*40,'tree':'b'*40}, 'paths': {'evidence_root': str(self.root)},
                    'pins': {'session': {}, 'worker': {}, 'driver': {}}}
        fake = (manifest, {}, 1.0, {'session': b'', 'worker': b''})
        with patch.object(D, 'validate_transport', return_value=fake), \
             patch.object(D, 'load_bytes', side_effect=[session, worker]), \
             patch.object(D, 'read_pin', side_effect=OSError('final pin fixture')), \
             patch.object(D.signal, 'getsignal', return_value=signal.SIG_DFL), \
             patch.object(D.signal, 'signal'):
            with self.assertRaises(RuntimeError) as caught:
                D.main()
        self.assertIs(caught.exception, primary)
        self.assertEqual(len(primary.__notes__), 3)
        self.assertTrue(cancellation.sealed)

    def test_partial_signal_setup_still_restores_prior_handlers(self):
        cancellation = types.SimpleNamespace(finalizing=False, sealed=False, poison=lambda: None)
        session = types.SimpleNamespace(_Cancellation=lambda: cancellation)
        worker = types.SimpleNamespace(run=lambda *a, **k: self.fail('worker must not run'))
        manifest = {'installed': {'repository':'/synthetic/repo','head':'a'*40,'tree':'b'*40}, 'paths': {'evidence_root': str(self.root)},
                    'pins': {'session': {}, 'worker': {}}}
        fake = (manifest, {}, 1.0, {'session': b'', 'worker': b''})
        primary = RuntimeError('partial signal setup')
        with patch.object(D, 'validate_transport', return_value=fake), \
             patch.object(D, 'load_bytes', side_effect=[session, worker]), \
             patch.object(D, 'read_pin', return_value=b''), \
             patch.object(D.signal, 'getsignal', return_value=signal.SIG_DFL), \
             patch.object(D.signal, 'signal', side_effect=[None, primary, None, None]) as changed:
            with self.assertRaises(RuntimeError) as caught:
                D.main()
        self.assertIs(caught.exception, primary)
        self.assertEqual(changed.call_count, 4)
        self.assertTrue(cancellation.sealed)

    def test_late_final_pin_failure_poisons_already_sealed_owner(self):
        poisoned = []
        cancellation = types.SimpleNamespace(finalizing=False, sealed=False,
                                              poison=lambda: poisoned.append(True))
        session = types.SimpleNamespace(_Cancellation=lambda: cancellation)
        def run(_session, _request, sink, **_):
            sink.emit('terminal', {'status': 'PARTIAL_MEASUREMENT_STAGE_PASSED'})
            cancellation.sealed = True
        worker = types.SimpleNamespace(run=run)
        manifest = {'installed': {'repository':'/synthetic/repo','head':'a'*40,'tree':'b'*40}, 'paths': {'evidence_root': str(self.root)},
                    'pins': {'session': {}, 'worker': {}}}
        fake = (manifest, {}, 1.0, {'session': b'', 'worker': b''})
        primary = OSError('late source preservation')
        with patch.object(D, 'validate_transport', return_value=fake), \
             patch.object(D, 'load_bytes', side_effect=[session, worker]), \
             patch.object(D, 'read_pin', side_effect=primary), \
             patch.object(D.signal, 'getsignal', return_value=signal.SIG_DFL), \
             patch.object(D.signal, 'signal'):
            with self.assertRaises(OSError) as caught:
                D.main()
        self.assertIs(caught.exception, primary)
        self.assertEqual(poisoned, [True])
        self.assertTrue((self.root / 'driver-failure.jsonl').is_file())


if __name__ == '__main__':
    unittest.main(verbosity=2)
