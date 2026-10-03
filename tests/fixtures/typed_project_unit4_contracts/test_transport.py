#!/usr/bin/env python3
"""Synthetic transport boundary controls; no compiler or published oracle runs."""
import copy
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest import mock
import transport as t

def sha(data):
    return hashlib.sha256(data).hexdigest()

def gzip_bytes(data):
    out = io.BytesIO()
    with gzip.GzipFile(filename='', mode='wb', fileobj=out, mtime=0) as stream:
        stream.write(data)
    return out.getvalue()

def fixture(directory, contents=None, mutate=None, tar_mutate=None, blob_mutate=None):
    directory.mkdir()
    contents = contents or {
        '/workspace/fixture/parser.json': b'parser',
        '/workspace/fixture/public.json': b'public',
        '/workspace/fixture/source.ox': b'fn main() -> i32 { return 1; }\n',
        '/workspace/fixture/nested.json.gz': gzip_bytes(b'{"source_only":true}\n'),
    }
    rows = []
    tar_buf = io.BytesIO()
    with tarfile.open(fileobj=tar_buf, mode='w', format=tarfile.USTAR_FORMAT) as archive:
        for logical, data in sorted(contents.items()):
            row = {'logical_path':logical, 'archive_path':logical[1:], 'bytes':len(data), 'sha256':sha(data)}
            if logical.endswith('.gz'):
                decoded = gzip.decompress(data)
                row['decoded'] = {'bytes':len(decoded), 'sha256':sha(decoded)}
            rows.append(row)
            info = tarfile.TarInfo(logical[1:])
            info.size = len(data)
            info.mode = 0o644
            info.uid = info.gid = info.mtime = 0
            info.uname = info.gname = ''
            archive.addfile(info, io.BytesIO(data))
    tar = tar_buf.getvalue()
    if tar_mutate:
        tar = tar_mutate(tar, rows)
    blob = gzip_bytes(tar)
    if blob_mutate:
        blob = blob_mutate(blob)
    manifest = {
        'schema':t.SCHEMA,
        'members':rows,
        'archive':{'path':'authorities.tar.gz','bytes':len(blob),'sha256':sha(blob),'decoded_bytes':len(tar),'decoded_sha256':sha(tar),'member_count':len(rows),'member_bytes':sum(r['bytes'] for r in rows)},
        'active_contracts':{key:{'logical_path':logical,'sha256':sha(contents[logical])} for key,logical in [('parser','/workspace/fixture/parser.json'),('public','/workspace/fixture/public.json')]},
    }
    trusted_contracts = copy.deepcopy(manifest['active_contracts'])
    if mutate:
        mutate(manifest)
    raw_manifest = json.dumps(manifest).encode()
    (directory/'transport-manifest.json').write_bytes(raw_manifest)
    # Synthetic fixtures explicitly replace trust roots only inside this test process.
    t.TRUSTED_MANIFEST_SHA256 = sha(raw_manifest)
    t.TRUSTED_CONTRACTS = trusted_contracts
    (directory/'authorities.tar.gz').write_bytes(blob)
    return manifest

class TransportControls(unittest.TestCase):
    def setUp(self):
        manifest_pin, contract_pin = t.TRUSTED_MANIFEST_SHA256, t.TRUSTED_CONTRACTS
        self.addCleanup(setattr, t, 'TRUSTED_MANIFEST_SHA256', manifest_pin)
        self.addCleanup(setattr, t, 'TRUSTED_CONTRACTS', contract_pin)
        self.temp = tempfile.TemporaryDirectory(prefix='oxid-contract-transport-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.package = self.root/'package'
        self.dest = self.root/'materialized'

    def rejects(self, **kwargs):
        fixture(self.package, **kwargs)
        with self.assertRaises(t.Reject):
            t.materialize(self.package, self.dest)
        self.assertFalse(os.path.lexists(self.dest), 'invalid archive created destination')

    def test_valid_roundtrip_and_exact_resolver(self):
        expected = fixture(self.package)
        got = t.materialize(self.package, self.dest)
        self.assertEqual(got, expected)
        t.verify_materialized(got, self.dest)
        p = t.resolve_original_path(got, self.dest, '/workspace/fixture/source.ox')
        self.assertEqual(p.read_bytes(), b'fn main() -> i32 { return 1; }\n')
        with self.assertRaises(t.Reject):
            t.resolve_original_path(got, self.dest, '/workspace/fixture/unlisted.ox')

    def test_self_consistent_stale_package_rejected(self):
        fixture(self.package)
        expected_pin, expected_contracts = t.TRUSTED_MANIFEST_SHA256, copy.deepcopy(t.TRUSTED_CONTRACTS)
        other = self.root/'stale'
        fixture(other, contents={'/workspace/fixture/parser.json':b'stale-parser','/workspace/fixture/public.json':b'stale-public'})
        t.TRUSTED_MANIFEST_SHA256, t.TRUSTED_CONTRACTS = expected_pin, expected_contracts
        with self.assertRaises(t.Reject):t.materialize(other,self.dest)
        self.assertFalse(self.dest.exists())

    def test_stale_freeze_rejected_even_with_format_manifest_pin(self):
        fixture(self.package)
        t.TRUSTED_CONTRACTS['public']['sha256']='0'*64
        with self.assertRaises(t.Reject):t.materialize(self.package,self.dest)
        self.assertFalse(self.dest.exists())

    def test_bad_compressed_identity(self):
        self.rejects(mutate=lambda m:m['archive'].__setitem__('sha256','0'*64))

    def test_bad_decoded_identity(self):
        self.rejects(mutate=lambda m:m['archive'].__setitem__('decoded_sha256','0'*64))

    def test_member_content_mutation(self):
        def change(raw, rows):
            raw = bytearray(raw)
            raw[512] ^= 1
            return bytes(raw)
        self.rejects(tar_mutate=change)

    def test_duplicate_member(self):
        self.rejects(mutate=lambda m:m['members'].append(copy.deepcopy(m['members'][0])))

    def test_unsorted_inventory(self):
        self.rejects(mutate=lambda m:m['members'].reverse())

    def test_unsafe_member_paths(self):
        for bad in ['/absolute','workspace/../escape','workspace//empty','workspace/./dot','workspace\\backslash','workspace/C:drive','workspace/CON','workspace/trailing.']:
            with self.subTest(path=bad):
                package=self.root/('case-'+str(len(list(self.root.iterdir()))))
                def change(m):
                    m['members'][0]['archive_path']=bad
                    m['members'][0]['logical_path']='/'+bad
                fixture(package,mutate=change)
                with self.assertRaises(t.Reject):t.materialize(package,self.dest)
                self.assertFalse(self.dest.exists())

    def test_file_directory_collision(self):
        contents={'/workspace/fixture/parser.json':b'parser','/workspace/fixture/public.json':b'public','/workspace/fixture/a':b'file','/workspace/fixture/a/b':b'nested'}
        self.rejects(contents=contents)

    def test_portable_directory_case_collision(self):
        contents={'/workspace/fixture/parser.json':b'parser','/workspace/fixture/public.json':b'public','/workspace/fixture/A/x':b'x','/workspace/fixture/a/y':b'y'}
        self.rejects(contents=contents)

    def test_symlink_hardlink_directory_and_pax_headers(self):
        for kind in [tarfile.SYMTYPE,tarfile.LNKTYPE,tarfile.DIRTYPE,tarfile.XHDTYPE,tarfile.GNUTYPE_LONGNAME]:
            with self.subTest(kind=kind):
                package=self.root/('kind-'+kind.decode())
                def change(raw,rows):
                    info=tarfile.TarInfo(rows[0]['archive_path']);info.type=kind;info.linkname='escape';info.size=rows[0]['bytes']
                    return info.tobuf(format=tarfile.USTAR_FORMAT)+raw[512:]
                fixture(package,tar_mutate=change)
                with self.assertRaises(t.Reject):t.materialize(package,self.dest)
                self.assertFalse(self.dest.exists())

    def test_noncanonical_header_metadata(self):
        def change(raw,rows):
            info=tarfile.TarInfo(rows[0]['archive_path']);info.size=rows[0]['bytes'];info.mode=0o777
            return info.tobuf(format=tarfile.USTAR_FORMAT)+raw[512:]
        self.rejects(tar_mutate=change)

    def test_nonzero_member_padding(self):
        def change(raw,rows):
            raw=bytearray(raw);raw[512+rows[0]['bytes']]=1;return bytes(raw)
        self.rejects(tar_mutate=change)

    def test_extra_tar_member(self):
        def change(raw,rows):
            offset=sum(512+((r['bytes']+511)//512)*512 for r in rows)
            info=tarfile.TarInfo('workspace/hidden.ox');info.size=1
            hidden=info.tobuf(format=tarfile.USTAR_FORMAT)+b'x'+b'\0'*511
            return raw[:offset]+hidden+raw[offset+len(hidden):]
        self.rejects(tar_mutate=change)

    def test_trailing_tar_garbage(self):
        self.rejects(tar_mutate=lambda raw,rows:raw[:-1]+b'x')

    def test_truncated_concatenated_and_trailing_gzip(self):
        for label,change in [('truncated',lambda b:b[:-5]),('concatenated',lambda b:b+gzip_bytes(b'')),('trailing',lambda b:b+b'garbage')]:
            with self.subTest(label=label):
                package=self.root/label;fixture(package,blob_mutate=change)
                with self.assertRaises(t.Reject):t.materialize(package,self.dest)
                self.assertFalse(self.dest.exists())

    def test_outer_decompression_bound(self):
        self.rejects(mutate=lambda m:m['archive'].__setitem__('decoded_bytes',64))

    def test_nested_decompression_bound(self):
        def change(m):
            next(r for r in m['members'] if 'decoded' in r)['decoded']['bytes']=1
        self.rejects(mutate=change)

    def test_nested_decoded_identity(self):
        def change(m):
            next(r for r in m['members'] if 'decoded' in r)['decoded']['sha256']='0'*64
        self.rejects(mutate=change)

    def test_duplicate_json_key(self):
        fixture(self.package)
        p=self.package/'transport-manifest.json';raw=p.read_text();p.write_text('{"schema":"wrong",'+raw[1:]);t.TRUSTED_MANIFEST_SHA256=sha(p.read_bytes())
        with self.assertRaises(t.Reject):t.materialize(self.package,self.dest)
        self.assertFalse(self.dest.exists())

    def test_wrong_active_contract_binding(self):
        self.rejects(mutate=lambda m:m['active_contracts']['public'].__setitem__('sha256','0'*64))

    def test_existing_destination_unchanged(self):
        fixture(self.package);self.dest.mkdir();(self.dest/'sentinel').write_bytes(b'keep')
        with self.assertRaises(t.Reject):t.materialize(self.package,self.dest)
        self.assertEqual(list(self.dest.iterdir()),[self.dest/'sentinel'])
        self.assertEqual((self.dest/'sentinel').read_bytes(),b'keep')

    def test_symlink_archive_rejected(self):
        fixture(self.package);p=self.package/'authorities.tar.gz';target=self.root/'saved.gz';p.rename(target)
        try:p.symlink_to(target)
        except OSError as error:self.skipTest('symlink capability: '+str(error))
        with self.assertRaises(t.Reject):t.materialize(self.package,self.dest)
        self.assertFalse(self.dest.exists())

    def test_symlink_destination_ancestor_rejected(self):
        fixture(self.package);real=self.root/'real';real.mkdir();link=self.root/'link'
        try:link.symlink_to(real,target_is_directory=True)
        except OSError as error:self.skipTest('symlink capability: '+str(error))
        with self.assertRaises(t.Reject):t.materialize(self.package,link/'out')
        self.assertFalse((real/'out').exists())

    def test_extra_missing_and_changed_materialized_files(self):
        manifest=fixture(self.package);t.materialize(self.package,self.dest)
        extra=self.dest/'extra';extra.write_bytes(b'x')
        with self.assertRaises(t.Reject):t.verify_materialized(manifest,self.dest)
        extra.unlink();target=self.dest/'workspace/fixture/source.ox';data=target.read_bytes();target.unlink()
        with self.assertRaises(t.Reject):t.verify_materialized(manifest,self.dest)
        target.write_bytes(data+b'x')
        with self.assertRaises(t.Reject):t.verify_materialized(manifest,self.dest)

    def test_extra_empty_materialized_directory(self):
        manifest=fixture(self.package);t.materialize(self.package,self.dest);(self.dest/'extra-dir').mkdir()
        with self.assertRaises(t.Reject):t.verify_materialized(manifest,self.dest)

if __name__=='__main__':
    unittest.main(verbosity=2)
