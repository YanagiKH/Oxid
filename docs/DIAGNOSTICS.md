# Diagnostics

Oxid diagnostics should be compact, structured, and actionable.

## Required parts

- severity
- file path
- line
- column
- message
- hint
- recovery suggestion when possible

## Output shape

A diagnostic should read like:

```text
error: message
 --> file.ox:12:8
  = help: small hint
```

## Better-than-minimum behavior

- include a snippet when available
- include a suggestion when a parse shape can be simplified
- classify recoverable warnings separately from hard errors
- keep the wording short
- keep the fix close to the source of the issue
