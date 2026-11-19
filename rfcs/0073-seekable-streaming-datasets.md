# Absolute file seeking and dataset starting rows

## Public behavior

Add the closed builtin `std::fs::seek_absolute(file: &mut File, offset: i64) -> FileStatus`. A successful call sets the OS position to the nonnegative byte offset from the beginning of the file. A seek beyond EOF succeeds when the OS permits it; subsequent reads observe EOF until data exists there. The operation neither grows the file nor reads or validates skipped bytes. A negative offset returns `FileStatus::Error(EINVAL)` without an OS attempt. A closed file returns `Error(EBADF)`. Other OS errors, including `ESPIPE` and `EINTR`, are returned from a single attempt with no retry.

File support remains limited to the previously admitted 64-bit little-endian Linux/macOS x86_64 and ARM64 paths. Both reference and native helpers require signed 64-bit `off_t`. This RFC adds no Windows File execution or general cross-platform guarantee.

The streaming dataset reader adds `open_at(path, batch_size, start_row)`. It validates batch size, the nonnegative start, the existing header/dimensions, and `start_row < rows`. For start zero it delegates to the original sequential `open`. Otherwise it computes `24i64 + start_row * (features + targets) * 8i64` using checked arithmetic, seeks and clears prefetched bytes only after success. A failed seek closes the newly opened reader and returns its error. No partly repositioned reader is returned.

`position` is the absolute row index, `row_count` remains the complete header count, and `remaining` is their difference. The original `next` operation retains batch atomicity, poison/cleanup behavior and exact EOF validation. Only selected rows are decoded and validated. Header prefetch can read skipped bytes but does not validate them. The dataset must remain unchanged throughout an application run.

The training application accepts an optional fourth positional starting row. Omitted/zero starts retain the old invocation; every epoch and initial/final evaluation reads the same suffix. Reported rows and batch counts describe that suffix. The model is still initialized from its existing fixed seed: this is dataset selection, not checkpoint resume or shuffling.

## Authority and effect ordering

The builtin retains the existing `FileStatus` nominal identity and exact exclusive `File` reference. Its scalar argument is `i64`, not `i32`; the verifier checks the canonical function, ordered signature, result owner and instruction. Aliases resolve through the catalog, and ordinary loan/move rules still apply.

The reference consumer uses the existing complete File return preparation. It validates the destination owner and caller result, generations, caller continuation and loan releases, then prepays the return cost before seeking. The File registry and wide scalar value are checked before the OS call. After the OS attempt, result/loan delivery consists only of prepared writes and release updates; there is no new allocation or fallible return admission.

The native emitter calls a dedicated helper with an `i64` offset. The existing six-operation File helper and its ABI remain unchanged. The new helper validates descriptor class, result nonoverlap and complete return fuel before calling `lseek`, then writes the existing status. Hosted execution uses the existing per-operation unlimited work convention while retaining authority, memory and error checks. There is no retry loop.

## Compatibility and accounting

Append Item ordinal 27 and Function ordinal 19. Existing fixed catalog IDs remain unchanged. Old vector inventory bits 27 through 36 remain at those exact positions; seek uses bit 37. The packed inventory stays eight bytes with byte alignment. Actual catalog iterator lengths grow to 28/20 and are included in the catalog's typed compiler control bank. No budget ceiling increases.

No File descriptor, Machine, Frame, owner, loan, result or heap-ledger layout changes. No new registry allocation is needed for seek. The existing statement/return charge formulas remain intact. The additional wide offset decode and OS helper roles, native helper frames and emitter formatting scratch are separately represented and charged only when an admitted witness contains seek. New LLVM helper declarations are likewise conditional. The shared C runtime translation unit contains the new helper, so its compiled object may change even for older programs; this does not imply unchanged object-byte identity.

## Acceptance boundaries

Tests cover old IDs/vector bits, aliases and wrong resource/borrow/offset types; raw signature/site forgery; reference return generation/loan preparation and exact return fuel versus one less; native result overlap, forged descriptors, negative offsets, one-attempt interruption, hosted behavior and closed handles. Native C and public programs use real sparse offsets above 4 GiB and real nonseekable input, observing bytes after failure.

The application controller independently computes selected-row SGD and row bytes. It checks explicit-zero/default equality, prefix prefetch invalidation, cross-buffer starts, skipped NaN versus selected failures, EOF/trailing/truncated behavior, invalid starts, sparse datasets, frozen locks and previous artifact preservation. It builds O0/O2 with each supplied compiler and runs copied native binaries after moving source roots, from fresh working directories with an empty environment. It preserves raw command streams, compiler/tool/source/lock hashes and bounded sparse fixture definitions. No benchmark or complete G4/G9 claim follows from these checks.
