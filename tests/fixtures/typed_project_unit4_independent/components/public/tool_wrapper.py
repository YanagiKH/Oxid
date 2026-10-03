#!/usr/bin/env python3
"""Passive external-tool observer. Executes the real tool without output changes."""
import hashlib,json,os,pathlib,subprocess,sys,time
name=pathlib.Path(sys.argv[0]).name
real=pathlib.Path(os.environ['UNIT4_REAL_LLVM'])/name
argv=sys.argv[1:]
if name=='opt' and argv==['-passes=verify','-disable-output','program.ll']:
    src=pathlib.Path('program.ll')
    (pathlib.Path(os.environ['UNIT4_TOOL_EVIDENCE'])/'program.ll').write_bytes(src.read_bytes())
start=time.time_ns()
p=subprocess.run([str(real)]+argv,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
record={'tool':name,'real_path':str(real),'real_sha256':hashlib.sha256(real.read_bytes()).hexdigest(),
        'argv':argv,'cwd':os.getcwd(),'started_ns':start,'completed_ns':time.time_ns(),
        'status':p.returncode,'stdout_sha256':hashlib.sha256(p.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(p.stderr).hexdigest()}
fd=os.open(os.environ['UNIT4_TOOL_LOG'],os.O_WRONLY|os.O_APPEND|os.O_CREAT,0o600)
os.write(fd,(json.dumps(record,sort_keys=True)+'\n').encode());os.close(fd)
sys.stdout.buffer.write(p.stdout);sys.stderr.buffer.write(p.stderr)
sys.exit(p.returncode)
