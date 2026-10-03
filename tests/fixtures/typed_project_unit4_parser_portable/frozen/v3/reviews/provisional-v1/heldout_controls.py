#!/usr/bin/env python3
"""Independent, local-only admission controls; never compile or run Rust."""
import contextlib,copy,json,os,subprocess,sys,time
from pathlib import Path

ROOT=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT/'snapshot'))
import portable as p
A=p.authority()
HIST=Path('/workspace/scratch/8266abf56995/oxid-parser-unit4-adapter')
CURRENT=Path('/workspace/scratch/8266abf56995/oxid-recovery-main')
TOOLCHAIN=Path('/workspace/scratch/8266abf56995/toolchains/rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu')
records=[]

@contextlib.contextmanager
def poison(values):
    old=os.environ.copy()
    try:
        os.environ.update(values)
        yield
    finally:
        os.environ.clear();os.environ.update(old)

def control(name,expected,observed,details):
    records.append({'name':name,'expected':expected,'observed':observed,'status':'pass' if observed==expected else 'fail','details':details})

# The known real directories remain read-only. Git identity values are obtained
# without repository/worktree/index/object/config variables in our own oracle.
clean=p.minimal_env(ROOT)
def real_git(path,*args):
    return subprocess.check_output(['/usr/bin/git','-C',str(path),*args],env=clean,text=True).strip()
truth={str(x):{'head':real_git(x,'rev-parse','HEAD'),'tree':real_git(x,'rev-parse','HEAD^{tree}')} for x in (HIST,CURRENT)}
for target,other in ((CURRENT,HIST),(HIST,CURRENT)):
    with poison({'GIT_DIR':real_git(other,'rev-parse','--absolute-git-dir')}):
        got=p.verify_checkout(target,A)
    observed={'head':got['head'],'tree':got['tree']}
    control('git_repository_override_'+target.name,truth[str(target)],observed,{'target':str(target),'poisoned_GIT_DIR':real_git(other,'rev-parse','--absolute-git-dir'),'source_map_still_pinned':True})

# Hard links copy the exact selected 62 Rust files into an isolated review root.
# None of the links is modified. Only the new unselected executable is written.
# Probe only command resolution; no Rust/C/Cargo compilation is performed.
clone=Path('/workspace/scratch/8266abf56995/portable-parser-review-selected-toolchain-plus-extra');clone.mkdir(exist_ok=True)
for entry in A['compiler_files']:
    dest=clone/entry['path'];dest.parent.mkdir(parents=True,exist_ok=True)
    if not dest.exists(): os.link(TOOLCHAIN/entry['path'],dest)
shadow=clone/'bin/which'
shadow.write_text('#!/bin/sh\nprintf "UNPINNED_WHICH_EXECUTED\\n"\n')
shadow.chmod(0o755)
try:
    p.verify_toolchain(clone,A)
    accepted=True
except p.Rejected:
    accepted=False
home=ROOT/'probe-home';home.mkdir(exist_ok=True)
probe=subprocess.run(['which','rustc'],env=p.minimal_env(home,ROOT/'unused-cache',clone),text=True,capture_output=True,timeout=5)
control('unselected_toolchain_executable_shadow',False,accepted and probe.stdout.strip()=='UNPINNED_WHICH_EXECUTED',{'selected_files':len(A['compiler_files']),'extra_path':str(shadow),'probe_argv':['which','rustc'],'probe_stdout':probe.stdout,'probe_exit':probe.returncode,'compiler_executions':0})

# Native subtool resolution inherits the same toolchain-first PATH. This is a
# harmless shadow script, not an assembler or compilation invocation.
native_shadow=clone/'bin/as'
native_shadow.write_text('#!/bin/sh\nprintf "UNPINNED_AS_EXECUTED\\n"\n')
native_shadow.chmod(0o755)
p.verify_toolchain(clone,A)
native_probe=subprocess.run(['as','--version'],env=p.minimal_env(home,ROOT/'unused-cache',clone),text=True,capture_output=True,timeout=5)
control('unselected_native_assembler_shadow',False,native_probe.stdout.strip()=='UNPINNED_AS_EXECUTED',{'selected_files':len(A['compiler_files']),'extra_path':str(native_shadow),'probe_argv':['as','--version'],'probe_stdout':native_probe.stdout,'probe_exit':native_probe.returncode,'compiler_executions':0,'qualification':'demonstrates native subtool PATH resolution, not a real compiler invocation'})

# The original derivation must preserve every AST node except the admission
# prefix of execution_manifest. Compare complete function bodies by independent
# Python AST decomposition, while pinning the full source identity first.
import ast
raw=(ROOT/'snapshot/frozen/comparator/comparator.py').read_text()
start=raw.index(p.PREFIX_START,raw.index('def execution_manifest(path, contract, approval):\n'))
end=raw.index(p.PREFIX_END,start)
replacement='    manifest, host, build, profile = portable_admission(path, contract, approval)\n'
derived=raw[:start]+replacement+raw[end:]
orig_tree=ast.parse(raw);new_tree=ast.parse(derived)
orig_functions={x.name:ast.dump(x,include_attributes=False) for x in orig_tree.body if isinstance(x,(ast.FunctionDef,ast.AsyncFunctionDef)) and x.name!='execution_manifest'}
new_functions={x.name:ast.dump(x,include_attributes=False) for x in new_tree.body if isinstance(x,(ast.FunctionDef,ast.AsyncFunctionDef)) and x.name!='execution_manifest'}
control('all_non_admission_function_asts_retained',orig_functions,new_functions,{'function_count':len(orig_functions)})
orig_function=next(x for x in orig_tree.body if isinstance(x,ast.FunctionDef) and x.name=='execution_manifest')
new_function=next(x for x in new_tree.body if isinstance(x,ast.FunctionDef) and x.name=='execution_manifest')
idx=next(i for i,x in enumerate(orig_function.body) if isinstance(x,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='cases' for t in x.targets))
control('raw_receipt_and_roster_suffix_asts_retained', [ast.dump(x,include_attributes=False) for x in orig_function.body[idx:]], [ast.dump(x,include_attributes=False) for x in new_function.body[1:]],{'retained_statement_count':len(new_function.body)-1})

# Keep concise results instead of serializing complete AST source strings.
for rec in records:
    if 'asts_retained' in rec['name']:
        rec['expected']='identical AST';rec['observed']='identical AST' if rec['status']=='pass' else 'different AST'
report={'schema':'oxid-portable-independent-heldout-controls-v1','scope':'local source/recipe review, no compiler or candidate execution','target_checkpoint_sha256':p.sha((ROOT/'snapshot/checkpoint.json').read_bytes()),'target_implementation_sha256':p.sha((ROOT/'snapshot/portable.py').read_bytes()),'authority_sha256':p.AUTHORITY_SHA,'records':records,'compiler_executions':0,'candidate_executions':0,'external_writes':0}
p.write(ROOT/'heldout-report.json',report)
print(json.dumps(report,indent=2))
raise SystemExit(any(x['status']=='fail' for x in records))
