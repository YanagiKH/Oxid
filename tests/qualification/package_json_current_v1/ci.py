"""Exact two-argument CI successor with all 173 original obligations retained."""
import copy
import json
import generation as g

NAME = 'package-json-direct-consumer-dispositions-v1.json'
OLD_PLAN = (191975, 'ebdae13dee1a8bcfc5a62fba03e8088e9662cea0c676c44428975433b94707d1')
OLD_WORKFLOW = (67408, '8af4c6810e332afcaebb692087238b598800db70f7198890b50a7f99f531fb96')
CURRENT_SOURCE = (76328, 'd33ecc4e41975a062790540ba9e9a270d7c6043a31f8f4b9a8d7a044c79530b0')


def derive(old_raw, old_workflow, current_workflow, current_source):
    g.pin(old_raw, OLD_PLAN, 'accepted complete dispositions')
    g.pin(old_workflow, OLD_WORKFLOW, 'accepted complete workflow')
    g.pin(current_source, CURRENT_SOURCE, 'reviewed source manifest')
    before = b'--source-manifest-sha256 ' + g.PREDECESSOR_SHA.encode()
    after = b'--source-manifest-sha256 ' + g.sha(current_source).encode()
    g.need(old_workflow.count(before) == 2 and current_workflow == old_workflow.replace(before, after),
           'exact two current workflow arguments')
    plan = json.loads(old_raw); expected = copy.deepcopy(plan)
    g.need(len(plan['entries']) == plan['entry_count'] == 173, 'complete direct consumer roster')
    changed = []
    for row in expected['entries']:
        if row['id'] in ('ci.yml:L751', 'ci.yml:L764'):
            g.need(row['active_command'].count(before.decode()) == 1, 'exact consumer command argument')
            row['active_command'] = row['active_command'].replace(before.decode(), after.decode())
            row['active_command_sha256'] = g.sha(row['active_command'].encode())
            changed.append(row['id'])
        else:
            g.need(before.decode() not in row['active_command'], 'unexpected source argument')
    g.need(changed == ['ci.yml:L751', 'ci.yml:L764'], 'exact two current commands')
    expected.update(schema='oxid-package-json-direct-ci-dispositions-v1',
                    compiler_head=g.CHECKPOINT['head'], compiler_tree=g.CHECKPOINT['tree'],
                    current_source_manifest_sha256=g.sha(current_source),
                    current_workflow=g.identity('.github/workflows/ci.yml',current_workflow),
                    package_json_predecessor=g.identity('published-direct-consumer-dispositions-v1.json',old_raw),
                    package_json_source=json.loads(current_source)['package_json_source'])
    return expected


def validate(old_raw, old_workflow, current_workflow, current_source, current_raw):
    expected = derive(old_raw, old_workflow, current_workflow, current_source)
    g.need(current_raw == g.encoded(expected), 'complete named disposition generation')
    return {'entry_count': 173, 'changed_commands': ['ci.yml:L751', 'ci.yml:L764'],
            'argument_lines': [755, 768], 'source_only': True, 'execution_qualified': False}
