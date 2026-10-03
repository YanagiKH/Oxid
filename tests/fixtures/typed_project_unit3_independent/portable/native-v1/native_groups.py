"""Exact source-only expansion of the frozen native controller's four stages."""
import pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parents[1]))
from common import *

GROUPS=('native-default','native-fuel','reference-fuel','driver','no-clobber')
def key(row):return tuple(row[name]for name in ('group','id','operation','format','fuel','noclobber'))
def roster(component):
    component=pathlib.Path(component)
    native=read_json(component/'native-requests.json');driver=read_json(component/'driver-requests.proposed.json');fuel=read_json(component/'fuel-requests.frozen.json');controls=read_json(component/'no-clobber-requests.json')['requests']
    require(len(native)==32 and len(driver)==12 and len(controls)==10 and sum(len(x['fuel_budgets'])for x in fuel)==18,'frozen native roster dimensions changed')
    requests={x['id']:x for x in native+driver};rows=[]
    def add(request,group,operation,format='json',budget=None,noclobber=None):
        rows.append({'group':group,'id':request['id'],'operation':operation,'format':format,'fuel':budget,'noclobber':noclobber,'request':request})
    for request in native:add(request,'native-default','compile')
    for request in fuel:
        for budget in request['fuel_budgets']:
            add(request,'native-fuel','compile',budget=budget);add(request,'reference-fuel','run','text',budget)
    for request in driver:
        for operation in ('check','run','compile'):
            for format in ('text','json'):add(request,'driver',operation,format)
    for control in controls:
        require(control['output_kind']in('regular','symlink'),'unknown no-clobber output kind')
        add(requests[control['id']],'no-clobber','compile',control['format'],noclobber='file'if control['output_kind']=='regular'else'symlink')
    require(len(rows)==150 and len({key(row)for row in rows})==150,'missing/duplicate frozen native dispatch')
    return rows

def select(component,group,case,operation,format,fuel,noclobber):
    require(group in GROUPS,'explicit frozen native group required')
    desired=(group,case,operation,format,fuel,noclobber)
    matches=[row for row in roster(component)if key(row)==desired]
    require(len(matches)==1,'invocation is not one exact frozen native roster row')
    return matches[0]['group'],matches[0]['request']
