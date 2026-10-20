# Explicit native project package

```
oxid package --manifest-path PATH --output ARCHIVE
oxid package --manifest-path WORKSPACE --package NAME --output ARCHIVE --entry-mode process
```

This opt-in command checks and builds the selected typed project with the ordinary native backend, then packages the retained executable. It never runs the application. `--entry-mode result|process` defaults to `result`; `--profile dev|release` defaults to `dev` (O0), while release uses O2 for generated IR and the C runtime; `--message-format text|json` is supported. Workspace selection and local path dependencies use the same immutable compiler session as CHECK, RUN, BUILD and TEST. The selected project's managed name is required and must match `[A-Za-z0-9][A-Za-z0-9_-]{0,63}`. Neither the archive filename nor a reloaded manifest supplies that name. Without `--manifest-path`, the legacy package route is unchanged.

`--output` is required, accepts either `--output ARCHIVE` or `--output=ARCHIVE`, and is relative to the caller's working directory. Its parent directory must already exist. An existing file, directory or symlink, including a dangling symlink, is refused before compilation. A hard-link publication after serialization is the final no-replacement check if another process creates the output meanwhile. The temporary archive lives beside the output; cleanup is best effort. There is no crash-durability or atomic-snapshot guarantee against noncooperating filesystem mutation.

The command uses the existing managed executable namespace `.oxid/typed-preview/result/NAME` or `.oxid/typed-preview/process/NAME` below the selected project. Release uses `.oxid/typed-preview/release-result/NAME` or `.oxid/typed-preview/release-process/NAME` instead. It retains the exact artifact descriptor and typed profile returned by that build. Archive reads start at offset zero with positional I/O, without reopening the mutable executable pathname or changing a shared file cursor. Only a regular file of at most 64 MiB is accepted; the `u64` size check precedes conversion and fallible allocation. This is a packaging limit, not an increase in compiler or runtime budgets.

The uncompressed ustar archive contains, in order:

- `bin/NAME`, mode 0755: the retained ELF executable bytes.
- `package.json`, mode 0644: schema `oxid-native-bundle-v1`, selected name, target, native profile, explicit entry mode, executable path, SHA-256, and host-provided dynamic runtime metadata.
- `README.txt`, mode 0644: extraction, execution and compatibility guidance.

The descriptive native profile is `native-preview-O0` for dev and `native-preview-O2` for release; it comes from the retained artifact, not the filename. See [native build profiles](native-build-profiles.md) for CLI applicability and publication isolation.

Owner IDs and timestamps are zero; names, order and padding are fixed. Identical executable bytes and metadata produce identical archive bytes. This does not promise reproducible compiler builds or universal deployment. The build host remains Linux x86_64 with LLVM/Clang/LLD 19.1.7. Execution requires a compatible Linux x86_64 host with the necessary dynamic runtime libraries; these libraries are not bundled. No registry access, downloads, dependency installation, arbitrary executable input or implicit project discovery is added.

The two publication phases are reported separately. A build failure publishes neither a new executable nor an archive. A later packaging failure can leave the new managed executable published while no archive from this invocation exists. Reporting failure after the archive hard link leaves both files published. No failure restores the previous managed executable or removes a published archive.

JSON output consists of ordinary diagnostic records followed by exactly one `package-summary` record, with `schema_version`, `edition`, `kind`, `success`, `errors`, `entry_mode`, `managed_published`, `managed_output`, `archive_published`, and `archive_output`. Unpublished paths are null; successful managed publication remains visible even when `success` is false. There is no intermediate build summary. If the reporting stream itself fails, the command attempts an `E0721` stderr diagnostic naming both publication facts; a complete JSON record cannot be guaranteed on a broken stream. Output preflight, artifact read and serialization/publication failures use `E0720`. Existing compiler diagnostics retain their codes. Exit status is zero on success, two for an `E0500` invariant failure, and one for other failures.

The package route adds one fallibly allocated output-path string outside the unchanged manifest Request. The borrow-only scanner adds one `Option<&str>`; Route still fits its preexisting maximum variant. Ordinary development inspection on Linux x86_64 measured Route 56 bytes/alignment 8, test-build Request 80 bytes, Scan 144 bytes and the existing named-owner inventory 1944 bytes. The output payload and executable byte buffer belong to the package phase and are not a new claim about the manifest transport envelope. No frozen layout baseline or admission limit was changed, and these observations are not source qualification.

Ordinary tests use synthetic ELF bytes to exercise the supplied V2 serializer and real managed-file publication. `cargo test --test native_package_core` also checks CLI, workspace, preflight and failure reports without LLVM. The ignored native case needs `OXID_TEST_LLVM_BIN` and a standard tar reader; it independently extracts and runs Result/Process artifacts. Actual supported-toolchain acceptance is separate evidence.
