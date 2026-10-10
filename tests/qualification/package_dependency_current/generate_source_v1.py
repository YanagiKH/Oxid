#!/usr/bin/env python3
"""PROPOSAL: candidate-only exact main.rs successor; never activate or build.

Do not invoke until the source checkpoint, recipe checkpoint and invocation have
been independently reviewed. Review-reference arguments record provenance; they
are not an authorization mechanism. This file has not been executed or tested.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import types

sys.dont_write_bytecode = True
BASE_HEAD = '0fef9bf4c664c52f441e1c8b842edfcb57fa15d0'
BASE_TREE = 'fc5515d09dce7e8cdb67a43c890272683f7b4755'
SOURCE = 'tests/fixtures/typed_project_source_binding/'
SAVED = 'tests/fixtures/typed_project_source_binding_byte_storage_v1/'
RECIPE_PATH = 'tests/qualification/package_dependency_current/generate_source_v1.py'
OLD_MANIFEST = 'lexer-reservation-source-v2.json'
OLD_SHA = '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'
OLD_AUTHORITY_SHA = 'a3853a2ba52b581bf80af53cb7ddd25e990deec87c221902e7b9d88f33b15c9c'
EXPECTED_MAIN_SHA = '73b79fa86e076a48bf3a0ca7548a1864b9a2928670f9e13512c280a63fec93cb'
EXPECTED_PATCH_SHA = '3b123cfa29136244e62038e885761e68858d2b9b472bc14231d2e7a60a13586d'
EXPECTED_PATCH_BYTES = 752
PATHS = ('src/main.rs',)
SOURCE_COMMIT_ALLOWED = frozenset(PATHS + ('tests/package_manifest_validation.rs',
    'docs/PACKAGES.md', 'docs/feature-status.json'))
ACCOUNTING = ('src/frontend/declaration_index/resource.rs',
    'src/frontend/declaration_index/sealed.rs',
    'src/frontend/declaration_index/u8_reservation.rs')
RUNNER_SHA = '584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76'
SCANNER_SHA = '7664dc9ff6597c3c586154af3b3a38f095147d025b8fd6e544f6c7a93f250ff8'
PATCH_RECIPE = 'git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE_TREE CHECKPOINT_TREE -- PATHS'
OUTPUT_NAMES = ('package-dependency-source-v1.json',
    'package-dependency-authority-v1.json', 'package-dependency-transition-v1.patch',
    'source-generation-receipt.json')


def need(condition, why):
    if not condition:
        raise ValueError(why)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def serial(value):
    return (json.dumps(value, sort_keys=True, indent=2) + '\n').encode()


def row(name, raw):
    return {'path': name, 'bytes': len(raw), 'sha256': sha(raw)}


def identity(name, raw):
    return dict(row(name, raw), mode='100644', git_blob=hashlib.sha1(
        b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest())


def regular(path):
    need(path.is_file() and not any(p.is_symlink() for p in (path, *path.parents)),
         'nonregular or symlink input: ' + str(path))
    return path.read_bytes()


def module(name, path, raw, digest):
    need(sha(raw) == digest, 'unapproved retained public helper: ' + name)
    value = types.ModuleType(name)
    value.__file__ = str(path)
    exec(compile(raw, str(path), 'exec'), value.__dict__)
    return value


def derive(args):
    need(not sys.flags.optimize, 'normal Python required')
    repo = args.repository
    need(repo.is_absolute() and repo == repo.resolve(), 'canonical repository required')
    need(not any(p.is_symlink() for p in (repo, *repo.parents)), 'symlink repository')
    for name in ('source_head', 'source_tree', 'adapter_head'):
        need(re.fullmatch('[0-9a-f]{40}', getattr(args, name)) is not None,
             'full committed identity required: ' + name)
    need(args.source_review_reference.strip() and args.adapter_review_reference.strip(),
         'explicit review provenance required')
    env = {'PATH': '/usr/bin:/bin', 'HOME': '/nonexistent/oxid-source-git-home',
        'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8', 'GIT_CONFIG_NOSYSTEM': '1',
        'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_NO_REPLACE_OBJECTS': '1',
        'GIT_TERMINAL_PROMPT': '0', 'GIT_OPTIONAL_LOCKS': '0',
        'GIT_NO_LAZY_FETCH': '1'}

    def git(*argv):
        return subprocess.check_output(['/usr/bin/git', '-c', 'core.hooksPath=/dev/null',
            '-c', 'core.fsmonitor=false', '-c', 'diff.external=', '-C', str(repo), *argv], env=env)

    def blob(commit, name):
        return git('show', commit + ':' + name)

    stores = []
    for option in ('--git-dir', '--git-common-dir'):
        path = Path(git('rev-parse', '--path-format=absolute', option).decode().strip())
        need(path.is_absolute() and path.is_dir(), 'invalid absolute Git store')
        need(not any(p.is_symlink() for p in (path, *path.parents)), 'symlink Git store')
        canonical = path.resolve(strict=True)
        need(path == canonical, 'noncanonical Git store')
        stores.append(canonical)

    def tree(commit, *paths):
        result = {}
        for item in git('ls-tree', '-r', '-z', commit, '--', *paths).split(b'\0'):
            if item:
                meta, name = item.split(b'\t', 1)
                mode, kind, oid = meta.decode().split()
                name = name.decode()
                need(str(PurePosixPath(name)) == name and not name.startswith('/')
                     and '..' not in PurePosixPath(name).parts, 'unsafe Git path')
                need(name not in result, 'duplicate Git member')
                result[name] = (mode, kind, oid)
        return result

    C, T, A = args.source_head, args.source_tree, args.adapter_head
    for commit in (BASE_HEAD, C, A):
        need(git('cat-file', '-t', commit).decode().strip() == 'commit',
             'checkpoint must be a commit object')
    need(git('rev-parse', BASE_HEAD + '^{tree}').decode().strip() == BASE_TREE, 'base tree')
    need(git('rev-parse', C + '^{tree}').decode().strip() == T, 'source tree')
    git('merge-base', '--is-ancestor', BASE_HEAD, C)
    git('merge-base', '--is-ancestor', C, A)
    recipe = regular(Path(__file__).absolute())
    need(blob(A, RECIPE_PATH) == recipe, 'adapter commit must contain this exact recipe')
    need(tree(A, RECIPE_PATH) == {RECIPE_PATH: ('100644', 'blob',
        identity(RECIPE_PATH, recipe)['git_blob'])}, 'committed recipe mode/type/blob')
    changed = set(git('diff', '--name-only', '--no-renames', '-z', BASE_HEAD, C).decode().rstrip('\0').split('\0'))
    need('src/main.rs' in changed and changed <= SOURCE_COMMIT_ALLOWED,
         'source commit exceeds reviewed four-path scope')
    raw = blob(BASE_HEAD, SOURCE + OLD_MANIFEST)
    need(len(raw) == 74328 and sha(raw) == OLD_SHA, 'immutable source-v2 manifest')
    need(blob(BASE_HEAD, SOURCE + 'current-source.json') == raw, 'baseline active alias')
    previous = json.loads(raw)
    names = [r['path'] for r in previous['files']]
    need(names == sorted(set(names)) and len(names) == 376, 'complete predecessor roster')
    authority_raw = blob(BASE_HEAD, SOURCE + 'lexer-reservation-authority-v2.json')
    need(sha(authority_raw) == OLD_AUTHORITY_SHA, 'immutable source-v2 authority')
    old_authority = json.loads(authority_raw)

    def committed(commit):
        records = tree(commit, *names)
        need(set(records) == set(names), 'selected membership')
        bodies = {n: blob(commit, n) for n in names}
        need(records == {n: ('100644', 'blob', identity(n, bodies[n])['git_blob']) for n in names},
             'selected Git mode/type/blob')
        compiler = tree(commit, 'src', 'native')
        need(compiler == {n: records[n] for n in names if n.startswith(('src/', 'native/'))}
             and len(compiler) == 288, 'complete compiler membership')
        return bodies

    before, after = committed(BASE_HEAD), committed(C)
    need([row(n, before[n]) for n in names] == previous['files'], 'baseline source-v2 equality')
    need(committed(A) == after, 'adapter checkpoint changes selected compiler bytes')
    need([n for n in names if before[n] != after[n]] == list(PATHS), 'exact one-file delta')
    need(sha(after['src/main.rs']) == EXPECTED_MAIN_SHA, 'unreviewed duplicate-alias implementation')
    # Existing named source artifacts, facades and lexer adapter bodies remain exact.
    # Added files are possible in A, but none of these existing bodies may change.
    roots = (SOURCE.rstrip('/'), SAVED.rstrip('/'), 'tests/qualification/lexer_reservation_current')
    preserved = tree(BASE_HEAD, *roots)
    for commit in (C, A):
        now = tree(commit, *roots)
        need(all(now.get(n) == value for n, value in preserved.items()), 'historical artifact changed')
    patch = git('diff', '--binary', '--no-ext-diff', '--no-textconv', '--no-renames',
                '--abbrev=7', BASE_TREE, T, '--', *PATHS)
    # Config/attributes may affect Git's rendering even for fixed tree objects.
    # Accept only independently reviewed exact bytes, never a merely equivalent diff.
    need(len(patch) == EXPECTED_PATCH_BYTES and sha(patch) == EXPECTED_PATCH_SHA,
         'Git patch rendering differs from exact reviewed patch')
    api_raw = blob(BASE_HEAD, SAVED + 'run.py')
    api = module('retained_package_inverse', repo / SAVED / 'run.py', api_raw, RUNNER_SHA)
    scanner_raw = blob(BASE_HEAD, SAVED + 'u8_cross_host.py')
    scanner = module('retained_package_scanner', repo / SAVED / 'u8_cross_host.py', scanner_raw, SCANNER_SHA)
    restored, touched = api.apply_inverse_patch(after, patch, sha(patch), len(patch), PATHS)
    need(restored == before and list(touched) == list(PATHS), 'complete one-file inverse')
    for label, inputs in (('wrong-stage', before), ('double-inverse', restored)):
        try:
            api.apply_inverse_patch(inputs, patch, sha(patch), len(patch), PATHS)
        except api.BindingError:
            pass
        else:
            raise ValueError(label + ' unexpectedly accepted')
    includes = scanner.include_directives(after, api)
    need(includes == scanner.include_directives(before, api)
         == old_authority['compile_time_include_directives'] and len(includes) == 136,
         'ordered include inventory changed')
    fixtures = [identity(n, after[n]) for n in names if n.startswith(('fixtures/', 'tests/fixtures/'))]
    need(fixtures == old_authority['compile_time_fixture_inputs'] and len(fixtures) == 78,
         'fixture closure changed')
    accounting = [identity(n, after[n]) for n in ACCOUNTING]
    need(accounting == old_authority['unit2_accounting_dependencies'], 'accounting dependency changed')
    current = dict(previous, files=[row(n, after[n]) for n in names],
        purpose='Exact duplicate dependency alias validation source successor; execution unqualified',
        reviewed_source_head=C, source_only_tree=T,
        package_dependency_predecessor_sha256=OLD_SHA,
        package_dependency_base_head=BASE_HEAD, package_dependency_base_tree=BASE_TREE)
    current_raw = serial(current)
    authority = {'schema': 'oxid-package-dependency-source-transition-v1',
        'recipe': PATCH_RECIPE, 'git_extra_safety_flags': ['--no-textconv'],
        'base_head': BASE_HEAD, 'base_tree': BASE_TREE,
        'reviewed_source_head': C, 'source_only_tree': T, 'adapter_head': A,
        'predecessor_source': row(OLD_MANIFEST, raw),
        'current_source': row(OUTPUT_NAMES[0], current_raw),
        'transition_patch': row(OUTPUT_NAMES[2], patch), 'transition_paths': list(PATHS),
        'compiler_additions': [], 'fixture_additions': [], 'removed_paths': [],
        'current_source_members': 376, 'predecessor_source_members': 376,
        'compiler_source_members': 288, 'compiler_bodies': 291,
        'current_input_identities': [identity(n, after[n]) for n in names],
        'predecessor_input_identities': [identity(n, before[n]) for n in names],
        'transition_inputs': [{'path': n, 'before': identity(n, before[n]),
                               'after': identity(n, after[n])} for n in PATHS],
        'compile_time_fixture_inputs': fixtures, 'compile_time_include_directives': includes,
        'compile_time_include_additions': [], 'unit2_accounting_dependencies': accounting}
    products = dict(zip(OUTPUT_NAMES[:3], (current_raw, serial(authority), patch)))
    products[OUTPUT_NAMES[3]] = serial({'schema': 'oxid-package-dependency-candidate-receipt-v1',
        'status': 'candidate-only-awaiting-independent-review', 'compiler_head': C,
        'compiler_tree': T, 'adapter_head': A, 'recipe': row(RECIPE_PATH, recipe),
        'source_review_reference': args.source_review_reference,
        'adapter_review_reference': args.adapter_review_reference,
        'outputs': [row(n, b) for n, b in products.items()],
        'checks': {'committed_closures': 'passed', 'complete_one_file_inverse': 'passed',
            'wrong_stage_and_double_inverse': 'passed', 'fixtures_includes_accounting': 'passed',
            'full_historical_preflight': 'not-run', 'admission_mutation_matrix': 'not-run',
            'public_parser_successor': 'not-implemented', 'execution_qualification': 'not-run'},
        'compiler_invocations': 0, 'source_materializations': 0, 'activated': False,
        'execution_qualified': False})
    return repo, products, stores


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('source-head', 'source-tree', 'adapter-head', 'source-review-reference', 'adapter-review-reference'):
        parser.add_argument('--' + name, required=True)
    parser.add_argument('--repository', type=Path, required=True)
    parser.add_argument('--candidate-output-dir', type=Path, required=True)
    args = parser.parse_args()
    out = args.candidate_output_dir
    need(out.is_absolute() and out == out.resolve(strict=False), 'canonical absolute output')
    need(not out.exists() and out.parent.is_dir(), 'fresh candidate directory required')
    need(not any(p.is_symlink() for p in (out, *out.parents)), 'symlink output')
    repo, products, stores = derive(args)
    for forbidden in (repo, *stores):
        need(out != forbidden and forbidden not in out.parents and out not in forbidden.parents,
             'output overlaps repository/Git stores')
    need(tuple(products) == OUTPUT_NAMES, 'closed output roster')
    out.mkdir(exist_ok=False)
    # No overwrite, cleanup or activation path. A partial failure remains evidence.
    for name, raw in products.items():
        with (out / name).open('xb') as stream:
            stream.write(raw)
    print(json.dumps({'status': 'candidate-only', 'outputs': [row(n, b) for n, b in products.items()]}, sort_keys=True))


if __name__ == '__main__':
    main()
