#!/usr/bin/env python3
"""Bind finite enum-vector inputs and explicit exclusions to the full replay.

The unchanged retained controller owns all semantic/resource predicates and both
build profiles. Source and observer qualification remain explicitly provisional.
"""
import argparse
from contextlib import contextmanager
import datetime
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
import types


ROOT = Path(__file__).resolve().parents[1]
PREDECESSOR = "5917f581e69c0a0c69e4b5b059bb97dced55331d"
PREDECESSOR_POLICY = "scripts/verify_offline_vendor_compatibility.py"
PREDECESSOR_POLICY_PIN = (27286, "ee64cc7f63051764594a5866101cff9987ca7ddf20b764e7e1aa88f975c9b7a1")
REVIEWED_SOURCE_HEAD = "a54fc93c3d2ee47f098de59cf1d220f35a1fd86b"
PINS_PROVISIONAL = True
OBSERVER_PINS_PROVISIONAL = True
OBSERVER_ADAPTER = "scripts/owned_enum_vector_compatibility_observer.py"
OBSERVER_ADAPTER_PIN = (7495, "ff932fb13ae4db32fe15a389505be7a7ffa2fc0557b15f9b1b153c3602de3fd9")
V13_ADAPTER = "scripts/owned_record_vector_compatibility_observer.py"
V13_ADAPTER_PIN = (7268, "ac49c38f23b0d3a5929fe3d52edae1cb85341c72b16249522e30dc155e1fa8f3")
V13_DERIVED_PIN = (32684, "8e65804b8486ec2fda4bca21875c3c296f0b722bd6c40041ee1144b4c44e6042")

ENUM_CHANGED = frozenset((
    "native/experimental_owned_record.c",
    "native/experimental_owned_record.h",
    "src/frontend/declaration_index/generics.rs",
    "src/frontend/declaration_index/generics/nominal.rs",
    "src/frontend/declaration_index/generics/owned_candidate.rs",
    "src/frontend/declaration_index/sealed.rs",
    "src/frontend/native.rs",
    "src/frontend/oir/owned/execute.rs",
    "src/frontend/oir/owned/execute_composition.rs",
    "src/frontend/oir/owned/execute_experimental.rs",
    "src/frontend/oir/owned/execute_owned_vector.rs",
    "src/frontend/oir/owned/execute_resources.rs",
    "src/frontend/oir/owned/execute_vector.rs",
    "src/frontend/oir/owned/mod.rs",
    "src/frontend/oir/owned/native.rs",
    "src/frontend/oir/owned/native_composition.rs",
    "src/frontend/oir/owned/native_owned_vector.rs",
    "src/frontend/oir/owned/native_owned_vector_schemas.rs",
    "src/frontend/oir/owned/native_resources.rs",
    "src/frontend/oir/owned/owned_record_bounds.rs",
    "src/frontend/oir/owned/owned_record_codec.rs",
    "src/frontend/oir/owned/owned_vector_context.rs",
    "src/frontend/oir/owned/owned_vector_descriptors.rs",
    "src/frontend/oir/owned/owned_vector_plan.rs",
    "src/frontend/oir/owned/owned_vector_queries.rs",
    "src/frontend/oir/owned/owned_vector_resources.rs",
    "src/frontend/oir/owned/owned_vector_verified.rs",
    "src/frontend/oir/owned/reviewer_array_reference_tests.rs",
    "src/frontend/oir/owned/shape.rs",
    "src/frontend/oir/owned/source/array_pipeline_rows.rs",
    "src/frontend/oir/owned/source/budget.rs",
    "src/frontend/oir/owned/source/candidate_adapter.rs",
    "src/frontend/oir/owned/source/diagnostic.rs",
    "src/frontend/oir/owned/source/owned_vector_source.rs",
    "src/frontend/oir/owned/source/program.rs",
    "src/frontend/oir/owned/source/record_enum_layouts.rs",
    "src/frontend/oir/owned/source/resolve.rs",
    "src/frontend/oir/owned/source/typeck.rs",
    "src/frontend/oir/owned/vector_type.rs",
    "src/frontend/oir/owned/verified_program_queries.rs",
    "src/frontend/oir/owned_types.rs",
    "src/frontend/oir/owned_types/enums.rs",
    "src/frontend/oir/owned_types/owned_vector.rs",
    "src/frontend/oir/owned_types/resources.rs",
    "src/frontend/owned_vector_builtins.rs",
))
ENUM_ADDED = frozenset((
    "src/frontend/declaration_index/generics/owned_builtin_enum_key_tests.rs",
    "src/frontend/declaration_index/generics/owned_builtin_enum_keys.rs",
    "src/frontend/oir/owned/native_owned_enum_vector_schema_tests.rs",
    "src/frontend/oir/owned/owned_enum_vector_bounds_tests.rs",
    "src/frontend/oir/owned/owned_enum_vector_resource_tests.rs",
    "src/frontend/oir/owned/source/owned_enum_vector_boundary_tests.rs",
    "src/frontend/oir/owned/source/owned_enum_vector_cases.rs",
    "src/frontend/oir/owned/source/owned_enum_vector_source_tests.rs",
    "src/frontend/oir/owned_types/owned_enum_vector_payload_tests.rs",
    "src/frontend/oir/owned_types/owned_enum_vector_tests.rs",
    "src/frontend/owned_enum_vector_catalog_tests.rs",
))
REVIEWED_INPUTS = {
    "native/experimental_owned_record.c": (11933, "d07290a26d63c3f360997e32d2d4f2a3593a95832d09850de1d750aac4df455a"),
    "native/experimental_owned_record.h": (1602, "56f02ed4ed9cb5ea9c7ee8f453320265c2f5a05763cbfa9ee83dce2bb4bf03a4"),
    "src/frontend/declaration_index/generics.rs": (136792, "9b537bf78f18c932ea7c88b09570b5784726c4cdc407da89ff53045ccbd77f9c"),
    "src/frontend/declaration_index/generics/nominal.rs": (34232, "88f16fc1ad280b731edfb3be4fa35cf7e169b202288739d974f422a633beeb8f"),
    "src/frontend/declaration_index/generics/owned_builtin_enum_key_tests.rs": (27015, "22b36722409738ac56f9ebc8ed24f01c34015fcd2ebf912e12b057f333ba1253"),
    "src/frontend/declaration_index/generics/owned_builtin_enum_keys.rs": (11020, "e68a64b53c9ad816bcdf2a091285100a4edfab34ca92479c03b1d10dd9178dc0"),
    "src/frontend/declaration_index/generics/owned_candidate.rs": (82984, "d35ce98b67b94d20fcc9b7c2bc5899904e6767ccc14b3145da3485bbdc44afb0"),
    "src/frontend/declaration_index/sealed.rs": (121094, "9fc2034f0a55e5a5f02796cdda2e5d233405bcd74137ce4fe6723c91e62ec949"),
    "src/frontend/native.rs": (32268, "9b9ca1d108c7d244e6070d5abcc16700bdf1b20a18f869f21648bc8e5a24a675"),
    "src/frontend/oir/owned/execute.rs": (224320, "9153ed04ba01f481abc83b9920ffd5eb08d17840404d9a3ea5d8e1c481a31217"),
    "src/frontend/oir/owned/execute_composition.rs": (11432, "5c6db3508418b1458504924575ef4de178ab5b31ebda2f5155e30e24102caaf5"),
    "src/frontend/oir/owned/execute_experimental.rs": (28216, "492a4c2d6b62d2c5762f7c0283b44ef98ac60e200a138a1d809f9529cc998a71"),
    "src/frontend/oir/owned/execute_owned_vector.rs": (13995, "05960415fa0d7a0501a5b629003b837be1bb8df213c2deea990bb7d35140c913"),
    "src/frontend/oir/owned/execute_resources.rs": (16614, "c5f7937dd80bf28c7182b6e0af2e8105255c7057ab4ad77465b8e9aa243d390c"),
    "src/frontend/oir/owned/execute_vector.rs": (71933, "473aed33f0fa4eb807b4e2316d28498a876ba062e4cc097a065a980ee28bb8c9"),
    "src/frontend/oir/owned/mod.rs": (18669, "6593f9491ee568a8f8413dc55c9391dd950e766f595b504230e4dafd756e6f65"),
    "src/frontend/oir/owned/native.rs": (283298, "3646eaef49baa0c4d0b0e69d8bc1f123947c88cf32fc2df602ef883bbc883850"),
    "src/frontend/oir/owned/native_composition.rs": (31876, "0cb0ec3e5f58a6ae7b5c086cdbd6d4ce970877bbe34cca163df33b723edd58a1"),
    "src/frontend/oir/owned/native_owned_enum_vector_schema_tests.rs": (4194, "0ef86429aaa3d1d537deba5d5e52146dd108eef5415ac1f7bc2a795a26315854"),
    "src/frontend/oir/owned/native_owned_vector.rs": (7795, "447cb2cb31bdc27d74f1d027121c0653cd12355b8d1d37474f0fc2cea6a87278"),
    "src/frontend/oir/owned/native_owned_vector_schemas.rs": (13541, "606ee2307867300c19afe912d26599f3bad960d009a17807369a93b870735e16"),
    "src/frontend/oir/owned/native_resources.rs": (10202, "b4e402d962b5b58a1172f9ab71e082c5f94c4f82e3b58b171fa460edf30505c8"),
    "src/frontend/oir/owned/owned_enum_vector_bounds_tests.rs": (5418, "b5f379976c5fe4979ca90c171049b13ea382817694a843bdacfff1873ba86c1d"),
    "src/frontend/oir/owned/owned_enum_vector_resource_tests.rs": (13434, "a85355ca602f88dd92ba44de64a37962f48b0a9a1c27ce4fc3e219b10a16c887"),
    "src/frontend/oir/owned/owned_record_bounds.rs": (16335, "2d991061933394a68eff1f3cb31c08a3c45d5d4fb5bb8558b49c3da10713c856"),
    "src/frontend/oir/owned/owned_record_codec.rs": (15474, "d408bd0c4ba48bed1e0eb760883b4704085b9542b554ab3c2526acf8465abc9a"),
    "src/frontend/oir/owned/owned_vector_context.rs": (21050, "fa45a9629d241f5c38f526d0330e610ca8a61a9bacb97fb910ef392146216d40"),
    "src/frontend/oir/owned/owned_vector_descriptors.rs": (19835, "020b72818cd38011d0e0f75863172fa770eefefeced6201d586d6b769ab4ce7c"),
    "src/frontend/oir/owned/owned_vector_plan.rs": (5689, "0dcad2aeb9a36e4d8bab2983389d4e67d8604b3f9c66a2db823b9e98cb8bb4b4"),
    "src/frontend/oir/owned/owned_vector_queries.rs": (8545, "a3ceba34d9afdaf64846ee64f5eb0fdc04e0e6d2848a052d7998f2608b950ff5"),
    "src/frontend/oir/owned/owned_vector_resources.rs": (32967, "6090d432c2fe7c54b11cee3fc4ebf3a1ff6fda1370a57c682dc33635a906635a"),
    "src/frontend/oir/owned/owned_vector_verified.rs": (9686, "0fe9c99f7ef1a7ea0f6fc9aa34c5b988e6e3192fc3ef443d5dac7183d313c64b"),
    "src/frontend/oir/owned/reviewer_array_reference_tests.rs": (67266, "5c6144bc8c7c0268761961919245e46973cc42006340d179a848d892ed6e6e87"),
    "src/frontend/oir/owned/shape.rs": (67934, "a978e88b925f5c7915fb528acaa3e2717fc3c647f541c596d4737aa0039d3223"),
    "src/frontend/oir/owned/source/array_pipeline_rows.rs": (58592, "b96b0d70caacd0f14af2c7e8a3686a6d49f32e45a97fba9a24267f16e88729fb"),
    "src/frontend/oir/owned/source/budget.rs": (19815, "8c40619453ba3d38e3034487c23202712672ad75ffbfe18702c0c8f7373ded90"),
    "src/frontend/oir/owned/source/candidate_adapter.rs": (45477, "1ac365ba068267266045eb180bb2a03a0c57b7b486b766c6f4a8b28955c14acd"),
    "src/frontend/oir/owned/source/diagnostic.rs": (9236, "751750583651a2effb6e8d824667fcff9cfa85767602553ccbca0cc5c695ee53"),
    "src/frontend/oir/owned/source/owned_enum_vector_boundary_tests.rs": (13381, "e22a93fbe6cd606620807befb0f822a75427a28cd3f1bf0a60b6f8d132335c8f"),
    "src/frontend/oir/owned/source/owned_enum_vector_cases.rs": (7383, "439e6f19d00196e62a2a04135e5d3d3abb30067b5c3881202c85dc16730f7907"),
    "src/frontend/oir/owned/source/owned_enum_vector_source_tests.rs": (11969, "0b7dcb97e3ba0acbb96c123521fea0f2d5e9a3311dd2b1fac3b43f3254ac413b"),
    "src/frontend/oir/owned/source/owned_vector_source.rs": (6076, "bc11e0a8f9b6d72182135c3fc353b5e63fd1fca8642280c2af390558cac4a30a"),
    "src/frontend/oir/owned/source/program.rs": (51302, "bcb497aa0a5906d2f5e9537c877a2c746099e95afa0a4057c6a02773a7eb01d1"),
    "src/frontend/oir/owned/source/record_enum_layouts.rs": (13764, "5170681eae700b762d986ff61ffa068d1adc75fcf5a260c2d03fc09d4b20041e"),
    "src/frontend/oir/owned/source/resolve.rs": (173761, "3cbf2d4b84aadddecf8e6a10d5ac7dbdb12e7fe530ab49fb9c9a6918a1aedbdc"),
    "src/frontend/oir/owned/source/typeck.rs": (234152, "75193232eb6464d822fcf2d913f0cb70334dc00959ec47c170827c841ee23eb3"),
    "src/frontend/oir/owned/vector_type.rs": (4571, "ca3f185a77ad6c6f3720f22f7cb692cbf60a5210118722317b2bf0015c84fd0e"),
    "src/frontend/oir/owned/verified_program_queries.rs": (8583, "dc151a605e27fc8685a10ac2646da465383a8c6e0fcd8e451bc0e153f013659d"),
    "src/frontend/oir/owned_types.rs": (113553, "0db560949c84c14c2f7c288a8be5e4b0ce756872cffe4cbe285af4a59ef97ef4"),
    "src/frontend/oir/owned_types/enums.rs": (67830, "553b517817f2beb8ee1abeef940c19476880fd430329885be9dc0ae4b69251fd"),
    "src/frontend/oir/owned_types/owned_enum_vector_payload_tests.rs": (845, "9dd244635b629a2b4ac7c0810461ead6366f2fe9c0c29ca8f6a8ec2323d4afd8"),
    "src/frontend/oir/owned_types/owned_enum_vector_tests.rs": (3365, "56fe5c4b2b79568d772f49dd44a796afe56a2908739a7480c9e64a42b02ce144"),
    "src/frontend/oir/owned_types/owned_vector.rs": (16960, "6d9cf7f444058dccd46cb92bc8e78dce822af0104797bc93c79eb85e4f401e82"),
    "src/frontend/oir/owned_types/resources.rs": (33933, "0062c522375d3aba7e4895bad8fe56643b086b3683bf0d867de73ecc8ae967c5"),
    "src/frontend/owned_enum_vector_catalog_tests.rs": (3139, "f69d5e984738359da0c83ee516fd14ac170039b1af64d673ab3af58a64da6c59"),
    "src/frontend/owned_vector_builtins.rs": (12319, "44d4c153da66b1f0fc7b5180fdaf406e5c5de66c146b0b3f678846bcb324a2ff"),
}

# Independent literal selector/asset inventory; repinning a source never admits
# an unreviewed include, configuration, environment input or C directive.
REVIEWED_DIRECTIVES = {
    "native/experimental_owned_record.c": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": ((b'#include "experimental_owned_record.h"\n', 1), (b'#define OXID_OWNED_MAX_DEPTH 64u\n', 1), (b'#ifndef OXID_OWNED_VECTOR_SCHEMAS\n', 1), (b'#else\n', 1), (b'#endif\n', 1), (b'#ifndef OXID_OWNED_VECTOR_SCHEMAS\n', 1), (b'#else\n', 1), (b'#ifdef OXID_OWNED_ENUM_VECTOR_SCHEMAS\n', 1), (b'#else\n', 1), (b'#endif\n', 2), (b'#ifndef OXID_OWNED_VECTOR_SCHEMAS\n', 1), (b'#else\n', 1), (b'#endif\n', 1)),
        "build_script": None,
    },
    "native/experimental_owned_record.h": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": ((b'#ifndef OXID_EXPERIMENTAL_OWNED_RECORD_H\n', 1), (b'#define OXID_EXPERIMENTAL_OWNED_RECORD_H\n', 1), (b'#include "experimental_vector.h"\n', 1), (b'#ifdef OXID_OWNED_VECTOR_SCHEMAS\n', 1), (b'#endif\n', 1), (b'#ifdef OXID_OWNED_ENUM_VECTOR_SCHEMAS\n', 1), (b'#endif\n', 2)),
        "build_script": None,
    },
    "src/frontend/declaration_index/generics.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[cfg(test)]', 1), (b'#[derive(Default)]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 3), (b'#[allow(dead_code)]', 1), (b'#[derive(Debug, PartialEq, Eq)]', 3), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[derive(Clone, Copy)]', 2), (b'#[allow(clippy::too_many_arguments)]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/declaration_index/generics/nominal.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Debug, PartialEq, Eq)]', 2), (b'#[derive(Debug, Default, PartialEq, Eq)]', 1), (b'#[allow(dead_code)]', 1), (b'#[allow(clippy::too_many_arguments)]', 3)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/declaration_index/generics/owned_builtin_enum_key_tests.rs": {
        "legacy": ([], [b'#[path = "../../oir/owned/source/owned_enum_vector_cases.rs"]']),
        "attributes": ((b'#[path = "../../oir/owned/source/owned_enum_vector_cases.rs"]', 1), (b'#[allow(dead_code)]', 1), (b'#[test]', 1), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[test]', 4), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[test]', 1), (b'#[allow(dead_code)]', 6)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/declaration_index/generics/owned_builtin_enum_keys.rs": {
        "legacy": ([], [b'#[path = "owned_builtin_enum_key_tests.rs"]']),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[repr(C)]', 1), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[path = "owned_builtin_enum_key_tests.rs"]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/declaration_index/generics/owned_candidate.rs": {
        "legacy": ([], [b'#[path = "owned_builtin_enum_keys.rs"]']),
        "attributes": ((b'#[path = "owned_builtin_enum_keys.rs"]', 1), (b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[repr(C)]', 1), (b'#[derive(Debug)]', 2), (b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[allow(dead_code)]', 4), (b'#[allow(clippy::too_many_arguments)]', 4), (b'#[cfg(test)]', 1), (b'#[test]', 14), (b'#[allow(dead_code)]', 10), (b'#[test]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/declaration_index/sealed.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Debug)]', 1), (b'#[repr(C, align(8))]', 1), (b'#[derive(Debug)]', 1), (b'#[cfg(test)]', 3), (b'#[derive(Debug)]', 3), (b'#[cfg(test)]', 5), (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 3), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 4), (b'#[cfg(not(test))]', 1), (b'#[allow(unused_mut)]', 1), (b'#[cfg(test)]', 11), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 9), (b'#[test]', 2), (b'#[cfg(test)]', 3), (b'#[test]', 5)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/native.rs": {
        "legacy": ([b'include_str!("../../native/typed_preview.c")', b'include_str!("../../native/typed_character.c")', b'include_str!("../../native/typed_float.c")', b'include_bytes!(concat!(env!("OUT_DIR")', b'include_str!("../../native/typed_integer128.c")', b'include_str!("../../native/typed_bytebuf.c")', b'include_str!("../../native/experimental_bytebuf.c")', b'include_str!("../../native/experimental_files.c")', b'include_str!("../../native/experimental_arguments.c")', b'include_str!("../../native/experimental_bytebuf.h")', b'include_str!("../../native/experimental_vector.c")', b'include_str!("../../native/experimental_vector.h")', b'include_str!("../../native/typed_vector.c")', b'include_str!("../../native/experimental_owned_record.c")', b'include_str!("../../native/experimental_owned_record.h")', b'include_str!("../../native/experimental_owned_vector.c")', b'include_str!("../../native/experimental_owned_vector.h")'], []),
        "attributes": ((b'#[cfg(all(\n    any(target_os = "linux", target_os = "macos"),\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[cfg(not(all(\n    any(target_os = "linux", target_os = "macos"),\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n)))]', 1), (b'#[cfg(unix)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[cfg(test)]', 1), (b'#[test]', 2)),
        "macros": ((b'include_str!("../../native/typed_preview.c")', 1), (b'include_str!("../../native/typed_character.c")', 1), (b'include_str!("../../native/typed_float.c")', 1), (b'include_bytes!(concat!(env!("OUT_DIR"), "/float_format.a"))', 1), (b'env!("OUT_DIR")', 1), (b'include_str!("../../native/typed_integer128.c")', 1), (b'include_str!("../../native/typed_bytebuf.c")', 1), (b'include_str!("../../native/experimental_bytebuf.c")', 1), (b'include_str!("../../native/experimental_files.c")', 1), (b'include_str!("../../native/experimental_arguments.c")', 1), (b'include_str!("../../native/experimental_bytebuf.h")', 1), (b'include_str!("../../native/experimental_vector.c")', 1), (b'include_str!("../../native/experimental_vector.h")', 1), (b'include_str!("../../native/typed_vector.c")', 1), (b'include_str!("../../native/experimental_owned_record.c")', 1), (b'include_str!("../../native/experimental_owned_record.h")', 1), (b'include_str!("../../native/experimental_owned_vector.c")', 1), (b'include_str!("../../native/experimental_owned_vector.h")', 1), (b'cfg!(windows)', 1), (b'cfg!(target_os = "macos")', 1), (b'cfg!(windows)', 1), (b'cfg!(target_os = "macos")', 4)),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/execute.rs": {
        "legacy": ([], [b'#[path = "execute_floats.rs"]', b'#[path = "execute_values.rs"]', b'#[path = "execute_bitwise.rs"]', b'#[path = "execute_integer128.rs"]', b'#[path = "execute_integers.rs"]', b'#[path = "execute_experimental.rs"]', b'#[path = "execute_owned_vector.rs"]', b'#[path = "execute_vector.rs"]', b'#[path = "execute_composition.rs"]', b'#[path = "execute_resources.rs"]', b'#[path = "array_observe.rs"]', b'#[path = "execute_tests.rs"]', b'#[path = "execute_owned_vector_tests.rs"]', b'#[path = "execute_owned_vector_entry_tests.rs"]', b'#[path = "builtin_output_reference_tests.rs"]', b'#[path = "byte_storage_codec_tests.rs"]']),
        "attributes": ((b'#[path = "execute_floats.rs"]', 1), (b'#[path = "execute_values.rs"]', 1), (b'#[path = "execute_bitwise.rs"]', 1), (b'#[path = "execute_integer128.rs"]', 1), (b'#[path = "execute_integers.rs"]', 1), (b'#[path = "execute_experimental.rs"]', 1), (b'#[path = "execute_owned_vector.rs"]', 1), (b'#[path = "execute_vector.rs"]', 1), (b'#[path = "execute_composition.rs"]', 1), (b'#[path = "execute_resources.rs"]', 1), (b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy)]', 2), (b'#[cfg(test)]', 1), (b'#[derive(Clone, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy)]', 2), (b'#[cfg(test)]', 66), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[cfg(test)]', 13), (b'#[path = "array_observe.rs"]', 1), (b'#[cfg(test)]', 4), (b'#[path = "execute_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "execute_owned_vector_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "execute_owned_vector_entry_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "builtin_output_reference_tests.rs"]', 1), (b'#[cfg(test)]', 2), (b'#[test]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 1), (b'#[cfg(test)]', 1), (b'#[path = "byte_storage_codec_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 4)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/execute_composition.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[cfg(test)]', 1), (b'#[test]', 2)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/execute_experimental.rs": {
        "legacy": ([], [b'#[path = "execute_io_delivery.rs"]', b'#[path = "execute_file_delivery.rs"]']),
        "attributes": ((b'#[path = "execute_io_delivery.rs"]', 1), (b'#[path = "execute_file_delivery.rs"]', 1), (b'#[derive(Clone)]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[cfg(test)]', 3)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/execute_owned_vector.rs": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/execute_resources.rs": {
        "legacy": ([], [b'#[path = "execute_resource_tests.rs"]', b'#[path = "execute_vector_resource_tests.rs"]']),
        "attributes": ((b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[path = "execute_resource_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "execute_vector_resource_tests.rs"]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/execute_vector.rs": {
        "legacy": ([], [b'#[path = "execute_vector_tests.rs"]']),
        "attributes": ((b'#[derive(Clone, Copy)]', 1), (b'#[allow(dead_code)]', 3), (b'#[cfg(test)]', 6), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[cfg(test)]', 5), (b'#[path = "execute_vector_tests.rs"]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/mod.rs": {
        "legacy": ([], []),
        "attributes": ((b'#![allow(dead_code)]', 1), (b'#![allow(clippy::result_large_err)]', 1), (b'#[cfg(test)]', 3), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[derive(Clone, Debug)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Debug)]', 1), (b'#[derive(Clone, Debug)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Clone, Debug)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Debug)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Debug)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[derive(Clone, Debug)]', 2), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Debug)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Clone, Debug)]', 3), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 5), (b'#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 36)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native.rs": {
        "legacy": ([], [b'#[path = "native_bitwise.rs"]', b'#[path = "native_composition.rs"]', b'#[path = "native_experimental.rs"]', b'#[path = "native_floats.rs"]', b'#[path = "native_integer128.rs"]', b'#[path = "native_integers.rs"]', b'#[path = "native_numeric.rs"]', b'#[path = "native_owned_vector.rs"]', b'#[path = "native_owned_vector_schemas.rs"]', b'#[path = "native_reachability.rs"]', b'#[path = "native_resources.rs"]', b'#[path = "native_vector.rs"]', b'#[path = "builtin_input_native_tests.rs"]', b'#[path = "builtin_output_native_tests.rs"]', b'#[path = "native_tests.rs"]', b'#[path = "native_vector_tests.rs"]', b'#[path = "native_inventory_admission_tests.rs"]', b'#[path = "hosted_capacity_tests.rs"]', b'#[path = "byte_storage_native_tests.rs"]', b'#[path = "copy_record_native_tests.rs"]', b'#[path = "copy_record_debug_tests.rs"]', b'#[path = "record_vector_native_tests.rs"]', b'#[path = "native_owned_vector_schema_tests.rs"]', b'#[path = "native_owned_enum_vector_schema_tests.rs"]', b'#[path = "native_owned_vector_tests.rs"]', b'#[path = "native_owned_vector_commit_tests.rs"]']),
        "attributes": ((b'#[cfg(test)]', 1), (b'#[path = "native_bitwise.rs"]', 1), (b'#[path = "native_composition.rs"]', 1), (b'#[path = "native_experimental.rs"]', 1), (b'#[path = "native_floats.rs"]', 1), (b'#[path = "native_integer128.rs"]', 1), (b'#[path = "native_integers.rs"]', 1), (b'#[path = "native_numeric.rs"]', 1), (b'#[path = "native_owned_vector.rs"]', 1), (b'#[path = "native_owned_vector_schemas.rs"]', 1), (b'#[path = "native_reachability.rs"]', 1), (b'#[path = "native_resources.rs"]', 1), (b'#[path = "native_vector.rs"]', 1), (b'#[allow(dead_code)]', 2), (b'#[derive(Clone, Debug, Default)]', 1), (b'#[derive(Default)]', 1), (b'#[cfg(test)]', 2), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Clone, Copy, Default, Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[cfg(test)]', 3), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(clippy::too_many_arguments)]', 3), (b'#[derive(Clone, Copy)]', 1), (b'#[cfg(test)]', 2), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]', 1), (b'#[repr(transparent)]', 1), (b'#[allow(non_upper_case_globals)]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[allow(clippy::too_many_arguments)]', 2), (b'#[cfg(test)]', 2), (b'#[derive(Clone, Copy)]', 2), (b'#[allow(dead_code)]', 1), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[cfg(test)]', 2), (b'#[derive(Clone, Copy)]', 1), (b'#[cfg(test)]', 3), (b'#[path = "builtin_input_native_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "builtin_output_native_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "native_tests.rs"]', 1), (b'#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]', 1), (b'#[path = "native_vector_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[cfg(test)]', 1), (b'#[path = "native_inventory_admission_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "hosted_capacity_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 4), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[cfg(test)]', 1), (b'#[test]', 1), (b'#[allow(dead_code)]', 2), (b'#[test]', 1), (b'#[allow(dead_code)]', 4), (b'#[test]', 1), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[cfg(test)]', 1), (b'#[path = "byte_storage_native_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[cfg(test)]', 1), (b'#[path = "copy_record_native_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "copy_record_debug_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "record_vector_native_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "native_owned_vector_schema_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "native_owned_enum_vector_schema_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "native_owned_vector_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "native_owned_vector_commit_tests.rs"]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_composition.rs": {
        "legacy": ([], [b'#[path = "native_vector_composition_tests.rs"]']),
        "attributes": ((b'#[allow(dead_code)]', 1), (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[cfg(test)]', 1), (b'#[path = "native_vector_composition_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[cfg(all(\n        target_os = "linux",\n        target_pointer_width = "64",\n        target_endian = "little",\n        any(target_arch = "x86_64", target_arch = "aarch64")\n    ))]', 1), (b'#[test]', 1), (b'#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]', 1)),
        "macros": (),
        "c_preprocessor": ((b'#include <stdint.h>\n', 1), (b'#include <string.h>\n', 1), (b'#define CHECK(condition) do { if (!(condition)) return __LINE__; } while (0)\n', 1)),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_owned_enum_vector_schema_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_owned_vector.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[allow(dead_code)]', 2), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(clippy::too_many_arguments)]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_owned_vector_schemas.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy)]', 1), (b'#[allow(dead_code)]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_resources.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[allow(dead_code)]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[allow(clippy::too_many_arguments)]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_enum_vector_bounds_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 2),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_enum_vector_resource_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 5),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_record_bounds.rs": {
        "legacy": ([], [b'#[path = "owned_enum_vector_bounds_tests.rs"]']),
        "attributes": ((b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Debug)]', 2), (b'#[derive(Clone, Copy)]', 2), (b'#[repr(C)]', 1), (b'#[cfg(test)]', 1), (b'#[path = "owned_enum_vector_bounds_tests.rs"]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_record_codec.rs": {
        "legacy": ([], [b'#[path = "owned_record_bounds.rs"]']),
        "attributes": ((b'#[path = "owned_record_bounds.rs"]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[cfg(test)]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_context.rs": {
        "legacy": ([], [b'#[path = "owned_vector_queries.rs"]', b'#[path = "shape_declaration_queries.rs"]', b'#[path = "owned_vector_proof.rs"]', b'#[path = "owned_vector_verified.rs"]']),
        "attributes": ((b'#[path = "owned_vector_queries.rs"]', 1), (b'#[path = "shape_declaration_queries.rs"]', 1), (b'#[path = "owned_vector_proof.rs"]', 1), (b'#[path = "owned_vector_verified.rs"]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy)]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_descriptors.rs": {
        "legacy": ([], [b'#[path = "owned_vector_context.rs"]', b'#[path = "owned_vector_descriptor_tests.rs"]']),
        "attributes": ((b'#[path = "owned_vector_context.rs"]', 1), (b'#[derive(Clone, Copy, Debug, Default)]', 1), (b'#[derive(Debug)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[derive(Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[path = "owned_vector_descriptor_tests.rs"]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_plan.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy)]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_queries.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy)]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_resources.rs": {
        "legacy": ([], [b'#[path = "owned_vector_resource_tests.rs"]', b'#[path = "owned_enum_vector_resource_tests.rs"]']),
        "attributes": ((b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[repr(C)]', 1), (b'#[repr(C, align(16))]', 1), (b'#[cfg(test)]', 4), (b'#[path = "owned_vector_resource_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "owned_enum_vector_resource_tests.rs"]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_verified.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Debug)]', 2),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/reviewer_array_reference_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 14),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/shape.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Debug, Clone, Copy, PartialEq, Eq)]', 1), (b'#[derive(Clone, Default)]', 2), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[derive(Clone, Copy)]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/array_pipeline_rows.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[cfg(test)]', 1), (b'#[test]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/budget.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 6), (b'#[test]', 1), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[cfg(test)]', 1), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 8), (b'#[test]', 1), (b'#[cfg(test)]', 3), (b'#[derive(Clone, Copy)]', 1), (b'#[cfg(test)]', 4), (b'#[test]', 1), (b'#[cfg(test)]', 3)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/candidate_adapter.rs": {
        "legacy": ([], [b'#[path = "candidate_mutations.rs"]', b'#[path = "candidate_native.rs"]']),
        "attributes": ((b'#[derive(Default)]', 1), (b'#[test]', 1), (b'#[ignore = "source receipt collector; requires explicit source and fresh output directories"]', 1), (b'#[test]', 4), (b'#[cfg(test)]', 1), (b'#[path = "candidate_mutations.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "candidate_native.rs"]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/diagnostic.rs": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/owned_enum_vector_boundary_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 6),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/owned_enum_vector_cases.rs": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/owned_enum_vector_source_tests.rs": {
        "legacy": ([], [b'#[path = "owned_enum_vector_boundary_tests.rs"]', b'#[path = "owned_enum_vector_cases.rs"]']),
        "attributes": ((b'#[path = "owned_enum_vector_boundary_tests.rs"]', 1), (b'#[path = "owned_enum_vector_cases.rs"]', 1), (b'#[test]', 3)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/owned_vector_source.rs": {
        "legacy": ([], [b'#[path = "owned_vector_source_tests.rs"]']),
        "attributes": ((b'#[allow(dead_code)]', 4), (b'#[cfg(test)]', 1), (b'#[path = "owned_vector_source_tests.rs"]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/program.rs": {
        "legacy": ([], [b'#[path = "experimental_source_tests.rs"]', b'#[path = "i64_source_tests.rs"]', b'#[path = "owned_enum_vector_source_tests.rs"]', b'#[path = "owned_text_source_resource_tests.rs"]', b'#[path = "owned_vector_source_execution_tests.rs"]', b'#[path = "owned_vector_source_facade_tests.rs"]', b'#[path = "candidate_adapter.rs"]']),
        "attributes": ((b'#[cfg(test)]', 2), (b'#[path = "experimental_source_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "i64_source_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "owned_enum_vector_source_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "owned_text_source_resource_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "owned_vector_source_execution_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "owned_vector_source_facade_tests.rs"]', 1), (b'#[derive(Debug)]', 3), (b'#[test]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 5), (b'#[path = "candidate_adapter.rs"]', 1), (b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[allow(dead_code)]', 1), (b'#[derive(Debug)]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 2), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[derive(Debug)]', 1), (b'#[cfg(test)]', 3), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 1), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 3), (b'#[test]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/record_enum_layouts.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[cfg(test)]', 1), (b'#[test]', 3)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/resolve.rs": {
        "legacy": ([], [b'#[path = "resolver_paid_tests.rs"]', b'#[path = "resolver_enum_tests.rs"]', b'#[path = "enum_storage_failure_tests.rs"]', b'#[path = "builtin_signature_tests.rs"]', b'#[path = "output_typing_tests.rs"]', b'#[path = "byte_storage_type_tests.rs"]']),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 7), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 2), (b'#[derive(Debug)]', 1), (b'#[allow(clippy::large_enum_variant)]', 1), (b'#[derive(Debug)]', 2), (b'#[test]', 1), (b'#[cfg(test)]', 2), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(clippy::blocks_in_conditions)]', 1), (b'#[cfg(test)]', 4), (b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 1), (b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 11), (b'#[cfg(test)]', 2), (b'#[test]', 2), (b'#[cfg(test)]', 4), (b'#[test]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[path = "resolver_paid_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 1), (b'#[cfg(test)]', 1), (b'#[path = "resolver_enum_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "enum_storage_failure_tests.rs"]', 1), (b'#[allow(dead_code)]', 2), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 4), (b'#[cfg(test)]', 1), (b'#[path = "builtin_signature_tests.rs"]', 1), (b'#[cfg(test)]', 2), (b'#[path = "output_typing_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "byte_storage_type_tests.rs"]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/typeck.rs": {
        "legacy": ([], [b'#[path = "enum_type_tests.rs"]']),
        "attributes": ((b'#[derive(Debug)]', 3), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(clippy::large_enum_variant)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 19), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 1), (b'#[derive(Debug)]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 2), (b'#[cfg(test)]', 2), (b'#[allow(clippy::blocks_in_conditions)]', 1), (b'#[allow(dead_code)]', 3), (b'#[cfg(test)]', 2), (b'#[allow(dead_code)]', 4), (b'#[allow(clippy::type_complexity)]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 7), (b'#[test]', 1), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[test]', 1), (b'#[cfg(test)]', 2), (b'#[allow(clippy::too_many_arguments)]', 2), (b'#[allow(clippy::unused_enumerate_index)]', 1), (b'#[cfg(test)]', 3), (b'#[cfg(not(test))]', 1), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[cfg(test)]', 7), (b'#[test]', 2), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[cfg(test)]', 1), (b'#[test]', 4), (b'#[allow(dead_code)]', 1), (b'#[test]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 5), (b'#[cfg(test)]', 1), (b'#[path = "enum_type_tests.rs"]', 1), (b'#[allow(dead_code)]', 3), (b'#[test]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/vector_type.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/verified_program_queries.rs": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned_types.rs": {
        "legacy": ([], []),
        "attributes": ((b'#![allow(dead_code)]', 1), (b'#[allow(unused_imports)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(unused_imports)]', 2), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 3), (b'#[repr(transparent)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 4), (b'#[repr(C, align(8))]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[repr(C, align(8))]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 8), (b'#[derive(Debug)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1), (b'#[derive(Debug)]', 2), (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[derive(Debug)]', 2), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Clone, Copy, Default)]', 1), (b'#[derive(Default)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[allow(dead_code)]', 2), (b'#[test]', 29), (b'#[cfg(test)]', 7)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned_types/enums.rs": {
        "legacy": ([], [b'#[path = "owned_enum_vector_payload_tests.rs"]']),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[derive(Debug)]', 1), (b'#[cfg_attr(test, derive(Clone))]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1), (b'#[derive(Debug)]', 2), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[path = "owned_enum_vector_payload_tests.rs"]', 1), (b'#[derive(Debug)]', 2), (b'#[allow(clippy::too_many_arguments)]', 2), (b'#[derive(Clone, Copy)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 23)),
        "macros": ((b'cfg!(target_pointer_width = "64")', 1),),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned_types/owned_enum_vector_payload_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned_types/owned_enum_vector_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 1), (b'#[cfg(target_pointer_width = "64")]', 1)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned_types/owned_vector.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy, Debug)]', 2), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[test]', 3)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned_types/resources.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[allow(dead_code)]', 1), (b'#[repr(u8)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 8)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/owned_enum_vector_catalog_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 2),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/owned_vector_builtins.rs": {
        "legacy": ([], [b'#[path = "owned_enum_vector_catalog_tests.rs"]']),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]', 1), (b'#[repr(u8)]', 1), (b'#[cfg(test)]', 1), (b'#[path = "owned_enum_vector_catalog_tests.rs"]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[repr(u8)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 3), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 3)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load_predecessor(repo):
    path = repo / PREDECESSOR_POLICY
    require(not path.is_symlink() and not any(parent.is_symlink() for parent in path.parents),
            "symlink pinned offline-vendor controller")
    before = path.stat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and not before.st_mode & 0o111
            and before.st_size == PREDECESSOR_POLICY_PIN[0], "unsafe pinned offline-vendor controller")
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, name) for name in fields) == tuple(getattr(path.stat(), name) for name in fields)
            and (len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN,
            "changed pinned offline-vendor controller")
    module = types.ModuleType("_enum_vector_retained_offline_vendor")
    module.__file__ = str(path)
    exec(compile(body, str(path), "exec"), module.__dict__)
    return module


predecessor = load_predecessor(ROOT)
owned_policy = predecessor.predecessor.predecessor
library = predecessor.library
RETAINED_RUN = predecessor.RETAINED_RUN
PREDECESSOR_ADDED = predecessor.PARENT_ADDED
PARENT_ADDED = PREDECESSOR_ADDED | ENUM_ADDED
PREDECESSOR_POLICIES = {**predecessor.PREDECESSOR_POLICIES,
                        PREDECESSOR_POLICY: PREDECESSOR_POLICY_PIN[1]}
for _name in ("BASELINE", "BASELINE_SOURCE_SHA", "BINDING", "RETAINED", "RETAINED_RUNNER_SHA",
              "sha", "encoded", "identity", "regular", "git", "source_directives", "directive_manifest"):
    globals()[_name] = getattr(predecessor, _name)


def capture_sources(repo, baseline, parent):
    expected_parent = set(baseline) | PREDECESSOR_ADDED
    expected = expected_parent | ENUM_ADDED
    require(set(parent) == expected_parent and len(parent) == 576,
            "wrong pinned offline-vendor parent membership")
    require(len(ENUM_CHANGED) == 45 and len(ENUM_ADDED) == 11
            and ENUM_CHANGED <= expected_parent and not ENUM_ADDED & expected_parent
            and len(expected) == 587, "wrong finite enum-vector source membership")
    require(set(REVIEWED_INPUTS) == ENUM_CHANGED | ENUM_ADDED
            and set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS), "wrong enum-vector pin/directive membership")
    actual, directories = set(), set()
    for part in ("src", "native"):
        require((repo / part).is_dir() and not (repo / part).is_symlink(), "missing compiler source root")
        for directory, dirs, files in os.walk(repo / part, followlinks=False):
            for name in dirs + files:
                path = Path(directory) / name
                require(not path.is_symlink(), "symlink compiler source member")
                (directories if name in dirs else actual).add(path.relative_to(repo).as_posix())
    compiler = {name for name in expected if name.startswith(("src/", "native/"))}
    require(actual == compiler, "unexpected compiler source membership")
    expected_dirs = {str(path) for name in compiler for path in PurePosixPath(name).parents
                     if str(path) not in (".", "src", "native")}
    require(directories == expected_dirs, "unexpected compiler source directory")
    try:
        inputs = {name: regular(repo, name) for name in sorted(expected)}
    except OSError as error:
        raise ValueError("unreadable enum-vector compiler input: " + str(error)) from error
    for name, before in parent.items():
        if name not in ENUM_CHANGED:
            require(inputs[name] == before, "unreviewed change outside enum-vector boundary: " + name)
    for name, (length, digest) in REVIEWED_INPUTS.items():
        require((len(inputs[name]), sha(inputs[name])) == (length, digest),
                "enum-vector reviewed input changed: " + name)
        require(source_directives(name, inputs[name]) == REVIEWED_DIRECTIVES[name],
                "enum-vector compile-time input directive changed: " + name)
    return inputs


def authenticate_enum_observer(repo, prior):
    old_body = regular(repo, V13_ADAPTER)
    require((len(old_body), sha(old_body)) == V13_ADAPTER_PIN
            and prior.DERIVED_PIN == V13_DERIVED_PIN,
            "changed pinned v13 observer boundary")
    body = regular(repo, OBSERVER_ADAPTER)
    require((len(body), sha(body)) == OBSERVER_ADAPTER_PIN, "changed pinned enum-vector observer adapter")
    adapter = types.ModuleType("_enum_vector_observer_successor")
    adapter.__file__ = str(repo / OBSERVER_ADAPTER)
    exec(compile(body, adapter.__file__, "exec"), adapter.__dict__)
    require(adapter.PARENT == Path(V13_ADAPTER).name and adapter.PARENT_PIN == V13_ADAPTER_PIN
            and adapter.predecessor.descriptor() == prior.descriptor()
            and adapter.PARENT_DERIVED_PIN == prior.DERIVED_PIN,
            "enum-vector observer predecessor differs")
    require(regular(repo, V13_ADAPTER) == old_body, "v13 observer changed during authentication")
    return adapter, body


@contextmanager
def bound_observer():
    names = ("OWNED_VECTOR_OBSERVER_ADAPTER", "OWNED_VECTOR_OBSERVER_ADAPTER_PIN",
             "authenticate_owned_vector_observer_adapter")
    before = {name: getattr(owned_policy, name) for name in names}
    require(before["OWNED_VECTOR_OBSERVER_ADAPTER"] == V13_ADAPTER
            and before["OWNED_VECTOR_OBSERVER_ADAPTER_PIN"] == V13_ADAPTER_PIN,
            "unexpected inherited observer binding")
    # Authenticate the complete predecessor while its original globals still
    # identify v13. Calling this old loader after replacing its path would bind
    # the old pin to the new file and could never authenticate a real successor.
    prior, old_body = before["authenticate_owned_vector_observer_adapter"](ROOT)
    require((len(old_body), sha(old_body)) == V13_ADAPTER_PIN, "wrong authenticated v13 controller")
    authenticate_enum_observer(ROOT, prior)

    def authenticate(repo):
        return authenticate_enum_observer(repo, prior)

    bindings = {"OWNED_VECTOR_OBSERVER_ADAPTER": OBSERVER_ADAPTER,
                "OWNED_VECTOR_OBSERVER_ADAPTER_PIN": OBSERVER_ADAPTER_PIN,
                "authenticate_owned_vector_observer_adapter": authenticate}
    try:
        for name, value in bindings.items():
            setattr(owned_policy, name, value)
        yield
        require(library.run is RETAINED_RUN, "retained runner changed")
    finally:
        for name, value in before.items():
            setattr(owned_policy, name, value)


def current_manifest(inputs, baseline, checkout, parent):
    previous_names = set(baseline) | predecessor.PREDECESSOR_ADDED
    inherited = predecessor.current_manifest(inputs=parent, baseline=baseline, checkout=checkout,
                                            parent={name: parent[name] for name in previous_names})
    result = {**inherited, "source_policy_parent": PREDECESSOR,
              "source_policy_inputs": PREDECESSOR_POLICIES,
              "source_policy_input_members": len(PREDECESSOR_POLICIES),
              "predecessor_compiler_input_members": len(parent),
              "owned_enum_vector_pins_provisional": PINS_PROVISIONAL,
              "owned_enum_vector_observer_pins_provisional": OBSERVER_PINS_PROVISIONAL,
              "owned_enum_vector_reviewed_source_head": REVIEWED_SOURCE_HEAD,
              "owned_enum_vector_controller": identity("scripts/verify_owned_enum_vector_compatibility.py",
                                                          regular(ROOT, "scripts/verify_owned_enum_vector_compatibility.py")),
              "owned_enum_vector_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                                      for name, (length, digest) in sorted(REVIEWED_INPUTS.items())],
              "owned_enum_vector_directive_inventory": [
                  {"path": name, "before": directive_manifest(source_directives(name, parent[name])) if name in parent else None,
                   "after": directive_manifest(REVIEWED_DIRECTIVES[name])} for name in sorted(REVIEWED_INPUTS)],
              "changes": [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, value)}
                          for name, value in sorted(inputs.items()) if name in baseline and value != baseline[name]],
              "additions": sorted(PARENT_ADDED),
              "files": [identity(name, value) for name, value in sorted(inputs.items())],
              "unchanged_baseline_inputs": sum(inputs[name] == value for name, value in baseline.items()),
              "owned_enum_vector_semantics_qualified_by_this_gate": False,
              "owned_enum_vector_allocator_failures_qualified_by_this_gate": False,
              "owned_enum_vector_platform_qualified_by_this_gate": False,
              "general_heap_collections_qualified_by_this_gate": False,
              "g2_complete_qualified_by_this_gate": False}
    require(all(value is False for name, value in result.items() if name.endswith("qualified_by_this_gate")),
            "unexpected new qualification claim")
    return result


@contextmanager
def bound_predecessor():
    bindings = {"PREDECESSOR": PREDECESSOR, "PREDECESSOR_ADDED": PREDECESSOR_ADDED,
                "PREDECESSOR_POLICIES": PREDECESSOR_POLICIES, "PINS_PROVISIONAL": PINS_PROVISIONAL,
                "capture_sources": capture_sources, "current_manifest": current_manifest}
    require(library.run is RETAINED_RUN and predecessor.RETAINED_RUN is RETAINED_RUN
            and owned_policy.RETAINED_RUN is RETAINED_RUN, "retained execution function was replaced")
    with bound_observer(), predecessor.bound_predecessor() as context:
        before = {name: getattr(library, name) for name in bindings}
        try:
            for name, value in bindings.items():
                setattr(library, name, value)
            yield context
            require(library.run is RETAINED_RUN, "retained execution function changed")
        finally:
            for name, value in before.items():
                setattr(library, name, value)


def run_enum_observer_controls(repo, output, captured, seam, original, adapter, previous_executions):
    require(type(previous_executions) is int and previous_executions > 0,
            "missing inherited observer execution census")
    require(tuple(adapter.OWNED_ENUM_VECTOR_CONTROL_NAMES) == (
        "frontend::oir::unit2_observer::current_unit2_owned_enum_vector_adapter_denies_values",
        "frontend::oir::unit2_observer::current_unit2_owned_enum_vector_adapter_denies_shared_vectors",
        "frontend::oir::unit2_observer::current_unit2_owned_enum_vector_adapter_denies_exclusive_vectors"),
        "wrong enum-vector observer control names")
    run = output / "unit2/run"
    controls = output / "owned-enum-vector-observer-controls"
    controls.mkdir()
    protocol_body = regular(Path(seam["resource_package_root"]), "protocol.py")
    require(protocol_body == captured["historical_bytes"]["protocol.py"], "enum observer control protocol changed")
    protocol = types.ModuleType("_enum_vector_observer_control_protocol")
    exec(compile(protocol_body, "protocol.py", "exec"), protocol.__dict__)
    receipts = []
    for profile in ("debug", "release"):
        original.assert_unchanged(repo, captured)
        receipt_body = (run / (profile + "-receipt.json")).read_bytes()
        receipt = json.loads(receipt_body)
        binary, target = Path(receipt["binary"]), run / "target"
        require(binary.is_absolute() and target in binary.parents and not binary.is_symlink()
                and binary.resolve(strict=True).is_relative_to(target.resolve(strict=True)),
                "enum-vector observer control binary outside isolated build")
        require(sha(binary.read_bytes()) == receipt["binary_sha256"], "enum observer control binary changed")
        assembly_body = (run / "assembly.json").read_bytes()
        original.check_entries(run / "source", json.loads(assembly_body)["files"], exact=True)
        owned_policy.verify_current_observer(output, adapter, captured["observer"])
        commands, streams = [], {}
        for label, filters in (("list", ["--list", adapter.OWNED_ENUM_VECTOR_CONTROL_FILTER]),
                               ("run", [adapter.OWNED_ENUM_VECTOR_CONTROL_FILTER, "--test-threads=1", "--color", "never"])):
            argv = [str(binary), *filters]
            out, err = controls / (profile + "-" + label + ".stdout"), controls / (profile + "-" + label + ".stderr")
            with out.open("xb") as stdout, err.open("xb") as stderr:
                completed = library.subprocess.run(argv, cwd=run / "source", stdout=stdout, stderr=stderr, timeout=60)
            commands.append({"argv": argv, "exit_status": completed.returncode,
                             "stdout": identity(out.name, out.read_bytes()), "stderr": identity(err.name, err.read_bytes())})
            (controls / (profile + "-commands.json")).write_bytes(encoded(commands))
            require(completed.returncode == 0 and err.read_bytes() == b"", "enum observer controls failed; see raw streams")
            streams[label] = out.read_text(encoding="utf-8")
        names = list(adapter.OWNED_ENUM_VECTOR_CONTROL_NAMES)
        protocol.rust_listing(streams["list"], names)
        result = protocol.rust_success(streams["run"], names)
        require(sha(binary.read_bytes()) == receipt["binary_sha256"]
                and (run / "assembly.json").read_bytes() == assembly_body
                and (run / (profile + "-receipt.json")).read_bytes() == receipt_body,
                "enum observer control binary/source binding changed")
        original.check_entries(run / "source", json.loads(assembly_body)["files"], exact=True)
        owned_policy.verify_current_observer(output, adapter, captured["observer"])
        original.assert_unchanged(repo, captured)
        record = {"status": "passed", "profile": profile, "tests": names,
                  "enum_ordinals_per_control": 3, "result": result,
                  "binary": identity(str(binary), binary.read_bytes()),
                  "source_inputs_sha256": original.CURRENT_SOURCE_SHA,
                  "commands": identity(profile + "-commands.json", (controls / (profile + "-commands.json")).read_bytes())}
        path = controls / (profile + "-receipt.json")
        path.write_bytes(encoded(record))
        receipts.append(identity(str(path.relative_to(output)), path.read_bytes()))
    original.verify_unit2_result(output, captured, seam, False)
    return {"owned_enum_vector_observer_control_tests_per_profile": 3,
            "owned_enum_vector_observer_denials_per_profile": 9,
            "owned_enum_vector_observer_control_receipts": receipts,
            "inherited_test_function_executions": previous_executions,
            "test_function_executions": previous_executions + 6}


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(repo == ROOT and Path(__file__).resolve() == repo / "scripts/verify_owned_enum_vector_compatibility.py",
            "run the enum-vector controller from the selected checkout")
    require(output != repo and repo not in output.parents and not output.exists(), "output must be new outside repository")
    output.mkdir(parents=True)
    controller = regular(repo, "scripts/verify_owned_enum_vector_compatibility.py")
    report = {"schema": "oxid-owned-enum-vector-retained-wrapper-v1", "status": "running", "semantic_pass": False,
              "compiler_executions": 0, "controller": identity("scripts/verify_owned_enum_vector_compatibility.py", controller),
              "source_policy_parent": PREDECESSOR, "inherited_input_members": 576, "expected_current_input_members": 587,
              "pins_provisional": PINS_PROVISIONAL, "observer_pins_provisional": OBSERVER_PINS_PROVISIONAL,
              "reviewed_source_head": REVIEWED_SOURCE_HEAD, "new_feature_qualification": False,
              "platform_qualification": False, "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    try:
        require(args.prepare_only or not PINS_PROVISIONAL,
                "provisional enum-vector source pins permit prepare-only admission, not qualification")
        require(args.prepare_only or not OBSERVER_PINS_PROVISIONAL,
                "provisional enum-vector observer pins permit prepare-only admission, not qualification")
        require(args.prepare_only or not predecessor.PINS_PROVISIONAL, "provisional predecessor source pins")
        delegated = argparse.Namespace(**vars(args))
        delegated.output = output / "retained"
        with bound_predecessor() as context:
            status = RETAINED_RUN(delegated)
            require(status == 0, "unchanged retained replay refused; inspect retained raw evidence")
            artifact = "prepared.json" if args.prepare_only else "result.json"
            raw = regular(delegated.output, artifact)
            retained = json.loads(raw)
            require(retained.get("status") == ("prepared" if args.prepare_only else "passed")
                    and retained.get("semantic_pass") is (not args.prepare_only)
                    and retained.get("current_input_members") == 587
                    and retained.get("historical_results_apply_to_current") is False
                    and retained.get("controller") == identity(owned_policy.PREDECESSOR_POLICY,
                                                               regular(repo, owned_policy.PREDECESSOR_POLICY)),
                    "retained replay lacks the exact enum-vector source/materialization binding")
            require(context and context["output"] == delegated.output, "missing unchanged observer preparation context")
            report["enum_vector_observer_adapter"] = context["adapter"].descriptor()
            report["enum_vector_observer_adapter_controller"] = identity(OBSERVER_ADAPTER, regular(repo, OBSERVER_ADAPTER))
            if not args.prepare_only:
                report.update(owned_policy.run_owned_vector_observer_controls(
                    repo, delegated.output, context["selected"], context["seam"], context["original"],
                    context["adapter"], retained["test_function_executions"]))
                report.update(run_enum_observer_controls(
                    repo, delegated.output, context["selected"], context["seam"], context["original"],
                    context["adapter"], report["test_function_executions"]))
        require(regular(repo, "scripts/verify_owned_enum_vector_compatibility.py") == controller,
                "enum-vector controller changed")
        report.update(status="prepared" if args.prepare_only else "passed", semantic_pass=retained["semantic_pass"],
                      compiler_executions=retained["compiler_executions"], current_input_members=retained["current_input_members"],
                      retained_artifact=identity("retained/" + artifact, raw), retained_source=retained["current_source"])
        report["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        (output / artifact).write_bytes(encoded(report))
        print(json.dumps({"status": report["status"], "semantic_pass": report["semantic_pass"], "output": str(output)}))
        return 0
    except (Exception, KeyboardInterrupt) as error:
        report.update(status="failed", semantic_pass=False, error=f"{type(error).__name__}: {error}")
        (output / "failure.json").write_bytes(encoded(report))
        print(report["error"], file=sys.stderr)
        return 1
    finally:
        report.setdefault("finished_at_utc", datetime.datetime.now(datetime.timezone.utc).isoformat())
        (output / "wrapper.json").write_bytes(encoded(report))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--fetch-baseline", action="store_true")
    parser.add_argument("--allow-dirty", action="store_true")
    parser.add_argument("--prepare-only", action="store_true")
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
