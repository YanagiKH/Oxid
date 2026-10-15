"""Inactive source-control entry template. Derive final pins before publication.

Discovery imports only this standard-library entrypoint. The explicit suites
are authenticated and constructed only when their containing suite is run.
This draft is deliberately unable to run without the final derivation.
"""
from contextlib import contextmanager, ExitStack
import ast
import copy
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import signal
import selectors
import time
import stat
import subprocess
import sys
import tempfile
import threading
import types
import unittest
from unittest import mock

_CONFIG = {'ci_data': {'audit': 'docs/architecture/fallible-lexer-source-consumer-audit.md', 'middle': 'tests/qualification/published_formatter_current/source-proof/published-main-ci.yml', 'old_dispositions': 'tests/qualification/lexer_reservation_current/direct-consumer-dispositions.json', 'oldest': 'scripts/predecessors/byte_storage_ci_v1/.github/workflows/ci.yml'}, 'historical_package_manifest': 'tests/qualification/published_formatter_current/predecessor-v1/tests/fixtures/typed_project_source_binding/package-manifest.json', 'input_roster': [{'path': 'Cargo.lock', 'bytes': 3026, 'sha256': '6c25157082fded430e7f50621af590602cbd3b466eb66355c96b155aaff3b67a'}, {'path': 'Cargo.toml', 'bytes': 304, 'sha256': 'e6759d31c81be52f2e0974f19bd48fa2a2216f9017274b96a52673750a68635f'}, {'path': 'build.rs', 'bytes': 364, 'sha256': 'dccc386117e49482bc82619429c4fe7dbbf91d011a323d822fbf8ef44848eda9'}, {'path': 'compiler/main.ox', 'bytes': 1583, 'sha256': '37363118ca7802e87aa703466286c58c4fcf27f11cddbc84a92b2038648f953c'}, {'path': 'compiler/providers.toml', 'bytes': 726, 'sha256': 'f9c604942ba5cdc81e25a7896df775fb734960886581d95e26082fb105d012fc'}, {'path': 'docs/architecture/fallible-lexer-source-consumer-audit.md', 'bytes': 68742, 'sha256': '8e5791d93dadfdf41f42c266ab9f1a2921317d6f60f0a9fd7bd6d0396a712caa'}, {'path': 'fixtures/owned_source/batch.ox', 'bytes': 1238, 'sha256': 'dbbadef9a035e3af62aab6ae2ca0ac19531684e4ad8ef25bc4d81a4fa7c07db2'}, {'path': 'fixtures/typed-frontend-v2/sources.json', 'bytes': 5461, 'sha256': '08629aee51a25e05fd7bf4b121ff8a7d882683687425f4e5469f8b508618d608'}, {'path': 'fixtures/typed-record-composition-samples/main.ox', 'bytes': 314, 'sha256': 'caa20efc4525f04c9cd0db34a0cdf373ead9148010f8c9d7fc48a99ea0460df2'}, {'path': 'fixtures/typed-record-composition-samples/model.ox', 'bytes': 113, 'sha256': 'e1c95d2ae30a1b422547bde3c95218680277ed2c3d2b6b2f930170c65c00cfdb'}, {'path': 'fixtures/typed-record-composition-samples/ops.ox', 'bytes': 558, 'sha256': 'b9ad72b3f89f0c2f1e403677b16dd492a64b84464a6f2c424dbe5338db238a80'}, {'path': 'fixtures/typed-streaming-lexer/data.ox', 'bytes': 934, 'sha256': 'fcdcbb04a6a82d241655d04aa2a11fe56720d9e243dddb8dafaab9376a98d7b0'}, {'path': 'fixtures/typed-streaming-lexer/frame.ox', 'bytes': 1690, 'sha256': 'abead8c5ab6cd0207b32b20ee00a724a060a3652a1224ad3fbec6545bac8d636'}, {'path': 'fixtures/typed-streaming-lexer/keywords.ox', 'bytes': 2066, 'sha256': '73a0070f751f74c076708cc8458c09ed1d7f8cbd30220c0f2d2c3038a1249e11'}, {'path': 'fixtures/typed-streaming-lexer/main.ox', 'bytes': 1604, 'sha256': '5fb8fba3182c364e0e00048270a30c6315161505b18cbf312026952bd81514b3'}, {'path': 'fixtures/typed-streaming-lexer/scanner.ox', 'bytes': 4176, 'sha256': 'b30888d125fd4a560369828b365812b27a34f993caefc22fe1cbb17e4f49d555'}, {'path': 'fixtures/typed-streaming-lexer/sources.json', 'bytes': 984, 'sha256': 'c591fab7de5fc47578566f9b85dfd088506f07a6e2f6202af0c05646142063a4'}, {'path': 'native/oxid_cpp_bridge.cpp', 'bytes': 236, 'sha256': '51ab1721c64ae45253635f77dd006fe79acca20449352b395c6f204b883ed2a9'}, {'path': 'native/oxid_cpp_bridge.h', 'bytes': 186, 'sha256': '8905d31153b0fbde1bc7df9a3d2ab37f28d2972d86aebb313d17285da11bc9d3'}, {'path': 'native/oxid_ffi.c', 'bytes': 333, 'sha256': '79e06f3c1a8d0585f2ffa9298e8c6db390ccac7295dc7800d6788c54b68c758e'}, {'path': 'native/oxid_ffi.h', 'bytes': 231, 'sha256': '04c480159cc1b62fea78c1d13141d81d6cce4048a7c46e1e33b7d8928a804858'}, {'path': 'native/typed_preview.c', 'bytes': 3269, 'sha256': '84059e89c157386028b87f8f81468fda2d6ce0760bf4e9824b3568efc59a8a7e'}, {'path': 'rfcs/0014-owned-structs-call-borrows.md', 'bytes': 33204, 'sha256': '0685326d0a5da434449fe2aa8fb4f9a9d4ff542d0e2f1b4b410846901b4fc89a'}, {'path': 'scripts/build_hir_producers_v2.py', 'bytes': 3986, 'sha256': '32b382b037065919f2ce94e2de4393da49cb8a6ef1a6ffd439fae8d0f2bbeeb8'}, {'path': 'scripts/build_streaming_lexer.py', 'bytes': 4505, 'sha256': '2a5f6c5eef9fd95bfdda90bd888d5496ac1833d4f831b5a5af0c480d3c01adcd'}, {'path': 'scripts/predecessors/byte_storage_ci_v1/.github/workflows/ci.yml', 'bytes': 67395, 'sha256': '7ae94a3f70dd543a28b780912d7ca48fced54d39e0ba0cfdcd2152b5d5b10c57'}, {'path': 'src/cli.rs', 'bytes': 429, 'sha256': '247e03ccfe2e59b764bc84a7ae09127ee56873294acb8be4b9553acc03349f10'}, {'path': 'src/frontend/ast.rs', 'bytes': 37436, 'sha256': '142a0e7f7ec1c38a90b2fac0e5084d1f3129740b01d261422c62f832604218aa'}, {'path': 'src/frontend/branch_tests.rs', 'bytes': 9523, 'sha256': '962d4310ae3674c3ab97ad14fb7e3f8eae618c4c871c71001af36b60ed398fe3'}, {'path': 'src/frontend/builtin_catalog.rs', 'bytes': 7913, 'sha256': 'aef2125976942cb34ca71b562c25b8dbb2acbc4961433c7bb16d88607403eb17'}, {'path': 'src/frontend/declaration_index.rs', 'bytes': 68821, 'sha256': '6dff23f28bda9c5ba4f5f0f8ccaca6ddd862df5a89fe1fd039e1be0148510e97'}, {'path': 'src/frontend/declaration_index/builtin_tests.rs', 'bytes': 79581, 'sha256': 'ca071c9faf70a89f5850f2e9056da11e62f245a016a8f006041c8a2f2ecbac63'}, {'path': 'src/frontend/declaration_index/enum_query_tests.rs', 'bytes': 45564, 'sha256': '456db59525bea0d63dd856b3b4a97d5e6e0c9183fa9493cb75b6138018084d98'}, {'path': 'src/frontend/declaration_index/enum_tests.rs', 'bytes': 38998, 'sha256': '38132c31de838089d5f6b25154e4bff1180057aa8df8df5c420c0e07eb7008dd'}, {'path': 'src/frontend/declaration_index/enum_views.rs', 'bytes': 5420, 'sha256': '6148d317b8bc429e812856eae8275a88cb689649ce8b58d12bb5aa2291f1ff0d'}, {'path': 'src/frontend/declaration_index/resource.rs', 'bytes': 18128, 'sha256': 'a3003ffdf58e62399955f1fdca6a5b8a46c4acd4640b4c8489fd633be44de0f4'}, {'path': 'src/frontend/declaration_index/sealed.rs', 'bytes': 76573, 'sha256': '2f006bf29bd0901ea7f350e9b88c0e5feea22531cfafe960b58fb7084904fcdb'}, {'path': 'src/frontend/declaration_index/source_owner.rs', 'bytes': 20939, 'sha256': '4d7228bd2200d5f8989e65c1d79ff56b4c29a6e307f239c739c33d4f2f70af09'}, {'path': 'src/frontend/declaration_index/tests.rs', 'bytes': 20868, 'sha256': '008d975c52915813a4ab53f8999d04c7f2ac8010e230d9823030d29c0bc7a5b2'}, {'path': 'src/frontend/declaration_index/u8_integration_tests.rs', 'bytes': 19574, 'sha256': 'c1b2366877f9e313c02eeb54191fceb6c2d6ea159e84cd1e457bd9efdafe1952'}, {'path': 'src/frontend/declaration_index/u8_reservation.rs', 'bytes': 26194, 'sha256': 'd71816d62a1e2ff8eda15b956ab9dadedd7abe217469149a4fd8238d08b71b85'}, {'path': 'src/frontend/diagnostic.rs', 'bytes': 10885, 'sha256': '5e7ae2a1063615c47429da50ad422cb0623b0cdc1ff9e23e45bc0aa48bfed169'}, {'path': 'src/frontend/driver.rs', 'bytes': 23929, 'sha256': '39077d0d2c18bcfd88356a27c8de53e2192f555bb51c9877b263d96f4e4bfb19'}, {'path': 'src/frontend/enum_public_tests.rs', 'bytes': 24124, 'sha256': '569ae818dd0aae0d02fc88c01f77ba7404610d070c04556abc5e106352a0ea27'}, {'path': 'src/frontend/format.rs', 'bytes': 48378, 'sha256': 'e0d64d8f5a799f0a2bd805111d4f60fe2e21c904e1f733c25ddddeace271fe3d'}, {'path': 'src/frontend/format/ast_tests.rs', 'bytes': 62934, 'sha256': '865a062db162bbaa45446286675813d0443c0e8b2f365ba3db4c8db9644eee66'}, {'path': 'src/frontend/format/enum_candidate_tests.rs', 'bytes': 12921, 'sha256': '93db32377f47ed87a806ac32739509276c9619683fe666ef5249dd73a3fdeb3b'}, {'path': 'src/frontend/format/resource_tests.rs', 'bytes': 9137, 'sha256': '365478b6980017989ae80a5a38b46b2273e1be4701898ddd95f61a731dc71492'}, {'path': 'src/frontend/format_cli.rs', 'bytes': 14718, 'sha256': 'a60fc1eca53986145ffbc222ef91a2d3ee509ff4f9ae5b30308796f385c33ec6'}, {'path': 'src/frontend/hir.rs', 'bytes': 27125, 'sha256': 'b1706f183c35297aad16e58494cc1b0708b0f9ada634ab59ec2cad0a45865e6b'}, {'path': 'src/frontend/hir_producer.rs', 'bytes': 10409, 'sha256': '3d52cbe8f0f7907ffec24c7cdce65d06edc6c2cf166df2f7617436e5344c3946'}, {'path': 'src/frontend/hir_producer/bundle.rs', 'bytes': 39487, 'sha256': '184e9994566525f9a1d8c9993daf3a4c201ababf4f1529694ad03d82a3eaef99'}, {'path': 'src/frontend/hir_producer/supervisor.rs', 'bytes': 18739, 'sha256': '646af36a8e2668f7b0071047fae592dab053fea3149d75388a56f6e9acf96dd7'}, {'path': 'src/frontend/hir_protocol.rs', 'bytes': 668, 'sha256': '4d13e72a837c11461da4c6ef1b59611c5a521ccfd7c3ef2568c44968a7fe1947'}, {'path': 'src/frontend/lexer.rs', 'bytes': 35513, 'sha256': '49f7ef95ba97cb1cf4929e1002c016b26a0a673ff8126d1580695dbfdb3f55e3'}, {'path': 'src/frontend/lexical_provider.rs', 'bytes': 30273, 'sha256': '9d52958fb812b23cb52407bf76452ca098a321e4f6960de94d03c609b2ff7164'}, {'path': 'src/frontend/lexical_provider/bundle.rs', 'bytes': 37806, 'sha256': 'b5d1582e58effcf087e91d7b5794b4580c9b498a41c764ad5075353412d825d5'}, {'path': 'src/frontend/lexical_provider/supervisor.rs', 'bytes': 24999, 'sha256': '2cff4ba67c560743d4bde8395cd5c7acab2363676f4d91b491060d283fe4107c'}, {'path': 'src/frontend/lexical_provider/tests.rs', 'bytes': 45309, 'sha256': 'ce1d8ea73c4a49531fd6cd39d91de60c38ad70cb8f2c62bc5e284f1c7cec4c60'}, {'path': 'src/frontend/lexical_provider/wire.rs', 'bytes': 12654, 'sha256': '29e64aad438345e9b5bfaeb45ce25ed2c145db3b0705704670715733f4d8cdcb'}, {'path': 'src/frontend/mod.rs', 'bytes': 540, 'sha256': 'e1b402ca37401989ebe7244d731c54c68acd2fad1aa89b7f1117f645eda6bf8c'}, {'path': 'src/frontend/native.rs', 'bytes': 7505, 'sha256': '2303197eeff30da4a4c552f1131de64cd871f1fe0ff6ea3afe60010db68645b4'}, {'path': 'src/frontend/oir/arithmetic_tests.rs', 'bytes': 9856, 'sha256': '5f9bdcab64c21baa32148332b261edd4657133bc23b2b96ec0955dfa5b079af9'}, {'path': 'src/frontend/oir/cfg_tests.rs', 'bytes': 23209, 'sha256': '6361add0fcdc3cc3bd6ad2001fb1bea4640b913f96ad9a64396a0e0ecb7eaacc'}, {'path': 'src/frontend/oir/comparison_tests.rs', 'bytes': 9098, 'sha256': 'eea9ed5b72e767ce94d8aa338641603ea9e3d7c0602c28736a0679ae289f64f0'}, {'path': 'src/frontend/oir/cyclic_tests.rs', 'bytes': 41897, 'sha256': '64a4e547d99bccfa0cb036d18c4c0e081f9e1585ce5cdbb918f81d845aac7ac1'}, {'path': 'src/frontend/oir/execute.rs', 'bytes': 20848, 'sha256': '0cecc5efc511b152bf50c4f9ccb50ee41f6b9ea389a2576a0ad5f339a45a58a4'}, {'path': 'src/frontend/oir/execute_measurement.rs', 'bytes': 12194, 'sha256': '337d5181608ced1b891ff246f1fa12ccb6ee7b8b0dc324ff6fbb72a05506f01f'}, {'path': 'src/frontend/oir/execute_tests.rs', 'bytes': 38336, 'sha256': '1d8d706bdad5d29edbbe55f244897725a28352f6f52a0c543842f6a1876353f4'}, {'path': 'src/frontend/oir/i32_tests.rs', 'bytes': 6612, 'sha256': '41da8b4a739bb6edf4d43be097db5c1e2653083e2f92f7f938c44c8b08348278'}, {'path': 'src/frontend/oir/logical_tests.rs', 'bytes': 19222, 'sha256': 'e78e96cd7e3df44beef8ca0736b67d3d3df1992ac56e111d4af99d947929dcc2'}, {'path': 'src/frontend/oir/loop_control_tests.rs', 'bytes': 9862, 'sha256': '35e1f943942d50b287327081d40c5f55352a72345a63a1afb5495ced4d67cecc'}, {'path': 'src/frontend/oir/lower.rs', 'bytes': 41926, 'sha256': '7983aa33c8ef36d6656939cf762e028b1f81edd6e972c20087c7c4cd458984f3'}, {'path': 'src/frontend/oir/lower_measurement.rs', 'bytes': 13874, 'sha256': '6b793bee158515b816a3fc26888e9f9554f575b538f9be36bfa855c6fcd1bed9'}, {'path': 'src/frontend/oir/mod.rs', 'bytes': 11355, 'sha256': '13e8fab8d46335a999667742950cbcadb81f0b6683df2c95dbd309e141a01ede'}, {'path': 'src/frontend/oir/mutable_tests.rs', 'bytes': 18721, 'sha256': 'e6982441dd14b499199c4eaeec101488a4c491fdad2061633fd36ac53dbe630e'}, {'path': 'src/frontend/oir/native.rs', 'bytes': 90351, 'sha256': '73a6e7128f29d820971146bbdb4d951bce697c34f4408f4ac021666708c284f8'}, {'path': 'src/frontend/oir/native_emit_cost.rs', 'bytes': 32446, 'sha256': 'cf9c8ec244e433d92cf796491f4b47c91049b6865a89e614894658d03fc56e62'}, {'path': 'src/frontend/oir/native_emit_observation.rs', 'bytes': 19992, 'sha256': '01527e75684ed5e49c2cdfa439082d525ab939b3dc95c61bd638b2e89723b036'}, {'path': 'src/frontend/oir/native_emit_work.rs', 'bytes': 28557, 'sha256': 'f06514b0ae6e891143603ca0a6464d7fbdae1a815b97b78a0dd67703b62d4404'}, {'path': 'src/frontend/oir/native_private_emit.rs', 'bytes': 9919, 'sha256': '6bd3b61a2d554a6a92cec5264e84231d97e3521bf876ea9159e76e4b5e567838'}, {'path': 'src/frontend/oir/native_private_emit_tests.rs', 'bytes': 11335, 'sha256': 'e273b461c4e23796d59cf55222640f573a5bae08be1681d181a831375b02808d'}, {'path': 'src/frontend/oir/native_scalar_resource.rs', 'bytes': 18755, 'sha256': '440bd1446549aeb09799369b548f1968405d126e9d447f6700cd81aac96a72da'}, {'path': 'src/frontend/oir/native_scalar_resource_proof.md', 'bytes': 4056, 'sha256': '86109e8f794282d9104c73e17b169449363c6786ade2563a50b799132072135f'}, {'path': 'src/frontend/oir/native_u8_tests.rs', 'bytes': 22369, 'sha256': 'ae2b7d2fd4de1b6defdfd7276e0c5e651087aa4dbe1c9cc36f3b14ec3f9e220d'}, {'path': 'src/frontend/oir/negation_raw_tests.rs', 'bytes': 16234, 'sha256': '57e22f6e90ea60ede3ac474631e29e6158925ca5cd5cf481424a8b032ff0fd5e'}, {'path': 'src/frontend/oir/owned/allocator_review_controls.rs', 'bytes': 16905, 'sha256': '3b50bf34dbbf9e6064688dc8dedb98221c4c30596ed50e55209731d8ffc10fd1'}, {'path': 'src/frontend/oir/owned/array_native_resource_tests.rs', 'bytes': 52496, 'sha256': 'c711f3cc0f8005da59eb3f2361573ae611b88ee65d3ee6813d51958ad48d6daa'}, {'path': 'src/frontend/oir/owned/array_native_tests.rs', 'bytes': 51016, 'sha256': 'b48a5524b30ae60663fbacddcde470d8cdcff0d3e5351dbc7625345fa21a73c7'}, {'path': 'src/frontend/oir/owned/array_observe.rs', 'bytes': 23985, 'sha256': '4a475e36bd85875df6c6b66932a4f193ad7ead8a3cf9c6716dab696a2caf4816'}, {'path': 'src/frontend/oir/owned/array_reference_boundary_tests.rs', 'bytes': 33365, 'sha256': '2e2470b355597f6ced9379b9fbd921be3f10e6b90315f732178c507197db88b7'}, {'path': 'src/frontend/oir/owned/array_reference_tests.rs', 'bytes': 44819, 'sha256': '2f55c2de967a0c23254affa2ff33a5dde0e3a105e1be7dee94bf565dc39657fc'}, {'path': 'src/frontend/oir/owned/array_tests.rs', 'bytes': 33593, 'sha256': 'ed689dc5d4d8dfb4f67474482414852900a17a88a4c2eecd9020e3442f1eeb91'}, {'path': 'src/frontend/oir/owned/budget.rs', 'bytes': 18561, 'sha256': 'b7f450f8eb2866942ca5a09b844d17dd33e88ca483207339fb94baa5c0f27119'}, {'path': 'src/frontend/oir/owned/builtin_descriptor_tests.rs', 'bytes': 23816, 'sha256': '8bfc3b65c5de00cd8d9f402432ee969e71a6946a91a0905a667851277b275257'}, {'path': 'src/frontend/oir/owned/builtin_input_fixtures.rs', 'bytes': 20354, 'sha256': '83594ab6226b0dd6a94dc8c1b7a0c79015fa351ff43ea15a3620a5edefbe8176'}, {'path': 'src/frontend/oir/owned/builtin_input_native_tests.rs', 'bytes': 10979, 'sha256': 'd9eb20679769c6464b6123dc2a2767bb64b4ad2dde081ba16da5ef8b989df3ee'}, {'path': 'src/frontend/oir/owned/builtin_input_tests.rs', 'bytes': 15488, 'sha256': '84401aad98d67af4da43c4b199be7843e568c0ab709d319068850d1798a669c9'}, {'path': 'src/frontend/oir/owned/builtin_origin_tests.rs', 'bytes': 5463, 'sha256': 'de4b9fcb584769b58241e5b147806616c5b618581fe1061bcbc62225ca1a1af2'}, {'path': 'src/frontend/oir/owned/builtin_output_descriptor_tests.rs', 'bytes': 20202, 'sha256': 'e3c8a1090288acc8abe9b27b163571614d192948647db5fdddb3d1f4a98b0cad'}, {'path': 'src/frontend/oir/owned/builtin_output_fixtures.rs', 'bytes': 9795, 'sha256': 'ee170ab6e64a7f9f20e8bdfe9f666de04f91d0e91d550dd845e075ba21ac5568'}, {'path': 'src/frontend/oir/owned/builtin_output_native_tests.rs', 'bytes': 18610, 'sha256': 'adaf30a1bce3f453e1288dd73dd45ad958a9308c7411e2c813164eeadc179f0c'}, {'path': 'src/frontend/oir/owned/builtin_output_process_tests.rs', 'bytes': 19461, 'sha256': '19dce2f69ff9d089f54661864473f0fb2f7c73bb1a406a3f51243cd8a966f0e6'}, {'path': 'src/frontend/oir/owned/builtin_output_reference_tests.rs', 'bytes': 8356, 'sha256': 'e90553824e6aeaeafd572c82bebd34c8dfbd40bebf3b826140dd62492b833fad'}, {'path': 'src/frontend/oir/owned/builtins.rs', 'bytes': 11697, 'sha256': '2c7e5001e6985dcd1acc86a6d5543d864935919e08a5459cff76b8816c70d42e'}, {'path': 'src/frontend/oir/owned/byte_storage_codec_tests.rs', 'bytes': 24522, 'sha256': 'b2e43ea4b3d022d3fbf6c825d034c7786e525ac8b3d9664e2b42eeb78ad0f44a'}, {'path': 'src/frontend/oir/owned/byte_storage_native_tests.rs', 'bytes': 58053, 'sha256': '0db294445eb319fe93462cb062ff6b2af5c449144e2b9a8469d4e622d66d2737'}, {'path': 'src/frontend/oir/owned/byte_storage_reference_tests.rs', 'bytes': 5685, 'sha256': '8dfc6cd85245a93783b3450a07735fa3cb8cf604adfc03dfa9b60090ce44ef2c'}, {'path': 'src/frontend/oir/owned/cfg.rs', 'bytes': 7051, 'sha256': 'b73dc29914ff16a6230b9c48c83af3244720a7ad42f6ebb636afa2bb08a2f542'}, {'path': 'src/frontend/oir/owned/composition_native_tests.rs', 'bytes': 23696, 'sha256': '58b8b14fffea67516868567d035e90db33f70b24403310ac8d2f9c4eae957743'}, {'path': 'src/frontend/oir/owned/composition_reference_tests.rs', 'bytes': 28985, 'sha256': 'f3c7fe0ae68e476214fdc73faa6dfe2a92975f33aaa43ff32fcc5bd1193afbe7'}, {'path': 'src/frontend/oir/owned/composition_verifier_tests.rs', 'bytes': 7570, 'sha256': '318c84103c4d1647d79af8cda5ebae684ea11c49cd41980fe058bfce01233303'}, {'path': 'src/frontend/oir/owned/consumer_fixtures.rs', 'bytes': 45750, 'sha256': 'b3ef73d733c32f6c7e51c6457e28d8b9e3ed6139047aeb192758a0e17ab84817'}, {'path': 'src/frontend/oir/owned/consumer_pilot.rs', 'bytes': 35134, 'sha256': 'f8ec36aaa4cd3fd7e9a697d4774477cbaa0f680591bd85372b762998de3e0c21'}, {'path': 'src/frontend/oir/owned/consumer_tests.rs', 'bytes': 2296, 'sha256': '648d6414c27a6eae115d7be88d84d8005fc84470b8c675dbd4db5346df30647c'}, {'path': 'src/frontend/oir/owned/denial_tests.rs', 'bytes': 27121, 'sha256': '4bbf3f5debc2dac7345a8047d0cd8ac04b5c1556d8a47268de175872debe1339'}, {'path': 'src/frontend/oir/owned/enum_admission_tests.rs', 'bytes': 6402, 'sha256': 'ef9c602fb587bb528981dc79993e2ec2daa251851bc2252cb80154aa70ce3141'}, {'path': 'src/frontend/oir/owned/enum_consumer_fixtures.rs', 'bytes': 30800, 'sha256': 'e50f3f4c2501f55c6e288d07ed827e3dda0f204fd5666caba1141ada2fd6af18'}, {'path': 'src/frontend/oir/owned/enum_formatter_allocation_tests.rs', 'bytes': 34694, 'sha256': '100eb783c44d6a9363c0446a2874cc5339bedc10d719f52e609913c0d9fcdbd7'}, {'path': 'src/frontend/oir/owned/enum_index_allocation_tests.rs', 'bytes': 30040, 'sha256': '716b89f66bd23f47b73b5ecb7b225aee8b0a70eb033dc4069a60656f16776685'}, {'path': 'src/frontend/oir/owned/enum_layout_tests.rs', 'bytes': 1240, 'sha256': '04a74f489dea181459e2a0742c2d8edb4ce8da49478409305ed3fc211e82e980'}, {'path': 'src/frontend/oir/owned/enum_match_tests.rs', 'bytes': 49630, 'sha256': '3d86eff348440ea987de3d539213e195ff25747662ffad72fb32bded22ba0f16'}, {'path': 'src/frontend/oir/owned/enum_native_tests.rs', 'bytes': 57403, 'sha256': '3dcd52dc5da5bb1ac5311cd65d3ee5ea3a02be56c3282ca2c9b6ddd092fb58f5'}, {'path': 'src/frontend/oir/owned/enum_parser_allocation_tests.rs', 'bytes': 19475, 'sha256': 'a05bf06cf7121b5a68b08e1c003f3362dc6858229e518c9db9ac9908a6760a81'}, {'path': 'src/frontend/oir/owned/enum_query_allocation_tests.rs', 'bytes': 13414, 'sha256': 'b775d56a0e259a71ce3451fdbff6aa09bee4e5c6becfb5e50c6fd9f61550d0f7'}, {'path': 'src/frontend/oir/owned/enum_reference_tests.rs', 'bytes': 42213, 'sha256': 'dc1cba4c9a338f036f35b93eeaafa924fdeeffdb41f3c810a330df7661d30458'}, {'path': 'src/frontend/oir/owned/execute.rs', 'bytes': 122423, 'sha256': '63c1dd8b2c0b32cb5ac36a50539dd7bb6f1c0b6bf45243ff8abe6162aea1aee8'}, {'path': 'src/frontend/oir/owned/execute_tests.rs', 'bytes': 41762, 'sha256': 'd0dfae8bfdf649eb40212a1b5371bddb176fc4f7da91ff3892fde6b94b512dd7'}, {'path': 'src/frontend/oir/owned/flow.rs', 'bytes': 42665, 'sha256': '879e4bf1c2fe9edc3a3d8defeb92b6b9dd90e9ab8ff8638569dae54b94c8f290'}, {'path': 'src/frontend/oir/owned/input.rs', 'bytes': 1616, 'sha256': '4dfecbe4d57ac9dfcda967fc24635704330dcec4602a544883d0a09dfac1cbea'}, {'path': 'src/frontend/oir/owned/mod.rs', 'bytes': 13783, 'sha256': '8d55153fcc28870aab09df96cae217afc1d9915f7d56163fada5cb529c96cf54'}, {'path': 'src/frontend/oir/owned/native.rs', 'bytes': 156077, 'sha256': '85721e3d9aca382a76467a453bb098a6af9601dd2ae663600aa9f6dabca97c2a'}, {'path': 'src/frontend/oir/owned/native_heldout_review.rs', 'bytes': 25498, 'sha256': 'e8534ff9ac0ff275970de0bc0b8bfc59605a395bba7f96bd093c5b91e22fc4b2'}, {'path': 'src/frontend/oir/owned/native_inventory_admission_tests.rs', 'bytes': 17805, 'sha256': 'd0d5d1b80e725b9bdaeda7bf8dcfe483b6c71e941acb33dd3504cb42f7814029'}, {'path': 'src/frontend/oir/owned/native_inventory_tests.rs', 'bytes': 22820, 'sha256': '8192cd48ba0f8e147c7d6f09662a52f41b8439da2144117189d319935259a4ae'}, {'path': 'src/frontend/oir/owned/native_storage.rs', 'bytes': 18118, 'sha256': 'fd9451ebda9e1d27d896fc4e2a31a0c45cb226dea0c71cdda4463ad6811a15c0'}, {'path': 'src/frontend/oir/owned/native_storage_tests.rs', 'bytes': 17001, 'sha256': '12c3e0d6202a46f3c58b9462c781e80a484118f34ddee8075e38d5bebfa6afc1'}, {'path': 'src/frontend/oir/owned/native_tests.rs', 'bytes': 85951, 'sha256': 'b24ae27949d3b66e3bad994128e8815983d44838421b55a6adfc544e0c6f3c16'}, {'path': 'src/frontend/oir/owned/negation_raw_tests.rs', 'bytes': 13503, 'sha256': 'ff9769b343eb5ae4368abbece53cb0cfdf6d7df829e37f342cde49f1b7224998'}, {'path': 'src/frontend/oir/owned/oracle_tests.rs', 'bytes': 114781, 'sha256': 'a2c78eb3ffc1b5f33f455fe93daf52c2d8b008da41a161798a39c9b6175ea337'}, {'path': 'src/frontend/oir/owned/origin_tests.rs', 'bytes': 21959, 'sha256': 'bef38d831e14ee24c57a2d1fd2e04d1771bcb7241598a4a670f2c61f7e99d05f'}, {'path': 'src/frontend/oir/owned/output.rs', 'bytes': 2751, 'sha256': '23da6c55e1d22e4eb8c4d3d596aae1280e291301ec37fb412c44eb1fe921f720'}, {'path': 'src/frontend/oir/owned/plan.rs', 'bytes': 21672, 'sha256': '1885bc3bfaff7e7de8c7b74084b2bf44f3e6d0609b71c62dab16560c7275f093'}, {'path': 'src/frontend/oir/owned/process.rs', 'bytes': 2896, 'sha256': '3a9862b1735a38aa43e1bd79e2326b9bce23d9f6ffc6565b481e0c57da7723df'}, {'path': 'src/frontend/oir/owned/projected_slice_native_tests.rs', 'bytes': 18310, 'sha256': '24cf3fe21ed0e3ca68e239df695e438a9758b93d7c724d170466e70c96096c04'}, {'path': 'src/frontend/oir/owned/reviewer_allocator.rs', 'bytes': 11796, 'sha256': '59de66000273559c9a0a8357970147d624f31d4a79563986b430a4550ac3ec5a'}, {'path': 'src/frontend/oir/owned/reviewer_array_observer_tests.rs', 'bytes': 1616, 'sha256': 'de545a0ff0d68e851d474e0fbac43a54def930ecf86113b456340bb36fd5ba30'}, {'path': 'src/frontend/oir/owned/reviewer_array_reference_tests.rs', 'bytes': 65101, 'sha256': '4a2c34aaba8d6bd7484a7aa4eedf7808ac57acea9401b5131105d0ab52fdd537'}, {'path': 'src/frontend/oir/owned/reviewer_heldout.rs', 'bytes': 57852, 'sha256': '63bb5a7bb9813904aa0d0b20b62e5d886f91d9d0ca9b7092efa0d5cf1eb16e68'}, {'path': 'src/frontend/oir/owned/reviewer_origins.rs', 'bytes': 23036, 'sha256': '1f43e4e3fecb081a5f2272834d31c18cd91e07bc7eb0297f6b6c41805ca461fe'}, {'path': 'src/frontend/oir/owned/reviewer_reference_tests.rs', 'bytes': 71678, 'sha256': '7df7f76cd83acc276994d58cb7d0da5bcfcb131a1d1be4ae1c4b85b7ef1484bd'}, {'path': 'src/frontend/oir/owned/shape.rs', 'bytes': 48005, 'sha256': 'bfd683b9a0ae2df09c6078672ad3e51e50a058e29efc93af53285e7f51a36e2a'}, {'path': 'src/frontend/oir/owned/slice_native_tests.rs', 'bytes': 12790, 'sha256': '0fadb3ee8154810ab825b2993e35db3489bc30da6fd65d71be374b8e671a81ce'}, {'path': 'src/frontend/oir/owned/source/array_consumer_tests.rs', 'bytes': 18087, 'sha256': '9fc6effe228f6c23b602b4da8cca6e27bd2f8b0311ccbdebaa7494381080aaf8'}, {'path': 'src/frontend/oir/owned/source/array_pipeline.rs', 'bytes': 48105, 'sha256': '691bc9f3ab2ca9f49da1e8e80f464cedda18ef360f01ad12c8f660a18a8cc653'}, {'path': 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'bytes': 53200, 'sha256': 'f3c2fbc653a7d6203432bbd98a32ec772b93fe295f72a3449bbcc11f111e7f07'}, {'path': 'src/frontend/oir/owned/source/array_pipeline_tests.rs', 'bytes': 24684, 'sha256': 'ed2019228d92597668633f1dad830380d5fa010fe74388940fc38dc39b5e55b7'}, {'path': 'src/frontend/oir/owned/source/array_pipeline_transport.rs', 'bytes': 12200, 'sha256': 'd45b13d62cca76ca06f9b89bf6d51db7692fede9e002e167c0f0a41af8ce1b2f'}, {'path': 'src/frontend/oir/owned/source/array_type_controls.rs', 'bytes': 12644, 'sha256': 'ab61e226fed7a2103c7c4bb6fab2af7ddad9a405f233f299f30c01753659a713'}, {'path': 'src/frontend/oir/owned/source/array_types_tests.rs', 'bytes': 44616, 'sha256': 'f7cdd484cfdf9c19a849dccb29366f20cea4e84668007350547e474a4673bf94'}, {'path': 'src/frontend/oir/owned/source/association.rs', 'bytes': 54174, 'sha256': '54f086e149b7df27adcbcffe9b325a3ae82f3869742bd0023da9fbfefdce5fc0'}, {'path': 'src/frontend/oir/owned/source/budget.rs', 'bytes': 15279, 'sha256': '916524c951e65f1dd4b723530ce8774929c110f15b9619cfb34fa2b0d7e04f52'}, {'path': 'src/frontend/oir/owned/source/budget_tests.rs', 'bytes': 11422, 'sha256': 'f531a8e3a4c681db7588e8bee1ccbefe4f7f12b1a0fb73acb822ed0198cbbf48'}, {'path': 'src/frontend/oir/owned/source/builtin_lower.rs', 'bytes': 7476, 'sha256': 'bac907c1c2162e8e0c343c3176f22e135974998c453f15d6aa388985cb56320d'}, {'path': 'src/frontend/oir/owned/source/builtin_signature_tests.rs', 'bytes': 28844, 'sha256': '322ab1dc9ef58b48220e4aeb9b99b0d4c8cc9b020c3bfbffa0523a8518e09571'}, {'path': 'src/frontend/oir/owned/source/builtin_source_tests.rs', 'bytes': 11536, 'sha256': '8d59b6b2e30f7ea7cbd850c23f565d430612698d0fecfe7197a54ae3b3d28317'}, {'path': 'src/frontend/oir/owned/source/byte_storage_association_tests.rs', 'bytes': 3788, 'sha256': '87206c95a6b7f361b3092fe2936710adafe2411720cfa1517d7c6bc8d617e818'}, {'path': 'src/frontend/oir/owned/source/byte_storage_authority_tests.rs', 'bytes': 9515, 'sha256': 'dfc8fbdfcd1b5d278de85b7e219f3d71cf2b249c6d1d607d3c4024da94c48658'}, {'path': 'src/frontend/oir/owned/source/byte_storage_fuel_tests.rs', 'bytes': 29968, 'sha256': '240b012ad1fd1808e712bc4f37b669fbd64049dad95242c23253c505c42010e0'}, {'path': 'src/frontend/oir/owned/source/byte_storage_native_fixture.rs', 'bytes': 827, 'sha256': '8b8638382334f91fe40886c66efbf45946e8e4e66c8515a69b6a0701e1477858'}, {'path': 'src/frontend/oir/owned/source/byte_storage_raw_tests.rs', 'bytes': 10425, 'sha256': '8a4b233b5ce3e4cd3c56b92b846997dbe2f794f9763cb123ab78a0fc7ccc1657'}, {'path': 'src/frontend/oir/owned/source/byte_storage_resources.rs', 'bytes': 10158, 'sha256': 'dfede700eb8540e010d8bbed4eb493c08aa8767e9226db6c8082559df40b2fb6'}, {'path': 'src/frontend/oir/owned/source/byte_storage_tests.rs', 'bytes': 25395, 'sha256': 'e5336e96f93377867a9f51a13f596f34b3f59107006226edff5e41f0445bfb2b'}, {'path': 'src/frontend/oir/owned/source/byte_storage_trust_tests.rs', 'bytes': 21384, 'sha256': '94d8b96af5ad8d304b7e77bd0cccd7fe1aa7db0f901af245b8492cf19ccc1e38'}, {'path': 'src/frontend/oir/owned/source/byte_storage_type_tests.rs', 'bytes': 12887, 'sha256': '9a5de79eb21eaa488a6225a2d2bf0ffdd39ee209018a69504140537934422110'}, {'path': 'src/frontend/oir/owned/source/candidate_adapter.rs', 'bytes': 41762, 'sha256': '16fcfa3aa73c38c5b56ba04b9ede693791ddb37d8af9d29d58495c9455af792c'}, {'path': 'src/frontend/oir/owned/source/candidate_mutations.rs', 'bytes': 21510, 'sha256': '6b6285b75426144ddcc20ff65175c4beb7761dc08ada3f7e7318053322ca075e'}, {'path': 'src/frontend/oir/owned/source/candidate_native.rs', 'bytes': 32115, 'sha256': '45c10ff22c65b09d92f9547dfc9150a601659ce92b0e272e8273fd557c2b04b2'}, {'path': 'src/frontend/oir/owned/source/diagnostic.rs', 'bytes': 8003, 'sha256': 'a5f4e81f44d978e533536928ebb6f1d267a7f4e42ce83a3aa8ebb2e9780af8db'}, {'path': 'src/frontend/oir/owned/source/diagnostic_tests.rs', 'bytes': 19667, 'sha256': '589bd31889a77a27c8ec0f7916e75a470cf50d9e04d0477b65e9fe71f572cf76'}, {'path': 'src/frontend/oir/owned/source/enum_native_source_tests.rs', 'bytes': 26987, 'sha256': '23e5f371c241348709064af58026f5cd3ca55f6fa8b62be07cb45a51d8dda719'}, {'path': 'src/frontend/oir/owned/source/enum_storage_failure_tests.rs', 'bytes': 14521, 'sha256': '1f783fd55e95d8cdbe5bd24a01b87620fc3a358b328a1038a0699ae3ad5c827b'}, {'path': 'src/frontend/oir/owned/source/enum_type_tests.rs', 'bytes': 21948, 'sha256': '834394d4cc3640534257c6b74a6200a824c78a8c4a2a36f2e0973800c3b68173'}, {'path': 'src/frontend/oir/owned/source/hir.rs', 'bytes': 6027, 'sha256': 'f4b25def2b2bb04db9983e1d55043638f104f451a97ab0bf2a8879c8bd195f61'}, {'path': 'src/frontend/oir/owned/source/hir_budget.rs', 'bytes': 47753, 'sha256': 'c9071bbad8f379c9c28546785e8235cc723afd2d41b4428ee8d107b53c254b86'}, {'path': 'src/frontend/oir/owned/source/hir_budget_tests.rs', 'bytes': 53199, 'sha256': 'b9f384a6ea369cc59514a4adda4097574413df2f2e58ba88f2c729ea2c06dc89'}, {'path': 'src/frontend/oir/owned/source/lower.rs', 'bytes': 100849, 'sha256': '7bc8863223e7f6af89ab81fd8375eadfb7ba37e853d9c6c5e8d03d60ee67401c'}, {'path': 'src/frontend/oir/owned/source/lower_tests.rs', 'bytes': 10371, 'sha256': '031b2d1e77edc63d4ab3db72eb77a6c1d3d1b2278a52092058fa79f7258e3649'}, {'path': 'src/frontend/oir/owned/source/mod.rs', 'bytes': 1767, 'sha256': 'f977087603f7adaebc1ffd88230e8acee91844da1e6075a40eb4269862b4a90d'}, {'path': 'src/frontend/oir/owned/source/native_resource_tests.rs', 'bytes': 7238, 'sha256': 'ce881a1637bd4f2971adf231449e0e50eb90d1910456a12e80fe3945daa47f91'}, {'path': 'src/frontend/oir/owned/source/output_lower_tests.rs', 'bytes': 31909, 'sha256': '198014c625eb33be6e2a4e85f8c775752f0ed340fe47b12ea07f37e21f63fa4e'}, {'path': 'src/frontend/oir/owned/source/output_source_tests.rs', 'bytes': 22054, 'sha256': '70fedb159c112982c33c64550ec8c007cd5e6e3fac41753d0fd8ec7edddc62cf'}, {'path': 'src/frontend/oir/owned/source/output_typing_tests.rs', 'bytes': 44450, 'sha256': 'aedfd65ad31da31e9ec530cbc4e332280f2d3d485f88426593c9e279d23ba784'}, {'path': 'src/frontend/oir/owned/source/program.rs', 'bytes': 31675, 'sha256': 'a4423e32910db1f00f23e5f84010c74d3b5ea93c57aee6e43b0abdf0c42ce4ad'}, {'path': 'src/frontend/oir/owned/source/projected_slice_raw_tests.rs', 'bytes': 4126, 'sha256': '05a98c16899b51c18df1fa2c77502ea4d6d1913ac6c53710a3ee028df8e4da88'}, {'path': 'src/frontend/oir/owned/source/resolve.rs', 'bytes': 127650, 'sha256': 'a55cba645425c4cce8061027ac30f5afe13477dd96854f12ccc124e997e07f20'}, {'path': 'src/frontend/oir/owned/source/resolver_enum_tests.rs', 'bytes': 23989, 'sha256': 'a753a284d8b5e0ab41b168123b359f504c2480089e336680e3281a478c0092fe'}, {'path': 'src/frontend/oir/owned/source/resolver_inventory_tests.rs', 'bytes': 17772, 'sha256': '99f51419dcf0fe03ec58edc4b69801ea6f06c879cc2a4fcafda5c51af6860296'}, {'path': 'src/frontend/oir/owned/source/resolver_paid_tests.rs', 'bytes': 107995, 'sha256': '46b8ef266803d2b1d046b0bbc8c3de3192fe9f361ea1b54f24921adc3140fdaf'}, {'path': 'src/frontend/oir/owned/source/resolver_storage.rs', 'bytes': 29081, 'sha256': '7fbf5d082952934bdf2c0fad4f0d99796fef3e31ecd33eaa01c953e6cdce7761'}, {'path': 'src/frontend/oir/owned/source/resolver_storage_tests.rs', 'bytes': 24000, 'sha256': '7c99053b314fabbf0826d41463a3cef6386e948d04deec73047176e1e9af2059'}, {'path': 'src/frontend/oir/owned/source/resource_fixtures.rs', 'bytes': 7164, 'sha256': '0e65aa030e3aa56b15f7a5f66050454e75d84a508ab4cb43dbb07094511d6023'}, {'path': 'src/frontend/oir/owned/source/reviewer_heldout.rs', 'bytes': 26974, 'sha256': 'adb4f0586bfe7cd96051954e39c60ee80fe796c237f6a70f6a1dd4a301eb124e'}, {'path': 'src/frontend/oir/owned/source/reviewer_resource_native.rs', 'bytes': 1862, 'sha256': '1a5bfa835de5b22b89d8facc66adc941970dfd968e4d59f88123b6faf8cdb6ed'}, {'path': 'src/frontend/oir/owned/source/reviewer_resource_runtime.rs', 'bytes': 1929, 'sha256': 'f7d7ceb22354a2a5dbef97540204930103574ecbdbad8ef92b3278455979e01b'}, {'path': 'src/frontend/oir/owned/source/reviewer_source.rs', 'bytes': 26674, 'sha256': 'bdbc50a2af4b4de486314f1941c2bec200754ccd508092f851a928eaa6423874'}, {'path': 'src/frontend/oir/owned/source/runtime_resource_tests.rs', 'bytes': 10067, 'sha256': '11e973beae6cbd3a12cea6ca3ea463433d799d246209ff80ad2a40f88330380d'}, {'path': 'src/frontend/oir/owned/source/slice_raw_tests.rs', 'bytes': 4010, 'sha256': 'e67a18eaa5053886c25e901e5e0baa489ddb7bc74a1d3b1cffa4783976cec06a'}, {'path': 'src/frontend/oir/owned/source/slice_tests.rs', 'bytes': 6645, 'sha256': 'ec12348d642da4c289daa2d0c2e57459e0e729d1a0fb1615d1a4aceee2062c3c'}, {'path': 'src/frontend/oir/owned/source/tests.rs', 'bytes': 20592, 'sha256': '686a67ca817c06462d573ed03cef5f77aafd5545f5a0e9065f8d829560be1a51'}, {'path': 'src/frontend/oir/owned/source/type_storage.rs', 'bytes': 68334, 'sha256': '792fa2d8b36fdaac302616327eca2c2f50550142c967e783546eddf736385a2d'}, {'path': 'src/frontend/oir/owned/source/type_storage_tests.rs', 'bytes': 117370, 'sha256': 'ea04fe1eb4b77951258541d894da383ad3207de818691ab8f920c8a730c63479'}, {'path': 'src/frontend/oir/owned/source/typeck.rs', 'bytes': 201573, 'sha256': '4e4b81aa9ded291a89aa36375066e8e15722789606e3143878021fcec933865c'}, {'path': 'src/frontend/oir/owned/source/u8_resources.rs', 'bytes': 6871, 'sha256': '188a16e1b68a5d57be88edce2c7fd7c9941cc6461b9161fd74467c19ff37525f'}, {'path': 'src/frontend/oir/owned/source/u8_tests.rs', 'bytes': 27343, 'sha256': 'db402af2b3cc0b8051c884add4893e54c538317865b5e849c80b8476fdf51402'}, {'path': 'src/frontend/oir/owned/storage.rs', 'bytes': 1925, 'sha256': '1190bf5496f5fb3bbbc30eeea6767f3b2e8544acc01c8c5aaa948743dea1c597'}, {'path': 'src/frontend/oir/owned/tests.rs', 'bytes': 51046, 'sha256': 'bf72cd7f0e2444aba8283cb58843daa4ebef7bc03959d6cd64f67e3ccb428524'}, {'path': 'src/frontend/oir/owned/u8_tests.rs', 'bytes': 11095, 'sha256': '6f031e6524f87ceb1df6d33b227b6d311faad407c6b59461ed3eb59312f1da21'}, {'path': 'src/frontend/oir/owned/verified.rs', 'bytes': 12803, 'sha256': '6ee74d28ac16a3552db789c8a7bca5e6a272b7ec536626cf02ef5f9fb796eb5d'}, {'path': 'src/frontend/oir/owned_types.rs', 'bytes': 71889, 'sha256': 'b6406a8e4068ac935bc076235a83600e127f1a39fb2f21368aad5d02c507b604'}, {'path': 'src/frontend/oir/owned_types/array_tests.rs', 'bytes': 12755, 'sha256': '0f30f9b9534ea7be134d5cfa7f0b913338195069ec9c5cb4484ae0597db9dc23'}, {'path': 'src/frontend/oir/owned_types/byte_storage_tests.rs', 'bytes': 9074, 'sha256': 'c30d2359bb65e524df8d781d855bb9b07d5553541d584670e9e137420ada38b3'}, {'path': 'src/frontend/oir/owned_types/composition_tests.rs', 'bytes': 8678, 'sha256': 'd09fc584c482fc6effd983bd3763e5afe71f177baf76708d5728106500fe89f5'}, {'path': 'src/frontend/oir/owned_types/enum_integration_tests.rs', 'bytes': 11459, 'sha256': '0235dd1f9eb0a4b474a1cad60d8afa52bb58ea0a320591e4e0775aa3ed224913'}, {'path': 'src/frontend/oir/owned_types/enums.rs', 'bytes': 40929, 'sha256': 'aa4aef386db4b1297047dfeab24b466a6010a940bbc7ee2599a3174979c67053'}, {'path': 'src/frontend/oir/owned_types/u8_tests.rs', 'bytes': 2642, 'sha256': '4f938fd912541afb7ed555ead2d10118b2390676510169122c82f6b900a6b9e5'}, {'path': 'src/frontend/oir/project.rs', 'bytes': 3916, 'sha256': 'c0a9f6ed2ba3749ab7ab68fa5ef7d10d7ead2603c45fa1fee510674d155e7b77'}, {'path': 'src/frontend/oir/project_execution_tests.rs', 'bytes': 9682, 'sha256': 'b137754bf7a126d07d1f8b73708dc9cfc95dba5d249660884daa5d233052c78c'}, {'path': 'src/frontend/oir/source.rs', 'bytes': 9940, 'sha256': '9d03d9d9c4e078cab29561f24366097b16493162a91519ea2c29c9ded5dfead4'}, {'path': 'src/frontend/oir/source/association.rs', 'bytes': 29015, 'sha256': '678383fe00f81f021236997794a2623abde47e4a0a30bcbda337cfee02f27148'}, {'path': 'src/frontend/oir/source/conversion_seen.rs', 'bytes': 17404, 'sha256': 'e37fea5cb063ae9830eaa8b20bf10973edf24f58cf5119780a89a855d88e826f'}, {'path': 'src/frontend/oir/source/hir_import.rs', 'bytes': 17381, 'sha256': 'f14ab8cdd0ea01adea1e1d6aae0457546da868297530c1ce96eac3ff6ee35f7d'}, {'path': 'src/frontend/oir/source/hir_import/allocation.rs', 'bytes': 27362, 'sha256': 'a6894e68300e9449dfd76a8275ee7c99b351ad31f6eced1316087f88a40d73a9'}, {'path': 'src/frontend/oir/source/hir_import/allocation/tests.rs', 'bytes': 21318, 'sha256': '210318b300f3eada1e53db4628c38aa9685b6f256d2f80105fcf4a3d1206cec9'}, {'path': 'src/frontend/oir/source/hir_import/ast_compare.rs', 'bytes': 43079, 'sha256': 'cb2613697231a532131ac252b9f484f60324ba2efd9dc3b207896e8a0e4e08a4'}, {'path': 'src/frontend/oir/source/hir_import/ast_compare/tests.rs', 'bytes': 16458, 'sha256': '92e464fd1b632cdb560a2a74291d3b3a82457589d384f8122f8ee27efac91523'}, {'path': 'src/frontend/oir/source/hir_import/candidate.rs', 'bytes': 52501, 'sha256': '5f8836dcb03ca0472e209e883d7b2efbe74c841ee5cade6191acf0beb738f1b9'}, {'path': 'src/frontend/oir/source/hir_import/candidate/cleanup_controls.rs', 'bytes': 8845, 'sha256': '2ab9f21b765b18180ec783567452410e5c44e3ff2c767138724a829899a9a705'}, {'path': 'src/frontend/oir/source/hir_import/candidate/emit_entry_tests.rs', 'bytes': 4179, 'sha256': '109cc52c6680a8681a0bc4592bcbdd59dc9a63914e50a999a5c537c1145c75d1'}, {'path': 'src/frontend/oir/source/hir_import/candidate/emit_terminal.rs', 'bytes': 18799, 'sha256': '771f33f085808da3b7e425fd70dac11e3f579cdd1bd2f1e48deb9364b2adbfcc'}, {'path': 'src/frontend/oir/source/hir_import/candidate/resolution_controls.rs', 'bytes': 6704, 'sha256': '73ae3be2a0252f40e91e52fa028b5d008a2a7359d79495a0645767c92d416eb8'}, {'path': 'src/frontend/oir/source/hir_import/candidate/run_entry_tests.rs', 'bytes': 15361, 'sha256': '204512a95f11a63e8a2205cb24de223051ed1d344a44335b7e53edde7b634c78'}, {'path': 'src/frontend/oir/source/hir_import/candidate/tests.rs', 'bytes': 8490, 'sha256': '9c3d87c7d73089c8b482df8aaa16af920f5ec7dd283d4a9bd308bbb04cdff42d'}, {'path': 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs', 'bytes': 17309, 'sha256': '1002fe2df927b798b9940a7bd0e4bd6ede890eda8d96990545817e8e8ebb189c'}, {'path': 'src/frontend/oir/source/hir_import/candidate/verify_terminal.rs', 'bytes': 24816, 'sha256': 'ba1b8f423f5e5b7ddebdfad0dbf64884a126689f0977e6e7ec10e2cb8e089be7'}, {'path': 'src/frontend/oir/source/hir_import/emit_failure_tests.rs', 'bytes': 10414, 'sha256': '28b1d4d51393757a2f56d639578347f8803a2e1f2b4e5aad0bd06f6d9d9d477c'}, {'path': 'src/frontend/oir/source/hir_import/emit_measurements.rs', 'bytes': 2439, 'sha256': '1a79b3e928116b5aa3dcd7d4d72f6b78fb48fb82437003004c08ea882c951b13'}, {'path': 'src/frontend/oir/source/hir_import/emit_native_tests.rs', 'bytes': 4999, 'sha256': 'b855ca249b3c2015d37102d83b54c6b069c2fd8ffce29b232cec2bff315c982f'}, {'path': 'src/frontend/oir/source/hir_import/emit_resource_tests.rs', 'bytes': 12267, 'sha256': '92920bb913b6c5633b936901cd303be4ef167cf16772406f496953ae233fc8cd'}, {'path': 'src/frontend/oir/source/hir_import/emit_tests.rs', 'bytes': 21087, 'sha256': '1276cb1c28fd593c1410bcebee71eb1af116d682a117a6c10ec59ac76a262763'}, {'path': 'src/frontend/oir/source/hir_import/leaf.rs', 'bytes': 42173, 'sha256': '1cad264331afe067dbcf1a3a3650aa104d5e8517d338453ee39ebff6dc8c35a4'}, {'path': 'src/frontend/oir/source/hir_import/pass_measurements.rs', 'bytes': 5565, 'sha256': '94225eadf94e19ced1a810d7097d9210e4fa31dec5d6e29d7a8127905afc4888'}, {'path': 'src/frontend/oir/source/hir_import/producer_diagnostic.rs', 'bytes': 11979, 'sha256': '97d3b3847285859e14e6f368be1ed3e790fc1c5aa49966349a4a1da7d6522106'}, {'path': 'src/frontend/oir/source/hir_import/producer_diagnostic/tests.rs', 'bytes': 11631, 'sha256': '1347400fa8ce7ecdcb02c9650c652975de0ec475fa167da4dd41ccec665f9b5e'}, {'path': 'src/frontend/oir/source/hir_import/public_facade.rs', 'bytes': 10968, 'sha256': 'd26704aa11b19d48d8305969e9a46fabbc7d6fbda157b9b9aef26b3d64cd545e'}, {'path': 'src/frontend/oir/source/hir_import/run_execution_tests.rs', 'bytes': 13016, 'sha256': '1c114dc4355bb58f71a23bbdbe709ab3cba6a29417cbd3a9089df9dd6f69c605'}, {'path': 'src/frontend/oir/source/hir_import/run_measurements.rs', 'bytes': 2418, 'sha256': 'b679e60c6fe189d9ea0c72b8a0c25e4562cdd65838f97ae9539e49e6f07a79b0'}, {'path': 'src/frontend/oir/source/hir_import/tests.rs', 'bytes': 21006, 'sha256': '827f58abd71375cf7ea528ecaaaa2dae47a2ec13ca9cb2cc5283324640f0d33e'}, {'path': 'src/frontend/oir/source/hir_import/u8_resource_successor.rs', 'bytes': 6181, 'sha256': 'b9dcbd9eb956fc4e092965b5772189f8ef293d8b431503d6bdb80ffc7cb96834'}, {'path': 'src/frontend/oir/source/hir_import/verify_diagnostic_tests.rs', 'bytes': 10322, 'sha256': '3c72074acf7d4520f83b1a1b6e359736bea3fa21845445b870df2f5eb5ebeb09'}, {'path': 'src/frontend/oir/source/hir_import/verify_fact_tests.rs', 'bytes': 16589, 'sha256': '5e3282aef6bc62d0b57d97dfc9173f887719b6967bb9d24e9bd4132a42a47f03'}, {'path': 'src/frontend/oir/source/hir_import/verify_tests.rs', 'bytes': 13809, 'sha256': 'e6bc46cbe24afebdb0028ecf3db60ef8f40ebf4c38da8951971bf87e7a997b21'}, {'path': 'src/frontend/oir/source/sealed.rs', 'bytes': 16522, 'sha256': '5ea4ccb8f1a4779fdea2e13675fdb7c1ca92aad1553e44b65fa0d310cb829b45'}, {'path': 'src/frontend/oir/source/u8_association_resource_proof.md', 'bytes': 8923, 'sha256': '420d26d53d4c8a1c177eeae6e518dd983590d14b2b8fc90ebfd936464966fa2e'}, {'path': 'src/frontend/oir/source_oracle.rs', 'bytes': 15037, 'sha256': '3ec0b5336db5fbfd1fa347b75957afe0ab4f335f649c5fef9de32409547084bc'}, {'path': 'src/frontend/oir/tests.rs', 'bytes': 38333, 'sha256': '6456e1cd19794092624780fcac05a3d5575668a6313e6602d419e18aabef08d9'}, {'path': 'src/frontend/oir/u8_association_tests.rs', 'bytes': 15112, 'sha256': '9b54ae676a470127119fe5eea2058b2a7caf3528386aac65e56d0e7dc6b3c715'}, {'path': 'src/frontend/oir/u8_carrier_tests.rs', 'bytes': 1353, 'sha256': '21bbc7a6121acb41e61ee5da26e573ea511ac418487ba5ce712df8f2809de702'}, {'path': 'src/frontend/oir/u8_execute_tests.rs', 'bytes': 6029, 'sha256': '8c22f2d1e831e7b5d7897ce2935bf64c8b302233bf1bb37e1857b72d278a78f0'}, {'path': 'src/frontend/oir/u8_tests.rs', 'bytes': 15461, 'sha256': '2c12ff82af74324dd210c643896f602f3973eff7741d276d3380b259ec99ef99'}, {'path': 'src/frontend/oir/unary_source_tests.rs', 'bytes': 12485, 'sha256': '8830f5cb0b4193ba2fc7fc25c2b522793153fea6ca479535fe8c54c2ba2e60db'}, {'path': 'src/frontend/oir/verify.rs', 'bytes': 44481, 'sha256': 'f308e84d0234d01023d860126a815150d8bf8612262a7974e0c5db29279be6b9'}, {'path': 'src/frontend/oir/verify_measurement.rs', 'bytes': 15617, 'sha256': '428afda4df3658a7218b135816fd49d7e5a2bf93a77758133dbca7b946f5d582'}, {'path': 'src/frontend/options.rs', 'bytes': 65162, 'sha256': 'e3fe01d971920d6446a8605e6da5d7d81e87a87ab2f451392ca8a5d91737cccf'}, {'path': 'src/frontend/owned_diagnostic.rs', 'bytes': 35472, 'sha256': '1c63c498be8fad2e4a8bb0924ac4ce344757d78f7d715207e62db0839007245e'}, {'path': 'src/frontend/owned_syntax_tests.rs', 'bytes': 14131, 'sha256': '202f4f091697bf761aeced18a08bf6286f9287296709c8cd7e20deec5511a241'}, {'path': 'src/frontend/parser.rs', 'bytes': 72574, 'sha256': 'dac6191d9ec2a292333f047993d021c611959a10f73f4c03f5a5f622dc55f47a'}, {'path': 'src/frontend/parser/activation_tests.rs', 'bytes': 6213, 'sha256': 'ee4e2ee3fae43d34729c382a64839bc864eb5c357b0ce018eceafb920dfd2ede'}, {'path': 'src/frontend/parser/array_syntax_tests.rs', 'bytes': 26623, 'sha256': '0846b8969bf745f72a6c9fe760667bac500b98f41228c7c2be402eac042fe16c'}, {'path': 'src/frontend/parser/arrays.rs', 'bytes': 8450, 'sha256': '62e620c260a4747e38b52d07d0ba5b81335f85917db43691486f31c7075052c7'}, {'path': 'src/frontend/parser/builtin_tests.rs', 'bytes': 4023, 'sha256': '6ea9bae81b7f5975092e34616277d8c195d1583bdaa4ef60a2046baf7dc5cb08'}, {'path': 'src/frontend/parser/conversions.rs', 'bytes': 1530, 'sha256': '52c64d7e00db8bfa72bec277c1091a6cac6731700106c1ca11c4a72879d27f8b'}, {'path': 'src/frontend/parser/enum_syntax_tests.rs', 'bytes': 28137, 'sha256': '5a2126d7d9f29404534e6625d587b16ca2930260c93144200f402d5060c95866'}, {'path': 'src/frontend/parser/enums.rs', 'bytes': 14613, 'sha256': 'b91566987fd8a8b3d3e668deaf525ba5acfe002cb167bcdff25792d00218158f'}, {'path': 'src/frontend/parser/project_tests.rs', 'bytes': 20559, 'sha256': 'c44b66b619286a59cd6ca658281557ef2f755f3b84d787551c08dbe7ff6813a1'}, {'path': 'src/frontend/parser/u8_syntax_tests.rs', 'bytes': 12577, 'sha256': 'c34ab1e2a4ee0d3ca3b7829020252538d19664697a5c4dda3644d6b2ca5e68b0'}, {'path': 'src/frontend/project.rs', 'bytes': 174215, 'sha256': '886590c847516fac273826f0e8f0cf34fe37d66a0c6933d88385e4b288d5df84'}, {'path': 'src/frontend/project/array_syntax_tests.rs', 'bytes': 7105, 'sha256': 'f356caaf5569c956498c65222fe029294a1e7c00f617ffb79cc935ad04c54894'}, {'path': 'src/frontend/project/budget.rs', 'bytes': 9315, 'sha256': '9df928482cffae56585a6ef6486fdc4aa2a99d12e5acb90f37408e21f0fcba11'}, {'path': 'src/frontend/project/budget_real_null_observer.rs', 'bytes': 137261, 'sha256': '95a0c5d2980d646e39689d97a2aef848df32c08ff8b02dcebb4fff7266f1bff3'}, {'path': 'src/frontend/project/budget_string_null_tests.rs', 'bytes': 9478, 'sha256': '4b04c47727c7b09be89d81f84bfb48dc5f70c798338db363f0a57c7337091d25'}, {'path': 'src/frontend/project/builtin_tests.rs', 'bytes': 7129, 'sha256': '32b007d24f1cecc36a71496a74d0ba15f92c9d879350b41856206f57acd975e0'}, {'path': 'src/frontend/project/enum_carrier_tests.rs', 'bytes': 15872, 'sha256': '0398e1ba384d440395c4d596900c18e81a6815f5a0101bde66a46a7433544bd7'}, {'path': 'src/frontend/project/enum_index_tests.rs', 'bytes': 16636, 'sha256': '9760bfe2fce438fcc2579e3a150a518871b7c0b03cb498bda7935212ab79b4cc'}, {'path': 'src/frontend/project/filesystem.rs', 'bytes': 5237, 'sha256': '6e27861bc9440d015f015c23bf31cb76d238392b7712aede89371ca3e85ddcb7'}, {'path': 'src/frontend/project/tests.rs', 'bytes': 30152, 'sha256': 'c4c5b2945a0119f42c50b025e8543a75cf8b1158548d7427c3bb62c3dfbf6023'}, {'path': 'src/frontend/project/unit2_tests.rs', 'bytes': 9753, 'sha256': '36cf09544b605cc140ca0ed5cc811501c97ba8ceed14584ed890a1cf29e05e4f'}, {'path': 'src/frontend/source.rs', 'bytes': 15666, 'sha256': '38a69de7b01cc0d8160079e98d266a83af2feb969796417952663b5a2c69d176'}, {'path': 'src/frontend/stdin_public_tests.rs', 'bytes': 4243, 'sha256': 'a96ff15b38dca1c86b40ae39aad1e187ef946b239f05db0e68cb9a46831281ea'}, {'path': 'src/frontend/typeck.rs', 'bytes': 35009, 'sha256': '5ca324b75669403428a140f1fcf55d6e4e437084d499fc2b0aa43dfd0a9d2ac9'}, {'path': 'src/frontend/typeck_measurement.rs', 'bytes': 9106, 'sha256': '86debeb7bac4deb2c2954e52c4dc984325a7aca4a73c906b53c959871e082d0b'}, {'path': 'src/legacy/syntax.rs', 'bytes': 31532, 'sha256': 'df3f53c48637cc8b828419a09b58ad4b576549b577c3e6e9946b5b47b60e8ecb'}, {'path': 'src/main.ox', 'bytes': 217, 'sha256': '60cf624a39a0cd4d351cfff6fa84f4c00fc960f5ce90c84127732009f0bc0004'}, {'path': 'src/main.rs', 'bytes': 133034, 'sha256': '78b13c07197b184a9d086578cb64baf47f788472d0f69e8333728a6d7c81ebea'}, {'path': 'src/runtime/artifact.rs', 'bytes': 40814, 'sha256': 'e6dee1a888a4fb2b74152f6807564b938632a4f2f4d836f4bfb59a45b669b85f'}, {'path': 'src/runtime/benchmark.rs', 'bytes': 18182, 'sha256': '5a88a610964ac16ae0eea700e20258c76d177d2ad10e96cc1e8ae7cf0577bcee'}, {'path': 'src/runtime/data.rs', 'bytes': 24185, 'sha256': '055db751a03b4c04977d14ff0bd346a1b5d6589c5cddd8a4ad6b4a48a2e63d07'}, {'path': 'src/runtime/network.rs', 'bytes': 43322, 'sha256': '8b54174a9717be3e97d7ceed45c2fc8fa29448bd7fb780cdec6efb85d9c31608'}, {'path': 'src/runtime/packages.rs', 'bytes': 77147, 'sha256': 'acd42add4312cf38a8baea9caab43748f0d89b649dfd83fd1949b339941ccd74'}, {'path': 'stdlib/frontend/bytecode.ox', 'bytes': 3884, 'sha256': '9c4664937e5d04596bdd3ec73234ac2c1eb08fbc97e160a09dba6a2c606a8818'}, {'path': 'tests/fixtures/bounded_enum_scanner/main.ox', 'bytes': 666, 'sha256': '768232ca66cd91a6439cf21393a8666f4ec10bcfac4d097caece354440278c3a'}, {'path': 'tests/fixtures/bounded_enum_scanner/scanner.ox', 'bytes': 1142, 'sha256': 'd13847eaecd69667031e54bb6f3f329365427a483e0bff537d0b28882516ac14'}, {'path': 'tests/fixtures/checked_hir_import/public-source.txt', 'bytes': 117, 'sha256': '5cc21baa00b3555df83a8ac154e2e497052d6a11dac7e2e390a038efe51cf08f'}, {'path': 'tests/fixtures/checked_hir_import/public-success.bin', 'bytes': 2607, 'sha256': '8bd6c66e859cfd0692e18ef7e3ca94f01e6d280eb2aca55849d415b26862cf6c'}, {'path': 'tests/fixtures/checked_hir_import/rich-source.txt', 'bytes': 113, 'sha256': 'ebc49f61b8f3941b8aafdfa779cfdd34577c0c3a2ba0f44bf54c683f656a4522'}, {'path': 'tests/fixtures/checked_hir_import/rich-success.bin', 'bytes': 2607, 'sha256': '399a9cb43dd2e16d3e0eed1c160fb1428371e90f089d72ea23149acfee961f52'}, {'path': 'tests/fixtures/checked_hir_import/scalar-arithmetic-source.txt', 'bytes': 40, 'sha256': '45426a4f69ab245011785fc5ffa2c188ec84d1fa1f92c703978fd133e9677d11'}, {'path': 'tests/fixtures/checked_hir_import/scalar-arithmetic-success.bin', 'bytes': 2607, 'sha256': 'a4f09ea7f3143a3dd0711643e9c345b4612a10047f5fb4e4bfa1710dd47a8bbf'}, {'path': 'tests/fixtures/checked_hir_import/scalar-assignment-source.txt', 'bytes': 66, 'sha256': '069ced25a829737d0a5fa0b22afff5e0ff6bf125f40e97782ac5585969b827dc'}, {'path': 'tests/fixtures/checked_hir_import/scalar-assignment-success.bin', 'bytes': 2607, 'sha256': '67266a6ffcfc8936de9168268ebc97eee08a993b0bf821bb785556e93d9a75f3'}, {'path': 'tests/fixtures/checked_hir_import/scalar-boolean-source.txt', 'bytes': 49, 'sha256': '62d9083f423cf86ee8d3c5cf17328ebdca4eeeb35cf28a1d6c10e345201c98d0'}, {'path': 'tests/fixtures/checked_hir_import/scalar-boolean-success.bin', 'bytes': 2607, 'sha256': '8b5158aad56e24ed5aaf16aacde46c305df5dfff74151dec230d87a67f01da8f'}, {'path': 'tests/fixtures/checked_hir_import/scalar-comparison-source.txt', 'bytes': 65, 'sha256': '4ecd3203de480fd90db18b79ec2fe1488f7d5a9bd16b474b209be4be1b5c4b17'}, {'path': 'tests/fixtures/checked_hir_import/scalar-comparison-success.bin', 'bytes': 2607, 'sha256': '21aa0ebd769e1a512156da0d1c211d30d5ce8376fa0b03dee0130d71301e7ad6'}, {'path': 'tests/fixtures/checked_hir_import/scalar-loop-source.txt', 'bytes': 61, 'sha256': 'ee822fb34204aa7a35cc51a88485884f3e0c333a2ba7120ee4afe514f687873b'}, {'path': 'tests/fixtures/checked_hir_import/scalar-loop-success.bin', 'bytes': 2607, 'sha256': '7db60e88f053890b3c3b28dea3c6fac4f0cae461fbd71b388e07e4742d3ea30c'}, {'path': 'tests/fixtures/checked_hir_import/scalar-unit-source.txt', 'bytes': 40, 'sha256': '6f577d023451219fa71acb94a38b5d0d9a9ca4a881f7d4c43e24544fa1ef2570'}, {'path': 'tests/fixtures/checked_hir_import/scalar-unit-success.bin', 'bytes': 2607, 'sha256': 'f521d526e3565116ada4f5f5a034e2e81295f2e5e63638ebf56ed86632a74812'}, {'path': 'tests/fixtures/checked_hir_import/synthetic-division-source.txt', 'bytes': 27, 'sha256': 'd2c2bc9fa0c23c66e0c143b0f3df1840aadf64809cbd620892ed04e01b5593ba'}, {'path': 'tests/fixtures/checked_hir_import/synthetic-division-success.bin', 'bytes': 2607, 'sha256': 'cad0fa1af44a5a5a147945f09c46d9c3046e802e0a52e9340b0ae2201ce86dbb'}, {'path': 'tests/fixtures/checked_hir_import/synthetic-overflow-source.txt', 'bytes': 36, 'sha256': 'be94cc8f44f475237901d1de3ca8ac9dd9856c88ea8d1bc9874e7319d9553f09'}, {'path': 'tests/fixtures/checked_hir_import/synthetic-overflow-success.bin', 'bytes': 2607, 'sha256': 'a525797bfa202b73a1e09910bf5c2080abb73671e33e6ea9ed29f767a7226313'}, {'path': 'tests/fixtures/checked_hir_import_v2/source-255.txt', 'bytes': 255, 'sha256': 'f616a9ac1025210fa1d282b06aadb8c3088b653154c64afe01d64c8d9c838ee3'}, {'path': 'tests/fixtures/checked_hir_import_v2/success-255.bin', 'bytes': 2607, 'sha256': '09a5ec7761d7da0aed5f8c94d8e374dc577f4cd649aa964c43efa79d58f10f05'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-call-context-excluded/main.ox', 'bytes': 80, 'sha256': '3ba0c58117e88a6f8d8120b16f67aa488db903883dd4de2d451768d85c24e864'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-no-context/main.ox', 'bytes': 45, 'sha256': '32abf2281ee2e873b6833f7cbbcd759dbd49144771080937f36a827a3e0d2c73'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-nonzero-annotation/main.ox', 'bytes': 55, 'sha256': '51891e6f066c88b32d396bd6e6e95392fecafdd030242d24c4b3072615f09ba6'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-reassignment-context-excluded/main.ox', 'bytes': 65, 'sha256': 'e6a4dc72429365d2e2d824527f147ebab94225e8fb5702f42b6df6f74059b901'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-return-context-excluded/main.ox', 'bytes': 69, 'sha256': '83c56501682fae222e3820d099f74259e90f3d2b9990831131bd82d9ac1f56fc'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-scalar-context/main.ox', 'bytes': 50, 'sha256': '89aa766d25bae2f31e85a4e2dd8980fdabf5087992c9de4be6c70c6daf5d321c'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/grouped-complete-access-and-index/main.ox', 'bytes': 86, 'sha256': 'd46eff9703a62da2ddd9fe899f597ee8202aaed2bfaff8ea5a7f17b2ce0c6a36'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-first-heterogeneous-element/main.ox', 'bytes': 54, 'sha256': 'df523d928d4a7477da5256625fbe451ce238b7ab591420e0ef8323c7b057e893'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-length-max-trailing-comma/main.ox', 'bytes': 2097, 'sha256': 'd7399e8bebf8c3b2ef244099c272d67fa2c128d891dd2a796771659a15bbae77'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-nested-nonempty-is-nonscalar/main.ox', 'bytes': 46, 'sha256': '78d77948d26a4f3da2dcf4d9f3cb8832f675df72f02e51e5c0db01697847f167'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/nested-empty-does-not-inherit-context/main.ox', 'bytes': 55, 'sha256': 'a6a002293dd3c4604743cbf65ae0c2b45600207eace37b3de9537fe6cc6d9110'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-array-bad-index/main.ox', 'bytes': 50, 'sha256': '886823f12d8608f6b41dc1b7afd7b57bd7785756c613dc34000d2bbfcdd397fc'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-base-kind-before-index-kind/main.ox', 'bytes': 48, 'sha256': 'd4e236ab70385f6f9a35fb79b0a30a73da0bac4481a39866e7b4fca95b444763'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-internal-type-first/main.ox', 'bytes': 52, 'sha256': 'a87bf6397c591d05f826a5caa13ac2ef00eed7272fa6ac5b89c0750ae50e14f1'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-literal-range-before-base-type/main.ox', 'bytes': 54, 'sha256': '5096b68a9719251d73044315059a6c4b684d72563de11ea0826755d0d14b606b'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-resolution-before-base-type/main.ox', 'bytes': 53, 'sha256': '3d0b69e3d153aa899d8246eb772e204c62b8ccd4ba3d43b42e97ce5ee80435da'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-unknown-base-first/main.ox', 'bytes': 48, 'sha256': '347e55639c69fa58344c565651efbce0c02756f9e10e8b6aa9cc5ebf4c1037d3'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-bool-length-0/main.ox', 'bytes': 64, 'sha256': '74ff8f03050b26a17cc8e65b5f2cac0362f08c3e7463495a577c3230bcf6dba7'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-bool-length-1/main.ox', 'bytes': 65, 'sha256': '12d84b0e19246e1c5a8352c09d18feac105b71080f8e76469ef0622a9bfc7902'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-i32-length-0/main.ox', 'bytes': 63, 'sha256': 'f3c451023455108b03cfdfe280c21d261b9f9d8925c81c42523c028a5799b09e'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-i32-length-1/main.ox', 'bytes': 61, 'sha256': 'c8fe5273fa8927ab53e36e958328995cf60898eb443d40f9120a49405b9512ff'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-0/main.ox', 'bytes': 62, 'sha256': '6d9524db67db301fa9ed33ea58c87e59ed739379f97668c2f21f29a1dddff06d'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-1/main.ox', 'bytes': 61, 'sha256': '5a93cd48bdc89ef5b83a75c27adbe026a3a28e09bbcb622cabd8960c61d70b11'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/structural-identities-pairwise-distinct/main.ox', 'bytes': 139, 'sha256': 'a6e226c08152c3957ca65d7817dfe770d9599491681ed45fd31c11e166b7817d'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/utf8-read-primary/main.ox', 'bytes': 61, 'sha256': '36f08bdef9d688a7be7652c859feec77f178eb00b4633630b2d3b951abc764e8'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/whole-program-resolution-before-earlier-function-type/main.ox', 'bytes': 90, 'sha256': 'c250b4834e3eaf99d9b6279866a884fcd3c74b05e85039d3843d4eda973b250b'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-base-kind-before-index-kind/main.ox', 'bytes': 59, 'sha256': '6fed7af8b18a4a8f0c30361f87dce387d7972bcf3cffcf7fb47b16635264e28c'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-both-subtree-errors-rhs-wins/main.ox', 'bytes': 67, 'sha256': '509cb3c35a9bccbc86ee6120b9fa198d464a7338971aec90d1015bc4e3c71d67'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-element-before-mutability/main.ox', 'bytes': 58, 'sha256': '8f64ca26780e1efc1c6f3342484c95f65d5db38000319114f7b697325d5f6a5e'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-index-internal-type-before-base/main.ox', 'bytes': 63, 'sha256': 'd540d8cfaa277c1c332c9934b508b30d0da9baffeb25df890f0c37d0b73d355c'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-index-kind-before-rhs-element/main.ox', 'bytes': 61, 'sha256': '57750be685c0c6643a6c4057a14df01a86cca4ba74db1c4affdadcd5e9fd8bc4'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-one-conflict-element/main.ox', 'bytes': 62, 'sha256': 'c5f9d7422e93168b8f37d967bc4ee3c85699c82002059bc0d7d1d9779e6608aa'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-one-conflict-index/main.ox', 'bytes': 61, 'sha256': '18a666eaa5eec25909c9de0c839ffebcd862bc8094aeefc3814585bc1a52dd7f'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-owner-mutability-last/main.ox', 'bytes': 54, 'sha256': '7062109c43d146c79791a627277b996c9b8c89d5c1609a894ebb3aecd4fbf876'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-resolve-base-before-both-operands/main.ox', 'bytes': 62, 'sha256': '8807388125d28711c73e6552914843218d6d3c443a61250e9316149f6509cc31'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-resolve-rhs-before-index/main.ox', 'bytes': 73, 'sha256': '082576c5d3fb1835bf6c8e9fb23ace1cdd77671e3d0f804501d0eb94a94e5e0b'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-rhs-internal-type-first/main.ox', 'bytes': 63, 'sha256': '7f7c337f34ef1bf8d18452b1b960aabffaad98273a1d8e30af7d94c97a437e27'}, {'path': 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-whole-resolution-before-rhs-typing/main.ox', 'bytes': 73, 'sha256': '711d3dc384f243f12112f934514a16fcff7f88e83c6aa8a0f98612808e0be158'}, {'path': 'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-array-free/main.ox', 'bytes': 31, 'sha256': 'c53554f73fe3910c5ed769d2129f95c5228ab98058d3e771401d562b6b29eac9'}, {'path': 'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-empty/main.ox', 'bytes': 0, 'sha256': 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'}, {'path': 'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-record-only/main.ox', 'bytes': 20, 'sha256': '7f5b005fd1e8d62c964b7e1d5be5c3f85d571a5d24ac1a4fd3d97e4100bb44a3'}, {'path': 'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/reference-access-modes/main.ox', 'bytes': 174, 'sha256': 'c22dcb52cc3078bf9396fc915bb26e43eb5c91930f93a7ec71b89c51e7dc71a8'}, {'path': 'tests/fixtures/producer_diagnostic/duplicate-source.txt', 'bytes': 31, 'sha256': 'b62fbb273ad1e0a884f4c1e5ddbc45886bc4e962e94ab03761ea9d0e4b40cef2'}, {'path': 'tests/fixtures/producer_diagnostic/duplicate.bin', 'bytes': 1575, 'sha256': 'd6a0707e9f0ed1351602a830166ea4a18823653457962f016913493b61fca38c'}, {'path': 'tests/fixtures/producer_diagnostic/end255-source.txt', 'bytes': 255, 'sha256': '0ea89521240db1e81600391e78d2888f3f13d8c3aa945a2eb9b40577f90ecc73'}, {'path': 'tests/fixtures/producer_diagnostic/end255.bin', 'bytes': 1575, 'sha256': '4f1b818fb3c6036c7e9d7df32dab4a7b176f0d425aeab2652a372ad3b13bbb38'}, {'path': 'tests/fixtures/producer_diagnostic/multiple-source.txt', 'bytes': 48, 'sha256': '25016a9cacca43132e7c5d29f4d2f11a9719d49427b8a5a24c209e5f50cb138c'}, {'path': 'tests/fixtures/producer_diagnostic/multiple.bin', 'bytes': 1575, 'sha256': 'b3fdb49bf2f5da837121d6cdb80ac4a541e71416c6096e73d8e419c84e0e5fd0'}, {'path': 'tests/fixtures/producer_diagnostic/provenance.json', 'bytes': 2584, 'sha256': 'ddf68e898be3895717461388368122f00eb31c3b3b27fd8677e652b263438155'}, {'path': 'tests/fixtures/producer_diagnostic/unknown-type-source.txt', 'bytes': 35, 'sha256': 'a40af9465791100a701df35e87a9f23902eb4a2cfd14985aba05cdf1532c1ff0'}, {'path': 'tests/fixtures/producer_diagnostic/unknown-type.bin', 'bytes': 1575, 'sha256': 'a9445cbcfdf920b50e960c730aa33aa2d74379d93c7f21f04d3348ce6dd7a840'}, {'path': 'tests/fixtures/typed_project_source_binding/README.md', 'bytes': 52715, 'sha256': '94604e559465a310f8177c32fb6bac47c7bb511e20b226f25040ff1dec5b28ef'}, {'path': 'tests/fixtures/typed_project_source_binding/authority.json', 'bytes': 9916, 'sha256': '73e3f96fa48bf3d478c923681108bb119bce4d5eb8441f763d72c29a596e09ce'}, {'path': 'tests/fixtures/typed_project_source_binding/byte-storage-authority.json', 'bytes': 182657, 'sha256': 'e8f678e0dd5b641194a286a6245d80fbe727fb5099ce4b49a426f79da8e66d85'}, {'path': 'tests/fixtures/typed_project_source_binding/byte-storage-source-v1.json', 'bytes': 74192, 'sha256': '402db5018af489c30b2a57ed3ef558c055013af2b727a3ad0eb39ffc42125efa'}, {'path': 'tests/fixtures/typed_project_source_binding/byte-storage-transition.patch', 'bytes': 282373, 'sha256': 'c3d74ac39e441cd1f55f908d6ae57aaa8c1cf686e8deaa005eed29a37e15bb15'}, {'path': 'tests/fixtures/typed_project_source_binding/byte_storage.py', 'bytes': 9896, 'sha256': 'e9eef8a6475d6c79c5e93a127af8ed99e46eb029de4da7909371b44dcd6f6084'}, {'path': 'tests/fixtures/typed_project_source_binding/cache-admission-authority.json', 'bytes': 94920, 'sha256': '07ff6c8427fdc108a2d995a6d94a5788df77e0f85d12892446956eb9d6a43440'}, {'path': 'tests/fixtures/typed_project_source_binding/cache-admission-source.json', 'bytes': 67820, 'sha256': '82cd3f0733ee6b457341e7607ff3883593138aa64b3763e217f0e53ef1662c67'}, {'path': 'tests/fixtures/typed_project_source_binding/cache-admission-transition.patch', 'bytes': 10191, 'sha256': '302fab79c45fb34a41a3a641e44fe0880bbfb765554dfe306bcc07cdf67cc01d'}, {'path': 'tests/fixtures/typed_project_source_binding/cache-preservation-authority.json', 'bytes': 94920, 'sha256': 'e9f62aaa5d6544c999dc857427f6c5c52abb8308502f980be822e2ebf55eae99'}, {'path': 'tests/fixtures/typed_project_source_binding/cache-preservation-source.json', 'bytes': 67558, 'sha256': '481bc1f3f7b68530151d2b0d1e9bed76467b744f9087197320dfe286cbe98102'}, {'path': 'tests/fixtures/typed_project_source_binding/cache-preservation-transition.patch', 'bytes': 6541, 'sha256': 'ea517427d0fa75bf5fbc09aaa0c5ad504cb9e66b747895953716a1137e75f1c1'}, {'path': 'tests/fixtures/typed_project_source_binding/cache_admission.py', 'bytes': 5529, 'sha256': 'f6ef6f7e5dcb01d8f21bdc4911bf7072f401750c971b0d5c6f136935608e0077'}, {'path': 'tests/fixtures/typed_project_source_binding/cache_preservation.py', 'bytes': 5563, 'sha256': 'a2a2a8c65a97eaba7fcfcf709d7430179ebef24c5ac0625e8c226eec74aebaf1'}, {'path': 'tests/fixtures/typed_project_source_binding/combined-authority.json', 'bytes': 19738, 'sha256': 'f28aae703e7f3c1010d91e2f53a4f66a9f728fe5248edf1f4832f12f59d90670'}, {'path': 'tests/fixtures/typed_project_source_binding/combined-source.json', 'bytes': 35021, 'sha256': '221524ad3faf7ea8e8b336cf8497a2eb7e2fbe476a1e829f510b2ee98dc82487'}, {'path': 'tests/fixtures/typed_project_source_binding/combined-transition.patch', 'bytes': 376315, 'sha256': '8ef58e282f1e37a7c04e653222fb74cb40f144aaa4364723773a608df2182683'}, {'path': 'tests/fixtures/typed_project_source_binding/composition-authority.json', 'bytes': 47077, 'sha256': 'f387deb3d73643cf51109e1aee7a59717c12cfe1ac70cd3a6e14f1aafa01ee8a'}, {'path': 'tests/fixtures/typed_project_source_binding/composition-source.json', 'bytes': 37404, 'sha256': 'eff7e18f49b30ebd24a10645f352502211b03de1359127edefc9a43d004f2c16'}, {'path': 'tests/fixtures/typed_project_source_binding/composition-transition.patch', 'bytes': 238025, 'sha256': '5862ed320a9823b20eb1854b888fd2f66d3498cf58289fecac469a087cef09a6'}, {'path': 'tests/fixtures/typed_project_source_binding/current-source.json', 'bytes': 74567, 'sha256': 'd83edc200b9b900668ccc7ee9679a7972e1d497d10acf5f82ccd602680dd04e9'}, {'path': 'tests/fixtures/typed_project_source_binding/division-authority.json', 'bytes': 29723, 'sha256': 'f2a848cf361ba2907d1f1e437256a28c0996189de9228de148f041a0ce9c0164'}, {'path': 'tests/fixtures/typed_project_source_binding/division-source.json', 'bytes': 35161, 'sha256': 'd3f3d2c8dc254bdb2b86381325a943925a39fde0eb2b89a10d1de8e6bfbd7f33'}, {'path': 'tests/fixtures/typed_project_source_binding/division-transition.patch', 'bytes': 49895, 'sha256': '65319908325ce79bd46fb6014b0392b697e3a9d16447562d213dd882a0e2efb1'}, {'path': 'tests/fixtures/typed_project_source_binding/enum-authority.json', 'bytes': 156306, 'sha256': 'e539957635eaa99b1ca806f73ecc04e99a05d7319231f2385c213c63902a9923'}, {'path': 'tests/fixtures/typed_project_source_binding/enum-enabled-qualified-values-v1.json', 'bytes': 9983, 'sha256': 'aad90776af8cda1a305efc9581d8fcbb40e4eedb8da0ca9aef703fc4fd8af425'}, {'path': 'tests/fixtures/typed_project_source_binding/enum-resource-authority.json', 'bytes': 10804, 'sha256': '9e81f265f1cef93ee41ca326580d19f2728133569d4453310a4a086545031527'}, {'path': 'tests/fixtures/typed_project_source_binding/enum-source.json', 'bytes': 45493, 'sha256': '21ebc2e9f7c1b29111b35488334850aa27317bfc2400ad32963c3d7e18a16669'}, {'path': 'tests/fixtures/typed_project_source_binding/enum-transition.patch', 'bytes': 1805505, 'sha256': '57b5f94476c5c419c66dae9dd609de4350a1e224209290221bf80190c8b39204'}, {'path': 'tests/fixtures/typed_project_source_binding/enum_enabled_qualified_values_v1.py', 'bytes': 9510, 'sha256': '999e9f8cd75ae2010a11d20a40c357bf28d42b658d6293e93cb73a36cb665288'}, {'path': 'tests/fixtures/typed_project_source_binding/formatter-authority.json', 'bytes': 1464, 'sha256': 'f060dd4e264a7261517f496176d9d3def438a1e151616e313972db0545f9b4d2'}, {'path': 'tests/fixtures/typed_project_source_binding/formatter-source.json', 'bytes': 23052, 'sha256': '69d89c46f23a99f7dc20911a4054cde7d97a98352d3fc1349e63ee7949ffcf06'}, {'path': 'tests/fixtures/typed_project_source_binding/formatter-transition.patch', 'bytes': 75550, 'sha256': '8e3bb083c6fbf8846a99476a57a80cb19c7163e5ebbabbd7f3e0305f9ac752b4'}, {'path': 'tests/fixtures/typed_project_source_binding/frontend-v2-authority.json', 'bytes': 137275, 'sha256': '597982c5f195a304bcc7bad4c7afba776db8fc40c7286e5ed3f8da6fa55c16f6'}, {'path': 'tests/fixtures/typed_project_source_binding/frontend-v2-source.json', 'bytes': 63969, 'sha256': 'd294963af70126d6415e035f5ef1652c00953a22e6d26d586e0e1c1352cd6c8a'}, {'path': 'tests/fixtures/typed_project_source_binding/frontend-v2-transition.patch', 'bytes': 67003, 'sha256': 'b0fbd3b34584d76c7ff3bc074732b62c9c1b6b381ff1ca4cc88f99f93d964d32'}, {'path': 'tests/fixtures/typed_project_source_binding/frontend_v2.py', 'bytes': 8042, 'sha256': 'c5b514f24e0dc4be32117198f4604d8e1f36b29cecdd17962666ebf3d43a62d7'}, {'path': 'tests/fixtures/typed_project_source_binding/hir-import-authority.json', 'bytes': 158041, 'sha256': 'a4105c042d49b8145393d59482ae1e6b3872cb1de5ef6760fd098c71bb42ce34'}, {'path': 'tests/fixtures/typed_project_source_binding/hir-import-source.json', 'bytes': 62558, 'sha256': '8911a4d5964408ee94c9bb1a108b157e9143405d93118cfcec4ca9e63fd12746'}, {'path': 'tests/fixtures/typed_project_source_binding/hir-import-transition.patch', 'bytes': 656303, 'sha256': 'a3a0b5f8e109e7baf6a9713418ce6bc132a9a2e481be5a6a4f42dee08efb8947'}, {'path': 'tests/fixtures/typed_project_source_binding/hir-producer-authority.json', 'bytes': 130362, 'sha256': 'b6054a027220f74d8a6c5210b033569c51ea2e1ff991c73e70488a548060806d'}, {'path': 'tests/fixtures/typed_project_source_binding/hir-producer-source.json', 'bytes': 63220, 'sha256': '17d7473695424f8ccf570b3cb4129b08d0291650eab45356c66f8b6864a96680'}, {'path': 'tests/fixtures/typed_project_source_binding/hir-producer-transition.patch', 'bytes': 96326, 'sha256': '3ebdfff79cc4b4cc2b8c73c8efdf5b8532af33b2d6375e3e509027e8cd43ecdb'}, {'path': 'tests/fixtures/typed_project_source_binding/lexer-reservation-authority-v1.json', 'bytes': 274321, 'sha256': '90e995e932ea3a0d5697936efef04569886d7d6d2f353a5e032e13e8350cb046'}, {'path': 'tests/fixtures/typed_project_source_binding/lexer-reservation-authority-v2.json', 'bytes': 275010, 'sha256': 'a3853a2ba52b581bf80af53cb7ddd25e990deec87c221902e7b9d88f33b15c9c'}, {'path': 'tests/fixtures/typed_project_source_binding/lexer-reservation-source-v1.json', 'bytes': 74327, 'sha256': 'aa021e6046786300d13b12c22e5cff3f2565b1ae8f739e0698bdad93fd33a633'}, {'path': 'tests/fixtures/typed_project_source_binding/lexer-reservation-source-v2.json', 'bytes': 74328, 'sha256': '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'}, {'path': 'tests/fixtures/typed_project_source_binding/lexer-reservation-transition-v1.patch', 'bytes': 387673, 'sha256': 'a9c58470b2c280a4ecc0bdd253e39354d0ae037048181952a89545fadec80255'}, {'path': 'tests/fixtures/typed_project_source_binding/lexer-reservation-transition-v2.patch', 'bytes': 398900, 'sha256': 'c8cbadeac02f6876b884c36d0d7d00f7dee275480cdc4b99542e10cf0a1b4438'}, {'path': 'tests/fixtures/typed_project_source_binding/lexical-provider-authority.json', 'bytes': 136523, 'sha256': '69ea522493ab027b8dfcb3b1fb158f8dde14e7ff283e85d8e1ae6c29dc256a02'}, {'path': 'tests/fixtures/typed_project_source_binding/lexical-provider-source.json', 'bytes': 67100, 'sha256': '952c7cf86d2be1036781155d38f81af8854c0fb26487c4dfa1dc24bc575309db'}, {'path': 'tests/fixtures/typed_project_source_binding/lexical-provider-transition.patch', 'bytes': 156583, 'sha256': '400270e547f1214abd9885d724641427fb8b0871f69f25176818446218d0d458'}, {'path': 'tests/fixtures/typed_project_source_binding/lexical_provider.py', 'bytes': 9638, 'sha256': '7fde52366254948674a447063b028ca1277bec5671a32f2a0500c2ad7871ad4c'}, {'path': 'tests/fixtures/typed_project_source_binding/native-inventory-authority.json', 'bytes': 104690, 'sha256': '7c8edaeea1a69abce66b40f7b59bd29584c4927584fbb3f05a633b0ebdb8ca58'}, {'path': 'tests/fixtures/typed_project_source_binding/native-inventory-source.json', 'bytes': 51412, 'sha256': '52eeeb97c2b13d04315bcc0eac68995c0587ade263078ca7adf944d9be92f842'}, {'path': 'tests/fixtures/typed_project_source_binding/native-inventory-transition.patch', 'bytes': 61893, 'sha256': '4be68264904f4c059d98f49fb86de16dc9f9af6332801a175b0e118bf2277f76'}, {'path': 'tests/fixtures/typed_project_source_binding/native-storage-authority.json', 'bytes': 102974, 'sha256': '1124d02c4aa34b6c6caf31bfac47332ea9938294d8813ae49d42a975e3bce4da'}, {'path': 'tests/fixtures/typed_project_source_binding/native-storage-source.json', 'bytes': 50842, 'sha256': '0a4d6471f394e42e0a584cadab2c758190c25b79fb2e49bebde99aae06884303'}, {'path': 'tests/fixtures/typed_project_source_binding/native-storage-transition.patch', 'bytes': 53819, 'sha256': '9e2260d93e908363833cd114902a6f73aa54a5f2528a92d98fa317e26240565c'}, {'path': 'tests/fixtures/typed_project_source_binding/package-integrity-authority.json', 'bytes': 94900, 'sha256': 'f5a44432927c0e84d58226fcb988860098b2c44d9fe01e4874a771632f89f724'}, {'path': 'tests/fixtures/typed_project_source_binding/package-integrity-source.json', 'bytes': 67336, 'sha256': '4d114bbb9b375de743bc18508ebcb48301604f5b417ca0b44d788bf22189c99f'}, {'path': 'tests/fixtures/typed_project_source_binding/package-integrity-transition.patch', 'bytes': 16227, 'sha256': '79c53c652528fe770939ba42dd7a478a309b04555e398d33298bc96644316a49'}, {'path': 'tests/fixtures/typed_project_source_binding/package-manifest.json', 'bytes': 16025, 'sha256': '0981e12abf0c8b171cfe8058a6cc43a97e62eda19a4d0d9a522c7c03de271355'}, {'path': 'tests/fixtures/typed_project_source_binding/package_integrity.py', 'bytes': 5445, 'sha256': '263e898f966fbc1cb007dfab86a0b16504a2a65d11c4edcdb3cbbbcba34af857'}, {'path': 'tests/fixtures/typed_project_source_binding/predecessor-source.json', 'bytes': 22314, 'sha256': '7c3de8673eca2bf2267251a9b3235a123bcefb1538785f3400a1fa0d073c5bb8'}, {'path': 'tests/fixtures/typed_project_source_binding/producer-diagnostic-authority.json', 'bytes': 135620, 'sha256': 'fa52793cd0717dbf2bf44b819afc32258e257cb0522b0f503d8d7cefbfea0dfb'}, {'path': 'tests/fixtures/typed_project_source_binding/producer-diagnostic-source.json', 'bytes': 66021, 'sha256': '35e7e43cb1ef5de8be0c1a78d9e5ac70b1a2caf2efe1e37ba05b7445916c2e29'}, {'path': 'tests/fixtures/typed_project_source_binding/producer-diagnostic-transition.patch', 'bytes': 37700, 'sha256': 'c344823854314d8569d04055cd14031d2b6cd48cd95b8ea6a8301027af0ffe7e'}, {'path': 'tests/fixtures/typed_project_source_binding/producer_diagnostic.py', 'bytes': 10369, 'sha256': '15afd2ed29427665fbbad206efc00d1abf8ba0fd214bb98913d7f2d414db5823'}, {'path': 'tests/fixtures/typed_project_source_binding/projected-authority.json', 'bytes': 48552, 'sha256': 'f3d9ea09236fd17532cf896ae8df510945a93f5d7b576295cfe05c030d809bdc'}, {'path': 'tests/fixtures/typed_project_source_binding/projected-source.json', 'bytes': 38636, 'sha256': '850555bcc78b355029ed2ff0a4a094762f0ea4c0c5bcf5f728d30bbbcc213304'}, {'path': 'tests/fixtures/typed_project_source_binding/projected-transition.patch', 'bytes': 124986, 'sha256': '55d60b92bc3cb828a4cb1ddb610ef74bc10afa65e95a7668476048b1fb2ecf73'}, {'path': 'tests/fixtures/typed_project_source_binding/published-formatter-authority-v1.json', 'bytes': 265133, 'sha256': 'dac2e66cf02e2d6d5c14531b30de8cda88696676747509419a8478e6ed871c43'}, {'path': 'tests/fixtures/typed_project_source_binding/published-formatter-source-v1.json', 'bytes': 74567, 'sha256': 'd83edc200b9b900668ccc7ee9679a7972e1d497d10acf5f82ccd602680dd04e9'}, {'path': 'tests/fixtures/typed_project_source_binding/published-formatter-transition-v1.patch', 'bytes': 1522, 'sha256': 'c6163a923cf56bde945d753628bc6f2c7c4ad717affd35e8bafe7a0c195bccd6'}, {'path': 'tests/fixtures/typed_project_source_binding/published-main-source-proof-v1.json', 'bytes': 219557, 'sha256': '854ab1a1f5ad1075b836d7ea7387e7692b0adae59cd709921e6a8eae17770819'}, {'path': 'tests/fixtures/typed_project_source_binding/run.py', 'bytes': 6721, 'sha256': '2850ee1b67698494d5f8aa5c13dc457c7b927eb5f680abffff917be2496439ce'}, {'path': 'tests/fixtures/typed_project_source_binding/slices-authority.json', 'bytes': 55961, 'sha256': '2e8dc2ab5506e179ffe5628e8a46eb6ec362ddb2e26a8a007800eb7029f3069f'}, {'path': 'tests/fixtures/typed_project_source_binding/slices-source.json', 'bytes': 35876, 'sha256': 'f3fcde4169957c850dfe14491b0ddc4fcc6e75ac0ba81fccb4b3ebe9041c6660'}, {'path': 'tests/fixtures/typed_project_source_binding/slices-transition.patch', 'bytes': 134654, 'sha256': '7e41c881086ab6816d302177aad5ea580547a7577ff1e0c0055843f59ba4de20'}, {'path': 'tests/fixtures/typed_project_source_binding/source-transition.patch', 'bytes': 605300, 'sha256': '63055a4b1a2cb63ce6a160a53e5c8131c4c288c198cd9af6ea421b5c2931fc18'}, {'path': 'tests/fixtures/typed_project_source_binding/stdin-authority.json', 'bytes': 150228, 'sha256': 'ff9f806e0211367c8c31d1084ce5aa80f3175b0e65c54a0d3860df0ced8cac08'}, {'path': 'tests/fixtures/typed_project_source_binding/stdin-source.json', 'bytes': 48300, 'sha256': 'bad88720c3002658bbc85de8cc50f63d88186df2871ee5a03ea8a7da0722d13f'}, {'path': 'tests/fixtures/typed_project_source_binding/stdin-transition.patch', 'bytes': 429695, 'sha256': '3bebb1cb45dab0cc5a24c6d1f7aac0b011ef543f35cab984b51fa2dd91e518e8'}, {'path': 'tests/fixtures/typed_project_source_binding/stdout-authority.json', 'bytes': 132738, 'sha256': '3015dfb1237578b4903b5a865f3ae7daf7819485c3399c2bfd589b8749d4066e'}, {'path': 'tests/fixtures/typed_project_source_binding/stdout-source.json', 'bytes': 50310, 'sha256': '3ae8ee6cbaf6697f0735fcf4e0cb345724d76c2bae6f5046fdfb441d02ecc936'}, {'path': 'tests/fixtures/typed_project_source_binding/stdout-transition.patch', 'bytes': 527951, 'sha256': '80dad62cc0e9fa87b026753401b3e5bb59e0bc200696b1fa825676cd662fb1ea'}, {'path': 'tests/fixtures/typed_project_source_binding/u8-authority.json', 'bytes': 221718, 'sha256': '407d3319bbc2dc404d65768cbe117d314dc486eb5b99169e0104c5536c029cda'}, {'path': 'tests/fixtures/typed_project_source_binding/u8-cross-host-authority.json', 'bytes': 160226, 'sha256': 'c602ed97acae3437929069a2aa01244de9d54a9611167e0abefa289f6aa3c26c'}, {'path': 'tests/fixtures/typed_project_source_binding/u8-cross-host-source.json', 'bytes': 71510, 'sha256': '20f13e26a80cc55fd1e76ee76f8ec10988e9723d644dbc9a474fa7bcf6d4e0a4'}, {'path': 'tests/fixtures/typed_project_source_binding/u8-cross-host-transition.patch', 'bytes': 17278, 'sha256': '8f62ca183f15292ef1cf043e5799a5e36d5a7c9b16fa95eb5a0b710e833e35a6'}, {'path': 'tests/fixtures/typed_project_source_binding/u8-source.json', 'bytes': 71254, 'sha256': '35ee91911bb62c38c831aecb97c918bd14d9516013f5da3e62f445a1153e1cc4'}, {'path': 'tests/fixtures/typed_project_source_binding/u8-transition.patch', 'bytes': 484704, 'sha256': '133d3a599b7c95929d4b691b7390645d0fca44d8fe0fff40e1fbc79c11091d27'}, {'path': 'tests/fixtures/typed_project_source_binding/u8_cross_host.py', 'bytes': 7911, 'sha256': '7664dc9ff6597c3c586154af3b3a38f095147d025b8fd6e544f6c7a93f250ff8'}, {'path': 'tests/fixtures/typed_project_source_binding/u8_source.py', 'bytes': 13829, 'sha256': 'a89918a6016a008a44ad0e0c8afe8771d225ee3fb8cf3ea56c32acc5ac3922ec'}, {'path': 'tests/fixtures/typed_project_source_binding/unary-authority.json', 'bytes': 36993, 'sha256': 'ed2d16dd5b24a55005e399630f3ad7402017fca9e8615b98d232d273ec418a31'}, {'path': 'tests/fixtures/typed_project_source_binding/unary-source.json', 'bytes': 38090, 'sha256': 'd9a1e93d59a479f3965b6770257583ec66e06c98a5a74063f7fe29db17df5220'}, {'path': 'tests/fixtures/typed_project_source_binding/unary-transition.patch', 'bytes': 72327, 'sha256': '4a1e4bfa577ff02bb3c5320eb1994b281831929692acd347daf243b15ae31796'}, {'path': 'tests/fixtures/typed_project_source_binding/unit2-u8-resource-authority.json', 'bytes': 5336, 'sha256': '9cad87cddf974c53b94bb5883884fd895ca50eef6ea939b300dcbcd09930d612'}, {'path': 'tests/fixtures/typed_project_source_binding/unit2_u8_resource.py', 'bytes': 12758, 'sha256': 'eb7c611e986a46d8468edc57a51b7ba708dbac81eb7d0a05bde190f2a530394b'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/README.md', 'bytes': 51601, 'sha256': '28c6a635918a60c45a01dcfcd63f109fa2702f512d27a5a6e5409ee1b3e5e3bb'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/authority.json', 'bytes': 9916, 'sha256': '73e3f96fa48bf3d478c923681108bb119bce4d5eb8441f763d72c29a596e09ce'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/byte-storage-authority.json', 'bytes': 182657, 'sha256': 'e8f678e0dd5b641194a286a6245d80fbe727fb5099ce4b49a426f79da8e66d85'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/byte-storage-transition.patch', 'bytes': 282373, 'sha256': 'c3d74ac39e441cd1f55f908d6ae57aaa8c1cf686e8deaa005eed29a37e15bb15'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/byte_storage.py', 'bytes': 9896, 'sha256': 'e9eef8a6475d6c79c5e93a127af8ed99e46eb029de4da7909371b44dcd6f6084'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/cache-admission-authority.json', 'bytes': 94920, 'sha256': '07ff6c8427fdc108a2d995a6d94a5788df77e0f85d12892446956eb9d6a43440'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/cache-admission-source.json', 'bytes': 67820, 'sha256': '82cd3f0733ee6b457341e7607ff3883593138aa64b3763e217f0e53ef1662c67'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/cache-admission-transition.patch', 'bytes': 10191, 'sha256': '302fab79c45fb34a41a3a641e44fe0880bbfb765554dfe306bcc07cdf67cc01d'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/cache-preservation-authority.json', 'bytes': 94920, 'sha256': 'e9f62aaa5d6544c999dc857427f6c5c52abb8308502f980be822e2ebf55eae99'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/cache-preservation-source.json', 'bytes': 67558, 'sha256': '481bc1f3f7b68530151d2b0d1e9bed76467b744f9087197320dfe286cbe98102'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/cache-preservation-transition.patch', 'bytes': 6541, 'sha256': 'ea517427d0fa75bf5fbc09aaa0c5ad504cb9e66b747895953716a1137e75f1c1'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/cache_admission.py', 'bytes': 5529, 'sha256': 'f6ef6f7e5dcb01d8f21bdc4911bf7072f401750c971b0d5c6f136935608e0077'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/cache_preservation.py', 'bytes': 5563, 'sha256': 'a2a2a8c65a97eaba7fcfcf709d7430179ebef24c5ac0625e8c226eec74aebaf1'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/combined-authority.json', 'bytes': 19738, 'sha256': 'f28aae703e7f3c1010d91e2f53a4f66a9f728fe5248edf1f4832f12f59d90670'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/combined-source.json', 'bytes': 35021, 'sha256': '221524ad3faf7ea8e8b336cf8497a2eb7e2fbe476a1e829f510b2ee98dc82487'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/combined-transition.patch', 'bytes': 376315, 'sha256': '8ef58e282f1e37a7c04e653222fb74cb40f144aaa4364723773a608df2182683'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/composition-authority.json', 'bytes': 47077, 'sha256': 'f387deb3d73643cf51109e1aee7a59717c12cfe1ac70cd3a6e14f1aafa01ee8a'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/composition-source.json', 'bytes': 37404, 'sha256': 'eff7e18f49b30ebd24a10645f352502211b03de1359127edefc9a43d004f2c16'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/composition-transition.patch', 'bytes': 238025, 'sha256': '5862ed320a9823b20eb1854b888fd2f66d3498cf58289fecac469a087cef09a6'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/current-source.json', 'bytes': 74192, 'sha256': '402db5018af489c30b2a57ed3ef558c055013af2b727a3ad0eb39ffc42125efa'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/division-authority.json', 'bytes': 29723, 'sha256': 'f2a848cf361ba2907d1f1e437256a28c0996189de9228de148f041a0ce9c0164'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/division-source.json', 'bytes': 35161, 'sha256': 'd3f3d2c8dc254bdb2b86381325a943925a39fde0eb2b89a10d1de8e6bfbd7f33'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/division-transition.patch', 'bytes': 49895, 'sha256': '65319908325ce79bd46fb6014b0392b697e3a9d16447562d213dd882a0e2efb1'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/enum-authority.json', 'bytes': 156306, 'sha256': 'e539957635eaa99b1ca806f73ecc04e99a05d7319231f2385c213c63902a9923'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/enum-enabled-qualified-values-v1.json', 'bytes': 9983, 'sha256': 'aad90776af8cda1a305efc9581d8fcbb40e4eedb8da0ca9aef703fc4fd8af425'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/enum-resource-authority.json', 'bytes': 10804, 'sha256': '9e81f265f1cef93ee41ca326580d19f2728133569d4453310a4a086545031527'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/enum-source.json', 'bytes': 45493, 'sha256': '21ebc2e9f7c1b29111b35488334850aa27317bfc2400ad32963c3d7e18a16669'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/enum-transition.patch', 'bytes': 1805505, 'sha256': '57b5f94476c5c419c66dae9dd609de4350a1e224209290221bf80190c8b39204'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/enum_enabled_qualified_values_v1.py', 'bytes': 9510, 'sha256': '999e9f8cd75ae2010a11d20a40c357bf28d42b658d6293e93cb73a36cb665288'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/formatter-authority.json', 'bytes': 1464, 'sha256': 'f060dd4e264a7261517f496176d9d3def438a1e151616e313972db0545f9b4d2'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/formatter-source.json', 'bytes': 23052, 'sha256': '69d89c46f23a99f7dc20911a4054cde7d97a98352d3fc1349e63ee7949ffcf06'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/formatter-transition.patch', 'bytes': 75550, 'sha256': '8e3bb083c6fbf8846a99476a57a80cb19c7163e5ebbabbd7f3e0305f9ac752b4'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/frontend-v2-authority.json', 'bytes': 137275, 'sha256': '597982c5f195a304bcc7bad4c7afba776db8fc40c7286e5ed3f8da6fa55c16f6'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/frontend-v2-source.json', 'bytes': 63969, 'sha256': 'd294963af70126d6415e035f5ef1652c00953a22e6d26d586e0e1c1352cd6c8a'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/frontend-v2-transition.patch', 'bytes': 67003, 'sha256': 'b0fbd3b34584d76c7ff3bc074732b62c9c1b6b381ff1ca4cc88f99f93d964d32'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/frontend_v2.py', 'bytes': 8042, 'sha256': 'c5b514f24e0dc4be32117198f4604d8e1f36b29cecdd17962666ebf3d43a62d7'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/hir-import-authority.json', 'bytes': 158041, 'sha256': 'a4105c042d49b8145393d59482ae1e6b3872cb1de5ef6760fd098c71bb42ce34'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/hir-import-source.json', 'bytes': 62558, 'sha256': '8911a4d5964408ee94c9bb1a108b157e9143405d93118cfcec4ca9e63fd12746'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/hir-import-transition.patch', 'bytes': 656303, 'sha256': 'a3a0b5f8e109e7baf6a9713418ce6bc132a9a2e481be5a6a4f42dee08efb8947'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/hir-producer-authority.json', 'bytes': 130362, 'sha256': 'b6054a027220f74d8a6c5210b033569c51ea2e1ff991c73e70488a548060806d'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/hir-producer-source.json', 'bytes': 63220, 'sha256': '17d7473695424f8ccf570b3cb4129b08d0291650eab45356c66f8b6864a96680'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/hir-producer-transition.patch', 'bytes': 96326, 'sha256': '3ebdfff79cc4b4cc2b8c73c8efdf5b8532af33b2d6375e3e509027e8cd43ecdb'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/lexical-provider-authority.json', 'bytes': 136523, 'sha256': '69ea522493ab027b8dfcb3b1fb158f8dde14e7ff283e85d8e1ae6c29dc256a02'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/lexical-provider-source.json', 'bytes': 67100, 'sha256': '952c7cf86d2be1036781155d38f81af8854c0fb26487c4dfa1dc24bc575309db'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/lexical-provider-transition.patch', 'bytes': 156583, 'sha256': '400270e547f1214abd9885d724641427fb8b0871f69f25176818446218d0d458'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/lexical_provider.py', 'bytes': 9638, 'sha256': '7fde52366254948674a447063b028ca1277bec5671a32f2a0500c2ad7871ad4c'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/native-inventory-authority.json', 'bytes': 104690, 'sha256': '7c8edaeea1a69abce66b40f7b59bd29584c4927584fbb3f05a633b0ebdb8ca58'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/native-inventory-source.json', 'bytes': 51412, 'sha256': '52eeeb97c2b13d04315bcc0eac68995c0587ade263078ca7adf944d9be92f842'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/native-inventory-transition.patch', 'bytes': 61893, 'sha256': '4be68264904f4c059d98f49fb86de16dc9f9af6332801a175b0e118bf2277f76'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/native-storage-authority.json', 'bytes': 102974, 'sha256': '1124d02c4aa34b6c6caf31bfac47332ea9938294d8813ae49d42a975e3bce4da'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/native-storage-source.json', 'bytes': 50842, 'sha256': '0a4d6471f394e42e0a584cadab2c758190c25b79fb2e49bebde99aae06884303'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/native-storage-transition.patch', 'bytes': 53819, 'sha256': '9e2260d93e908363833cd114902a6f73aa54a5f2528a92d98fa317e26240565c'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/package-integrity-authority.json', 'bytes': 94900, 'sha256': 'f5a44432927c0e84d58226fcb988860098b2c44d9fe01e4874a771632f89f724'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/package-integrity-source.json', 'bytes': 67336, 'sha256': '4d114bbb9b375de743bc18508ebcb48301604f5b417ca0b44d788bf22189c99f'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/package-integrity-transition.patch', 'bytes': 16227, 'sha256': '79c53c652528fe770939ba42dd7a478a309b04555e398d33298bc96644316a49'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/package-manifest.json', 'bytes': 14146, 'sha256': 'c68a80705e43c2711d492d68fdbf63213e7d7b7f02dd1288c12403382ad3208c'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/package_integrity.py', 'bytes': 5445, 'sha256': '263e898f966fbc1cb007dfab86a0b16504a2a65d11c4edcdb3cbbbcba34af857'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/predecessor-source.json', 'bytes': 22314, 'sha256': '7c3de8673eca2bf2267251a9b3235a123bcefb1538785f3400a1fa0d073c5bb8'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/producer-diagnostic-authority.json', 'bytes': 135620, 'sha256': 'fa52793cd0717dbf2bf44b819afc32258e257cb0522b0f503d8d7cefbfea0dfb'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/producer-diagnostic-source.json', 'bytes': 66021, 'sha256': '35e7e43cb1ef5de8be0c1a78d9e5ac70b1a2caf2efe1e37ba05b7445916c2e29'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/producer-diagnostic-transition.patch', 'bytes': 37700, 'sha256': 'c344823854314d8569d04055cd14031d2b6cd48cd95b8ea6a8301027af0ffe7e'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/producer_diagnostic.py', 'bytes': 10369, 'sha256': '15afd2ed29427665fbbad206efc00d1abf8ba0fd214bb98913d7f2d414db5823'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/projected-authority.json', 'bytes': 48552, 'sha256': 'f3d9ea09236fd17532cf896ae8df510945a93f5d7b576295cfe05c030d809bdc'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/projected-source.json', 'bytes': 38636, 'sha256': '850555bcc78b355029ed2ff0a4a094762f0ea4c0c5bcf5f728d30bbbcc213304'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/projected-transition.patch', 'bytes': 124986, 'sha256': '55d60b92bc3cb828a4cb1ddb610ef74bc10afa65e95a7668476048b1fb2ecf73'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py', 'bytes': 216279, 'sha256': '584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/slices-authority.json', 'bytes': 55961, 'sha256': '2e8dc2ab5506e179ffe5628e8a46eb6ec362ddb2e26a8a007800eb7029f3069f'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/slices-source.json', 'bytes': 35876, 'sha256': 'f3fcde4169957c850dfe14491b0ddc4fcc6e75ac0ba81fccb4b3ebe9041c6660'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/slices-transition.patch', 'bytes': 134654, 'sha256': '7e41c881086ab6816d302177aad5ea580547a7577ff1e0c0055843f59ba4de20'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/source-transition.patch', 'bytes': 605300, 'sha256': '63055a4b1a2cb63ce6a160a53e5c8131c4c288c198cd9af6ea421b5c2931fc18'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/stdin-authority.json', 'bytes': 150228, 'sha256': 'ff9f806e0211367c8c31d1084ce5aa80f3175b0e65c54a0d3860df0ced8cac08'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/stdin-source.json', 'bytes': 48300, 'sha256': 'bad88720c3002658bbc85de8cc50f63d88186df2871ee5a03ea8a7da0722d13f'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/stdin-transition.patch', 'bytes': 429695, 'sha256': '3bebb1cb45dab0cc5a24c6d1f7aac0b011ef543f35cab984b51fa2dd91e518e8'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/stdout-authority.json', 'bytes': 132738, 'sha256': '3015dfb1237578b4903b5a865f3ae7daf7819485c3399c2bfd589b8749d4066e'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/stdout-source.json', 'bytes': 50310, 'sha256': '3ae8ee6cbaf6697f0735fcf4e0cb345724d76c2bae6f5046fdfb441d02ecc936'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/stdout-transition.patch', 'bytes': 527951, 'sha256': '80dad62cc0e9fa87b026753401b3e5bb59e0bc200696b1fa825676cd662fb1ea'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/u8-authority.json', 'bytes': 221718, 'sha256': '407d3319bbc2dc404d65768cbe117d314dc486eb5b99169e0104c5536c029cda'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/u8-cross-host-authority.json', 'bytes': 160226, 'sha256': 'c602ed97acae3437929069a2aa01244de9d54a9611167e0abefa289f6aa3c26c'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/u8-cross-host-source.json', 'bytes': 71510, 'sha256': '20f13e26a80cc55fd1e76ee76f8ec10988e9723d644dbc9a474fa7bcf6d4e0a4'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/u8-cross-host-transition.patch', 'bytes': 17278, 'sha256': '8f62ca183f15292ef1cf043e5799a5e36d5a7c9b16fa95eb5a0b710e833e35a6'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/u8-source.json', 'bytes': 71254, 'sha256': '35ee91911bb62c38c831aecb97c918bd14d9516013f5da3e62f445a1153e1cc4'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/u8-transition.patch', 'bytes': 484704, 'sha256': '133d3a599b7c95929d4b691b7390645d0fca44d8fe0fff40e1fbc79c11091d27'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/u8_cross_host.py', 'bytes': 7911, 'sha256': '7664dc9ff6597c3c586154af3b3a38f095147d025b8fd6e544f6c7a93f250ff8'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/u8_source.py', 'bytes': 13829, 'sha256': 'a89918a6016a008a44ad0e0c8afe8771d225ee3fb8cf3ea56c32acc5ac3922ec'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/unary-authority.json', 'bytes': 36993, 'sha256': 'ed2d16dd5b24a55005e399630f3ad7402017fca9e8615b98d232d273ec418a31'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/unary-source.json', 'bytes': 38090, 'sha256': 'd9a1e93d59a479f3965b6770257583ec66e06c98a5a74063f7fe29db17df5220'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/unary-transition.patch', 'bytes': 72327, 'sha256': '4a1e4bfa577ff02bb3c5320eb1994b281831929692acd347daf243b15ae31796'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/unit2-u8-resource-authority.json', 'bytes': 5336, 'sha256': '9cad87cddf974c53b94bb5883884fd895ca50eef6ea939b300dcbcd09930d612'}, {'path': 'tests/fixtures/typed_project_source_binding_byte_storage_v1/unit2_u8_resource.py', 'bytes': 12758, 'sha256': 'eb7c611e986a46d8468edc57a51b7ba708dbac81eb7d0a05bde190f2a530394b'}, {'path': 'tests/fixtures/typed_project_unit2_independent/README.md', 'bytes': 4477, 'sha256': 'c61f9d715aa2f9b7cdda63e89251338fe7bac20a5f48b9bdf094ae5f466e8076'}, {'path': 'tests/fixtures/typed_project_unit2_independent/archive/resource/artifact-manifest.json', 'bytes': 20554, 'sha256': '80e158846079cb13a98b4347276249c588ef851cbb0f05199b0658b53ac8b81f'}, {'path': 'tests/fixtures/typed_project_unit2_independent/archive/resource/exposure-origin-review-test.rs', 'bytes': 1522, 'sha256': 'dea66e57c2051e94f1a2bfef8c3a6bee65855653bd1f5ab6b8cbbad314396832'}, {'path': 'tests/fixtures/typed_project_unit2_independent/archive/resource/old-admission-review-test.rs', 'bytes': 793, 'sha256': 'deb1dd6bc59707a716830647b3aaa3dedc11c4a12007ff250d8efdb9124eed09'}, {'path': 'tests/fixtures/typed_project_unit2_independent/archive/resource/parser-resource-review-tests.rs', 'bytes': 1951, 'sha256': '5a26942d2864979a430b28aa5714ceea43edf8e80cf9a6f7c789d66c12a11b73'}, {'path': 'tests/fixtures/typed_project_unit2_independent/archive/resource/prepare-candidate.py', 'bytes': 2019, 'sha256': '78f3c1ae1ae413b00a56d0db4021c26b321fbb928eaa3eaf2024dacff75dc582'}, {'path': 'tests/fixtures/typed_project_unit2_independent/archive/resource/publication-resource-proof.md', 'bytes': 9073, 'sha256': '54e50faf0ddcc45c61ee3a6ebc9bd1ab9de36c561cbd4c7e6f22f8ac4b15828c'}, {'path': 'tests/fixtures/typed_project_unit2_independent/archive/resource/resource-review-tests.rs', 'bytes': 22687, 'sha256': 'a5472f85b6dca1ed9e649c96575b0cf8a45fe37f204040806cd0747a237f1669'}, {'path': 'tests/fixtures/typed_project_unit2_independent/archive/resource/run-resource-review.py', 'bytes': 3736, 'sha256': '8185ffc1e7d4be6362a2853bcd1fe8a7c08a3a99c2a94ae2e1bfe015bb2d0f9b'}, {'path': 'tests/fixtures/typed_project_unit2_independent/archive/resource/source-identity-review-tests.rs', 'bytes': 834, 'sha256': 'e6663812028e2eeff9ab1e23929784e9bfd15fa3ac965524be1a016b6e46dfd5'}, {'path': 'tests/fixtures/typed_project_unit2_independent/archive/resource/supplemental-resource-review-tests.rs', 'bytes': 2177, 'sha256': 'cbc394c383b39962e1ca088d81a4ac671676b5a4023730415263584eed39c819'}, {'path': 'tests/fixtures/typed_project_unit2_independent/archive/semantic/compare_observations.py', 'bytes': 5623, 'sha256': '5688107196336a19932a0d9445f3927b47841b56aaea5ed4d17f94afee0e7980'}, {'path': 'tests/fixtures/typed_project_unit2_independent/archive/semantic/normalize.py', 'bytes': 12697, 'sha256': '1b48ff9de1ca56963fe648290c1f518842f4e633c4c0643abc22f030e4fb2c06'}, {'path': 'tests/fixtures/typed_project_unit2_independent/archive/semantic/observer.rs', 'bytes': 13965, 'sha256': 'f2403aace53b6255a94b8b3ec0290db025638c5af94571d5729355672681fb00'}, {'path': 'tests/fixtures/typed_project_unit2_independent/archive/semantic/portable-README.md', 'bytes': 3108, 'sha256': 'dc3b176f8f888fa8b26bdce51cce6d07536175142f7d702a729b1c1b6a8f8c00'}, {'path': 'tests/fixtures/typed_project_unit2_independent/archive/semantic/read-source-callback.patch', 'bytes': 474, 'sha256': 'ad05189f79f5c8c50dab7aa0b01147159f8bbfc37e161a75b0a5304c7ea1f2fd'}, {'path': 'tests/fixtures/typed_project_unit2_independent/assemble.py', 'bytes': 3639, 'sha256': '9e57060897d54e447e584c255f9ff82f30f10f95f57a2b628ed6b1a38513c55c'}, {'path': 'tests/fixtures/typed_project_unit2_independent/export-deltas.json', 'bytes': 538, 'sha256': '09e59f5c8c7d6bdd644602c1cd6b7f6b1f45ee8fa4a0d92de99d19f073301d7c'}, {'path': 'tests/fixtures/typed_project_unit2_independent/package-inputs.json', 'bytes': 5836, 'sha256': '4dae6a8de321b09d972d7dc345d19001332fe397ed05ccd3edaaf4f38a5db916'}, {'path': 'tests/fixtures/typed_project_unit2_independent/protocol.py', 'bytes': 5033, 'sha256': '01988929b7051f72762efb59e0c3e78e9dd22372b00bd58efdaa10d149b8eddc'}, {'path': 'tests/fixtures/typed_project_unit2_independent/resource-test-names.json', 'bytes': 2593, 'sha256': '8bc208a6971dc9a1492d74462bf82a19019619b1bd3c86e2299b419c5e1427c1'}, {'path': 'tests/fixtures/typed_project_unit2_independent/run.py', 'bytes': 11951, 'sha256': 'de011fe01d3836b77ae6edb018fed7bb519da57afa40cb23c62f4fb3e203fead'}, {'path': 'tests/fixtures/typed_project_unit2_independent/semantic/README.md', 'bytes': 3283, 'sha256': '030246f1560023125b6e942373a01b59a308b4e2ed497edf5a2cc6254e1e3621'}, {'path': 'tests/fixtures/typed_project_unit2_independent/semantic/compare.py', 'bytes': 5661, 'sha256': '6e6642dfc28e37bf5c8c147719f52088ce3e2e5037cf15356ff287d6dd01df5c'}, {'path': 'tests/fixtures/typed_project_unit2_independent/semantic/corpus-manifest.json', 'bytes': 609, 'sha256': '9457c6182f4a45e15308c5fe73512415ecef2ea6e0ec8fac37eb42f4fb8a8933'}, {'path': 'tests/fixtures/typed_project_unit2_independent/semantic/corpus.jsonl.gz', 'bytes': 1050383, 'sha256': 'a5b1853d854b307ca31c79154b518fb9eddf19218467b5562af81979d1369994'}, {'path': 'tests/fixtures/typed_project_unit2_independent/semantic/frozen_compare.py', 'bytes': 4842, 'sha256': 'b4b343275fd42a798bc6a1aec0d526c828e68040d71d73940a8b819a4b13b74a'}, {'path': 'tests/fixtures/typed_project_unit2_independent/semantic/legacy-scalar-schedule-clarification.json', 'bytes': 1457, 'sha256': '18472233e83df397f1a0a2eb67df339380b4ca15a860558d9851906ad300ea00'}, {'path': 'tests/fixtures/typed_project_unit2_independent/semantic/materialize.py', 'bytes': 3001, 'sha256': '1985893a164e16f4a7ad1e0b3c324f4fb8f8eb1fc20d6995c6e82c759a1a7eeb'}, {'path': 'tests/fixtures/typed_project_unit2_independent/semantic/normalize.py', 'bytes': 12697, 'sha256': '1b48ff9de1ca56963fe648290c1f518842f4e633c4c0643abc22f030e4fb2c06'}, {'path': 'tests/fixtures/typed_project_unit2_independent/semantic/observer.rs', 'bytes': 13965, 'sha256': 'f2403aace53b6255a94b8b3ec0290db025638c5af94571d5729355672681fb00'}, {'path': 'tests/fixtures/typed_project_unit2_independent/semantic/original93-prose-expectations.json', 'bytes': 13035, 'sha256': '4742fee4eb4d5b09f2da3698ded582fcb08a7af9f13d9bf1369d9d547332187c'}, {'path': 'tests/fixtures/typed_project_unit2_independent/semantic/package-manifest.json', 'bytes': 2531, 'sha256': '710c79c5c39c2aea8d3b8c51de3251adce9a755d4190e2434d289696d85c1936'}, {'path': 'tests/fixtures/typed_project_unit2_independent/semantic/read-source-callback.patch', 'bytes': 474, 'sha256': 'ad05189f79f5c8c50dab7aa0b01147159f8bbfc37e161a75b0a5304c7ea1f2fd'}, {'path': 'tests/fixtures/typed_project_unit2_independent/source-inputs.json', 'bytes': 19509, 'sha256': '074b419d2af8a2c44d834ba95363b9ca0f5ac059445806adf48bc2c1db1234ad'}, {'path': 'tests/fixtures/typed_project_unit2_independent/test_protocol.py', 'bytes': 5532, 'sha256': 'e617691babf6a12fb5630e7c967c3c3656125f6ed7d4f29ad85b1898cb1dd58d'}, {'path': 'tests/fixtures/typed_project_unit3_compatibility/run.py', 'bytes': 7162, 'sha256': '5e2a7daec99812fcc709afc9356537172d10c2dad991a5d91a9407e5b92cd73e'}, {'path': 'tests/fixtures/typed_project_unit3_independent/manifests/core-v1.json', 'bytes': 19899, 'sha256': '53ed114674e59266c8e9a809f6cba5ef35c9ea819318ab64a7ae6d9aebd20910'}, {'path': 'tests/fixtures/typed_project_unit3_independent/manifests/platform-scope.patch', 'bytes': 2581, 'sha256': 'a6c0a70927fcc44f68244038e88ceb371fe0a78a8b8d4c0aa1d83475d415a1d9'}, {'path': 'tests/fixtures/typed_project_unit3_independent/manifests/project_execution_tests.core-v1.rs', 'bytes': 6567, 'sha256': '1185aa4c2e86644c3b14acae290adb043ec27d2a222018b1e67997317dc5851d'}, {'path': 'tests/fixtures/typed_project_unit3_independent/manifests/selected-current.json', 'bytes': 20051, 'sha256': 'a8e24a8d14c8b47140297f9b2b37adc75b2932911f8745df5937d58bdb2dc963'}, {'path': 'tests/fixtures/typed_project_unit3_independent/portable/assemble.py', 'bytes': 5487, 'sha256': '6eaa0333a0c6cede58c85a57cd51901b25e8e1423c1b96b3a177d4caafa1d965'}, {'path': 'tests/fixtures/typed_project_unit3_independent/portable/bridge.py', 'bytes': 3844, 'sha256': '0a67058d7592c1ce50ba52933784826f6883f566445bfe3da4aff49662f12e25'}, {'path': 'tests/fixtures/typed_project_unit3_independent/portable/common.py', 'bytes': 3521, 'sha256': '7b8b6d476c66eb7c2e833dff9dfe7eb42308bf8a67fd38a9a501f3b490eb7297'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/authority.json', 'bytes': 179689, 'sha256': '02b72b3dcf45c695e5c523d71bb1c83e15c082556cf36029829fefc7a71571b0'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/amendment/AMENDMENT-CHECKPOINT-v1.json', 'bytes': 2634, 'sha256': '5737321c95c0c293f80f89455024611d250908a4c039364059ca393f33fd2679'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/amendment/README.md', 'bytes': 5738, 'sha256': '4ace9224a08cc7de4031846a83daccbc61724396da2832a09d63056ac74203ea'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/amendment/artifacts/amendment.json', 'bytes': 3473, 'sha256': '70b7228c3b3c3cec074dc10a84e8e338ed9b443f6b95b1be883b9a43b225e451'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/amendment/artifacts/effective-contract.json', 'bytes': 2538, 'sha256': '9bf8a80cbb30c384a70fd5696e19428d2b0aad535740e7cbe2ce8992ef96f020'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/amendment/artifacts/source-coordinate-proof.json', 'bytes': 11237, 'sha256': '3be833baa8aff4d1d26a0c3351ecf39950873f8a2ef2768283847c08440fd63d'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/amendment/build_amendment.py', 'bytes': 12051, 'sha256': '6f51be43ce2f921247c7bf90e7fd9257aea2b89150d7d9e5cf1fca654ec4e508'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/amendment/history/provisional-v1/README.md', 'bytes': 5005, 'sha256': '7475997edc06c7b1c522d27d901d50f038ebfefbd006c7f315786e21f0210d73'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/amendment/history/provisional-v1/artifacts/amendment.json', 'bytes': 3473, 'sha256': '70b7228c3b3c3cec074dc10a84e8e338ed9b443f6b95b1be883b9a43b225e451'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/amendment/history/provisional-v1/artifacts/effective-contract.json', 'bytes': 2321, 'sha256': '8d3bf7cdb027e1c9589275787dafbd8c6b89439521f51201dcc47571cd014c72'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/amendment/history/provisional-v1/artifacts/source-coordinate-proof.json', 'bytes': 11237, 'sha256': '3be833baa8aff4d1d26a0c3351ecf39950873f8a2ef2768283847c08440fd63d'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/amendment/history/provisional-v1/build_amendment.py', 'bytes': 11834, 'sha256': '1b5ee10bbece4710029a5ab787a63fdaea729e7594f2033a5e315dad0d3c896b'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/amendment/receipt-identity.json', 'bytes': 630, 'sha256': 'be620a01f51104d9d61acec3427c237e4d48c1e9ced20e757614b19e4ae9d2e8'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/amendment/validation.json', 'bytes': 1236, 'sha256': 'ff4d3679ed3c874cfa8f7f12e1cb0da6f7e2679bae025388b4d1f783d468b755'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/comparator/comparator.py', 'bytes': 97198, 'sha256': '7c40e4782bee8082dc41534227348c26f952f3b870904cda9e71862b0be42a6b'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/comparator/debug_decoder.py', 'bytes': 4081, 'sha256': '8fb1708d7824eb453db5c036ac2e7d49ca76e69ff222f1c3cf1e16f41272aa41'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/comparator/test_mutations.py', 'bytes': 62411, 'sha256': '755b8364d459d8dc058e3360289155290771ec8941738583102290ff665848e3'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/helpers/API.md', 'bytes': 6952, 'sha256': 'bcf69d653c297ec11fd537ed5cd3b6b0f54f5bca445cd94f7e3c40c0eef929a1'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/helpers/HISTORY.md', 'bytes': 4755, 'sha256': '5672ff074676f4f1e130a8ebef0e9948ffd9ed4747831ff33f4d6748c91ff2dc'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/helpers/README.md', 'bytes': 2709, 'sha256': 'a9363683144c7de31f77a26ca27f4409e648a5ac9fecf249ff7c280c1fb8785e'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/helpers/build.py', 'bytes': 3059, 'sha256': '4a81dcd5a2252f7dbd5045075e23f54d7f08844a1617abab4180ab7181c3d793'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/helpers/observer.rs', 'bytes': 19856, 'sha256': '6c48934b5f690ad65671c62bd3968a933ae79c5a576d641133de2eacf0e18d9a'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/helpers/parse_debug.py', 'bytes': 4192, 'sha256': '89c77305d8b51bebbe71b06c8e3b2d82a09ae76a05f53eadae5e2820d48b2822'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/helpers/passivity.py', 'bytes': 3903, 'sha256': '2f1977e2e8694d676441d04c0301ac84826057320c6d64014d711a693d297f6e'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/helpers/prepare.py', 'bytes': 12616, 'sha256': '736933f3b966aa5ec2ed3aece40fa3837674ef11144b58fbad0ce8d58f907864'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/helpers/run.py', 'bytes': 10906, 'sha256': '78814a6f57713cbde350abd526e04d3cfec63092b06fd4a38034f985ed225997'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/frozen/helpers/test_adapter_shape.py', 'bytes': 8327, 'sha256': '215659e7e76c3d7ff4b3687c94a2cd9812e94ecb5530a59e12de4759f0a9a629'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/portable.py', 'bytes': 47092, 'sha256': 'cbbd81c6461dc26498fa1551efacb23b219d0fed78048e4ead5929154b823eed'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/reviews/amendment-review.json', 'bytes': 1921, 'sha256': 'a87d877c7e31d76ca0431188e3b862b94432baeb6bf3469017bab65e307ff89f'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/reviews/comparator-checkpoint.json', 'bytes': 4111, 'sha256': '78b152c74d9623ae2a66bc12c169bb6c9ed4077657d9bcde0e60978331a5afdc'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/reviews/comparator-review.json', 'bytes': 5623, 'sha256': '7cc6d3c3291703c1383c76d79b67ffcec30997a1a524fd0272cc6059ce8b0594'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/reviews/comparator-semantic-review.json', 'bytes': 2033, 'sha256': 'ab06219cff1133a722115e3e5ed309b46b33cac57625c67fa8d674793eec9e44'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/reviews/helper-checkpoint.json', 'bytes': 1813, 'sha256': 'ac7c9de19f1d123a935c6c658063b6f9324378465dd42280e6b76e350ef536c8'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/reviews/helper-review.json', 'bytes': 2703, 'sha256': 'e655e3740ad8be86f49a2d2f7ebadc84e4b3597dee4a7d218a8ad446b57dca85'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/reviews/portable-v2-checkpoint.json', 'bytes': 7632, 'sha256': 'ead24771bd5ba5a34664de68b7cd5e24e0fe247baed076de81da69d86b0d5e44'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/reviews/provisional-v1/REVIEW.md', 'bytes': 13490, 'sha256': '7090344fdcac0dfa88498f58586f3b446ef7127cbbf0ccb1671f583e3082b44a'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/reviews/provisional-v1/heldout-report.json', 'bytes': 3373, 'sha256': '3b227d785545baeff8eae0c85b0b5b266081816455720c1225a7b78f957790aa'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/reviews/provisional-v1/heldout_controls.py', 'bytes': 6405, 'sha256': '64794a7b90960a75188db75fc69b9c7a5bd3914134bfb62926e3706c02c57e3d'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/reviews/provisional-v1/producer-checkpoint.json', 'bytes': 5471, 'sha256': '37ffb1cf0c9b4be7f324079c39d472fb81f09cc0473219f582939befb0b8d659'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/reviews/provisional-v1/review-checkpoint.json', 'bytes': 3711, 'sha256': 'e6c02aa5be9eb941ff7e1a32cbc7216b6a5c533fefdbd68b3bb5fe38becddd48'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/reviews/provisional-v1/session-report.json', 'bytes': 1897, 'sha256': '87106ad220bd804ab416c49dcf085b2565a3b77a91835085da9f3e3061cd262b'}, {'path': 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/reviews/provisional-v1/session_controls.py', 'bytes': 9882, 'sha256': '245e472fb215235a803f549a76d39b4f60466a73d1bb4a9983543fbb7e12a586'}, {'path': 'tests/qualification/lexer_reservation_current/adapters.py', 'bytes': 10711, 'sha256': '0520fbf2c651f9b22897a19937c54749b0153ee234bae424dc4132d48e198e2a'}, {'path': 'tests/qualification/lexer_reservation_current/ci_inventory.py', 'bytes': 2081, 'sha256': 'd02b2938830cc2fe3c731cfe2ee7c44b7bd985f22ff10d0c021d788d618dac12'}, {'path': 'tests/qualification/lexer_reservation_current/current.py', 'bytes': 16325, 'sha256': 'f15abe15b48cf6a5b7a196ec77c45d8c34c8e0fca6d98dfbf2bb35fcabbcf850'}, {'path': 'tests/qualification/lexer_reservation_current/direct-consumer-dispositions.json', 'bytes': 191055, 'sha256': 'b69f0585ffd35d889023e3bcefe668db142b0d3adbd7a18cfa3823c88ffa96f0'}, {'path': 'tests/qualification/lexer_reservation_current/seal.py', 'bytes': 783, 'sha256': 'fa8c298cc5cc65dee984034190ab2e1fe945812e7a873004e14b3d3e242cf878'}, {'path': 'tests/qualification/lexer_reservation_current/source_transition.py', 'bytes': 12008, 'sha256': 'e642061c863ac1b836eeaeea72c8fdc1152097be4dc285ee2d071ea3372da9a0'}, {'path': 'tests/qualification/lexer_reservation_current/unit1_current_support.py', 'bytes': 13805, 'sha256': 'b30bd1b8a6f6f3650d0d55d750d7a78b93da3aa31a8cbc31ac3d4d5cded1eab5'}, {'path': 'tests/qualification/published_formatter_current/derive_windows_bindings_v1.py', 'bytes': 13388, 'sha256': '5c77a2f6ffe2500ee8d9effb5c031a36db22dad26bf57676b1f2d82ab113d8b3'}, {'path': 'tests/qualification/published_formatter_current/derive_windows_bindings_v2.py', 'bytes': 12627, 'sha256': '04ca5363362a6049116ff71bd82f28c53aa6dc9e1f61285849c0efe25336cd24'}, {'path': 'tests/qualification/published_formatter_current/predecessor-v1/tests/fixtures/typed_project_source_binding/current-source.json', 'bytes': 74328, 'sha256': '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'}, {'path': 'tests/qualification/published_formatter_current/predecessor-v1/tests/fixtures/typed_project_source_binding/package-manifest.json', 'bytes': 15335, 'sha256': '5fb5ee78a42609dab51f8e8bb23c325305954f49c52f0d18df6bbe47e57548bc'}, {'path': 'tests/qualification/published_formatter_current/predecessor-v1/tests/fixtures/typed_project_source_binding/run.py', 'bytes': 2360, 'sha256': '164d1e6213e140aca02f8f4a29c716f8af6c4af26e166e18fc0777251c0c2e1e'}, {'path': 'tests/qualification/published_formatter_current/predecessor-v1/tests/qualification/unit4_parser_current/authority.json', 'bytes': 524533, 'sha256': 'd8a02a6e970417b2108fcb8ff5e9eb2b32b068d562a7042d73eabaf504dae35c'}, {'path': 'tests/qualification/published_formatter_current/predecessor-v1/tests/qualification/unit4_parser_current/portable.py', 'bytes': 244286, 'sha256': '3366c49f267ab98bd792a1cd1b01ebbaaca548bc19685d841eb688cf39f99162'}, {'path': 'tests/qualification/published_formatter_current/predecessor-view-v1.json', 'bytes': 241022, 'sha256': 'e262dd17ebae52f7d662a1408420b795607e3ef6ae4612b1fe0a1a33b15081c8'}, {'path': 'tests/qualification/published_formatter_current/published_ci_inventory.py', 'bytes': 8640, 'sha256': 'b4ceba16b7cd03893035e6fe63c2ef70740ccba2704277237fe6909b1d939e2e'}, {'path': 'tests/qualification/published_formatter_current/published_current.py', 'bytes': 32290, 'sha256': '11f96d500198406d29e1a1e0880ade13dd8e2afd799315a08d5febcf5e889393'}, {'path': 'tests/qualification/published_formatter_current/published_transition.py', 'bytes': 12447, 'sha256': '069f7610fe2ffd3e51c47b1d2ed8f6db61f774dccb1dabf6977b24472e973a45'}, {'path': 'tests/qualification/published_formatter_current/published_view.py', 'bytes': 12882, 'sha256': '11a381be0f642138663d46c928b48b434332cb8152cf5006dd452fd178076e9b'}, {'path': 'tests/qualification/published_formatter_current/source-proof/ci-control-expectation-derivation-v3.json', 'bytes': 1874, 'sha256': '77a8e7e5634a921c8b3108e2039db3fa60718cb7e32c8850770026af64e03a4b'}, {'path': 'tests/qualification/published_formatter_current/source-proof/published-main-ci.yml', 'bytes': 67408, 'sha256': '81dfdfe82d9e477c4b6535a10967fc8d6861bcbcea5f90d341c694797ced3269'}, {'path': 'tests/qualification/published_formatter_current/source-proof/windows-bindings-derivation-v1.json', 'bytes': 9041, 'sha256': '5bf750b51e05d5f232a34b2ad0a37447182c22e60a93fafd452d0b61f61e56be'}, {'path': 'tests/qualification/published_formatter_current/source-proof/windows-bindings-derivation-v2.json', 'bytes': 7932, 'sha256': '704db63fe2a424f71e26a2166c49061ce808188f89f89311031d6de91a7404c8'}, {'path': 'tests/qualification/published_formatter_current/test_published_ci_inventory.py', 'bytes': 12577, 'sha256': 'aa86d277358e586030bf6a1c8d6ec6e8823512e99a736704f28c1c6d167b01c1'}, {'path': 'tests/qualification/published_formatter_current/test_published_transition.py', 'bytes': 63498, 'sha256': '028070d47034ec426366336f1252bd01b26a04dcab384cafacb7112bbe3f0609'}, {'path': 'tests/qualification/published_formatter_current/test_published_view.py', 'bytes': 11345, 'sha256': 'b876b6d58fe5b40e274168117188e34236cf80d8304cc17eff9f2fddb9b54bd0'}, {'path': 'tests/qualification/unit4_parser_current/byte-storage-authority-v1.json', 'bytes': 518280, 'sha256': '446f9c022dd7ae3070a9fe8a07e4973fdbb12f4623b8a293b6826ed3ef576c5b'}, {'path': 'tests/qualification/unit4_parser_current/cache-admission-authority.json', 'bytes': 486349, 'sha256': '47725dbdd2cd13da289205a15f7de53610e782f3e2f2037695be374e8fa7ad91'}, {'path': 'tests/qualification/unit4_parser_current/cache-preservation-authority.json', 'bytes': 485191, 'sha256': 'f9202d30c38ca09e51879146a53864c7107a49e6e07ce581e22dfe9afa78d377'}, {'path': 'tests/qualification/unit4_parser_current/lexer-phase-authority-v1.json', 'bytes': 524533, 'sha256': 'd8a02a6e970417b2108fcb8ff5e9eb2b32b068d562a7042d73eabaf504dae35c'}, {'path': 'tests/qualification/unit4_parser_current/lexer-phase-composition-v1.json', 'bytes': 3141, 'sha256': '5cf6b9cc9a6fd7253fd2f04b7107dd5eb631bbb438c5ad94f2b63ecf73deef36'}, {'path': 'tests/qualification/unit4_parser_current/lexer-reservation-authority-v2.json', 'bytes': 520103, 'sha256': '917431fe269d807c76b2f16ae7fef7f4e6c70aa301f9823f65de4fe039b94baf'}, {'path': 'tests/qualification/unit4_parser_current/lexer_phase.py', 'bytes': 54473, 'sha256': '1067d223defa8de2a6b0faa71910d9acc97d635491022216e5d59fd48c62ed0e'}, {'path': 'tests/qualification/unit4_parser_current/lexer_phase_controls.py', 'bytes': 33650, 'sha256': 'cf8667d190c9b50c2f69b623783a33c8c931f795e359a43980ef2486ad44d8c9'}, {'path': 'tests/qualification/unit4_parser_current/lexer_reservation.py', 'bytes': 5545, 'sha256': 'fb579ecf36dc045dae7f501e693156b8ee1d3772193c28c130e529e8a97814ac'}, {'path': 'tests/qualification/unit4_parser_current/lexical-provider-authority.json', 'bytes': 482448, 'sha256': '209ad73bf9240366136787f044c837fffaaeae9a9909faa9855003387fdade2f'}, {'path': 'tests/qualification/unit4_parser_current/package-integrity-authority.json', 'bytes': 484020, 'sha256': '974aebf4121f323f1674a19b7442d42e495c2075e99ae9f70c6cc1f1fb59a7a5'}, {'path': 'tests/qualification/unit4_parser_current/producer-diagnostic-authority.json', 'bytes': 477258, 'sha256': 'ecc09429f644abeaa9f92db9b65b73a181c6ee259472d94660a61861ee0360c9'}, {'path': 'tests/qualification/unit4_parser_current/record-composition-diagnostics-v1.json', 'bytes': 28188, 'sha256': 'f0ec986bb6192df43e6e1248d08ba26df6b1eb8efc920222044b981974082c25'}, {'path': 'tests/qualification/unit4_parser_current/record_composition_amendment.py', 'bytes': 4128, 'sha256': '30f48bd4310452067cecdd4e732f91c7ed3425e292cbae3375421cbfbac33f08'}, {'path': 'tests/qualification/unit4_parser_current/test_lexer_phase.py', 'bytes': 51494, 'sha256': '858e2809bde1dc52ca4f088c4131ce539ab007f2d7f70214816499e73961ec7c'}, {'path': 'tests/qualification/unit4_parser_current/u8-authority.json', 'bytes': 504509, 'sha256': '48b3864d7b04f2e4afcdc35c2a42fd56720dcbd80ac789f359e9f04e5eff48be'}, {'path': 'tests/qualification/unit4_parser_current/u8-cross-host-authority.json', 'bytes': 505592, 'sha256': '29ab8e953b865fe723719e1ee7b6054b0d90288658fdc3b12f8c83a873dc42a9'}, {'path': 'tests/qualification/unit4_parser_current/u8_policy_controls.py', 'bytes': 9018, 'sha256': '90807a872ef4b042abfc6f29d785a903c54fa00ae7dc364d26fe07dc6baa7b95'}, {'path': 'tests/qualification/unit4_public_v3/authority.py', 'bytes': 1999, 'sha256': 'c4a57c41e38b64cf00f5e3c540d4b30b017dfba343b7f8ac12b13d1d69c23b92'}, {'path': 'tests/qualification/unit4_public_v3/build.py', 'bytes': 14635, 'sha256': 'c4b19037171c61c52288b67b3e1a9c2b9cc55ca3c3aebc3adf056ed904f84d2e'}, {'path': 'tests/qualification/unit4_public_v3/byte_storage_source_authority.py', 'bytes': 1404, 'sha256': '14629fd20362700e69c3aca0ce079748bf829533f1d2e5098f43b26ba3df0ba5'}, {'path': 'tests/qualification/unit4_public_v3/lexer-reservation-lifecycle-v2.json', 'bytes': 282549, 'sha256': 'f8e6e92eda7c0d07dea9a73301be9318f32c4a0eddb7126a581ad17e88d10e86'}, {'path': 'tests/qualification/unit4_public_v3/lexer_reservation_lifecycle.py', 'bytes': 9393, 'sha256': '155471428cc243fbb7845ec2dcd21fb982f51e7e8264ccf804f1df07338c91ec'}, {'path': 'tests/qualification/unit4_public_v3/observer-lexer-reservation-v2.patch', 'bytes': 7337, 'sha256': '826468198bfd2575d0d01713bb803f19f31b732d482dbf336b7e37567f890824'}, {'path': 'tests/qualification/unit4_public_v3/observer-u8-v1.patch', 'bytes': 7541, 'sha256': 'c64b43cd0b630fcae46ac4cf73812ad0eb614696474e3af5d75d79b32f5047cf'}, {'path': 'tests/qualification/unit4_public_v3/published-lifecycle-v1.json', 'bytes': 149158, 'sha256': '46b8bdf4d5d1559cd681ef00026220111d0c1244533da2b8e470d38e19bf69c2'}, {'path': 'tests/qualification/unit4_public_v3/published_lifecycle.py', 'bytes': 46537, 'sha256': 'afa277d0586fdea4ff9d76fe83d7025b3b399390d44d356857dfb27b2612fe29'}, {'path': 'tests/qualification/unit4_public_v3/test_lexer_reservation.py', 'bytes': 4426, 'sha256': '3178d84744642553d1a7505095391525c03e0631a351a542bd7411f438dcab5e'}, {'path': 'tests/typed_frontend.rs', 'bytes': 16540, 'sha256': 'fd7e5cf5e34b0c176a7e4cfc80ca1000b79b4ee1a67e31a6e91003126fa0da68'}, {'path': 'tests/typed_project_dispatch.rs', 'bytes': 5862, 'sha256': '23e5bb6d0050b8adef32aada00f779c1a43c07dc42bd762d23529b794ac5d6a5'}, {'path': 'tests/typed_record_composition.rs', 'bytes': 13218, 'sha256': '347a35be3794ef927789a3d036e99ccb45bcb1b52c49272a8647b1924b5c8db8'}], 'schema': 'oxid-published-source-ci-entry-v1', 'source_seal': {'authority': {'bytes': 265133, 'path': 'published-formatter-authority-v1.json', 'sha256': 'dac2e66cf02e2d6d5c14531b30de8cda88696676747509419a8478e6ed871c43'}, 'patch': {'bytes': 1522, 'path': 'published-formatter-transition-v1.patch', 'sha256': 'c6163a923cf56bde945d753628bc6f2c7c4ad717affd35e8bafe7a0c195bccd6'}, 'predecessor_view': {'bytes': 241022, 'path': 'tests/qualification/published_formatter_current/predecessor-view-v1.json', 'sha256': 'e262dd17ebae52f7d662a1408420b795607e3ef6ae4612b1fe0a1a33b15081c8'}, 'schema': 'oxid-published-main-source-seal-v1', 'source_manifest': {'bytes': 74567, 'path': 'published-formatter-source-v1.json', 'sha256': 'd83edc200b9b900668ccc7ee9679a7972e1d497d10acf5f82ccd602680dd04e9'}, 'source_proof': {'bytes': 219557, 'path': 'published-main-source-proof-v1.json', 'sha256': '854ab1a1f5ad1075b836d7ea7387e7692b0adae59cd709921e6a8eae17770819'}}, 'test_rosters': {'ci': ['test_bad_source_digest_rejected', 'test_changed_audit_rejected', 'test_changed_old_disposition_rejected', 'test_complete_173_roster_and_every_disposition_preserved', 'test_equal_float_descriptor_sizes_are_rejected', 'test_new_provenance_names_real_source_checkpoint', 'test_oldest_workflow_tamper_rejected', 'test_only_two_physical_argument_lines_change', 'test_plan_descriptor_fields_and_path_closed', 'test_published_middle_workflow_tamper_rejected', 'test_rehashed_plan_cannot_change_any_original_entry_field', 'test_rehashed_plan_cannot_claim_execution_or_stale_source', 'test_rehashed_plan_cannot_omit_reorder_or_duplicate_consumer', 'test_rehashed_plan_rejects_duplicate_keys', 'test_rehashed_plan_rejects_equal_numeric_types', 'test_source_descriptor_fields_and_path_closed', 'test_three_generations_have_genuine_fixed_bindings', 'test_unapproved_current_workflow_rejected_even_if_rehashed', 'test_valid_complete_record_makes_no_execution_claim', 'test_workflow_descriptor_fields_and_path_closed'], 'public_original': ['test_complete_maps_and_exact_inverse', 'test_each_exact_hunk_context_is_checked', 'test_missing_duplicate_and_wrapper_hooks_reject', 'test_nonlexer_insertions_unchanged', 'test_reordered_and_duplicate_patch_sections_rejected', 'test_stale_predecessor_and_tampered_patch_reject'], 'source_integration': ['test_genuine_complete_376_inverse_and_provenance', 'test_genuine_proof_schema_and_evidence_pins_reject_rehashes', 'test_genuine_identity_modes_blobs_fixtures_and_includes_are_fixed', 'test_genuine_inverse_rejects_wrong_stage_tail_and_membership', 'test_genuine_source_body_mutations_rejected', 'test_genuine_complete_source_membership_and_modes_rejected', 'test_genuine_package99_preserves_old95_boundary', 'test_genuine_shared_package_body_coherent_rehash_rejected', 'test_genuine_capture_preserves_historical_and_accounting_chain', 'test_genuine_capture_rejects_post_admission_type_confusion', 'test_genuine_paired_capture_mutations_reject_canonical_mismatch', 'test_genuine_prepared_unit2_uses_actual_assembly_and_old_verifier', 'test_genuine_prepared_unit2_rejects_changed_main_and_assembly', 'test_independent_consumer_accepts_exact_original_snapshot', 'test_independent_consumer_rejects_coherent_dispatcher_manifest_tamper'], 'source_unit': ['test_source_positive_prefix_stops_before_source_read', 'test_source_seal_schema_is_exact_and_noncyclic', 'test_source_named_descriptors_and_alias_are_exact', 'test_source_authority_schema_is_exact', 'test_source_published_and_historical_provenance_are_distinct', 'test_source_manifest_provenance_and_metadata_are_preserved', 'test_source_membership_requires_all_376_ordered_rows', 'test_source_transition_is_exactly_main', 'test_source_fixed_predecessor_bytes_cannot_be_rehashed', 'test_package_positive_roster_stops_before_body_acceptance', 'test_package_requires_old95_plus_exact_four_names', 'test_package_duplicate_and_self_manifest_rows_rejected', 'test_package_manifest_metadata_is_preserved', 'test_exact_capture_equality_rejects_bool_int_and_float_aliases', 'test_exact_descriptor_and_receipt_overlap_types_are_preserved', 'test_assert_unchanged_rejects_equal_valued_type_mutation', 'test_view_prefers_published_predecessor_over_older_map', 'test_view_accepts_only_explicit_source_only_inverse_stage', 'test_view_checks_complete_predecessor_bodies', 'test_capture_retains_complete_old_record_and_closes_view', 'test_capture_rejects_live_objects_and_temporary_paths', 'test_capture_cannot_call_old_preflight_on_current_root', 'test_unit2_uses_actual_instrumented_assembly_map', 'test_unit2_keeps_all_original_verifier_result_fields', 'test_unit2_does_not_suppress_original_verifier_rejection', 'test_unit2_rejects_stale_main_lexer_or_accounting_body', 'test_unit2_rejects_changed_assembly_or_current_manifest', 'test_unit2_rejects_changed_prepared_accounting_binding', 'test_failure_primary_is_written_before_validation', 'test_failure_invalid_capture_preserves_primary_error', 'test_failure_sidecar_write_error_cannot_mask_primary', 'test_failure_sidecar_keyboard_interrupt_cannot_mask_primary', 'test_failure_without_capture_keeps_original_writer', 'test_terminal_result_provenance_rejection_writes_failure', 'test_terminal_prepared_provenance_rejection_writes_failure', 'test_planned_provenance_rejection_uses_original_failure_path', 'test_successful_extension_keeps_original_receipt_fields'], 'view': ['test_actual_inverse_is_preferred_over_older_predecessor_map', 'test_bad_descriptor_fails_without_materialization', 'test_changed_current_capture_source_fails_before_materialization', 'test_cleanup_failure_alone_remains_failure', 'test_cleanup_failure_retains_operation_and_preservation_objects', 'test_complete_view_body_and_roster_then_cleanup', 'test_extra_empty_directory_is_rejected_before_close', 'test_extra_file_is_rejected_before_close', 'test_missing_fixed_seal_fails_before_capture', 'test_next_sequential_view_runs_only_after_clean_close', 'test_operation_and_preservation_failure_both_retained', 'test_operation_failure_retains_exact_object_and_cleans_view', 'test_postclosure_body_drift_is_rejected_and_cleaned', 'test_same_module_overlap_is_rejected_before_second_capture']}}
SOURCE_HEAD = "fbff527c895e6cd4ec9350bd8b5d460bc20bb80d"
SOURCE_TREE = "89e90c3ec83d0712d0037a0407fc6571883c5e9c"
M_HEAD = "15eac633e167c261ba0246ced49cfdc1c6cef434"
HISTORICAL_HEAD = "b3abc9f0dda99d6d8fe65d9c3a9ed31dedcbd489"
PACKAGE = "tests/fixtures/typed_project_source_binding"
RETAINED = "tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py"
MODULES = (
    ("adapters", "tests/qualification/lexer_reservation_current/adapters.py"),
    ("source_transition", "tests/qualification/lexer_reservation_current/source_transition.py"),
    ("seal", "tests/qualification/lexer_reservation_current/seal.py"),
    ("current", "tests/qualification/lexer_reservation_current/current.py"),
    ("published_transition", "tests/qualification/published_formatter_current/published_transition.py"),
    ("published_view", "tests/qualification/published_formatter_current/published_view.py"),
    ("published_current", "tests/qualification/published_formatter_current/published_current.py"),
    ("ci_inventory", "tests/qualification/lexer_reservation_current/ci_inventory.py"),
    ("published_ci_inventory", "tests/qualification/published_formatter_current/published_ci_inventory.py"),
    ("test_published_transition", "tests/qualification/published_formatter_current/test_published_transition.py"),
    ("test_published_view", "tests/qualification/published_formatter_current/test_published_view.py"),
    ("test_published_ci_inventory", "tests/qualification/published_formatter_current/test_published_ci_inventory.py"),
    ("_published_source_real_consumer", "tests/qualification/lexer_reservation_current/unit1_current_support.py"),
    ("published_lifecycle", "tests/qualification/unit4_public_v3/published_lifecycle.py"),
    ("_published_original_public_tests", "tests/qualification/unit4_public_v3/test_lexer_reservation.py"),
)
VIEW_PREFIXES = (
    "oxid-published-main-", "oxid-lexer-retained-source-",
    "oxid-retained-u8-cross-host-", "oxid-retained-u8-source-",
    "oxid-retained-cache-admission-", "oxid-retained-cache-preservation-",
    "oxid-retained-package-integrity-", "oxid-retained-lexical-provider-",
    "oxid-retained-producer-diagnostic-", "oxid-retained-frontend-v2-",
)
STREAMS = (
    "runner.stderr", "runner.stdout", "unit2/runner.stderr", "unit2/runner.stdout",
    "unit2/run/materialize.stderr", "unit2/run/materialize.stdout",
    "unit2/run/python-assertions.stderr", "unit2/run/python-assertions.stdout",
)
EVIDENCE = STREAMS + (
    "unit2/command.json", "unit2/invocation.json", "unit2/prepared.json",
    "unit2/run/assembly.json", "unit2/run/commands.json",
    "unit2/run/invocation.json", "unit2/run/prepared.json",
)


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def encoded(value):
    return (json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + "\n").encode()


def relative(name):
    need(type(name) is str and name and not name.startswith("/") and "\\" not in name
         and str(PurePosixPath(name)) == name
         and all(part not in (".", "..") for part in name.split("/")), "unsafe fixed relative path")
    return name


def descriptor(name, raw):
    return {"path": name, "bytes": len(raw), "sha256": digest(raw)}


def read_regular(path, cap=2 * 1024 * 1024):
    path = Path(path)
    for parent in (*reversed(path.parents), path):
        need(not parent.is_symlink(), "aliased CI source input")
    before = path.lstat()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    stamp = lambda value: tuple(getattr(value, field) for field in fields)
    need(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and not before.st_mode & 0o111
         and 0 <= before.st_size <= cap, "unsafe or oversized CI source input")
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NOATIME | os.O_NONBLOCK)
    try:
        need(stamp(os.fstat(fd)) == stamp(before), "CI source input replaced before read")
        pieces, left = [], before.st_size
        while left:
            part = os.read(fd, min(left, 65536))
            need(part, "short CI source input")
            pieces.append(part)
            left -= len(part)
        need(os.read(fd, 1) == b"" and stamp(os.fstat(fd)) == stamp(before),
             "CI source input changed during read")
    finally:
        os.close(fd)
    need(stamp(path.lstat()) == stamp(before), "CI source input name changed during read")
    return b"".join(pieces)


def write_new(path, raw, cap):
    need(type(raw) is bytes and len(raw) <= cap, "CI source output cap exceeded")
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("xb") as stream:
        need(stream.write(raw) == len(raw), "short CI source output")
    need(read_regular(path, cap) == raw, "CI source output readback changed")


def load_buffer(name, path, raw, register=False):
    module = types.ModuleType(name)
    module.__file__ = str(path)
    if register:
        sys.modules[name] = module
    exec(compile(raw, str(path), "exec"), vars(module))
    return module


def authenticate(repo, configuration):
    """Read every explicitly pinned input before compiling any target helper."""
    need(type(configuration) is dict, "inactive entrypoint: final derivation is required")
    rows = configuration["input_roster"]
    need(type(rows) is list and len(rows) <= 2048, "invalid bounded CI input roster")
    bodies = {}
    for row in rows:
        need(type(row) is dict and set(row) == {"path", "bytes", "sha256"}, "invalid CI pin")
        name = relative(row["path"])
        need(name not in bodies and type(row["bytes"]) is int and 0 <= row["bytes"] <= 2097152
             and re.fullmatch("[0-9a-f]{64}", row["sha256"]) is not None, "invalid CI pin fields")
        raw = read_regular(repo / name)
        need(descriptor(name, raw) == row, "changed independently pinned CI input: " + name)
        bodies[name] = raw
    need([row["path"] for row in rows] == sorted(bodies), "unordered fixed CI input roster")
    need(all(path in bodies for _, path in MODULES) and RETAINED in bodies,
         "missing executable CI source-control dependency")
    need(PACKAGE + "/run.py" in bodies and PACKAGE + "/package-manifest.json" in bodies,
         "missing real consumer input buffers")
    need(configuration["source_seal"]["schema"] == "oxid-published-main-source-seal-v1",
         "missing fixed source seal")
    return bodies


def bounded_git(repo, arguments):
    """Read only the current HEAD and its own commit body; no parent fetches."""
    env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
    env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull, GIT_OPTIONAL_LOCKS="0",
               GIT_NO_LAZY_FETCH="1", GIT_TERMINAL_PROMPT="0")
    command = ["git", "--no-optional-locks", "-C", str(repo), *arguments]
    child = subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    try:
        pieces, count, deadline = [], 0, time.monotonic() + 20
        with selectors.DefaultSelector() as selector:
            selector.register(child.stdout, selectors.EVENT_READ)
            while True:
                remaining = deadline - time.monotonic()
                need(remaining > 0 and selector.select(remaining), "current Git metadata read timed out")
                part = os.read(child.stdout.fileno(), 65536)
                if not part:
                    break
                pieces.append(part)
                count += len(part)
                need(count <= 1048576, "oversized current Git commit metadata")
        raw = b"".join(pieces)
        need(child.wait(timeout=max(0.01, deadline - time.monotonic())) == 0,
             "current checkout Git metadata read failed")
    except BaseException:
        child.kill()
        child.wait()
        raise
    finally:
        child.stdout.close()
    return raw


def checkout_identity(repo):
    need(os.environ.get("GITHUB_ACTIONS") == "true", "this entrypoint requires its GitHub CI checkout")
    need(os.environ.get("GITHUB_REPOSITORY") == "YanagiKH/Oxid", "wrong CI repository")
    head = bounded_git(repo, ["rev-parse", "--verify", "HEAD"]).decode("ascii").strip()
    need(re.fullmatch("[0-9a-f]{40}", head) is not None and head == os.environ.get("GITHUB_SHA"),
         "checkout HEAD differs from the GitHub event SHA")
    raw = bounded_git(repo, ["cat-file", "commit", head])
    need(hashlib.sha1(b"commit " + str(len(raw)).encode() + b"\0" + raw).hexdigest() == head,
         "current commit body does not bind checkout HEAD")
    header = raw.split(b"\n\n", 1)[0].splitlines()
    trees = [line[5:].decode("ascii") for line in header if line.startswith(b"tree ")]
    parents = [line[7:].decode("ascii") for line in header if line.startswith(b"parent ")]
    need(len(trees) == 1 and re.fullmatch("[0-9a-f]{40}", trees[0]) is not None
         and all(re.fullmatch("[0-9a-f]{40}", p) is not None for p in parents), "invalid current commit headers")
    event_name = os.environ.get("GITHUB_EVENT_NAME")
    result = {"checkout_head": head, "checkout_tree": trees[0], "checkout_parents": parents,
              "source_checkpoint_head": SOURCE_HEAD, "source_checkpoint_tree": SOURCE_TREE,
              "published_main_head": M_HEAD, "historical_source_head": HISTORICAL_HEAD,
              "event_name": event_name, "form": "event-checkout", "full_checkpoint_reconstruction": False}
    if event_name == "pull_request":
        event = json.loads(read_regular(Path(os.environ["GITHUB_EVENT_PATH"]), 1048576))
        pr = event["pull_request"]
        need(event["repository"]["full_name"] == pr["base"]["repo"]["full_name"] == "YanagiKH/Oxid"
             and type(event["number"]) is int and event["number"] > 0
             and pr["number"] == event["number"], "wrong pull-request event identity")
        event_head, event_base = pr["head"]["sha"], pr["base"]["sha"]
        need(all(type(x) is str and re.fullmatch("[0-9a-f]{40}", x) for x in (event_head, event_base))
             and type(pr["head"]["repo"]["full_name"]) is str
             and re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", pr["head"]["repo"]["full_name"]),
             "invalid PR head/base repository identities")
        if head == event_head:
            result["form"] = "direct-pr-head"
        else:
            need(len(parents) == 2 and len(set(parents)) == 2
                 and set(parents) == {event_head, event_base}, "checkout is not the exact PR head/base merge")
            result["form"] = "event-two-parent-merge"
        result.update(pr_number=event["number"], pr_head=event_head, pr_base=event_base,
                      pr_head_repository=pr["head"]["repo"]["full_name"],
                      merge_tree_is_pr_head_tree=False if head != event_head else None)
    return result


def clean_exact(root, snapshot):
    """Delete only previously reconciled generated files and directories."""
    files, directories = snapshot
    for name in sorted(files, reverse=True):
        (root / name).unlink()
    for name in sorted(directories, key=lambda x: (-len(PurePosixPath(x).parts), x)):
        (root / name).rmdir()
    root.rmdir()
    need(not os.path.lexists(root), "generated tree cleanup left residue")


def snapshot_generated(root, max_files, max_dirs, per_file=4194304):
    """Bounded traversal of a single newly generated tree, never the checkout."""
    files, directories, pending = {}, set(), [""]
    while pending:
        parent = pending.pop()
        with os.scandir(root / parent) as entries:
            for item in entries:
                name = parent + "/" + item.name if parent else item.name
                info = item.stat(follow_symlinks=False)
                if stat.S_ISDIR(info.st_mode):
                    need(len(directories) < max_dirs, "generated directory bound exceeded")
                    directories.add(name)
                    pending.append(name)
                else:
                    need(len(files) < max_files and stat.S_ISREG(info.st_mode) and info.st_nlink == 1
                         and not info.st_mode & 0o111, "unsafe generated member")
                    raw = read_regular(root / name, per_file)
                    files[name] = descriptor(name, raw)
    return files, directories


def roster_sha(names):
    return digest(("\n".join(sorted(names)) + "\n").encode())


class ViewEffects:
    """One M view, one nested old-lexer view, and serial known old byte views."""
    def __init__(self, root):
        self.root, self.active, self.dead = root, [], []
        self.failed = False
        self.preparing = False
        self.original = tempfile.TemporaryDirectory

    def assert_no_residue(self):
        need(not self.failed and not self.active and all(not os.path.lexists(p) for p in self.dead),
             "unreconciled predecessor view remains")

    def assert_view_entered(self, root):
        need(self.active and self.active[-1][0] == "oxid-published-main-"
             and Path(root) == self.active[-1][1] / "repository", "unobserved genuine M view")

    def assert_single_view_lifetime(self):
        need(not self.failed, "predecessor view lifetime failed")

    def temporary(self, suffix=None, prefix=None, dir=None, **kwargs):
        need(not self.failed and not self.preparing and prefix in VIEW_PREFIXES
             and suffix in (None, "") and not kwargs, "unapproved or overlapping view lifetime")
        active = [kind for kind, _ in self.active]
        if prefix == VIEW_PREFIXES[0]:
            need(not active, "overlapping full M view")
        elif prefix == VIEW_PREFIXES[1]:
            need(active == [VIEW_PREFIXES[0]], "old lexer outside its one M view")
        else:
            need(active == list(VIEW_PREFIXES[:2]), "overlapping old byte views")
        need(dir is None or Path(dir) == self.root, "view outside isolated CI temporary root")
        owner = self
        prior_modules = dict(sys.modules)
        inner = self.original(prefix=prefix, dir=self.root)
        item = (prefix, Path(inner.name))
        owner.active.append(item)

        class Scope:
            name = inner.name

            def __enter__(self):
                return inner.__enter__()

            def __exit__(self, typ, value, traceback):
                try:
                    need(owner.active and owner.active[-1] == item, "non-LIFO view closure")
                    for name, module in tuple(sys.modules.items()):
                        path = getattr(module, "__file__", "")
                        if isinstance(path, str) and path.startswith(str(item[1]) + "/"):
                            if name in prior_modules:
                                sys.modules[name] = prior_modules[name]
                            else:
                                del sys.modules[name]
                    inner.__exit__(typ, value, traceback)
                    need(not os.path.lexists(item[1]), "view cleanup left residue")
                except BaseException as cleanup:
                    owner.failed = True
                    inner._finalizer.detach()
                    if value is not None and value is not cleanup:
                        raise BaseExceptionGroup("view operation and cleanup failed", [value, cleanup]) from None
                    raise
                owner.active.pop()
                owner.dead.append(item[1])
                return False
        return Scope()


EXPORT_LIMIT = 32 * 1024 * 1024
LOG_LIMIT = EXPORT_LIMIT


def exact_json(raw):
    def unique(pairs):
        value = {}
        for key, item in pairs:
            need(key not in value, "duplicate evidence JSON key")
            value[key] = item
        return value
    return json.loads(raw, object_pairs_hook=unique)


def verify_preparation_commands(output, commands):
    """Original complete command records, not synthetic child receipts."""
    outer = exact_json(read_regular(output / "unit2/command.json", 131072))
    need(set(outer) == {"argv", "cwd", "exit_status", "stdout_sha256", "stderr_sha256"}
         and outer["argv"] == commands["unit2"]["argv"] and outer["cwd"] == commands["unit2"]["cwd"]
         and type(outer["exit_status"]) is int and outer["exit_status"] == 0,
         "changed genuine Unit2 child command")
    for suffix in ("stdout", "stderr"):
        need(outer[suffix + "_sha256"] == digest(read_regular(output / ("unit2/runner." + suffix), LOG_LIMIT)),
             "changed complete genuine Unit2 stream")
    leaves = exact_json(read_regular(output / "unit2/run/commands.json", 131072))
    need(type(leaves) is list and len(leaves) == 2, "wrong exact prepare-only leaf command count")
    fields = {"label", "argv", "cwd", "started_at_utc", "finished_at_utc", "exit_status", "stdout_sha256", "stderr_sha256"}
    for record, role, label in zip(leaves, ("assertions", "materialize"), ("python-assertions", "materialize")):
        need(type(record) is dict and set(record) == fields and record["label"] == label
             and record["argv"] == commands[role]["argv"] and record["cwd"] == commands[role]["cwd"]
             and type(record["exit_status"]) is int and record["exit_status"] == 0
             and all(type(record[key]) is str and 0 < len(record[key]) <= 64
                     for key in ("started_at_utc", "finished_at_utc")), "changed genuine preparation leaf command")
        for suffix in ("stdout", "stderr"):
            raw = read_regular(output / ("unit2/run/" + label + "." + suffix), LOG_LIMIT)
            need(record[suffix + "_sha256"] == digest(raw), "changed complete genuine leaf stream")


def exception_record(error):
    """Preserve bounded full messages and cause/context/group identities."""
    nodes, seen = [], {}
    def visit(value):
        if value is None:
            return None
        if id(value) in seen:
            return seen[id(value)]
        need(len(nodes) < 64, "exception evidence exceeds its finite node budget")
        index = len(nodes)
        seen[id(value)] = index
        row = {"type": type(value).__module__ + "." + type(value).__qualname__, "message": str(value)}
        nodes.append(row)
        row.update(cause=visit(value.__cause__), context=visit(value.__context__),
                   suppress_context=value.__suppress_context__,
                   group=[visit(child) for child in getattr(value, "exceptions", ())])
        return index
    return {"root": visit(error), "exceptions": nodes}


class ExistingUnit2Evidence:
    """Finite exclusive additions to an already completed, unsealed root."""
    old_names = ("result.json", "plan.json", "command.json", "resource-seam.json", "runner.stdout", "runner.stderr")

    def __init__(self, repo, configuration, bodies, current_runtime, current_capture):
        parent = Path(os.environ["RUNNER_TEMP"])
        need(parent.is_absolute() and parent.resolve(strict=True) == parent, "noncanonical evidence parent")
        self.root = parent / "typed-project-unit2-results"
        for path in (*reversed(self.root.parents), self.root):
            need(not path.is_symlink() and path.is_dir(), "aliased or missing completed Unit2 evidence root")
        self.identity = self.root_identity(self.root.lstat())
        need(self.identity[2] == os.getuid(), "completed Unit2 root has a different owner")
        self.old = {name: read_regular(self.root / name, 1048576) for name in self.old_names}
        result, plan, command, seam = (exact_json(self.old[name])
            for name in ("result.json", "plan.json", "command.json", "resource-seam.json"))
        need(result["schema"] == plan["schema"] == "oxid-current-archive-binding-v1"
             and result["action"] == plan["action"] == "run-unit2"
             and result["status"] == "passed-current-unit2" and result["semantic_pass"] is True
             and result["direct_current_execution"] is True and plan["status"] == "planned"
             and plan["repository"] == str(repo) and plan["current_source_members"] == 376
             and type(plan["current_source_members"]) is int
             and result["invocation_id"] == plan["invocation_id"]
             and re.fullmatch("[0-9a-f]{32}", result["invocation_id"]) is not None,
             "existing Unit2 root is not a genuine completed current execution")
        source = configuration["source_seal"]["source_manifest"]["sha256"]
        controller, manifest = digest(bodies[PACKAGE + "/run.py"]), digest(bodies[PACKAGE + "/package-manifest.json"])
        for value in (result, plan):
            need(value["controller_sha256"] == controller and value["current_source_sha256"] == source
                 and value["adapter_package_sha256"] == manifest, "old execution source/controller binding differs")
            expected = current_runtime._modules['package_current'].outer_receipt(current_capture)
            verify_successor_receipt(value, expected)
        need(result["plan_sha256"] == command["plan_sha256"] == digest(self.old["plan.json"])
             and result["resource_seam_sha256"] == digest(self.old["resource-seam.json"])
             and result["command_sha256"] == digest(self.old["command.json"]), "old completed receipt hashes disagree")
        need(type(command["exit_status"]) is int and command["exit_status"] == 0
             and command["cwd"] == str(repo), "old compatibility command did not succeed in this checkout")
        argv = command["argv"]
        need(type(argv) is list and len(argv) == 13 and argv[1:3] == ["-B", seam["compatibility_runner"]]
             and argv[3:9] == ["--repo", str(repo), "--source-manifest", str(repo / PACKAGE / "current-source.json"),
                               "--output", str(self.root / "unit2")]
             and argv[9] == "--cargo" and argv[11] == "--rustc"
             and all(type(argv[i]) is str and Path(argv[i]).is_absolute() for i in (0, 10, 12))
             and seam["compatibility_runner"] == str(self.root / "compatibility/typed_project_unit3_compatibility/run.py")
             and seam["resource_package_root"] == str(self.root / "compatibility/typed_project_unit2_independent"),
             "old compatibility command/seam is not the existing full-run layout")
        for suffix in ("stdout", "stderr"):
            need(command[suffix] == descriptor("runner." + suffix, self.old["runner." + suffix]),
                 "old execution complete stream binding differs")
        # Verify the actual new full Unit2 execution with the unchanged original
        # verifier and new ordinary-source checks before exporting any evidence.
        current_runtime.verify_unit2_result(self.root, current_capture, seam, False)
        self.attempted, self.written_bytes, self.written = False, 0, []
        self.slots = {"checkout.json": 8192, "complete.json": 16384, "error.json": 524288}
        for number in (1, 2, 3):
            for name in EVIDENCE:
                self.slots["case-" + str(number) + "/" + name] = LOG_LIMIT if name in STREAMS else 131072
            self.slots["case-" + str(number) + "/host-command.json"] = 65536
            self.slots["case-" + str(number) + "/tree-summary.json"] = 8192
        need(len(self.slots) == 54, "changed finite exporter slots")
        for name in (*self.slots, "index.json"):
            need(not os.path.lexists(self.root / self.flat(name)), "source-control export namespace already exists")
        self.fd = os.open(self.root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            self.preserve()
        except BaseException as primary:
            try:
                self.close()
            except BaseException as cleanup:
                raise BaseExceptionGroup("evidence admission and descriptor closure failed", [primary, cleanup]) from None
            raise

    @staticmethod
    def root_identity(info):
        return (info.st_dev, info.st_ino, info.st_uid, info.st_gid, info.st_mode)

    @staticmethod
    def flat(name):
        relative(name)
        return "source-controls-package-json-v1-" + name.replace("/", "--")

    def preserve(self):
        need(not self.root.is_symlink() and self.root_identity(self.root.lstat()) == self.identity
             and self.root_identity(os.fstat(self.fd)) == self.identity, "existing Unit2 root identity changed")
        for name, raw in self.old.items():
            need(read_regular(self.root / name, 1048576) == raw, "old completed Unit2 evidence changed: " + name)

    def create(self, name, raw, cap, source_path=None):
        need(type(raw) is bytes and len(raw) <= cap and self.written_bytes + len(raw) <= EXPORT_LIMIT,
             "complete export exceeds its declared byte envelope; originals retained")
        self.preserve()
        flat = self.flat(name)
        fd = os.open(flat, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=self.fd)
        try:
            offset = 0
            while offset < len(raw):
                count = os.write(fd, raw[offset:offset + 65536])
                need(count > 0, "short exclusive evidence write")
                offset += count
            os.fsync(fd)
        except BaseException as primary:
            try:
                os.close(fd)
            except BaseException as cleanup:
                raise BaseExceptionGroup("evidence write and descriptor closure failed", [primary, cleanup]) from None
            raise
        else:
            os.close(fd)
        need(read_regular(self.root / flat, cap) == raw, "flat evidence readback differs from original bytes")
        self.written_bytes += len(raw)
        self.written.append({"original_path": name, "read_from": str(source_path) if source_path is not None else None,
                             "flat_file": descriptor(flat, raw)})
        self.preserve()

    def export(self, suite, primary=None):
        try:
            self._export(suite, primary)
        except BaseException as operation:
            try:
                self.preserve()
            except BaseException as preservation:
                raise BaseExceptionGroup("evidence export and old-root preservation failed", [operation, preservation]) from None
            raise

    def _export(self, suite, primary=None):
        need(not self.attempted, "no automatic evidence-export retry")
        self.attempted = True
        missing = []
        # Copy complete original case evidence before the terminal summary.
        # Only the final, fully verified index commits an export; an absent or
        # incomplete index makes even a passed complete.json non-authoritative.
        ordered = [name for name in self.slots if name != "complete.json"] + ["complete.json"]
        for name in ordered:
            cap = self.slots[name]
            if name == "complete.json" and primary is None:
                need(missing == ["error.json"], "successful evidence export is missing an original output")
            raw = None
            path = suite.evidence / name
            if name == "error.json" and primary is not None:
                raw = encoded(exception_record(primary))
            elif suite.work_created and os.path.lexists(path):
                raw = read_regular(path, cap)
            elif name.startswith("case-"):
                case_name, original = name.split("/", 1)
                number = int(case_name.removeprefix("case-"))
                if original == "host-command.json" and number in suite.host_commands:
                    raw = encoded(suite.host_commands[number])
                elif suite.work_created and original in EVIDENCE and number == suite.preparations:
                    partial = suite.work / "unit2-case" / original
                    if os.path.lexists(partial):
                        raw = read_regular(partial, cap)
                        path = partial
            if raw is None:
                missing.append(name)
                continue
            self.create(name, raw, cap, path if suite.work_created and os.path.lexists(path) else None)
        self.preserve()
        index = {"schema": "oxid-package-json-source-controls-flat-evidence-v1", "status": "failed" if primary is not None else "passed-source-controls-only",
                 "entries": self.written.copy(), "absent_expected_paths": missing,
                 "aggregate_bytes_before_index": self.written_bytes, "aggregate_limit_bytes": EXPORT_LIMIT,
                 "file_limit_including_index": 55, "original_unit2_inputs": [descriptor(name, raw) for name, raw in self.old.items()],
                 "source_or_execution_qualification_from_exports": False,
                  "completion_rule": "require this final index, all exact entries and unchanged old Unit2 inputs"}
        if primary is None:
            need(missing == ["error.json"], "successful evidence export is missing an original output")
        need(len(self.written) < 55, "exported file count exceeded")
        self.create("index.json", encoded(index), 65536)

    def close(self):
        fd, self.fd = self.fd, None
        if fd is not None:
            os.close(fd)


def process_group_absent(pgid):
    try:
        os.killpg(pgid, 0)
    except ProcessLookupError:
        return True
    return False


def stop_preparation_group(child, primary):
    """Own only this compatibility group; preserve every cleanup failure."""
    failures = [primary]
    try:
        os.killpg(child.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    except BaseException as cleanup:
        failures.append(cleanup)
    try:
        child.wait(timeout=5)
    except BaseException as cleanup:
        failures.append(cleanup)
    try:
        deadline = time.monotonic() + 5
        while not process_group_absent(child.pid):
            need(time.monotonic() < deadline, "preparation descendants remain after group termination")
            time.sleep(0.025)
    except BaseException as cleanup:
        failures.append(cleanup)
    if len(failures) > 1:
        raise BaseExceptionGroup("preparation operation and process-group cleanup failed", failures) from None


def commands_for(repo, output):
    python = sys.executable
    base = output / "unit2"
    return {
        "compatibility": {"argv": [python, "-B", str(output / "compatibility/typed_project_unit3_compatibility/run.py"),
            "--repo", str(repo), "--source-manifest", str(repo / PACKAGE / "current-source.json"),
            "--output", str(base), "--cargo", "cargo", "--rustc", "rustc", "--prepare-only"], "cwd": str(repo)},
        "unit2": {"argv": [python, "-B", str(base / "derived-package/run.py"), "--repo", str(repo),
            "--output", str(base / "run"), "--cargo", "cargo", "--rustc", "rustc", "--prepare-only"], "cwd": str(repo)},
        "assertions": {"argv": [python, "-B", "-c", "import sys; print(sys.flags.optimize); assert __debug__"],
                       "cwd": str(base / "run")},
        "materialize": {"argv": [python, "-B", str(base / "derived-package/semantic/materialize.py"),
                                str(base / "run/observer-inputs")], "cwd": str(base / "run")},
    }


class SourceControls(unittest.TestSuite):
    """Fail closed as one explicit suite; no hidden second unittest command."""
    def __init__(self):
        super().__init__()
        self.failed = False
        self.preparations = 0
        self.host_commands = {}
        self.original_failures = []
        self.work_created = False

    def countTestCases(self):
        return 92

    def healthy(self):
        need(not self.failed and not self.result.shouldStop, "source-control batch stopped after failure")
        self.effects.assert_no_residue()
        free = os.statvfs(self.work)
        need(free.f_bavail * free.f_frsize >= 256 * 1024 * 1024 and free.f_favail >= 32768,
             "insufficient fresh CI temporary capacity")

    def raw_runtime(self):
        return load_buffer("_fresh_published_source_retained", self.repo / RETAINED, self.bodies[RETAINED])

    @contextmanager
    def isolated_repo_case(self, specification):
        self.healthy()
        need(set(specification) <= {"replace", "remove", "add", "symlink", "directory", "mode"},
             "unapproved selected-source negative mutation")
        root = Path(tempfile.mkdtemp(prefix="source-negative-", dir=self.work))
        expected = dict(self.fixture["current_inputs"])
        for name, raw in expected.items():
            write_new(root / relative(name), raw, 2097152)
        try:
            for name in specification.get("remove", []):
                (root / relative(name)).unlink()
            for name, raw in specification.get("replace", {}).items():
                need(name in expected and type(raw) is bytes, "invalid source replacement")
                (root / name).write_bytes(raw)
            for name, raw in specification.get("add", {}).items():
                need(name not in expected, "negative addition overlaps source")
                write_new(root / relative(name), raw, 2097152)
            for name, target in specification.get("symlink", {}).items():
                need(name == "src/main.rs" and target == "../Cargo.toml", "unapproved negative symlink")
                (root / name).unlink()
                (root / name).symlink_to(target)
            for name in specification.get("directory", []):
                (root / relative(name)).mkdir()
            for name, mode in specification.get("mode", {}).items():
                need(name == "src/main.rs" and mode == 0o755, "unapproved negative mode")
                (root / name).chmod(mode)
            yield root
        except BaseException:
            self.failed = True
            raise
        finally:
            # Expected malformed members are restored before exact cleanup.
            errors = []
            try:
                for name in specification.get("symlink", {}):
                    need((root / name).is_symlink(), "negative symlink disappeared")
                    (root / name).unlink()
                    (root / name).write_bytes(expected[name])
                for name in specification.get("mode", {}):
                    (root / name).chmod(0o644)
                for name in specification.get("remove", []):
                    (root / name).write_bytes(expected[name])
                for name in specification.get("replace", {}):
                    (root / name).write_bytes(expected[name])
                for name in specification.get("add", {}):
                    (root / name).unlink()
                for name in specification.get("directory", []):
                    (root / name).rmdir()
                actual = snapshot_generated(root, 376, 128)
                need(actual[0] == {name: descriptor(name, raw) for name, raw in expected.items()},
                     "negative selected-source fixture changed unexpectedly")
                clean_exact(root, actual)
            except BaseException:
                self.failed = True
                raise

    @contextmanager
    def prepared_unit2_case(self):
        self.healthy()
        need(self.preparations < 3, "more than the three prescribed genuine preparations")
        c = self.modules["published_current"]
        capture = c.preflight(self.repo, self.package, self.seal)
        self.effects.assert_no_residue()
        self.preparations += 1
        output = self.work / "unit2-case"
        need(not os.path.lexists(output), "previous preparation not reconciled")
        output.mkdir()
        evidence = self.evidence / ("case-" + str(self.preparations))
        evidence.mkdir()
        rows = commands_for(self.repo, output)
        self.host_commands[self.preparations] = {**rows["compatibility"],
            "record_kind": "source-control-host-launch", "pid": None, "exit_status": None}
        env = dict(os.environ, CARGO_BUILD_JOBS="2", CARGO_INCREMENTAL="0", PYTHONDONTWRITEBYTECODE="1")
        env.pop("PYTHONOPTIMIZE", None)
        self.effects.preparing = True
        completed = False
        child = None
        group_owned = False
        try:
            installed = c.install(self.raw_runtime(), self.package, self.seal)
            seam = installed.prepare_unit2(output, capture)
            with (output / "runner.stdout").open("xb") as out, (output / "runner.stderr").open("xb") as err:
                child = subprocess.Popen(rows["compatibility"]["argv"], cwd=self.repo, env=env,
                                         stdout=out, stderr=err, start_new_session=True)
                group_owned = True
                self.host_commands[self.preparations]["pid"] = child.pid
                code = child.wait(timeout=180)
                self.host_commands[self.preparations]["exit_status"] = code
            write_new(evidence / "host-command.json", encoded({**self.host_commands[self.preparations],
                "stdout": descriptor("runner.stdout", read_regular(output / "runner.stdout", LOG_LIMIT)),
                "stderr": descriptor("runner.stderr", read_regular(output / "runner.stderr", LOG_LIMIT))}), 65536)
            need(code == 0, "genuine compatibility preparation failed")
            verify_preparation_commands(output, rows)
            need(process_group_absent(child.pid), "successful preparation left a live process group")
            group_owned = False
            # This numeric PGID is now closed and may never be targeted again.
            # Genuine verifier before every case; no manufactured preparation.
            installed.verify_unit2_result(output, capture, seam, True)
            before = snapshot_generated(output, 16948, 10059)
            need(len(before[0]) == 16948 and len(before[1]) == 10059
                 and roster_sha(before[0]) == "13446b678d7c497028c3803b072d7c5d39225ae9c8c713d28fac4955d6a3da37"
                 and roster_sha(before[1] | {"."}) == "2504ca6427c2bb94044d137ab34431a219496284897ac0dd2651f07a5aadb838",
                 "genuine preparation finite roster differs from reviewed cost")
            # The accepted test installs its own wrapper on this NEW raw module.
            yield {"runtime": self.raw_runtime(), "output": output, "captured": capture, "seam": seam}
            need(not self.result.shouldStop, "test failure halted preparation batch")
            installed.verify_unit2_result(output, capture, seam, True)
            after = snapshot_generated(output, 16948, 10059)
            need(before == after, "prepared tree not restored after its genuine control")
            for name in EVIDENCE:
                limit = LOG_LIMIT if name in STREAMS else 131072
                write_new(evidence / name, read_regular(output / name, limit), limit)
            write_new(evidence / "tree-summary.json", encoded({"files": len(after[0]), "directories": len(after[1]) + 1,
                "file_roster_sha256": roster_sha(after[0]), "directory_roster_sha256": roster_sha(after[1] | {"."}),
                "all_generated_bodies_preserved": True, "source_checkpoint_head": SOURCE_HEAD,
                "prepare_only": True, "compiler_executions": 0}), 8192)
            clean_exact(output, after)
            completed = True
        except BaseException as primary:
            self.failed = True
            if group_owned:
                stop_preparation_group(child, primary)
                group_owned = False
            raise
        finally:
            self.effects.preparing = False
            if child is not None:
                self.host_commands[self.preparations]["exit_status"] = child.returncode
            if not completed:
                # Retain unexpected partial output and all evidence; no retry,
                # no next M view, and no opportunistic failure-tree deletion.
                self.failed = True

    def construct(self):
        m = self.modules
        t, c, v = (m[name] for name in ("published_transition", "published_current", "published_view"))
        raw, bodies = c._current_package(self.repo, self.package, self.seal)
        _, _, api = c.retained_current.capture_retained(self.package)
        admitted = t.admit(self.repo, bodies, self.seal, api)
        need(t.SOURCE_CHECKPOINT == {"head": SOURCE_HEAD, "tree": SOURCE_TREE, "parent": M_HEAD}
             and t.PUBLISHED_MAIN["head"] == M_HEAD and t.HISTORICAL_SOURCE["head"] == HISTORICAL_HEAD,
             "published, historical and current source identities were conflated")
        self.fixture = {"repo": self.repo, "package": self.package, "seal": self.seal,
                        "package_bytes": bodies, "package_manifest": raw,
                        "current_inputs": admitted["inputs"], "predecessor_inputs": admitted["predecessor_inputs"],
                        "public_api": api}
        consumer = m["_published_source_real_consumer"]
        consumer_path = dict(MODULES)["_published_source_real_consumer"]
        boundary = {"verify": lambda package_bytes, manifest_raw:
                        consumer.verify_source_entrypoint(package_bytes["run.py"], manifest_raw),
                    "rejection_type": ValueError, "implementation_bytes": self.bodies[consumer_path],
                    "identity": descriptor(consumer_path, self.bodies[consumer_path])}
        source_tests = m["test_published_transition"]
        unit = source_tests.build_unit_suite(t, c, v,
            retained_source_manifest_bytes=bodies[t.PREDECESSOR_NAME],
            retained_package_manifest_bytes=self.bodies[_CONFIG["historical_package_manifest"]],
            retained_runner_bytes=self.bodies[RETAINED])
        integration = source_tests.build_integration_suite(t, c, v, fixture=self.fixture,
            isolated_repo_case=self.isolated_repo_case, prepared_unit2_case=self.prepared_unit2_case,
            consumer_boundary=boundary)
        # Genuine view controls exercise the full outer capture, including its
        # published_predecessor. Keep the integration's pure admission separate.
        self.effects.assert_no_residue()
        need(not os.path.lexists(self.work / "unit2-case"),
             "cannot capture a view fixture while a preparation tree remains")
        view_capture = c.preflight(self.repo, self.package, self.seal)
        self.effects.assert_no_residue()
        need(c.same(view_capture["inputs"], admitted["inputs"])
             and c.same(view_capture["published_predecessor"]["inputs"], admitted["predecessor_inputs"]),
             "genuine view capture disagrees with the integration source admission")
        # _capture itself is read-only; preflight's genuine M/older views above
        # have completely closed before any preparation or view test starts.
        _, expected_m = v._capture(self.repo, view_capture, self.seal["predecessor_view"])
        view = m["test_published_view"].build_suite(v, c.adapters.Reject, repo=self.repo,
            admitted=view_capture, seal=self.seal["predecessor_view"], expected_bodies=expected_m, effects=self.effects)
        ci = m["test_published_ci_inventory"].build_suite(m["published_ci_inventory"], c.adapters.Reject,
            audit=self.bodies[_CONFIG["ci_data"]["audit"]], oldest=self.bodies[_CONFIG["ci_data"]["oldest"]],
            middle=self.bodies[_CONFIG["ci_data"]["middle"]],
            old_dispositions=self.bodies[_CONFIG["ci_data"]["old_dispositions"]],
            source_descriptor=copy.deepcopy(self.seal["source_manifest"]))
        public = unittest.defaultTestLoader.loadTestsFromTestCase(
            m["_published_original_public_tests"].CurrentLifecycle)
        for name, suite, count in (("source_unit", unit, 37), ("source_integration", integration, 15),
                                   ("view", view, 14), ("ci", ci, 20), ("public_original", public, 6)):
            cases = list(suite)
            need(suite.countTestCases() == len(cases) == count
                 and tuple(case._testMethodName for case in cases) == tuple(_CONFIG["test_rosters"][name]),
                 "changed exact CI control roster: " + name)
            need(all(not getattr(case, "__unittest_skip__", False)
                     and not getattr(getattr(case, case._testMethodName), "__unittest_skip__", False)
                     and not getattr(getattr(case, case._testMethodName), "__unittest_expecting_failure__", False)
                     for case in cases), "disabled CI source control")
        # Run the complete source integration before view fault-injection tests;
        # positive preparation precedes both fresh negative preparations.
        return unittest.TestSuite([unit, integration, view, ci, public])

    def run(self, result, debug=False):
        self.result = result
        case = unittest.FunctionTestCase(lambda: None, description="published formatter source-control setup")
        old_failfast = result.failfast
        prior_modules = dict(sys.modules)
        previous_temp = tempfile.tempdir
        failure = None
        try:
            need(not debug, "debug mode is not an admitted CI execution route")
            need(sys.flags.optimize == 0 and __debug__ and sys.dont_write_bytecode
                 and threading.active_count() == 1, "source controls require one assertion-enabled -B host")
            self.actual_repo = ACTUAL_REPOSITORY
            self.current_runtime, self.current_capture = bootstrap_successor(self.actual_repo)
            view_module = self.current_runtime._modules['predecessor']
            self.accepted_scope = view_module.view(
                self.actual_repo, self.current_capture['package_json_predecessor']['inputs'],
                self.current_capture['package_json_seal']['predecessor_view'])
            self.repo = self.accepted_scope.__enter__()
            self.accepted_entered = True
            self.package = self.repo / PACKAGE
            self.bodies = authenticate(self.repo, _CONFIG)
            self.seal = copy.deepcopy(_CONFIG["source_seal"])
            actual_bodies = {
                PACKAGE + '/run.py': self.current_capture['package_bytes']['run.py'],
                PACKAGE + '/package-manifest.json': self.current_capture['package_manifest']}
            self.exporter = ExistingUnit2Evidence(
                self.actual_repo, {'source_seal': self.current_capture['package_json_seal']},
                actual_bodies, self.current_runtime, self.current_capture)
            self.work = self.exporter.root.parent / "oxid-package-json-source-ci-v1"
            self.evidence = self.work / "evidence"
            need(not os.path.lexists(self.work), "source-control output already exists; no automatic rerun")
            checkout = checkout_identity(self.actual_repo)
            checkout['controls_stage'] = 'accepted-formatter-predecessor'
            checkout['actual_current_source'] = self.current_capture['current']['package_json_source']
            self.work.mkdir(mode=0o700)
            self.work_created = True
            self.evidence.mkdir()
            write_new(self.evidence / "checkout.json", encoded(checkout), 8192)
            self.effects = ViewEffects(self.work)
            tempfile.tempdir = str(self.work)
            self.healthy()
            result.failfast = True
            with mock.patch.object(tempfile, "TemporaryDirectory", self.effects.temporary):
                self.modules = {name: load_buffer(name, self.repo / path, self.bodies[path], True)
                                for name, path in MODULES}
                suite = self.construct()
                before = result.testsRun
                # Preserve original unittest exception objects without changing
                # any assertion or result callback behavior.
                def remember(method, is_subtest=False):
                    def record(*args):
                        error = args[-1] if is_subtest else args[1]
                        if error is not None:
                            self.original_failures.append(error[1])
                        return method(*args)
                    return record
                with ExitStack() as result_scope:
                    for name in ("addError", "addFailure", "addSubTest"):
                        result_scope.enter_context(mock.patch.object(result, name,
                            side_effect=remember(getattr(result, name), name == "addSubTest")))
                    suite.run(result)
                if self.original_failures:
                    raise BaseExceptionGroup("original CI source-control failures", self.original_failures)
                need(not result.shouldStop and not self.failed and result.testsRun - before == 92
                     and self.preparations == 3, "CI source controls failed or did not complete their exact roster")
                self.effects.assert_no_residue()
            for name, raw in self.bodies.items():
                need(read_regular(self.repo / name) == raw, "final CI pinned input changed: " + name)
            # Verify and close the complete outer fixture BEFORE committing
            # a successful completion/index. Preservation failure exports failure.
            self.modules = None
            self.fixture = None
            self.accepted_entered = False
            self.accepted_scope.__exit__(None, None, None)
            write_new(self.evidence / "complete.json", encoded({"status": "passed-source-controls-only",
                "source_unit": 37, "source_integration": 15, "genuine_view": 14, "ci_transport": 20,
                "original_public_lifecycle": 6, "additional_public_lifecycle_pending": 36,
                "fresh_preparations": 3, "preparation_child_launches": 12, "git_metadata_launches": 2,
                "maximum_live_preparation_processes_including_host": 4,
                "compiler_executions": 0, "execution_qualified": False, "checkout": checkout, "accepted_fixture_view": {"files":2724,"logical_bytes":78507317,"scope":"entire predecessor control batch; closed before terminal export","separate_from_nested_view_effect_audit":True}}), 16384)
            self.exporter.export(self)
            stream = getattr(result, "stream", None)
            if stream is not None:
                stream.write("\nPublished source controls: 37 unit, 15 genuine integration, 14 genuine view, 20 CI, 6 original public; "
                             + checkout["form"] + " checkout " + checkout["checkout_head"] + "\n")
        except BaseException as primary:
            self.failed = True
            failure = sys.exc_info()
            if hasattr(self, "exporter") and not self.exporter.attempted and hasattr(self, "evidence"):
                try:
                    self.exporter.export(self, primary)
                except BaseException as export_error:
                    combined = BaseExceptionGroup("source controls and complete evidence export failed", [primary, export_error])
                    failure = (type(combined), combined, combined.__traceback__)
        finally:
            if hasattr(self, "exporter"):
                try:
                    self.exporter.close()
                except BaseException as cleanup:
                    self.failed = True
                    if failure is None:
                        failure = sys.exc_info()
                    else:
                        combined = BaseExceptionGroup("source controls and evidence descriptor closure failed", [failure[1], cleanup])
                        failure = (type(combined), combined, combined.__traceback__)
            result.failfast = old_failfast
            tempfile.tempdir = previous_temp
            # Restore aliases without retaining modules rooted in closed views.
            for name in tuple(sys.modules):
                if name in prior_modules:
                    sys.modules[name] = prior_modules[name]
                else:
                    module = sys.modules[name]
                    file = getattr(module, "__file__", "")
                    if name in dict(MODULES) or (isinstance(file, str) and hasattr(self, "work")
                                                and file.startswith(str(self.work) + "/")):
                        del sys.modules[name]
        if getattr(self, 'accepted_entered', False):
            self.modules = None
            self.fixture = None
            try:
                self.accepted_scope.__exit__(None, None, None)
            except BaseException as cleanup:
                self.failed = True
                if failure is None:
                    failure = sys.exc_info()
                else:
                    combined = BaseExceptionGroup('controls and accepted-view closure failed', [failure[1], cleanup])
                    failure = (type(combined), combined, combined.__traceback__)
            finally:
                self.accepted_entered = False
        if failure is not None:
            result.startTest(case)
            result.addError(case, failure)
            result.stopTest(case)
            result.stop()
        return result


def load_tests(loader, tests, pattern):
    return SourceControls()

_SUCCESSOR = {'dispatcher': {'path': 'tests/fixtures/typed_project_source_binding/run.py', 'bytes': 6774, 'sha256': 'a2fbd5c06b82ad5d3ffc74d15a49ecf58efe2a24a6e98cacf328e5ecd45fc184'}, 'source_seal': {'predecessor_view': {'bytes': 1023787, 'path': 'tests/qualification/package_json_current_v1/accepted-view-v1.json', 'sha256': '60d41866e80fba0fc9b6d69263cfc6865aa2cdff6684dea01c01e3555dcaa867'}, 'source_manifest': {'bytes': 76328, 'path': 'package-json-source-v1.json', 'sha256': 'd33ecc4e41975a062790540ba9e9a270d7c6043a31f8f4b9a8d7a044c79530b0'}}}

# This configuration is independent of the retained v1 test-fixture roster.
ACTUAL_REPOSITORY = Path(__file__).absolute().parents[3]


def bootstrap_successor(repo):
    row = _SUCCESSOR['dispatcher']
    raw = read_regular(repo / row['path'])
    need(descriptor(row['path'], raw) == row, 'independently pinned successor dispatcher')
    runtime = load_buffer('package_json_hosted_current', repo / row['path'], raw)
    need(runtime._SOURCE_SEAL == _SUCCESSOR['source_seal'], 'successor source seal')
    captured = runtime.preflight(repo)
    need(captured['current']['reviewed_source_head'] == '858c70d862587b6a8b5d59a70afe4b8334b1f150'
         and captured['current']['source_only_tree'] == 'c44b7dfb2938dc510016ff4939104a2236ba2e9c',
         'actual published successor source association')
    return runtime, captured


def verify_successor_receipt(value, expected):
    # Canonical bytes distinguish bool/int and retain every nested predecessor.
    need(type(value) is dict and type(expected) is dict, 'complete source receipt dictionaries')
    for key, body in expected.items():
        need(key in value and encoded(value[key]) == encoded(body),
             'current execution outer provenance differs: ' + key)

