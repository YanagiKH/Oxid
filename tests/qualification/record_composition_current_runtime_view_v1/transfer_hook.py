#!/usr/bin/env python3
"""One exact reversible test-only transfer write-site migration."""
import hashlib

IDENTITY = {'schema': 'oxid-record-current-transfer-hook-v1', 'path': 'src/frontend/oir/owned/execute.rs', 'input_stage': 'exact derived record_composition_current/runtime_overlay.py execute file before controls', 'original': {'bytes': 82904, 'sha256': 'd11c5328054fcb6f967420f1c69aae142d3129dc34629bed77c139715d891abc'}, 'derived': {'bytes': 85328, 'sha256': 'd81814d49827d7dc37e52d14b82a76e7f507cf9407954253b10bae2e2813335d'}, 'substitutions': 1, 'scope': 'successful transfer_payload store_leaf site only; scalar fields in original order; empty sentinel and Invoke excluded; no postprocessed traces'}
OLD = b'        for leaf in declarations\n            .leaves(record)\n            .map_err(|_| bad("transfer leaves", span))?\n        {\n            let value = self.load_leaf(from, leaf, span)?;\n            self.store_leaf(to, leaf, value, span)?;\n        }\n'
NEW = b'        // Test-only historical scalar-record transfer observation. Pair actual\n        // root fields with actual leaves; never infer writes from a later trace.\n        #[cfg(test)]\n        let mut unit3_fields = if crate::frontend::unit3_observer::enabled() {\n            let AggregateTy::Record(record_id) = record else {\n                panic!("OBSERVATION_UNSUPPORTED: historical record transfer");\n            };\n            let fields = declarations.fields(record_id).expect("verified record fields");\n            assert!(fields.iter().all(|field| matches!(field.value_ty(), ValueTy::Scalar(_))),\n                "OBSERVATION_UNSUPPORTED: historical scalar record transfer");\n            Some(fields.iter())\n        } else {\n            None\n        };\n        #[cfg(test)]\n        let unit3_expected_leaves = unit3_fields.as_ref().map(|fields| fields.len().max(1));\n        #[cfg(test)]\n        let mut unit3_leaf_count = 0usize;\n        for leaf in declarations\n            .leaves(record)\n            .map_err(|_| bad("transfer leaves", span))?\n        {\n            let value = self.load_leaf(from, leaf, span)?;\n            #[cfg(test)]\n            let unit3_write = if let Some(fields) = unit3_fields.as_mut() {\n                unit3_leaf_count += 1;\n                fields.next().map(|field| {\n                    assert_eq!(field.offset(), leaf.offset, "historical field/leaf offset");\n                    assert_eq!(field.value_ty(), ValueTy::Scalar(leaf.ty), "historical field/leaf type");\n                    let offset = destination.start.checked_add(leaf.offset).expect("preflighted leaf offset");\n                    let end = offset.checked_add(scalar_size(leaf.ty)).expect("preflighted leaf range");\n                    let before = self.frames[to.frame as usize].payload.get(offset..end).map(|bytes| bytes.to_vec());\n                    (field.id(), offset, end, before)\n                })\n            } else {\n                None\n            };\n            self.store_leaf(to, leaf, value, span)?;\n            #[cfg(test)]\n            if let Some((field, offset, end, before)) = unit3_write {\n                crate::frontend::unit3_observer::event("payload_write", &(\n                    to, field, value, span, before,\n                    self.frames[to.frame as usize].payload.get(offset..end),\n                ));\n            }\n        }\n        #[cfg(test)]\n        if let Some(expected) = unit3_expected_leaves {\n            assert_eq!(unit3_leaf_count, expected, "historical transfer leaf count");\n            assert_eq!(unit3_fields.expect("enabled field iterator").len(), 0,\n                "historical transfer omitted field");\n        }\n'

def identity(data):
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def derive(original):
    if type(original) is not bytes or identity(original) != IDENTITY["original"]:
        raise ValueError("exact prior overlay identity required")
    if original.count(OLD) != 1:
        raise ValueError("exact transfer loop cardinality required")
    result = original.replace(OLD, NEW)
    if identity(result) != IDENTITY["derived"] or reverse(result) != original:
        raise ValueError("derived transfer overlay or reversal differs")
    return result


def reverse(derived):
    if type(derived) is not bytes or identity(derived) != IDENTITY["derived"]:
        raise ValueError("exact migrated overlay identity required")
    if derived.count(NEW) != 1:
        raise ValueError("exact migrated transfer loop cardinality required")
    result = derived.replace(NEW, OLD)
    if identity(result) != IDENTITY["original"]:
        raise ValueError("prior transfer overlay reversal differs")
    return result
