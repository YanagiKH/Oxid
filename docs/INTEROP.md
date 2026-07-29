# Interop

Oxid should be able to call into major host languages with a short, repeatable workflow and then expose Oxid functions back to those hosts.

## Goals

- keep the user-facing steps simple
- keep the bridge descriptions readable
- keep C and C++ as the native boundary
- keep Java and Python as first-class host workflows
- keep Oxid callable from the foreign side as well

## Preview surface

### C

- `c_header(name)`
- `c_library(name)`
- `c_call(symbol, signature)`
- `c_export(symbol, signature)`
- `c_bind(symbol, signature)`

### C++

- `cpp_namespace(name)`
- `cpp_class(name)`
- `cpp_method(name)`
- `cpp_bind(symbol, signature)`

### Java

- `java_package(name)`
- `java_class(name)`
- `java_method(name)`
- `java_bind(symbol, signature)`

### Python

- `python_module(name)`
- `python_function(name)`
- `python_script(name)`
- `python_bind(symbol, signature)`

## Intended flow

1. declare the host target
2. generate a minimal bridge stub
3. link the library or runtime
4. call the host from Oxid
5. call Oxid back from the host

## Notes

The current implementation in this package is a workflow and preview layer. It is designed to guide the future compiler/runtime boundary, not to claim every native bridge is already finished.
