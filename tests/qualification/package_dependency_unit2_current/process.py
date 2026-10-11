"""Finite Linux owned-process supervisor for the Unit2 producer.

Resource limits are operational bounds, not a network or syscall sandbox.
"""
import hashlib,json,os,resource,selectors,signal,stat,subprocess,time
from pathlib import Path

def need(value,message):
 if not value:raise ValueError(message)

def digest(path,tick=lambda:None):
 path=Path(path);need(stat.S_ISREG(path.lstat().st_mode),'nonregular evidence');h=hashlib.sha256()
 with path.open('rb') as stream:
  while True:
   tick();chunk=stream.read(1024*1024)
   if not chunk:break
   h.update(chunk)
 return h.hexdigest()

def members(pgid):
 found=[]
 for path in Path('/proc').glob('[0-9]*/stat'):
  try:
   raw=path.read_text();end=raw.rindex(')');fields=raw[end+2:].split()
   if int(fields[2])!=pgid:continue
   member={'pid':int(path.parent.name),'ppid':int(fields[1]),'start_ticks':int(fields[19]),'rss':int(fields[21])*os.sysconf('SC_PAGE_SIZE'),'comm':raw[raw.index('(')+1:end]}
   found.append(member)
   for key,read in [('cmdline',lambda:(path.parent/'cmdline').read_bytes().replace(b'\0',b' ').decode(errors='replace')),('exe',lambda:os.readlink(path.parent/'exe'))]:
    try:member[key]=read()
    except OSError as error:member[key+'_unavailable']=type(error).__name__
  except (FileNotFoundError,ProcessLookupError):pass
 return found

def disk_bytes(root,tick):
 total=0
 for path in Path(root).rglob('*'):
  tick();value=path.lstat();need(stat.S_ISDIR(value.st_mode) or stat.S_ISREG(value.st_mode),'special output entry')
  if stat.S_ISREG(value.st_mode):total+=value.st_blocks*512
 return total

def write_json(path,value):
 with Path(path).open('x') as stream:json.dump(value,stream,indent=2,allow_nan=False);stream.write('\n')

COMPILE={'wall':600,'cpu':600,'as':8*1024**3,'file':2*1024**3,'rss':8*1024**3,'processes':16,'stream':32*1024**2,'disk':6*1024**3}
RUNTIME={'wall':180,'cpu':180,'as':2*1024**3,'file':256*1024**2,'rss':3*1024**3,'processes':4,'stream':16*1024**2,'disk':2*1024**3}

def run_operation(command,cwd,env,operation,index,bounds,tick,disk_roots):
 """Run once; always return captured failure/cleanup evidence, never retry."""
 operation=Path(operation);need(not operation.exists() and not operation.is_symlink(),'fresh operation required');operation.mkdir()
 started=time.monotonic();spawn=None;finished=None;process=None;selector=None;outputs={};counts={'stdout':0,'stderr':0};samples=[];failure=None;cleanup=None;remaining=[];code=None
 def limits():
  resource.setrlimit(resource.RLIMIT_CORE,(0,0));resource.setrlimit(resource.RLIMIT_AS,(bounds['as'],bounds['as']));resource.setrlimit(resource.RLIMIT_FSIZE,(bounds['file'],bounds['file']));resource.setrlimit(resource.RLIMIT_CPU,(bounds['cpu'],bounds['cpu']+10))
 def sample(terminal=False):
  tick();group=members(process.pid) if process else [];size=sum(disk_bytes(root,tick) for root in disk_roots)
  value={'elapsed':time.monotonic()-started,'members':group,'output_bytes':size};samples.append(value)
  need(value['elapsed']<=bounds['wall'],'operation deadline');need(size<=bounds['disk'],'output disk limit');need(len(group)<=bounds['processes'] and sum(row['rss'] for row in group)<=bounds['rss'],'process/RSS limit')
  usage=os.statvfs(operation);need(usage.f_bavail*usage.f_frsize>=16*1024**3,'free space floor')
  if terminal:need(not group,'owned group remains')
 try:
  tick();spawn=time.time_ns();process=subprocess.Popen(command,cwd=cwd,env=env,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True,preexec_fn=limits)
  write_json(operation/'ownership.json',{'pid':process.pid,'pgid':process.pid,'argv':command})
  for name in counts:outputs[name]=(operation/(name+'.log')).open('xb')
  selector=selectors.DefaultSelector();selector.register(process.stdout,selectors.EVENT_READ,'stdout');selector.register(process.stderr,selectors.EVENT_READ,'stderr');next_sample=0
  while selector.get_map() or process.poll() is None:
   tick()
   if time.monotonic()>=next_sample:sample();next_sample=time.monotonic()+0.1
   for key,_ in selector.select(0.05):
    chunk=os.read(key.fileobj.fileno(),65536);name=key.data
    if not chunk:selector.unregister(key.fileobj);continue
    capacity=bounds['stream']-counts[name];outputs[name].write(chunk[:max(0,capacity)]);counts[name]+=len(chunk);need(counts[name]<=bounds['stream'],'stream limit')
  code=process.poll();sample(True);need(code==0,'child nonzero exit')
 except BaseException as error:failure=type(error).__name__+': '+str(error)
 finally:
  if process:
   try:
    if process.poll() is None or members(process.pid):
     try:os.killpg(process.pid,signal.SIGTERM)
     except ProcessLookupError:pass
     until=time.monotonic()+5
     while members(process.pid) and time.monotonic()<until:process.poll();time.sleep(0.1)
     if members(process.pid):
      try:os.killpg(process.pid,signal.SIGKILL)
      except ProcessLookupError:pass
    code=process.wait(timeout=5);finished=time.time_ns();remaining=members(process.pid);need(not remaining,'remaining owned group')
   except BaseException as error:cleanup=str(error);failure=failure or 'cleanup failure'
  for stream in outputs.values():stream.close()
  if selector:selector.close()
  if process:process.stdout.close();process.stderr.close()
 evidence={}
 try:
  for path in operation.iterdir():
   need(stat.S_ISREG(path.lstat().st_mode),'nonregular operation evidence');evidence[path.name]={'bytes':path.stat().st_size,'sha256':digest(path,tick)}
  need(set(evidence)=={'stdout.log','stderr.log','ownership.json'},'operation evidence closure')
 except BaseException as error:failure=failure or 'evidence: '+str(error)
 return {'index':index,'spawn_ns':spawn,'finish_ns':finished,'pid':process.pid if process else None,'exit':code,'failure':failure,'cleanup_error':cleanup,'remaining_owned_group':remaining,'stream_bytes':counts,'samples':samples,'evidence':evidence}
