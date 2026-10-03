# Portable observer bounded replay review

The approved portable observer replay passes independent review. Its exact roster
is `scalar-root_child-absolute-entry0-i32` and
`owned-root_child-absolute-root-exclusive_forward`, each in debug and release.
There are four observations and no additional source or native corpus runs.

The reviewed assembly/protocol candidate is
`1b779ee2404a1d579cc6ee64be96cd79454656a12314cd04d4348c7352735bfc`.
It retains core-v1 manifest
`53ed114674e59266c8e9a809f6cba5ef35c9ea819318ab64a7ae6d9aebd20910`,
the byte-identical observer controller/parser, and the exact shared 445-file
source transport. The earlier static/source-only review and synthetic protocol
controls are recorded in `portable-apparatus-review.md` and
`portable-source-review-v1.json`. Those synthetic controls receive no compiler
qualification credit.

The result audit verified 460 identities across the replay manifest, prepared
source, actual Cargo terminal/artifact/profile records, executable, wrappers,
requests and observation artifacts. Both builds used the intended source tree
and profile; each wrapper and child reached a complete observed outcome with
assertions enabled. There are no extra case/profile observations in the roster.

The strengthened passivity comparison was independently rerun over retained
artifacts and reproduced byte-for-byte at
`d63cb7e127670b66fd36995ad59b4117f8a5e24bf17ca9adb45c990c8a3fe834`.
It compares the complete loaded/index structures, namespace work, raw program,
both audit walks, checked immutable body, diagnostics and complete reference
journals with their same-profile standalone counterparts. Known source-root
prefixes are relocated; the sole numeric relocation adjustment is retained path
storage. That delta must equal the actual UTF-8 byte-length change across all
stored SourceFile paths, module canonical paths and canonical root, and index
usage must equal loaded usage. It is not an ignored resource counter.

Scalar LLVM text also agrees after relocation of decoded diagnostic strings
and their checked length declarations/uses, preserving all other bytes. The
owned request has no native descriptor. These are LLVM emission comparisons,
not ELF execution or a new native target qualification.

`portable-replay-evidence-check.json` and `portable-passivity-recomparison.json`
retain this review. The native portable wrapper and final multi-component
publication assembly are separate gates. No compiler was rerun by this review.
