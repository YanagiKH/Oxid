"""Effects-blocked runner controls; no original producer is loaded or invoked."""
import hashlib
from pathlib import Path
import runpy
import sys
import unittest
assert sys.executable == '/usr/bin/python3.13' and sys.version_info[:3] == (3,13,5)
assert sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize
assert hashlib.sha256(Path(sys.executable).read_bytes()).hexdigest() == '889c603f0d17cb54060951bcf4c4f9b8c9ebd9e52b392c70209bbb9755d797d9'
def firewall(event,args):
    if event in ('subprocess.Popen','os.system','os.posix_spawn','os.posix_spawnp','os.fork','os.forkpty','os.exec','socket.connect','socket.bind','ctypes.dlopen'):
        raise RuntimeError('source-only controls forbid effects: '+event)
# Load inert standard ctypes before the firewall; no CDLL, syscall or producer use.
import ctypes
sys.addaudithook(firewall)
suite=unittest.TestSuite()
for path in sorted(Path(__file__).parent.glob('test_*.py')):
    module=runpy.run_path(str(path),run_name='runner_source_'+path.stem)
    suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(value) for value in module.values()
                   if isinstance(value,type) and issubclass(value,unittest.TestCase))
result=unittest.TextTestRunner(verbosity=2).run(suite)
print({'tests':result.testsRun,'failures':len(result.failures),'errors':len(result.errors),'skips':len(result.skipped),
       'source_only':True,'real_operations':0,'execution_qualified':False})
raise SystemExit(not result.wasSuccessful() or bool(result.skipped))
