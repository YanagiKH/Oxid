#!/usr/bin/env python3
"""Verify and reconstruct independent array-lowering expectations, never Oxid output."""
import argparse, contextlib, hashlib, io, json, re, subprocess, sys, types
from pathlib import Path

ROOT = Path(__file__).resolve().parent
COMMIT = 'bb2b5d7c39d5940cbbf959444646c2dacb8edcb4'
TREE = 'a5398061eecc0e35726b648bf7edcd56434e1337'
HISTORICAL_MANIFEST = 'a40c00bc49a4415dad96300826a1c2d4f699e2c3777e461f2c472536938a579f'
CORRECTIONS = [
 ('authority/checkpoint-01-correction-1/correction.json', '9b16217f35d2399926d932a234b112334e0ead1fa2773f650c9f59777247c0b9'),
 ('authority/checkpoint-01-correction-2/correction.json', 'deed466acb14f0a660ab74c4ac4af3af63d16e9484b578c15df5c9750b7eb591'),
]
STORAGE = ('authority/checkpoint-01-correction-2/STORAGE-IDENTITIES.md', 'bb1c6d0b0d33e0e09d63fd575ced6ff42f0ef5c686cc3da641f4060de4a31c36')

def require(condition, message):
    if not condition:
        raise ValueError(message)

def sha(body):
    return hashlib.sha256(body).hexdigest()

def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON key: ' + key)
        result[key] = value
    return result

def read_json(path):
    return json.loads(path.read_bytes(), object_pairs_hook=unique_object)

def relative_name(name):
    require(isinstance(name, str) and bool(name) and '\\' not in name and ':' not in name,
            'nonportable member path')
    require(all(part not in ('', '.', '..') for part in name.split('/')), 'noncanonical member path')
    require(not name.startswith('/'), 'absolute member path')
    return name

def member(root, name):
    relative_name(name)
    path = root
    for part in name.split('/'):
        path = path / part
        require(not path.is_symlink(), 'symlinked member: ' + name)
    require(path.is_file(), 'missing regular member: ' + name)
    require(path.resolve().is_relative_to(root.resolve()), 'member escapes root: ' + name)
    return path

def inventory(root, exclude=()):
    result = {}
    for path in sorted(root.rglob('*')):
        require(not path.is_symlink(), 'symlink in inventory: ' + str(path))
        if path.is_file():
            name = relative_name(path.relative_to(root).as_posix())
            if name not in exclude:
                body = path.read_bytes()
                result[name] = {'bytes': len(body), 'sha256': sha(body)}
        else:
            require(path.is_dir(), 'nonregular inventory entry')
    return result

def check_identity(path, identity):
    require(isinstance(identity, dict) and set(identity) == {'bytes', 'sha256'}, 'malformed member identity')
    require(type(identity['bytes']) is int and identity['bytes'] >= 0, 'invalid byte length')
    require(isinstance(identity['sha256'], str) and re.fullmatch('[0-9a-f]{64}', identity['sha256']) is not None,
            'invalid SHA-256')
    body = path.read_bytes()
    require(len(body) == identity['bytes'] and sha(body) == identity['sha256'], 'member identity mismatch: ' + str(path))

def verify_package(expected_manifest=None):
    path = member(ROOT, 'freeze-manifest.json')
    digest = sha(path.read_bytes())
    if expected_manifest is not None:
        require(re.fullmatch('[0-9a-f]{64}', expected_manifest) is not None, 'invalid expected manifest hash')
        require(digest == expected_manifest, 'root manifest pin mismatch')
    value = read_json(path)
    files = value.get('files') if isinstance(value, dict) else None
    require(isinstance(files, dict) and bool(files), 'root manifest files must be a nonempty dictionary')
    require('freeze-manifest.json' not in files, 'root manifest may not self-register')
    for name, identity in files.items():
        check_identity(member(ROOT, name), identity)
    require(inventory(ROOT, ['freeze-manifest.json']) == files, 'current package membership is not exact')
    lineage = read_json(member(ROOT, 'lineage.json'))
    require(lineage['baseline_commit'] == COMMIT and lineage['baseline_tree'] == TREE, 'baseline identity mismatch')
    data_sources = sorted(name for name in files if name.endswith('.ox'))
    require(len(data_sources) == 5 and data_sources == sorted(lineage['source_data_registration']), 'source-data inventory must be exactly five paths')
    original = member(ROOT, 'authority/checkpoint-01/freeze-manifest.json')
    require(sha(original.read_bytes()) == HISTORICAL_MANIFEST, 'historical authority manifest mismatch')
    records = read_json(original)['files']
    require(isinstance(records, list) and len(records) == 14, 'historical manifest shape mismatch')
    old = {}
    for entry in records:
        name = relative_name(entry['path'])
        require(name not in old and set(entry) == {'path', 'bytes', 'sha256'}, 'malformed historical entry')
        old[name] = {'bytes': entry['bytes'], 'sha256': entry['sha256']}
    omitted = {'README.md', 'generate.py'}
    require(set(lineage['historical_authority']['historical_members_intentionally_not_packaged']) == omitted,
            'historical omission map mismatch')
    for name, identity in old.items():
        if name not in omitted:
            check_identity(member(ROOT, 'authority/checkpoint-01/' + name), identity)
    require(lineage['portable_adapter']['original_generator_sha256'] == old['generate.py']['sha256'],
            'original generator lineage mismatch')
    for name, expected in CORRECTIONS + [STORAGE]:
        require(sha(member(ROOT, name).read_bytes()) == expected, 'correction/storage identity mismatch')
    return digest, old, lineage

def git(repo, *args):
    return subprocess.check_output(['git', '-C', str(repo), *args], stderr=subprocess.PIPE)

def verify_repository(repo):
    require(git(repo, 'rev-parse', COMMIT + '^{tree}').decode().strip() == TREE, 'named baseline tree mismatch')
    bindings = read_json(member(ROOT, 'authority/checkpoint-01/inputs.json'))
    require(bindings['commit'] == COMMIT, 'source binding commit mismatch')
    for row in bindings['source']:
        relative_name(row['path'])
        body = git(repo, 'show', COMMIT + ':' + row['path'])
        require(len(body) == row['bytes'] and sha(body) == row['sha256'], 'published source-object mismatch')
        require(git(repo, 'rev-parse', COMMIT + ':' + row['path']).decode().strip() == row['git_blob'], 'published Git blob mismatch')
    return len(bindings['source'])

def write_new(path, body):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as stream:
        stream.write(body)

def json_bytes(value):
    return (json.dumps(value, indent=2, ensure_ascii=False) + '\n').encode()

def apply_corrections(models):
    applied = []
    touched = set()
    for path, expected_hash in CORRECTIONS:
        correction = read_json(member(ROOT, path))
        require(correction['preserves_original_freeze'] == HISTORICAL_MANIFEST, 'correction authority mismatch')
        for row in correction['replacements']:
            name = row['path']
            require(name.startswith('checkpoint-01/models/') and name.endswith('.json'), 'correction target not a model')
            model_name = name.removeprefix('checkpoint-01/models/')
            relative_name(model_name)
            require('/' not in model_name and model_name in models, 'unknown correction model')
            original = member(ROOT, 'authority/checkpoint-01/models/' + model_name).read_bytes()
            require(sha(original) == row['original_sha256'], 'correction original-model hash mismatch')
            pointer = row['json_pointer']
            require(re.fullmatch('/runtime/steps/[0-9]+/effects', pointer) is not None, 'unexpected correction pointer')
            require((model_name, pointer) not in touched, 'duplicate correction target')
            touched.add((model_name, pointer))
            step = int(pointer.split('/')[3])
            require(step < len(models[model_name]['runtime']['steps']), 'correction index out of range')
            slot = models[model_name]['runtime']['steps'][step]
            require(slot['effects'] == row['expected_original_value'], 'correction precondition differs')
            slot['effects'] = row['replacement_value']
            applied.append({'model': model_name, 'pointer': pointer, 'correction_sha256': expected_hash})
    require(len(applied) == 7, 'correction application count differs')
    return applied

def protected_repository_paths(repo):
    protected = {repo.resolve(), ROOT.resolve()}
    for flag in ('--git-dir', '--git-common-dir'):
        raw = git(repo, 'rev-parse', '--path-format=absolute', flag).decode('utf-8').strip()
        protected.add(Path(raw).resolve(strict=True))
    if git(repo, 'rev-parse', '--is-bare-repository').decode('utf-8').strip() == 'false':
        raw = git(repo, 'rev-parse', '--show-toplevel').decode('utf-8').strip()
        protected.add(Path(raw).resolve(strict=True))
    return protected


def reconstruct(repo, output, manifest_hash, historical):
    # Caller chooses an existing parent and a new child directory. No overwrite or retry merge.
    require(output.name not in ('', '.', '..'), 'invalid output name')
    require(output.parent.exists(), 'output parent must exist')
    require(not output.exists() and not output.is_symlink(), 'output already exists; zero-overwrite contract')
    parent = output.parent.resolve(strict=True)
    output = parent / output.name
    require(not output.exists() and not output.is_symlink(), 'canonical output already exists')
    for protected in protected_repository_paths(repo):
        require(not output.is_relative_to(protected),
                'output is inside the frozen package or input repository/Git storage')
    output.mkdir()
    original = output / 'reconstructed-original'
    original.mkdir()
    module = types.ModuleType('frozen_source_model')
    module.__file__ = str(member(ROOT, 'model_generator.py'))
    exec(compile(member(ROOT, 'model_generator.py').read_bytes(), module.__file__, 'exec'), module.__dict__)
    module.ROOT, module.REPO = original, str(repo)
    with contextlib.redirect_stdout(io.StringIO()):
        module.main()
    expected_generated = {name: identity for name, identity in historical.items()
                          if name.startswith(('models/', 'fixtures/')) or name == 'summary.json'}
    require(len(expected_generated) == 11, 'reconstructed member count differs')
    require(inventory(original) == expected_generated, 'regenerated model/source bytes differ from frozen historical expectations')
    models = {path.name: read_json(path) for path in sorted((original / 'models').glob('*.json'))}
    require(len(models) == 5, 'exactly five models required')
    applied = apply_corrections(models)
    for name, model in models.items():
        write_new(output / 'effective-models' / name, json_bytes(model))
    receipt = {
        'schema': 'oxid-independent-lowering-model-reconstruction-v1',
        'package_manifest_sha256': manifest_hash, 'baseline_commit': COMMIT, 'baseline_tree': TREE,
        'historical_authority_manifest_sha256': HISTORICAL_MANIFEST,
        'historical_reconstructed_members': 11, 'effective_model_count': 5,
        'applied_corrections': applied, 'storage_identity_statement_sha256': STORAGE[1],
        'semantic_facts_changed_by_packaging': False, 'compiler_or_candidate_executed': False,
        'reference_or_native_execution_claim': False,
    }
    write_new(output / 'receipt.json', json_bytes(receipt))
    write_new(output / 'output-manifest.json', json_bytes({'files': inventory(output)}))
    return sha((output / 'output-manifest.json').read_bytes())

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', required=True, type=Path, help='local Git object store containing the exact published baseline; never fetched')
    parser.add_argument('--out', type=Path, help='new output directory under an existing parent; omitted means verify only')
    parser.add_argument('--expected-manifest-sha256', help='optional external root-manifest pin; repository source-data registration pins it separately')
    args = parser.parse_args()
    package_hash, historical, lineage = verify_package(args.expected_manifest_sha256)
    repo = args.repo.resolve(strict=True)
    sources = verify_repository(repo)
    result = {'package_verified': True, 'package_manifest_sha256': package_hash,
              'historical_authority_identity_verified': True, 'current_membership_exact': True,
              'registered_source_data_paths': lineage['source_data_registration'],
              'named_source_objects_verified': sources, 'compiler_or_candidate_executed': False}
    if args.out is not None:
        result['output_manifest_sha256'] = reconstruct(repo, args.out, package_hash, historical)
    print(json.dumps(result, sort_keys=True))

if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, IndexError, TypeError, subprocess.CalledProcessError) as error:
        print('contract replay rejected: ' + str(error), file=sys.stderr)
        raise SystemExit(1)
