"""Synthetic text fixtures only: no Oxid build, LLVM verifier or ELF execution."""
import copy
import json
from pathlib import Path
import tempfile
import unittest

import observe_storage as observer
import bind_chain


def synthetic_fixture(element="i32", length=0):
    size = observer.extent(element, length)
    step = ((size + 3) // 4) * 4 + 4
    offsets = {f"%o{i}": (i // 2) * step + (0 if i % 2 == 0 else step-4)
               for i in range(14)}
    observations = []
    lines = ["; SYNTHETIC TEXT TEST ONLY, NOT AN OXID-EMITTED MODULE\n",
             'target triple = "x86_64-unknown-linux-gnu"\n',
             "declare void @__oxid_overflow(ptr, i64) noreturn\n",
             "define internal i32 @__oxid_owned_fn_0() noinline {\nentry:\n",
             f"  %owners = alloca [{7*step} x i8], align 4\n"]
    lines += [f"  {owner} = getelementptr i8, ptr %owners, i64 {offset}\n"
              for owner, offset in offsets.items()]

    def add(oid, kind, dest, source=None, function="__oxid_owned_fn_0"):
        if source is None:
            if length == 0:
                typ = "i32" if element == "i32" else "i8"
                op = f"  store {typ} 0, ptr {dest}, align 1\n"
            else:
                typ = {"i32":"i32", "bool":"i1", "unit":"i8"}[element]
                stride = 4 if element == "i32" else 1
                op = "".join(f"  %{oid}_{i} = getelementptr i8, ptr {dest}, i64 {i*stride}\n"
                             f"  store {typ} 0, ptr %{oid}_{i}, align 1\n" for i in range(length))
        else:
            typ = "i32" if element == "i32" else "i8"
            op = (f"  %{oid}_v = load {typ}, ptr {source}, align 1\n"
                  f"  store {typ} %{oid}_v, ptr {dest}, align 1\n")
        local = offsets if function.endswith("0") else {"%o0": 0}
        dest_offset = offsets["%o10"] if dest == "%result" else local[dest]
        row = dict(id=oid, kind=kind, function=function, destination=dest, extent=size,
                   owner_extent=[dest_offset, dest_offset+size], operation=op,
                   operation_sha256=observer.sha(op), preserve_ranges=[])
        if length > 0 and element in {"i32","bool"}:
            stage=0 if oid in {"construct_local","move_owner"} else 1 if oid in {"construct_temp_1","replace_moved"} else 2
            row["expected_scalar"] = [-91,-54,-17][stage] if element=="i32" else [True,False,True][stage]
        if source in local and dest != "%result":
            row["preserve_ranges"] = [[local[source], local[source]+size]]
        observations.append(row)
        return op

    lines.append(add("construct_local", "construct_local", "%o0"))
    lines.append(add("move_owner", "move_initialize", "%o4", "%o0"))
    lines.append(add("construct_temp_1", "construct_temporary", "%o2"))
    lines.append(add("replace_moved", "replace_moved", "%o0", "%o2"))
    lines.append(add("construct_temp_2", "construct_temporary", "%o6"))
    lines.append(add("replace_available", "replace_available", "%o0", "%o6"))
    lines.append(add("self_move", "self_move_temporary", "%o12", "%o0"))
    lines.append(add("self_replace", "self_replace", "%o0", "%o12"))
    lines.append(add("stage", "prepare_owned", "%o8", "%o0"))
    call = "  call void @__oxid_owned_fn_1(ptr %o10, ptr %o8)\n"
    lines += [call, "  ret i32 0\n}\n"]
    callee_size = ((size+3)//4)*4
    lines += ["define internal void @__oxid_owned_fn_1(ptr %result, ptr %arg0) noinline {\nentry:\n",
              f"  %owners = alloca [{callee_size} x i8], align 4\n",
              "  %o0 = getelementptr i8, ptr %owners, i64 0\n"]
    lines.append(add("incoming", "incoming_owned", "%o0", "%arg0", "__oxid_owned_fn_1"))
    lines.append(add("returned", "return_owned", "%result", "%o0", "__oxid_owned_fn_1"))
    observations[-1]["external_storage"] = dict(function="__oxid_owned_fn_0", owner="%o10", call=call)
    lines.append("  ret void\n}\n")
    source = "".join(lines)
    manifest = dict(schema=observer.VERSION, source_sha256=observer.sha(source),
                    fixture="SYNTHETIC_TEXT_TEST_self_replacement_chain", element=element, length=length,
                    arenas=[dict(function="__oxid_owned_fn_0", alloca=f"  %owners = alloca [{7*step} x i8], align 4\n", owner_offsets=offsets),
                            dict(function="__oxid_owned_fn_1", alloca=f"  %owners = alloca [{callee_size} x i8], align 4\n", owner_offsets={"%o0":0})],
                    observations=observations)
    return source, manifest


class TextObservationTests(unittest.TestCase):
    @staticmethod
    def spelled_self_chain(element="i32", length=0):
        source,_=synthetic_fixture(element,length)
        mapping={"construct_local":"f0_b0_i17", "move_owner":"f0_b0_i19",
                 "construct_temp_1":"f0_b0_i21", "replace_moved":"f0_b0_i22",
                 "construct_temp_2":"f0_b0_i24", "replace_available":"f0_b0_i25",
                 "self_move":"f0_b0_i29", "self_replace":"f0_b0_i30",
                 "stage":"f0_b0_i32", "incoming":"f1_param0", "returned":"f1_b0_term"}
        for old,new in mapping.items():
            source=source.replace("%"+old+"_","%"+new+"_")
        return source

    def test_independent_self_chain_ordinal_binding(self):
        for element,length in [("i32",0),("bool",0),("unit",0),("unit",4)]:
            source=self.spelled_self_chain(element,length)
            manifest=bind_chain.bind(source,observer.sha(source),element,length,True,"synthetic_bound")
            self.assertEqual(len(manifest["observations"]),11)
            observer.instrument(source,manifest)

    def test_guarded_caller_result_binding(self):
        source=self.spelled_self_chain()
        source=source.replace("@__oxid_owned_fn_1(ptr %o10, ptr %o8)",
                              "@__oxid_owned_fn_1(ptr %fuel, ptr %o10, ptr %o8)")
        manifest=bind_chain.bind(source,observer.sha(source),"i32",0,True,"synthetic_guarded")
        observer.instrument(source,manifest)

    def test_binder_rejects_changed_frozen_ordinal(self):
        source=self.spelled_self_chain().replace("%f0_b0_i19_","%f0_b0_i20_")
        with self.assertRaisesRegex(ValueError,"missing emitted operation"):
            bind_chain.bind(source,observer.sha(source),"i32",0,True,"synthetic_bad_ordinal")

    def test_binder_selected_mutant_is_narrow(self):
        source=self.spelled_self_chain()
        manifest=bind_chain.bind(source,observer.sha(source),"i32",0,True,"synthetic_bound")
        for oid in ["construct_local","move_initialize","prepare_owned","incoming_owned","return_owned","self_replace"]:
            spec=bind_chain.choose_mutant(source,manifest,oid)
            observer.narrow_mutant(source,manifest,spec)

    def test_full_paths_reconstruct(self):
        source, manifest = synthetic_fixture()
        output, receipt = observer.instrument(source, manifest)
        self.assertEqual(observer.restore(output, receipt["patches"]), source)
        self.assertEqual(len(receipt["observations"]), 11)
        self.assertEqual({o["kind"] for o in receipt["observations"]}, observer.KINDS)
        for obs in receipt["observations"]:
            self.assertEqual(obs["before_sha256"], obs["after_sha256"])
            self.assertEqual(obs["payload_check_count"], 4)
            self.assertEqual(obs["initialized_observed_offsets"], [-1,4,0,1,2,3])
        self.assertIn("%owners = getelementptr i8, ptr %__oxid_physical_arena, i64 4", output)
        self.assertTrue(receipt["operation_bodies_byte_equal"])

    def test_initialized_full_extents_and_no_nonempty_bool_byte_claim(self):
        for element, length, count in [("i32",0,4),("bool",0,1),("unit",0,1),
                                        ("unit",4,4),("unit",1024,1024),("bool",4,4),("i32",1,4)]:
            with self.subTest(element=element,length=length):
                source, manifest = synthetic_fixture(element,length)
                output, receipt = observer.instrument(source,manifest)
                self.assertTrue(all(o["payload_check_count"] == count for o in receipt["observations"]))
                if element=="bool" and length>0:
                    self.assertIn(f"i64 {length}, i8 164)",output)
                    self.assertTrue(all(o["payload_byte_checks"]==0 for o in receipt["observations"]))
                else:
                    self.assertIn(f"i64 {observer.extent(element,length)})", output)

    def test_i32_exact_repeated_cell_bytes(self):
        source,manifest=synthetic_fixture("i32",4)
        _,receipt=observer.instrument(source,manifest)
        expected={"construct_local":-91,"move_owner":-91,"construct_temp_1":-54,
                  "replace_moved":-54,"construct_temp_2":-17,"replace_available":-17,
                  "self_move":-17,"self_replace":-17,"stage":-17,"incoming":-17,"returned":-17}
        for site in receipt["observations"]:
            self.assertEqual(site["expected_scalar"],expected[site["id"]])
            self.assertEqual(site["expected_i32_cell_bytes"],list(expected[site["id"]].to_bytes(4,"little",signed=True)))
            self.assertEqual(site["i32_payload_byte_checks"],16)

    def test_bool_opposing_poison_and_only_i1_loads(self):
        source,manifest=synthetic_fixture("bool",4)
        output,receipt=observer.instrument(source,manifest)
        for site in receipt["observations"]:
            self.assertNotEqual(site["poison"]&1,int(site["expected_scalar"]))
            self.assertEqual(site["payload_i1_checks"],4)
            self.assertEqual(site["payload_byte_checks"],0)
            self.assertFalse(site["bool_padding_bits_observed"])
            self.assertEqual(output.count(f"@__oxid_physical_message_{receipt['observations'].index(site)}_bool_cell_"),8)
        self.assertIn("load volatile i1, ptr %cell",observer.BOOL_HELPERS)
        self.assertNotIn("load volatile i8",observer.BOOL_HELPERS)

    def test_i32_expectation_wrong_type_rejected(self):
        source,manifest=synthetic_fixture("i32",4)
        manifest["observations"][0]["expected_scalar"]=True
        with self.assertRaisesRegex(ValueError,"signed-i32"):
            observer.instrument(source,manifest)

    def test_bool_expectation_wrong_type_rejected(self):
        source,manifest=synthetic_fixture("bool",4)
        manifest["observations"][0]["expected_scalar"]=1
        with self.assertRaisesRegex(ValueError,"exact boolean"):
            observer.instrument(source,manifest)

    def test_large_arrays_use_three_check_calls_per_operation(self):
        for element in ["i32","bool","unit"]:
            source,manifest=synthetic_fixture(element,1024)
            output,receipt=observer.instrument(source,manifest)
            for site in receipt["observations"]:
                self.assertEqual(site["check_implementation"],"compact_loop")
                self.assertEqual(site["static_check_call_count"],3)
                self.assertEqual(site["runtime_payload_interval"],[0,observer.extent(element,1024)])
            self.assertEqual(output.count("  call void @__oxid_physical_expect"),33)
            # Two guard globals and one operation-specific payload global per operation.
            self.assertEqual(output.count(" = private constant ["),33)
        self.assertIn("%ordinal = urem i64 %i, 4",observer.BYTE_PATTERN_HELPER)
        self.assertIn("load volatile i1",observer.BOOL_LOOP_HELPER)
        self.assertNotIn("load volatile i8",observer.BOOL_LOOP_HELPER)

    def test_hash_binding_rejects_other_source(self):
        source, manifest = synthetic_fixture()
        with self.assertRaisesRegex(ValueError, "source hash"):
            observer.instrument(source+"; edit\n", manifest)

    def test_owner_offset_binding_rejects_drift(self):
        source, manifest = synthetic_fixture()
        manifest["arenas"][0]["owner_offsets"]["%o0"] = 4
        with self.assertRaisesRegex(ValueError, "owner offsets"):
            observer.instrument(source, manifest)

    def test_source_overlap_rejected(self):
        source, manifest = synthetic_fixture()
        manifest["observations"][0]["preserve_ranges"] = [[4,8]]
        with self.assertRaisesRegex(ValueError, "overlaps"):
            observer.instrument(source, manifest)

    def test_payload_extent_rejected(self):
        source, manifest = synthetic_fixture()
        manifest["observations"][0]["extent"] = 1
        with self.assertRaisesRegex(ValueError, "extent"):
            observer.instrument(source, manifest)

    def test_terminator_anchor_rejected(self):
        source, manifest = synthetic_fixture()
        item = manifest["observations"][-1]
        item["operation"] += "  ret void\n"
        item["operation_sha256"] = observer.sha(item["operation"])
        with self.assertRaisesRegex(ValueError, "terminator"):
            observer.instrument(source, manifest)

    def test_wrong_result_binding_rejected(self):
        source, manifest = synthetic_fixture()
        manifest["observations"][-1]["external_storage"]["owner"] = "%o8"
        with self.assertRaisesRegex(ValueError, "result is not"):
            observer.instrument(source, manifest)

    def test_missing_return_allocation_proof_rejected(self):
        source, manifest = synthetic_fixture()
        del manifest["observations"][-1]["external_storage"]
        with self.assertRaisesRegex(ValueError, "caller arena"):
            observer.instrument(source, manifest)

    def test_observation_tamper_rejected(self):
        source, manifest = synthetic_fixture()
        output, receipt = observer.instrument(source, manifest)
        with self.assertRaisesRegex(ValueError, "output hash"):
            observer.verify_receipt(source, output.replace("i8 165", "i8 0"), receipt)

    def test_body_tamper_with_updated_output_hash_rejected(self):
        source, manifest = synthetic_fixture()
        output, receipt = observer.instrument(source, manifest)
        tampered = output.replace("  store i32 0, ptr %o0, align 1", "  store i32 1, ptr %o0, align 1")
        receipt["output_sha256"] = observer.sha(tampered)
        with self.assertRaisesRegex(ValueError, "differs from receipt"):
            observer.verify_receipt(source, tampered, receipt)

    def test_observer_prefix_collision_rejected(self):
        source, manifest = synthetic_fixture()
        source += "; __oxid_physical_collision\n"
        manifest["source_sha256"] = observer.sha(source)
        with self.assertRaisesRegex(ValueError, "prefix"):
            observer.instrument(source, manifest)

    def test_duplicate_id_rejected(self):
        source, manifest = synthetic_fixture()
        manifest["observations"][1]["id"] = manifest["observations"][0]["id"]
        with self.assertRaisesRegex(ValueError, "duplicate"):
            observer.instrument(source, manifest)

    def test_one_byte_constructor_mutant(self):
        source, manifest = synthetic_fixture()
        op = manifest["observations"][0]["operation"]
        spec = dict(id="constructor_one_byte", source_sha256=observer.sha(source),
                    observation_id="construct_local", changes=[dict(**{"from":op,"to":op.replace("i32","i8")})])
        mutant, mm, receipt = observer.narrow_mutant(source,manifest,spec)
        self.assertEqual(receipt["expected_payload"], [0,165,165,165])
        self.assertEqual(receipt["execution_status"], "not-run")
        output, ir = observer.instrument(mutant,mm)
        observer.verify_receipt(mutant,output,ir)
        self.assertEqual(sum(o["payload_check_count"] for o in ir["observations"]),44)

    def test_type_consistent_transfer_mutants(self):
        for oid in ["move_owner","incoming","returned","stage"]:
            source, manifest = synthetic_fixture()
            item = next(o for o in manifest["observations"] if o["id"]==oid)
            changes = [{"from":line,"to":line.replace("i32","i8")}
                       for line in item["operation"].splitlines(keepends=True)]
            spec = dict(id=oid+"_mutant",source_sha256=observer.sha(source),observation_id=oid,changes=changes)
            mutant, mm, _ = observer.narrow_mutant(source,manifest,spec)
            output, receipt = observer.instrument(mutant,mm)
            observer.verify_receipt(mutant,output,receipt)

    def test_non_narrowing_mutation_rejected(self):
        source, manifest = synthetic_fixture()
        op = manifest["observations"][0]["operation"]
        spec = dict(id="bad",source_sha256=observer.sha(source),observation_id="construct_local",
                    changes=[{"from":op,"to":op.replace("i32 0","i8 1")}])
        with self.assertRaisesRegex(ValueError,"only narrow"):
            observer.narrow_mutant(source,manifest,spec)

    def test_half_typed_copy_mutation_rejected(self):
        source, manifest = synthetic_fixture()
        op = manifest["observations"][1]["operation"].splitlines(keepends=True)[0]
        spec = dict(id="bad",source_sha256=observer.sha(source),observation_id="move_owner",
                    changes=[{"from":op,"to":op.replace("i32","i8")}])
        with self.assertRaisesRegex(ValueError,"type-consistent"):
            observer.narrow_mutant(source,manifest,spec)

    def test_duplicate_operation_anchor_rejected(self):
        source, manifest = synthetic_fixture()
        op = manifest["observations"][0]["operation"]
        source = source.replace(op,op+op)
        manifest["source_sha256"] = observer.sha(source)
        with self.assertRaisesRegex(ValueError,"one exact occurrence"):
            observer.instrument(source,manifest)

    def test_outer_guard_storage_is_allocated(self):
        source, manifest = synthetic_fixture()
        _, receipt = observer.instrument(source,manifest)
        arenas = {p["id"]:p for p in receipt["patches"] if p["kind"]=="arena_rebase"}
        for obs in receipt["observations"]:
            arena = arenas[obs["storage_function"]]
            rebased = obs["owner_offset"] + arena["rebase"]
            self.assertGreaterEqual(rebased-1,0)
            self.assertLess(rebased+obs["extent"],arena["physical_extent"])

    def test_helpers_are_only_byte_storage_and_existing_fatal_abi(self):
        calls = [line.strip() for line in observer.HELPERS.splitlines() if "call " in line]
        self.assertEqual(calls,["call void @__oxid_overflow(ptr %message, i64 %message_len)"])
        self.assertIn("store volatile i8 165", observer.HELPERS)
        self.assertIn("load volatile i8", observer.HELPERS)
        self.assertNotIn("alloca",observer.HELPERS)

    def test_harness_tsv_and_per_case_receipts(self):
        source, manifest = synthetic_fixture()
        manifest["case_id"] = "synthetic_empty_i32"
        op = manifest["observations"][0]["operation"]
        spec = dict(id="construct_one_byte",source_sha256=observer.sha(source),
                    observation_id="construct_local",changes=[{"from":op,"to":op.replace("i32","i8")}])
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)/"bundle"
            receipt = observer.write_bundle(source,manifest,root,[spec])
            rows = (root/"harness.tsv").read_text().splitlines()
            self.assertEqual(rows[0],"case\tmodule\tstatus\tstdout_hex\tstderr_hex")
            self.assertEqual(rows[1],"synthetic_empty_i32_positive\tobserved.ll\t0\t300a\t")
            self.assertEqual(rows[2].split("\t")[2:4],["1",""])
            self.assertEqual(bytes.fromhex(rows[2].split("\t")[4]).decode(),
                             "physical storage check failed: construct_local payload_1 expected 0\n")
            self.assertEqual((root/"production.ll").read_text(),source)
            self.assertEqual(len(receipt["cases"]),2)
            for row in receipt["cases"]:
                self.assertEqual(row["original_production_sha256"],observer.sha(source))
                self.assertEqual(row["module_sha256"],observer.sha((root/row["module"]).read_text()))
                self.assertEqual(row["execution_status"],"not-run")

    def test_bundle_will_not_overwrite_evidence(self):
        source, manifest = synthetic_fixture()
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)/"bundle"
            observer.write_bundle(source,manifest,root)
            with self.assertRaises(FileExistsError):
                observer.write_bundle(source,manifest,root)


if __name__ == "__main__":
    unittest.main(verbosity=2)
