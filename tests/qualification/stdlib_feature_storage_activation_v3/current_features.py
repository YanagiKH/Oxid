"""Separate fixed feature admission. This does not extend compiler authority."""
import hashlib
import json
from pathlib import Path
import types
ROOT = 'tests/qualification/stdlib_feature_storage_activation_v3/'
DESCRIPTOR = (4937, 'e25d24ec64d951e7607dddbb9780737bf28fa2abbf75a138006adfc685ee07c4')
GENERATION = (13680, '66ca7aee20a7d17b332ab3c2f8bad72d5882e7a52f07d86ebbd6ccaa85cae9ea')
NAMES = ('docs/API.md', 'docs/MODULES.md', 'stdlib/numbers.ox', 'tests/legacy_modules.rs', 'tests/stdlib_numbers.rs')
def need(ok, message):
    if not ok: raise ValueError(message)
def reader(repo):
    p=Path(repo)/'tests/qualification/package_json_current_v1/generation.py'
    raw=p.read_bytes()
    need((len(raw),hashlib.sha256(raw).hexdigest())==GENERATION,'fixed feature reader')
    m=types.ModuleType('fixed_feature_reader');m.__file__=str(p)
    exec(compile(raw,str(p),'exec'),m.__dict__)
    return m

def descriptor(repo):
    g=reader(repo);raw=g.regular(repo,ROOT+'current-feature-files-v1.json',DESCRIPTOR[0])
    need((len(raw),g.sha(raw))==DESCRIPTOR,'fixed current feature descriptor')
    return json.loads(raw)

def verify(repo, composition):
    g=reader(repo);value=descriptor(repo)
    need(type(composition) is str and composition in value['compositions'],'fixed feature composition')
    rows=value['compositions'][composition]
    for row in rows:
        raw=g.regular(repo,row['path'],row['bytes'])
        need(g.identity(row['path'],raw)=={k:row[k] for k in ('path','bytes','sha256')} and g.gitid('blob',raw)==row['git_blob'],'changed current feature: '+row['path'])
    present={r['path'] for r in rows}
    for name in set(NAMES)-present:
        p=Path(repo)/name
        need(not p.exists() and not p.is_symlink(),'unexpected current feature: '+name)
    return rows
