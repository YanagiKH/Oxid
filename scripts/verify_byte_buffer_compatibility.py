#!/usr/bin/env python3
"""Check scalar/record compatibility after adding growable byte buffers.

Only exact reviewed compiler/runtime inputs may differ from the pinned byte-field
parent. Historical assertions and comparators remain exact. A separately pinned
observer successor explicitly rejects ByteBuf; its old observations remain exact.
Buffer, cleanup, range and I/O semantics have separate source/native/application gates.
"""
import argparse
import copy
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import sys
import tarfile
import types

BASELINE = "9ff8ef90a26f003f4616838dfc186fa3bfb6845c"
BASELINE_SOURCE_SHA = "d33ecc4e41975a062790540ba9e9a270d7c6043a31f8f4b9a8d7a044c79530b0"
RETAINED_RUNNER_SHA = "584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76"
BINDING = "tests/fixtures/typed_project_source_binding"
RETAINED = "tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py"
OBSERVER_ADAPTER = "scripts/byte_buffer_observer.py"
OBSERVER_ADAPTER_BYTES = 6120
OBSERVER_ADAPTER_SHA = "46063f42b5e169b0d7536bedbcbf5df57e9b1c56d19e0c45bffd7e1522bfeccf"
ORCHESTRATION_CHANGED = frozenset(("src/cli.rs", "src/main.rs", "src/frontend/mod.rs",
                                  "src/frontend/driver.rs", "src/frontend/native.rs"))
PACKAGE_CHANGED = frozenset(("src/frontend/project.rs", "src/frontend/declaration_index.rs",
                            "src/frontend/declaration_index/sealed.rs",
                            "src/frontend/declaration_index/source_owner.rs",
                            "src/frontend/oir/project.rs", "src/frontend/oir/source.rs",
                            "src/frontend/oir/source/sealed.rs"))
NATIVE_PRESENTATION_CHANGED = frozenset(("src/frontend/oir/native.rs",
                                        "src/frontend/oir/owned/native.rs",
                                        "src/frontend/oir/source/sealed.rs",
                                        "src/frontend/oir/owned/source/program.rs"))
PROJECT_CHANGED = ORCHESTRATION_CHANGED | PACKAGE_CHANGED | NATIVE_PRESENTATION_CHANGED
PROJECT_ADDED = frozenset(("src/project_cli.rs", "src/project_cli/workspace.rs", "src/project_cli/package.rs",
                   "src/project_package.rs", "src/project_package/archive.rs", "src/project_lock.rs", "src/doctor_cli.rs",
                   "src/frontend/native_cache.rs", "src/frontend/native/doctor.rs", "src/frontend/project/packages.rs",
                   "src/frontend/project/packages/tests.rs", "src/frontend/build_config.rs"))
PREDECESSOR = "158f5bf670a4760be585afb104599f9e8a6c4a9e"
PREDECESSOR_POLICY_SHA = "ee3f22bc9666331bd39ec043679a70ace45971b331af85fd57b7e0203b6adb77"
PREDECESSOR_POLICIES = {
    'scripts/verify_project_workflow_current.py': '76d4df296015df6ca7b5b46509b778e17d23c922fd80df4b4ed3d182d81356c6',
    'scripts/verify_enum_payload_compatibility.py': '19cee78fc3b4908a913a5442a86dcc7dbb373ba4b733166db26097be22f2d03b',
    'scripts/verify_byte_field_compatibility.py': 'ee3f22bc9666331bd39ec043679a70ace45971b331af85fd57b7e0203b6adb77',
}
OWNED_ENUM_CHANGED = frozenset((
    'src/frontend/ast.rs',
    'src/frontend/declaration_index/enum_views.rs',
    'src/frontend/format/ast_tests.rs',
    'src/frontend/oir/owned/enum_native_tests.rs',
    'src/frontend/oir/owned/execute.rs',
    'src/frontend/oir/owned/flow.rs',
    'src/frontend/oir/owned/mod.rs',
    'src/frontend/oir/owned/native.rs',
    'src/frontend/oir/owned/plan.rs',
    'src/frontend/oir/owned/shape.rs',
    'src/frontend/oir/owned/source/array_pipeline_rows.rs',
    'src/frontend/oir/owned/source/association.rs',
    'src/frontend/oir/owned/source/byte_storage_tests.rs',
    'src/frontend/oir/owned/source/candidate_adapter.rs',
    'src/frontend/oir/owned/source/candidate_native.rs',
    'src/frontend/oir/owned/source/hir_budget.rs',
    'src/frontend/oir/owned/source/lower.rs',
    'src/frontend/oir/owned/source/mod.rs',
    'src/frontend/oir/owned/source/resolve.rs',
    'src/frontend/oir/owned/source/typeck.rs',
    'src/frontend/oir/owned_types.rs',
    'src/frontend/oir/owned_types/byte_storage_tests.rs',
    'src/frontend/oir/owned_types/enums.rs',
    'src/frontend/parser/enum_syntax_tests.rs',
    'src/frontend/parser/enums.rs',
))
OWNED_ENUM_ADDED = frozenset((
    'src/frontend/oir/owned/owned_enum_tests.rs',
    'src/frontend/oir/owned/source/owned_enum_resources.rs',
    'src/frontend/oir/owned_types/owned_enum_tests.rs',
))
BYTE_FIELD_CHANGED = frozenset((
    'src/frontend/oir/owned/byte_storage_codec_tests.rs',
    'src/frontend/oir/owned/byte_storage_native_tests.rs',
    'src/frontend/oir/owned/source/byte_storage_native_fixture.rs',
    'src/frontend/oir/owned/source/byte_storage_raw_tests.rs',
    'src/frontend/oir/owned/source/byte_storage_type_tests.rs',
    'src/frontend/oir/owned/source/resolve.rs',
    'src/frontend/oir/owned/source/u8_tests.rs',
    'src/frontend/oir/owned_types.rs',
    'src/frontend/oir/owned_types/byte_storage_tests.rs',
    'src/frontend/oir/owned_types/u8_tests.rs',
))
BYTE_BUFFER_CHANGED = frozenset((
    'src/frontend/ast.rs',
    'src/frontend/builtin_catalog.rs',
    'src/frontend/declaration_index.rs',
    'src/frontend/declaration_index/builtin_tests.rs',
    'src/frontend/declaration_index/enum_views.rs',
    'src/frontend/declaration_index/sealed.rs',
    'src/frontend/declaration_index/source_owner.rs',
    'src/frontend/format.rs',
    'src/frontend/format/ast_tests.rs',
    'src/frontend/lexer.rs',
    'src/frontend/native.rs',
    'src/frontend/oir/owned/array_native_resource_tests.rs',
    'src/frontend/oir/owned/budget.rs',
    'src/frontend/oir/owned/builtin_output_fixtures.rs',
    'src/frontend/oir/owned/builtin_output_reference_tests.rs',
    'src/frontend/oir/owned/builtins.rs',
    'src/frontend/oir/owned/byte_storage_codec_tests.rs',
    'src/frontend/oir/owned/cfg.rs',
    'src/frontend/oir/owned/composition_native_tests.rs',
    'src/frontend/oir/owned/execute.rs',
    'src/frontend/oir/owned/execute_tests.rs',
    'src/frontend/oir/owned/flow.rs',
    'src/frontend/oir/owned/mod.rs',
    'src/frontend/oir/owned/native.rs',
    'src/frontend/oir/owned/native_storage.rs',
    'src/frontend/oir/owned/plan.rs',
    'src/frontend/oir/owned/reviewer_array_reference_tests.rs',
    'src/frontend/oir/owned/reviewer_reference_tests.rs',
    'src/frontend/oir/owned/shape.rs',
    'src/frontend/oir/owned/source/array_pipeline_rows.rs',
    'src/frontend/oir/owned/source/association.rs',
    'src/frontend/oir/owned/source/budget.rs',
    'src/frontend/oir/owned/source/candidate_adapter.rs',
    'src/frontend/oir/owned/source/candidate_native.rs',
    'src/frontend/oir/owned/source/hir.rs',
    'src/frontend/oir/owned/source/hir_budget.rs',
    'src/frontend/oir/owned/source/lower.rs',
    'src/frontend/oir/owned/source/mod.rs',
    'src/frontend/oir/owned/source/program.rs',
    'src/frontend/oir/owned/source/resolve.rs',
    'src/frontend/oir/owned/source/type_storage.rs',
    'src/frontend/oir/owned/source/typeck.rs',
    'src/frontend/oir/owned/verified.rs',
    'src/frontend/oir/owned_types.rs',
    'src/frontend/oir/owned_types/composition_tests.rs',
    'src/frontend/oir/owned_types/enums.rs',
    'src/frontend/oir/source/sealed.rs',
    'src/frontend/parser.rs',
    'src/frontend/parser/arrays.rs',
))
BYTE_BUFFER_ADDED = frozenset((
    'native/experimental_bytebuf.c',
    'native/experimental_bytebuf.h',
    'native/typed_bytebuf.c',
    'src/frontend/declaration_index/experimental_tests.rs',
    'src/frontend/experimental_builtins.rs',
    'src/frontend/oir/owned/binary_io.rs',
    'src/frontend/oir/owned/byte_buffer.rs',
    'src/frontend/oir/owned/byte_buffer/tests.rs',
    'src/frontend/oir/owned/execute_experimental.rs',
    'src/frontend/oir/owned/execute_io_delivery.rs',
    'src/frontend/oir/owned/execute_resource_tests.rs',
    'src/frontend/oir/owned/execute_resources.rs',
    'src/frontend/oir/owned/experimental_descriptors.rs',
    'src/frontend/oir/owned/native_experimental.rs',
    'src/frontend/oir/owned/native_resources.rs',
    'src/frontend/oir/owned/resource_gate_tests.rs',
    'src/frontend/oir/owned/source/experimental_lower.rs',
    'src/frontend/oir/owned/source/experimental_source_tests.rs',
    'src/frontend/oir/owned/source/io_delivery_source_tests.rs',
    'src/frontend/oir/owned/source/resource_native_tests.rs',
    'src/frontend/oir/owned_types/resources.rs',
))
PARENT_ADDED = PROJECT_ADDED | OWNED_ENUM_ADDED
BYTE_BUFFER_INPUTS = {
    'native/experimental_bytebuf.c': (10228, 'ab04af7ffe43dc0e4e69f2e019f632f8f05befeb121742404ecb327c50df970d'),
    'native/experimental_bytebuf.h': (2652, 'aa5313d9f41c08f7c326631d2be6cfaab2e0dc21a580b51ba937d3ea87fc6702'),
    'native/typed_bytebuf.c': (4780, 'da2f05441d299e9b00cddf195dfca433b252d3d22d5dfbc791754cb637091396'),
    'src/frontend/ast.rs': (41263, '721b1821701c93c47c86a3f3ed77c2aeba5da04272ab46cc12e613564a12921c'),
    'src/frontend/builtin_catalog.rs': (13895, '1f56e61df915ca53d3cb86bb921e9905473fbe48798f8dcd911da79f46ff46a7'),
    'src/frontend/declaration_index.rs': (74020, 'a6b84acb80960130617619558dd6df94342defae60add904e63d83391c0e549c'),
    'src/frontend/declaration_index/builtin_tests.rs': (79640, '33db37cac972c7387fb02edc858bdbe586350a327bad5a501d75b752b5ee1660'),
    'src/frontend/declaration_index/enum_views.rs': (10293, 'bcb9cf93b657c19a3ce4703f7b9bc72bd23d3b28f83838d791d9c38ac35e1c53'),
    'src/frontend/declaration_index/experimental_tests.rs': (2959, 'a4ddeac2904f0a0c800212298a0d58833767579784545c78d26db93492399d8b'),
    'src/frontend/declaration_index/sealed.rs': (81935, 'f6c47635c271d8d52c30a737da7dc9a3d622db3a82675049efe9028257678d40'),
    'src/frontend/declaration_index/source_owner.rs': (22907, '2b572aa97c80df710e820453fbbc480931177da7a0e6a311e6121e04cf718319'),
    'src/frontend/experimental_builtins.rs': (11752, '8274b19e04ead5acf0c131ab634a1912a64f6f1fd6cf804ca720b8b523382fb5'),
    'src/frontend/format.rs': (49189, '2962224a97b9a1f38238ef1e51805bd988afd0e3c9f3fe5195b28e61615890b9'),
    'src/frontend/format/ast_tests.rs': (64896, '131b1d253ad164abe6853081003edc0855bd88b23dff9f246d64576d4c9afa66'),
    'src/frontend/lexer.rs': (35626, 'fe32aa3190308dbaf9f9ab7ce285df352d29998531285e80238b043fbd502100'),
    'src/frontend/native.rs': (17164, '6c1235e66cde3888d4f830f5a1663a372a5288242fd4102b6cd17a926670d2e4'),
    'src/frontend/oir/owned/array_native_resource_tests.rs': (52860, 'b39d9e1b5bf217b79ce5e5d047d3437503b2901e74de34f6277b2e15733d478a'),
    'src/frontend/oir/owned/binary_io.rs': (13032, '1299b903428f68c805bdecf8209f71d1796381b77c41596839ad6ff112c24a72'),
    'src/frontend/oir/owned/budget.rs': (18974, 'b2fe847a7c087415c49a13f9d4b55f9eb726284b6ade1ca58366ae7dc88d378c'),
    'src/frontend/oir/owned/builtin_output_fixtures.rs': (9876, '1b7842e51e1782af1c082569605cff557c20f71731725857d025125c83f6220e'),
    'src/frontend/oir/owned/builtin_output_reference_tests.rs': (8444, 'a4651ba632bb47df09302c583c4972ee1c728e84c3815bc2d11997bb941bbc7d'),
    'src/frontend/oir/owned/builtins.rs': (12974, '770bb5f0001e2370de1bb22d38572d1ef40b2ed82e66553d017f343cb850b522'),
    'src/frontend/oir/owned/byte_buffer.rs': (20036, '87680dba3465c524e176dc8e398b26d662ec115e78a9a62c18f0aa3a67647612'),
    'src/frontend/oir/owned/byte_buffer/tests.rs': (9599, 'ad4fe6988c4fc485cb095119a33cc73e3d02f0690fb736c1c9728eeeeaa5d9c6'),
    'src/frontend/oir/owned/byte_storage_codec_tests.rs': (33637, '3c35950b7eda8052bec1934f1490bda2c59fe48c1dc936ba02f1c06449e10bfa'),
    'src/frontend/oir/owned/cfg.rs': (7831, 'c600bfbbd4b4f97ed6cc5749ae471e8b81ea0c82336a77663dfb30a127044e77'),
    'src/frontend/oir/owned/composition_native_tests.rs': (23871, '257c160804e353e2a2f039975410b2e1e0b3bf25100d056f105e928899377339'),
    'src/frontend/oir/owned/execute.rs': (175063, '97e2b3371a882728e1cb459bc472f4d4bc83acbe73d0afa4628249d36436f9a5'),
    'src/frontend/oir/owned/execute_experimental.rs': (22634, '9f78389e839ec098abac33275f47beb54b8d4a5bc8c75eeb9b92894917ea5472'),
    'src/frontend/oir/owned/execute_io_delivery.rs': (11218, '4c4f5b7c62e93e1a0f31bc2eee42bb084e6ec51ff4ded650300d4f020100e51a'),
    'src/frontend/oir/owned/execute_resource_tests.rs': (5421, '609a132691161a6aa1b94acb639bece2468db5c1e2279856aa8a2f3626124a03'),
    'src/frontend/oir/owned/execute_resources.rs': (3403, '551fc4ed7596dde06f63376dddf3c9a94447f705d5fbf9b72fec52abd8e9c4a1'),
    'src/frontend/oir/owned/execute_tests.rs': (41806, 'ab5dd564969a2e32bf202b94ee7cba9f113dfe53665cf294ad2801d66d2a5f5a'),
    'src/frontend/oir/owned/experimental_descriptors.rs': (6355, '03001a1de55d6063cc94253306c278d4a5151ecd722177e95e34d4aaebd97f5e'),
    'src/frontend/oir/owned/flow.rs': (45913, 'ed974ccb1b4dc753c0d6a8d595b8018f26ff963032075bb5edcede3d54b35ee8'),
    'src/frontend/oir/owned/mod.rs': (14835, '89a9a3cd9db87c95c53333b51f3df23e1d7670ff99a89580875f554495bf893d'),
    'src/frontend/oir/owned/native.rs': (210243, '582aceaf20d1657b20fb2342b3b49c8f265375e11a93c2e9896cfea3d6c07ce9'),
    'src/frontend/oir/owned/native_experimental.rs': (8909, 'e31f1edb1bdd461ef40d8c9d45ffc4227c1b37cdc244847aa465b4ff84dfaa50'),
    'src/frontend/oir/owned/native_resources.rs': (4605, '75b1f00520ebaf88e3089c7d147efa38581c8e94b926933482477b62accb50ac'),
    'src/frontend/oir/owned/native_storage.rs': (18394, '4e1162ce9356e95bc88b722cfcdb72b0229ec87b377d0adc054c9570504ee08b'),
    'src/frontend/oir/owned/plan.rs': (24538, 'f85c35e4cc6db88d92bf4d72eb74000e92647c86b19fde68f1620942e647220a'),
    'src/frontend/oir/owned/resource_gate_tests.rs': (2616, 'dd8a49cb5fdb8b49098fa81a3d0a04614cea85af57feef61ca6df55f1e5926d9'),
    'src/frontend/oir/owned/reviewer_array_reference_tests.rs': (65157, '9ae5dd326379520bace4869189a904c3fbdb23526288b437f120b36c167888fb'),
    'src/frontend/oir/owned/reviewer_reference_tests.rs': (71722, '8a8aa8cc7c7b5a0fffa34ba774272cb043211dd06c886b9b6940adfdb1e210ee'),
    'src/frontend/oir/owned/shape.rs': (54832, 'aad33959d3dae68b227ccb3877e35df785d1d021200cb898b646ff4d7b14490a'),
    'src/frontend/oir/owned/source/array_pipeline_rows.rs': (53584, '2afe86f0c771d17727600aed34b7a22595d78c975eb847cadb59efd747657305'),
    'src/frontend/oir/owned/source/association.rs': (59601, '9247f6ebbbe4d74bc458e2b4552199d890ad684aea8a4ab352e0b9eb6a76ed6f'),
    'src/frontend/oir/owned/source/budget.rs': (16050, '8b6dec5a73510dd0b2a684a2589f727eff260ef0fcb8844804cd73411da79482'),
    'src/frontend/oir/owned/source/candidate_adapter.rs': (42035, '792380ac00a0fe1d0cde6881a90dd6b43f9c2a73385de1628f9bd36eb9b8acbb'),
    'src/frontend/oir/owned/source/candidate_native.rs': (32342, '3c4c00383aaa164e71e540407fd99bec25630577fc53e471c8073c90f05e3648'),
    'src/frontend/oir/owned/source/experimental_lower.rs': (11234, 'ac019ec6da52684d6655b56b69d31e4f3f4775a9e12e1f00e38b99e9aaf9249f'),
    'src/frontend/oir/owned/source/experimental_source_tests.rs': (22049, 'ef94222f7df852ea5363688141ba32a4148cda9aba19483bfddc3b441287ba76'),
    'src/frontend/oir/owned/source/hir.rs': (6195, 'd646e5c44afe95da4649bbfa0cb5f79b4be2b8a59648bc2bec2ea1d29c566306'),
    'src/frontend/oir/owned/source/hir_budget.rs': (55375, '8b9edf16cdf3648a13e62f35d3d1ba056f07a6df7d6d2e828c1713d8ca0bc08d'),
    'src/frontend/oir/owned/source/io_delivery_source_tests.rs': (13530, '3ec849f66b5590af9f76027fec8627bacfe68536a895b9d0b8d4169bf555aa1f'),
    'src/frontend/oir/owned/source/lower.rs': (108552, '6dd046ed9789a8712c741a3a644815536a21b8044558ce9fa2f2dbf211a3eb68'),
    'src/frontend/oir/owned/source/mod.rs': (1907, '058389ae050bf9601c07cb5ec1b0070831983d914985f23be8cb2a82e8af0174'),
    'src/frontend/oir/owned/source/program.rs': (34083, '79430af93a3f3266594f5e56ae2f82356dfe949034a8eaea3fe44e08d6fe33a4'),
    'src/frontend/oir/owned/source/resolve.rs': (138124, '2868116fe2e32a5beed9f1a14df2c4a47905d6b20d01703ad83b9e1c4f1ae1b0'),
    'src/frontend/oir/owned/source/resource_native_tests.rs': (14546, '4f9200d3f4f86e5041ab092185d9cd0278153861b748b17b10521d8d98619b42'),
    'src/frontend/oir/owned/source/type_storage.rs': (68435, '49f02680a7a0fd588ea9b4c5160a38da9ed3b20548be66c0670b87faf94ab663'),
    'src/frontend/oir/owned/source/typeck.rs': (207644, '59ad8687b4116ae02a8f46a05a32315ba6149bec0d8e9eb4a3256bfe4d170abd'),
    'src/frontend/oir/owned/verified.rs': (13364, '14eacc852c2c4cf35955a8256da0335f9181271ac08cdaa8b93216205b6df1d5'),
    'src/frontend/oir/owned_types.rs': (76908, '2665d7a14e2d6d874959c1605c2722a6e47900d0c3fdca36d6536ccb93ccdea2'),
    'src/frontend/oir/owned_types/composition_tests.rs': (8682, 'd4b7a316001028a1c3157eec98c7ead1c07e81168e8f1ee96da964da7eb0899f'),
    'src/frontend/oir/owned_types/enums.rs': (54395, 'e1463b463ceab57fea84a4d5d3d31f52a3bec14b6b32c4e8b1c9b2cc6825583e'),
    'src/frontend/oir/owned_types/resources.rs': (22000, 'e8223796819de72c9ea08425a6bbc2e8786e8f679a69f25f2130102cbec63164'),
    'src/frontend/oir/source/sealed.rs': (18940, '579dde40c9efe23f133a4eec10af2e1218bf188eb1c61a41d742ed5672719638'),
    'src/frontend/parser.rs': (75289, '82ad131699c8ea2db2cf23792f594147b4da465fba108f4a403f3c2a2e721563'),
    'src/frontend/parser/arrays.rs': (9576, 'e77fd6bc8ae7544a93562fc4ff283d1229edd7dc1e174229d50e8289464b1465'),
}

REVIEWED_DIRECTIVES = {
    'src/frontend/builtin_catalog.rs': ((), (b'#[path = "experimental_builtins.rs"]',)),
    'src/frontend/declaration_index/builtin_tests.rs': ((), (b'#[path = "experimental_tests.rs"]',)),
    'src/frontend/native.rs': ((b'include_str!("../../native/typed_preview.c")', b'include_str!("../../native/typed_bytebuf.c")', b'include_str!("../../native/experimental_bytebuf.c")', b'include_str!("../../native/experimental_bytebuf.h")'), ()),
    'src/frontend/oir/owned/execute.rs': ((), (b'#[path = "execute_experimental.rs"]', b'#[path = "execute_resources.rs"]', b'#[path = "array_observe.rs"]', b'#[path = "execute_tests.rs"]', b'#[path = "builtin_output_reference_tests.rs"]', b'#[path = "byte_storage_codec_tests.rs"]')),
    'src/frontend/oir/owned/execute_experimental.rs': ((), (b'#[path = "execute_io_delivery.rs"]',)),
    'src/frontend/oir/owned/execute_resources.rs': ((), (b'#[path = "execute_resource_tests.rs"]',)),
    'src/frontend/oir/owned/native.rs': ((), (b'#[path = "native_experimental.rs"]', b'#[path = "native_resources.rs"]', b'#[path = "builtin_input_native_tests.rs"]', b'#[path = "builtin_output_native_tests.rs"]', b'#[path = "native_tests.rs"]', b'#[path = "native_inventory_admission_tests.rs"]', b'#[path = "byte_storage_native_tests.rs"]')),
    'src/frontend/oir/owned/source/experimental_source_tests.rs': ((), (b'#[path = "io_delivery_source_tests.rs"]',)),
    'src/frontend/oir/owned/source/program.rs': ((), (b'#[path = "experimental_source_tests.rs"]', b'#[path = "candidate_adapter.rs"]')),
}

CHANGED = PROJECT_CHANGED | OWNED_ENUM_CHANGED | BYTE_FIELD_CHANGED | BYTE_BUFFER_CHANGED
ADDED = PARENT_ADDED | BYTE_BUFFER_ADDED
INSTRUMENTED = frozenset(("src/frontend/declaration_index.rs", "src/frontend/parser.rs",
                          "src/frontend/source.rs", "src/frontend/oir/mod.rs",
                          "src/frontend/project.rs"))
# This admission scanner is deliberately conservative, not a Rust evaluator.
# Complete file pins and the literal reviewed directives admit only the listed
# new compile-time inputs, including exact embedded runtime/module expressions.
GAP = rb'(?:\s|/\*.*?\*/|//[^\n]*(?:\n|$))*'
INCLUDE_START = re.compile(rb'\binclude(?:_str|_bytes)?' + GAP + rb'!', re.S)
INCLUDE = re.compile(INCLUDE_START.pattern + GAP
                     + rb'(?:\([^;]*?\)|\{[^;]*?\}|\[[^;]*?\])', re.S)
PATH_ATTRIBUTE = re.compile(rb'#' + GAP + rb'!?' + GAP + rb'\[[^\]]*\bpath\b[^\]]*\]', re.S)



def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(body):
    return hashlib.sha256(body).hexdigest()


def encoded(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def identity(name, body):
    return {"path": name, "bytes": len(body), "sha256": sha(body)}


def directives(body):
    includes = INCLUDE.findall(body)
    require(len(includes) == len(INCLUDE_START.findall(body)), "incomplete compile-time include")
    return includes, PATH_ATTRIBUTE.findall(body)



def regular(root, name):
    path = root / name
    require(not path.is_symlink() and all(not p.is_symlink() for p in path.parents),
            "symlink source: " + name)
    before = path.stat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
            and not before.st_mode & 0o111 and before.st_size <= 2 * 1024 * 1024,
            "nonregular, executable, linked or oversized source: " + name)
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, f) for f in fields)
            == tuple(getattr(path.stat(), f) for f in fields) and len(body) == before.st_size,
            "source changed during read: " + name)
    return body


def capture_sources(repo, baseline, predecessor):
    """Admit only the exact reviewed buffer delta; every other byte is parental.

    Historical predicates still qualify scalar/record compatibility. New heap,
    ownership, range and binary-I/O semantics require their separate checks.
    """
    actual, directories = set(), set()
    for part in ("src", "native"):
        require((repo / part).is_dir() and not (repo / part).is_symlink(), "missing source root")
        for parent, dirs, files in os.walk(repo / part, followlinks=False):
            for name in dirs + files:
                path = Path(parent) / name
                require(not path.is_symlink(), "symlink source member")
                relative = path.relative_to(repo).as_posix()
                (directories if name in dirs else actual).add(relative)
    expected = {n for n in baseline if n.startswith(("src/", "native/"))} | ADDED
    require(actual == expected, "unexpected compiler source membership")
    expected_dirs = {str(p) for n in expected for p in PurePosixPath(n).parents
                     if str(p) not in (".", "src", "native")}
    require(directories == expected_dirs, "unexpected compiler source directory")
    inputs = {name: regular(repo, name) for name in sorted(set(baseline) | ADDED)}
    for name, before in baseline.items():
        if name not in CHANGED:
            require(inputs[name] == before, "unchanged semantic/resource input changed: " + name)
    require(set(predecessor) == set(baseline) | PARENT_ADDED,
            "wrong predecessor compiler input membership")
    require(BYTE_BUFFER_CHANGED <= predecessor.keys()
            and not BYTE_BUFFER_ADDED & predecessor.keys(), "wrong buffer delta boundary")
    for name, body in predecessor.items():
        if name not in BYTE_BUFFER_CHANGED:
            require(inputs[name] == body,
                    "unreviewed change outside byte buffer boundary: " + name)
    require(set(BYTE_BUFFER_INPUTS) == BYTE_BUFFER_CHANGED | BYTE_BUFFER_ADDED,
            "wrong reviewed byte buffer input membership")
    for name, (length, digest) in BYTE_BUFFER_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "byte buffer source does not match reviewed input: " + name)
    require(REVIEWED_DIRECTIVES.keys() <= BYTE_BUFFER_INPUTS.keys(),
            "unreviewed compile-time directive boundary")
    # Literal directives are reviewed alongside exact complete-file pins. This
    # scanner is only supplementary: aliases, generated paths and macro tricks
    # cannot escape complete byte identity and the closed member roster.
    for name in CHANGED | ADDED:
        expected_directives = REVIEWED_DIRECTIVES.get(name)
        if expected_directives is None:
            expected_directives = tuple(tuple(part) for part in directives(predecessor.get(name, b"")))
        require(tuple(tuple(part) for part in directives(inputs[name])) == expected_directives,
                "byte buffer compile-time input directive changed: " + name)
    return inputs


def git(repo, *args):
    result = subprocess.run(["git", "--no-replace-objects", "-C", str(repo), *args],
                            capture_output=True, timeout=120)
    require(result.returncode == 0, "git command failed: " + " ".join(args)
            + ": " + result.stderr.decode(errors="replace")[-2000:])
    return result.stdout


def checkout_identity(repo, expected_head, allow_dirty):
    require(re.fullmatch("[0-9a-f]{40}", expected_head) is not None, "invalid expected head")
    head = git(repo, "rev-parse", "HEAD").decode().strip()
    require(head == expected_head, "checkout does not match expected head")
    status = git(repo, "status", "--porcelain=v1", "--untracked-files=all")
    require(allow_dirty or not status, "committed qualification requires a clean checkout")
    return {"head": head, "tree": git(repo, "rev-parse", "HEAD^{tree}").decode().strip(),
            "source_kind": "worktree" if status else "committed",
            "worktree_status_sha256": sha(status)}


def extract_baseline(repo, destination, fetch=False, revision=BASELINE):
    exists = subprocess.run(["git", "--no-replace-objects", "-C", str(repo),
                             "cat-file", "-e", revision + "^{commit}"],
                            capture_output=True).returncode == 0
    if not exists and fetch:
        git(repo, "fetch", "--no-tags", "--depth=1", "https://github.com/YanagiKH/Oxid.git", revision)
    require(git(repo, "rev-parse", revision + "^{commit}").decode().strip() == revision,
            "missing pinned baseline; use --fetch-baseline to fetch its exact public commit")
    require(git(repo, "rev-parse", "--show-object-format").strip() == b"sha1",
            "pinned repository requires SHA-1 Git object identities")
    expected = {}
    for row in git(repo, "ls-tree", "-rzl", "--full-tree", revision).split(b"\0"):
        if not row:
            continue
        metadata, name = row.split(b"\t", 1)
        mode, kind, object_id, length = metadata.split()
        require(mode in (b"100644", b"100755") and kind == b"blob"
                and re.fullmatch(rb"[0-9a-f]{40}", object_id) is not None and length.isdigit(),
                "nonregular pinned tree member")
        name = name.decode("utf-8")
        require(name not in expected, "duplicate pinned tree member")
        expected[name] = (int(mode[-3:], 8), object_id.decode("ascii"), int(length))
    expected_dirs = {str(p) for name in expected for p in PurePosixPath(name).parents
                     if str(p) != "."}
    destination.mkdir()
    archive = destination.with_suffix(".tar")
    with archive.open("xb") as stream:
        result = subprocess.run(["git", "--no-replace-objects", "-c", "tar.umask=0022", "-C", str(repo),
                                 "archive", "--format=tar", revision],
                                stdout=stream, stderr=subprocess.PIPE, timeout=120)
    require(result.returncode == 0, "cannot archive pinned baseline")
    seen, directories = set(), set()
    with tarfile.open(archive) as source:
        for member in source:
            name = PurePosixPath(member.name)
            require(not name.is_absolute() and ".." not in name.parts, "unsafe baseline archive path")
            path = destination.joinpath(*name.parts)
            if member.isdir():
                require(member.name in expected_dirs and member.name not in directories,
                        "unexpected pinned archive directory")
                directories.add(member.name)
                path.mkdir(parents=True, exist_ok=True)
            else:
                require(member.isfile(), "nonregular baseline archive member")
                require(member.name in expected and member.name not in seen,
                        "unexpected pinned archive member")
                mode, object_id, length = expected[member.name]
                require(member.mode & 0o7777 == mode, "pinned archive mode changed")
                require(member.size == length, "pinned archive blob length changed")
                with source.extractfile(member) as incoming:
                    body = incoming.read(length + 1)
                require(len(body) == length, "pinned archive blob read length changed")
                blob = b"blob " + str(len(body)).encode("ascii") + b"\0" + body
                require(hashlib.sha1(blob, usedforsecurity=False).hexdigest() == object_id,
                        "pinned archive blob changed: " + member.name)
                seen.add(member.name)
                path.parent.mkdir(parents=True, exist_ok=True)
                with path.open("xb") as target:
                    target.write(body)
                path.chmod(mode)
    require(seen == set(expected) and directories == expected_dirs,
            "pinned archive tree membership changed")
    archive.unlink()


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def current_manifest(inputs, baseline, checkout, predecessor):
    changes = [{"path": n, "before": identity(n, baseline[n]), "after": identity(n, b)}
               for n, b in sorted(inputs.items()) if n in baseline and b != baseline[n]]
    return {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
            "checkout": checkout, "changes": changes, "additions": sorted(ADDED),
            "assertion_scope": "retained single-package Unit2 semantics and resources",
            "package_boundary_inputs": sorted(PACKAGE_CHANGED),
            "native_presentation_inputs": sorted(NATIVE_PRESENTATION_CHANGED),
            "package_semantics_qualified_by_this_gate": False,
            "workspace_semantics_qualified_by_this_gate": False,
            "native_reproducibility_qualified_by_this_gate": False,
            "native_optimization_qualified_by_this_gate": False,
            "owned_enum_semantics_qualified_by_this_gate": False,
            "byte_field_semantics_qualified_by_this_gate": False,
            "byte_buffer_semantics_qualified_by_this_gate": False,
            "binary_io_semantics_qualified_by_this_gate": False,
            "native_heap_safety_qualified_by_this_gate": False,
            "source_policy_parent": PREDECESSOR,
            "source_policy_inputs": PREDECESSOR_POLICIES,
            "byte_buffer_boundary_inputs": sorted(BYTE_BUFFER_CHANGED),
            "byte_buffer_added_inputs": sorted(BYTE_BUFFER_ADDED),
            "byte_buffer_reviewed_inputs": [{"path": n, "bytes": size, "sha256": digest}
                                             for n, (size, digest) in sorted(BYTE_BUFFER_INPUTS.items())],
            "byte_buffer_changes": [{"path": n, "before": identity(n, predecessor[n]),
                                     "after": identity(n, inputs[n])}
                                    for n in sorted(BYTE_BUFFER_CHANGED)
                                    if inputs[n] != predecessor[n]],
            "unchanged_baseline_inputs": len(baseline) - len(changes),
            "files": [identity(n, b) for n, b in sorted(inputs.items())]}


def authenticate_policies(repo, predecessor_root):
    for policy, expected_sha in PREDECESSOR_POLICIES.items():
        before = regular(predecessor_root, policy)
        require(sha(before) == expected_sha,
                "wrong pinned predecessor source policy: " + policy)
        require(regular(repo, policy) == before,
                "predecessor source policy changed: " + policy)


def verify_materialized(output, inputs):
    source = output / "unit2/run/source"
    assembly = json.loads((output / "unit2/run/assembly.json").read_bytes())
    changes = assembly["instrumentation"]
    require(len(changes) == len(INSTRUMENTED) and {r["path"] for r in changes} == INSTRUMENTED,
            "unexpected Unit2 instrumentation scope")
    for row in changes:
        require(row["before_sha256"] == sha(inputs[row["path"]]), "wrong instrumentation input")
        require(row["after_sha256"] == sha(regular(source, row["path"])), "changed instrumentation output")
    for name, body in inputs.items():
        if name not in INSTRUMENTED:
            require(regular(source, name) == body, "materialized current input changed: " + name)


def authenticate_observer_adapter(repo):
    body = regular(repo, OBSERVER_ADAPTER)
    require(len(body) == OBSERVER_ADAPTER_BYTES and sha(body) == OBSERVER_ADAPTER_SHA,
            "changed pinned ByteBuf observer adapter")
    adapter = types.ModuleType("_byte_buffer_observer_successor")
    adapter.__file__ = str(repo / OBSERVER_ADAPTER)
    # Execute the authenticated capture, rather than reopening a mutable module
    # path or accepting a cached Python bytecode file.
    exec(compile(body, adapter.__file__, "exec"), adapter.__dict__)
    return adapter, body


def prepare_current_observer(original, output, captured, adapter):
    selected = copy.deepcopy(captured)
    selected["observer"] = adapter.adapt(captured["observer"])
    seam = original.prepare_unit2(output, selected)
    seam["observer_adapter"] = {"predecessor": seam["observer_adapter"],
                                "successor": adapter.descriptor()}
    require(regular(Path(seam["resource_package_root"]), adapter.OBSERVER) == selected["observer"],
            "prepared ByteBuf observer differs from pinned successor")
    return selected, seam


def verify_current_observer(output, adapter, expected):
    body = regular(output / "unit2/run/source", "src/frontend/oir/unit2_observer.rs")
    require(body == expected and len(body) == adapter.DERIVED_BYTES and sha(body) == adapter.DERIVED_SHA,
            "materialized ByteBuf observer differs from pinned successor")


def verify_byte_buffer_control_output(protocol, listing, execution, adapter):
    protocol.rust_listing(listing, list(adapter.CONTROL_NAMES))
    return protocol.rust_success(execution, [name + " - should panic" for name in adapter.CONTROL_NAMES])


def run_byte_buffer_observer_controls(repo, output, captured, seam, original, adapter):
    """Run three separate rejection controls without changing the old six controls."""
    run = output / "unit2/run"
    controls = output / "byte-buffer-observer-controls"
    require(not controls.exists(), "ByteBuf observer controls output exists")
    controls.mkdir()
    protocol_body = regular(Path(seam["resource_package_root"]), "protocol.py")
    require(protocol_body == captured["historical_bytes"]["protocol.py"],
            "ByteBuf observer control protocol changed")
    protocol = types.ModuleType("_byte_buffer_observer_protocol")
    protocol.__file__ = str(Path(seam["resource_package_root"]) / "protocol.py")
    exec(compile(protocol_body, protocol.__file__, "exec"), protocol.__dict__)
    receipts = []
    for profile in ("debug", "release"):
        original.assert_unchanged(repo, captured)
        source = run / "source"
        assembly_body = (run / "assembly.json").read_bytes()
        assembly = json.loads(assembly_body)
        original.check_entries(source, assembly["files"], exact=True)
        verify_current_observer(output, adapter, captured["observer"])
        receipt_body = (run / (profile + "-receipt.json")).read_bytes()
        original_receipt = json.loads(receipt_body)
        binary = Path(original_receipt["binary"])
        require(binary.is_absolute() and (run / "target") in binary.parents and not binary.is_symlink(),
                "ByteBuf observer control binary outside isolated build")
        require(sha(binary.read_bytes()) == original_receipt["binary_sha256"],
                "ByteBuf observer control binary changed")
        commands, streams = [], {}
        for label, filters in (("list", ["--list", adapter.CONTROL_FILTER]),
                               ("run", [adapter.CONTROL_FILTER, "--test-threads=1", "--color", "never"])):
            argv = [str(binary), *filters]
            started = datetime.datetime.now(datetime.timezone.utc).isoformat()
            out, err = controls / (profile + "-" + label + ".stdout"), controls / (profile + "-" + label + ".stderr")
            env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
            env.pop("PYTHONOPTIMIZE", None)
            status, timed_out = None, False
            with out.open("xb") as stdout, err.open("xb") as stderr:
                try:
                    status = subprocess.run(argv, cwd=source, env=env, stdout=stdout,
                                            stderr=stderr, timeout=60).returncode
                except subprocess.TimeoutExpired:
                    timed_out = True
            commands.append({"argv": argv, "cwd": str(source), "started_at_utc": started,
                             "finished_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                             "exit_status": status, "timed_out": timed_out,
                             "stdout": identity(out.name, out.read_bytes()),
                             "stderr": identity(err.name, err.read_bytes())})
            (controls / (profile + "-commands.json")).write_bytes(encoded(commands))
            require(status == 0 and not timed_out, "ByteBuf observer controls failed; see retained streams")
            require(sha(binary.read_bytes()) == original_receipt["binary_sha256"],
                    "ByteBuf observer control binary changed")
            streams[label] = out.read_text(encoding="utf-8")
        result = verify_byte_buffer_control_output(protocol, streams["list"], streams["run"], adapter)
        original.check_entries(source, assembly["files"], exact=True)
        verify_current_observer(output, adapter, captured["observer"])
        require((run / "assembly.json").read_bytes() == assembly_body
                and (run / (profile + "-receipt.json")).read_bytes() == receipt_body,
                "ByteBuf observer control source binding changed")
        original.assert_unchanged(repo, captured)
        record = {"schema": "oxid-byte-buffer-unit2-observer-controls-v1", "profile": profile,
                  "status": "passed", "tests": list(adapter.CONTROL_NAMES), "result": result,
                  "source_inputs_sha256": original.CURRENT_SOURCE_SHA, "observer_adapter": adapter.descriptor(),
                  "original_unit2_receipt": identity(profile + "-receipt.json", receipt_body),
                  "binary": identity(str(binary), binary.read_bytes()),
                  "assembly": identity("assembly.json", assembly_body),
                  "commands": identity(profile + "-commands.json", (controls / (profile + "-commands.json")).read_bytes())}
        path = controls / (profile + "-receipt.json")
        path.write_bytes(encoded(record))
        receipts.append(identity(str(path.relative_to(output)), path.read_bytes()))
    return {"byte_buffer_observer_adapter_version": adapter.VERSION,
            "byte_buffer_observer_control_tests_per_profile": 3,
            "byte_buffer_observer_control_receipts": receipts,
            "test_function_executions": 2 * (1 + 21 + 6 + 3)}


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained scalar/record semantics and resources on the byte-buffer successor; heap and binary-I/O semantics qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_byte_buffer_compatibility.py", Path(__file__).read_bytes())}
    aliases = dict(sys.modules)
    try:
        checkout = checkout_identity(repo, args.expected_head, args.allow_dirty)
        baseline_root = output / "baseline"
        extract_baseline(repo, baseline_root, args.fetch_baseline)
        old_source = regular(baseline_root, BINDING + "/current-source.json")
        require(sha(old_source) == BASELINE_SOURCE_SHA, "wrong historical source manifest")
        original_body = regular(baseline_root, RETAINED)
        require(sha(original_body) == RETAINED_RUNNER_SHA, "wrong retained Unit2 predicates")
        historical = load("r1_historical_source_binding", baseline_root / BINDING / "run.py")
        captured = historical.preflight(baseline_root)
        original = load("r1_retained_unit2_assertions", baseline_root / RETAINED)
        predecessor_root = output / "predecessor"
        extract_baseline(repo, predecessor_root, args.fetch_baseline, PREDECESSOR)
        authenticate_policies(repo, predecessor_root)
        observer_adapter, observer_adapter_body = authenticate_observer_adapter(repo)
        predecessor_inputs = {name: regular(predecessor_root, name)
                              for name in sorted(set(captured["inputs"]) | PARENT_ADDED)}
        inputs = capture_sources(repo, captured["inputs"], predecessor_inputs)
        manifest = current_manifest(inputs, captured["inputs"], checkout, predecessor_inputs)
        manifest_raw = encoded(manifest)
        manifest_path = output / "current-source.json"
        manifest_path.write_bytes(manifest_raw)
        report.update(checkout=checkout, current_source=identity("current-source.json", manifest_raw),
                      historical_source=identity(BINDING + "/current-source.json", old_source),
                      retained_assertions=identity(RETAINED, original_body),
                      current_input_members=len(inputs), historical_results_apply_to_current=False)
        selected, seam = prepare_current_observer(original, output, captured, observer_adapter)
        report["byte_buffer_observer_adapter"] = observer_adapter.descriptor()
        report["byte_buffer_observer_adapter_controller"] = identity(OBSERVER_ADAPTER, observer_adapter_body)
        (output / "resource-seam.json").write_bytes(encoded(seam))
        # The separate source manifest is the existing compatibility seam. Other
        # historical captures, identities, comparators and expectations stay exact.
        selected["package_bytes"]["current-source.json"] = manifest_raw

        def unchanged(actual_repo, _capture):
            require(checkout_identity(actual_repo, args.expected_head, args.allow_dirty) == checkout,
                    "checkout changed during qualification")
            authenticate_policies(actual_repo, predecessor_root)
            require(authenticate_observer_adapter(actual_repo)[1] == observer_adapter_body,
                    "ByteBuf observer adapter changed during qualification")
            require(capture_sources(actual_repo, captured["inputs"], predecessor_inputs) == inputs,
                    "current source changed during qualification")
            require(manifest_path.read_bytes() == manifest_raw, "current manifest changed")

        original.assert_unchanged = unchanged
        original.CURRENT_SOURCE_SHA = sha(manifest_raw)
        unchanged(repo, selected)
        command = [sys.executable, "-B", seam["compatibility_runner"], "--repo", str(repo),
                   "--source-manifest", str(manifest_path), "--output", str(output / "unit2"),
                   "--cargo", args.cargo, "--rustc", args.rustc]
        if args.prepare_only:
            command.append("--prepare-only")
        env = dict(os.environ, CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="2", PYTHONDONTWRITEBYTECODE="1")
        env.pop("PYTHONOPTIMIZE", None)
        report["compiler_executions"] = 0 if args.prepare_only else "see retained Cargo command evidence"
        with (output / "runner.stdout").open("xb") as stdout, (output / "runner.stderr").open("xb") as stderr:
            completed = subprocess.run(command, cwd=repo, env=env, stdout=stdout, stderr=stderr, timeout=5400)
        (output / "command.json").write_bytes(encoded({"argv": command, "exit_status": completed.returncode,
            "stdout": identity("runner.stdout", (output / "runner.stdout").read_bytes()),
            "stderr": identity("runner.stderr", (output / "runner.stderr").read_bytes())}))
        require(completed.returncode == 0, "retained Unit2 gate failed; see runner.stderr and unit2 evidence")
        verified = original.verify_unit2_result(output, selected, seam, args.prepare_only)
        verify_materialized(output, inputs)
        verify_current_observer(output, observer_adapter, selected["observer"])
        if not args.prepare_only:
            verified.update(original.run_unit2_observer_controls(repo, output, selected, seam))
            verified.update(run_byte_buffer_observer_controls(repo, output, selected, seam, original, observer_adapter))
            original.verify_unit2_result(output, selected, seam, False)
            verify_materialized(output, inputs)
            verify_current_observer(output, observer_adapter, selected["observer"])
        unchanged(repo, selected)
        report.update(verified)
        report["status"] = "prepared" if args.prepare_only else "passed"
        report["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        artifact = "prepared.json" if args.prepare_only else "result.json"
        (output / artifact).write_bytes(encoded(report))
        print(json.dumps({"status": report["status"], "semantic_pass": report["semantic_pass"],
                          "source_kind": checkout["source_kind"], "output": str(output)}))
        return 0
    except (Exception, KeyboardInterrupt) as error:
        report.update(status="failed", semantic_pass=False, error=f"{type(error).__name__}: {error}")
        (output / "failure.json").write_bytes(encoded(report))
        print(report["error"], file=sys.stderr)
        return 1
    finally:
        for name in set(sys.modules) - set(aliases):
            sys.modules.pop(name, None)
        sys.modules.update(aliases)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--fetch-baseline", action="store_true", help="fetch only the fixed public baseline commit if missing")
    parser.add_argument("--allow-dirty", action="store_true", help="record local worktree evidence; never label it committed")
    parser.add_argument("--prepare-only", action="store_true", help="exercise admission/assembly with zero compiler executions")
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
