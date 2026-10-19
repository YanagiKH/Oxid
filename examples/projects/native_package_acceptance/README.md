# Native package acceptance

Independent acceptance fixture for explicit native packaging
(`tests/native_package_acceptance.rs`, contract in `spec/native-package.md`).
With LLVM 19.1.7 on Linux x86_64 (`OXID_LLVM_BIN=/usr/lib/llvm-19/bin`), from
this directory:

```sh
oxid package --manifest-path oxid.toml --package npkg-app --output app.tar
oxid package --manifest-path oxid.toml --package npkg-library --output lib.tar
oxid package --manifest-path oxid.toml --package npkg-proc --entry-mode process --output proc.tar
```

Packaging never runs the application. After extracting with a standard tar
reader, `./bin/npkg-app` prints `33` (`scale(pair_sum(make_pair()))` = 3 * 11)
and exits 0; `./bin/npkg-library` prints `55`; the Process-mode `./bin/npkg-proc`
writes `PKG` and a newline and exits 3. The app and proc members reach the
library only through their explicit `lib = "../library"` dependency. Managed
executables land only below the selected member, for example
`app/.oxid/typed-preview/result/npkg-app`; the archive path is relative to the
caller's working directory. `npkg-proc` in the default Result mode is rejected
with `E0609` because bounded stdout requires Process entry mode.
