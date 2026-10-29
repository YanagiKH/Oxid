#!/usr/bin/env python3
"""Bind the finite owned-vector source delta to the unchanged retained replay.

The frozen registry controller owns every inherited semantic/resource predicate
and debug/release build. Two explicit observer exclusions keep owned-record
vectors outside the old wire. New semantic/platform claims remain false.
Provisional source or observer pins permit preparation only, never qualification.
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
PREDECESSOR = "af0fe5e1fbaf4fb49b62e894a53ad4b5ed307ee7"
PREDECESSOR_POLICY = "scripts/verify_registry_compatibility.py"
PREDECESSOR_POLICY_PIN = (35050, "d6bd23496f38bde3d89e7e97a2d724b73a0f233cfef99a6dc9fad752bdec9766")
REVIEWED_SOURCE_HEAD = 'bd8978a411e4dba19430037640a24b3f08fe5a33'
PINS_PROVISIONAL = False
OWNED_VECTOR_OBSERVER_ADAPTER = "scripts/owned_record_vector_compatibility_observer.py"
OWNED_VECTOR_OBSERVER_ADAPTER_PIN = (
    7268, "ac49c38f23b0d3a5929fe3d52edae1cb85341c72b16249522e30dc155e1fa8f3")
OWNED_VECTOR_OBSERVER_PROVISIONAL = False

# This one source asset is compiled by the two new enum regressions. It is
# separate from the 107 compiler-file directive rows and the 44 new modules.
OWNED_VECTOR_SOURCE_ASSETS = {
    "scripts/owned_record_vector_acceptance/enum16.ox": (
        1746, "34a810415ec8697d9b74ff58f39e3c0787cabb33970bf3eed7e64a9e5472a715"),
}
SOURCE_ASSET_INCLUDE_BINDINGS = {
    "scripts/owned_record_vector_acceptance/enum16.ox": (
        "src/frontend/oir/owned/source/owned_vector_source_resource_tests.rs",
        "../../../../../scripts/owned_record_vector_acceptance/enum16.ox",
        b'include_str!("../../../../../scripts/owned_record_vector_acceptance/enum16.ox")',
        2,
    ),
}

OWNED_VECTOR_CHANGED = frozenset((
    "native/experimental_bytebuf.c",
    "native/experimental_vector.c",
    "native/typed_bytebuf.c",
    "native/typed_vector.c",
    "src/frontend/builtin_catalog.rs",
    "src/frontend/declaration_index.rs",
    "src/frontend/declaration_index/generics.rs",
    "src/frontend/declaration_index/generics/nominal.rs",
    "src/frontend/declaration_index/sealed.rs",
    "src/frontend/declaration_index/tests.rs",
    "src/frontend/experimental_builtins.rs",
    "src/frontend/native.rs",
    "src/frontend/oir/owned/array_observe.rs",
    "src/frontend/oir/owned/builtins.rs",
    "src/frontend/oir/owned/byte_buffer.rs",
    "src/frontend/oir/owned/cfg.rs",
    "src/frontend/oir/owned/debug.rs",
    "src/frontend/oir/owned/execute.rs",
    "src/frontend/oir/owned/execute_composition.rs",
    "src/frontend/oir/owned/execute_experimental.rs",
    "src/frontend/oir/owned/execute_file_delivery.rs",
    "src/frontend/oir/owned/execute_io_delivery.rs",
    "src/frontend/oir/owned/execute_resources.rs",
    "src/frontend/oir/owned/execute_vector.rs",
    "src/frontend/oir/owned/flow.rs",
    "src/frontend/oir/owned/mod.rs",
    "src/frontend/oir/owned/native.rs",
    "src/frontend/oir/owned/native_composition.rs",
    "src/frontend/oir/owned/native_experimental.rs",
    "src/frontend/oir/owned/native_reachability.rs",
    "src/frontend/oir/owned/native_resources.rs",
    "src/frontend/oir/owned/native_storage.rs",
    "src/frontend/oir/owned/native_tests.rs",
    "src/frontend/oir/owned/native_vector.rs",
    "src/frontend/oir/owned/native_vector_composition_tests.rs",
    "src/frontend/oir/owned/native_vector_layout.rs",
    "src/frontend/oir/owned/plan.rs",
    "src/frontend/oir/owned/reviewer_array_reference_tests.rs",
    "src/frontend/oir/owned/shape.rs",
    "src/frontend/oir/owned/source/array_pipeline_rows.rs",
    "src/frontend/oir/owned/source/association.rs",
    "src/frontend/oir/owned/source/budget.rs",
    "src/frontend/oir/owned/source/candidate_adapter.rs",
    "src/frontend/oir/owned/source/candidate_native.rs",
    "src/frontend/oir/owned/source/hir_budget.rs",
    "src/frontend/oir/owned/source/lower.rs",
    "src/frontend/oir/owned/source/mod.rs",
    "src/frontend/oir/owned/source/program.rs",
    "src/frontend/oir/owned/source/resolve.rs",
    "src/frontend/oir/owned/source/typeck.rs",
    "src/frontend/oir/owned/source/vector_lower.rs",
    "src/frontend/oir/owned/vector_descriptors.rs",
    "src/frontend/oir/owned/vector_type.rs",
    "src/frontend/oir/owned/verified.rs",
    "src/frontend/oir/owned_types.rs",
    "src/frontend/oir/owned_types/composition.rs",
    "src/frontend/oir/owned_types/enums.rs",
    "src/frontend/oir/owned_types/resources.rs",
    "src/frontend/oir/source.rs",
    "src/frontend/oir/source/sealed.rs",
    "src/frontend/parser.rs",
    "src/frontend/vector_builtins.rs",
    'src/frontend/declaration_index/resource.rs',
))
OWNED_VECTOR_ADDED = frozenset((
    "native/experimental_owned_record.c",
    "native/experimental_owned_record.h",
    "native/experimental_owned_vector.c",
    "native/experimental_owned_vector.h",
    "src/frontend/declaration_index/generics/owned_candidate.rs",
    "src/frontend/oir/owned/execute_owned_vector.rs",
    "src/frontend/oir/owned/execute_owned_vector_entry_tests.rs",
    "src/frontend/oir/owned/execute_owned_vector_tests.rs",
    "src/frontend/oir/owned/native_owned_vector.rs",
    "src/frontend/oir/owned/native_owned_vector_commit_tests.rs",
    "src/frontend/oir/owned/native_owned_vector_schema_tests.rs",
    "src/frontend/oir/owned/native_owned_vector_schemas.rs",
    "src/frontend/oir/owned/native_owned_vector_tests.rs",
    "src/frontend/oir/owned/owned_record_bounds.rs",
    "src/frontend/oir/owned/owned_record_codec.rs",
    "src/frontend/oir/owned/owned_vector_context.rs",
    "src/frontend/oir/owned/owned_vector_context_tests.rs",
    "src/frontend/oir/owned/owned_vector_declaration_tests.rs",
    "src/frontend/oir/owned/owned_vector_descriptor_tests.rs",
    "src/frontend/oir/owned/owned_vector_descriptors.rs",
    "src/frontend/oir/owned/owned_vector_execution_fixture.rs",
    "src/frontend/oir/owned/owned_vector_file_fixture.rs",
    "src/frontend/oir/owned/owned_vector_plan.rs",
    "src/frontend/oir/owned/owned_vector_proof.rs",
    "src/frontend/oir/owned/owned_vector_proof_tests.rs",
    "src/frontend/oir/owned/owned_vector_queries.rs",
    "src/frontend/oir/owned/owned_vector_query_tests.rs",
    "src/frontend/oir/owned/owned_vector_resource_fixture.rs",
    "src/frontend/oir/owned/owned_vector_resource_tests.rs",
    "src/frontend/oir/owned/owned_vector_resources.rs",
    "src/frontend/oir/owned/owned_vector_signature_tests.rs",
    "src/frontend/oir/owned/owned_vector_type_tests.rs",
    "src/frontend/oir/owned/owned_vector_verified.rs",
    "src/frontend/oir/owned/owned_vector_witness_tests.rs",
    "src/frontend/oir/owned/shape_declaration_queries.rs",
    "src/frontend/oir/owned/source/owned_vector_source.rs",
    "src/frontend/oir/owned/source/owned_vector_source_execution_tests.rs",
    "src/frontend/oir/owned/source/owned_vector_source_facade_tests.rs",
    "src/frontend/oir/owned/source/owned_vector_source_resource_tests.rs",
    "src/frontend/oir/owned/source/owned_vector_source_tests.rs",
    "src/frontend/oir/owned/verified_program_queries.rs",
    "src/frontend/oir/owned_types/owned_vector.rs",
    "src/frontend/oir/source/owned_vector_route_tests.rs",
    "src/frontend/owned_vector_builtins.rs",
))
TEST_NAMED_INPUTS = frozenset((
    "src/frontend/declaration_index/tests.rs",
    "src/frontend/oir/owned/execute_owned_vector_entry_tests.rs",
    "src/frontend/oir/owned/execute_owned_vector_tests.rs",
    "src/frontend/oir/owned/native_owned_vector_commit_tests.rs",
    "src/frontend/oir/owned/native_owned_vector_schema_tests.rs",
    "src/frontend/oir/owned/native_owned_vector_tests.rs",
    "src/frontend/oir/owned/native_tests.rs",
    "src/frontend/oir/owned/native_vector_composition_tests.rs",
    "src/frontend/oir/owned/owned_vector_context_tests.rs",
    "src/frontend/oir/owned/owned_vector_declaration_tests.rs",
    "src/frontend/oir/owned/owned_vector_descriptor_tests.rs",
    "src/frontend/oir/owned/owned_vector_proof_tests.rs",
    "src/frontend/oir/owned/owned_vector_query_tests.rs",
    "src/frontend/oir/owned/owned_vector_resource_tests.rs",
    "src/frontend/oir/owned/owned_vector_signature_tests.rs",
    "src/frontend/oir/owned/owned_vector_type_tests.rs",
    "src/frontend/oir/owned/owned_vector_witness_tests.rs",
    "src/frontend/oir/owned/reviewer_array_reference_tests.rs",
    "src/frontend/oir/owned/source/owned_vector_source_execution_tests.rs",
    "src/frontend/oir/owned/source/owned_vector_source_facade_tests.rs",
    "src/frontend/oir/owned/source/owned_vector_source_resource_tests.rs",
    "src/frontend/oir/owned/source/owned_vector_source_tests.rs",
    "src/frontend/oir/source/owned_vector_route_tests.rs",
))

REVIEWED_INPUTS = {
    "native/experimental_bytebuf.c": (10868, "bf03337c137a3f734b051c29a8ebc6cbd8267fc1aaa0b5fe61db7062fbac6f58"),
    "native/experimental_owned_record.c": (11357, "1cb8b2a26a3f9890ded96d6cbcfc87efe5e8c72bb93c6f45f2d9d5b985c95f87"),
    "native/experimental_owned_record.h": (1297, "bf762117bffe0300368dfa317cefe31ce7ef4cb3774bf0b41d1502a20405f595"),
    "native/experimental_owned_vector.c": (20388, "4e780724f96d307f91431b55ac6651e6ce6fb00cbba0df3b03e0d30f91945521"),
    "native/experimental_owned_vector.h": (2566, "bc602990eb54a5c095a5a18511060955b266ae8c89cf43edbd5745304a7e2ed7"),
    "native/experimental_vector.c": (19138, "0c6cecb78d61e7ea15911df5d6af8664660f8f31bc47ddd68849533580279d98"),
    "native/typed_bytebuf.c": (7171, "93bd553c27c95ebf44f03e98fa10a9981df1376130c544776139425426626e84"),
    "native/typed_vector.c": (13271, "dba4dbe9f87b054324e31ac4a0055ec04ba3c88c20d13a2a7ee4f31b0fa793da"),
    "src/frontend/builtin_catalog.rs": (16047, "04715518121dfdba149fe57ae95d8ac284523c66169d07248f2fd29b99c7e3dd"),
    "src/frontend/declaration_index.rs": (88927, 'd8fb66a407a598a84ba67d771caf5fe80f6d8f6038f798dbbbe96afc322d7bca'),
    "src/frontend/declaration_index/generics.rs": (136375, "c6f188c5853cc47d2e3e600805c831b7cebec8debde131dac23227804d123a81"),
    "src/frontend/declaration_index/generics/nominal.rs": (33872, "4dfea0a548a46534f555721960174c21bc33994f040db466f3966f601123e216"),
    "src/frontend/declaration_index/generics/owned_candidate.rs": (77372, "402c0a472e493af11a7acb28a6c8345be1ab8b610d135af043919e04cb18550e"),
    "src/frontend/declaration_index/sealed.rs": (120632, "b015eef9a6917570788ddd44cfe63301112d4030ba51c9495982eed98d90b5ea"),
    "src/frontend/declaration_index/tests.rs": (24645, '18740c7fd41777e4c4789a87dccd81f47c1b9e72a504e7b06ad00d583f279803'),
    "src/frontend/experimental_builtins.rs": (30277, "9f724e9a412db4eb58705a40d7b9b7e0432155185bc4721ab8a88dca58505cd0"),
    "src/frontend/native.rs": (30974, "e74711de9a4de85ad6b5c49b622b89093f5e6e763a41a6ee6f3de4681419c5e0"),
    "src/frontend/oir/owned/array_observe.rs": (24256, "becb20579e8669ce8b5af067aff733f58ef8a211a7ac7d1ea7ccac8f4ff303a3"),
    "src/frontend/oir/owned/builtins.rs": (15657, "ad7714aa8d222eee6d980a6b9bc260afb13632973adeb15638585f43f5e38044"),
    "src/frontend/oir/owned/byte_buffer.rs": (21734, "7954c14edaf81d6d23d1c3531b3ee78d387f50557d96b72f0e918f48d249de00"),
    "src/frontend/oir/owned/cfg.rs": (8320, "35fa35093a5c48ad3b5fe15705f29523eefa1987427bf18f6e8b24ed7dfe6a96"),
    "src/frontend/oir/owned/debug.rs": (7768, "ad319650ee3e0c1ca33144bd9376435d064451793a3887bba8a598c4563d8eb7"),
    "src/frontend/oir/owned/execute.rs": (224080, "74a6dcdd127498487828c6ce8be0cb04db404c7a00d9d39383e65f6ee977f194"),
    "src/frontend/oir/owned/execute_composition.rs": (11378, "7a500b372958a7b20358d3b33d3391067d99762909063e3034e819699264f786"),
    "src/frontend/oir/owned/execute_experimental.rs": (28087, "cde54e6e545b7343c70811b28c7a10d6d323b2b3fdcdd9cf012eef13ca2a8b64"),
    "src/frontend/oir/owned/execute_file_delivery.rs": (20232, "31fbdab87c759ba8ebb720d8177c3ef7c2fdc4166d48f10c7d77d2f7d92cbbd1"),
    "src/frontend/oir/owned/execute_io_delivery.rs": (15146, "800c4a819826a22e968282890443c8c12d316be93944d9c98c2ffd0903d9bcd6"),
    "src/frontend/oir/owned/execute_owned_vector.rs": (14020, "7658135da06950b7f81270aef5e5035dff3a0488295258dcf3b612cc29197eff"),
    "src/frontend/oir/owned/execute_owned_vector_entry_tests.rs": (9270, "6a76db822e8e03aab1ffb06b66235bab3f2544cf942c2f507438ea5eb25fc996"),
    "src/frontend/oir/owned/execute_owned_vector_tests.rs": (9535, "ad9c96c52a946629e296829f994dc001cc314a9966fcec29df8b6ff023c878e2"),
    "src/frontend/oir/owned/execute_resources.rs": (15848, "059a3b628d394e3b68490b57d7641d3bc624ad1bf5ad979e80821d0c31575403"),
    "src/frontend/oir/owned/execute_vector.rs": (71439, "88cd97e0fb54417ea93d2226479bdd2eea99081fa73285f0d58071636747c621"),
    "src/frontend/oir/owned/flow.rs": (61424, "f0b3567400b919f162fe8655e33013a2daa64836531aa841ee17009dda1399be"),
    "src/frontend/oir/owned/mod.rs": (18348, "d98108a44163a415fb78e2137dd1372c429ac8ee53183a3b5ca5fe3e475bc787"),
    # New-domain admitted Destructure/Replace commits count one leaf pass;
    # the legacy full move and all existing validation/release counts remain.
    "src/frontend/oir/owned/native.rs": (283027, "ba8d2885b273e9446ae3d4557f96af80a5188e240d36551a6cfaf93bbf34fdfb"),
    "src/frontend/oir/owned/native_composition.rs": (31602, "8955acccc6ffcdec0feaa85ca2edd7fb6017d0d5bedd19c0d2e9ec468b55181b"),
    "src/frontend/oir/owned/native_experimental.rs": (20717, "6e39876252896abd10e1b995ae1a422efd5947ece251bf204091aed9ae0fe3eb"),
    "src/frontend/oir/owned/native_owned_vector.rs": (7382, "21178ea13827ca6d23b0ed62cf372cd34fe29969bd2f45ef077428922820ec09"),
    "src/frontend/oir/owned/native_owned_vector_commit_tests.rs": (8084, "3a3204b25260fe3fef13561aed8dee01e17d6d16fd17f47142336112245e0be9"),
    "src/frontend/oir/owned/native_owned_vector_schema_tests.rs": (9877, "8b4acb8baa13cd4b83e8841699496f4b0bba90e3a2bb945f8be82bcf041a79da"),
    "src/frontend/oir/owned/native_owned_vector_schemas.rs": (11534, "8348611b082b9b1700f97bcc8a41c682212b3b1c28a3926715440d11e0965ffa"),
    "src/frontend/oir/owned/native_owned_vector_tests.rs": (19580, "53238ac86963b39df7a25ef733778d6a0788c1263660545dd5245de202da8397"),
    "src/frontend/oir/owned/native_reachability.rs": (12062, "deeaf89988ba621d6f1e7f42f3555b43d8d2dad671f932b3a6ccdda3585fc950"),
    "src/frontend/oir/owned/native_resources.rs": (9664, "666590fde0f9273a4d407961b4943a6dc6d1a30e63c86248743c7cee443f73ba"),
    "src/frontend/oir/owned/native_storage.rs": (20292, "c42fb803235ad98a93c245d6c4c2e9d8889b071850fb1f4c8e4245a153d691c3"),
    "src/frontend/oir/owned/native_tests.rs": (91447, "5c184e5fa1578a4e7073af16b743381e93959379cf43d1978db71655b0e5bdbc"),
    "src/frontend/oir/owned/native_vector.rs": (22726, "6e6f90d960a52e1fe688867fb3d9be7a6fd8b71dc4117194d4e88f353e289634"),
    "src/frontend/oir/owned/native_vector_composition_tests.rs": (3209, "bee4e891b8402d8362f7c174082a502809137a45ce3938d0e92f67b34cc98304"),
    "src/frontend/oir/owned/native_vector_layout.rs": (6420, "a8456b87498878906fdf8b64fc8cd79b397edf105d9e886a09729e3745d49e19"),
    "src/frontend/oir/owned/owned_record_bounds.rs": (14185, "0473aa41721b4ff99af4575557c284b1c9a0362690804782eb352e3a1597da98"),
    "src/frontend/oir/owned/owned_record_codec.rs": (14993, "b492a2c8739e951caf84d1de83bdb659affdeeea4b3b7ec5a96675da2b20fe7f"),
    "src/frontend/oir/owned/owned_vector_context.rs": (20682, "59f3a7a1d3ca46cad6d04d2dc2857b1e0fab190c2240d0b8ff5062450ca120f1"),
    "src/frontend/oir/owned/owned_vector_context_tests.rs": (15866, "4152dc2a7ea692991758af78c24ebd21d4eff7a351739a136f780a91e5b551fb"),
    "src/frontend/oir/owned/owned_vector_declaration_tests.rs": (17952, "2f2261e76dd647d1a827d0fb20cba7ac541e44f6a998554c17b9c961c0bc1044"),
    "src/frontend/oir/owned/owned_vector_descriptor_tests.rs": (24215, "15aa254e2e0afe648b865706147904da0c208004335411f29b294966c8d57ce8"),
    "src/frontend/oir/owned/owned_vector_descriptors.rs": (19782, "ecf107b0d0c16a4048c3eda8216a5c0720b24ad40a44d394e633254713490620"),
    "src/frontend/oir/owned/owned_vector_execution_fixture.rs": (13490, "7e5e39ce107aea27fd0502a3d28011b7a26cecd7b93979aac801de7f670f2c0e"),
    "src/frontend/oir/owned/owned_vector_file_fixture.rs": (15089, "9c88d9d4a04f058ca985eb9751cf57a5e96fea2784518f4829902a7422a696d6"),
    "src/frontend/oir/owned/owned_vector_plan.rs": (5534, "17004814a903fba3275ee96f5b39bff29cc3ebce45839382cb6b234e8d55e319"),
    "src/frontend/oir/owned/owned_vector_proof.rs": (3645, "ab5deb626bdc633e3f7ae0ec210c96627a8ff4bf8a75a4478451386ef1f32e07"),
    "src/frontend/oir/owned/owned_vector_proof_tests.rs": (12546, "8c13dc77d94da5d07bf27f0b8614af7c74962929c498c16ab4a5e659f8c114c9"),
    "src/frontend/oir/owned/owned_vector_queries.rs": (8213, "c02361cbc55bb673c19aea3012fa66ad0c5a6d3ca490b9e0e997b0da53e6f3c2"),
    "src/frontend/oir/owned/owned_vector_query_tests.rs": (7419, "5880307a61134aad1cdd8d02aa3bddad482fcdbf5e642ebbab9d55eaacc4a75c"),
    "src/frontend/oir/owned/owned_vector_resource_fixture.rs": (12233, "57c3eb624595ded747e415e34285cf06ce9afbc62f588760c39d658ea270dc27"),
    "src/frontend/oir/owned/owned_vector_resource_tests.rs": (39073, "d8d01ee2bf9f58d5f11747cc57aec794102c0ddc05203d9d9f7c8a956c459db1"),
    "src/frontend/oir/owned/owned_vector_resources.rs": (32897, "0fa2cc5c43fbbc79c78ef024074cf1052c897628ca29e4b52c4c4c26519455da"),
    "src/frontend/oir/owned/owned_vector_signature_tests.rs": (10298, "48120dad25b405fb35d756ee667685a6f9e2575112461d030cbf285d7461d3de"),
    "src/frontend/oir/owned/owned_vector_type_tests.rs": (2677, "dcf397f1630c6f74dceb1b313a8b31f93e2991c1660d34961092f64341690b79"),
    "src/frontend/oir/owned/owned_vector_verified.rs": (9482, "600b051ea6c231a876e5e4d0571695c6fc278032c1e8fb75862b962498e34663"),
    "src/frontend/oir/owned/owned_vector_witness_tests.rs": (19080, "0f556f855c661fad7c42202d0c8d8e18ae58122d34e62b5cf1016341f0c5497d"),
    "src/frontend/oir/owned/plan.rs": (42153, "80f5ef1c2b6140a141fada658ca1ef044228026d8d98079e8667884a85f35fec"),
    "src/frontend/oir/owned/reviewer_array_reference_tests.rs": (67198, "91b05bf8183698e128135eb681216cd0b6f6ad78e08c2abf26505ecd8c51bfe8"),
    "src/frontend/oir/owned/shape.rs": (67757, "613a8f35233dd3bf5498dd8033a007a03fd57accb9a12d023c29338ec77204e9"),
    "src/frontend/oir/owned/shape_declaration_queries.rs": (15465, "34b2a457c5472f6dccbf3ad876e3357acabc630be3c7ba94eb8b01a2cee36061"),
    "src/frontend/oir/owned/source/array_pipeline_rows.rs": (58558, "dc901f432340bdf508fa2abfbfaf249b9a98a561d80b0dcc7118a6bafeaaf02b"),
    "src/frontend/oir/owned/source/association.rs": (100683, "4027876972220d25c071c6fd4a6fe3aba57b41a6b571e7bbe7ed1aeb29f46249"),
    "src/frontend/oir/owned/source/budget.rs": (19532, "62e86dbd19b16a814f6641a122712da8c49a2beb3faf0c777d150ff23dfec0c6"),
    "src/frontend/oir/owned/source/candidate_adapter.rs": (45443, "ac7280deaefa721a2d66692fb1049389b0c6724c03f6575c84219a36b5d5721b"),
    "src/frontend/oir/owned/source/candidate_native.rs": (32958, "bf41890059cd884bf224936ffcf4bb0ad7fba06e53236cee35ca1c2d29f0049b"),
    "src/frontend/oir/owned/source/hir_budget.rs": (63154, "ff8055654b5e8d4bb9dd74c618889d1032afd5dc6be36e31e66065fa3d606a08"),
    "src/frontend/oir/owned/source/lower.rs": (137834, "ea78de1f32842913717e472e84dfff1770e7af63758fb3c7611d7663d9aff392"),
    "src/frontend/oir/owned/source/mod.rs": (2673, "73c5d684022d39dcc5b7b680895b60461d9db3ac599543d9f45e661e9c1f149b"),
    "src/frontend/oir/owned/source/owned_vector_source.rs": (5527, "c82df8b90a0dc02c600f44eae8db99a8ae027742a4f85d3ca28c26b318c37a45"),
    "src/frontend/oir/owned/source/owned_vector_source_execution_tests.rs": (17171, "e8975bbc928bcd1b6b66b26846cc0ac3539035ce1606e8d444c754abcaf37d84"),
    "src/frontend/oir/owned/source/owned_vector_source_facade_tests.rs": (14791, "b186947eadb5a9f97f231d9d228f1349b935b24e211ca57f543e3761dab53a15"),
    # Two real active enum regressions, both with 3 allocations/552-byte peak.
    "src/frontend/oir/owned/source/owned_vector_source_resource_tests.rs": (6455, "8e344457b9bbc8068a7a11ceb50a2cce6f3094860801edddc018b3d04a1463a2"),
    "src/frontend/oir/owned/source/owned_vector_source_tests.rs": (9957, "e628a7e6d1e91bdc61fb10095aee121e0973301c4eab3c4ef6c95d45779b92a2"),
    "src/frontend/oir/owned/source/program.rs": (51108, "79b95deeae03fedc9652c4fa8cf846543c5566bf24362cfd9ccec3182236cfa2"),
    "src/frontend/oir/owned/source/resolve.rs": (173533, "fff398b80b2e59c133364eb9ec95538c57a429d76915967653b181eedab6bac9"),
    "src/frontend/oir/owned/source/typeck.rs": (233343, "367ecc4818c7075c0110d274318246c608e686c58b13cdf7796ecc02b730fb95"),
    "src/frontend/oir/owned/source/vector_lower.rs": (10189, "9b982fc35fa531447a76a8069cdfb99b9c681c6f66fc624eeb910c24ded34460"),
    "src/frontend/oir/owned/vector_descriptors.rs": (8302, "0d12ba1b27152bcadf095b0d4b50e96190624896c08f220bab5674697f3305b3"),
    "src/frontend/oir/owned/vector_type.rs": (4537, "3001b8ce01fc88e0053dd8ea812c39d93edabae8ac7595ce14bda531c94a7b21"),
    "src/frontend/oir/owned/verified.rs": (31118, "aa6814b098f4623a7637560701f11dbcce50ee66075409b52902574b29ab3f1f"),
    "src/frontend/oir/owned/verified_program_queries.rs": (8498, "2110dbf511ab92d9ee81f82b4f5534ee72dffca450f12a4117eef80bc08eabfb"),
    "src/frontend/oir/owned_types.rs": (107407, "5fb0f3dc2eb15aff2b34559ae2029192d1db2696560425fdc7e70c19e949e6b2"),
    "src/frontend/oir/owned_types/composition.rs": (20577, "00f3b661a853f1eea7e1c2bda005f2be1d3a3c409cb7b227d1cfcdf3b55e13a1"),
    "src/frontend/oir/owned_types/enums.rs": (66813, "d379c55dac6057cb6599b8b7a83eaf9a54c19eae261722d241b3c33fd8edee46"),
    "src/frontend/oir/owned_types/owned_vector.rs": (13414, "0823df65ff4259f414da05d3efac8017e47824a06550e412d9fbfefac844ef17"),
    "src/frontend/oir/owned_types/resources.rs": (32943, "5a13dcacc213c491900e161df4ab200fbff77a592ad3d0677af8ba6503b217ac"),
    "src/frontend/oir/source.rs": (10166, "91b295d84b5f5e272a2d42406137dd36216611ee0df2b9f63942f15e4b168842"),
    "src/frontend/oir/source/owned_vector_route_tests.rs": (22494, '6e4826683aec2c058ad884761b08e792818d39ad2ebcfae6854ba4fa5a1be802'),
    "src/frontend/oir/source/sealed.rs": (45476, '5f6d3d64872f828c82bf45d6496b98010f60c5b559225852e11cf8f27612a6c1'),
    "src/frontend/owned_vector_builtins.rs": (10964, "ec68a168a344b9dd0b6025918c7ddcdbc8d3f101568334f4c4f285fd22f3fd5b"),
    "src/frontend/parser.rs": (108481, "da3bb5bf78b1ce2bc60e6f8475b0df75495ebe2e76f0110f6a08a9bcc376cb7c"),
    "src/frontend/vector_builtins.rs": (16545, "665a72d3389ccdde34b12648ecd7762a73c8e2f7470dd3eeb3a36fc4f96c862c"),
    "src/frontend/declaration_index/resource.rs": (19774, '98cb16f86d1bfeceb22889aae3e5457af34dbdd1818830024b1f34d2951d15bb'),
}

REVIEWED_DIRECTIVES = {
    "native/experimental_bytebuf.c": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": ((b'#include "experimental_bytebuf.h"\n', 1), (b'#include "experimental_vector.h"\n', 1), (b'#ifdef OXID_OWNED_VECTOR_RUNTIME\n', 1), (b'#include "experimental_owned_vector.h"\n', 1), (b'#endif\n', 1), (b'#include <errno.h>\n', 1), (b'#include <limits.h>\n', 1), (b'#include <stdlib.h>\n', 1), (b'#include <string.h>\n', 1), (b'#include <unistd.h>\n', 1), (b'#ifdef OXID_BYTEBUF_TESTING\n', 1), (b'#else\n', 1), (b'#endif\n', 1), (b'#ifdef OXID_BYTEBUF_TESTING\n', 1), (b'#else\n', 1), (b'#endif\n', 1), (b'#ifdef OXID_OWNED_VECTOR_RUNTIME\n', 1), (b'#endif\n', 1), (b'#include "experimental_vector.c"\n', 1),),
        "build_script": None,
    },
    "native/experimental_owned_record.c": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": ((b'#include "experimental_owned_record.h"\n', 1), (b'#define OXID_OWNED_MAX_DEPTH 64u\n', 1), (b'#ifndef OXID_OWNED_VECTOR_SCHEMAS\n', 1), (b'#else\n', 1), (b'#endif\n', 1), (b'#ifndef OXID_OWNED_VECTOR_SCHEMAS\n', 1), (b'#else\n', 1), (b'#endif\n', 1), (b'#ifndef OXID_OWNED_VECTOR_SCHEMAS\n', 1), (b'#else\n', 1), (b'#endif\n', 1),),
        "build_script": None,
    },
    "native/experimental_owned_record.h": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": ((b'#ifndef OXID_EXPERIMENTAL_OWNED_RECORD_H\n', 1), (b'#define OXID_EXPERIMENTAL_OWNED_RECORD_H\n', 1), (b'#include "experimental_vector.h"\n', 1), (b'#ifdef OXID_OWNED_VECTOR_SCHEMAS\n', 1), (b'#endif\n', 2),),
        "build_script": None,
    },
    "native/experimental_owned_vector.c": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": ((b'#include "experimental_owned_vector.h"\n', 1), (b'#include <limits.h>\n', 1), (b'#ifdef OXID_OWNED_VECTOR_RUNTIME\n', 1), (b'#include "experimental_owned_record.c"\n', 1), (b'#ifndef OXID_OWNED_VECTOR_ALIGNED_ALLOC\n', 1), (b'#define OXID_OWNED_VECTOR_ALIGNED_ALLOC aligned_alloc\n', 1), (b'#endif\n', 2),),
        "build_script": None,
    },
    "native/experimental_owned_vector.h": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": ((b'#ifndef OXID_EXPERIMENTAL_OWNED_VECTOR_H\n', 1), (b'#define OXID_EXPERIMENTAL_OWNED_VECTOR_H\n', 1), (b'#include "experimental_bytebuf.h"\n', 1), (b'#include "experimental_owned_record.h"\n', 1), (b'#define OXID_OWNED_VECTOR_NODE (UINT32_MAX - UINT32_C(2))\n', 1), (b'#define OXID_OWNED_VECTOR_HEADER UINT64_C(64)\n', 1), (b'#define OXID_OWNED_VECTOR_ALIGNMENT 16u\n', 1), (b'#endif\n', 1),),
        "build_script": None,
    },
    "native/experimental_vector.c": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": ((b'#include "experimental_vector.h"\n', 1), (b'#include <limits.h>\n', 1), (b'#include <stdlib.h>\n', 1), (b'#include <string.h>\n', 1), (b'#ifndef OXID_VECTOR_ALIGNED_ALLOC\n', 1), (b'#define OXID_VECTOR_ALIGNED_ALLOC aligned_alloc\n', 1), (b'#endif\n', 1), (b'#ifndef OXID_RECORD_VECTOR_SCHEMAS\n', 1), (b'#else\n', 1), (b'#endif\n', 1), (b'#ifdef OXID_OWNED_VECTOR_RUNTIME\n', 1), (b'#endif\n', 1),),
        "build_script": None,
    },
    "native/typed_bytebuf.c": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": ((b'#include "experimental_bytebuf.c"\n', 1), (b'#include "experimental_vector.h"\n', 1), (b'#include "experimental_files.c"\n', 1), (b'#ifdef OXID_OWNED_VECTOR_RUNTIME\n', 1), (b'#include "experimental_owned_vector.c"\n', 1), (b'#endif\n', 1), (b'#include "typed_vector.c"\n', 1), (b'#ifdef OXID_BYTEBUF_TESTING\n', 1), (b'#endif\n', 1), (b'#ifdef OXID_BYTEBUF_TESTING\n', 1), (b'#endif\n', 1),),
        "build_script": None,
    },
    "native/typed_vector.c": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": ((b'#ifdef OXID_OWNED_VECTOR_RUNTIME\n', 1), (b'#endif\n', 1),),
        "build_script": None,
    },
    "src/frontend/builtin_catalog.rs": {
        "legacy": ([], [b'#[path = "experimental_builtins.rs"]', b'#[path = "owned_vector_builtins.rs"]', b'#[path = "vector_builtins.rs"]']),
        "attributes": ((b'#![allow(dead_code)]', 1), (b'#[path = "experimental_builtins.rs"]', 1), (b'#[path = "owned_vector_builtins.rs"]', 1), (b'#[path = "vector_builtins.rs"]', 1), (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1), (b'#[default]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[allow(dead_code)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 4), (b'#[cfg(test)]', 1), (b'#[test]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/declaration_index.rs": {
        "legacy": ([], []),
        "attributes": ((b'#![allow(dead_code)]', 1), (b'#[cfg(test)]', 7), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[repr(C)]', 1), (b'#[derive(Clone, Copy, Debug, Default)]', 1), (b'#[repr(C)]', 1), (b'#[derive(Clone, Copy, Debug, Default)]', 1), (b'#[repr(C)]', 1), (b'#[derive(Clone, Copy, Debug, Default)]', 1), (b'#[repr(C)]', 1), (b'#[derive(Clone, Copy, Debug, Default)]', 1), (b'#[repr(C)]', 1), (b'#[derive(Clone, Copy, Debug, Default)]', 1), (b'#[repr(C)]', 1), (b'#[derive(Clone, Copy, Debug, Default)]', 1), (b'#[repr(C)]', 1), (b'#[derive(Clone, Copy, Debug, Default)]', 1), (b'#[repr(C)]', 1), (b'#[derive(Clone, Copy, Debug, Default)]', 1), (b'#[repr(C)]', 1), (b'#[derive(Clone, Copy, Debug, Default)]', 1), (b'#[repr(C)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[repr(C)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Debug)]', 2), (b'#[allow(dead_code)]', 1), (b'#[repr(C, align(8))]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 3), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy, Debug)]', 2), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[cfg(test)]', 4), (b'#[derive(Debug)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/declaration_index/generics.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[cfg(test)]', 1), (b'#[derive(Default)]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 3), (b'#[allow(dead_code)]', 1), (b'#[derive(Debug, PartialEq, Eq)]', 3), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[derive(Clone, Copy)]', 2), (b'#[allow(clippy::too_many_arguments)]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/declaration_index/generics/nominal.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Debug, PartialEq, Eq)]', 2), (b'#[derive(Debug, Default, PartialEq, Eq)]', 1), (b'#[allow(dead_code)]', 1), (b'#[allow(clippy::too_many_arguments)]', 3),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/declaration_index/generics/owned_candidate.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[derive(Debug)]', 2), (b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[allow(dead_code)]', 4), (b'#[allow(clippy::too_many_arguments)]', 3), (b'#[cfg(test)]', 1), (b'#[test]', 14), (b'#[allow(dead_code)]', 10), (b'#[test]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/declaration_index/sealed.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Debug)]', 1), (b'#[repr(C, align(8))]', 1), (b'#[derive(Debug)]', 1), (b'#[cfg(test)]', 3), (b'#[derive(Debug)]', 3), (b'#[cfg(test)]', 5), (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 3), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 4), (b'#[cfg(not(test))]', 1), (b'#[allow(unused_mut)]', 1), (b'#[cfg(test)]', 11), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 9), (b'#[test]', 2), (b'#[cfg(test)]', 3), (b'#[test]', 5),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/declaration_index/tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[cfg(target_os = "linux")]', 1), (b'#[test]', 1), (b'#[cfg(target_os = "linux")]', 1), (b'#[test]', 1), (b'#[cfg(target_os = "linux")]', 1), (b'#[test]', 1), (b'#[cfg(target_os = "linux")]', 1), (b'#[test]', 1), (b'#[cfg(target_os = "linux")]', 1), (b'#[test]', 2), (b'#[cfg(target_os = "linux")]', 1), (b'#[test]', 2), (b'#[cfg(target_os = "linux")]', 1), (b'#[test]', 3), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[test]', 1), (b'#[cfg(target_os = "linux")]', 1), (b'#[test]', 2), (b'#[cfg(target_os = "linux")]', 1), (b'#[test]', 1), (b'#[cfg(target_os = "linux")]', 1), (b'#[test]', 2), (b'#[cfg(target_os = "linux")]', 1), (b'#[test]', 1), (b'#[cfg(target_os = "linux")]', 1), (b'#[test]', 2),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/experimental_builtins.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[repr(u8)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1), (b'#[repr(transparent)]', 1), (b'#[allow(dead_code)]', 3), (b'#[cfg(test)]', 1), (b'#[test]', 4),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/native.rs": {
        "legacy": ([b'include_str!("../../native/typed_preview.c")', b'include_str!("../../native/typed_character.c")', b'include_str!("../../native/typed_float.c")', b'include_bytes!(concat!(env!("OUT_DIR")', b'include_str!("../../native/typed_integer128.c")', b'include_str!("../../native/typed_bytebuf.c")', b'include_str!("../../native/experimental_bytebuf.c")', b'include_str!("../../native/experimental_files.c")', b'include_str!("../../native/experimental_arguments.c")', b'include_str!("../../native/experimental_bytebuf.h")', b'include_str!("../../native/experimental_vector.c")', b'include_str!("../../native/experimental_vector.h")', b'include_str!("../../native/typed_vector.c")', b'include_str!("../../native/experimental_owned_record.c")', b'include_str!("../../native/experimental_owned_record.h")', b'include_str!("../../native/experimental_owned_vector.c")', b'include_str!("../../native/experimental_owned_vector.h")'], []),
        "attributes": ((b'#[cfg(all(\n    any(target_os = "linux", target_os = "macos"),\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[cfg(not(all(\n    any(target_os = "linux", target_os = "macos"),\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n)))]', 1), (b'#[cfg(unix)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[cfg(test)]', 1), (b'#[test]', 2)),
        "macros": ((b'include_str!("../../native/typed_preview.c")', 1), (b'include_str!("../../native/typed_character.c")', 1), (b'include_str!("../../native/typed_float.c")', 1), (b'include_bytes!(concat!(env!("OUT_DIR"), "/float_format.a"))', 1), (b'env!("OUT_DIR")', 1), (b'include_str!("../../native/typed_integer128.c")', 1), (b'include_str!("../../native/typed_bytebuf.c")', 1), (b'include_str!("../../native/experimental_bytebuf.c")', 1), (b'include_str!("../../native/experimental_files.c")', 1), (b'include_str!("../../native/experimental_arguments.c")', 1), (b'include_str!("../../native/experimental_bytebuf.h")', 1), (b'include_str!("../../native/experimental_vector.c")', 1), (b'include_str!("../../native/experimental_vector.h")', 1), (b'include_str!("../../native/typed_vector.c")', 1), (b'include_str!("../../native/experimental_owned_record.c")', 1), (b'include_str!("../../native/experimental_owned_record.h")', 1), (b'include_str!("../../native/experimental_owned_vector.c")', 1), (b'include_str!("../../native/experimental_owned_vector.h")', 1), (b'cfg!(windows)', 1), (b'cfg!(target_os = "macos")', 1), (b'cfg!(windows)', 1), (b'cfg!(target_os = "macos")', 4),),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/array_observe.rs": {
        "legacy": ([], [b'#[path = "reviewer_array_observer_tests.rs"]']),
        "attributes": ((b'#[derive(Clone, Copy, Debug, Default)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[derive(Debug)]', 1), (b'#[derive(Default)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[test]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[path = "reviewer_array_observer_tests.rs"]', 1), (b'#[test]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/builtins.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/byte_buffer.rs": {
        "legacy": ([], [b'#[path = "file_resources.rs"]', b'#[path = "owned_vector_resources.rs"]', b'#[path = "vector_resources.rs"]']),
        "attributes": ((b'#[path = "file_resources.rs"]', 1), (b'#[path = "owned_vector_resources.rs"]', 1), (b'#[path = "vector_resources.rs"]', 1), (b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[repr(C)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[repr(i32)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[repr(C)]', 1), (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[cfg(all(not(test), target_pointer_width = "64"))]', 1), (b'#[cfg(test)]', 2), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 2), (b'#[derive(Clone, Copy, Default)]', 1), (b'#[derive(Clone, Copy, Debug, Default)]', 1), (b'#[cfg(test)]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/cfg.rs": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/debug.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[allow(dead_code)]', 2), (b'#[derive(Clone, Copy)]', 3), (b'#[cfg(test)]', 1), (b'#[test]', 1), (b'#[cfg(all(test, target_pointer_width = "64"))]', 1), (b'#[test]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/execute.rs": {
        "legacy": ([], [b'#[path = "execute_floats.rs"]', b'#[path = "execute_values.rs"]', b'#[path = "execute_bitwise.rs"]', b'#[path = "execute_integer128.rs"]', b'#[path = "execute_integers.rs"]', b'#[path = "execute_experimental.rs"]', b'#[path = "execute_owned_vector.rs"]', b'#[path = "execute_vector.rs"]', b'#[path = "execute_composition.rs"]', b'#[path = "execute_resources.rs"]', b'#[path = "array_observe.rs"]', b'#[path = "execute_tests.rs"]', b'#[path = "execute_owned_vector_tests.rs"]', b'#[path = "execute_owned_vector_entry_tests.rs"]', b'#[path = "builtin_output_reference_tests.rs"]', b'#[path = "byte_storage_codec_tests.rs"]']),
        "attributes": ((b'#[path = "execute_floats.rs"]', 1), (b'#[path = "execute_values.rs"]', 1), (b'#[path = "execute_bitwise.rs"]', 1), (b'#[path = "execute_integer128.rs"]', 1), (b'#[path = "execute_integers.rs"]', 1), (b'#[path = "execute_experimental.rs"]', 1), (b'#[path = "execute_owned_vector.rs"]', 1), (b'#[path = "execute_vector.rs"]', 1), (b'#[path = "execute_composition.rs"]', 1), (b'#[path = "execute_resources.rs"]', 1), (b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy)]', 2), (b'#[cfg(test)]', 1), (b'#[derive(Clone, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy)]', 2), (b'#[cfg(test)]', 66), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[cfg(test)]', 13), (b'#[path = "array_observe.rs"]', 1), (b'#[cfg(test)]', 4), (b'#[path = "execute_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "execute_owned_vector_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "execute_owned_vector_entry_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "builtin_output_reference_tests.rs"]', 1), (b'#[cfg(test)]', 2), (b'#[test]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 1), (b'#[cfg(test)]', 1), (b'#[path = "byte_storage_codec_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 4),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/execute_composition.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[cfg(test)]', 1), (b'#[test]', 2),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/execute_experimental.rs": {
        "legacy": ([], [b'#[path = "execute_io_delivery.rs"]', b'#[path = "execute_file_delivery.rs"]']),
        "attributes": ((b'#[path = "execute_io_delivery.rs"]', 1), (b'#[path = "execute_file_delivery.rs"]', 1), (b'#[derive(Clone)]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[cfg(test)]', 3),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/execute_file_delivery.rs": {
        "legacy": ([], [b'#[path = "execute_file_seek_tests.rs"]']),
        "attributes": ((b'#[cfg(all(\n    test,\n    any(target_os = "linux", target_os = "macos"),\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[path = "execute_file_seek_tests.rs"]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[allow(dead_code)]', 2), (b'#[cfg(test)]', 2),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/execute_io_delivery.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[allow(dead_code)]', 2), (b'#[cfg(test)]', 3),),
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
    "src/frontend/oir/owned/execute_owned_vector_entry_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 4),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/execute_owned_vector_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 3),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/execute_resources.rs": {
        "legacy": ([], [b'#[path = "execute_resource_tests.rs"]', b'#[path = "execute_vector_resource_tests.rs"]']),
        "attributes": ((b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[path = "execute_resource_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "execute_vector_resource_tests.rs"]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/execute_vector.rs": {
        "legacy": ([], [b'#[path = "execute_vector_tests.rs"]']),
        "attributes": ((b'#[derive(Clone, Copy)]', 1), (b'#[allow(dead_code)]', 3), (b'#[cfg(test)]', 6), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[cfg(test)]', 5), (b'#[path = "execute_vector_tests.rs"]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/flow.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 8), (b'#[repr(u8)]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/mod.rs": {
        "legacy": ([], []),
        "attributes": ((b'#![allow(dead_code)]', 1), (b'#![allow(clippy::result_large_err)]', 1), (b'#[cfg(test)]', 3), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[derive(Clone, Debug)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Debug)]', 1), (b'#[derive(Clone, Debug)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Clone, Debug)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Debug)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Debug)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[derive(Clone, Debug)]', 2), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Debug)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Clone, Debug)]', 3), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 5), (b'#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 36),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native.rs": {
        "legacy": ([], [b'#[path = "native_bitwise.rs"]', b'#[path = "native_composition.rs"]', b'#[path = "native_experimental.rs"]', b'#[path = "native_floats.rs"]', b'#[path = "native_integer128.rs"]', b'#[path = "native_integers.rs"]', b'#[path = "native_numeric.rs"]', b'#[path = "native_owned_vector.rs"]', b'#[path = "native_owned_vector_schemas.rs"]', b'#[path = "native_reachability.rs"]', b'#[path = "native_resources.rs"]', b'#[path = "native_vector.rs"]', b'#[path = "builtin_input_native_tests.rs"]', b'#[path = "builtin_output_native_tests.rs"]', b'#[path = "native_tests.rs"]', b'#[path = "native_vector_tests.rs"]', b'#[path = "native_inventory_admission_tests.rs"]', b'#[path = "hosted_capacity_tests.rs"]', b'#[path = "byte_storage_native_tests.rs"]', b'#[path = "copy_record_native_tests.rs"]', b'#[path = "copy_record_debug_tests.rs"]', b'#[path = "record_vector_native_tests.rs"]', b'#[path = "native_owned_vector_schema_tests.rs"]', b'#[path = "native_owned_vector_tests.rs"]', b'#[path = "native_owned_vector_commit_tests.rs"]']),
        "attributes": ((b'#[cfg(test)]', 1), (b'#[path = "native_bitwise.rs"]', 1), (b'#[path = "native_composition.rs"]', 1), (b'#[path = "native_experimental.rs"]', 1), (b'#[path = "native_floats.rs"]', 1), (b'#[path = "native_integer128.rs"]', 1), (b'#[path = "native_integers.rs"]', 1), (b'#[path = "native_numeric.rs"]', 1), (b'#[path = "native_owned_vector.rs"]', 1), (b'#[path = "native_owned_vector_schemas.rs"]', 1), (b'#[path = "native_reachability.rs"]', 1), (b'#[path = "native_resources.rs"]', 1), (b'#[path = "native_vector.rs"]', 1), (b'#[allow(dead_code)]', 2), (b'#[derive(Clone, Debug, Default)]', 1), (b'#[derive(Default)]', 1), (b'#[cfg(test)]', 2), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Clone, Copy, Default, Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[cfg(test)]', 3), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(clippy::too_many_arguments)]', 3), (b'#[derive(Clone, Copy)]', 1), (b'#[cfg(test)]', 2), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]', 1), (b'#[repr(transparent)]', 1), (b'#[allow(non_upper_case_globals)]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[allow(clippy::too_many_arguments)]', 2), (b'#[cfg(test)]', 2), (b'#[derive(Clone, Copy)]', 2), (b'#[allow(dead_code)]', 1), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[cfg(test)]', 2), (b'#[derive(Clone, Copy)]', 1), (b'#[cfg(test)]', 3), (b'#[path = "builtin_input_native_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "builtin_output_native_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "native_tests.rs"]', 1), (b'#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]', 1), (b'#[path = "native_vector_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[cfg(test)]', 1), (b'#[path = "native_inventory_admission_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "hosted_capacity_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 4), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[cfg(test)]', 1), (b'#[test]', 1), (b'#[allow(dead_code)]', 2), (b'#[test]', 1), (b'#[allow(dead_code)]', 4), (b'#[test]', 1), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[cfg(test)]', 1), (b'#[path = "byte_storage_native_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[cfg(test)]', 1), (b'#[path = "copy_record_native_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "copy_record_debug_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "record_vector_native_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "native_owned_vector_schema_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "native_owned_vector_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "native_owned_vector_commit_tests.rs"]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_composition.rs": {
        "legacy": ([], [b'#[path = "native_vector_composition_tests.rs"]']),
        "attributes": ((b'#[allow(dead_code)]', 1), (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[cfg(test)]', 1), (b'#[path = "native_vector_composition_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[cfg(all(\n        target_os = "linux",\n        target_pointer_width = "64",\n        target_endian = "little",\n        any(target_arch = "x86_64", target_arch = "aarch64")\n    ))]', 1), (b'#[test]', 1), (b'#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]', 1),),
        "macros": (),
        "c_preprocessor": ((b'#include <stdint.h>\n', 1), (b'#include <string.h>\n', 1), (b'#define CHECK(condition) do { if (!(condition)) return __LINE__; } while (0)\n', 1),),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_experimental.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 2), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(clippy::too_many_arguments)]', 4),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_owned_vector.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[allow(dead_code)]', 2), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(clippy::too_many_arguments)]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_owned_vector_commit_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 2),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_owned_vector_schema_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 4),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_owned_vector_schemas.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy)]', 1), (b'#[allow(dead_code)]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_owned_vector_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 8),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_reachability.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy)]', 2), (b'#[cfg(test)]', 1), (b'#[test]', 7),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_resources.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[allow(dead_code)]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[allow(clippy::too_many_arguments)]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_storage.rs": {
        "legacy": ([], [b'#[path = "native_inventory_tests.rs"]', b'#[path = "native_storage_tests.rs"]']),
        "attributes": ((b'#[cfg(test)]', 1), (b'#[path = "native_inventory_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "native_storage_tests.rs"]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_tests.rs": {
        "legacy": ([], [b'#[path = "source/native_resource_tests.rs"]', b'#[path = "native_heldout_review.rs"]', b'#[path = "source/reviewer_resource_native.rs"]', b'#[path = "array_native_resource_tests.rs"]', b'#[path = "array_native_tests.rs"]', b'#[path = "slice_native_tests.rs"]', b'#[path = "composition_native_tests.rs"]', b'#[path = "projected_slice_native_tests.rs"]', b'#[path = "enum_native_tests.rs"]']),
        "attributes": ((b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[test]', 1), (b'#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]', 1), (b'#[path = "source/native_resource_tests.rs"]', 1), (b'#[test]', 4), (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[test]', 1), (b'#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]', 1), (b'#[test]', 1), (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[test]', 1), (b'#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]', 1), (b'#[test]', 1), (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[test]', 1), (b'#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]', 1), (b'#[test]', 3), (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[test]', 1), (b'#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]', 1), (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[test]', 1), (b'#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]', 1), (b'#[test]', 1), (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[test]', 1), (b'#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]', 1), (b'#[test]', 4), (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[test]', 1), (b'#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]', 1), (b'#[test]', 1), (b'#[path = "native_heldout_review.rs"]', 1), (b'#[path = "source/reviewer_resource_native.rs"]', 1), (b'#[path = "array_native_resource_tests.rs"]', 1), (b'#[path = "array_native_tests.rs"]', 1), (b'#[path = "slice_native_tests.rs"]', 1), (b'#[path = "composition_native_tests.rs"]', 1), (b'#[path = "projected_slice_native_tests.rs"]', 1), (b'#[path = "enum_native_tests.rs"]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_vector.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[allow(dead_code)]', 3), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C, align(16))]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(clippy::too_many_arguments)]', 3), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 1), (b'#[repr(C)]', 1), (b'#[allow(dead_code)]', 2),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_vector_composition_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 3),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/native_vector_layout.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[repr(C)]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_record_bounds.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Debug)]', 2), (b'#[derive(Clone, Copy)]', 2),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_record_codec.rs": {
        "legacy": ([], [b'#[path = "owned_record_bounds.rs"]']),
        "attributes": ((b'#[path = "owned_record_bounds.rs"]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[cfg(test)]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_context.rs": {
        "legacy": ([], [b'#[path = "owned_vector_queries.rs"]', b'#[path = "shape_declaration_queries.rs"]', b'#[path = "owned_vector_proof.rs"]', b'#[path = "owned_vector_verified.rs"]']),
        "attributes": ((b'#[path = "owned_vector_queries.rs"]', 1), (b'#[path = "shape_declaration_queries.rs"]', 1), (b'#[path = "owned_vector_proof.rs"]', 1), (b'#[path = "owned_vector_verified.rs"]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy)]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_context_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 6), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[test]', 2),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_declaration_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 8),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_descriptor_tests.rs": {
        "legacy": ([], [b'#[path = "owned_vector_context_tests.rs"]', b'#[path = "owned_vector_query_tests.rs"]', b'#[path = "owned_vector_signature_tests.rs"]', b'#[path = "owned_vector_proof_tests.rs"]', b'#[path = "owned_vector_declaration_tests.rs"]', b'#[path = "owned_vector_execution_fixture.rs"]']),
        "attributes": ((b'#[path = "owned_vector_context_tests.rs"]', 1), (b'#[path = "owned_vector_query_tests.rs"]', 1), (b'#[path = "owned_vector_signature_tests.rs"]', 1), (b'#[path = "owned_vector_proof_tests.rs"]', 1), (b'#[path = "owned_vector_declaration_tests.rs"]', 1), (b'#[path = "owned_vector_execution_fixture.rs"]', 1), (b'#[test]', 7), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[test]', 2),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_descriptors.rs": {
        "legacy": ([], [b'#[path = "owned_vector_context.rs"]', b'#[path = "owned_vector_descriptor_tests.rs"]']),
        "attributes": ((b'#[path = "owned_vector_context.rs"]', 1), (b'#[derive(Clone, Copy, Debug, Default)]', 1), (b'#[derive(Debug)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[derive(Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[path = "owned_vector_descriptor_tests.rs"]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_execution_fixture.rs": {
        "legacy": ([], [b'#[path = "owned_vector_resource_fixture.rs"]', b'#[path = "owned_vector_file_fixture.rs"]']),
        "attributes": ((b'#[path = "owned_vector_resource_fixture.rs"]', 1), (b'#[path = "owned_vector_file_fixture.rs"]', 1), (b'#[derive(Clone, Copy)]', 2), (b'#[test]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_file_fixture.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[cfg(all(\n        any(target_os = "linux", target_os = "macos"),\n        target_pointer_width = "64",\n        target_endian = "little",\n        any(target_arch = "x86_64", target_arch = "aarch64")\n    ))]', 1), (b'#[cfg(target_os = "linux")]', 1), (b'#[test]', 1), (b'#[cfg(all(\n    any(target_os = "linux", target_os = "macos"),\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[test]', 1), (b'#[cfg(all(\n    any(target_os = "linux", target_os = "macos"),\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[test]', 1), (b'#[cfg(all(\n    any(target_os = "linux", target_os = "macos"),\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 2), (b'#[test]', 1),),
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
    "src/frontend/oir/owned/owned_vector_proof.rs": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_proof_tests.rs": {
        "legacy": ([], [b'#[path = "owned_vector_witness_tests.rs"]']),
        "attributes": ((b'#[path = "owned_vector_witness_tests.rs"]', 1), (b'#[test]', 7),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_queries.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy)]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_query_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 4),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_resource_fixture.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 3),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_resource_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 14), (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64"),\n))]', 1), (b'#[test]', 4), (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64"),\n))]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_resources.rs": {
        "legacy": ([], [b'#[path = "owned_vector_resource_tests.rs"]']),
        "attributes": ((b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[repr(C)]', 1), (b'#[repr(C, align(16))]', 1), (b'#[cfg(test)]', 4), (b'#[path = "owned_vector_resource_tests.rs"]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_signature_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 6),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/owned_vector_type_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 2),),
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
    "src/frontend/oir/owned/owned_vector_witness_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 8),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/plan.rs": {
        "legacy": ([], [b'#[path = "native_storage.rs"]', b'#[path = "native_vector_layout.rs"]', b'#[path = "owned_vector_plan.rs"]']),
        "attributes": ((b'#[path = "native_storage.rs"]', 1), (b'#[path = "native_vector_layout.rs"]', 1), (b'#[path = "owned_vector_plan.rs"]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1), (b'#[derive(Debug)]', 3), (b'#[cfg(test)]', 7), (b'#[test]', 1), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[test]', 1),),
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
        "attributes": ((b'#[derive(Debug, Clone, Copy, PartialEq, Eq)]', 1), (b'#[derive(Clone, Default)]', 2), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[derive(Clone, Copy)]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/shape_declaration_queries.rs": {
        "legacy": ([], []),
        "attributes": (),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/array_pipeline_rows.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[cfg(test)]', 1), (b'#[test]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/association.rs": {
        "legacy": ([], [b'#[path = "output_lower_tests.rs"]', b'#[path = "byte_storage_association_tests.rs"]']),
        "attributes": ((b'#[allow(clippy::large_enum_variant)]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 3), (b'#[derive(Clone, Copy)]', 1), (b'#[cfg(test)]', 2), (b'#[allow(dead_code)]', 2), (b'#[cfg(test)]', 1), (b'#[path = "output_lower_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 2), (b'#[test]', 3), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 1), (b'#[test]', 1), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[cfg(test)]', 1), (b'#[test]', 3), (b'#[cfg(test)]', 2), (b'#[path = "byte_storage_association_tests.rs"]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/budget.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 6), (b'#[test]', 1), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[cfg(test)]', 1), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 8), (b'#[test]', 1), (b'#[cfg(test)]', 3), (b'#[derive(Clone, Copy)]', 1), (b'#[cfg(test)]', 4), (b'#[test]', 1), (b'#[cfg(test)]', 3),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/candidate_adapter.rs": {
        "legacy": ([], [b'#[path = "candidate_mutations.rs"]', b'#[path = "candidate_native.rs"]']),
        "attributes": ((b'#[derive(Default)]', 1), (b'#[test]', 1), (b'#[ignore = "source receipt collector; requires explicit source and fresh output directories"]', 1), (b'#[test]', 4), (b'#[cfg(test)]', 1), (b'#[path = "candidate_mutations.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "candidate_native.rs"]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/candidate_native.rs": {
        "legacy": ([], [b'#[ignore = "bounded native source collector; requires frozen original source, budget controls, pinned LLVM and a fresh output path"]']),
        "attributes": ((b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[test]', 1), (b'#[ignore = "bounded native source collector; requires frozen original source, budget controls, pinned LLVM and a fresh output path"]', 1), (b'#[test]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/hir_budget.rs": {
        "legacy": ([], [b'#[path = "byte_storage_resources.rs"]', b'#[path = "hir_budget_tests.rs"]']),
        "attributes": ((b'#![allow(dead_code)]', 1), (b'#[path = "byte_storage_resources.rs"]', 1), (b'#[cfg(test)]', 5), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[allow(dead_code)]', 3), (b'#[derive(Clone, Copy)]', 2), (b'#[cfg(test)]', 2), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[path = "hir_budget_tests.rs"]', 1), (b'#[cfg(test)]', 2), (b'#[test]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/lower.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy)]', 8), (b'#[cfg(test)]', 16), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 5), (b'#[test]', 1), (b'#[allow(dead_code)]', 2), (b'#[test]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/mod.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[cfg(test)]', 11), (b'#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]', 1), (b'#[cfg(test)]', 27),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/owned_vector_source.rs": {
        "legacy": ([], [b'#[path = "owned_vector_source_tests.rs"]']),
        "attributes": ((b'#[allow(dead_code)]', 4), (b'#[cfg(test)]', 1), (b'#[path = "owned_vector_source_tests.rs"]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/owned_vector_source_execution_tests.rs": {
        "legacy": ([], [b'#[path = "owned_vector_source_resource_tests.rs"]']),
        "attributes": ((b'#[test]', 7), (b'#[path = "owned_vector_source_resource_tests.rs"]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/owned_vector_source_facade_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 4), (b'#[cfg(target_pointer_width = "64")]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/owned_vector_source_resource_tests.rs": {
        "legacy": ([b'include_str!("../../../../../scripts/owned_record_vector_acceptance/enum16.ox")', b'include_str!("../../../../../scripts/owned_record_vector_acceptance/enum16.ox")'], []),
        "attributes": ((b'#[cfg(target_os = "linux")]', 1), (b'#[test]', 3), (b'#[cfg(all(\n    any(target_os = "linux", target_os = "macos"),\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[test]', 1), (b'#[cfg(all(\n    any(target_os = "linux", target_os = "macos"),\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1), (b'#[test]', 1),),
        "macros": ((b'include_str!("../../../../../scripts/owned_record_vector_acceptance/enum16.ox")', 2),),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/owned_vector_source_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 6),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/program.rs": {
        "legacy": ([], [b'#[path = "experimental_source_tests.rs"]', b'#[path = "i64_source_tests.rs"]', b'#[path = "owned_vector_source_execution_tests.rs"]', b'#[path = "owned_vector_source_facade_tests.rs"]', b'#[path = "candidate_adapter.rs"]']),
        "attributes": ((b'#[cfg(test)]', 2), (b'#[path = "experimental_source_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "i64_source_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "owned_vector_source_execution_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "owned_vector_source_facade_tests.rs"]', 1), (b'#[derive(Debug)]', 3), (b'#[test]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 5), (b'#[path = "candidate_adapter.rs"]', 1), (b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[allow(dead_code)]', 1), (b'#[derive(Debug)]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 2), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[derive(Debug)]', 1), (b'#[cfg(test)]', 3), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 1), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 3), (b'#[test]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/resolve.rs": {
        "legacy": ([], [b'#[path = "resolver_paid_tests.rs"]', b'#[path = "resolver_enum_tests.rs"]', b'#[path = "enum_storage_failure_tests.rs"]', b'#[path = "builtin_signature_tests.rs"]', b'#[path = "output_typing_tests.rs"]', b'#[path = "byte_storage_type_tests.rs"]']),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 7), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 2), (b'#[derive(Debug)]', 1), (b'#[allow(clippy::large_enum_variant)]', 1), (b'#[derive(Debug)]', 2), (b'#[test]', 1), (b'#[cfg(test)]', 2), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(clippy::blocks_in_conditions)]', 1), (b'#[cfg(test)]', 4), (b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 1), (b'#[derive(Debug, PartialEq, Eq)]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 11), (b'#[cfg(test)]', 2), (b'#[test]', 2), (b'#[cfg(test)]', 4), (b'#[test]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[path = "resolver_paid_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 1), (b'#[cfg(test)]', 1), (b'#[path = "resolver_enum_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "enum_storage_failure_tests.rs"]', 1), (b'#[allow(dead_code)]', 2), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 4), (b'#[cfg(test)]', 1), (b'#[path = "builtin_signature_tests.rs"]', 1), (b'#[cfg(test)]', 2), (b'#[path = "output_typing_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "byte_storage_type_tests.rs"]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/typeck.rs": {
        "legacy": ([], [b'#[path = "enum_type_tests.rs"]']),
        "attributes": ((b'#[derive(Debug)]', 3), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(clippy::large_enum_variant)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 19), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 1), (b'#[derive(Debug)]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(dead_code)]', 2), (b'#[cfg(test)]', 2), (b'#[allow(clippy::blocks_in_conditions)]', 1), (b'#[allow(dead_code)]', 3), (b'#[cfg(test)]', 2), (b'#[allow(dead_code)]', 4), (b'#[allow(clippy::type_complexity)]', 1), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 7), (b'#[test]', 1), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[test]', 1), (b'#[cfg(test)]', 2), (b'#[allow(clippy::too_many_arguments)]', 2), (b'#[allow(clippy::unused_enumerate_index)]', 1), (b'#[cfg(test)]', 3), (b'#[cfg(not(test))]', 1), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[cfg(test)]', 7), (b'#[test]', 2), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[cfg(test)]', 1), (b'#[test]', 4), (b'#[allow(dead_code)]', 1), (b'#[test]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 5), (b'#[cfg(test)]', 1), (b'#[path = "enum_type_tests.rs"]', 1), (b'#[allow(dead_code)]', 3), (b'#[test]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/source/vector_lower.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[allow(dead_code)]', 2),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/vector_descriptors.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[allow(dead_code)]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/vector_type.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned/verified.rs": {
        "legacy": ([], [b'#[path = "verified_program_queries.rs"]']),
        "attributes": ((b'#[path = "verified_program_queries.rs"]', 1), (b'#[derive(Debug)]', 2), (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 3), (b'#[cfg(test)]', 5), (b'#[allow(dead_code)]', 1), (b'#[test]', 2),),
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
        "attributes": ((b'#![allow(dead_code)]', 1), (b'#[allow(unused_imports)]', 1), (b'#[cfg(test)]', 1), (b'#[allow(unused_imports)]', 2), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 5), (b'#[repr(C, align(8))]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[repr(C, align(8))]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 8), (b'#[derive(Debug)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1), (b'#[derive(Debug)]', 2), (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[derive(Debug)]', 2), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Clone, Copy, Default)]', 1), (b'#[derive(Default)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[allow(dead_code)]', 2), (b'#[test]', 29), (b'#[cfg(test)]', 6),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned_types/composition.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[cfg(test)]', 1), (b'#[test]', 6),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned_types/enums.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[derive(Debug)]', 1), (b'#[cfg_attr(test, derive(Clone))]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1), (b'#[derive(Debug)]', 2), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Debug)]', 2), (b'#[allow(clippy::too_many_arguments)]', 2), (b'#[derive(Clone, Copy)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 23),),
        "macros": ((b'cfg!(target_pointer_width = "64")', 1),),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned_types/owned_vector.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy, Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[test]', 3),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/owned_types/resources.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2), (b'#[allow(dead_code)]', 1), (b'#[repr(u8)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 8),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/source.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[cfg(test)]', 3), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 7),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/source/owned_vector_route_tests.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[test]', 5), (b'#[cfg(target_pointer_width = "64")]', 1), (b'#[test]', 7)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/oir/source/sealed.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[allow(clippy::large_enum_variant)]', 1), (b'#[derive(Debug)]', 3), (b'#[allow(dead_code)]', 1), (b'#[derive(Clone, Copy)]', 1), (b'#[allow(clippy::large_enum_variant)]', 1), (b'#[allow(dead_code)]', 1), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[cfg(test)]', 7), (b'#[derive(Debug)]', 1), (b'#[cfg(test)]', 2), (b'#[allow(dead_code)]', 2), (b'#[cfg(test)]', 2), (b'#[test]', 1), (b'#[cfg(target_pointer_width = "64")]', 2), (b'#[allow(dead_code)]', 1), (b'#[test]', 2)),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/owned_vector_builtins.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]', 1), (b'#[repr(u8)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[repr(u8)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 3), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 3),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/parser.rs": {
        "legacy": ([], [b'#[path = "parser/arrays.rs"]', b'#[path = "parser/characters.rs"]', b'#[path = "parser/conversions.rs"]', b'#[path = "parser/enums.rs"]', b'#[path = "parser/u8_syntax_tests.rs"]', b'#[path = "parser/builtin_tests.rs"]', b'#[path = "parser/array_syntax_tests.rs"]', b'#[path = "parser/enum_syntax_tests.rs"]', b'#[path = "parser/project_tests.rs"]', b'#[path = "parser/activation_tests.rs"]']),
        "attributes": ((b'#[derive(Clone, Copy, PartialEq, Eq)]', 1), (b'#[allow(clippy::enum_variant_names)]', 1), (b'#[cfg(test)]', 2), (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 2), (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 2), (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 3), (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 3), (b'#[allow(dead_code)]', 1), (b'#[cfg(test)]', 4), (b'#[allow(clippy::too_many_arguments)]', 1), (b'#[derive(Copy)]', 1), (b'#[path = "parser/arrays.rs"]', 1), (b'#[path = "parser/characters.rs"]', 1), (b'#[path = "parser/conversions.rs"]', 1), (b'#[path = "parser/enums.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "parser/u8_syntax_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "parser/builtin_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "parser/array_syntax_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "parser/enum_syntax_tests.rs"]', 1), (b'#[cfg(test)]', 4), (b'#[path = "parser/project_tests.rs"]', 1), (b'#[cfg(test)]', 1), (b'#[path = "parser/activation_tests.rs"]', 1), (b'#[test]', 1), (b'#[cfg(test)]', 1),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/vector_builtins.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[repr(u8)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 4),),
        "macros": (),
        "c_preprocessor": (),
        "build_script": None,
    },
    "src/frontend/declaration_index/resource.rs": {
        "legacy": ([], []),
        "attributes": ((b'#[cfg(test)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[derive(Debug)]', 1), (b'#[cfg(test)]', 5), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[cfg(test)]', 9), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 2), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 5), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 1), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 1), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 1), (b'#[derive(Clone, Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[derive(Clone, Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[derive(Clone, Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[derive(Clone, Debug)]', 1), (b'#[derive(Clone, Copy, Debug, Default)]', 1), (b'#[derive(Clone, Copy, Debug)]', 1), (b'#[cfg(test)]', 1), (b'#[cfg(not(test))]', 1), (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1), (b'#[cfg(test)]', 1), (b'#[cfg(not(test))]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 1)),
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
    require(not path.is_symlink() and all(not parent.is_symlink() for parent in path.parents),
            "symlink pinned registry controller")
    before = path.stat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and not before.st_mode & 0o111
            and before.st_size == PREDECESSOR_POLICY_PIN[0], "unsafe pinned registry controller")
    raw = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, field) for field in fields)
            == tuple(getattr(path.stat(), field) for field in fields)
            and (len(raw), hashlib.sha256(raw).hexdigest()) == PREDECESSOR_POLICY_PIN,
            "changed pinned registry controller")
    module = types.ModuleType("_owned_vector_retained_registry")
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


library = load_predecessor(ROOT)
RETAINED_RUN = library.run
REGISTRY_MANIFEST = library.current_manifest
for _name in (
    "BASELINE", "BASELINE_SOURCE_SHA", "RETAINED_RUNNER_SHA", "BINDING", "RETAINED",
    "sha", "encoded", "identity", "git", "regular", "source_directives", "UNCHANGED_INPUTS",
    "authenticate_observer_adapter", "authenticate_index_resource_adapter",
    "authenticate_parser_resource_adapter", "prepare_current_observer",
    "verify_current_observer", "verify_materialized", "run_byte_buffer_observer_controls",
    "run_file_observer_controls", "run_i64_observer_controls", "run_integer_width_observer_controls",
    "run_integer128_observer_controls", "run_float_observer_controls", "run_character_observer_controls",
    "run_vector_observer_controls", "run_record_vector_observer_controls",
):
    globals()[_name] = getattr(library, _name)
PREDECESSOR_ADDED = library.PARENT_ADDED
PARENT_ADDED = PREDECESSOR_ADDED | OWNED_VECTOR_ADDED | frozenset(OWNED_VECTOR_SOURCE_ASSETS)
PREDECESSOR_POLICIES = {**library.PREDECESSOR_POLICIES,
                        PREDECESSOR_POLICY: PREDECESSOR_POLICY_PIN[1]}


def authenticate_owned_vector_observer_adapter(repo):
    parent, _ = authenticate_observer_adapter(repo)
    body = regular(repo, OWNED_VECTOR_OBSERVER_ADAPTER)
    require((len(body), sha(body)) == OWNED_VECTOR_OBSERVER_ADAPTER_PIN,
            "changed pinned owned-vector observer adapter")
    adapter = types.ModuleType("_owned_vector_observer_successor")
    adapter.__file__ = str(repo / OWNED_VECTOR_OBSERVER_ADAPTER)
    exec(compile(body, adapter.__file__, "exec"), adapter.__dict__)
    require(adapter.predecessor.descriptor() == parent.descriptor()
            and adapter.PARENT_DERIVED_PIN == parent.DERIVED_PIN,
            "owned-vector observer predecessor differs")
    return adapter, body


def capture_sources(repo, baseline, parent):
    expected_parent = set(baseline) | PREDECESSOR_ADDED
    expected = expected_parent | OWNED_VECTOR_ADDED | set(OWNED_VECTOR_SOURCE_ASSETS)
    require(set(parent) == expected_parent and len(parent) == 527,
            "wrong retained registry compiler input membership")
    require(OWNED_VECTOR_CHANGED <= expected_parent and not OWNED_VECTOR_ADDED & expected_parent
            and len(OWNED_VECTOR_CHANGED) == 63 and len(OWNED_VECTOR_ADDED) == 44,
            "wrong finite owned-vector delta membership")
    require(set(REVIEWED_INPUTS) == OWNED_VECTOR_CHANGED | OWNED_VECTOR_ADDED
            and set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS),
            "wrong finite owned-vector pin/directive membership")
    require(not set(UNCHANGED_INPUTS) & (OWNED_VECTOR_CHANGED | OWNED_VECTOR_ADDED)
            and set(UNCHANGED_INPUTS) <= expected_parent,
            "protected input cannot enter the reviewed delta")
    require(TEST_NAMED_INPUTS <= set(REVIEWED_INPUTS),
            "test-only named source outside reviewed delta")
    actual, directories = set(), set()
    for part in ("src", "native"):
        require((repo / part).is_dir() and not (repo / part).is_symlink(), "missing source root")
        for directory, dirs, files in os.walk(repo / part, followlinks=False):
            for name in dirs + files:
                path = Path(directory) / name
                require(not path.is_symlink(), "symlink source member")
                (directories if name in dirs else actual).add(path.relative_to(repo).as_posix())
    compiler = {name for name in expected if name.startswith(("src/", "native/"))}
    require(actual == compiler, "unexpected compiler source membership")
    expected_dirs = {str(path) for name in compiler for path in PurePosixPath(name).parents
                     if str(path) not in (".", "src", "native")}
    require(directories == expected_dirs, "unexpected compiler source directory")
    inputs = {name: regular(repo, name) for name in sorted(expected)}
    for name, before in parent.items():
        if name not in OWNED_VECTOR_CHANGED:
            require(inputs[name] == before, "unreviewed change outside finite owned-vector boundary: " + name)
    for name, (length, digest) in REVIEWED_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "finite owned-vector source does not match reviewed input: " + name)
        require(source_directives(name, inputs[name]) == REVIEWED_DIRECTIVES[name],
                "finite owned-vector compile-time directive changed: " + name)
    validate_source_assets(repo, inputs)
    return inputs


def validate_source_assets(repo, inputs):
    require(set(OWNED_VECTOR_SOURCE_ASSETS) == {"scripts/owned_record_vector_acceptance/enum16.ox"}
            and set(SOURCE_ASSET_INCLUDE_BINDINGS) == set(OWNED_VECTOR_SOURCE_ASSETS),
            "wrong compiled source asset membership")
    require(not set(OWNED_VECTOR_SOURCE_ASSETS) & set(REVIEWED_DIRECTIVES),
            "compiled source asset cannot become a compiler directive row")
    for name, (length, digest) in OWNED_VECTOR_SOURCE_ASSETS.items():
        body = inputs[name]
        require(len(body) == length and sha(body) == digest,
                "compiled source asset does not match reviewed input: " + name)
        owner, relative, included, count = SOURCE_ASSET_INCLUDE_BINDINGS[name]
        require(owner in REVIEWED_DIRECTIVES and count == 2
                and included == b'include_str!("' + relative.encode("utf-8") + b'")'
                and source_directives(owner, inputs[owner])["legacy"][0].count(included) == count,
                "compiled source asset include binding changed: " + name)
        require(((repo / owner).parent / relative).resolve(strict=True) == (repo / name).resolve(strict=True),
                "compiled source asset include path escaped its admitted input: " + name)


def directive_manifest(value):
    """Lossless report encoding only; admission uses the original byte census."""
    if isinstance(value, bytes):
        return {"bytes_hex": value.hex()}
    if isinstance(value, dict):
        return {name: directive_manifest(item) for name, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [directive_manifest(item) for item in value]
    return value


def current_manifest(inputs, baseline, checkout, parent):
    observer, observer_body = authenticate_owned_vector_observer_adapter(ROOT)
    inherited = REGISTRY_MANIFEST(parent, baseline, checkout, parent)
    result = {key: value for key, value in inherited.items() if key.endswith("qualified_by_this_gate")}
    result.update({
        "schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
        "checkout": checkout, "assertion_scope": "retained bounded scalar/record semantics and resources",
        "debug_information_enabled_for_retained_claim": False,
        "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
        "source_policy_input_members": len(PREDECESSOR_POLICIES),
        "predecessor_compiler_input_members": len(parent),
        "owned_vector_pins_provisional": PINS_PROVISIONAL,
        "owned_vector_observer_pins_provisional": OWNED_VECTOR_OBSERVER_PROVISIONAL,
        "owned_vector_observer_adapter": observer.descriptor(),
        "owned_vector_observer_adapter_controller": identity(OWNED_VECTOR_OBSERVER_ADAPTER, observer_body),
        "owned_vector_reviewed_source_head": REVIEWED_SOURCE_HEAD,
        "owned_vector_controller": identity("scripts/verify_owned_record_vector_compatibility.py",
                                          regular(ROOT, "scripts/verify_owned_record_vector_compatibility.py")),
        "retained_protected_inputs": {name: identity(name, parent[name]) for name in sorted(UNCHANGED_INPUTS)},
        "owned_vector_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                        for name, (length, digest) in sorted(REVIEWED_INPUTS.items())],
        "owned_vector_compiled_source_assets": [
            {**identity(name, inputs[name]), "included_by": SOURCE_ASSET_INCLUDE_BINDINGS[name][0],
             "relative_include": SOURCE_ASSET_INCLUDE_BINDINGS[name][1],
             "include_occurrences": SOURCE_ASSET_INCLUDE_BINDINGS[name][3]}
            for name in sorted(OWNED_VECTOR_SOURCE_ASSETS)],
        "owned_vector_changes": [{"path": name, "before": identity(name, parent[name]),
                                 "after": identity(name, inputs[name])}
                                for name in sorted(OWNED_VECTOR_CHANGED)],
        "owned_vector_directive_inventory": [
            {"path": name, "test_named_module": name in TEST_NAMED_INPUTS,
             "before": directive_manifest(source_directives(name, parent[name])) if name in parent else None,
             "after": directive_manifest(REVIEWED_DIRECTIVES[name])}
            for name in sorted(REVIEWED_INPUTS)],
        "changes": [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, value)}
                    for name, value in sorted(inputs.items()) if name in baseline and value != baseline[name]],
        "additions": sorted(PARENT_ADDED),
        "unchanged_baseline_inputs": sum(inputs[name] == value for name, value in baseline.items()),
        "files": [identity(name, value) for name, value in sorted(inputs.items())],
        "owned_record_vector_semantics_qualified_by_this_gate": False,
        "owned_record_vector_source_execution_qualified_by_this_gate": False,
        "owned_record_vector_native_semantics_qualified_by_this_gate": False,
        "owned_record_vector_debug_semantics_qualified_by_this_gate": False,
        "general_heap_collections_qualified_by_this_gate": False,
        "g2_complete_qualified_by_this_gate": False,
    })
    require(all(value is False for key, value in result.items() if key.endswith("qualified_by_this_gate")),
            "unexpected new qualification claim")
    return result


@contextmanager
def bound_registry():
    """Bind admission and the explicit observer successor at existing seams.

    Original runner, assembly, selectors, comparators, verification and old
    control functions stay identical. Preparation delegates without copying its
    returned context, so additional controls use the actual isolated binaries.
    """
    context = {}

    def prepare(original, output, captured, adapter, index_adapter, parser_adapter):
        require(not context, "owned-vector observer preparation repeated")
        prepared = prepare_current_observer(original, output, captured, adapter, index_adapter, parser_adapter)
        context.update(original=original, output=output, selected=prepared[0],
                       seam=prepared[1], adapter=adapter, prepared=prepared)
        return prepared

    bindings = {
        "PREDECESSOR": PREDECESSOR,
        "PREDECESSOR_ADDED": PREDECESSOR_ADDED,
        "PREDECESSOR_POLICIES": PREDECESSOR_POLICIES,
        "PINS_PROVISIONAL": PINS_PROVISIONAL,
        "capture_sources": capture_sources,
        "current_manifest": current_manifest,
        "OBSERVER_ADAPTER": OWNED_VECTOR_OBSERVER_ADAPTER,
        "authenticate_observer_adapter": authenticate_owned_vector_observer_adapter,
        "prepare_current_observer": prepare,
    }
    before = {name: getattr(library, name) for name in bindings}
    require(library.run is RETAINED_RUN, "retained runner was replaced")
    try:
        for name, value in bindings.items():
            setattr(library, name, value)
        yield context
        require(library.run is RETAINED_RUN, "retained runner changed")
    finally:
        for name, value in before.items():
            setattr(library, name, value)


def run_owned_vector_observer_controls(repo, output, captured, seam, original, adapter, previous_executions):
    """Append three exclusions per existing binary; no rebuild or old selector change."""
    require(type(previous_executions) is int and previous_executions > 0,
            "missing inherited observer execution census")
    run = output / "unit2/run"
    controls = output / "owned-record-vector-observer-controls"
    controls.mkdir()
    protocol_body = regular(Path(seam["resource_package_root"]), "protocol.py")
    require(protocol_body == captured["historical_bytes"]["protocol.py"],
            "owned-vector observer control protocol changed")
    protocol = types.ModuleType("_owned_vector_observer_control_protocol")
    exec(compile(protocol_body, "protocol.py", "exec"), protocol.__dict__)
    receipts = []
    for profile in ("debug", "release"):
        original.assert_unchanged(repo, captured)
        receipt_body = (run / (profile + "-receipt.json")).read_bytes()
        receipt = json.loads(receipt_body)
        binary = Path(receipt["binary"])
        target = run / "target"
        require(binary.is_absolute() and target in binary.parents and not binary.is_symlink()
                and binary.resolve(strict=True).is_relative_to(target.resolve(strict=True)),
                "owned-vector observer control binary outside isolated build")
        require(sha(binary.read_bytes()) == receipt["binary_sha256"],
                "owned-vector observer control binary changed")
        assembly_body = (run / "assembly.json").read_bytes()
        original.check_entries(run / "source", json.loads(assembly_body)["files"], exact=True)
        verify_current_observer(output, adapter, captured["observer"])
        commands, streams = [], {}
        for label, filters in (("list", ["--list", adapter.OWNED_RECORD_VECTOR_CONTROL_FILTER]),
                              ("run", [adapter.OWNED_RECORD_VECTOR_CONTROL_FILTER,
                                       "--test-threads=1", "--color", "never"])):
            argv = [str(binary), *filters]
            out, err = controls / (profile + "-" + label + ".stdout"), controls / (profile + "-" + label + ".stderr")
            with out.open("xb") as stdout, err.open("xb") as stderr:
                completed = library.subprocess.run(argv, cwd=run / "source", stdout=stdout, stderr=stderr, timeout=60)
            commands.append({"argv": argv, "exit_status": completed.returncode,
                             "stdout": identity(out.name, out.read_bytes()),
                             "stderr": identity(err.name, err.read_bytes())})
            (controls / (profile + "-commands.json")).write_bytes(encoded(commands))
            require(completed.returncode == 0 and err.read_bytes() == b"",
                    "owned-vector observer controls failed; see raw streams")
            streams[label] = out.read_text(encoding="utf-8")
        names = list(adapter.OWNED_RECORD_VECTOR_CONTROL_NAMES)
        protocol.rust_listing(streams["list"], names)
        result = protocol.rust_success(streams["run"], names)
        require(sha(binary.read_bytes()) == receipt["binary_sha256"],
                "owned-vector observer control binary changed")
        require((run / "assembly.json").read_bytes() == assembly_body
                and (run / (profile + "-receipt.json")).read_bytes() == receipt_body,
                "owned-vector observer control source binding changed")
        original.check_entries(run / "source", json.loads(assembly_body)["files"], exact=True)
        verify_current_observer(output, adapter, captured["observer"])
        original.assert_unchanged(repo, captured)
        record = {"status": "passed", "profile": profile, "tests": names,
                  "nominal_ordinals_per_control": 3, "result": result,
                  "binary": identity(str(binary), binary.read_bytes()),
                  "source_inputs_sha256": original.CURRENT_SOURCE_SHA,
                  "commands": identity(profile + "-commands.json", (controls / (profile + "-commands.json")).read_bytes())}
        path = controls / (profile + "-receipt.json")
        path.write_bytes(encoded(record))
        receipts.append(identity(str(path.relative_to(output)), path.read_bytes()))
    original.verify_unit2_result(output, captured, seam, False)
    count = len(adapter.OWNED_RECORD_VECTOR_CONTROL_NAMES)
    require(count == 3, "wrong owned-vector observer control count")
    return {"owned_record_vector_observer_control_tests_per_profile": count,
            "owned_record_vector_observer_denials_per_profile": count * 3,
            "owned_record_vector_observer_control_receipts": receipts,
            "inherited_test_function_executions": previous_executions,
            "test_function_executions": previous_executions + 2 * count}


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(repo == ROOT and Path(__file__).resolve() == repo / "scripts/verify_owned_record_vector_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents and not output.exists(), "output must be new outside repository")
    output.mkdir(parents=True)
    controller = regular(repo, "scripts/verify_owned_record_vector_compatibility.py")
    report = {"schema": "oxid-owned-record-vector-retained-wrapper-v1", "status": "running",
              "scope": "delegated unchanged inherited default-emission semantic/resource replay",
              "semantic_pass": False, "compiler_executions": 0,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_owned_record_vector_compatibility.py", controller),
              "retained_controller": identity(PREDECESSOR_POLICY, regular(repo, PREDECESSOR_POLICY)),
              "source_policy_parent": PREDECESSOR, "inherited_input_members": 527,
              "reviewed_source_head": REVIEWED_SOURCE_HEAD, "pins_provisional": PINS_PROVISIONAL,
              "observer_pins_provisional": OWNED_VECTOR_OBSERVER_PROVISIONAL,
              "new_feature_qualification": False, "platform_qualification": False}
    try:
        require(args.prepare_only or not PINS_PROVISIONAL,
                "provisional owned-vector pins permit prepare-only admission, not qualification")
        require(args.prepare_only or not OWNED_VECTOR_OBSERVER_PROVISIONAL,
                "provisional owned-vector observer pins permit prepare-only admission, not qualification")
        delegated = argparse.Namespace(**vars(args))
        delegated.output = output / "retained"
        with bound_registry() as context:
            status = RETAINED_RUN(delegated)
            require(status == 0, "retained controller refused; inspect retained/failure.json and raw evidence")
            artifact = "prepared.json" if args.prepare_only else "result.json"
            raw = regular(output / "retained", artifact)
            retained = json.loads(raw)
            require(retained.get("status") == ("prepared" if args.prepare_only else "passed")
                    and retained.get("semantic_pass") is (not args.prepare_only),
                    "retained result does not establish the requested replay")
            require(retained.get("current_input_members") == 572
                    and retained.get("controller") == identity(PREDECESSOR_POLICY, regular(repo, PREDECESSOR_POLICY))
                    and retained.get("historical_results_apply_to_current") is False,
                    "retained result lacks the exact new source/materialization binding")
            require(context and context["output"] == delegated.output,
                    "missing real owned-vector observer preparation context")
            report["owned_vector_observer_adapter"] = context["adapter"].descriptor()
            report["owned_vector_observer_adapter_controller"] = identity(
                OWNED_VECTOR_OBSERVER_ADAPTER, regular(repo, OWNED_VECTOR_OBSERVER_ADAPTER))
            if not args.prepare_only:
                report.update(run_owned_vector_observer_controls(
                    repo, delegated.output, context["selected"], context["seam"],
                    context["original"], context["adapter"], retained["test_function_executions"]))
        require(regular(repo, "scripts/verify_owned_record_vector_compatibility.py") == controller,
                "wrapper controller changed during qualification")
        report.update(status="prepared" if args.prepare_only else "passed",
                      semantic_pass=retained["semantic_pass"],
                      compiler_executions=retained["compiler_executions"],
                      retained_artifact=identity("retained/" + artifact, raw),
                      retained_source=retained["current_source"])
        report["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        (output / artifact).write_bytes(encoded(report))
        print(json.dumps({"status": report["status"], "semantic_pass": report["semantic_pass"],
                          "output": str(output)}))
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
