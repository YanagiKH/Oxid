# Native file ownership and binary I/O

Status: experimental, `typed-preview`, Linux x86_64.

`std::fs` opens and creates files, reads and writes byte slices, synchronizes
file contents and closes handles. A `File` owns its OS descriptor. It moves
through functions, record fields and enum payloads; it cannot be copied,
constructed from an integer or converted to a byte slice. Leaving its owning
scope closes it, including after a runtime error.

```oxid
use std::fs::OpenResult;
use std::fs::create;
use std::fs::write_some;
use std::io::WriteSomeStatus;

fn main() -> i32 {
    let path = b"hello.txt";
    let opened = create(&path);
    match opened {
        OpenResult::Opened(file) => {
            let mut output = file;
            let bytes = b"Hello, Oxid!\n";
            let result = write_some(&mut output, &bytes);
            match result {
                WriteSomeStatus::Written(count) => { return count; },
                WriteSomeStatus::Error(code) => { return -1; },
            }
        },
        OpenResult::Error(code) => { return -2; },
        OpenResult::MemoryLimit => { return -3; },
        OpenResult::AllocationFailed => { return -4; },
    }
}
```

| Function | Result |
| --- | --- |
| `open_read(path: &[u8])` | `OpenResult` |
| `create(path: &[u8])` | `OpenResult` |
| `read_some(file: &mut File, bytes: &mut [u8])` | `std::io::ReadSomeStatus` |
| `write_some(file: &mut File, bytes: &[u8])` | `std::io::WriteSomeStatus` |
| `sync_all(file: &mut File)` | `FileStatus` |
| `close(file: &mut File)` | `FileStatus` |

`OpenResult` has `Opened(File)`, `Error(i32)`, `MemoryLimit` and
`AllocationFailed`. `FileStatus` has `Ok` and `Error(i32)`. The existing
`ReadSomeStatus` and `WriteSomeStatus` identities remain shared with `std::io`.
Aliases can distinguish the two modules' `read_some` and `write_some` imports.

Paths are Linux path bytes, not necessarily UTF-8. Relative paths use the
process working directory. An interior NUL returns `Error(22)` before any
open or truncation; 4096 or more path bytes return `Error(36)`. An empty path
is passed to the OS. Files use close-on-exec. `create` follows ordinary Linux
path resolution, creates a missing file with permissions `0600` filtered by
the process umask, and truncates an existing file. It is not an atomic replace.

Each operation makes at most one OS attempt. Reads return positive `Data(n)`,
`Eof`, or `Error(errno)`; an empty slice returns `Data(0)` without reading.
Writes return `Written(n)` or `Error(errno)`; an empty slice returns
`Written(0)` without writing. Partial progress is successful progress. Callers
loop over the remaining slice and choose whether to retry `EINTR`. A read error
does not establish useful bytes in the requested slice. A completed write is
not rolled back by a later user error.

`sync_all` uses `fsync` for that file's data and metadata. It does not synchronize
the containing directory. `close` releases the handle even if the OS reports an
error; Linux close is never retried after `EINTR`. Closing an already closed
handle returns `Ok`. Reading, writing or synchronizing a closed handle returns
`Error(9)`, including for an empty slice. Automatic cleanup discards close
errors; explicit close allows the program to inspect them. SIGPIPE is ignored
before source execution so a broken pipe can produce `Error(32)`.

The runtime assigns monotonically increasing private identities and checks their
resource class against the execution registry. Each live file adds one 32-byte
registry allocation to the same 16 MiB normalized live-memory limit as byte
buffers. Failed opens and interrupted closes release their registry storage.
The handle's 16-byte value layout and enum payload layout remain private.

All result extents, generations, return work and loan releases are prepared
before an OS attempt. Allocation refusals precede `open`, including `O_TRUNC`.
The public open work is `3 * path_length + 1` for accepted path lengths;
oversized paths need no byte scan. Read/write work pays the requested extent,
not only the eventual progress. Bounded execution prepays the immediate return;
hosted execution retains these checks without an accumulated program budget.
Path scratch, registry storage and compiler control carriers are separately
accounted. A raw descriptor claim does not authorize a syscall.

This increment does not add seek, directory operations, arguments, processes,
async I/O, deadlines, cancellation or additional native targets. Those remain
separate runtime work.
