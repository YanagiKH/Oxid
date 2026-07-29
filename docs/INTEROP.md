# Interop

Oxid interop previews cover C, C++, Java, and Python.

## Desired workflow

1. declare the foreign target
2. generate a small bridge stub
3. link the native or runtime dependency
4. call it from Oxid
5. expose Oxid back to the host if needed

## Intent

Interop should feel like a normal Oxid workflow rather than a separate subsystem.
