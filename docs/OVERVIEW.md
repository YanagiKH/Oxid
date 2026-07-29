# Overview

Oxid is organized around a small Rust bootstrap boundary and a growing Oxid-written developer surface.

## Structure

- Rust keeps the parser, runtime, filesystem, and CLI bootstrap boundary alive.
- Oxid owns the workflow previews, command summaries, package helpers, and self-hosting documentation.
- The visible developer experience should be runnable directly from the repository after clone.

## Practical goal

A user should be able to:

1. clone the repository,
2. run the bootstrap path,
3. execute Oxid examples,
4. preview the frontend and interop surfaces,
5. start writing Oxid-based projects immediately.
