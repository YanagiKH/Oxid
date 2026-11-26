# File worker

Count the bytes in files sent to a background process, one path per line.

```sh
cd apps/file-worker
oxid build --profile release
printf '%s\n' oxid.toml README.md | .oxid/bin/file-worker
```

Send regular file paths relative to the worker's current directory, or use
absolute paths. The worker accepts LF and CRLF line endings and reads a final
line without a newline. Spaces in paths are preserved. Each path can contain
up to 4,095 bytes. Paths to directories and named pipes produce an error.

The worker processes jobs in order and writes one line per finished job.

- `OK 241 1` means it read 241 bytes on its first attempt.
- `ERR io:2 3` means the file was still missing after three attempts. The
  worker waits 250 milliseconds before retrying a missing file.
- `ERR invalid-path 0` or `ERR path-too-long 0` means it rejected the path
  before opening a file.

Close the input to finish after the queued jobs. On Linux and macOS, press
Ctrl+C or send SIGTERM to stop the worker. It exits with status 130 and may
leave jobs without responses. A failed job still allows later jobs to run.
A stream failure ends the worker with status 74.
