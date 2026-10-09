#!/usr/bin/env python3
"""RFC0031 provider boundary successor; unchanged producers, protocols and caps.

This has a separate recipe/report from RFC0030. It distinguishes old valid and
unchanged invalid scalar controls from the newly accepted array domain. Neither
an old parser refusal nor an unrelated integer observation becomes authority
for newly valid byte-array source. Evidence remains outside the checkout.
"""
import argparse
import json
from pathlib import Path

import qualify_hir_producer_diagnostics as predecessor
import qualify_hir_u8_compatibility as scalar

require, digest = predecessor.require, predecessor.digest
VALID = (('old-i32', b'fn main()->i32{return 42;}'),)
INVALID = (('old-i64-error', b'fn f(x:i64)->i64{return x;}', 4),)
ACCEPTED = (
    ('byte-zero-owner', b'fn main()->i32{let a:[u8;0]=[];return a.len();}'),
    ('byte-value-parameter', b'fn f(a:[u8;1])->i32{return a.len();}'),
    ('byte-exact-reference', b'fn f(a:&[u8;1])->i32{return a.len();}'),
    ('byte-exact-exclusive', b'fn f(a:&mut[u8;1])->i32{return a.len();}'),
    ('byte-slice', b'fn f(a:&[u8])->i32{return a.len();}'),
    ('byte-exclusive-slice', b'fn f(a:&mut[u8])->i32{return a.len();}'),
    ('byte-inferred-literal', b'fn f(b:u8)->i32{let a=[b];return a.len();}'),
    ('byte-unused-helper', b'fn spare(a:[u8;0])->i32{return a.len();}fn main()->i32{return 0;}'),
)
FENCES = (
    ('byte-record-zero', b'struct R{a:[u8;0]}fn main()->i32{return 0;}', 'E0202'),
    ('direct-byte-record', b'struct R{b:u8}fn main()->i32{return 0;}', 'E0202'),
    ('byte-enum-payload', b'enum E{V(u8)}fn main()->i32{return 0;}', 'E0100'),
)
PROJECTS = ('project-child-before', 'project-child-after')
NEGATIVES = ('byte-array-integer-observation', 'byte-array-integer-artifact',
             'byte-array-old-error-artifact')


def recipe():
    return {'unchanged_valid': [name for name, _ in VALID],
            'unchanged_invalid': [name for name, _, _ in INVALID],
            'newly_accepted': [name for name, _ in ACCEPTED],
            'retained_fences': [name for name, _, _ in FENCES],
            'wrong_authority': list(NEGATIVES),
            'changed_project_identity': [name + '-' + route for name in PROJECTS for route in ('producers', 'import')]}


def expected_results():
    parities, refusals, captures = [], [], []
    for version in (1, 2):
        for name, source, kind in INVALID:
            captures.append((version, name, digest(source), kind))
            parities.extend(f'v{version}/parity/{name}/{operation}-{mode}'
                            for operation in predecessor.OPERATIONS for mode in ('json', 'text'))
        for name in [*(name for name, _ in ACCEPTED), *(name for name, _, _ in FENCES), *NEGATIVES]:
            refusals.extend(f'v{version}/refusals/{name}/{operation}-{mode}'
                            for operation in predecessor.OPERATIONS for mode in ('json', 'text'))
    for version in (1, 2):
        # Per-version project checks follow that version's scalar recipe.
        additions = [f'v{version}/refusals/{name}-{route}/{operation}-{mode}'
                     for name in PROJECTS for route in ('producers', 'import')
                     for operation in predecessor.OPERATIONS for mode in ('json', 'text')]
        index = next((i for i, row in enumerate(refusals) if row.startswith(f'v{version + 1}/')), len(refusals))
        refusals[index:index] = additions
    return parities, refusals, captures


class Run(predecessor.Run):
    positive = scalar.Run.positive
    canonical = scalar.Run.canonical

    def input_identities(self):
        result = super().input_identities()
        result['byte_storage_harness_sha256'] = digest(Path(__file__).read_bytes())
        result['byte_storage_recipe_sha256'] = digest(json.dumps(recipe(), sort_keys=True).encode())
        result['unchanged_scalar_helper_sha256'] = digest(Path(scalar.__file__).read_bytes())
        return result

    def execute(self):
        for version in (1, 2):
            bundle, hashes = self.build(version)
            for name, source in VALID:
                self.positive(version, name, source, bundle, hashes)
            for name, source, kind in INVALID:
                captured = self.capture(version, name, source, bundle, kind)
                self.parity(version, name, source, captured, bundle, hashes)
            for name, source in ACCEPTED:
                self.canonical(version, name, source, None)
                self.refuse(version, name, source, bundle)
            for name, source, code in FENCES:
                self.canonical(version, name, source, code)
                # The unchanged enum grammar fails before provider dispatch.
                # Exact base b5455ad CLI independently confirms E0100/span9..11.
                refusal = 'E0100' if name == 'byte-enum-payload' else 'E0703'
                self.refuse(version, name, source, bundle, code=refusal)
                if refusal == 'E0100':
                    for operation in predecessor.OPERATIONS:
                        stream = self.root / f'v{version}/refusals/{name}/{operation}-json/stdout'
                        rows = [json.loads(line) for line in stream.read_text().splitlines()]
                        require(not any(row.get('kind') == 'hir-producer' for row in rows),
                                'pre-provider parse refusal unexpectedly dispatched a producer')
                        primary = rows[0].get('primary', {})
                        require(rows[0].get('stage') == 'parse' and
                                rows[0].get('message') == 'only bool, i32 and () enum payloads are supported' and
                                (primary.get('start'), primary.get('end')) == (9, 11),
                                'unchanged enum parse precedence or span changed')
            # Genuine integer producer bytes are only a deliberately wrong
            # source authority. Never modify them into an alleged byte AST.
            # Match only the declared source length so orchestration reaches
            # the authoritative AST comparison; do not merely trip length.
            donor_source = VALID[0][1].ljust(len(ACCEPTED[0][1]), b' ')
            require(len(donor_source) == len(ACCEPTED[0][1]), 'donor length mismatch')
            self.capture(version, 'integer-donor', donor_source, bundle)
            donor = self.root / f'v{version}/captures/integer-donor' 
            opa = (donor / 'parser/stdout').read_bytes()
            wire = (donor / 'static/stdout').read_bytes()
            require(len(opa) == scalar.OPA_BYTES and len(wire) == scalar.SUCCESS_BYTES,
                    'actual integer donor protocol changed')
            selected = self.root / f'v{version}/negative-bundles/byte-array-integer-observation'
            selected.mkdir(parents=True)
            self.helper(selected / 'parser', opa)
            self.helper(selected / 'static', wire)
            self.manifest(selected, version)
            self.refuse(version, NEGATIVES[0], ACCEPTED[0][1], selected, code='E0702')
            artifact = selected / 'integer.bin'; artifact.write_bytes(wire)
            self.refuse(version, NEGATIVES[1], ACCEPTED[0][1], artifact,
                        '--experimental-hir-import', 'E0702')
            old_error = self.root / f'v{version}/captures/old-i64-error/static/stdout'
            self.refuse(version, NEGATIVES[2], ACCEPTED[0][1], old_error,
                        '--experimental-hir-import', 'E0702')
            self.project_controls(version, bundle, artifact)
        self.finish_byte_storage()

    def project_controls(self, version, bundle, artifact):
        # Immutable predecessor hir_producer::produce rejects owner.count()!=1
        # before spawn; hir_import's Domain boundary independently requires one
        # root-only source. Change only child bytes, retaining identical roots.
        root_text = b'mod child;fn main()->i32{return 0;}'
        for name, element in zip(PROJECTS, ('bool', 'u8')):
            child_text = f'pub fn spare(a:[{element};0])->i32{{return a.len();}}'.encode()
            for route, option, selected, code in (
                ('producers', '--experimental-hir-producers', bundle, 'E0703'),
                ('import', '--experimental-hir-import', artifact, 'E0702'),
            ):
                directory = self.root / f'v{version}/refusals/{name}-{route}'
                directory.mkdir(parents=True)
                path = directory / 'source.ox'
                path.write_bytes(root_text)
                (directory / 'child.ox').write_bytes(child_text)
                canonical = self.call(str(directory.relative_to(self.root)) + '/canonical-check',
                                      self.command('check', path), status=0, guarded=True)
                canonical_rows = [json.loads(line) for line in canonical.stdout.splitlines()]
                require(not canonical.stderr and len(canonical_rows) == 1 and
                        canonical_rows[0].get('kind') == 'check-summary' and
                        canonical_rows[0].get('success') is True and canonical_rows[0].get('errors') == 0,
                        'project control is not canonically valid')
                for operation in predecessor.OPERATIONS:
                    for json_mode in (True, False):
                        mode = 'json' if json_mode else 'text'
                        label = str(directory.relative_to(self.root)) + '/' + operation + '-' + mode
                        target = directory / (operation + '-' + mode + '.elf')
                        if json_mode:
                            target.write_bytes(b'KEEP-EXISTING-OUTPUT')
                        result = self.call(label, self.command(operation, path, option, selected,
                                           target, json_mode), status=1, guarded=True)
                        if json_mode:
                            rows = predecessor.json_rows(result)
                            predecessor.validate_failure(rows, operation, code)
                            require(not any(row.get('kind') == 'hir-producer' for row in rows),
                                    'multi-file rejection dispatched a producer')
                            require(target.read_bytes() == b'KEEP-EXISTING-OUTPUT', 'project refusal clobbered output')
                        else:
                            require(not result.stdout and code.encode() in result.stderr and not target.exists(),
                                    'project refusal ran or published output')
                        require(path.read_bytes() == root_text and (directory / 'child.ox').read_bytes() == child_text,
                                'project source identity changed during qualification')
                        self.refusals.append(label)

    def finish_byte_storage(self):
        require(self.input_identities() == self.identities, 'qualification inputs changed')
        parities, refusals, captures = expected_results()
        require(self.parities == parities and self.refusals == refusals,
                'incomplete byte-storage provider recipe')
        require([(r['version'], r['name'], r['source_sha256'], r['kind']) for r in self.captures] == captures,
                'incomplete unchanged diagnostic captures')
        for receipt in self.receipts:
            directory = self.root / receipt['name']
            for stream in ('stdout', 'stderr'):
                require(digest((directory / stream).read_bytes()) == receipt[stream + '_sha256'],
                        'retained provider stream changed')
            if receipt['stdin_sha256'] is not None:
                require(digest((directory / 'stdin.bin').read_bytes()) == receipt['stdin_sha256'],
                        'retained provider input changed')
        require(not self.marker.exists() and not list(self.snapshots.iterdir()),
                'provider refusal invoked LLVM or leaked a workspace')
        report = dict(self.identities, schema='rfc0031-byte-storage-provider-boundary-v1',
                      status='passed', recipe=recipe(),
                      frozen_protocols=['OPA1/AST1/STF1', 'OPA2/AST2/STF2'],
                      caps=[128, 255], frame_bytes=[1559, 2607, 1575],
                      unchanged_invalid_parities=parities, closed_boundary_refusals=refusals,
                      pre_provider_parse_refusals=[r for r in refusals if '/byte-enum-payload/' in r],
                      protocol_or_import_refusals=[r for r in refusals if '/byte-enum-payload/' not in r],
                      captures=self.captures, invalid_source_llvm_invocations=0,
                      receipts_sha256=digest((self.root / 'receipts.json').read_bytes()))
        (self.root / 'byte-storage-summary.json').write_text(json.dumps(report, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', required=True, type=Path)
    parser.add_argument('--llvm-bin', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--cc', default='cc')
    args = parser.parse_args()
    Run(args.compiler, args.llvm_bin, args.output, args.cc).execute()


if __name__ == '__main__':
    main()
