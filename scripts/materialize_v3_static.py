#!/usr/bin/env python3
from pathlib import Path
import re,hashlib,json,sys
root=Path(__file__).resolve().parents[1]; base=root/'fixtures/typed-frontend-v3'; out=Path(sys.argv[1]); out.mkdir()
pending=['ast_static_main']; selected={}
while pending:
 name=pending.pop()
 if name in selected:continue
 options=[base/k/(name+'.ox') for k in ('static','shared')]; files=[p for p in options if p.exists()]
 if len(files)!=1:raise ValueError((name,files))
 data=files[0].read_bytes(); selected[name]={'path':str(files[0].relative_to(root)), 'sha256':hashlib.sha256(data).hexdigest()}; (out/(name+'.ox')).write_bytes(data); pending+=re.findall(r'\bmod\s+(\w+)\s*;',data.decode())
(out/'source-manifest.json').write_text(json.dumps(selected,indent=2)+'\n')
