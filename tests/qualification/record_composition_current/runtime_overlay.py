#!/usr/bin/env python3
"""Derive only the sealed current Unit3 test overlay; no I/O or replay authority."""
import ast
import re
import observer_adapters as adapter
import current_hooks

CURRENT_FILES = [{'after': {'bytes': 1083, 'sha256': 'dcebc5a5c11a90523e86fc41225a7358d4a3242d002b05bca2e407bb4427197a'}, 'before': {'bytes': 387, 'sha256': 'da20f3e1d112eeb5fbf0403bdd9950da301eed73895e565f1804f0dfc219ab88'}, 'path': 'src/frontend/mod.rs'}, {'after': {'bytes': 10157, 'sha256': '97440f37f6369271e9fee10bdaf222917d3118b4103afc061ff299b93e1b3a13'}, 'before': {'bytes': 10121, 'sha256': '6f8d3dc57cd22142850c4f22427481062b3fa36ec127120464684bc187940211'}, 'path': 'src/frontend/oir/mod.rs'}, {'after': {'bytes': 10556, 'sha256': '7fd4cb610a10e06a94207cc509efb6936544ddd0d3a084cc9f0f9eab6db05431'}, 'before': {'bytes': 10521, 'sha256': '9098ba8f33bb40408becb35f48079dcdeffe013ffcc93493bbdfb8c37f2493d7'}, 'path': 'src/frontend/oir/owned/mod.rs'}, {'after': {'bytes': 8836, 'sha256': 'e87155ed6d58c85518f0e0812bac9af42ad7517eebc11b58b2a9c8a5d5893a17'}, 'before': {'bytes': 8048, 'sha256': 'b1f898639b7799a6de20f0ce04c1d897323c59f6413972fd70a6de30bc73924d'}, 'path': 'src/frontend/oir/source/association.rs'}, {'after': {'bytes': 12729, 'sha256': '0af2f67eaa41b0e67105950683320b44d14eeeee0b07f617d8ad04deee286706'}, 'before': {'bytes': 11816, 'sha256': '274509c636a38c48f8b98cbebbccbfeb55d4abd349139cac8e3df44d4cf73944'}, 'path': 'src/frontend/oir/owned/source/association.rs'}, {'after': {'bytes': 7500, 'sha256': '672c776f4b75940c04ad6dfdaa989999ee2c9a0eb2fc18670f7ee22e86ba14d8'}, 'before': {'bytes': 7427, 'sha256': 'b18fe96214f67aae59b5b291090db491851d0d3ab2428b39082f4010cd0c3c27'}, 'path': 'src/frontend/oir/source/sealed.rs'}, {'after': {'bytes': 20210, 'sha256': '0a85c4c35aa4a053ec97452e4ac898e7499db1dac03836232347a8ac1f1c1718'}, 'before': {'bytes': 18562, 'sha256': '71620baca0cc8f7120937a07d4a957a66fa3f640f1b99a570ad85012cc3dbc19'}, 'path': 'src/frontend/oir/execute.rs'}, {'after': {'bytes': 82904, 'sha256': 'd11c5328054fcb6f967420f1c69aae142d3129dc34629bed77c139715d891abc'}, 'before': {'bytes': 80482, 'sha256': '6e5fbf2687161543f1b5d0b640dd182df30bd2b5c65e6dd7b12a54428ed976fe'}, 'path': 'src/frontend/oir/owned/execute.rs'}, {'after': {'bytes': 61252, 'sha256': 'd414e376631d2476de11738210c811ecabca5fd9760306fa5193b665442bef9f'}, 'before': {'bytes': 61158, 'sha256': '94994e04af35679352943af92caec78c35ea92fb61275e4b4d5275ee44577f6c'}, 'path': 'src/frontend/oir/native.rs'}, {'after': {'bytes': 100135, 'sha256': 'ce65e724c2ac8f982d8b8971a4945c6dc8912da61659f0d340b6d1ef8361434a'}, 'before': {'bytes': 100045, 'sha256': '3d723a53ff96aed27abbc690d1732dc085c7de154e7bc6ff563c3c7f7ef65152'}, 'path': 'src/frontend/oir/owned/native.rs'}, {'after': {'bytes': 9459, 'sha256': '68b72b333273db47158b01cb55ec81f1ccd309f6c5519c3a07d06f10caf22ef9'}, 'before': {'bytes': 0, 'sha256': 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'}, 'path': 'src/frontend/unit3_observer.rs'}, {'after': {'bytes': 3345, 'sha256': '235fb7964b083141da0c000f70930792fb2a13ff0d391a2ef8393739fdba008e'}, 'before': {'bytes': 0, 'sha256': 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'}, 'path': 'src/frontend/oir/unit3_raw_scalar.rs'}, {'after': {'bytes': 6279, 'sha256': '5d8b42dab3a707af67e0e3a2972bdfcca901a85792821a800e466f313ac4e870'}, 'before': {'bytes': 0, 'sha256': 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'}, 'path': 'src/frontend/oir/owned/unit3_raw_owned.rs'}, {'path': 'src/frontend/oir/owned/array_observe.rs', 'before': {'sha256': '59289c38e1e89d28fc5563e643fcaab12586c39ce4c48f46d3d7dcaa0a1137af', 'bytes': 16256}, 'after': {'sha256': '2a42af5b87d738630de34cf392ede82d4a8c0ad0ae0ce51deb658dc5fa9a8d85', 'bytes': 16504}}]

def derive_runtime_overlay(originals, original_prepare, original_patch):
    """Return 11 instrumented files and 3 exact archived/adapted test modules.

    The pinned archived recipe is evaluated only for its helper definitions,
    two literal settings, and ten source transformations. Its filesystem,
    manifest, CLI, materialization and patch-emission code is never executed.
    Input and output identities constrain every transformation in normal and
    optimized Python alike. Original archive identities are never retargeted.
    """
    expected = {row["path"]: row for row in CURRENT_FILES}
    existing = {path: row for path, row in expected.items() if row["before"]["bytes"]}
    adapter.require(set(originals) == set(existing), "current runtime source membership differs")
    for path, row in existing.items():
        adapter._identity(originals[path], row["before"], path)
    recipe = adapter.derive_observer_prepare(original_prepare)
    changed = {}
    def edit(path, transform):
        adapter.require(path in existing and path not in changed, "runtime recipe target differs")
        changed[path] = transform(originals[path].decode("utf-8")).encode("utf-8")
    namespace = {"re": re, "edit": edit}
    for node in ast.parse(recipe).body:
        helper = isinstance(node, ast.FunctionDef) and node.name != "edit"
        setting = isinstance(node, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id in ("J", "macros") for target in node.targets)
        call = (isinstance(node, ast.Expr) and isinstance(node.value, ast.Call)
                and isinstance(node.value.func, ast.Name) and node.value.func.id == "edit")
        if helper or setting or call:
            exec(compile(ast.Module([node], []), "<sealed-unit3-recipe>", "exec"), namespace)
    additions = adapter.archived_observer_additions(original_patch)
    additions["src/frontend/unit3_observer.rs"] = adapter.derive_journal(additions["src/frontend/unit3_observer.rs"])
    additions["src/frontend/oir/owned/unit3_raw_owned.rs"] = adapter.derive_raw_walker(additions["src/frontend/oir/owned/unit3_raw_owned.rs"])
    changed.update(additions)
    path = current_hooks.EVENT_PATH
    changed[path] = current_hooks.derive(path, originals[path])
    adapter.require(set(changed) == set(expected), "current runtime output membership differs")
    for path, data in changed.items():
        adapter._identity(data, expected[path]["after"], path)
    return changed
