"""Inactive pure producer for the published-main formatter source successor.

Inputs are explicit authenticated bytes. Generation writes no files, invokes no
compiler, and does not reproduce the unavailable historical Git/measurement
recipes. Git headers are derived from genuine blob IDs; hunks use difflib.
"""
import copy
import datetime
import difflib
import hashlib
import json
import os

import published_transition as transition
import source_transition as retained

READBACKS = {
    "published-main-git-readback-v1.json": (570203, "8234cd0e4125740960258011d49d40a7068383aed3d72752777cdd66eadd0788"),
    "pr60-source-git-readback-v1.json": (568665, "dfea14e0092143c9d348a073c95b264f52ae0e67c1e08179fe8edae64c755da7"),
}
EVIDENCE_ROOT = "tests/qualification/published_formatter_current/source-proof/"
PRODUCER_PATH = "tests/qualification/published_formatter_current/generate_published_source_v1.py"
RECEIPT_NAME = "published-source-generation-v1.json"


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def encoded(value):
    return (json.dumps(value, sort_keys=True, indent=2) + "\n").encode()


def binding(name, raw):
    return {"path": name, "bytes": len(raw), "sha256": sha(raw)}


def gitid(kind, raw):
    return hashlib.sha1(kind.encode() + b" " + str(len(raw)).encode()
                        + b"\0" + raw).hexdigest()

def check_tree(record, commit, tree):
    need(record['repository']=='YanagiKH/Oxid' and record['commit']['sha']==commit
         and record['commit']['tree']['sha']==tree and record['tree']['sha']==tree
         and record['tree']['truncated'] is False, 'actual checkpoint association')
    rows = record['tree']['entries']
    need(len(rows)==3358 and len(rows)<=4096, 'fixed complete tree inventory count')
    bypath, children = {}, {'':[]}
    for row in rows:
        path = row['path']
        need(isinstance(path,str) and path and not path.startswith('/') and '\\' not in path
             and all(x not in ('','.','..') for x in path.split('/')), 'canonical Git path')
        need(path not in bypath, 'duplicate Git member')
        need(len(row['sha'])==40 and all(x in '0123456789abcdef' for x in row['sha']), 'Git object ID')
        need((row['type'],row['mode']) in {
            ('tree','040000'),('blob','100644'),('blob','100755'),('blob','120000'),
            ('commit','160000')}, 'Git object mode/type')
        bypath[path] = row
        parent, _, name = path.rpartition('/')
        children.setdefault(parent,[]).append(row)
        if row['type']=='tree':
            children.setdefault(path,[])
    need(set(children)-{''} == {p for p,r in bypath.items() if r['type']=='tree'},
         'complete Git directory ancestry')
    calculated = {}
    for parent, members in children.items():
        ordered = sorted(members,key=lambda r:os.fsencode(r['path'].rsplit('/',1)[-1])+
                         (b'/' if r['type']=='tree' else b''))
        body = b''.join(r['mode'].lstrip('0').encode()+b' '+
                        os.fsencode(r['path'].rsplit('/',1)[-1])+b'\0'+bytes.fromhex(r['sha'])
                        for r in ordered)
        actual = gitid('tree',body)
        expected = tree if not parent else bypath[parent]['sha']
        need(actual==expected, 'complete Git tree-node mismatch: '+parent)
        calculated[parent] = actual
    return bypath, len(calculated)

def commit_body(record):
    """Verify exact current commit bytes when metadata supplies a reproducible body."""
    item = record['commit']; verification = item['verification']
    variants = []
    if verification.get('payload') and verification.get('signature'):
        headers, message = verification['payload'].split('\n\n',1)
        lines = verification['signature'].split('\n')
        signature = '\n'.join(['gpgsig '+lines[0]]+[' '+line for line in lines[1:]])
        variants.append((headers+'\n'+signature+'\n\n'+message).encode())
    else:
        prefix = 'tree '+item['tree']['sha']+'\n'
        prefix += ''.join('parent '+p['sha']+'\n' for p in item['parents'])
        for offset in ('+0000','+0900'):
            headers = prefix
            for role in ('author','committer'):
                who = item[role]
                epoch = int(datetime.datetime.fromisoformat(who['date'].replace('Z','+00:00')).timestamp())
                headers += role+' '+who['name']+' <'+who['email']+'> '+str(epoch)+' '+offset+'\n'
            for ending in ('','\n'):
                variants.append((headers+'\n'+item['message']+ending).encode())
    matched = [b for b in variants if gitid('commit',b)==item['sha']]
    need(len(matched)==1, 'exact published commit body not uniquely reproduced')
    return {'object_sha1':item['sha'],'type':'commit','bytes':len(matched[0]),
            'body_sha256':sha(matched[0]),'exact_current_commit_object_verified':True,
            'historical_missing_objects_recovered':False}


def exact_patch(before, after):
    need(before.endswith(b"\n") and after.endswith(b"\n"),
         "fixed formatter bodies must retain their final newline")
    old, new = before.decode("utf-8"), after.decode("utf-8")
    need(old.encode("utf-8") == before and new.encode("utf-8") == after,
         "formatter source encoding changed")
    header = ("diff --git a/src/main.rs b/src/main.rs\nindex "
              + gitid("blob", before)[:7] + ".." + gitid("blob", after)[:7]
              + " 100644\n")
    hunks = "".join(difflib.unified_diff(
        old.splitlines(keepends=True), new.splitlines(keepends=True),
        fromfile="a/src/main.rs", tofile="b/src/main.rs", n=3, lineterm="\n"))
    need(hunks.startswith("--- a/src/main.rs\n+++ b/src/main.rs\n"),
         "missing exact unified transition body")
    return (header + hunks).encode("utf-8")


def generate(predecessor_raw, current_inputs, old_main, readback_bodies,
             producer_raw, api):
    """Return candidate artifacts only; caller must authenticate this producer.

    api is the separately authenticated unchanged retained public source API.
    It supplies the exact inverse parser and canonical include scanner. This
    function accepts byte maps, not repository paths or environment overrides.
    """
    need(len(predecessor_raw) == transition.PREDECESSOR_BYTES
         and sha(predecessor_raw) == transition.PREDECESSOR_SHA,
         "changed immutable source-v2 manifest")
    need(type(producer_raw) is bytes and 0 < len(producer_raw) <= 65536,
         "missing reviewed producer body")
    predecessor = json.loads(predecessor_raw)
    old_rows = retained.rows_map(predecessor["files"])
    need(len(old_rows) == 376 and list(current_inputs) == list(old_rows)
         and all(type(b) is bytes for b in current_inputs.values()),
         "exact ordered complete current input map required")
    need(gitid("blob", old_main) == transition.BEFORE_BLOB
         and gitid("blob", current_inputs["src/main.rs"]) == transition.AFTER_BLOB,
         "unreviewed formatter body")
    before = dict(current_inputs)
    before["src/main.rs"] = old_main
    for name, raw in before.items():
        need(binding(name, raw) == old_rows[name], "M differs from retained source-v2: " + name)
    before_ids = [retained.identity(n, b) for n, b in before.items()]
    after_ids = [retained.identity(n, b) for n, b in current_inputs.items()]
    need([n for n in before if before[n] != current_inputs[n]] == ["src/main.rs"],
         "unexplained complete source change")
    need(set(readback_bodies) == set(READBACKS), "wrong finite Git readback roster")
    records = {}
    for name, (size, digest) in READBACKS.items():
        raw = readback_bodies[name]
        need(len(raw) == size and sha(raw) == digest, "changed accepted Git readback")
        records[name] = json.loads(raw)
    m_record = records["published-main-git-readback-v1.json"]
    c_record = records["pr60-source-git-readback-v1.json"]
    m, m_nodes = check_tree(m_record, transition.PUBLISHED_MAIN["head"], transition.PUBLISHED_MAIN["tree"])
    c, c_nodes = check_tree(c_record, transition.SOURCE_CHECKPOINT["head"], transition.SOURCE_CHECKPOINT["tree"])
    commits = {"published_main": commit_body(m_record), "source_checkpoint": commit_body(c_record)}
    need([p["sha"] for p in c_record["commit"]["parents"]]
         == [transition.PUBLISHED_MAIN["head"]], "source checkpoint parent changed")
    for identities, tree in ((before_ids, m), (after_ids, c)):
        for row in identities:
            entry = tree[row["path"]]
            need(entry["type"] == "blob" and entry["mode"] == row["mode"] == "100644"
                 and entry["sha"] == row["git_blob"] and entry["size"] == row["bytes"],
                 "selected body differs from genuine Git member")
    compiler = [n for n in old_rows if n.startswith(("src/", "native/"))]
    need(len(compiler) == 288, "wrong complete compiler membership")
    for tree in (m, c):
        need(sorted(n for n, r in tree.items()
                    if n.startswith(("src/", "native/")) and r["type"] != "tree") == compiler,
             "Git compiler tree has missing, extra or nonblob members")
    changed = sorted(n for n in set(m) | set(c)
                     if ((m.get(n) or {}).get("type") != "tree"
                         or (c.get(n) or {}).get("type") != "tree")
                     and m.get(n) != c.get(n))
    need(changed == ["spec/legacy-0.9.md", "src/main.rs", "tests/legacy_formatter.rs"],
         "full PR leaf delta changed")
    need((sum(map(len, before.values())), sum(map(len, current_inputs.values())))
         == (7705716, 7706488), "selected byte totals changed")
    fixtures = [r for r in after_ids if r["path"].startswith(("fixtures/", "tests/fixtures/"))]
    need(len(fixtures) == 78 and fixtures ==
         [r for r in before_ids if r["path"].startswith(("fixtures/", "tests/fixtures/"))],
         "compile-time fixture body changed")
    includes = api.include_directives(current_inputs, api)
    need(len(includes) == 136 and includes == api.include_directives(before, api),
         "compile-time include expressions changed")
    accounting = [retained.identity(n, current_inputs[n]) for n in retained.ACCOUNTING_PATHS]
    need(all(current_inputs[n] == before[n] for n in retained.ACCOUNTING_PATHS),
         "immutable accounting dependency changed")
    patch = exact_patch(old_main, current_inputs["src/main.rs"])
    recovered, touched = api.apply_inverse_patch(
        current_inputs, patch, sha(patch), len(patch), ("src/main.rs",))
    need(touched == ["src/main.rs"] and recovered == before, "complete source inverse failed")
    forward_patch = exact_patch(current_inputs["src/main.rs"], old_main)
    forwarded, touched = api.apply_inverse_patch(
        before, forward_patch, sha(forward_patch), len(forward_patch), ("src/main.rs",))
    need(touched == ["src/main.rs"] and forwarded == current_inputs,
         "complete source forward check failed")
    current = copy.deepcopy(predecessor)
    current.update(
        files=[binding(n, b) for n, b in current_inputs.items()],
        purpose="Exact published-main formatter source successor; historical authorities retain their original identities.",
        reviewed_source_head=transition.SOURCE_CHECKPOINT["head"],
        source_only_tree=transition.SOURCE_CHECKPOINT["tree"],
        published_main_head=transition.PUBLISHED_MAIN["head"],
        published_main_tree=transition.PUBLISHED_MAIN["tree"],
        published_source_predecessor_sha256=transition.PREDECESSOR_SHA)
    current_raw = encoded(current)
    predecessor_descriptor = binding(transition.PREDECESSOR_NAME, predecessor_raw)
    source_descriptor = binding(transition.CURRENT_NAME, current_raw)
    patch_descriptor = binding(transition.PATCH_NAME, patch)
    readback_descriptors = {name: binding(EVIDENCE_ROOT + name, raw)
                            for name, raw in readback_bodies.items()}
    producer_descriptor = binding(PRODUCER_PATH, producer_raw)
    receipt = {
        "schema": "oxid-published-main-source-generation-v1",
        "recipe": transition.RECIPE,
        "producer": producer_descriptor,
        "published_main": transition.PUBLISHED_MAIN,
        "source_checkpoint": transition.SOURCE_CHECKPOINT,
        "retained_historical_source": transition.HISTORICAL_SOURCE,
        "actual_commit_objects": commits,
        "complete_tree_nodes": {"published_main": m_nodes, "source_checkpoint": c_nodes},
        "readbacks": readback_descriptors,
        "predecessor_source_manifest": predecessor_descriptor,
        "current_source_manifest": source_descriptor,
        "transition_patch": patch_descriptor,
        "selected_members": 376, "compiler_members": 288,
        "fixture_members": 78, "include_expressions": 136,
        "selected_bytes": {"published_main": 7705716, "source_checkpoint": 7706488},
        "complete_pr_changed_files": changed,
        "selected_source_changes": ["src/main.rs"],
        "complete_forward_inverse_equal": True,
        "filesystem_membership_checked_here": False,
        "historical_git_recovery_claimed": False,
        "historical_measurement_reproduction_claimed": False,
        "source_only": True, "execution_qualified": False,
    }
    receipt_raw = encoded(receipt)
    proof = {
        "schema": "oxid-published-main-selected-source-proof-v1",
        "published_main": transition.PUBLISHED_MAIN,
        "source_checkpoint": transition.SOURCE_CHECKPOINT,
        "retained_historical_source": transition.HISTORICAL_SOURCE,
        "retained_source_manifest": predecessor_descriptor,
        "predecessor_input_identities": before_ids,
        "current_input_identities": after_ids,
        "complete_compiler_paths": compiler,
        "complete_pr_changed_files": changed,
        "git_object_evidence": {
            "published_main": readback_descriptors["published-main-git-readback-v1.json"],
            "source_checkpoint": readback_descriptors["pr60-source-git-readback-v1.json"],
            "verifier": producer_descriptor,
            "generation_receipt": binding(EVIDENCE_ROOT + RECEIPT_NAME, receipt_raw),
        },
        "source_only": True, "execution_qualified": False,
    }
    proof_raw = encoded(proof)
    authority = {
        "schema": "oxid-published-main-source-transition-v1",
        "recipe": transition.RECIPE,
        "published_main": transition.PUBLISHED_MAIN,
        "source_checkpoint": transition.SOURCE_CHECKPOINT,
        "retained_historical_source": transition.HISTORICAL_SOURCE,
        "predecessor_source_manifest": predecessor_descriptor,
        "current_source_manifest": source_descriptor,
        "transition_patch": patch_descriptor,
        "source_proof": binding(transition.PROOF_NAME, proof_raw),
        "transition_paths": ["src/main.rs"],
        "predecessor_input_identities": before_ids,
        "current_input_identities": after_ids,
        "compile_time_fixture_inputs": fixtures,
        "compile_time_include_directives": includes,
        "unit2_accounting_dependencies": accounting,
        "current_source_members": 376, "predecessor_source_members": 376,
        "compiler_source_members": 288, "compiler_bodies": 291,
        "source_only": True, "execution_qualified": False,
    }
    need(set(authority) == transition.AUTHORITY_FIELDS
         and set(proof) == transition.PROOF_FIELDS, "producer/validator schema mismatch")
    # The receipt precedes and does not hash the proof/authority, avoiding a
    # proof -> receipt -> authority -> proof cycle. No package self-pin is made.
    return {
        transition.CURRENT_NAME: current_raw,
        transition.PATCH_NAME: patch,
        transition.PROOF_NAME: proof_raw,
        transition.AUTHORITY_NAME: encoded(authority),
        EVIDENCE_ROOT + RECEIPT_NAME: receipt_raw,
        **{EVIDENCE_ROOT + name: raw for name, raw in readback_bodies.items()},
    }

