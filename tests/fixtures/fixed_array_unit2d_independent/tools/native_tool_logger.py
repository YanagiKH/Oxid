#!/usr/bin/python3
"""Transparent test-only receipt wrapper around pinned official LLVM tools."""
import json,os,subprocess,sys,time
from pathlib import Path
name=Path(sys.argv[0]).name
root=Path(os.environ['OXID_UNIT2D_TOOL_RECEIPTS']); root.mkdir(parents=True,exist_ok=True)
mapping=Path(os.environ['OXID_UNIT2D_TRUSTED_TOOLS'])
tools=json.loads(mapping.read_text()); target=tools[name]
identity=f'{time.time_ns()}-{os.getpid()}-{name}'
start=time.monotonic()
run=subprocess.run([target,*sys.argv[1:]],capture_output=True)
(root/(identity+'.stdout')).write_bytes(run.stdout)
(root/(identity+'.stderr')).write_bytes(run.stderr)
(root/(identity+'.json')).write_text(json.dumps(dict(tool=name,trusted_target=target,args=sys.argv[1:],cwd=os.getcwd(),exit=run.returncode,seconds=time.monotonic()-start,stdout=identity+'.stdout',stderr=identity+'.stderr'),indent=2)+'\n')
sys.stdout.buffer.write(run.stdout); sys.stderr.buffer.write(run.stderr)
sys.exit(run.returncode)
