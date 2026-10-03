#!/usr/bin/env python3
"""Deliberate output-race injector after successful real linking.
This is a negative-control tool observer, not a passive qualification wrapper.
"""
import hashlib,json,os,pathlib,shutil,subprocess,sys,time
name=pathlib.Path(sys.argv[0]).name
real=pathlib.Path(os.environ['UNIT4_REAL_LLVM'])/name
argv=sys.argv[1:];evidence=pathlib.Path(os.environ['UNIT4_TOOL_EVIDENCE'])
if name=='opt' and argv==['-passes=verify','-disable-output','program.ll']:
 (evidence/'program.ll').write_bytes(pathlib.Path('program.ll').read_bytes())
start=time.time_ns();p=subprocess.Popen([str(real)]+argv,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
stdout,stderr=p.communicate();finish=time.time_ns()
record={'tool':name,'real_path':str(real),'real_sha256':hashlib.sha256(real.read_bytes()).hexdigest(),
 'argv':argv,'cwd':os.getcwd(),'wrapper_pid':os.getpid(),'wrapper_ppid':os.getppid(),'real_tool_pid':p.pid,
 'started_ns':start,'completed_ns':finish,'status':p.returncode,'stdout':stdout.decode(),'stderr':stderr.decode(),
 'stdout_sha256':hashlib.sha256(stdout).hexdigest(),'stderr_sha256':hashlib.sha256(stderr).hexdigest()}
phase7=['--no-default-config','--target=x86_64-unknown-linux-gnu','-pie','-Wl,-z,noexecstack','-Wl,-z,relro','-Wl,-z,now','program.o','runtime.o','-o','executable','--ld-path='+str(pathlib.Path(sys.argv[0]).parent/'ld.lld')]
if name=='clang' and p.returncode==0 and argv==phase7:
 linked=pathlib.Path('executable');shutil.copy2(linked,evidence/'linked-before-publication.elf')
 linked_verified_ns=time.time_ns()
 output=pathlib.Path(os.environ['UNIT4_RACE_OUTPUT']);payload=b'winner'
 if os.path.lexists(output):raise RuntimeError('race output already exists before injection')
 absent_verified_ns=time.time_ns()
 descriptor=os.open(output,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600);created_ns=time.time_ns()
 try:
  written=os.write(descriptor,payload);os.fsync(descriptor)
 finally:os.close(descriptor)
 closed_ns=time.time_ns()
 if written!=len(payload):raise RuntimeError('race sentinel write incomplete')
 record['race']={'output':str(output),'created_exclusively':True,'bytes':written,'sha256':hashlib.sha256(output.read_bytes()).hexdigest(),
  'linked_elf_sha256':hashlib.sha256(linked.read_bytes()).hexdigest(),'created_after_real_link_success':True,'created_before_compiler_return':True,
  'linked_verified_ns':linked_verified_ns,'absent_verified_ns':absent_verified_ns,'created_ns':created_ns,'closed_ns':closed_ns,'readback_verified_ns':time.time_ns(),
  'file_identity':{k:getattr(output.stat(),k) for k in ('st_dev','st_ino','st_mode','st_size','st_mtime_ns','st_ctime_ns')}}
record['wrapper_return_ready_ns']=time.time_ns();record['wrapper_return_status']=p.returncode
fd=os.open(os.environ['UNIT4_TOOL_LOG'],os.O_WRONLY|os.O_APPEND|os.O_CREAT,0o600)
os.write(fd,(json.dumps(record,sort_keys=True)+'\n').encode());os.close(fd)
sys.stdout.buffer.write(stdout);sys.stderr.buffer.write(stderr);sys.exit(p.returncode)
