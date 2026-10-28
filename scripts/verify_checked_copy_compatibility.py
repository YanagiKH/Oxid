#!/usr/bin/env python3
"""Replay the retained bounded scalar/record assertions after checked Copy.

Only the exact reviewed compiler delta and an unparsed fixture initializer are
admitted. Checked Copy language/runtime behavior is qualified separately. All
predecessor policies, observer controls and resource predicates remain frozen.
"""
import argparse
import copy
import datetime
import hashlib
import itertools
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import sys
import types

PREDECESSOR = "18da51d8c3bdc013ba02ab183b4b639bd8afbce0"
COMPILER_PREDECESSOR = "85e427eff0e25fba2c332755fcdbf1a531ca9677"
PREDECESSOR_POLICY = "scripts/verify_typed_vector_compatibility.py"
PREDECESSOR_POLICY_BYTES = 306460
PREDECESSOR_POLICY_SHA = '6fb58c5296eb64d9462e849cbc06056d6852dd1a9323e124a219b5fc407166ca'
# Final reviewed source pins are admission authority, not execution evidence.
PINS_PROVISIONAL = False
CHECKED_COPY_CHANGED = frozenset(['build.rs',
 'src/frontend/ast.rs',
 'src/frontend/declaration_index/generics.rs',
 'src/frontend/declaration_index/generics/nominal.rs',
 'src/frontend/declaration_index/generics/vector_tests.rs',
 'src/frontend/declaration_index/sealed.rs',
 'src/frontend/declaration_index/source_owner.rs',
 'src/frontend/format/ast_tests.rs',
 'src/frontend/oir/owned/builtin_input_fixtures.rs',
 'src/frontend/oir/owned/composition_native_tests.rs',
 'src/frontend/oir/owned/composition_reference_tests.rs',
 'src/frontend/oir/owned/consumer_fixtures.rs',
 'src/frontend/oir/owned/execute.rs',
 'src/frontend/oir/owned/execute_composition.rs',
 'src/frontend/oir/owned/execute_resource_tests.rs',
 'src/frontend/oir/owned/execute_tests.rs',
 'src/frontend/oir/owned/flow.rs',
 'src/frontend/oir/owned/mod.rs',
 'src/frontend/oir/owned/native.rs',
 'src/frontend/oir/owned/native_tests.rs',
 'src/frontend/oir/owned/oracle_tests.rs',
 'src/frontend/oir/owned/origin_tests.rs',
 'src/frontend/oir/owned/plan.rs',
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs',
 'src/frontend/oir/owned/reviewer_heldout.rs',
 'src/frontend/oir/owned/reviewer_origins.rs',
 'src/frontend/oir/owned/reviewer_reference_tests.rs',
 'src/frontend/oir/owned/shape.rs',
 'src/frontend/oir/owned/source/array_pipeline_rows.rs',
 'src/frontend/oir/owned/source/association.rs',
 'src/frontend/oir/owned/source/budget_tests.rs',
 'src/frontend/oir/owned/source/byte_storage_raw_tests.rs',
 'src/frontend/oir/owned/source/byte_storage_resources.rs',
 'src/frontend/oir/owned/source/candidate_adapter.rs',
 'src/frontend/oir/owned/source/candidate_native.rs',
 'src/frontend/oir/owned/source/diagnostic.rs',
 'src/frontend/oir/owned/source/hir.rs',
 'src/frontend/oir/owned/source/lower.rs',
 'src/frontend/oir/owned/source/mod.rs',
 'src/frontend/oir/owned/source/record_enum_layouts.rs',
 'src/frontend/oir/owned/source/resolve.rs',
 'src/frontend/oir/owned/source/resolver_inventory_tests.rs',
 'src/frontend/oir/owned/source/resolver_paid_tests.rs',
 'src/frontend/oir/owned/source/resource_native_tests.rs',
 'src/frontend/oir/owned/source/reviewer_source.rs',
 'src/frontend/oir/owned/source/type_storage_tests.rs',
 'src/frontend/oir/owned/source/typeck.rs',
 'src/frontend/oir/owned/source/u8_tests.rs',
 'src/frontend/oir/owned/tests.rs',
 'src/frontend/oir/owned/vector_raw_tests.rs',
 'src/frontend/oir/owned/verified.rs',
 'src/frontend/oir/owned_types.rs',
 'src/frontend/oir/owned_types/array_tests.rs',
 'src/frontend/oir/owned_types/composition.rs',
 'src/frontend/parser.rs',
 'src/frontend/parser/enum_syntax_tests.rs',
 'src/frontend/parser/project_tests.rs',
 'tests/typed_frontend.rs'])
CHECKED_COPY_ADDED = frozenset(['src/frontend/oir/owned/copy_record_native_tests.rs',
 'src/frontend/oir/owned/source/copy_record_tests.rs',
 'src/frontend/oir/owned_types/copy_tests.rs',
 'src/frontend/parser/copy_record_tests.rs'])
CHECKED_COPY_INPUTS = {'build.rs': (2323, 'fd036f9bc355fcf70561ece365476652a362a0006784f913383a2d08d3af7ef0'),
 'src/frontend/ast.rs': (77941, 'f08ac1790409615c0269d36d0746b2357bf18ee828c82476e8a093a2d4047209'),
 'src/frontend/declaration_index/generics.rs': (132039,
                                                'db784e106e645fd13ec34dc3307f2df3400eb4c910cb99961cb1577b582da6b8'),
 'src/frontend/declaration_index/generics/nominal.rs': (25943,
                                                        'cf42c6cbb99b96eea44d330c716859579f6ac2d2033ae19e882db6c2697a13b8'),
 'src/frontend/declaration_index/generics/vector_tests.rs': (21975,
                                                             'acd6ddbb63feeb2e2f19cb99a15d842640c7c4f87e06d30bb787954303122328'),
 'src/frontend/declaration_index/sealed.rs': (107823,
                                              '3b7ec9e04b1ac80a4c6afac29c59fd2e0f15ac809e79a8692ebbd9c71b3564ea'),
 'src/frontend/declaration_index/source_owner.rs': (24761,
                                                    '89c32314d044611dbb22326f512f08a0c68c0798c8ed6a65d0fb1e6de782efc6'),
 'src/frontend/format/ast_tests.rs': (78827,
                                      'b23ae22304d02350e2ec046e9c9fd1b981fc29bda89185fd038322cda33ce879'),
 'src/frontend/oir/owned/builtin_input_fixtures.rs': (20528,
                                                      'd5f523c2077cbd449d687fbef33884d3e8b7a3515107bb2c23eed5a225897c47'),
 'src/frontend/oir/owned/composition_native_tests.rs': (24512,
                                                        'f3e10ca2d9946a5afc5b3370e7ff7f4bbdc46cb2e85b2bf94ec3c250b5c003fd'),
 'src/frontend/oir/owned/composition_reference_tests.rs': (29147,
                                                           '7697f8ca444c594d4358e532c5c3c960b16acb252c21f3ee6287cbdf03ae2127'),
 'src/frontend/oir/owned/consumer_fixtures.rs': (46238,
                                                 'b54c6f663d2c00d7aa70a853bdf521325680dc4d7fd18804a9148040e4da1f32'),
 'src/frontend/oir/owned/copy_record_native_tests.rs': (9434,
                                                        '96634ce52dfcf3a71f5c8c6c905f7b3c66214d87a25f22b4ac3da3962e480fb9'),
 'src/frontend/oir/owned/execute.rs': (215139,
                                       '85636d136b19018adeb566321c97cc8791e6c0f32fcc026c453eec8e8e519299'),
 'src/frontend/oir/owned/execute_composition.rs': (11228,
                                                   'e61e0639f513fafa8f558c66eea99f391434baf3f1ef2eebe7fffea25f130cd8'),
 'src/frontend/oir/owned/execute_resource_tests.rs': (5445,
                                                      '68bcf75c1f214e5f4f51e2ba1ab531badad70e4fc025e6247dda9d502a675634'),
 'src/frontend/oir/owned/execute_tests.rs': (55311,
                                             'e990323880bd2201668041b038dccd17508a311e389ae0d10eafc273437d6195'),
 'src/frontend/oir/owned/flow.rs': (54749,
                                    '8d6491054c085da8c2e3847970358cbbefdafcb38fd1f9b71e4daf5b940c1a67'),
 'src/frontend/oir/owned/mod.rs': (17198, '7327725e55986cb9d91c73ab321bfb59e1d83580213d1932ce4944592382fd16'),
 'src/frontend/oir/owned/native.rs': (272460,
                                      '64d4f8473b45866c3e3f95ce12b4b6287aa0dd0176065feeb40db3ee3f84b409'),
 'src/frontend/oir/owned/native_tests.rs': (91349,
                                            'dea6dc68a394da87ab0b7f47cbbebc9790924959e77d23a95b40071dc394517a'),
 'src/frontend/oir/owned/oracle_tests.rs': (115078,
                                            '4ecfdfd1804bb2befaee35b72a38a7332b8081ec2cc083f8277ebb7753af0625'),
 'src/frontend/oir/owned/origin_tests.rs': (22232,
                                            'b5602bbb71222bade380352bb010bd18d7005587c52e941e469e9c78ca426908'),
 'src/frontend/oir/owned/plan.rs': (34400,
                                    '5704b13e8bdfb3ec348ec24c1a1c620b7f0b38452be826c0cb097750aa15e384'),
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs': (66671,
                                                              '6d71a7b6ec0598059f9aecfd20fdf6115d1f9aba6df963afa5f69f88946f3a65'),
 'src/frontend/oir/owned/reviewer_heldout.rs': (58142,
                                                '0b9c7dc1d511fc04426db3860849e5833a54576815049e0a06f317be01155c51'),
 'src/frontend/oir/owned/reviewer_origins.rs': (23352,
                                                '06e00462084ade9cf9c1205ac19d721a38a867dbc04a01208d8002e332a2f102'),
 'src/frontend/oir/owned/reviewer_reference_tests.rs': (72304,
                                                        'bd3eb9b5730ca4882e327ca2b942a23f05acdd48252d80f92d037382130f7aa0'),
 'src/frontend/oir/owned/shape.rs': (62357,
                                     '4d88baaa106d3e6d40778efc696ebe6d4a1923d7ce9452540eb070bc8aa8a390'),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': (58151,
                                                          '0bcd2c5a335d545e345213645cce2c74350e4967abc147dcfd36adea974e750c'),
 'src/frontend/oir/owned/source/association.rs': (92016,
                                                  '4af47a9e594365b5fe9ea3045f5bda94983277923f1c38765055119e85f4174c'),
 'src/frontend/oir/owned/source/budget_tests.rs': (16969, 'ad8b26e56bff3bde84a511c0009c16a59871f307092eec0b9ef997121e9d35f4'),
 'src/frontend/oir/owned/source/byte_storage_raw_tests.rs': (10537,
                                                             '16e1bad3e78fa1b4cbcb75b46af6703685668d59379cc400ca569e936fae7f49'),
 'src/frontend/oir/owned/source/byte_storage_resources.rs': (10508,
                                                             '02adda29d53f7f6efa93d87eeaefbacd85e154c361aaded3610c8bb2c1554af2'),
 'src/frontend/oir/owned/source/candidate_adapter.rs': (44456,
                                                        '62fedbebc8fea1142d24e0c028ea8b679c7da980a197ce1e29033c5d418d2e7a'),
 'src/frontend/oir/owned/source/candidate_native.rs': (32777,
                                                       'b2d0d5d589d1f17d0a8ff0aa6c8ced32bb955a32cafec4714fd3cd09628a10c7'),
 'src/frontend/oir/owned/source/copy_record_tests.rs': (14748,
                                                        'e67fdccb81faa7326c7756e178967437954102e9f29a9d7511cb1c632fd23ed9'),
 'src/frontend/oir/owned/source/diagnostic.rs': (8349,
                                                 '2cec0d13c8d4bd2d98a78b8981ac4fb3fb5b24ef889fb006d6bf64ae7229cc00'),
 'src/frontend/oir/owned/source/hir.rs': (7475,
                                          'f3bd49a9047a30c00183ff5e745ac82653465dad4f612920730f45814543f867'),
 'src/frontend/oir/owned/source/lower.rs': (135332,
                                            '513472052dfa65cfd1d2661e1f7b5df2ba9b6acf74bcf93979f1dea3caf1f241'),
 'src/frontend/oir/owned/source/mod.rs': (2414,
                                          '69178b66d84bc3329de9290d39d1559625bb3ed869e5a93589bf8e2e33b776fd'),
 'src/frontend/oir/owned/source/record_enum_layouts.rs': (13512,
                                                          '805f58df1f8d30c21d0b39212f19dac4fae6c44d5bd66b738d3d11715630604d'),
 'src/frontend/oir/owned/source/resolve.rs': (169567,
                                              'e132f3d1c149e16ad3d886be6c5eeef8d9aecc013c9cebb6653497270e0a7dd7'),
 'src/frontend/oir/owned/source/resolver_inventory_tests.rs': (18357,
                                                               '1280b4e3d666a70f19d8961d5d6ead37bd528b324c96dce795e967cdd5f5e58d'),
 'src/frontend/oir/owned/source/resolver_paid_tests.rs': (114152,
                                                          'b76e759f556db2c16662d4b407ae3031099decd2214a6ecb6b7b746610bd95fd'),
 'src/frontend/oir/owned/source/resource_native_tests.rs': (14848,
                                                            'ab1708ae096725ffbbf9a01c8a758e115d7115d7be63fa221c769c21c9aa7eed'),
 'src/frontend/oir/owned/source/reviewer_source.rs': (40230,
                                                      '43f02d2e2459101a5d64ea472b30fa3522df8d73615f7fbf0af76a175760629f'),
 'src/frontend/oir/owned/source/type_storage_tests.rs': (117412,
                                                         'abc334e1b93b039f6785d9249936d95cc9c1105b1b5afd29b8793e651e69c3aa'),
 'src/frontend/oir/owned/source/typeck.rs': (222958,
                                             'ff1eafb776da88811c98c601c27e136a52a0b91197af94ec8227fe3128b03c3d'),
 'src/frontend/oir/owned/source/u8_tests.rs': (28506,
                                               '37f351c117f56ae4671380a1d27a9813ec9c11b84b1f36b62018294abbf01b1f'),
 'src/frontend/oir/owned/tests.rs': (79262,
                                     '1fe9b1f9f6b9eaaa23fbf9c9fcd5052007cf6f586fc6fac928286f03dabb54d7'),
 'src/frontend/oir/owned/vector_raw_tests.rs': (25612,
                                                '76c0383676df9427935e5a7cc0c071b7b33fe71a0bf1b855ef9ebf2588c2589b'),
 'src/frontend/oir/owned/verified.rs': (29743,
                                        '82929b5b04eaca44a01ff9043e95084b7861ca6b824852653257c5af9761a6e6'),
 'src/frontend/oir/owned_types.rs': (92642,
                                     '57a654a49c2bcc8314ddf21508bf693724bac167d66d1b4fc64d7787b4d9374a'),
 'src/frontend/oir/owned_types/array_tests.rs': (13673,
                                                 'c7711cfbe8247eef5f61e1d2d48c2c16b7977108ec2ebacabedada1d5cb77598'),
 'src/frontend/oir/owned_types/composition.rs': (19403,
                                                 '241b9d8fda3cebf78e54a718bead939ced1a995d2521e71a47dbdddb87ca072b'),
 'src/frontend/oir/owned_types/copy_tests.rs': (2777,
                                                'fdc811bbb0d833b7175c0f07f8646e1839b8346dd36b020882a1442e43f21668'),
 'src/frontend/parser.rs': (108037, '4652af04d7ec937a3371e22e8af20f3391d4fbed6dcba2791ba2700dba1336f0'),
 'src/frontend/parser/copy_record_tests.rs': (5076,
                                              'cccc115a1b54f61e6a480f16f2d2da466705157cc36fbba50b9845d2776d8b07'),
 'src/frontend/parser/enum_syntax_tests.rs': (29874,
                                              '5933428e4bca3cb7787b09fca307c2609890ca00bf6306209ccd6de12948e1fa'),
 'src/frontend/parser/project_tests.rs': (20739,
                                          'db15a633be81914819b1a953900767c1835878bb81e77430aa3c2c6b97e47c7a'),
 'tests/typed_frontend.rs': (16665, '480a98bf25597e4f04166f5fd02b3c2b2d9733df236c3ad17376f46f13d3d33d')}
# Ordered run-length encoding retains every complete selector byte and count.
REVIEWED_DIRECTIVES = {'build.rs': {'attributes': (),
              'build_script': b'fn main() {\n    println!("cargo:rerun-if-changed=native");\n    let targe'
                              b't = std::env::var("TARGET").expect("Cargo target");\n    // Match the support'
                              b"ed parser nesting envelope on Windows' primary thread.\n    if std::env::var("
                              b'"CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {\n        match std::env:'
                              b':var("CARGO_CFG_TARGET_ENV").as_deref() {\n            Ok("msvc") => println!'
                              b'("cargo:rustc-link-arg-bin=oxid=/STACK:8388608"),\n            Ok("gnu") => p'
                              b'rintln!("cargo:rustc-link-arg-bin=oxid=-Wl,--stack,8388608"),\n            _ '
                              b'=> (),\n        }\n    }\n    if matches!(\n        std::env::var("CARGO_CFG'
                              b'_TARGET_OS").as_deref(),\n        Ok("linux" | "macos")\n    ) && std::env'
                              b'::var("CARGO_CFG_TARGET_POINTER_WIDTH").as_deref() == Ok("64")\n        && st'
                              b'd::env::var("CARGO_CFG_TARGET_ENDIAN").as_deref() == Ok("little")\n        &&'
                              b' matches!(\n            std::env::var("CARGO_CFG_TARGET_ARCH").as_deref()'
                              b',\n            Ok("x86_64" | "aarch64")\n        )\n    {\n        let outpu'
                              b't = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_D'
                              b'IR"));\n        let compiler = std::env::var_os("RUSTC").expect("Cargo RUSTC"'
                              b');\n        let root = std::env::var("CARGO_MANIFEST_DIR").expect("Cargo mani'
                              b'fest path");\n        let status = std::process::Command::new(compiler)\n '
                              b'           .args([\n                "--edition=2021",\n                "--'
                              b'crate-name",\n                "oxid_float_format",\n                "--cra'
                              b'te-type=staticlib",\n                "--target",\n                &target,'
                              b'\n                "-Cpanic=abort",\n                "-Copt-level=3",\n     '
                              b'           "-Ccodegen-units=1",\n                "-Cembed-bitcode=no",\n  '
                              b'              &format!("--remap-path-prefix={root}=."),\n                "nat'
                              b'ive/float_format.rs",\n                "-o",\n            ])\n            .'
                              b'arg(output.join("float_format.a"))\n            .status()\n            .ex'
                              b'pect("build floating-point formatter");\n        assert!(status.success(), "f'
                              b'loating-point formatter build failed");\n    }\n\n    cc::Build::new()\n    '
                              b'    .file("native/oxid_ffi.c")\n        .include("native")\n        .warni'
                              b'ngs(false)\n        .compile("oxidffi_c");\n\n    cc::Build::new()\n        '
                              b'.cpp(true)\n        .file("native/oxid_cpp_bridge.cpp")\n        .include('
                              b'"native")\n        .flag_if_supported("-std=c++17")\n        .warnings(fal'
                              b'se)\n        .compile("oxidffi_cpp");\n}\n',
              'c_preprocessor': (),
              'legacy': ([], []),
              'macros': ()},
 'src/frontend/ast.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 9),
                                        (b'#[repr(u8)]', 1),
                                        (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 5),
                                        (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                        (b'#[cfg_attr(not(test), allow(dead_code))]', 1),
                                        (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                        (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 5),
                                        (b'#[derive(Debug, PartialEq, Eq)]', 3),
                                        (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2),
                                        (b'#[derive(Debug, PartialEq, Eq)]', 2),
                                        (b'#[derive(PartialEq, Eq)]', 1),
                                        (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                        (b'#[cfg_attr(not(test), allow(dead_code))]', 1),
                                        (b'#[derive(Debug, PartialEq, Eq)]', 2),
                                        (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                        (b'#[allow(dead_code)]', 1),
                                        (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                        (b'#[cfg_attr(not(test), allow(dead_code))]', 1),
                                        (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                        (b'#[derive(Debug, PartialEq, Eq)]', 6),
                                        (b'#[cfg_attr(not(test), allow(dead_code))]', 1),
                                        (b'#[derive(Debug, PartialEq, Eq)]', 3),
                                        (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                        (b'#[repr(u8)]', 1),
                                        (b'#[derive(Default)]', 1),
                                        (b'#[allow(clippy::too_many_arguments)]', 1),
                                        (b'#[cfg(test)]', 4),
                                        (b'#[allow(dead_code)]', 1),
                                        (b'#[cfg(test)]', 1),
                                        (b'#[test]', 1)),
                         'build_script': None,
                         'c_preprocessor': (),
                         'legacy': ([], []),
                         'macros': ()},
 'src/frontend/declaration_index/generics.rs': {'attributes': ((b'#[cfg(test)]', 1),
                                                               (b'#[derive(Default)]', 1),
                                                               (b'#[derive(Clone, Copy)]', 1),
                                                               (b'#[derive(Clone, Copy, Debug, PartialEq, '
                                                                b'Eq)]',
                                                                3),
                                                               (b'#[allow(dead_code)]', 1),
                                                               (b'#[derive(Debug, PartialEq, Eq)]', 3),
                                                               (b'#[allow(clippy::too_many_arguments)]', 1),
                                                               (b'#[derive(Clone, Copy)]', 2),
                                                               (b'#[allow(clippy::too_many_arguments)]', 1)),
                                                'build_script': None,
                                                'c_preprocessor': (),
                                                'legacy': ([], []),
                                                'macros': ()},
 'src/frontend/declaration_index/generics/nominal.rs': {'attributes': ((b'#[derive(Debug, PartialEq, Eq)]',
                                                                        2),
                                                                       (b'#[derive(Debug, Default, Partial'
                                                                        b'Eq, Eq)]',
                                                                        1),
                                                                       (b'#[allow(clippy::too_many_argumen'
                                                                        b'ts)]',
                                                                        3)),
                                                        'build_script': None,
                                                        'c_preprocessor': (),
                                                        'legacy': ([], []),
                                                        'macros': ()},
 'src/frontend/declaration_index/generics/vector_tests.rs': {'attributes': ((b'#[test]', 12),),
                                                             'build_script': None,
                                                             'c_preprocessor': (),
                                                             'legacy': ([], []),
                                                             'macros': ()},
 'src/frontend/declaration_index/sealed.rs': {'attributes': ((b'#[derive(Debug)]', 1),
                                                             (b'#[repr(C, align(8))]', 1),
                                                             (b'#[derive(Debug)]', 1),
                                                             (b'#[cfg(test)]', 3),
                                                             (b'#[derive(Debug)]', 3),
                                                             (b'#[cfg(test)]', 5),
                                                             (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                                             (b'#[cfg(test)]', 3),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 4),
                                                             (b'#[cfg(not(test))]', 1),
                                                             (b'#[allow(unused_mut)]', 1),
                                                             (b'#[cfg(test)]', 11),
                                                             (b'#[cfg(not(test))]', 1),
                                                             (b'#[cfg(test)]', 7),
                                                             (b'#[test]', 2),
                                                             (b'#[cfg(test)]', 3),
                                                             (b'#[test]', 5)),
                                              'build_script': None,
                                              'c_preprocessor': (),
                                              'legacy': ([], []),
                                              'macros': ()},
 'src/frontend/declaration_index/source_owner.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug)]', 2),
                                                                   (b'#[cfg(test)]', 1),
                                                                   (b'#[test]', 2),
                                                                   (b'#[cfg(target_pointer_width = "64")]',
                                                                    1),
                                                                   (b'#[test]', 1),
                                                                   (b'#[cfg(test)]', 1),
                                                                   (b'#[test]', 1)),
                                                    'build_script': None,
                                                    'c_preprocessor': (),
                                                    'legacy': ([], []),
                                                    'macros': ()},
 'src/frontend/format/ast_tests.rs': {'attributes': ((b'#[derive(Debug, PartialEq, Eq)]', 2),
                                                     (b'#[test]', 19)),
                                      'build_script': None,
                                      'c_preprocessor': (),
                                      'legacy': ([], []),
                                      'macros': ()},
 'src/frontend/oir/owned/builtin_input_fixtures.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, Partial'
                                                                      b'Eq, Eq)]',
                                                                      1),),
                                                      'build_script': None,
                                                      'c_preprocessor': (),
                                                      'legacy': ([], []),
                                                      'macros': ()},
 'src/frontend/oir/owned/composition_native_tests.rs': {'attributes': ((b'#[test]', 6),
                                                                       (b'#[cfg(all(\n    target_os = "linu'
                                                                        b'x",\n    target_pointer_width = "'
                                                                        b'64",\n    target_endian = "little'
                                                                        b'",\n    any(target_arch = "x86_64'
                                                                        b'", target_arch = "aarch64")\n))]',
                                                                        1),
                                                                       (b'#[test]', 1),
                                                                       (b'#[ignore = "requires pinned LLVM'
                                                                        b' 19.1.7 and one serialized nativ'
                                                                        b'e execution slot"]',
                                                                        1),
                                                                       (b'#[cfg(all(\n    target_os = "linu'
                                                                        b'x",\n    target_pointer_width = "'
                                                                        b'64",\n    target_endian = "little'
                                                                        b'",\n    any(target_arch = "x86_64'
                                                                        b'", target_arch = "aarch64")\n))]',
                                                                        1),
                                                                       (b'#[test]', 1),
                                                                       (b'#[ignore = "requires pinned LLVM'
                                                                        b' 19.1.7 and one serialized nativ'
                                                                        b'e execution slot"]',
                                                                        1),
                                                                       (b'#[cfg(all(\n    target_os = "linu'
                                                                        b'x",\n    target_pointer_width = "'
                                                                        b'64",\n    target_endian = "little'
                                                                        b'",\n    any(target_arch = "x86_64'
                                                                        b'", target_arch = "aarch64")\n))]',
                                                                        1),
                                                                       (b'#[test]', 1),
                                                                       (b'#[ignore = "requires pinned LLVM'
                                                                        b' 19.1.7 and one serialized nativ'
                                                                        b'e execution slot"]',
                                                                        1),
                                                                       (b'#[cfg(all(\n    target_os = "linu'
                                                                        b'x",\n    target_pointer_width = "'
                                                                        b'64",\n    target_endian = "little'
                                                                        b'",\n    any(target_arch = "x86_64'
                                                                        b'", target_arch = "aarch64")\n))]',
                                                                        1),
                                                                       (b'#[test]', 1),
                                                                       (b'#[ignore = "requires pinned LLVM'
                                                                        b' 19.1.7 and one serialized nativ'
                                                                        b'e execution slot"]',
                                                                        1),
                                                                       (b'#[cfg(all(\n    target_os = "linu'
                                                                        b'x",\n    target_pointer_width = "'
                                                                        b'64",\n    target_endian = "little'
                                                                        b'",\n    any(target_arch = "x86_64'
                                                                        b'", target_arch = "aarch64")\n))]',
                                                                        1),
                                                                       (b'#[test]', 1),
                                                                       (b'#[ignore = "requires pinned LLVM'
                                                                        b' 19.1.7 and one serialized nativ'
                                                                        b'e execution slot"]',
                                                                        1)),
                                                        'build_script': None,
                                                        'c_preprocessor': (),
                                                        'legacy': ([], []),
                                                        'macros': ()},
 'src/frontend/oir/owned/composition_reference_tests.rs': {'attributes': ((b'#[test]', 7),),
                                                           'build_script': None,
                                                           'c_preprocessor': (),
                                                           'legacy': ([], []),
                                                           'macros': ()},
 'src/frontend/oir/owned/consumer_fixtures.rs': {'attributes': (),
                                                 'build_script': None,
                                                 'c_preprocessor': (),
                                                 'legacy': ([], []),
                                                 'macros': ()},
 'src/frontend/oir/owned/copy_record_native_tests.rs': {'attributes': ((b'#[test]', 1),
                                                                       (b'#[derive(Copy)]', 1),
                                                                       (b'#[cfg(all(\n    target_os = "linu'
                                                                        b'x",\n    target_pointer_width = "'
                                                                        b'64",\n    target_endian = "little'
                                                                        b'",\n    any(target_arch = "x86_64'
                                                                        b'", target_arch = "aarch64")\n))]',
                                                                        1),
                                                                       (b'#[test]', 1),
                                                                       (b'#[ignore = "requires LLVM 19.1.7'
                                                                        b'; source-free Copy record execut'
                                                                        b'ion at O0 and O2"]',
                                                                        1),
                                                                       (b'#[cfg(all(\n    target_os = "linu'
                                                                        b'x",\n    target_pointer_width = "'
                                                                        b'64",\n    target_endian = "little'
                                                                        b'",\n    any(target_arch = "x86_64'
                                                                        b'", target_arch = "aarch64")\n))]',
                                                                        1),
                                                                       (b'#[test]', 1),
                                                                       (b'#[ignore = "requires LLVM 19.1.7'
                                                                        b'; late-leaf Copy corruption and '
                                                                        b'partial-write oracle controls at'
                                                                        b' O0/O2"]',
                                                                        1),
                                                                       (b'#[derive(Copy)]', 1)),
                                                        'build_script': None,
                                                        'c_preprocessor': (),
                                                        'legacy': ([], []),
                                                        'macros': ()},
 'src/frontend/oir/owned/execute.rs': {'attributes': ((b'#[path = "execute_floats.rs"]', 1),
                                                      (b'#[path = "execute_values.rs"]', 1),
                                                      (b'#[path = "execute_bitwise.rs"]', 1),
                                                      (b'#[path = "execute_integer128.rs"]', 1),
                                                      (b'#[path = "execute_integers.rs"]', 1),
                                                      (b'#[path = "execute_experimental.rs"]', 1),
                                                      (b'#[path = "execute_vector.rs"]', 1),
                                                      (b'#[path = "execute_composition.rs"]', 1),
                                                      (b'#[path = "execute_resources.rs"]', 1),
                                                      (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                                      (b'#[derive(Clone, Copy)]', 2),
                                                      (b'#[cfg(test)]', 1),
                                                      (b'#[derive(Clone, Debug, PartialEq, Eq)]', 1),
                                                      (b'#[derive(Clone, Copy)]', 2),
                                                      (b'#[cfg(test)]', 65),
                                                      (b'#[allow(clippy::too_many_arguments)]', 1),
                                                      (b'#[cfg(test)]', 12),
                                                      (b'#[path = "array_observe.rs"]', 1),
                                                      (b'#[cfg(test)]', 4),
                                                      (b'#[path = "execute_tests.rs"]', 1),
                                                      (b'#[cfg(test)]', 1),
                                                      (b'#[path = "builtin_output_reference_tests.rs"]', 1),
                                                      (b'#[cfg(test)]', 2),
                                                      (b'#[test]', 1),
                                                      (b'#[cfg(test)]', 1),
                                                      (b'#[test]', 1),
                                                      (b'#[cfg(test)]', 1),
                                                      (b'#[path = "byte_storage_codec_tests.rs"]', 1),
                                                      (b'#[cfg(test)]', 1),
                                                      (b'#[test]', 4)),
                                       'build_script': None,
                                       'c_preprocessor': (),
                                       'legacy': ([],
                                                  [b'#[path = "execute_floats.rs"]',
                                                   b'#[path = "execute_values.rs"]',
                                                   b'#[path = "execute_bitwise.rs"]',
                                                   b'#[path = "execute_integer128.rs"]',
                                                   b'#[path = "execute_integers.rs"]',
                                                   b'#[path = "execute_experimental.rs"]',
                                                   b'#[path = "execute_vector.rs"]',
                                                   b'#[path = "execute_composition.rs"]',
                                                   b'#[path = "execute_resources.rs"]',
                                                   b'#[path = "array_observe.rs"]',
                                                   b'#[path = "execute_tests.rs"]',
                                                   b'#[path = "builtin_output_reference_tests.rs"]',
                                                   b'#[path = "byte_storage_codec_tests.rs"]']),
                                       'macros': ()},
 'src/frontend/oir/owned/execute_composition.rs': {'attributes': ((b'#[cfg(test)]', 1), (b'#[test]', 2)),
                                                   'build_script': None,
                                                   'c_preprocessor': (),
                                                   'legacy': ([], []),
                                                   'macros': ()},
 'src/frontend/oir/owned/execute_resource_tests.rs': {'attributes': ((b'#[test]', 3),),
                                                      'build_script': None,
                                                      'c_preprocessor': (),
                                                      'legacy': ([], []),
                                                      'macros': ()},
 'src/frontend/oir/owned/execute_tests.rs': {'attributes': ((b'#[test]', 2),
                                                            (b'#[path = "source/runtime_resource_tests.rs"]',
                                                             1),
                                                            (b'#[test]', 17),
                                                            (b'#[path = "source/reviewer_resource_runtime.r'
                                                             b's"]',
                                                             1),
                                                            (b'#[test]', 3),
                                                            (b'#[derive(Copy)]', 1),
                                                            (b'#[test]', 1),
                                                            (b'#[derive(Copy)]', 1)),
                                             'build_script': None,
                                             'c_preprocessor': (),
                                             'legacy': ([b'include!("reviewer_reference_tests.rs")'],
                                                        [b'#[path = "source/runtime_resource_tests.rs"]',
                                                         b'#[path = "source/reviewer_resource_runtime.rs"]']),
                                             'macros': ((b'include!("reviewer_reference_tests.rs")', 1),)},
 'src/frontend/oir/owned/flow.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 8),
                                                   (b'#[repr(u8)]', 1),
                                                   (b'#[derive(Clone, Copy)]', 1)),
                                    'build_script': None,
                                    'c_preprocessor': (),
                                    'legacy': ([], []),
                                    'macros': ()},
 'src/frontend/oir/owned/mod.rs': {'attributes': ((b'#![allow(dead_code)]', 1),
                                                  (b'#![allow(clippy::result_large_err)]', 1),
                                                  (b'#[cfg(test)]', 2),
                                                  (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2),
                                                  (b'#[derive(Clone, Debug)]', 1),
                                                  (b'#[derive(Clone, Copy, Debug)]', 1),
                                                  (b'#[derive(Debug)]', 1),
                                                  (b'#[derive(Clone, Debug)]', 1),
                                                  (b'#[derive(Clone, Copy, Debug)]', 1),
                                                  (b'#[derive(Clone, Debug)]', 1),
                                                  (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                  (b'#[derive(Clone, Debug)]', 1),
                                                  (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                  (b'#[derive(Clone, Debug)]', 1),
                                                  (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                  (b'#[cfg(test)]', 1),
                                                  (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2),
                                                  (b'#[derive(Clone, Debug)]', 2),
                                                  (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                  (b'#[derive(Clone, Debug)]', 1),
                                                  (b'#[derive(Clone, Copy, Debug)]', 1),
                                                  (b'#[derive(Clone, Debug)]', 3),
                                                  (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 5),
                                                  (b'#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]',
                                                   1),
                                                  (b'#[cfg(test)]', 36)),
                                   'build_script': None,
                                   'c_preprocessor': (),
                                   'legacy': ([], []),
                                   'macros': ()},
 'src/frontend/oir/owned/native.rs': {'attributes': ((b'#[path = "native_bitwise.rs"]', 1),
                                                     (b'#[path = "native_composition.rs"]', 1),
                                                     (b'#[path = "native_experimental.rs"]', 1),
                                                     (b'#[path = "native_floats.rs"]', 1),
                                                     (b'#[path = "native_integer128.rs"]', 1),
                                                     (b'#[path = "native_integers.rs"]', 1),
                                                     (b'#[path = "native_numeric.rs"]', 1),
                                                     (b'#[path = "native_reachability.rs"]', 1),
                                                     (b'#[path = "native_resources.rs"]', 1),
                                                     (b'#[path = "native_vector.rs"]', 1),
                                                     (b'#[allow(dead_code)]', 1),
                                                     (b'#[derive(Clone, Debug, Default)]', 1),
                                                     (b'#[derive(Default)]', 1),
                                                     (b'#[cfg(test)]', 2),
                                                     (b'#[derive(Clone, Copy, Debug)]', 1),
                                                     (b'#[derive(Clone, Copy, Default, Debug)]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[derive(Clone, Copy, Debug)]', 1),
                                                     (b'#[cfg(test)]', 3),
                                                     (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[allow(clippy::too_many_arguments)]', 3),
                                                     (b'#[derive(Clone, Copy)]', 1),
                                                     (b'#[cfg(test)]', 2),
                                                     (b'#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialO'
                                                      b'rd, Ord)]',
                                                      1),
                                                     (b'#[repr(transparent)]', 1),
                                                     (b'#[allow(non_upper_case_globals)]', 1),
                                                     (b'#[derive(Clone, Copy)]', 1),
                                                     (b'#[allow(clippy::too_many_arguments)]', 2),
                                                     (b'#[cfg(test)]', 2),
                                                     (b'#[derive(Clone, Copy)]', 2),
                                                     (b'#[allow(dead_code)]', 1),
                                                     (b'#[allow(clippy::too_many_arguments)]', 1),
                                                     (b'#[cfg(test)]', 2),
                                                     (b'#[derive(Clone, Copy)]', 1),
                                                     (b'#[cfg(test)]', 3),
                                                     (b'#[path = "builtin_input_native_tests.rs"]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[path = "builtin_output_native_tests.rs"]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[path = "native_tests.rs"]', 1),
                                                     (b'#[cfg(all(test, any(target_os = "linux", target_os ='
                                                      b' "macos")))]',
                                                      1),
                                                     (b'#[path = "native_vector_tests.rs"]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[test]', 2),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[path = "native_inventory_admission_tests.rs"]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[path = "hosted_capacity_tests.rs"]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[test]', 4),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[test]', 2),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[test]', 1),
                                                     (b'#[allow(dead_code)]', 2),
                                                     (b'#[test]', 1),
                                                     (b'#[allow(dead_code)]', 4),
                                                     (b'#[test]', 1),
                                                     (b'#[cfg(target_pointer_width = "64")]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[path = "byte_storage_native_tests.rs"]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[test]', 2),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[path = "copy_record_native_tests.rs"]', 1)),
                                      'build_script': None,
                                      'c_preprocessor': (),
                                      'legacy': ([],
                                                 [b'#[path = "native_bitwise.rs"]',
                                                  b'#[path = "native_composition.rs"]',
                                                  b'#[path = "native_experimental.rs"]',
                                                  b'#[path = "native_floats.rs"]',
                                                  b'#[path = "native_integer128.rs"]',
                                                  b'#[path = "native_integers.rs"]',
                                                  b'#[path = "native_numeric.rs"]',
                                                  b'#[path = "native_reachability.rs"]',
                                                  b'#[path = "native_resources.rs"]',
                                                  b'#[path = "native_vector.rs"]',
                                                  b'#[path = "builtin_input_native_tests.rs"]',
                                                  b'#[path = "builtin_output_native_tests.rs"]',
                                                  b'#[path = "native_tests.rs"]',
                                                  b'#[path = "native_vector_tests.rs"]',
                                                  b'#[path = "native_inventory_admission_tests.rs"]',
                                                  b'#[path = "hosted_capacity_tests.rs"]',
                                                  b'#[path = "byte_storage_native_tests.rs"]',
                                                  b'#[path = "copy_record_native_tests.rs"]']),
                                      'macros': ((b'cfg!(all(target_os="macos",target_arch="aarch64"))', 1),
                                                 (b'cfg!(all(target_os="macos",target_arch="x86_64"))', 1),
                                                 (b'cfg!(all(target_os="linux",target_arch="aarch64"))', 1))},
 'src/frontend/oir/owned/native_tests.rs': {'attributes': ((b'#[cfg(all(\n    target_os = "linux",\n    targ'
                                                            b'et_pointer_width = "64",\n    target_endian ='
                                                            b' "little",\n    any(target_arch = "x86_64", t'
                                                            b'arget_arch = "aarch64")\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[ignore = "requires pinned LLVM 19.1.7; exp'
                                                            b'licitly run the owned native gate"]',
                                                            1),
                                                           (b'#[path = "source/native_resource_tests.rs"]',
                                                            1),
                                                           (b'#[test]', 4),
                                                           (b'#[cfg(all(\n    target_os = "linux",\n    targ'
                                                            b'et_pointer_width = "64",\n    target_endian ='
                                                            b' "little",\n    any(target_arch = "x86_64", t'
                                                            b'arget_arch = "aarch64")\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[ignore = "requires pinned LLVM 19.1.7; exp'
                                                            b'licitly run the owned native gate"]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[cfg(all(\n    target_os = "linux",\n    targ'
                                                            b'et_pointer_width = "64",\n    target_endian ='
                                                            b' "little",\n    any(target_arch = "x86_64", t'
                                                            b'arget_arch = "aarch64")\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[ignore = "requires pinned LLVM 19.1.7; exp'
                                                            b'licitly run the owned native gate"]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[cfg(all(\n    target_os = "linux",\n    targ'
                                                            b'et_pointer_width = "64",\n    target_endian ='
                                                            b' "little",\n    any(target_arch = "x86_64", t'
                                                            b'arget_arch = "aarch64")\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[ignore = "requires pinned LLVM 19.1.7; exp'
                                                            b'licitly run the owned native gate"]',
                                                            1),
                                                           (b'#[test]', 3),
                                                           (b'#[cfg(all(\n    target_os = "linux",\n    targ'
                                                            b'et_pointer_width = "64",\n    target_endian ='
                                                            b' "little",\n    any(target_arch = "x86_64", t'
                                                            b'arget_arch = "aarch64")\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[ignore = "requires pinned LLVM 19.1.7; exp'
                                                            b'licitly run the owned native gate"]',
                                                            1),
                                                           (b'#[cfg(all(\n    target_os = "linux",\n    targ'
                                                            b'et_pointer_width = "64",\n    target_endian ='
                                                            b' "little",\n    any(target_arch = "x86_64", t'
                                                            b'arget_arch = "aarch64")\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[ignore = "requires pinned LLVM 19.1.7; exp'
                                                            b'licitly run the owned native gate"]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[cfg(all(\n    target_os = "linux",\n    targ'
                                                            b'et_pointer_width = "64",\n    target_endian ='
                                                            b' "little",\n    any(target_arch = "x86_64", t'
                                                            b'arget_arch = "aarch64")\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[ignore = "requires pinned LLVM 19.1.7; exp'
                                                            b'licitly run the owned native gate"]',
                                                            1),
                                                           (b'#[test]', 4),
                                                           (b'#[cfg(all(\n    target_os = "linux",\n    targ'
                                                            b'et_pointer_width = "64",\n    target_endian ='
                                                            b' "little",\n    any(target_arch = "x86_64", t'
                                                            b'arget_arch = "aarch64")\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[ignore = "requires pinned LLVM 19.1.7; exp'
                                                            b'licitly run the owned native gate"]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[path = "native_heldout_review.rs"]', 1),
                                                           (b'#[path = "source/reviewer_resource_native.rs"]',
                                                            1),
                                                           (b'#[path = "array_native_resource_tests.rs"]', 1),
                                                           (b'#[path = "array_native_tests.rs"]', 1),
                                                           (b'#[path = "slice_native_tests.rs"]', 1),
                                                           (b'#[path = "composition_native_tests.rs"]', 1),
                                                           (b'#[path = "projected_slice_native_tests.rs"]',
                                                            1),
                                                           (b'#[path = "enum_native_tests.rs"]', 1)),
                                            'build_script': None,
                                            'c_preprocessor': (),
                                            'legacy': ([],
                                                       [b'#[path = "source/native_resource_tests.rs"]',
                                                        b'#[path = "native_heldout_review.rs"]',
                                                        b'#[path = "source/reviewer_resource_native.rs"]',
                                                        b'#[path = "array_native_resource_tests.rs"]',
                                                        b'#[path = "array_native_tests.rs"]',
                                                        b'#[path = "slice_native_tests.rs"]',
                                                        b'#[path = "composition_native_tests.rs"]',
                                                        b'#[path = "projected_slice_native_tests.rs"]',
                                                        b'#[path = "enum_native_tests.rs"]']),
                                            'macros': ()},
 'src/frontend/oir/owned/oracle_tests.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                            1),
                                                           (b'#[derive(Clone, Debug)]', 1),
                                                           (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                            2),
                                                           (b'#[derive(Clone, Copy, Debug)]', 1),
                                                           (b'#[derive(Default, Clone, Copy)]', 1),
                                                           (b'#[derive(Clone, Copy, Debug)]', 1),
                                                           (b'#[test]', 5),
                                                           (b'#[derive(Clone, Copy, Debug)]', 2),
                                                           (b'#[test]', 2),
                                                           (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                            2),
                                                           (b'#[derive(Clone, Debug)]', 3),
                                                           (b'#[derive(Clone, Copy, Debug)]', 1),
                                                           (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                            2),
                                                           (b'#[derive(Debug)]', 3),
                                                           (b'#[test]', 1),
                                                           (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                            1),
                                                           (b'#[test]', 6),
                                                           (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                            1),
                                                           (b'#[test]', 2)),
                                            'build_script': None,
                                            'c_preprocessor': (),
                                            'legacy': ([], []),
                                            'macros': ()},
 'src/frontend/oir/owned/origin_tests.rs': {'attributes': ((b'#[test]', 9),
                                                           (b'#[ignore = "maximum-block Some-origin work/p'
                                                            b'ayload/RSS qualification"]',
                                                            1),
                                                           (b'#[test]', 3)),
                                            'build_script': None,
                                            'c_preprocessor': (),
                                            'legacy': ([], []),
                                            'macros': ()},
 'src/frontend/oir/owned/plan.rs': {'attributes': ((b'#[path = "native_storage.rs"]', 1),
                                                   (b'#[path = "native_vector_layout.rs"]', 1),
                                                   (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                   (b'#[cfg(test)]', 1),
                                                   (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]',
                                                    1),
                                                   (b'#[derive(Debug)]', 3),
                                                   (b'#[cfg(test)]', 5),
                                                   (b'#[test]', 1),
                                                   (b'#[cfg(target_pointer_width = "64")]', 1),
                                                   (b'#[test]', 1)),
                                    'build_script': None,
                                    'c_preprocessor': (),
                                    'legacy': ([],
                                               [b'#[path = "native_storage.rs"]',
                                                b'#[path = "native_vector_layout.rs"]']),
                                    'macros': ()},
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs': {'attributes': ((b'#[test]', 14),),
                                                              'build_script': None,
                                                              'c_preprocessor': (),
                                                              'legacy': ([], []),
                                                              'macros': ()},
 'src/frontend/oir/owned/reviewer_heldout.rs': {'attributes': ((b'#[test]', 13),),
                                                'build_script': None,
                                                'c_preprocessor': (),
                                                'legacy': ([], []),
                                                'macros': ()},
 'src/frontend/oir/owned/reviewer_origins.rs': {'attributes': ((b'#[test]', 8),),
                                                'build_script': None,
                                                'c_preprocessor': (),
                                                'legacy': ([], []),
                                                'macros': ()},
 'src/frontend/oir/owned/reviewer_reference_tests.rs': {'attributes': ((b'#[test]', 10),
                                                                       (b'#[derive(Clone, Copy, Debug)]', 1),
                                                                       (b'#[derive(Clone, Debug)]', 1),
                                                                       (b'#[test]', 9)),
                                                        'build_script': None,
                                                        'c_preprocessor': (),
                                                        'legacy': ([], []),
                                                        'macros': ()},
 'src/frontend/oir/owned/shape.rs': {'attributes': ((b'#[derive(Debug, Clone, Copy, PartialEq, Eq)]', 1),
                                                    (b'#[derive(Clone, Default)]', 2),
                                                    (b'#[allow(clippy::too_many_arguments)]', 1),
                                                    (b'#[derive(Clone, Copy)]', 1)),
                                     'build_script': None,
                                     'c_preprocessor': (),
                                     'legacy': ([], []),
                                     'macros': ()},
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': {'attributes': ((b'#[cfg(test)]', 1),
                                                                         (b'#[test]', 1)),
                                                          'build_script': None,
                                                          'c_preprocessor': (),
                                                          'legacy': ([], []),
                                                          'macros': ()},
 'src/frontend/oir/owned/source/association.rs': {'attributes': ((b'#[cfg(test)]', 3),
                                                                 (b'#[derive(Clone, Copy)]', 1),
                                                                 (b'#[cfg(test)]', 2),
                                                                 (b'#[allow(dead_code)]', 2),
                                                                 (b'#[cfg(test)]', 1),
                                                                 (b'#[path = "output_lower_tests.rs"]', 1),
                                                                 (b'#[cfg(test)]', 1),
                                                                 (b'#[cfg(not(test))]', 1),
                                                                 (b'#[cfg(test)]', 2),
                                                                 (b'#[test]', 3),
                                                                 (b'#[cfg(test)]', 1),
                                                                 (b'#[allow(dead_code)]', 1),
                                                                 (b'#[test]', 1),
                                                                 (b'#[cfg(target_pointer_width = "64")]', 1),
                                                                 (b'#[cfg(test)]', 1),
                                                                 (b'#[test]', 2),
                                                                 (b'#[cfg(test)]', 1),
                                                                 (b'#[test]', 3),
                                                                 (b'#[cfg(test)]', 2),
                                                                 (b'#[path = "byte_storage_association_tests'
                                                                  b'.rs"]',
                                                                  1)),
                                                  'build_script': None,
                                                  'c_preprocessor': (),
                                                  'legacy': ([],
                                                             [b'#[path = "output_lower_tests.rs"]',
                                                              b'#[path = "byte_storage_association_tests.rs"'
                                                              b']']),
                                                  'macros': ()},
 'src/frontend/oir/owned/source/budget_tests.rs': {'attributes': ((b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[test]', 1),
                                                                  (b'#[cfg(target_pointer_width = "64")]', 1),
                                                                  (b'#[test]', 7),
                                                                  (b'#[ignore = "production-token-envelope so'
                                                                   b'urce payload and separate RSS qualificat'
                                                                   b'ion"]',
                                                                   1),
                                                                  (b'#[test]', 1)),
                                                   'build_script': None,
                                                   'c_preprocessor': (),
                                                   'legacy': ([], []),
                                                   'macros': ()},
 'src/frontend/oir/owned/source/byte_storage_raw_tests.rs': {'attributes': ((b'#[test]', 5),),
                                                             'build_script': None,
                                                             'c_preprocessor': (),
                                                             'legacy': ([], []),
                                                             'macros': ()},
 'src/frontend/oir/owned/source/byte_storage_resources.rs': {'attributes': ((b'#[allow(dead_code)]', 1),
                                                                            (b'#[cfg(test)]', 1),
                                                                            (b'#[test]', 2),
                                                                            (b'#[cfg(target_pointer_width ='
                                                                             b' "64")]',
                                                                             1),
                                                                            (b'#[test]', 2)),
                                                             'build_script': None,
                                                             'c_preprocessor': (),
                                                             'legacy': ([], []),
                                                             'macros': ()},
 'src/frontend/oir/owned/source/candidate_adapter.rs': {'attributes': ((b'#[derive(Default)]', 1),
                                                                       (b'#[test]', 1),
                                                                       (b'#[ignore = "source receipt colle'
                                                                        b'ctor; requires explicit source a'
                                                                        b'nd fresh output directories"]',
                                                                        1),
                                                                       (b'#[test]', 4),
                                                                       (b'#[cfg(test)]', 1),
                                                                       (b'#[path = "candidate_mutations.rs"]',
                                                                        1),
                                                                       (b'#[cfg(test)]', 1),
                                                                       (b'#[path = "candidate_native.rs"]',
                                                                        1)),
                                                        'build_script': None,
                                                        'c_preprocessor': (),
                                                        'legacy': ([],
                                                                   [b'#[path = "candidate_mutations.rs"]',
                                                                    b'#[path = "candidate_native.rs"]']),
                                                        'macros': ()},
 'src/frontend/oir/owned/source/candidate_native.rs': {'attributes': ((b'#[cfg(all(\n    target_os = "linu'
                                                                       b'x",\n    target_pointer_width = "'
                                                                       b'64",\n    target_endian = "little'
                                                                       b'",\n    any(target_arch = "x86_64'
                                                                       b'", target_arch = "aarch64")\n))]',
                                                                       1),
                                                                      (b'#[test]', 1),
                                                                      (b'#[ignore = "bounded native source co'
                                                                       b'llector; requires frozen original so'
                                                                       b'urce, budget controls, pinned LLVM a'
                                                                       b'nd a fresh output path"]',
                                                                       1),
                                                                      (b'#[test]', 1)),
                                                       'build_script': None,
                                                       'c_preprocessor': (),
                                                       'legacy': ([],
                                                                  [b'#[ignore = "bounded native source collec'
                                                                   b'tor; requires frozen original source, bu'
                                                                   b'dget controls, pinned LLVM and a fresh o'
                                                                   b'utput path"]']),
                                                       'macros': ()},
 'src/frontend/oir/owned/source/copy_record_tests.rs': {'attributes': ((b'#[derive(Copy)]', 2),
                                                                       (b'#[test]', 1),
                                                                       (b'#[derive(Copy)]', 3),
                                                                       (b'#[test]', 1),
                                                                       (b'#[derive(Copy)]', 11),
                                                                       (b'#[test]', 1),
                                                                       (b'#[derive(Copy)]', 6),
                                                                       (b'#[test]', 1),
                                                                       (b'#[derive(Copy)]', 1),
                                                                       (b'#[test]', 1),
                                                                       (b'#[derive(Copy)]', 2),
                                                                       (b'#[test]', 1)),
                                                        'build_script': None,
                                                        'c_preprocessor': (),
                                                        'legacy': ([], []),
                                                        'macros': ()},
 'src/frontend/oir/owned/source/diagnostic.rs': {'attributes': (),
                                                 'build_script': None,
                                                 'c_preprocessor': (),
                                                 'legacy': ([], []),
                                                 'macros': ()},
 'src/frontend/oir/owned/source/hir.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 4),
                                                         (b'#[derive(Debug)]', 2),
                                                         (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                         (b'#[derive(Debug)]', 3),
                                                         (b'#[allow(dead_code)]', 1),
                                                         (b'#[derive(Debug)]', 3),
                                                         (b'#[allow(dead_code)]', 1),
                                                         (b'#[derive(Debug)]', 2),
                                                         (b'#[allow(dead_code)]', 1),
                                                         (b'#[derive(Debug)]', 4),
                                                         (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                         (b'#[derive(Clone, Debug, PartialEq, Eq)]', 1)),
                                          'build_script': None,
                                          'c_preprocessor': (),
                                          'legacy': ([], []),
                                          'macros': ()},
 'src/frontend/oir/owned/source/lower.rs': {'attributes': ((b'#[derive(Clone, Copy)]', 8),
                                                           (b'#[cfg(test)]', 16),
                                                           (b'#[cfg(not(test))]', 1),
                                                           (b'#[cfg(test)]', 5),
                                                           (b'#[test]', 1),
                                                           (b'#[allow(dead_code)]', 2),
                                                           (b'#[test]', 1)),
                                            'build_script': None,
                                            'c_preprocessor': (),
                                            'legacy': ([], []),
                                            'macros': ()},
 'src/frontend/oir/owned/source/mod.rs': {'attributes': ((b'#[cfg(test)]', 10),
                                                         (b'#[cfg(all(test, target_os = "linux", target_arch'
                                                          b' = "x86_64"))]',
                                                          1),
                                                         (b'#[cfg(test)]', 26)),
                                          'build_script': None,
                                          'c_preprocessor': (),
                                          'legacy': ([], []),
                                          'macros': ()},
 'src/frontend/oir/owned/source/record_enum_layouts.rs': {'attributes': ((b'#[cfg(test)]', 1),
                                                                         (b'#[test]', 3)),
                                                          'build_script': None,
                                                          'c_preprocessor': (),
                                                          'legacy': ([], []),
                                                          'macros': ()},
 'src/frontend/oir/owned/source/resolve.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                              1),
                                                             (b'#[cfg(test)]', 7),
                                                             (b'#[cfg(not(test))]', 1),
                                                             (b'#[cfg(test)]', 2),
                                                             (b'#[derive(Debug)]', 1),
                                                             (b'#[allow(clippy::large_enum_variant)]', 1),
                                                             (b'#[derive(Debug)]', 2),
                                                             (b'#[test]', 1),
                                                             (b'#[cfg(test)]', 2),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[allow(clippy::blocks_in_conditions)]', 1),
                                                             (b'#[cfg(test)]', 4),
                                                             (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                              1),
                                                             (b'#[derive(Debug)]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[allow(dead_code)]', 11),
                                                             (b'#[cfg(test)]', 2),
                                                             (b'#[test]', 2),
                                                             (b'#[cfg(test)]', 4),
                                                             (b'#[test]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[test]', 2),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "resolver_paid_tests.rs"]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[test]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "resolver_enum_tests.rs"]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "enum_storage_failure_tests.rs"]',
                                                              1),
                                                             (b'#[allow(dead_code)]', 2),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[allow(dead_code)]', 4),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "builtin_signature_tests.rs"]', 1),
                                                             (b'#[cfg(test)]', 2),
                                                             (b'#[path = "output_typing_tests.rs"]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "byte_storage_type_tests.rs"]', 1)),
                                              'build_script': None,
                                              'c_preprocessor': (),
                                              'legacy': ([],
                                                         [b'#[path = "resolver_paid_tests.rs"]',
                                                          b'#[path = "resolver_enum_tests.rs"]',
                                                          b'#[path = "enum_storage_failure_tests.rs"]',
                                                          b'#[path = "builtin_signature_tests.rs"]',
                                                          b'#[path = "output_typing_tests.rs"]',
                                                          b'#[path = "byte_storage_type_tests.rs"]']),
                                              'macros': ()},
 'src/frontend/oir/owned/source/resolver_inventory_tests.rs': {'attributes': ((b'#[test]', 8),),
                                                               'build_script': None,
                                                               'c_preprocessor': (),
                                                               'legacy': ([], []),
                                                               'macros': ()},
 'src/frontend/oir/owned/source/resolver_paid_tests.rs': {'attributes': ((b'#[test]', 18),
                                                                         (b'#[cfg(target_os = "linux")]', 1),
                                                                         (b'#[test]', 4),
                                                                         (b'#[cfg(target_os = "linux")]', 1),
                                                                         (b'#[test]', 7),
                                                                         (b'#[cfg(target_pointer_width = "64'
                                                                          b'")]',
                                                                          1),
                                                                         (b'#[test]', 3),
                                                                         (b'#[cfg(target_pointer_width = "64'
                                                                          b'")]',
                                                                          1),
                                                                         (b'#[test]', 3)),
                                                          'build_script': None,
                                                          'c_preprocessor': (),
                                                          'legacy': ([], []),
                                                          'macros': ()},
 'src/frontend/oir/owned/source/resource_native_tests.rs': {'attributes': ((b'#[test]', 1),
                                                                           (b'#[ignore = "requires LLVM 19'
                                                                            b'.1.7; executes resource repl'
                                                                            b'acement faults at O0/O2"]',
                                                                            1),
                                                                           (b'#[test]', 1),
                                                                           (b'#[ignore = "requires LLVM 19'
                                                                            b'.1.7; executes fixed byte ra'
                                                                            b'nges at O0/O2"]',
                                                                            1)),
                                                            'build_script': None,
                                                            'c_preprocessor': (),
                                                            'legacy': ([], []),
                                                            'macros': ()},
 'src/frontend/oir/owned/source/reviewer_source.rs': {'attributes': ((b'#[allow(dead_code)]', 1),
                                                                     (b'#[repr(C)]', 6),
                                                                     (b'#[test]', 1),
                                                                     (b'#[cfg(target_pointer_width = "64")]',
                                                                      1),
                                                                     (b'#[repr(C)]', 11),
                                                                     (b'#[test]', 1),
                                                                     (b'#[cfg(target_pointer_width = "64")]',
                                                                      1),
                                                                     (b'#[derive(Clone, Copy, Debug, Default'
                                                                      b')]',
                                                                      1),
                                                                     (b'#[test]', 12),
                                                                     (b'#[ignore = "requires qualified LLVM '
                                                                      b'19.1.7 and separate native evidence '
                                                                      b'directory"]',
                                                                      1),
                                                                     (b'#[cfg(test)]', 1)),
                                                      'build_script': None,
                                                      'c_preprocessor': (),
                                                      'legacy': ([], []),
                                                      'macros': ()},
 'src/frontend/oir/owned/source/type_storage_tests.rs': {'attributes': ((b'#[test]', 32),
                                                                        (b'#[cfg(all(target_arch = "x86_64"'
                                                                         b', target_pointer_width = "64"))]',
                                                                         1),
                                                                        (b'#[test]', 25)),
                                                         'build_script': None,
                                                         'c_preprocessor': (),
                                                         'legacy': ([], []),
                                                         'macros': ()},
 'src/frontend/oir/owned/source/typeck.rs': {'attributes': ((b'#[derive(Debug)]', 3),
                                                            (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                             1),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[allow(clippy::large_enum_variant)]', 1),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[allow(dead_code)]', 19),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[derive(Debug)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[allow(dead_code)]', 2),
                                                            (b'#[cfg(test)]', 2),
                                                            (b'#[allow(clippy::blocks_in_conditions)]', 1),
                                                            (b'#[allow(dead_code)]', 3),
                                                            (b'#[cfg(test)]', 2),
                                                            (b'#[allow(dead_code)]', 4),
                                                            (b'#[allow(clippy::type_complexity)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[cfg(test)]', 4),
                                                            (b'#[test]', 1),
                                                            (b'#[cfg(test)]', 2),
                                                            (b'#[allow(clippy::too_many_arguments)]', 2),
                                                            (b'#[allow(clippy::unused_enumerate_index)]', 1),
                                                            (b'#[cfg(test)]', 3),
                                                            (b'#[cfg(not(test))]', 1),
                                                            (b'#[allow(clippy::too_many_arguments)]', 1),
                                                            (b'#[cfg(test)]', 7),
                                                            (b'#[test]', 2),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[test]', 2),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[test]', 4),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[test]', 1),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[test]', 5),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[path = "enum_type_tests.rs"]', 1),
                                                            (b'#[allow(dead_code)]', 2),
                                                            (b'#[test]', 1),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[test]', 1)),
                                             'build_script': None,
                                             'c_preprocessor': (),
                                             'legacy': ([], [b'#[path = "enum_type_tests.rs"]']),
                                             'macros': ()},
 'src/frontend/oir/owned/source/u8_tests.rs': {'attributes': ((b'#[test]', 11),
                                                              (b'#[cfg(all(\n    target_os = "linux",\n    '
                                                               b'target_pointer_width = "64",\n    target_'
                                                               b'endian = "little",\n    any(target_arch ='
                                                               b' "x86_64", target_arch = "aarch64")\n))]',
                                                               1),
                                                              (b'#[test]', 1),
                                                              (b'#[ignore = "requires pinned LLVM 19.1.7; exp'
                                                               b'licitly run the owned native gate"]',
                                                               1),
                                                              (b'#[test]', 1),
                                                              (b'#[cfg(target_pointer_width = "64")]', 1),
                                                              (b'#[test]', 2)),
                                               'build_script': None,
                                               'c_preprocessor': (),
                                               'legacy': ([], []),
                                               'macros': ()},
 'src/frontend/oir/owned/tests.rs': {'attributes': ((b'#[test]', 23),
                                                    (b'#[ignore = "maximum-block ownership scratch/RSS qual'
                                                     b'ification"]',
                                                     1),
                                                    (b'#[test]', 3),
                                                    (b'#[path = "reviewer_heldout.rs"]', 1),
                                                    (b'#[test]', 3),
                                                    (b'#[path = "array_tests.rs"]', 1),
                                                    (b'#[test]', 2)),
                                     'build_script': None,
                                     'c_preprocessor': (),
                                     'legacy': ([],
                                                [b'#[path = "reviewer_heldout.rs"]',
                                                 b'#[path = "array_tests.rs"]']),
                                     'macros': ()},
 'src/frontend/oir/owned/vector_raw_tests.rs': {'attributes': ((b'#[test]', 9),),
                                                'build_script': None,
                                                'c_preprocessor': (),
                                                'legacy': ([], []),
                                                'macros': ()},
 'src/frontend/oir/owned/verified.rs': {'attributes': ((b'#[derive(Debug)]', 2),
                                                       (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                                       (b'#[cfg(test)]', 1),
                                                       (b'#[test]', 3),
                                                       (b'#[cfg(test)]', 5),
                                                       (b'#[allow(dead_code)]', 1),
                                                       (b'#[test]', 2)),
                                        'build_script': None,
                                        'c_preprocessor': (),
                                        'legacy': ([], []),
                                        'macros': ()},
 'src/frontend/oir/owned_types.rs': {'attributes': ((b'#![allow(dead_code)]', 1),
                                                    (b'#[allow(unused_imports)]', 1),
                                                    (b'#[cfg(test)]', 1),
                                                    (b'#[allow(unused_imports)]', 2),
                                                    (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 5),
                                                    (b'#[repr(C, align(8))]', 1),
                                                    (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2),
                                                    (b'#[repr(C, align(8))]', 1),
                                                    (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 8),
                                                    (b'#[derive(Debug)]', 1),
                                                    (b'#[derive(Clone, Copy, Debug)]', 1),
                                                    (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                    (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]',
                                                     1),
                                                    (b'#[derive(Debug)]', 2),
                                                    (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]',
                                                     1),
                                                    (b'#[derive(Clone, Copy)]', 1),
                                                    (b'#[derive(Debug)]', 2),
                                                    (b'#[derive(Clone, Copy, Debug)]', 1),
                                                    (b'#[derive(Clone, Copy, Default)]', 1),
                                                    (b'#[derive(Default)]', 1),
                                                    (b'#[cfg(test)]', 1),
                                                    (b'#[test]', 2),
                                                    (b'#[allow(dead_code)]', 2),
                                                    (b'#[test]', 29),
                                                    (b'#[cfg(test)]', 6)),
                                     'build_script': None,
                                     'c_preprocessor': (),
                                     'legacy': ([], []),
                                     'macros': ()},
 'src/frontend/oir/owned_types/array_tests.rs': {'attributes': ((b'#[test]', 8),
                                                                (b'#[cfg(target_pointer_width = "64")]', 1),
                                                                (b'#[allow(dead_code)]', 1),
                                                                (b'#[repr(C)]', 1),
                                                                (b'#[allow(dead_code)]', 1),
                                                                (b'#[repr(C)]', 1),
                                                                (b'#[test]', 2),
                                                                (b'#[cfg(target_pointer_width = "64")]', 1),
                                                                (b'#[test]', 1)),
                                                 'build_script': None,
                                                 'c_preprocessor': (),
                                                 'legacy': ([], []),
                                                 'macros': ()},
 'src/frontend/oir/owned_types/composition.rs': {'attributes': ((b'#[cfg(test)]', 1), (b'#[test]', 6)),
                                                 'build_script': None,
                                                 'c_preprocessor': (),
                                                 'legacy': ([], []),
                                                 'macros': ()},
 'src/frontend/oir/owned_types/copy_tests.rs': {'attributes': ((b'#[test]', 2),),
                                                'build_script': None,
                                                'c_preprocessor': (),
                                                'legacy': ([], []),
                                                'macros': ()},
 'src/frontend/parser.rs': {'attributes': ((b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                           (b'#[allow(clippy::enum_variant_names)]', 1),
                                           (b'#[cfg(test)]', 2),
                                           (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                           (b'#[cfg(test)]', 2),
                                           (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                           (b'#[cfg(test)]', 2),
                                           (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                           (b'#[cfg(test)]', 3),
                                           (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                           (b'#[cfg(test)]', 3),
                                           (b'#[allow(dead_code)]', 1),
                                           (b'#[cfg(test)]', 4),
                                           (b'#[allow(clippy::too_many_arguments)]', 1),
                                           (b'#[derive(Copy)]', 1),
                                           (b'#[path = "parser/arrays.rs"]', 1),
                                           (b'#[path = "parser/characters.rs"]', 1),
                                           (b'#[path = "parser/conversions.rs"]', 1),
                                           (b'#[path = "parser/enums.rs"]', 1),
                                           (b'#[cfg(test)]', 1),
                                           (b'#[path = "parser/u8_syntax_tests.rs"]', 1),
                                           (b'#[cfg(test)]', 1),
                                           (b'#[path = "parser/builtin_tests.rs"]', 1),
                                           (b'#[cfg(test)]', 1),
                                           (b'#[path = "parser/array_syntax_tests.rs"]', 1),
                                           (b'#[cfg(test)]', 1),
                                           (b'#[path = "parser/enum_syntax_tests.rs"]', 1),
                                           (b'#[cfg(test)]', 4),
                                           (b'#[path = "parser/project_tests.rs"]', 1),
                                           (b'#[cfg(test)]', 1),
                                           (b'#[path = "parser/activation_tests.rs"]', 1),
                                           (b'#[test]', 1),
                                           (b'#[cfg(test)]', 1)),
                            'build_script': None,
                            'c_preprocessor': (),
                            'legacy': ([],
                                       [b'#[path = "parser/arrays.rs"]',
                                        b'#[path = "parser/characters.rs"]',
                                        b'#[path = "parser/conversions.rs"]',
                                        b'#[path = "parser/enums.rs"]',
                                        b'#[path = "parser/u8_syntax_tests.rs"]',
                                        b'#[path = "parser/builtin_tests.rs"]',
                                        b'#[path = "parser/array_syntax_tests.rs"]',
                                        b'#[path = "parser/enum_syntax_tests.rs"]',
                                        b'#[path = "parser/project_tests.rs"]',
                                        b'#[path = "parser/activation_tests.rs"]']),
                            'macros': ()},
 'src/frontend/parser/copy_record_tests.rs': {'attributes': ((b'#[test]', 1),
                                                             (b'#[derive(Copy)]', 1),
                                                             (b'#[derive(Clone)]', 1),
                                                             (b'#[derive(Copy,Clone)]', 1),
                                                             (b'#[derive(Copy)]', 4),
                                                             (b'#[Copy]', 1),
                                                             (b'#[derive(Copy)]', 1),
                                                             (b'#[test]', 1),
                                                             (b'#[derive(Copy)]', 1),
                                                             (b'#[test]', 1),
                                                             (b'#[derive(Copy)]', 1)),
                                              'build_script': None,
                                              'c_preprocessor': (),
                                              'legacy': ([], []),
                                              'macros': ()},
 'src/frontend/parser/enum_syntax_tests.rs': {'attributes': ((b'#[test]', 15),),
                                              'build_script': None,
                                              'c_preprocessor': (),
                                              'legacy': ([], []),
                                              'macros': ()},
 'src/frontend/parser/project_tests.rs': {'attributes': ((b'#[test]', 17),),
                                          'build_script': None,
                                          'c_preprocessor': (),
                                          'legacy': ([], []),
                                          'macros': ()},
 'tests/typed_frontend.rs': {'attributes': ((b'#[test]', 20),),
                             'build_script': None,
                             'c_preprocessor': (),
                             'legacy': ([], []),
                             'macros': ((b'env!("CARGO_BIN_EXE_oxid")', 1),)}}
PARSER_RESOURCE_ADAPTER = "scripts/checked_copy_parser_resource_adapter.py"
PARSER_RESOURCE_ADAPTER_PIN = (2238, 'ec22e6abffcd039b61376fea20dc956cf2e57182d22848a01a43ac452ae8c77c')

def require(condition, message):
    if not condition:
        raise ValueError(message)


def load_predecessor(repo):
    # Use the already frozen regular-file admission before executing its bytes.
    path = repo / PREDECESSOR_POLICY
    require(not path.is_symlink() and all(not p.is_symlink() for p in path.parents),
            "symlink pinned typed-vector source policy")
    before = path.stat()
    require(path.is_file() and before.st_nlink == 1 and not before.st_mode & 0o111
            and before.st_size == PREDECESSOR_POLICY_BYTES,
            "unsafe pinned typed-vector source policy")
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, f) for f in fields)
            == tuple(getattr(path.stat(), f) for f in fields)
            and (len(body), hashlib.sha256(body).hexdigest())
            == (PREDECESSOR_POLICY_BYTES, PREDECESSOR_POLICY_SHA),
            "changed pinned typed-vector source policy")
    module = types.ModuleType("_checked_copy_compatibility_parent")
    module.__file__ = str(path)
    exec(compile(body, module.__file__, "exec"), module.__dict__)
    return module


library = load_predecessor(Path(__file__).resolve().parents[1])
for _name in ("BASELINE", "BASELINE_SOURCE_SHA", "RETAINED_RUNNER_SHA", "BINDING", "RETAINED",
              "sha", "encoded", "identity", "load", "git", "checkout_identity", "extract_baseline",
              "regular", "verify_current_observer", "verify_materialized",
              "authenticate_observer_adapter", "authenticate_index_resource_adapter",
              "OBSERVER_ADAPTER", "INDEX_RESOURCE_ADAPTER", "UNCHANGED_INPUTS",
              "run_byte_buffer_observer_controls", "run_file_observer_controls",
              "run_i64_observer_controls", "run_integer_width_observer_controls",
              "run_integer128_observer_controls", "run_float_observer_controls",
              "run_character_observer_controls", "run_vector_observer_controls"):
    globals()[_name] = getattr(library, _name)
PREDECESSOR_ADDED = library.PARENT_ADDED
PARENT_ADDED = PREDECESSOR_ADDED | CHECKED_COPY_ADDED
PREDECESSOR_POLICIES = {**library.PREDECESSOR_POLICIES, PREDECESSOR_POLICY: PREDECESSOR_POLICY_SHA}


def source_directives(name, body):
    found = library.source_directives(name, body)
    for field in ("attributes", "macros", "c_preprocessor"):
        found[field] = tuple((value, sum(1 for _ in group))
                             for value, group in itertools.groupby(found[field]))
    return found


def authenticate_policies(repo, parent):
    for name, expected in PREDECESSOR_POLICIES.items():
        before = regular(parent, name)
        require(sha(before) == expected, "wrong pinned predecessor source policy: " + name)
        require(regular(repo, name) == before, "predecessor source policy changed: " + name)


def authenticate_parser_resource_adapter(repo):
    body = regular(repo, PARSER_RESOURCE_ADAPTER)
    require((len(body), sha(body)) == PARSER_RESOURCE_ADAPTER_PIN,
            "changed pinned checked-Copy parser resource adapter")
    module = types.ModuleType("_checked_copy_parser_resource_successor")
    module.__file__ = str(repo / PARSER_RESOURCE_ADAPTER)
    exec(compile(body, module.__file__, "exec"), module.__dict__)
    return module, body


def prepare_current_observer(original, output, captured, adapter, index_adapter, parser_adapter):
    # Reuse the actual predecessor transformations, including its full index
    # resource model. Only the unparsed parser fixture receives one new field.
    prior = library.library.adapt_parser_resource(captured["resource"])
    vector_resource = library.adapt_parser_resource(prior)
    resource = parser_adapter.adapt(vector_resource)
    current = copy.deepcopy(captured)
    current["u8_index_resource"] = index_adapter.adapt(captured["u8_index_resource"])
    selected, seam = library.library.prepare_with_parser_resource(
        original, output, current, adapter, resource)
    package = Path(seam["resource_package_root"])
    require(regular(package, index_adapter.RESOURCE) == current["u8_index_resource"],
            "prepared index resource differs from pinned typed-vector successor")
    require(regular(package, parser_adapter.RESOURCE) == resource,
            "prepared parser resource differs from pinned checked-Copy successor")
    seam["typed_vector_index_resource_adapter"] = index_adapter.descriptor()
    seam["typed_vector_parser_resource_adapter"] = {
        "before": identity(parser_adapter.RESOURCE, prior),
        "after": identity(parser_adapter.RESOURCE, vector_resource),
        "scope": "The unchanged predecessor initializes the unparsed named-slice fact to false.",
    }
    seam["checked_copy_parser_resource_adapter"] = parser_adapter.descriptor()
    return selected, seam


def capture_parent(repo, parent, names):
    objects = {}
    for row in git(repo, "ls-tree", "-rzl", "--full-tree", PREDECESSOR).split(b"\0"):
        if not row:
            continue
        metadata, name = row.split(b"\t", 1)
        mode, kind, object_id, length = metadata.split()
        require(kind == b"blob" and name not in objects, "invalid parent blob roster")
        objects[name] = (mode, object_id, length)
    result = {}
    for name in sorted(names):
        body = regular(parent, name)
        mode, object_id, length = objects[name.encode()]
        blob = b"blob " + str(len(body)).encode() + b"\0" + body
        require(mode == b"100644" and int(length) == len(body)
                and hashlib.sha1(blob, usedforsecurity=False).hexdigest().encode() == object_id,
                "parent compiler input differs from pinned Git blob: " + name)
        result[name] = body
    return result


def capture_sources(repo, baseline, parent):
    expected_parent = set(baseline) | PREDECESSOR_ADDED
    expected = expected_parent | CHECKED_COPY_ADDED
    require(not set(UNCHANGED_INPUTS) & (CHECKED_COPY_CHANGED | CHECKED_COPY_ADDED)
            and set(UNCHANGED_INPUTS) <= expected_parent,
            "retained protected input cannot enter the checked-Copy delta")
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(CHECKED_COPY_INPUTS) == CHECKED_COPY_CHANGED | CHECKED_COPY_ADDED
            and CHECKED_COPY_CHANGED <= expected_parent and not CHECKED_COPY_ADDED & expected_parent,
            "wrong reviewed checked-Copy input membership")
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
    for name, expected_pin in UNCHANGED_INPUTS.items():
        require((len(parent[name]), sha(parent[name])) == expected_pin
                and (len(inputs[name]), sha(inputs[name])) == expected_pin,
                "changed protected retained input: " + name)
    require(set(REVIEWED_DIRECTIVES) == set(CHECKED_COPY_INPUTS),
            "wrong reviewed checked-Copy directive membership")
    for name, before in parent.items():
        if name not in CHECKED_COPY_CHANGED:
            require(inputs[name] == before, "unreviewed change outside checked-Copy boundary: " + name)
    for name, (length, digest) in CHECKED_COPY_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "checked-Copy source does not match reviewed input: " + name)
        require(source_directives(name, inputs[name]) == REVIEWED_DIRECTIVES[name],
                "checked-Copy compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "source_policy_input_members": len(PREDECESSOR_POLICIES),
              "predecessor_compiler_input_members": len(parent),
              "checked_copy_pins_provisional": PINS_PROVISIONAL,
              "retained_protected_inputs": {name: {"bytes": pin[0], "sha256": pin[1]}
                                            for name, pin in sorted(UNCHANGED_INPUTS.items())},
              "checked_copy_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(CHECKED_COPY_INPUTS.items())],
              "checked_copy_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(CHECKED_COPY_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments", "i64", "integer_widths", "byte_integer", "integer128", "floats", "characters", "project_formatting", "record_field_borrowing", "hosted_native_recursion", "git_dependencies", "git_source_bundles", "inferred_generic_functions", "explicit_generic_arguments", "nominal_generics", "record_patterns", "record_enum_fields", "constructor_inference", "linux_native", "range_for", "hosted_heap", "documentation", "native_debug_variables", "language_server", "editor_navigation", "disjoint_fields", "local_references", "local_reference_bindings", "generic_field_borrows", "native_reachability", "scalar_debug_storage", "macos_native", "typed_vectors", "typed_scalar_ranges", "checked_copy_records"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_checked_copy_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources; checked Copy records qualified separately through source, reference and native tests",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_checked_copy_compatibility.py", Path(__file__).read_bytes())}
    aliases = dict(sys.modules)
    try:
        require(args.prepare_only or not PINS_PROVISIONAL,
                "provisional checked-Copy pins permit prepare-only admission, not qualification")
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
        index_adapter, index_adapter_body = authenticate_index_resource_adapter(repo)
        parser_adapter, parser_adapter_body = authenticate_parser_resource_adapter(repo)
        predecessor_inputs = capture_parent(repo, predecessor_root, set(captured["inputs"]) | PREDECESSOR_ADDED)
        inputs = capture_sources(repo, captured["inputs"], predecessor_inputs)
        manifest = current_manifest(inputs, captured["inputs"], checkout, predecessor_inputs)
        manifest_raw = encoded(manifest)
        manifest_path = output / "current-source.json"
        manifest_path.write_bytes(manifest_raw)
        report.update(checkout=checkout, current_source=identity("current-source.json", manifest_raw),
                      historical_source=identity(BINDING + "/current-source.json", old_source),
                      retained_assertions=identity(RETAINED, original_body),
                      current_input_members=len(inputs), historical_results_apply_to_current=False)
        selected, seam = prepare_current_observer(original, output, captured, observer_adapter, index_adapter, parser_adapter)
        report["byte_buffer_observer_adapter"] = observer_adapter.descriptor()
        report["byte_buffer_observer_adapter_controller"] = identity(OBSERVER_ADAPTER, observer_adapter_body)
        report["typed_vector_index_resource_controller"] = identity(INDEX_RESOURCE_ADAPTER, index_adapter_body)
        report["checked_copy_parser_resource_controller"] = identity(PARSER_RESOURCE_ADAPTER, parser_adapter_body)
        (output / "resource-seam.json").write_bytes(encoded(seam))
        # The separate source manifest is the existing compatibility seam. Other
        # historical captures, identities, comparators and expectations stay exact.
        selected["package_bytes"]["current-source.json"] = manifest_raw

        def unchanged(actual_repo, _capture):
            require(checkout_identity(actual_repo, args.expected_head, args.allow_dirty) == checkout,
                    "checkout changed during qualification")
            authenticate_policies(actual_repo, predecessor_root)
            require(capture_parent(actual_repo, predecessor_root, predecessor_inputs) == predecessor_inputs,
                    "pinned parent compiler materialization changed")
            require(authenticate_observer_adapter(actual_repo)[1] == observer_adapter_body,
                    "ByteBuf observer adapter changed during qualification")
            require(authenticate_index_resource_adapter(actual_repo)[1] == index_adapter_body,
                    "typed-vector index resource adapter changed during qualification")
            require(authenticate_parser_resource_adapter(actual_repo)[1] == parser_adapter_body,
                    "checked-Copy parser resource adapter changed during qualification")
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
        env = dict(os.environ, CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="1", PYTHONDONTWRITEBYTECODE="1")
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
            verified.update(run_file_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_i64_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_integer_width_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_integer128_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_float_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_character_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_vector_observer_controls(repo, output, selected, seam, original, observer_adapter, verified["test_function_executions"]))
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
