#!/usr/bin/env python3
from pathlib import Path
import sys
import unittest
import transfer_hook as hook

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / 'tests/qualification/record_composition_current'))
import observer_adapters as a
import runtime_overlay as runtime


def original():
    inputs = {r['path']:(REPO/r['path']).read_bytes() for r in runtime.CURRENT_FILES if r['before']['bytes']}
    return runtime.derive_runtime_overlay(inputs, (REPO/a.IDENTITIES['prepare']['original']['path']).read_bytes(),
        (REPO/a.IDENTITIES['raw_walker']['archived_container']['path']).read_bytes())[hook.IDENTITY['path']]


class TransferHook(unittest.TestCase):
    def test_exact_identity_and_full_reversal(self):
        old=original();new=hook.derive(old)
        self.assertEqual(hook.identity(new),hook.IDENTITY['derived']);self.assertEqual(hook.reverse(new),old)
    def test_source_mutation_double_adaptation_and_derived_mutation_rejected(self):
        old=original();new=hook.derive(old)
        for bad in (old+b'\n',new):
            with self.assertRaises(ValueError):hook.derive(bad)
        with self.assertRaises(ValueError):hook.reverse(new+b'\n')
    def test_only_exact_transfer_write_loop_changes(self):
        old=original();new=hook.derive(old)
        self.assertEqual(old.replace(hook.OLD,b'<loop>'),new.replace(hook.NEW,b'<loop>'))
        self.assertEqual(hook.NEW.count(b'::event("payload_write"'),1)
        self.assertLess(hook.NEW.index(b'self.store_leaf(to, leaf, value, span)?;'),hook.NEW.index(b'::event("payload_write"'))
        self.assertNotIn(b'fn store_leaf',hook.NEW)
        self.assertNotIn(b'&mut child.payload',hook.NEW)
    def test_actual_before_after_and_field_leaf_agreement_are_bound(self):
        for part in (b'field.offset(), leaf.offset',b'field.value_ty(), ValueTy::Scalar(leaf.ty)',b'payload.get(offset..end)',b'fields.next()',b'fields.len().max(1)'):
            self.assertIn(part,hook.NEW)


if __name__=='__main__':unittest.main()
