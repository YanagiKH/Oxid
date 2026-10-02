"""Portable qualification primitives. Optimized Python is never admitted."""
import hashlib,json,os,pathlib,sys
if not __debug__ or sys.flags.optimize or os.environ.get('PYTHONOPTIMIZE','0') not in ('','0'):
    raise SystemExit('PROTOCOL_REFUSED: optimized Python disables qualification checks; no worker executed')

SCHEMA='oxid-unit3-portable-v1'
def require(condition,message):
    if not condition:raise ValueError(message)
def identity(path):
    path=pathlib.Path(path);digest=hashlib.sha256();size=0
    with path.open('rb')as stream:
        while chunk:=stream.read(1024*1024):digest.update(chunk);size+=len(chunk)
    return {'bytes':size,'sha256':digest.hexdigest()}
def bound_file(path):return {'path':str(pathlib.Path(path).resolve()),**identity(path)}
def verify(path,expected):
    require(pathlib.Path(path).stat().st_size==expected['bytes'],f'byte-size mismatch before hashing: {path}')
    got=identity(path);require(got=={k:expected[k]for k in ('bytes','sha256')},f'identity mismatch: {path}');return got
def read_json(path):return json.loads(pathlib.Path(path).read_text())
def write_json(path,value):pathlib.Path(path).write_text(json.dumps(value,sort_keys=True,indent=2)+'\n')
def relative(value):
    require(isinstance(value,str)and value and not any(c in value for c in '\0\r\n\\'),'invalid relative path')
    p=pathlib.PurePosixPath(value)
    require(not p.is_absolute()and all(v not in ('','.','..')for v in value.split('/')),'unsafe relative path')
    require(str(p)==value,'noncanonical relative path');return pathlib.Path(*p.parts)
def fresh(path):
    path=pathlib.Path(path).resolve();require(not path.exists(),'output already exists');path.mkdir(parents=True);return path
def assertion_mode():return {'__debug__':__debug__,'optimize':sys.flags.optimize,'PYTHONOPTIMIZE':os.environ.get('PYTHONOPTIMIZE',''),'python':sys.version}
def safe_env(extra=None):
    env=os.environ.copy();env['PYTHONOPTIMIZE']='0'
    if extra:env.update(extra)
    return env
def run_bounded(command,*,timeout,env=None,cwd=None,stdout=None,stderr=None):
    """Kill only the process group started by this call on timeout."""
    import signal,subprocess
    process=subprocess.Popen(command,env=env,cwd=cwd,text=True,stdout=subprocess.PIPE if stdout is None else stdout,stderr=subprocess.PIPE if stderr is None else stderr,start_new_session=os.name=='posix')
    try:output,errors=process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        if os.name=='posix':
            try:os.killpg(process.pid,signal.SIGKILL)
            except ProcessLookupError:pass
        else:process.kill()
        output,errors=process.communicate()
        raise subprocess.TimeoutExpired(command,timeout,output=output,stderr=errors)
    return subprocess.CompletedProcess(command,process.returncode,output,errors)
def files_manifest(root):
    root=pathlib.Path(root)
    return [{'path':p.relative_to(root).as_posix(),**identity(p)}for p in sorted(root.rglob('*'))if p.is_file()]
def check_manifest(root,manifest,exact=False):
    root=pathlib.Path(root).resolve();seen=set()
    for item in manifest:
        rel=relative(item['path']);require(item['path']not in seen,'duplicate manifest path');seen.add(item['path'])
        path=root/rel;require(path.is_file()and not path.is_symlink()and path.resolve().is_relative_to(root),'missing/aliased file');verify(path,item)
    if exact:require({p.relative_to(root).as_posix()for p in root.rglob('*')if p.is_file()}==seen,'missing/extra file membership')
