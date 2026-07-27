# Macros and Compile-Time System

The current `macro` feature is a single-line source-level preprocessor.

Format:

```oxid
macro greet(name) => print "Hello, " + name;
```

Limits:

- Expansion currently happens as string replacement.
- This is suitable for boilerplate reduction and fixed syntax fragments.
- It can later be replaced with a real token-level macro system.
