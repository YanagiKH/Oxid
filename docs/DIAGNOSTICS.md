# Diagnostics and source ranges

Oxid attaches source ranges to parsed statements in the form:

```text
path/to/module.ox:start_line:start_column-end_line:end_column
```

Module compilation parses each file with its repository-relative source name. Imported functions retain their defining source directory for relative imports, and runtime errors are wrapped with the active statement range. OXBC artifacts serialize these ranges, so running a compiled artifact retains cross-module locations.

Parser errors report the source name and token position available at the failure. Diagnostics are designed for readable command-line output; structured diagnostic codes and editor protocols remain future frontend work.

