# Linked C and C++ helpers

Oxid's Rust host links four demonstration functions from [native/](../native/) through [build.rs](../build.rs):

- `c_len(text)` and `c_hash(text)`
- `cpp_len(text)` and `cpp_hash(text)`

These fixed helpers exercise the host's C/C++ ABI boundary. They are not a general-purpose foreign function declaration system, and they do not compile `.ox` source into native code. Building the host from source requires a C/C++ compiler; release binaries already include the helpers.

For running external programs or calling Oxid from another language, see [Interop](INTEROP.md).
