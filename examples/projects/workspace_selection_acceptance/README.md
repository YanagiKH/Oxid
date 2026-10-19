# Workspace selection acceptance

Independent acceptance fixture for explicit virtual-workspace selection
(`tests/workspace_selection_acceptance.rs`). From this directory:

```sh
oxid check --manifest-path oxid.toml --package wsaccept-app
oxid run --manifest-path oxid.toml --package wsaccept-app
oxid test --manifest-path oxid.toml --package wsaccept-app
oxid run --manifest-path oxid.toml --package=wsaccept-library
```

The app prints `33` (`scale(pair_sum(make_pair()))` = 3 * 11) and runs
`test_scale` and `test_pair`. Selecting the library prints `55` and discovers only
`test_library`. Omitting `--package` fails and lists
`wsaccept-app, wsaccept-library`. The app reaches the library only through its
explicit `lib = "../library"` dependency; workspace membership grants no imports.

With LLVM 19.1.7 (Linux x86_64, `OXID_LLVM_BIN=/usr/lib/llvm-19/bin`):

```sh
oxid build --manifest-path oxid.toml --package wsaccept-app
oxid run --manifest-path oxid.toml --package wsaccept-app --engine native
oxid test --manifest-path oxid.toml --package wsaccept-app --engine native
```

The artifact is `app/.oxid/typed-preview/result/wsaccept-app`; the virtual root
receives no application output. A first publication leaves `owner` and `next`
receipts and no `current`.
