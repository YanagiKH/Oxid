"""Dormant explicit CI sibling dispatch; historical facades are untouched."""
import hashlib,json,sys,types
from pathlib import Path
COMMON_SHA256='017a0b034ac120b246d2f9b88600cc3515534fe97c3e033b73a9f363e7b76e3d'
KEY='_oxid_package_dependency_unit4_ci_common_v1'
HERE=Path(__file__).resolve().parent
_COMMON=None
class Rejected(ValueError):pass
def common_api():
 global _COMMON
 raw=(HERE/'common.py').read_bytes()
 if type(COMMON_SHA256) is not str or hashlib.sha256(raw).hexdigest()!=COMMON_SHA256:raise Rejected('unsealed common helper')
 candidate=globals().get('_COMMON_OWNER') or _COMMON
 if candidate is not None:
  if sys.modules.get(KEY) is not candidate:raise Rejected('common owner slot changed')
  candidate.owner()
  if globals().get('_COMMON_OWNER') is not None:candidate.owns_sibling(sys.modules.get(__name__))
  return candidate
 if KEY in sys.modules:raise Rejected('unissued ambient common module')
 m=types.ModuleType(KEY);m.__file__=str(HERE/'common.py');sys.modules[KEY]=m
 try:exec(compile(raw,m.__file__,'exec'),m.__dict__)
 except BaseException:
  if sys.modules.get(KEY) is m:del sys.modules[KEY]
  raise
 _COMMON=m;m.owner();return m

def dispositions():
 q=common_api();q.package();d=q.decode(q.read(HERE/'consumer-dispositions.json'))
 q.need(d['schema']=='oxid-package-unit4-ci-consumer-dispositions-v1' and len(d['entries'])==173,'complete original consumer audit')
 original=q.decode(q.read(Path(__file__).resolve().parents[3]/d['predecessor']['path']))
 q.need(q.identity(Path(__file__).resolve().parents[3]/d['predecessor']['path'])['sha256']==d['predecessor']['sha256'],'retained original audit identity')
 q.need([r['id'] for r in d['entries']]==[r['id'] for r in original['entries']],'original ordered173 IDs')
 q.need(all(r['original']==old and r['disposition'] in ('executed','retained','blocked') and type(r['mandatory']) is bool for r,old in zip(d['entries'],original['entries'])),'explicit unchanged original dispositions')
 q.need(d['unresolved_mandatory_ids']==[r['id'] for r in d['entries'] if r['mandatory'] and r['disposition']=='blocked'],'complete mandatory blocker roster')
 return d

def dispatch(name,request):
 q=common_api();dispositions()
 if name=='ci-host':return q.load_sibling('gate').produce_host(request)
 if name=='ci-join':
  q.need(set(request)=={'repository','expected_head','event_sha','capsules'},'closed join facade request')
  context=q.expected_join(request['repository'],expected_head=request['expected_head'],event_sha=request['event_sha'])
  return q.load_sibling('join').join(context,request['capsules'])
 raise Rejected('no historical/current fallback for unknown facade route')
