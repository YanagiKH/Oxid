# Native release function selection

Status: experimental

Release builds of typed projects using owned values emit function bodies reachable from the selected entry point. This reduces LLVM text spent on unused library functions. Dev builds and the scalar backend retain their existing output.

## Semantics

The compiler still checks every declaration, type and ownership operation and admits the complete native execution plan before selecting bodies. An unused function containing a type error, an ownership error, forbidden I/O or an excessive native resource demand remains an error. The existing function, argument, slot, graph, depth, cost and storage limits are unchanged.

Selection follows all verified direct-call descriptors, including calls in both branches and recursive components. Each function is visited at most once. It does not evaluate branch conditions or perform general dead-code elimination. The entry function is always included. Function identities, arguments, return values, checks, runtime ABI, execution fuel and recursive frame handling remain unchanged.

All existing diagnostic and runtime-helper inventories are retained. The transfer expansion inventory selects the same function bodies as emission. Count and render use one immutable selection and must agree on exact bytes, expansions and work. The LLVM output limit remains 64 MiB; a smaller selected output may fit where the complete output does not.

## Resource accounting

The complete admission precedes selection, so the existing 256-function bounded and 1,024-function hosted limits apply. Reachability uses a fixed sixteen-word bitset and pending worklist. The new named control carriers have a separate conservative 1,024-byte transient bound, added to the existing emitter metadata charge. The inherited complete-output specialization is a zero-sized selector with no additional storage charge or graph walk.

Pending-word scans and direct-call visits are included in count/render work receipts. The additional resource-inventory selection scan is charged once per function. The walk terminates after at most one visit per function; each admitted call descriptor is inspected at most once for its reachable caller. The pending scan checks at most sixteen words per visit and at termination. No heap table or recursive compiler stack is introduced.

## Build workflow

`oxid build --profile release` selects this behavior, including when the profile comes from `oxid.toml`. `--debug-info`, `--reproducible` and `--incremental` remain supported. Existing object keys already bind compiler identity, optimization settings and exact LLVM text. A cache hit still follows source, ownership and native admission.

Debug information describes emitted functions; unused functions can be absent. This change does not introduce library exports or function pointers. Such features must supply any additional externally reachable roots before using this selection.

## Verification

Unit controls cover diamonds, mutually recursive calls, bitset word boundaries, the final admitted function, exact expansion accounting, control-storage refusal and an unused over-budget function. Public CLI checks retain unused type, ownership, recursion and storage errors before tools execute. LLVM tests compare dev/release function definitions and results, debug information, cold/cache-hit executable bytes and deployment after original source paths are moved.

Source-policy controls pin the four changed compiler inputs and one added input, authenticate the preceding policies, and preserve the retained bounded assertions. Optimized native behavior is qualified separately through public tests and real applications. Platform results apply only to the host actually recorded by each run.
