# Module system

## Goals

- keep imports short
- keep resolution predictable
- keep cache keys cheap
- keep module loading readable
- keep module grouping obvious

## Resolution flow

1. normalize the import string
2. resolve relative imports from the current file directory
3. build a stable module key
4. load the file once
5. register exported names
6. reuse cached results when the same canonical path is seen again

## Files

- `stdlib/frontend/modules.ox`
- `stdlib/frontend/pipeline.ox`

## Notes

Module aliases, local `mod` groups, and explicit front-end module summaries are used to reduce boilerplate and keep project code easier to scan.
