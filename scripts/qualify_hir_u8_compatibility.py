#!/usr/bin/env python3
"""RFC0030 current-compiler compatibility successor, separate from frozen evidence.

Builds unchanged v1/v2 producers using the existing reviewed builder. Captures
old-valid success, still-invalid diagnostics and changed-domain refusal separately.
Never rewrites a source seal, frozen roster, producer source or old observation.
This is a focused semantic compatibility gate, not parser/resource qualification.
"""
import argparse
import json
from pathlib import Path

import qualify_hir_producer_diagnostics as predecessor

require = predecessor.require
digest = predecessor.digest
OPA_BYTES, SUCCESS_BYTES, DIAGNOSTIC_BYTES = 1559, 2607, 1575
VALID = (
    ('plain-i32', b'fn main()->i32{return 42;}'),
    ('ordinary-u8-function', b'fn u8(x:i32)->i32{return x;}fn main()->i32{return u8(7);}'),
)
INVALID = (
    ('unchanged-i64-signature', b'fn f(x:i64)->i64{return x;}', 4),
    ('unchanged-public-i64', b'pub fn f(x:i64)->i64{return x;}', 4),
    ('multiple-type-errors', b'fn a()->i32{return true;}fn b()->bool{return 1;}', 8),
)
CHANGED_TYPES = (
    ('u8-signature', b'fn f(x:u8)->u8{return x;}'),
    ('u8-local', b'fn f(x:u8)->u8{let y:u8=x;return y;}'),
    ('u8-public-signature', b'pub fn f(x:u8)->u8{return x;}'),
)
CHANGED_SYNTAX = (
    ('narrow', b'fn f(x:i32)->u8{return x.to_u8_checked();}', None),
    ('widen', b'fn f(x:u8)->i32{return x.to_i32();}', None),
    ('reserved-record', b'struct u8{}fn main()->i32{return 0;}', 'E0208'),
    ('reserved-enum', b'enum u8{V}fn main()->i32{return 0;}', 'E0208'),
)


def recipe():
    return dict(valid=[name for name, _ in VALID],
                invalid=[name for name, _, _ in INVALID],
                changed_types=[name for name, _ in CHANGED_TYPES],
                changed_syntax=[name for name, _, _ in CHANGED_SYNTAX],
                negative_success=['u8-forged-i32-success'])


def expected_results():
    parities, refusals, captures = [], [], []
    for version in (1, 2):
        for name, source, kind in INVALID:
            captures.append((version, name, digest(source), kind))
            parities.extend(f'v{version}/parity/{name}/{operation}-{mode}'
                            for operation in predecessor.OPERATIONS for mode in ('json', 'text'))
        refused = []
        for name, source in CHANGED_TYPES:
            captures.append((version, name, digest(source), 4))
            refused.extend((name, name + '-artifact'))
        refused.extend(name for name, _, _ in CHANGED_SYNTAX)
        refused.extend(('u8-forged-i32-success', 'u8-forged-i32-artifact'))
        refusals.extend(f'v{version}/refusals/{name}/{operation}-{mode}'
                        for name in refused for operation in predecessor.OPERATIONS
                        for mode in ('json', 'text'))
    return parities, refusals, captures


def successful_check(result):
    require(result.returncode == 0 and not result.stderr, 'canonical check failed')
    rows = [json.loads(line) for line in result.stdout.splitlines()]
    require(rows and rows[-1].get('kind') == 'check-summary'
            and rows[-1].get('success') is True
            and not any(row.get('kind') == 'diagnostic' for row in rows),
            'canonical check did not establish success')
    return predecessor.without_provenance(rows)


def forged_i32_success(opa, byte_diagnostic, integer_success, version):
    """Only a negative control: genuine byte AST with forged integer facts."""
    digit = str(version).encode()
    require(len(opa) == OPA_BYTES and opa[:5] == b'OPA' + digit + b'\0',
            'byte source needs an authentic successful parser capture')
    require(len(byte_diagnostic) == DIAGNOSTIC_BYTES
            and byte_diagnostic[:OPA_BYTES] == opa
            and byte_diagnostic[OPA_BYTES:OPA_BYTES + 5] == b'STF' + digit + b'\1',
            'byte source needs the authentic old diagnostic')
    require(len(integer_success) == SUCCESS_BYTES
            and integer_success[OPA_BYTES:OPA_BYTES + 5] == b'STF' + digit + b'\0'
            and integer_success[8] == opa[8], 'integer donor has different row shape')
    return opa + integer_success[OPA_BYTES:]


class Run(predecessor.Run):
    def input_identities(self):
        result = super().input_identities()
        result['u8_harness_sha256'] = digest(Path(__file__).read_bytes())
        result['u8_recipe_sha256'] = digest(json.dumps(recipe(), sort_keys=True).encode())
        return result

    def positive(self, version, name, source, bundle, hashes):
        self.capture(version, name, source, bundle)
        directory = self.root / f'v{version}/captures/{name}'
        opa = (directory / 'parser/stdout').read_bytes()
        wire = (directory / 'static/stdout').read_bytes()
        path = directory / 'source.ox'
        ordinary = self.call(f'v{version}/valid/{name}/ordinary',
                             self.command('check', path), guarded=True)
        produced = self.call(f'v{version}/valid/{name}/producer',
                             self.command('check', path, '--experimental-hir-producers', bundle),
                             guarded=True)
        expected = successful_check(ordinary)
        # Producer text is entirely JSON; provenance is the only added content.
        actual_rows = [json.loads(line) for line in produced.stdout.splitlines()]
        require(not produced.stderr and predecessor.without_provenance(actual_rows) == expected,
                'unchanged successful source differs from canonical checker')
        predecessor.validate_provenance(actual_rows, source, opa, wire, hashes, version)

    def canonical(self, version, name, source, code):
        directory = self.root / f'v{version}/canonical/{name}'
        directory.mkdir(parents=True)
        path = directory / 'source.ox'
        path.write_bytes(source)
        result = self.call(f'v{version}/canonical/{name}/check', self.command('check', path),
                           status=0 if code is None else 1, guarded=True)
        if code is None:
            successful_check(result)
        else:
            predecessor.validate_failure(predecessor.json_rows(result), 'check', code)

    def execute(self):
        for version in (1, 2):
            bundle, hashes = self.build(version)
            for name, source in VALID:
                self.positive(version, name, source, bundle, hashes)
            for name, source, kind in INVALID:
                capture = self.capture(version, name, source, bundle, kind)
                self.parity(version, name, source, capture, bundle, hashes)
            for name, source in CHANGED_TYPES:
                self.canonical(version, name, source, None)
                capture = self.capture(version, name, source, bundle, 4)
                self.refuse(version, name, source, bundle)
                path, _, _ = capture
                self.refuse(version, name + '-artifact', source, path.parent / 'static/stdout',
                            '--experimental-hir-import', 'E0702')
            for name, source, code in CHANGED_SYNTAX:
                self.canonical(version, name, source, code)
                self.refuse(version, name, source, bundle)
            # Equal grammar shape, independently captured source/OPA identities.
            integer_source = b'fn f(x:i32)->i32{return x;}'
            self.capture(version, 'integer-donor', integer_source, bundle)
            directory = self.root / f'v{version}/captures'
            byte_directory = directory / 'u8-signature'
            opa = (byte_directory / 'parser/stdout').read_bytes()
            negative = forged_i32_success(opa,
                (byte_directory / 'static/stdout').read_bytes(),
                (directory / 'integer-donor/static/stdout').read_bytes(), version)
            selected = self.root / f'v{version}/negative-bundles/u8-forged-i32-success'
            selected.mkdir(parents=True)
            self.helper(selected / 'parser', opa)
            self.helper(selected / 'static', negative)
            self.manifest(selected, version)
            # The predecessor success-frame checker deliberately retains
            # E0702 even when orchestration obtained that frame from producers.
            self.refuse(version, 'u8-forged-i32-success', CHANGED_TYPES[0][1], selected,
                        code='E0702')
            artifact = selected / 'observation.bin'
            artifact.write_bytes(negative)
            self.refuse(version, 'u8-forged-i32-artifact', CHANGED_TYPES[0][1], artifact,
                        '--experimental-hir-import', 'E0702')
        self.finish_u8()

    def finish_u8(self):
        require(self.input_identities() == self.identities, 'qualification inputs changed')
        expected_parities, expected_refusals, expected_captures = expected_results()
        require(self.parities == expected_parities, 'incomplete or changed legacy parity recipe')
        require(self.refusals == expected_refusals, 'incomplete or changed refusal recipe')
        require([(entry['version'], entry['name'], entry['source_sha256'], entry['kind'])
                 for entry in self.captures] == expected_captures,
                'incomplete or changed producer captures')
        for receipt in self.receipts:
            directory = self.root / receipt['name']
            for stream in ('stdout', 'stderr'):
                require(digest((directory / stream).read_bytes()) == receipt[stream + '_sha256'],
                        'retained command stream changed')
            if receipt['stdin_sha256'] is not None:
                require(digest((directory / 'stdin.bin').read_bytes()) == receipt['stdin_sha256'],
                        'retained command input changed')
        require(not self.marker.exists() and not list(self.snapshots.iterdir()),
                'refusal invoked LLVM or leaked a workspace')
        report = dict(self.identities, schema='rfc0030-frozen-provider-successor-1',
                      status='passed', recipe=recipe(),
                      frozen_protocols=['OPA1/AST1/STF1', 'OPA2/AST2/STF2'],
                      caps=[128, 255], frame_bytes=[1559, 2607, 1575],
                      legacy_parities=self.parities, changed_domain_refusals=self.refusals,
                      captures=self.captures, invalid_source_llvm_invocations=0,
                      receipts_sha256=digest((self.root / 'receipts.json').read_bytes()))
        (self.root / 'u8-summary.json').write_text(json.dumps(report, indent=2) + '\n')


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
