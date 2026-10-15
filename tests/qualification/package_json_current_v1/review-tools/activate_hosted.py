"""Review-time complete facade generation; no compiler, qualification or publish."""
import json
from pathlib import Path
import subprocess
import sys
sys.dont_write_bytecode = True
repo = Path(__file__).absolute().parents[4]
root = repo / 'tests/qualification/package_json_current_v1'
sys.path.insert(0,str(root))
import generation as g
import derive_hosted as h
original = subprocess.check_output(['git','show',g.MAIN['head']+':'+h.FACADE],cwd=repo)
saved = root/'predecessor-v1'/h.FACADE
saved.parent.mkdir(parents=True,exist_ok=True)
saved.write_bytes(original)
subprocess.run([sys.executable,str(Path(__file__).with_name('activate_consumers.py'))],check=True,cwd=repo)
dispatcher = g.identity(g.PACKAGE+'run.py',(repo/g.PACKAGE/'run.py').read_bytes())
seal = json.loads((root/'source-seal-v1.json').read_bytes())
body, receipt = h.derive(original,dispatcher,seal)
(repo/h.NAME).write_bytes(body)
(repo/h.FACADE).write_bytes(h.facade(body))
(root/'source-proof/hosted-control-derivation-v1.json').write_bytes(g.encoded(receipt))
subprocess.run([sys.executable,str(Path(__file__).with_name('activate_ci.py'))],check=True,cwd=repo)
print('Derived hosted bridge; old92 controls preserved, actual new complete Unit2 receipt still required')
