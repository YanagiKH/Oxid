"""Inactive published-main source transport for the formatter checkpoint.

A separately reviewed dispatcher must authenticate this module, its retained
public API and a fixed seal before calling admit(). This file generates nothing,
runs no compiler and never treats a selected source view as a full Git checkout.
"""
import json

from adapters import binding, need, verify
import source_transition as retained

CURRENT_NAME = "published-formatter-source-v1.json"
AUTHORITY_NAME = "published-formatter-authority-v1.json"
PATCH_NAME = "published-formatter-transition-v1.patch"
PREDECESSOR_NAME = "lexer-reservation-source-v2.json"
PROOF_NAME = "published-main-source-proof-v1.json"
PREDECESSOR_SHA = "9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261"
PREDECESSOR_BYTES = 74328
PUBLISHED_MAIN = {
    "head": "15eac633e167c261ba0246ced49cfdc1c6cef434",
    "tree": "f66d5ae31272268c29f520ec523ee1fa31c15ee9",
}
SOURCE_CHECKPOINT = {
    "head": "fbff527c895e6cd4ec9350bd8b5d460bc20bb80d",
    "tree": "89e90c3ec83d0712d0037a0407fc6571883c5e9c",
    "parent": PUBLISHED_MAIN["head"],
}
HISTORICAL_SOURCE = {
    "head": "b3abc9f0dda99d6d8fe65d9c3a9ed31dedcbd489",
    "tree": "7f5c9aa08c569c4d0b5a27391d8fc68337075d36",
}
TRANSITION_PATHS = ("src/main.rs",)
BEFORE_BLOB = "fad8a28dd6914539f5b1a3d371f016bf3b1c47f2"
AFTER_BLOB = "476fe16b8506299475c5acbacf1b6976cc08324b"
RECIPE = "python-difflib-exact-unified-three-context-v1"
SEAL_FIELDS = frozenset((
    "schema", "source_manifest", "authority", "patch", "source_proof",
    "predecessor_view",
))
AUTHORITY_FIELDS = frozenset((
    "schema", "recipe", "published_main", "source_checkpoint",
    "retained_historical_source", "predecessor_source_manifest",
    "current_source_manifest", "transition_patch", "source_proof",
    "transition_paths", "predecessor_input_identities",
    "current_input_identities", "compile_time_fixture_inputs",
    "compile_time_include_directives", "unit2_accounting_dependencies",
    "current_source_members", "predecessor_source_members",
    "compiler_source_members", "compiler_bodies", "source_only",
    "execution_qualified",
))
PROOF_FIELDS = frozenset((
    "schema", "published_main", "source_checkpoint",
    "retained_historical_source", "retained_source_manifest",
    "predecessor_input_identities", "current_input_identities",
    "complete_compiler_paths", "complete_pr_changed_files",
    "git_object_evidence", "source_only", "execution_qualified",
))
MANIFEST_OUTER_FIELDS = frozenset((
    "published_main_head", "published_main_tree",
    "published_source_predecessor_sha256",
))
EVIDENCE_PATHS = {
    "published_main": "tests/qualification/published_formatter_current/source-proof/published-main-git-readback-v1.json",
    "source_checkpoint": "tests/qualification/published_formatter_current/source-proof/pr60-source-git-readback-v1.json",
    "verifier": "tests/qualification/published_formatter_current/generate_published_source_v1.py",
    "generation_receipt": "tests/qualification/published_formatter_current/source-proof/published-source-generation-v1.json",
}
READBACK_PINS = {
    "published_main": (570203, "8234cd0e4125740960258011d49d40a7068383aed3d72752777cdd66eadd0788"),
    "source_checkpoint": (568665, "dfea14e0092143c9d348a073c95b264f52ae0e67c1e08179fe8edae64c755da7"),
}


def pinned(package_bytes, descriptor, name):
    need(set(descriptor) == {"path", "bytes", "sha256"}
         and descriptor["path"] == name, "wrong named source descriptor")
    raw = package_bytes[name]
    verify(raw, descriptor, name)
    return json.loads(raw)


def manifest_rows(identities):
    return [{key: row[key] for key in ("path", "bytes", "sha256")}
            for row in identities]


def admit(repo, package_bytes, seal, api):
    """Read-only admission; all output maps are bytes, never live helper modules.

    The fixed proof's Git object evidence is independently generated/reviewed
    before sealing. This admission authenticates its exact bytes, complete source
    identities, and reversible relationship; it does not rerun a missing
    historical Git recipe or assert that compiler/runtime gates have passed.
    """
    need(set(seal) == SEAL_FIELDS
         and seal["schema"] == "oxid-published-main-source-seal-v1",
         "missing fixed published-source seal")
    current = pinned(package_bytes, seal["source_manifest"], CURRENT_NAME)
    authority = pinned(package_bytes, seal["authority"], AUTHORITY_NAME)
    proof = pinned(package_bytes, seal["source_proof"], PROOF_NAME)
    patch = package_bytes[PATCH_NAME]
    need(seal["patch"]["path"] == PATCH_NAME, "wrong named transition patch")
    verify(patch, seal["patch"], PATCH_NAME)
    predecessor_descriptor = {
        "path": PREDECESSOR_NAME, "bytes": PREDECESSOR_BYTES,
        "sha256": PREDECESSOR_SHA,
    }
    predecessor = pinned(package_bytes, predecessor_descriptor, PREDECESSOR_NAME)
    need(package_bytes["current-source.json"] == package_bytes[CURRENT_NAME],
         "current alias differs from named published source")
    need(predecessor["reviewed_source_head"] == HISTORICAL_SOURCE["head"]
         and predecessor["source_only_tree"] == HISTORICAL_SOURCE["tree"],
         "historical source checkpoint was relabeled")
    need(set(authority) == AUTHORITY_FIELDS
         and authority["schema"] == "oxid-published-main-source-transition-v1"
         and authority["recipe"] == RECIPE
         and authority["published_main"] == PUBLISHED_MAIN
         and authority["source_checkpoint"] == SOURCE_CHECKPOINT
         and authority["retained_historical_source"] == HISTORICAL_SOURCE,
         "wrong published/historical provenance or authority fields")
    need(authority["predecessor_source_manifest"] == predecessor_descriptor
         and authority["current_source_manifest"] == seal["source_manifest"]
         and authority["transition_patch"] == seal["patch"]
         and authority["source_proof"] == seal["source_proof"],
         "source authority descriptor mismatch")
    need(authority["source_only"] is True
         and authority["execution_qualified"] is False,
         "source admission cannot claim execution qualification")
    need(current["reviewed_source_head"] == SOURCE_CHECKPOINT["head"]
         and current["source_only_tree"] == SOURCE_CHECKPOINT["tree"]
         and current["published_main_head"] == PUBLISHED_MAIN["head"]
         and current["published_main_tree"] == PUBLISHED_MAIN["tree"]
         and current["published_source_predecessor_sha256"] == PREDECESSOR_SHA,
         "wrong current published source association")
    omit = {"files", "purpose", "reviewed_source_head", "source_only_tree"}
    need(set(current) == set(predecessor) | MANIFEST_OUTER_FIELDS
         and {k: v for k, v in current.items()
              if k not in omit | MANIFEST_OUTER_FIELDS}
         == {k: v for k, v in predecessor.items() if k not in omit},
         "changed retained source provenance")

    before = retained.rows_map(predecessor["files"])
    after = retained.rows_map(current["files"])
    need(len(before) == len(after) == 376 and list(before) == list(after),
         "missing, extra or reordered selected source member")
    need([name for name in after if after[name] != before[name]]
         == authority["transition_paths"] == list(TRANSITION_PATHS),
         "wrong exact one-file source transition")
    inputs = retained.read_inputs(repo, current["files"])
    retained.complete_compiler_members(repo, inputs)
    identities = [retained.identity(name, body) for name, body in inputs.items()]
    need(authority["current_input_identities"] == identities,
         "wrong complete current source identities")
    restored, touched = api.apply_inverse_patch(
        inputs, patch, seal["patch"]["sha256"], seal["patch"]["bytes"],
        TRANSITION_PATHS)
    need(touched == list(TRANSITION_PATHS) and sorted(restored) == list(before),
         "wrong inverse stage or selected membership")
    restored = dict(sorted(restored.items()))
    for name, body in restored.items():
        verify(body, before[name], name)
    old_identities = [retained.identity(n, b) for n, b in restored.items()]
    need(authority["predecessor_input_identities"] == old_identities,
         "wrong complete restored source identities")
    need(retained.identity("src/main.rs", restored["src/main.rs"])["git_blob"] == BEFORE_BLOB
         and retained.identity("src/main.rs", inputs["src/main.rs"])["git_blob"] == AFTER_BLOB,
         "unreviewed complete formatter source body")
    need(all(inputs[name] == restored[name] for name in after
             if name not in TRANSITION_PATHS),
         "unexplained complete source change")

    fixtures = [row for row in identities
                if row["path"].startswith(("tests/fixtures/", "fixtures/"))]
    old_fixtures = [row for row in old_identities
                    if row["path"].startswith(("tests/fixtures/", "fixtures/"))]
    includes = api.include_directives(inputs, api)
    old_includes = api.include_directives(restored, api)
    need(len(fixtures) == 78 and fixtures == old_fixtures
         and authority["compile_time_fixture_inputs"] == fixtures,
         "changed complete compile-time fixture closure")
    need(len(includes) == 136 and includes == old_includes
         and authority["compile_time_include_directives"] == includes,
         "changed, missing, reordered or duplicate include expression")
    dependencies = [retained.identity(n, inputs[n])
                    for n in retained.ACCOUNTING_PATHS]
    need(all(inputs[n] == restored[n] for n in retained.ACCOUNTING_PATHS)
         and authority["unit2_accounting_dependencies"] == dependencies,
         "changed immutable Unit2 accounting dependency")
    compiler_paths = [n for n in after if n.startswith(("src/", "native/"))]
    need(len(compiler_paths) == 288
         and (authority["current_source_members"],
              authority["predecessor_source_members"],
              authority["compiler_source_members"], authority["compiler_bodies"])
         == (376, 376, 288, 291), "wrong complete source cardinalities")

    need(set(proof) == PROOF_FIELDS
         and proof["schema"] == "oxid-published-main-selected-source-proof-v1"
         and proof["published_main"] == PUBLISHED_MAIN
         and proof["source_checkpoint"] == SOURCE_CHECKPOINT
         and proof["retained_historical_source"] == HISTORICAL_SOURCE
         and proof["retained_source_manifest"] == predecessor_descriptor
         and proof["predecessor_input_identities"] == old_identities
         and proof["current_input_identities"] == identities
         and proof["complete_compiler_paths"] == compiler_paths
         and proof["complete_pr_changed_files"]
         == ["spec/legacy-0.9.md", "src/main.rs", "tests/legacy_formatter.rs"]
         and proof["source_only"] is True
         and proof["execution_qualified"] is False,
         "changed sealed complete published-source proof")
    evidence = proof["git_object_evidence"]
    need(isinstance(evidence, dict) and set(evidence) == set(EVIDENCE_PATHS),
         "missing exact Git object evidence pins")
    for key, name in EVIDENCE_PATHS.items():
        row = evidence[key]
        need(type(row) is dict and set(row) == {"path", "bytes", "sha256"}
             and row["path"] == name and type(row["bytes"]) is int
             and 0 < row["bytes"] <= 1024 * 1024
             and type(row["sha256"]) is str and len(row["sha256"]) == 64
             and all(c in "0123456789abcdef" for c in row["sha256"]),
             "invalid fixed Git evidence descriptor")
        if key in READBACK_PINS:
            need((row["bytes"], row["sha256"]) == READBACK_PINS[key],
                 "changed accepted published Git readback")
    need(evidence["verifier"]["bytes"] <= 65536
         and evidence["generation_receipt"]["bytes"] <= 32768,
         "oversized source generation evidence")
    # These are sealed evidence descriptors. Full tree/commit reconstruction is
    # the separately reviewed generation step, not a claim of this runtime
    # source admission. The exact-head consumer closure pins their real files.
    return {
        "current": current, "inputs": inputs, "authority": authority,
        "predecessor": predecessor, "predecessor_inputs": restored,
        "transition_touched": touched, "source_proof": proof,
        "source_only": True, "execution_qualified": False,
    }

