#!/usr/bin/env python3
"""HELD source-only diagnostic watchdog. No effects occur on import or validation.

Capacity functions are exact copies of the accepted anchored launcher source
023c8225fa7ea37a73c3d950e67eca25d8f46b9d2452a703ddebf3f1ca913b94.
Only explicitly approved expected-growth constants differ. Polling is not a
hard allocation/write-rate bound. Multi-session ownership derives from reviewed
watchdog 2b5d66e3070fd326e5c89d55702add3a2d8c57e9e34b0700ce1777d9a3e6d6d0.
Actual effects and tiny subprocess tests require a separate exact release.
"""
import argparse
import base64
import ctypes
import hashlib
import json
import math
import os
from pathlib import Path
import re
import select
import signal
import stat
import subprocess
import sys
import time

CAP = 8 * 1024 ** 2
PHASE_CAP = 65536
MAX_PROCESSES = 128
OWNERSHIP_LIMITS = dict(live_pidfds=128, lifetime_identities=65536)
TOMBSTONE_MEMORY_BUDGET = 32 * 1024**2
MAX_PROC_ENTRIES = 65536
CANONICAL_PYTHON = '/usr/bin/python3.13'
PYTHON_SHA256 = '889c603f0d17cb54060951bcf4c4f9b8c9ebd9e52b392c70209bbb9755d797d9'
CAPACITY_GROWTH_BYTES = 3221225472
CAPACITY_GROWTH_INODES = 40960
CAPACITY_ALARM_BYTES = 2281701376
CAPACITY_CLEANUP_INODES = 1024
CAPACITY_SOURCE_SHA256 = '023c8225fa7ea37a73c3d950e67eca25d8f46b9d2452a703ddebf3f1ca913b94'
CAPACITY_POLICY_SHA256 = '1e20ab6bacb61cfca538da0cebc9e070cd3d37ee739c40bcd0b8aec9dc7ed144'
LIMITS = dict(aggregate_seconds=21600, setup_seconds=2400, selected_public_seconds=5400,
              parser_seconds=12600, work_stop_seconds=20400, preservation_seconds=1140,
              preservation_stop_seconds=21540, cleanup_seconds=60, output_bytes=67108864)
CAPACITY = dict(expected_growth_bytes=CAPACITY_GROWTH_BYTES,
                expected_growth_inodes=CAPACITY_GROWTH_INODES,
                alarm_bytes=CAPACITY_ALARM_BYTES, alarm_inode_fraction=[1, 5],
                alarm_cleanup_inodes=CAPACITY_CLEANUP_INODES)
PHASES = (('setup','setup'), ('selected-public','selected-public'), ('parser','parser'),
          ('preservation','preservation'))
PHASE_KEYS = {'event','name','group','monotonic','deadline_monotonic',
              'aggregate_started_monotonic','work_started_monotonic','error'}
FLAGS = dict(diagnostic_only=True, local_complete=False, execution_qualified=False,
             collection_complete=False, section_complete=False)
ENV_KEYS = {'CARGO_HOME','HOME','LANG','LC_ALL','PATH','PYTHONDONTWRITEBYTECODE',
            'PYTHONNOUSERSITE','PYTHONOPTIMIZE','RUSTUP_HOME','RUSTUP_TOOLCHAIN','TMPDIR'}

class Rejected(ValueError):
    pass


def need(value, message):
    if not value:
        raise Rejected(message)


def canonical(value):
    return (json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + '\n').encode()


def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=pairs,
                      parse_constant=lambda _: need(False, 'nonfinite JSON'))


def real(path, exists=True):
    need(type(path) is str, 'string path required')
    value = Path(path)
    need(value.is_absolute() and value.resolve(strict=exists) == value and
         not any(p.is_symlink() for p in (value, *value.parents)),
         'canonical path without symlinks required')
    return value


def body(path, cap=CAP):
    path=real(str(path)); before=path.lstat()
    need(stat.S_ISREG(before.st_mode) and before.st_size<=cap,'bounded regular file required')
    fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
    first=None
    try:
        opened=os.fstat(fd)
        need(stat.S_ISREG(opened.st_mode) and input_identity(opened)==input_identity(before),
             'input descriptor identity changed')
        chunks=[]; size=0
        while True:
            chunk=os.read(fd,min(65536,cap+1-size))
            if not chunk:break
            chunks.append(chunk); size+=len(chunk)
            need(size<=cap,'input exceeded bound')
        need(input_identity(os.fstat(fd))==input_identity(opened)==input_identity(path.lstat()) and
             size==before.st_size,'input body or path changed')
        return b''.join(chunks)
    except BaseException as error:first=error;raise
    finally:
        try:os.close(fd)
        except BaseException as error:
            if first is None:raise
            first.add_note('input close: '+str(error)[:1024])


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def identity(path, cap=CAP):
    raw = body(path, cap)
    return {'path': str(path), 'bytes': len(raw), 'sha256': digest(raw)}


def exclusive_json(path, value):
    raw = canonical(value)
    need(len(raw) <= CAP, 'bounded evidence record')
    with Path(path).open('xb') as stream:
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())


def capacity_identity(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid)


def capacity_open(request, invocation, report):
    """Anchor the actual output filesystem; no tree scan or new directory."""
    parent = real(str(Path(request['run_root']).parent))
    evidence = real(str(Path(invocation['terminal_record_path']).parent))
    before = parent.lstat()
    fd = os.open(parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        opened = os.fstat(fd)
        need(stat.S_ISDIR(opened.st_mode) and capacity_identity(opened) == capacity_identity(before),
             'capacity parent changed while opening')
        output_identity = capacity_identity(opened)
        evidence_identity = capacity_identity(evidence.lstat())
        need(evidence_identity[0] == output_identity[0], 'output and evidence require one filesystem')
        report['capacity'] = {
            'schema': 'oxid-unit4-sampled-capacity-observation-v1',
            'parent': str(parent), 'parent_identity': list(output_identity),
            'evidence_parent': str(evidence), 'evidence_identity': list(evidence_identity),
            'sample_target_seconds': 0.1, 'samples': 0, 'maximum_observed_gap_seconds': 0.0,
            'first': None, 'last': None, 'minimum_bytes_sample': None,
            'minimum_inodes_sample': None, 'alarm_sample': None,
            'expected_growth_bytes': CAPACITY_GROWTH_BYTES,
            'expected_growth_inodes': CAPACITY_GROWTH_INODES,
            'alarm_bytes': CAPACITY_ALARM_BYTES, 'alarm_inodes': None,
            'hard_no_overshoot_guarantee': False,
            'risk': 'Sampled safeguard only; aggregate intersample/cleanup growth and other writers are unbounded.',
            'anchor_released': False}
        return {'fd': fd, 'parent': parent, 'evidence': evidence,
                'parent_identity': output_identity, 'evidence_identity': evidence_identity,
                'root': Path(request['run_root']), 'root_identity': None, 'filesystem': None}
    except BaseException as error:
        try:
            os.close(fd)
        except BaseException as close_error:
            error.add_note('capacity anchor close failed: ' + repr(close_error))
        raise


def capacity_sample(state, report, *, initial=False, phase='running'):
    try:
        return _capacity_sample(state, report, initial=initial)
    except BaseException as error:
        report['capacity'].setdefault('sample_failure', {
            'monotonic': time.monotonic(), 'wall_ns': time.time_ns(),
            'phase': 'prelaunch' if initial else phase,
            'error_type': type(error).__name__, 'reason': str(error)[:2048],
            'free_space_observation_available': False})
        raise


def _capacity_sample(state, report, *, initial=False):
    """One constant-cost sample. Equality is admitted; below either floor cancels."""
    observed = report['capacity']
    need(capacity_identity(os.fstat(state['fd'])) == state['parent_identity'] and
         capacity_identity(state['parent'].lstat()) == state['parent_identity'],
         'capacity output parent identity changed')
    need(capacity_identity(state['evidence'].lstat()) == state['evidence_identity'],
         'capacity evidence parent identity changed')
    if os.path.lexists(state['root']):
        info = state['root'].lstat()
        need(stat.S_ISDIR(info.st_mode) and info.st_dev == state['parent_identity'][0],
             'capacity output root filesystem/type changed')
        identity = capacity_identity(info)
        need(state['root_identity'] in (None, identity), 'capacity output root identity changed')
        state['root_identity'] = identity
    else:
        need(state['root_identity'] is None, 'capacity observed output root disappeared')
    disk = os.fstatvfs(state['fd'])
    filesystem = (getattr(disk, 'f_fsid', None), disk.f_frsize, disk.f_bsize, disk.f_files)
    need(disk.f_frsize == disk.f_bsize == 4096 and disk.f_files > 0,
         'capacity filesystem geometry changed')
    need(state['filesystem'] in (None, filesystem), 'capacity filesystem identity changed')
    state['filesystem'] = filesystem
    alarm_inodes = (disk.f_files + 4) // 5 + CAPACITY_CLEANUP_INODES
    sample = {'monotonic': time.monotonic(), 'wall_ns': time.time_ns(),
              'available_blocks': disk.f_bavail, 'available_bytes': disk.f_bavail * disk.f_frsize,
              'available_inodes': disk.f_favail, 'fragment_bytes': disk.f_frsize,
              'filesystem_id': filesystem[0], 'total_inodes': disk.f_files, 'initial': initial}
    previous = observed['last']
    if previous is not None:
        observed['maximum_observed_gap_seconds'] = max(observed['maximum_observed_gap_seconds'],
                                                       sample['monotonic'] - previous['monotonic'])
    observed['samples'] += 1
    observed['first'] = observed['first'] or sample
    observed['last'] = sample
    observed['alarm_inodes'] = alarm_inodes
    for key, field in (('minimum_bytes_sample', 'available_bytes'), ('minimum_inodes_sample', 'available_inodes')):
        if observed[key] is None or sample[field] < observed[key][field]:
            observed[key] = sample
    required_bytes = CAPACITY_ALARM_BYTES + (CAPACITY_GROWTH_BYTES if initial else 0)
    required_inodes = alarm_inodes + (CAPACITY_GROWTH_INODES if initial else 0)
    if sample['available_bytes'] < required_bytes or sample['available_inodes'] < required_inodes:
        observed['alarm_sample'] = observed['alarm_sample'] or sample
        reason = 'capacity ' + ('prelaunch reserve unavailable' if initial else 'sampled free-space/inode alarm')
        observed.setdefault('first_alarm_reason', reason)
        return reason
    return None


def capacity_close(state, report):
    os.close(state['fd'])
    report['capacity']['anchor_released'] = True


def input_identity(info):
    return tuple(getattr(info,k) for k in ('st_dev','st_ino','st_mode','st_uid','st_gid','st_size','st_mtime_ns','st_ctime_ns'))


def pinned(row, *, keep_fd=False, checkpoint=lambda:None):
    """Read exact bytes from a retained descriptor; never execute reopened source."""
    need(type(row) is dict and set(row)=={'path','bytes','sha256'}, 'closed file pin')
    need(type(row['bytes']) is int and 0 <= row['bytes'] <= 16*1024**2 and
         type(row['sha256']) is str and re.fullmatch('[0-9a-f]{64}',row['sha256']), 'bounded exact pin')
    checkpoint()
    path=real(row['path']); before=path.lstat()
    fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_CLOEXEC|os.O_NONBLOCK)
    first=None
    try:
        opened=os.fstat(fd)
        need(stat.S_ISREG(opened.st_mode) and input_identity(opened)==input_identity(before) and opened.st_size==row['bytes'], 'pinned input identity')
        chunks=[]; remaining=row['bytes']+1
        while remaining:
            checkpoint()
            chunk=os.read(fd,min(65536,remaining))
            if not chunk:break
            chunks.append(chunk); remaining-=len(chunk)
        raw=b''.join(chunks)
        need(input_identity(os.fstat(fd))==input_identity(opened) and input_identity(path.lstat())==input_identity(opened) and len(raw)==row['bytes'] and
             digest(raw)==row['sha256'], 'pinned input bytes changed')
        checkpoint()
        if keep_fd:return raw,fd
        return raw
    except BaseException as error:
        first=error; raise
    finally:
        if not keep_fd or first is not None:
            try:os.close(fd)
            except BaseException as error:
                if first is None:raise
                first.add_note('pinned descriptor close: '+str(error)[:1024])


def validate(review_path,review_sha256):
    """Effects-blocked authentication; no modules with live owners are imported."""
    need(type(review_sha256) is str and re.fullmatch('[0-9a-f]{64}',review_sha256), 'exact review pin')
    raw=body(review_path)
    need(digest(raw)==review_sha256, 'review changed')
    review=decode(raw)
    need(type(review) is dict and set(review)=={'schema','status','manifest','launcher','limits',
         'capacity','environment','ownership_limits','no_automatic_retry'}, 'closed exact release review')
    need(review['schema']=='oxid-fresh-downstream-diagnostic-review-v1' and
         review['status']=='APPROVED_ONE_SHOT' and review['no_automatic_retry'] is True,
         'diagnostic effects HELD pending exact one-shot release')
    need(review['limits']==LIMITS and review['capacity']==CAPACITY and review['ownership_limits']==OWNERSHIP_LIMITS, 'fixed reviewed budgets')
    need(review['launcher']['path']==str(Path(__file__).resolve()), 'exact launcher location')
    pinned(review['launcher'])
    manifest_raw=pinned(review['manifest']); manifest=decode(manifest_raw)
    need(type(manifest) is dict and set(manifest)=={'schema','mode','pins','installed','paths','limits','capacity','environment','ownership_limits'},
         'closed input manifest')
    need(manifest['schema']=='oxid-fresh-downstream-diagnostic-input-v1' and manifest['mode']=='fresh-downstream' and
         manifest['limits']==LIMITS and manifest['capacity']==CAPACITY and manifest['ownership_limits']==OWNERSHIP_LIMITS, 'exact fresh-owner diagnostic manifest')
    pins=manifest['pins']
    need(type(pins) is dict and set(pins)=={'driver','worker','session','python','request','installed_admission',
         'capacity_launcher','capacity_policy'}, 'closed input pin roster')
    captured={name:pinned(row) for name,row in pins.items()}
    need(pins['python']['path']==CANONICAL_PYTHON and pins['python']['sha256']==PYTHON_SHA256,
         'canonical Python 3.13 pin')
    need(pins['capacity_launcher']['sha256']==CAPACITY_SOURCE_SHA256 and
         pins['capacity_policy']['sha256']==CAPACITY_POLICY_SHA256, 'accepted capacity source and policy')
    installed=manifest['installed']
    need(type(installed) is dict and set(installed)=={'repository','head','tree'} and
         all(type(installed[k]) is str and re.fullmatch('[0-9a-f]{40}',installed[k]) for k in ('head','tree')),
         'exact release-bound installed source identity')
    repo=real(installed['repository']); paths=manifest['paths']
    need(type(paths) is dict and set(paths)=={'run_root','evidence_root'}, 'closed output paths')
    roots=[real(paths[key],False) for key in ('run_root','evidence_root')]
    for root in roots:
        need(not os.path.lexists(root) and root.parent.is_dir(), 'fresh output roots')
        need(root!=repo and repo not in root.parents and root not in repo.parents, 'external output roots')
    need(roots[0]!=roots[1] and roots[0] not in roots[1].parents and roots[1] not in roots[0].parents,
         'disjoint output roots')
    request=decode(captured['request'])
    need(type(request) is dict and set(request)=={'schema','scope','expected_head','event_sha','run_root',
         'original_repo','historical_repo','public','parser'} and
         request['schema']=='oxid-unit4-local-session-request-v1' and request['scope']=='combined' and
         request['expected_head']==request['event_sha']==installed['head'] and
         request['run_root']==paths['run_root'], 'bound canonical combined request')
    env=review['environment']
    need(type(env) is dict and set(env)==ENV_KEYS and all(type(v) is str and '\0' not in v for v in env.values()) and
         env==manifest['environment'], 'closed exact clean environment')
    need(env['PYTHONDONTWRITEBYTECODE']=='1' and env['PYTHONNOUSERSITE']=='1' and env['PYTHONOPTIMIZE']=='0' and
         env['LANG']==env['LC_ALL']=='C.UTF-8' and env['RUSTUP_TOOLCHAIN']=='1.99.0' and
         env['HOME']==str(roots[0]/'home') and env['RUSTUP_HOME']==str(roots[0]/'rustup') and
         env['TMPDIR']==str(roots[0]/'tmp'), 'fixed interpreter and fresh toolchain environment')
    return dict(review=review,review_path=str(real(review_path)),review_sha256=review_sha256,
                manifest=manifest,request=request,driver=captured['driver'],captured=captured)


def phase_bytes(path,previous=b''):
    if not os.path.lexists(path):
        need(not previous,'phase file disappeared'); return previous
    fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC); first=None
    try:
        before=os.fstat(fd)
        need(stat.S_ISREG(before.st_mode) and before.st_size<=PHASE_CAP,'bounded regular phase log')
        raw=os.read(fd,PHASE_CAP+1); after=os.fstat(fd)
        need(len(raw)<=PHASE_CAP and (before.st_dev,before.st_ino)==(after.st_dev,after.st_ino) and
             (path.lstat().st_dev,path.lstat().st_ino)==(before.st_dev,before.st_ino), 'phase log identity changed')
        complete=raw[:raw.rfind(b'\n')+1]
        need(complete.startswith(previous), 'append-only completed phase prefix changed')
        return complete
    except BaseException as error:first=error; raise
    finally:
        try:os.close(fd)
        except BaseException as error:
            if first is None:raise
            first.add_note('phase close: '+str(error)[:1024])


def phase_deadline(name,group,phase_start,started,work):
    if group=='setup':return min(started+2400,started+20400)
    if group in ('selected-public','parser'):
        return min(phase_start+(5400 if group=='selected-public' else 12600),started+20400)
    return min(phase_start+1140,started+21540)


def phase_state(raw,started,now):
    """Whole bounded prefix, strict sequential phases; no repeated-start reset."""
    active=None; completed=[]; work=None; previous=started; last=None
    for line in raw.splitlines():
        need(0<len(line)<=8192,'bounded phase marker')
        row=decode(line)
        need(type(row) is dict and set(row)==PHASE_KEYS|set(FLAGS) and all(row[k] is v for k,v in FLAGS.items()) and row['event'] in ('start','end') and
             (row['name'],row['group']) in PHASES, 'closed phase marker')
        value=row['monotonic']
        need(type(value) in (int,float) and math.isfinite(value) and previous<=value<=now and
             row['aggregate_started_monotonic']==started, 'original monotonic phase budget')
        need(row['error'] is None or type(row['error']) in (dict,str), 'bounded phase error')
        previous=value; last=row
        if row['event']=='start':
            need(active is None and row['name'] not in completed, 'no overlapping/repeated phase')
            index=PHASES.index((row['name'],row['group']))
            need(index==len(completed) or (row['group']=='preservation' and completed), 'ordered work phases')
            if row['group'] in ('selected-public','parser') and work is None:work=value
            deadline=phase_deadline(row['name'],row['group'],value,started,work)
            need(row['deadline_monotonic']==deadline, 'unaltered phase deadline')
            active=row
        else:
            need(active is not None and row['name']==active['name'] and row['group']==active['group'] and
                 row['deadline_monotonic']==active['deadline_monotonic'], 'phase end matches start')
            need(value<=active['deadline_monotonic'] or row['error'] is not None, 'expired phase cannot report success')
            completed.append(row['name']); active=None
        need(row['work_started_monotonic']==work, 'unaltered aggregate work clock')
    if active is not None:deadline=active['deadline_monotonic']
    elif last is None:deadline=started+2400
    elif last['group']=='preservation':deadline=last['deadline_monotonic']
    elif work is not None:deadline=min(last['deadline_monotonic'],started+20400)
    else:deadline=min(started+2400,started+20400)
    return dict(active=active,last=last,completed=completed,deadline=deadline)


def process_table(deadline):
    """Reviewed bounded /proc inventory, with identity-specific failure labels."""
    rows={}; count=0
    for path in Path('/proc').iterdir():
        count+=1; need(count<=MAX_PROC_ENTRIES and time.monotonic()<deadline,'bounded process inventory')
        if not path.name.isdigit():continue
        try:rows[int(path.name)]=process_identity(int(path.name))
        except (FileNotFoundError,ProcessLookupError):continue
        except BaseException as error:
            error.add_note('process inventory pid='+path.name); raise
    return rows


def process_identity(pid):
    with (Path('/proc')/str(pid)/'stat').open('rb') as stream:raw=stream.read(65537)
    need(len(raw)<=65536 and raw.split(b' ',1)[0]==str(pid).encode(),'bounded exact process stat')
    end=raw.rfind(b')'); need(end>0,'process stat format'); tail=raw[end+2:].split()
    need(len(tail)>=22,'complete process stat')
    return dict(pid=pid,ppid=int(tail[1]),pgid=int(tail[2]),sid=int(tail[3]),
                start_ticks=int(tail[19]),state=tail[0].decode())


def same(row,old):
    return row is not None and all(row[key]==old[key] for key in ('pid','start_ticks'))


def pidfd_exited(fd):
    poller=select.poll(); poller.register(fd,select.POLLIN)
    events=poller.poll(0)
    need(all(flags & (select.POLLIN|select.POLLHUP) and not flags & (select.POLLERR|select.POLLNVAL)
             for _,flags in events),'valid retained pidfd')
    return bool(events)


def become_subreaper(deadline):
    """Dedicated launcher creates exactly one child and no other child sources."""
    table=process_table(deadline)
    need(not any(row['ppid']==os.getpid() for row in table.values()),'empty initial supervisor child roster')
    need(len(list((Path('/proc')/str(os.getpid())/'task').iterdir()))==1,'single-thread dedicated supervisor')
    libc=ctypes.CDLL(None,use_errno=True)
    need(libc.prctl(36,1,0,0,0)==0,'Linux child subreaper setup')
    state=ctypes.c_int()
    need(libc.prctl(37,ctypes.byref(state),0,0,0)==0 and state.value==1,'verified child subreaper')


def remember(report,phase,error):
    try:message=str(error)[:2048]
    except BaseException:message='<exception text unavailable>'
    row=dict(phase=phase,type=type(error).__name__,message=message)
    if report['first_error'] is None:report['first_error']=row
    elif len(report['secondary_errors'])<16:report['secondary_errors'].append(row)


def ownership_state(supervisor):
    return dict(supervisor=supervisor,owned={},producer=None,bootstrap_pidfd=None,
                retired=set(),retired_digest=hashlib.sha256(),retired_payload_bytes=0,
                retirement_close_errors=0,identity_count=0)


def retire_exited(state,table):
    """Retire only kernel-terminal, already-absent nonproducer identities.

    No numeric descriptor remains eligible after close is attempted, even when
    close fails. Tombstones never regain authority. The 65,536 ceiling is a
    fail-closed diagnostic resource policy, not a proved eight-build maximum.
    """
    for key,item in list(state['owned'].items()):
        if key==state['producer'] or item['reaped']:continue
        if same(table.get(key[0]),item['identity']) or not pidfd_exited(item['fd']):continue
        need(key not in state['retired'],'retired birth identity cannot regain authority')
        size=sys.getsizeof(key)+sum(sys.getsizeof(n) for n in key)
        # Allow worst-case next set resize in the admission check. Exact current
        # Python object allocation is reported separately from file bytes.
        need(state['retired_payload_bytes']+size+4*max(1024,sys.getsizeof(state['retired']))<=TOMBSTONE_MEMORY_BUDGET,
             'tombstone memory resource ceiling')
        state['retired'].add(key); state['retired_payload_bytes']+=size
        state['retired_digest'].update((str(key[0])+':'+str(key[1])+'\n').encode('ascii'))
        del state['owned'][key]
        fd=item['fd']; item['fd']=None; item['reaped']=True
        try:os.close(fd)
        except BaseException:
            state['retirement_close_errors']+=1
            raise


def acquire_owned(state,row,parent):
    key=(row['pid'],row['start_ticks'])
    need(key not in state['retired'],'retired birth identity cannot regain authority')
    if key in state['owned']:return
    need(len(state['owned'])<OWNERSHIP_LIMITS['live_pidfds'],'retained live pidfd resource ceiling')
    need(state['identity_count']<OWNERSHIP_LIMITS['lifetime_identities'],'lifetime identity resource ceiling')
    fd=os.pidfd_open(row['pid'],0)
    try:
        current=process_identity(row['pid'])
        need(same(current,row) and current['ppid'] in (parent['pid'],state['supervisor']['pid']) and
             current['start_ticks']>=parent['start_ticks'],'birth and parent association during pidfd capture')
        if current['ppid']==parent['pid']:
            need(same(process_identity(parent['pid']),parent),'owned parent birth changed during capture')
        state['owned'][key]=dict(identity=dict(current),fd=fd,reaped=False,signals=[])
        state['identity_count']+=1
    except BaseException as error:
        try:os.close(fd)
        except BaseException as close_error:error.add_note('pidfd close: '+str(close_error)[:1024])
        raise


def discover_owned(state,deadline):
    table=process_table(deadline); supervisor=state['supervisor']
    need(same(table.get(supervisor['pid']),supervisor),'supervisor birth identity changed')
    retire_exited(state,table)
    changed=True
    while changed:
        changed=False
        parents={supervisor['pid']:supervisor}
        for key,item in state['owned'].items():
            if same(table.get(key[0]),item['identity']):parents[key[0]]=table[key[0]]
        for row in table.values():
            key=(row['pid'],row['start_ticks'])
            if row['ppid'] in parents and key not in state['owned']:
                need(key not in state['retired'],'retired identity reappeared in owned ancestry')
                need(time.monotonic()<deadline,'owned discovery deadline')
                try:acquire_owned(state,row,parents[row['ppid']])
                except (FileNotFoundError,ProcessLookupError):continue
                changed=True
    return [row for row in table.values() if (row['pid'],row['start_ticks']) in state['owned']]


def signal_owned(state,signum,report,*,producer_only=False):
    """Never address a numeric PID/group and never signal after an owned reap."""
    for key,item in list(state['owned'].items()):
        if item['reaped'] or (producer_only and key!=state['producer']):continue
        if signum in item['signals']:continue
        try:
            if pidfd_exited(item['fd']):continue
            signal.pidfd_send_signal(item['fd'],signum,None,0)
            item['signals'].append(signum)
            if len(report['signals'])<MAX_PROCESSES*3:
                report['signals'].append(dict(pid=key[0],start_ticks=key[1],signal=int(signum)))
        except ProcessLookupError:pass
        except BaseException as error:remember(report,'signal-owned-'+str(key),error)


def retain_producer(state,pid):
    # Open before reading proc. The descriptor remains available even if every
    # subsequent identity read/report fails. It can only target this child.
    if state['bootstrap_pidfd'] is None:state['bootstrap_pidfd']=os.pidfd_open(pid,0)
    row=process_identity(pid)
    need(row['ppid']==state['supervisor']['pid'] and row['pid']==row['pgid']==row['sid'],
         'original direct producer session')
    current=process_identity(pid)
    need(same(current,row) and all(current[k]==row[k] for k in ('ppid','pgid','sid')),
         'producer birth association changed')
    key=(pid,row['start_ticks'])
    state['owned'][key]=dict(identity=row,fd=state['bootstrap_pidfd'],reaped=False,signals=[])
    state['bootstrap_pidfd']=None; state['producer']=key; state['identity_count']+=1
    return row


def peek_producer(state):
    item=state['owned'][state['producer']]
    need(not item['reaped'],'producer already reaped')
    return os.waitid(os.P_PIDFD,item['fd'],os.WEXITED|os.WNOHANG|os.WNOWAIT)


def cleanup_owned(state,report,deadline):
    """Finite freeze/discovery fixed point, kill, terminal proof, then reap.

    A pidfd exit alone never proves absence of unobserved descendants. Discovery
    errors do not suppress signals to already-retained kernel identities.
    """
    fixed=False; prior=None; stable=0; discovery_errors=False
    for _ in range(64):
        if time.monotonic()>=deadline:break
        try:
            members=discover_owned(state,deadline)
            keys={(r['pid'],r['start_ticks']) for r in members}
            stopped=all(r['state'] in ('T','t','Z','X') for r in members)
            stable=stable+1 if stopped and keys==prior else 0
            prior=keys
            if stable>=1:fixed=True; break
        except BaseException as error:
            discovery_errors=True; stable=0; remember(report,'cleanup-discovery',error)
        signal_owned(state,signal.SIGSTOP,report)
        time.sleep(min(.02,max(0,deadline-time.monotonic())))
    if not fixed:remember(report,'cleanup-fixed-point',Rejected('stopped creator fixed point unverified'))
    # Always attempt retained-identity termination, including failed discovery.
    signal_owned(state,signal.SIGKILL,report)
    terminal=False
    while time.monotonic()<deadline:
        try:
            count=state['identity_count']; members=discover_owned(state,deadline)
            if state['identity_count']!=count:
                fixed=False
                remember(report,'cleanup-late-discovery',Rejected('new identity after stop fixed point'))
            signal_owned(state,signal.SIGKILL,report)
            terminal=all(pidfd_exited(item['fd']) for item in state['owned'].values())
            if terminal and not any(row['state'] not in ('Z','X') for row in members):break
        except BaseException as error:
            discovery_errors=True; remember(report,'cleanup-terminal-discovery',error)
            signal_owned(state,signal.SIGKILL,report)
        time.sleep(min(.02,max(0,deadline-time.monotonic())))
    # Reap only after attempts to terminate every known creator. WNOWAIT above
    # kept the original worker PID unavailable for reuse throughout signalling.
    reaped=[]
    for key,item in state['owned'].items():
        try:
            need(time.monotonic()<deadline and pidfd_exited(item['fd']),'owned terminal exit unverified')
            try:
                info=os.waitid(os.P_PIDFD,item['fd'],os.WEXITED|os.WNOHANG|os.WNOWAIT)
                need(info is not None,'owned child exit not ready')
                result=os.waitid(os.P_PIDFD,item['fd'],os.WEXITED|os.WNOHANG)
                need(result is not None and result.si_pid==key[0],'exact owned child reap')
                item['reaped']=True
                if key==state['producer']:
                    report['returncode']=result.si_status if result.si_code==os.CLD_EXITED else -result.si_status
            except ChildProcessError:
                current=None
                try:current=process_identity(key[0])
                except (FileNotFoundError,ProcessLookupError):pass
                need(not same(current,item['identity']),'nonchild original identity still present')
                item['reaped']=True  # Its actual parent reaped it; pidfd already terminal.
            reaped.append(dict(pid=key[0],start_ticks=key[1],verified=True))
        except BaseException as error:remember(report,'cleanup-reap-'+str(key),error)
    absent=False
    try:
        remaining=discover_owned(state,deadline)
        absent=not remaining and not any(r['ppid']==state['supervisor']['pid'] for r in process_table(deadline).values())
    except BaseException as error:remember(report,'cleanup-final-roster',error)
    result=dict(stopped_fixed_point=fixed,discovery_errors=discovery_errors,all_retained_terminal=terminal,
                reaped_identities=reaped,retired_identities=dict(count=len(state['retired']),
                    sha256=state['retired_digest'].hexdigest(),encoding='ordered PID:start_ticks newline',
                    terminal_and_absence_verified=True,close_errors=state['retirement_close_errors'],
                    memory_bytes=state['retired_payload_bytes']+sys.getsizeof(state['retired']),
                    memory_budget_bytes=TOMBSTONE_MEMORY_BUDGET),
                ownership_limits=OWNERSHIP_LIMITS,limits_are_proven_setup_maximum=False,
                descendant_absence_verified=absent and fixed and not discovery_errors,
                complete=absent and fixed and terminal and not discovery_errors and not state['retirement_close_errors'] and
                         all(item['reaped'] for item in state['owned'].values()))
    return result


def output_bytes(root,*,terminal=False):
    total=0; count=0
    for path in root.iterdir():
        count+=1; need(count<=16,'bounded diagnostic evidence member count')
        info=path.lstat(); need(stat.S_ISREG(info.st_mode),'regular diagnostic evidence only')
        total+=info.st_size
    need(total<=LIMITS['output_bytes']-(512*1024 if terminal else 1024**2),'diagnostic evidence cap with terminal reserve')
    return total


def worker_result(root,report):
    path=root/'partial-downstream-diagnostic.json'
    need(os.path.lexists(path),'worker terminal receipt absent')
    value=decode(body(path,1024**2))
    need(type(value) is dict and value.get('schema')=='oxid-partial-downstream-diagnostic-v1' and all(value.get(k) is v for k,v in FLAGS.items()),'worker diagnostic-only terminal flags')
    report['worker_terminal']=value
    failure=value.get('failure')
    if failure is not None:
        need(type(failure) is dict and type(failure.get('message')) is str and type(failure.get('type')) is str,
             'worker first-failure schema')
        worker=dict(phase='worker',type=failure['type'],message=failure['message'][:2048])
        # Original producer failure has precedence over reporting/cleanup fallout.
        prior=report['first_error']
        if prior is None or prior['phase'].startswith(('cleanup','terminal','worker-receipt')):
            if prior is not None and len(report['secondary_errors'])<16:report['secondary_errors'].append(prior)
            report['first_error']=worker
        elif prior!=worker and len(report['secondary_errors'])<16:report['secondary_errors'].append(worker)
    return value


def command(auth,started,python_fd):
    driver=auth['driver']; manifest=auth['manifest']
    payload={'manifest':manifest,'request':auth['request'],'started_monotonic':started}
    bootstrap=("import base64,json,sys; p=sys.argv[1]; b=base64.b64decode(sys.argv[2],validate=True); "
       "inputs=json.loads(base64.b64decode(sys.argv[3],validate=True)); sys.argv=[p,'--worker']; "
       "exec(compile(b,p,'exec'),{'__name__':'__main__','__file__':p,"
       "'_AUTHENTICATED_DRIVER_BYTES':b,'_AUTHENTICATED_INPUTS':inputs})")
    argv=[CANONICAL_PYTHON,'-I','-B','-c',bootstrap,manifest['pins']['driver']['path'],
          base64.b64encode(driver).decode(),base64.b64encode(canonical(payload)).decode()]
    need(all(len(os.fsencode(arg))<120000 for arg in argv) and sum(len(os.fsencode(arg))+1 for arg in argv)<256000,
         'bounded exact authenticated bootstrap arguments')
    return argv


def run(review_path,review_sha256,*,execute=False):
    started=time.monotonic()
    need(execute is True,'diagnostic execution remains HELD without --execute and exact release')
    auth=validate(review_path,review_sha256)
    need(sys.platform=='linux' and sys.version_info[:3]==(3,13,5) and sys.dont_write_bytecode and
         not sys.flags.optimize and hasattr(os,'pidfd_open') and hasattr(signal,'pidfd_send_signal') and
         hasattr(os,'P_PIDFD'),'isolated normal canonical Linux Python 3.13 required')
    manifest=auth['manifest']; root=Path(manifest['paths']['evidence_root'])
    report=dict(FLAGS,schema='oxid-fresh-downstream-watchdog-terminal-v1',review_sha256=review_sha256,
                first_error=None,secondary_errors=[],signals=[],returncode=None,cleanup=None,
                automatic_retry=False,started_monotonic=started,producer_succeeded=False)
    python_fd=None; child=None; capacity=None; streams={}; previous={}; pending=[]; state=None
    phase_raw=b''; cancel_at=None; primary_exception=None
    def cancelled(signum,frame):
        if not pending:pending.append('supervisor '+signal.Signals(signum).name)
    try:
        _,python_fd=pinned(manifest['pins']['python'],keep_fd=True)
        need(os.access(CANONICAL_PYTHON,os.X_OK),'canonical Python executable')
        become_subreaper(started+2400)
        state=ownership_state(process_identity(os.getpid()))
        for sig in (signal.SIGTERM,signal.SIGINT):previous[sig]=signal.signal(sig,cancelled)
        root.mkdir(mode=0o700)
        capacity=capacity_open(auth['request'],{'terminal_record_path':str(root/'watchdog-terminal.json')},report)
        reason=capacity_sample(capacity,report,initial=True)
        need(reason is None,reason or 'capacity reserve')
        for name in ('stdout','stderr'):streams[name]=(root/(name+'.log')).open('xb')
        need(time.monotonic()<started+2400,'setup budget exhausted before producer launch')
        argv=command(auth,started,python_fd)
        child=subprocess.Popen(argv,executable=f'/proc/self/fd/{python_fd}',pass_fds=(python_fd,),
              stdin=subprocess.DEVNULL,stdout=streams['stdout'],stderr=streams['stderr'],
              cwd=manifest['installed']['repository'],env=manifest['environment'],
              shell=False,close_fds=True,start_new_session=True)
        # No Popen.poll or wait occurs until cleanup has addressed nested sessions.
        row=retain_producer(state,child.pid)
        report['original_producer_identity']=row
        exclusive_json(root/'watchdog-start.json',dict(FLAGS,started_monotonic=started,
                       review_sha256=review_sha256,original_producer_identity=row,
                       limits=LIMITS,capacity=CAPACITY,ownership_limits=OWNERSHIP_LIMITS,environment=manifest['environment']))
        while peek_producer(state) is None:
            now=time.monotonic()
            need(now<started+21540,'aggregate preservation stop')
            if pending:raise Rejected(pending[0])
            phase_raw=phase_bytes(root/'phase.jsonl',phase_raw)
            now=time.monotonic()
            phase=phase_state(phase_raw,started,now)
            need(now<phase['deadline'],'original synchronous phase/aggregate deadline')
            discover_owned(state,min(now+1,started+21540))
            reason=capacity_sample(capacity,report)
            need(reason is None,reason or 'capacity sample')
            output_bytes(root)
            time.sleep(.1)
    except BaseException as error:
        primary_exception=error; remember(report,'supervisor',error); cancel_at=time.monotonic()
    finally:
        # First give only the original producer the canonical TERM/unwind path.
        if child is not None and state is not None:
            try:
                if state['producer'] is None:
                    retain_producer(state,child.pid)
                if peek_producer(state) is None:
                    cancel_at=cancel_at if cancel_at is not None else time.monotonic()
                    signal_owned(state,signal.SIGTERM,report,producer_only=True)
                    grace=min(cancel_at+1140,started+21540)
                    # If preservation was already running, it gets no new clock.
                    try:
                        last=phase_state(phase_raw,started,time.monotonic())
                        if last['last'] and last['last']['group']=='preservation':grace=min(grace,last['deadline'])
                    except BaseException as error:remember(report,'cancel-phase-readback',error)
                    while time.monotonic()<grace and peek_producer(state) is None:
                        try:
                            discover_owned(state,min(grace,time.monotonic()+1))
                            reason=capacity_sample(capacity,report,phase='cancellation')
                            if reason is not None:remember(report,'cancel-capacity',Rejected(reason))
                            output_bytes(root)
                        except BaseException as error:remember(report,'cancel-observation',error)
                        time.sleep(min(.1,max(0,grace-time.monotonic())))
            except BaseException as error:
                remember(report,'cleanup-producer-cancellation',error)
                if state['bootstrap_pidfd'] is not None:
                    try:signal.pidfd_send_signal(state['bootstrap_pidfd'],signal.SIGTERM,None,0)
                    except ProcessLookupError:pass
                    except BaseException as secondary:remember(report,'cleanup-original-unbound-term',secondary)
                    grace=min((cancel_at if cancel_at is not None else time.monotonic())+1140,started+21540)
                    while time.monotonic()<grace:
                        try:
                            if pidfd_exited(state['bootstrap_pidfd']):break
                            discover_owned(state,min(grace,time.monotonic()+1))
                        except BaseException as secondary:remember(report,'cleanup-unbound-observation',secondary)
                        time.sleep(min(.1,max(0,grace-time.monotonic())))
            cleanup_deadline=min(started+21600,time.monotonic()+60)
            if state['bootstrap_pidfd'] is not None:
                try:signal.pidfd_send_signal(state['bootstrap_pidfd'],signal.SIGKILL,None,0)
                except ProcessLookupError:pass
                except BaseException as error:remember(report,'cleanup-original-unbound-kill',error)
            try:
                report['cleanup']=cleanup_owned(state,report,cleanup_deadline)
                if not report['cleanup']['complete']:remember(report,'cleanup-unverified',Rejected('complete descendant cleanup unverified'))
                if any(row['signal'] in (signal.SIGSTOP,signal.SIGKILL) for row in report['signals']):
                    remember(report,'cleanup-forced',Rejected('forced cleanup was required'))
            except BaseException as error:
                remember(report,'cleanup',error)
                # Even an unexpected reporting failure cannot skip retained kills.
                signal_owned(state,signal.SIGKILL,report)
            if child is not None and report['returncode'] is not None:
                child.returncode=report['returncode']  # waitid already reaped the original Popen child.
            try:worker_result(root,report)
            except BaseException as error:remember(report,'worker-receipt',error)
        if capacity is not None:
            try:
                reason=capacity_sample(capacity,report,phase='finalization')
                if reason is not None:remember(report,'terminal-capacity',Rejected(reason))
            except BaseException as error:remember(report,'terminal-capacity',error)
            try:capacity_close(capacity,report)
            except BaseException as error:remember(report,'terminal-capacity-close',error)
        for name,stream in streams.items():
            for action in ('flush','fsync','close'):
                try:
                    if action=='fsync':os.fsync(stream.fileno())
                    else:getattr(stream,action)()
                except BaseException as error:remember(report,'terminal-log-'+name+'-'+action,error)
        if state is not None:
            if state['bootstrap_pidfd'] is not None:
                try:os.close(state['bootstrap_pidfd'])
                except BaseException as error:remember(report,'terminal-unbound-pidfd',error)
            for key,item in state['owned'].items():
                try:os.close(item['fd'])
                except BaseException as error:remember(report,'terminal-pidfd-'+str(key),error)
        if python_fd is not None:
            try:os.close(python_fd)
            except BaseException as error:remember(report,'terminal-python-fd',error)
        for sig,handler in previous.items():
            try:signal.signal(sig,handler)
            except BaseException as error:remember(report,'terminal-signal-restore',error)
        # Independent terminal pin/report attempts use only the remaining original
        # aggregate budget; no timeout extension or post-reap signalling occurs.
        report['input_preservation']={'verified':False}
        try:
            need(time.monotonic()<started+21600,'original aggregate final readback deadline')
            need(digest(body(auth['review_path']))==review_sha256,'review terminal pin')
            def check_final_input():
                need(time.monotonic()<started+21600,'original aggregate input preservation deadline')
            for row in [auth['review']['manifest'],auth['review']['launcher'],*manifest['pins'].values()]:
                check_final_input()
                pinned(row,checkpoint=check_final_input)
            report['input_preservation']['verified']=True
        except BaseException as error:remember(report,'terminal-input-preservation',error)
        report['completed_monotonic']=time.monotonic()
        if report['completed_monotonic']>started+21600:remember(report,'terminal-deadline',Rejected('aggregate deadline exceeded'))
        report['producer_succeeded']=(report['first_error'] is None and report['returncode']==0 and
            report['cleanup'] is not None and report['cleanup']['complete'] and
            report['input_preservation']['verified'] and report.get('worker_terminal',{}).get('failure') is None and
            report.get('worker_terminal',{}).get('status')=='PARTIAL_DOWNSTREAM_DIAGNOSTIC_COMPLETE' and
            report.get('worker_terminal',{}).get('public_operations_completed')==680 and
            report.get('worker_terminal',{}).get('native_operations_completed')==68 and
            report.get('worker_terminal',{}).get('original_record_preservation',{}).get('original_records_readmitted')==869 and
            report.get('worker_terminal',{}).get('original_record_preservation',{}).get('unique_original_processes')==869 and
            report.get('worker_terminal',{}).get('original_record_preservation',{}).get('streams_verified_per_record')==2 and
            report.get('worker_terminal',{}).get('selected',{}).get('selected_sections_complete') is True and
            report.get('worker_terminal',{}).get('selected',{}).get('selected_sections')==['lifecycle','guards'] and
            report.get('worker_terminal',{}).get('selected',{}).get('local_cases')=={'lifecycle':318,'guards':22} and
            report.get('worker_terminal',{}).get('parser',{}).get('observations')==638 and
            report.get('worker_terminal',{}).get('parser',{}).get('stages')==['passivity-debug','collect-debug','passivity-release','collect-release','u8','phase','compare'] and
            report.get('worker_terminal',{}).get('build_results_retained')==8 and
            report.get('worker_terminal',{}).get('operation_cleanup',{}).get('status')=='VERIFIED' and
            report.get('worker_terminal',{}).get('operation_cleanup',{}).get('original_next')==869 and
            report.get('worker_terminal',{}).get('operation_cleanup',{}).get('operation_carriers')==869 and
            bool(report.get('worker_terminal',{}).get('preservation')) and
            all(v=='VERIFIED' for v in report['worker_terminal']['preservation'].values()))
        try:
            output_bytes(root)
            need(len(canonical(report))<=512*1024,'bounded primary terminal reserve')
            exclusive_json(root/'watchdog-terminal.json',report)
            need(time.monotonic()<=started+21600,'aggregate deadline during final terminal write')
            # Includes serialized terminal bytes. Keep half the emergency reserve
            # available for a correction if write/census exceeded the budget.
            output_bytes(root,terminal=True)
            need(time.monotonic()<=started+21600,'aggregate deadline during final terminal census')
        except BaseException as error:
            remember(report,'terminal-report',error); report['producer_succeeded']=False
            correction=dict(FLAGS,schema='oxid-fresh-downstream-watchdog-terminal-correction-v1',
                producer_succeeded=False,authoritative_outcome='FAILED_OR_UNVERIFIED',
                supersedes='watchdog-terminal.json',first_error=report['first_error'],
                secondary_errors=report['secondary_errors'],completed_monotonic=time.monotonic())
            try:exclusive_json(root/'watchdog-terminal-failure.json',correction)
            except BaseException as secondary:remember(report,'terminal-correction',secondary)
            try:print(json.dumps(report,sort_keys=True),file=sys.stderr)
            except BaseException as secondary:
                if primary_exception is not None:primary_exception.add_note('emergency report: '+str(secondary)[:1024])
    return report


def main(argv=None):
    parser=argparse.ArgumentParser()
    parser.add_argument('--review',required=True)
    parser.add_argument('--review-sha256',required=True)
    parser.add_argument('--execute',action='store_true')
    args=parser.parse_args(argv)
    report=run(args.review,args.review_sha256,execute=args.execute)
    return 0 if report['producer_succeeded'] else 1


if __name__=='__main__':
    raise SystemExit(main())
