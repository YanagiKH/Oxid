# RFC 0015: bounded typed modules, direct imports and visibility

Status: experimental public integration. Unit1 source-set/loader groundwork,
Unit2 shared declaration/import/visibility indexing and Unit3 linked execution
are now selected by public `typed-preview` check, run and compile. Unit4 adds
one-pass syntax recognition and preserves the original single-file route.
[Unit4 qualification](../docs/architecture/typed-project-unit4-validation.md)
records exact current-source, archived-source and platform evidence separately.
Declared-child loading admits Linux; other hosts return E0005/source. Native
remains Linux x86_64, LLVM 19.1.7 at O0. This describes the repository capability,
not support in an older released executable.

Proposal baseline: merged main `fbcfeb2a2de8fe9d335d6c8051d254cccdb663dc`, tree
`658c83465aed3a1423483dcdcc23a3d10037131f`. This proposal extends
[RFC 0014](0014-owned-structs-call-borrows.md) and the existing
[typed](../spec/typed-preview.md) and [native](../spec/native-preview.md) contracts.
Those specifications describe the available source language. The historical
[Unit1 ledger](../docs/architecture/typed-project-unit1-validation.md) and
[Unit2 ledger](../docs/architecture/typed-project-unit2-validation.md) record their
separate source identities, representation measurements and qualification.
The [Unit3 ledger](../docs/architecture/typed-project-unit3-validation.md) records
linked execution, the additional origin-association work, exact finite coverage
and the test-only publication successor's provenance bridge.

The architecture has been reviewed for staged implementation. That decision is
not acceptance of the full implementation or public activation. Implementation
and review ownership are recorded by the associated pull requests. There is no
new feature-inventory entry for private groundwork, and no M1, M2 or v1.0
completion claim.

## Implementation status and how to read this RFC

Sections 1–10 specify the bounded capability. Their syntax examples,
namespace/visibility outcomes, cross-file execution and pilots are requirements.
Public activation and exact qualification are recorded separately in Unit4.

| Area | Implementation state |
| --- | --- |
| Immutable source set, file-aware text and AST handles | Shared production source-set facade |
| Explicit child discovery | Declaration-only loader, with Linux admission and bounded filesystem policy |
| Direct imports, public declarations/fields and absolute item paths | Existing parser selected through one-pass load_typed |
| Declarations, imports, visibility and global index | Shared scalar/owned resolution and type checking |
| Linked scalar/owned program and consumers | One sealed program with original root-main identity |
| Public check/run/native compile | Unit4 dispatch; exact source/target qualification in the Unit4 ledger |

The private parser extends the existing parser. The narrow ModuleCandidate and
load_modules entry points preserve Unit1 discovery behavior; ProjectCandidate
and load_project_candidate provide the new private grammar. The shared index
checks effective visibility, signature exposure, construction and field access.
Its seal certifies source associations, original conflicts and atomic imports;
linked ownership verification remains separate. One-file code preserves its
previous local IDs, route selection and distinct scalar/owned diagnostic
schedules. An ordinary root with no module declarations acquires no new
file-kind, canonicalization, case or path admission.

Private type-only and executable checking share one route/index/resolution/type
schedule. Executable checking uses the existing selected lowerer and raw verifier,
with a separate allocation-free source-association audit between them. The source
seal binds the immutable map, verified body and original root-main identity for
both consumers. Incorrect original source/map/parser associations now produce an
internal diagnostic with null origins; that deliberate malformed-input change
has separate controls and is not claimed as unchanged Unit2 behavior.

## 1. Decision, value and exact boundary

Add declaration-only, local multi-file projects to the existing explicit
`typed-preview` check/run/LLVM compile routes. An entry file declares a bounded
tree of modules. Existing scalar and nominal owned values, moves and call-only
borrows work across those modules through one linked, verified in-memory OIR
program and the existing consumers. This makes small reusable APIs possible and
advances the roadmap's M1 namespace/visibility/source-map dependency for CLI,
background-job and eventual compiler-provider work. It is not all of M1, a new
memory model, a package system, or a production/v1.0 qualification.

Accepted types stay bool, i32, unit and nominal move-only structs whose fields
are scalar. Reference parameters and temporary call borrows/reborrows retain
their present exact-mode rules. No stored references, new lifetimes, nested
owned fields, destructors, heap, numeric types, globals, module initialization,
first-class functions, methods or new native targets are added.

Source entry remains one positional path. Existing command/option validation,
legacy selection, native tool/output admission and no-clobber rules remain.
Under this proposal, `check`/`run` read the entry and explicitly declared local module files;
they create no output/cache files and perform no user-program initialization.
`check` permits a project without main. `run`/`compile` require the original
function named main declared in root module 0, with current zero-argument and
scalar-result restrictions. An imported alias named main is not an entry.
A child module's main is an ordinary function. Root main need not be pub.

Excluded syntax/features: inline modules, string/path attributes, module aliases,
relative `self::`/`super::` paths, bare `m::f`, glob/group imports, `pub use`,
reexports, import-through-import aliases, external packages, manifests/lockfiles,
directory discovery of source modules, network search, separate compilation,
incremental caches, public/stable native ABI and new backend targets.

## 2. Grammar and contextual identifiers

Extend the current grammar only in these positions:

```text
item          := module_decl | import_decl | function | struct_decl
module_decl   := "pub"? "mod" name ";"
import_decl   := "use" absolute_item_path ("as" name)? ";"
function      := "pub"? "fn" name ...existing function grammar...
struct_decl   := "pub"? "struct" name "{" field_decls? "}"
field_decl    := "pub"? name ":" scalar_type
absolute_item_path := contextual_crate "::" name ("::" name)*
item_path     := name | absolute_item_path
value_type    := scalar_type | item_path
parameter_type := value_type | "&" item_path | "&" "mut" item_path
direct_call   := item_path "(" existing_arguments? ")"
struct_literal := item_path "{" existing_field_initializers? "}"
```

An absolute path has at least one segment after crate. Its intermediate segments
must be original module declarations; its endpoint is selected in the requested
namespace. Keep the existing lexer: `::` is **two Colon tokens** and both count
in the non-EOF ledger. In a supported contextual path position, require the
first colon's end byte to equal the second colon's start byte. Trivia may
surround a delimiter but cannot split its two bytes. Do not introduce a global
DoubleColon token or change ordinary invalid syntax such as `fn main()::i32 {}`
to a two-byte diagnostic span. `crate::f` is valid, as is
`crate::outer::f`. The grammar for borrow places, scalar field types, field
projections and assignment targets does not gain paths: they remain local
places such as `state.value` and `&mut state`. Qualified functions remain direct
callees rather than first-class values. The existing restriction on constructors
inside unparenthesized conditions applies equally to qualified constructors.

Only `mod`, `use`, `pub` gain keyword token kinds; they were already unsupported
reserved words. `as`, `crate`, `self`, `super`, `bool` and `i32` remain ordinary
identifier tokens. Recognize crate contextually only when followed by `::` in
an item-path position; recognize as only between a complete import path and its
alias. Thus `fn crate`, `fn as`, `let crate`, `struct crate` and `fn self` stay
legal. `use crate::as as crate;` is legal when its original endpoint exists.
`crate::crate::as` can name an ordinary module crate and declaration as.
`self::x`/`super::x` remain unsupported relative prefixes, but
`crate::self::x` can name an ordinary module self. Casts such as `5 as i32`
remain E0101/parse with their existing ordinary diagnostic.

`pub` is allowed once on the four listed declaration kinds, at file scope except
for fields. `pub use`, `pub let`, `pub(crate)`, nested mod/use statements, inline
module bodies and group/glob imports produce E0101/parse at the unsupported
construct. Missing required punctuation/name uses E0100/parse. A parse failure
does not cause any declared child file in that file to be loaded.

## 3. Modules, namespaces, imports and visibility

### 3.1 Names and lookup

Each module owns separate value and type/path namespaces. Functions occupy
value; structs and child modules occupy type/path. Therefore a function and
struct, or function and module, can share a spelling. A module and struct cannot.
Builtin scalar type spellings bool/i32 cannot be rebound in type/path, including
by a module or type import, but remain available as value names exactly as
before. Fields are local to their nominal RecordId and never module bindings.

An unqualified type/callee looks only in the current module's original
declarations and imports. There is no implicit parent search and no inherited
import scope. Visibility grants permission, not an unqualified binding. A bare
expression name remains a local variable. Existing local scope/initialization
and no-active-shadowing rules continue; a parameter/local cannot shadow any
current-module value declaration/import, even one written later. A parent or
sibling function accessible only by an absolute path does not prohibit a local
of that spelling. Locals may share a spelling with a type or module.

All original declarations in all loaded modules are collected before imports,
signatures or bodies resolve. Forward calls, mutually referring signatures and
cross-file recursive calls can resolve. Import declarations do not load files;
their module tree must have been declared with mod. Reference execution keeps
its recursion/fuel limits; native whole-project recursion admission is unchanged,
including unreachable functions and unused modules.

### 3.2 Direct imports and atomic paired binding

`use crate::m::X;` examines only original endpoint declarations in m. It imports
the function X, the struct X, or both if both exist. An endpoint module is not
importable. When a module and function share X, import only the original
function; the module is ignored as an ineligible endpoint and no module alias
is installed. A module-only endpoint is rejected. An endpoint import alias is never followed. There is no transitive
alias chase, reexport or import-order fixed point. Absolute qualified paths also
traverse original module declarations and select original endpoints, never
another module's import aliases. An import alias is available unqualified only
inside the module containing that import.

For each import, perform these steps in order: resolve intermediate original
modules left-to-right and check each one's visibility; discover original
function/struct endpoints; reject a module-only/alias-only/missing endpoint;
check type endpoint visibility before value endpoint visibility; reject any
original endpoint already imported into this destination module (type then
value order); check the alias against builtin type names and existing bindings
in type then value order; commit all affected bindings and target-seen records
together. Both endpoints must be accessible when
both exist. An inaccessible half is an error, not a reason silently to import
only the other half. An import never makes the target more visible.

Original declarations are preinstalled and always win over imports, regardless
of textual order. Imports are then considered in lexical order per module.
Any import/import or import/declaration collision in an affected namespace is
E0201. Importing the identical original endpoint twice is also E0201 even under
different aliases: `use crate::m::f as a; use crate::m::f as b;` is rejected at b,
with a as the first-import secondary. Track this per destination module and
namespace/original target ID; original declarations are not themselves imports.
For an overlap in either endpoint of a paired import, reject the whole import;
type overlap wins over value overlap, and the earliest committed import of that
target supplies the secondary. A paired import failing one namespace installs
neither half and adds no target-seen entries. A struct-only alias does not collide with
an existing function; a function-only alias does not collide with a struct or
module. Imported aliases themselves are module-private and not available as
qualified original endpoints. Mutual direct imports between modules are legal:
a and b may import each other's original f/g declarations. The import-dependency
graph may therefore cycle, and no initialization order is implied. Only recursive
alias-resolution dependencies are absent; original-only targets need no alias
chase or import fixed point. Do not reject a module graph just for mutual imports.

### 3.3 Full requester domains, including External

Visibility is defined independently of which modules or callers happen to be
loaded. Let the requester universe be every possible internal module path under
root, plus one abstract External requester. External is a specification device
for public API exposure; this capability does not compile external packages.

Use domains Public (the whole universe) and Subtree(m) (m and all possible
internal descendants of m; never External). In particular Public and
Subtree(root) are different even when all present callers are internal.
For an item declared in module m, its own permission is Public if pub, otherwise
Subtree(m). For a module n declared by its parent p, its own permission is Public
if pub, otherwise Subtree(p), **not Subtree(n)**. Root has Public permission.

The effective domain E(item) is the intersection of its own permission and the
permissions of every ancestor module on the path from root to its defining
module. For a module itself, include that module's own permission. Domains on a
declaration path are nested, so the intersection is Public or the narrowest
Subtree restriction. A requester r may access an item only if r belongs to this
effective domain. Path traversal checks every ancestor, including an ancestor
that is not explicitly spelt by an unqualified imported use. A prior successful
import is not an exemption from the target's domain.

For each pub function, and uniformly for all function signatures, require
E(function) to be a subset of E(T) for every nominal by-value parameter/result
and borrowed parameter referent T. Scalars impose no restriction. Imported type
aliases use their original RecordId/domain. Compare domains structurally:
Public is a subset only of Public; Subtree(a) is a subset of Public and of
Subtree(b) exactly when b is a prefix/ancestor of a. Do not enumerate only loaded
call sites. This rejects a pub root function returning a private root type even
if no caller uses it. It permits a pub function inside a private module when
the enclosing restriction makes the type sufficiently visible.

For a field f declared in struct S in module m, E(f) is E(S) intersected with
Public if pub, otherwise Subtree(m). Construction must be permitted for **every
declared field**, including fields absent from an invalid initializer; reading
or assigning a projection must be permitted for the selected field. A pub struct
does not make its fields public. A fieldless pub struct has no field privacy
restriction and can be constructed wherever the type is accessible. Moving,
borrowing, assigning an entire value and returning it need no access to its
private fields. Module descendants may access private ancestor fields via an
accessible type/path, just as the requester rule says.

## 4. Source identity and filesystem contract

### 4.1 One logical path, one expected file

For a typed entry `/work/app.ox`, root is crate and a logical module
`crate::jobs::retry` maps only to `/work/jobs/retry.ox`. `mod jobs;` in root
loads `/work/jobs.ox`; `mod retry;` in jobs loads `/work/jobs/retry.ox`.
There is no `mod.ox`, extension inference, parent search or working-directory
fallback. ASCII identifier grammar prevents path separators, dot components,
absolute paths and lexical escapes in module components.

Open/read the entry using its supplied path and preserve that exact spelling
in SourceMap, including relative components and entry symlinks. For an ordinary
one-file source, add no canonicalization, file-kind or case requirement. If the
parsed entry contains a mod declaration, canonicalize the **containing directory
of the invoked entry path** as project root and canonicalize the entry's file
path for identity. If `/view/app.ox` points to `/else/app.ox`, its modules are
under canonical `/view`, not `/else`. This entry exception may point outside
project root; module files may not. Parent-directory symlinks in the entry path
are handled by canonicalizing that directory, retaining old entry behavior.

Every component below canonical project root used for a module must be an exact
case-matching directory entry, with no symlink/reparse-style indirection. The
final component must be a regular file; intermediate components must be
directories. Reject symlinked final files, symlinked module directories,
dangling links, special files and canonical escapes. The implementation must
explicitly qualify equivalent platform detection before claiming another host;
a platform that cannot establish this contract returns a source-policy error,
not a silent weaker policy. Native qualification remains Linux x86_64 only.

After those checks, canonicalize the file and require a component-wise path
prefix under canonical project root. Use the canonical pathname as identity;
reject a canonical path already assigned to root or a prior module. Do not use
lossy Unicode path strings as identity. Distinct hardlink paths are distinct
modules, even if they share an inode/content; their structs remain nominally
distinct. No inode-based or cross-platform file-object identity promise is made.

Declared sibling module components must be unique both exactly and under ASCII
case folding. Thus `mod Jobs; mod jobs;` is rejected even on case-sensitive
filesystems. Apply this check at every parent, which also prevents case-folded
full logical-path collisions. Undeclared filesystem entries are not modules:
an unrelated `JOBS.ox` does not become a source or an ambiguity by itself.
The expected filename still must match case exactly, so `mod jobs;` cannot load
only `Jobs.ox` even on a case-insensitive host.

Exact-case verification may enumerate directory metadata solely to verify a
requested component, without opening unrelated files or discovering modules.
Scan the entire immediate directory unless its scan budget is exhausted, with
constant retained scratch, count entries/name units before retaining anything,
and record two booleans: an exact requested component exists, and an ASCII
case-fold match exists. After a successful complete scan, exact match permits
the component even if another differently cased entry exists; no exact match
but a folded match gives E0005 case mismatch; neither gives E0002 missing.
These flags never open or load other files. Either scan cap gives
the same bounded `module directory scan budget exceeded` diagnostic at the
current mod name; do not expose which cap happened to cross first or partially
observed case/missing-file facts. Thus success or this resource rejection does
not depend on enumeration order. Metadata I/O failures retain their ordinary
host-specific error qualification. No directory names need sorting or bulk collection. Component
verification results may be retained within this invocation as counted records;
there is no cross-invocation cache. Resource caps below include these probes.

These rules govern a stable filesystem. Ordinary path-based checks and opens
are not an adversarial-filesystem/TOCTOU sandbox, cannot prevent replacement
between metadata checks and reads, and impose no bound on OS I/O blocking.
The consumed source bytes are immutable after reading; do not reopen sources
for diagnostics or execution. Report source hashes in qualification manifests,
not an unsupported snapshot-consistency or content-authenticity guarantee.

### 4.2 Loading order, source spans and global declaration IDs

Parse root completely before visiting children. Then traverse child modules in
their declaration order, depth first, completing each child subtree before the
next sibling. All declared modules are loaded and checked, including unused
ones. ModuleId and SourceFileId are root 0 followed by this DFS preorder; each
successfully loaded module owns exactly one AST and one source file. A failed
file read/encoding check has no fabricated source file span: its primary is the
mod name in the declaring source. Display path for a child is the entry's
supplied parent path joined with its logical relative file path; do not replace
it with a canonical or target-symlink path. Retain canonical identity separately.

**ID ordering is module-major, not declaration-interleaved recursion.** First
assign every root function DefId and every root struct RecordId in lexical
per-kind order, including declarations following mod statements. Then assign
all declarations of each next DFS module, again lexical per-kind order. DefIds
and RecordIds are independent dense global sequences. Imports never allocate
either. FieldId is original global RecordId plus original lexical field index.
Within a function preserve predecessor local/place/expression/body-block order.
For one ordinary file, all these IDs remain exactly their old values.

Example: root `fn r0; mod a; fn r1; mod b;`, a `fn a0; mod c; fn a1;`, c
`fn c0;`, b `fn b0;` yields files root,a,c,b and function IDs
r0=0,r1=1,a0=2,a1=3,c0=4,b0=5. This schematic uses function headers only to show
order; actual fixtures must contain valid bodies.

All offsets remain local UTF-8 byte offsets in their original file. Spans keep
SourceFileId. Never concatenate source strings, offset-rebase the program, or
lookup source text using only start/end. Same offsets and same spelling in two
files do not identify the same declaration. A name can span tokens/trivia in an
absolute path but each segment retains its own original span. Primary and every
secondary diagnostic label select their own file independently. Existing LF,
CRLF, Unicode scalar column, control-character escaping and invalid-IR-span
handling remain unchanged.

## 5. Shared architecture and APIs

This is a private frontend design, not a new public Rust API. Names below are
contractual responsibilities; final code names may differ with reviewed mapping.

```text
SourceSetBuilder::load(entry, ProjectLimits) -> Result<ProjectSources, Diagnostics>
ProjectSources {
    frozen SourceMap,
    module_headers: dense ModuleId array,
    per_file_programs: dense SourceFileId -> ast::Program,
    original_item_handles,
    counted_source_and_path_usage,
    syntax_flavor: OriginalSingleFile | ProjectSyntax
}
ProjectSources::text(Span) -> &str                 // file-aware; checked origin
ProjectSources::file_ast(SourceFileId) -> &Program
ProjectSources::function(FunctionAstKey) -> &Function
FunctionAstKey = (SourceFileId, local_function_index)
RecordAstKey = (SourceFileId, local_record_index)
ExprKey = (SourceFileId, ast::ExprId)
BlockKey = (FunctionAstKey, ast::BodyBlockId)

DeclarationIndex::preflight(&ProjectSources, Limits) -> ProjectIndexUsage
DeclarationIndex::collect(&ProjectSources, usage) -> DeclarationFacts
DeclarationFacts::freeze_imports_and_domains(schedule) -> Result<DeclarationIndex, Diagnostics>
DeclarationIndex {
    global function/record -> original AST handle + ModuleId + name/visibility span,
    per-module original type/path and value bindings,
    per-module immutable direct-import bindings,
    field identities/permissions,
    Public/Subtree domain per declaration,
    root_original_main: Option<DefId>
}
lookup_type(requester, UnqualifiedOrAbsolutePath) -> nominal RecordId or scalar
lookup_callee(requester, UnqualifiedOrAbsolutePath) -> DefId
resolve_import(requester, AbsolutePath) -> atomic original type/value target pair
check_field_access(requester, RecordId, FieldId, use_span) -> Result
check_signature_exposure(DefId, nominal_type_occurrences) -> Result

check_project(&ProjectSources, &DeclarationIndex) -> CheckedSourceProgram
resolve_scalar_project(...) -> existing scalar HIR with global DefIds
resolve_owned_project(...) -> existing owned HIR with global DefIds/RecordIds
```

The builder owns Strings/ASTs during loading; no borrowed name views escape it.
Freeze the entire source set before constructing an index borrowing source text.
No self-referential struct with references into a moving builder is required:
own sources in the outer driver scope and borrow them in index/resolved programs.
Index keys should be spans/IDs or borrowed source slices, not cloned full names
per lookup. Interning, if chosen, must count its exact retained bytes and retain
the same original declaration identity; it cannot redefine equality/ID order.

AST expression IDs are currently file-local and block IDs function-local.
Pass the owning file/function explicitly with every AST access. Existing body
resolvers can operate one file/function view at a time with shared global index
access. Do not concatenate AST vectors without a separately reviewed complete
remapping; that unnecessary migration is excluded from this plan. Resolved
owned program must borrow SourceMap/ProjectSources, replacing its lone
source_text string; field name comparisons in type checking need this too.
Thread the original function/requester ModuleId and index into owned type
checking, where a field's RecordId becomes known. Privacy must also cover an
inferred `let value = factory(); value.private_field` or assignment, not only
constructor/type names visible during name resolution. For struct literals,
the RecordId is already known in name resolution: query all declared field
domains immediately there, before initializer field lookup, duplicate checks or
expression resolution. Report construction privacy as E0206/resolve; projection
privacy after binding-type discovery is E0206/type. Both use the same shared
field-domain predicate and original requester ModuleId. AST paths retain an
ordered vector/range of segment spans plus full path origin; never compare a
full path's raw text when trivia can occur between its segments.

One shared declaration/import/path/privacy implementation is authoritative for
both frontend routes. Both call the same lookup APIs and receive identical
namespace targets and privacy decisions. Retain separate existing scalar/owned
expression typing, lowering and witnesses. Project selection runs once, over
every parsed file, before either producer starts. Any struct declaration,
nominal annotation, reference parameter, constructor/field use or borrow
argument selects owned for the whole project, exactly extending the predecessor
predicate. A qualified function call, mod/use/pub alone does not select owned.
An unused struct in an unused loaded module still does. Unknown nominal-looking
types continue to choose the owned route and then receive the existing type
error. No per-file fallback or universal scalar-to-owned migration is allowed.

The index supplies identities/access facts, not an ownership proof. Producers
still construct one complete raw program. Existing authoritative declaration,
shape/CFG/ownership verifiers and consumer plan admission remain required.
Module metadata is erased before execution except diagnostic origins and
numeric resolved identities. It creates no runtime dispatch table, import
operation, initialization block, extra entry call or fuel charge.

## 6. Diagnostic identities, precedence and compatibility

### 6.1 Compatibility mode

OriginalSingleFile means the parsed file uses none of the newly supported
mod/use/pub/absolute-path constructs. Ordinary invalid syntax must keep its
predecessor lexer/parser diagnostic and recovery behavior; recognizing a new
construct is the only intended migration boundary. On OriginalSingleFile,
preserve exact accepted behavior, route, IDs, runtime costs, diagnostic code,
stage, count/order, primary/secondary spans and rendering. New metadata bounds
must be proved redundant for this admitted envelope before activation.

Public dispatch calls `load_typed` once. The parser's sticky recognition bit is
set only at actual new-grammar entry sites, never by source prescanning, trial
parsing or a synchronizer inspecting tokens. It does not reset after malformed
recognized syntax. A failed parse returns retained sources and diagnostics, not
a fabricated successful syntax flavor. Successful AST metadata remains the
authoritative OriginalSingleFile/ProjectSyntax selection.

Module/import node admission precedes keyword consumption. Top-level `pub fn`
and `pub struct` recognition follows pub consumption before the function/record
node gate; `pub mod` recognition follows its module node gate. Field admission
precedes public-field lookahead. Adjacent contextual colons recognize project
syntax before unsupported-prefix/path-context denials; ordinary punctuation and
nonadjacent colons do not. Forward field-prefix lookahead traverses disjoint
logical ranges and charges a terminal EOF to the initiating pub. The separately
bound parser observer qualifies these admission/recovery and resource boundaries.

There is a real predecessor ordering difference to preserve: scalar
`hir::resolve` collects duplicate-name and signature diagnostics interleaved
per function; owned resolve completes declaration conflicts before fields and
signatures. A shared collector must retain ordered declaration facts instead of
eagerly throwing an error that reorders the scalar sequence. Compatibility
adapters may replay those shared facts under the two historical schedules;
they must not duplicate namespace, imports or visibility rules. No project
syntax exists in this compatibility mode, so its visibility decisions are
trivial. Existing finite compatibility corpora are necessary but not sufficient;
collision-plus-bad-signature controls must protect the ordering difference.

### 6.2 ProjectSyntax phase order

For any newly recognized project construct, use this deterministic schedule:

1. Existing CLI validation before filesystem access. Entry read byte cap, OXBC
   rejection, UTF-8 validation, lex and parse keep their existing order.
2. DFS load. At each next mod declaration: checked count/depth/path arithmetic
   and caps; same-parent exact duplicate (E0201) or case-fold collision; exact
   component/file-kind/symlink checks; canonical containment/repeated-path
   checks; open/read remaining aggregate bytes; OXBC; UTF-8; lex; parse; then
   recurse. Stop at the first loading error or first file's parse error vector
   (up to existing 100 cap). A child error therefore precedes an unvisited
   sibling error. Root parse errors precede all child reads. Missing unused
   modules still fail. Distinct function/struct conflicts are resolved only
   after loading, so an earlier load failure can hide them.
3. Aggregate existing declaration-count admission, then new index preflight
   counts/payload/work bound, before allocating index output/sort scratch.
4. Original declaration conflicts in module-major lexical item order; report at
   most 100. E0201 primary is the later declaration name, secondary the earlier
   conflicting name. Module/module collisions already found while loading are
   not duplicated. For module/struct conflicts, lexical order chooses later.
   Scalar type-name rebinding is E0202. Stop before imports if any error.
5. Imports in module-major lexical order, each using the steps in §3.2. At most
   one diagnostic per import and 100 overall. Stop before signatures if any.
6. Resolve all record field declarations in global RecordId order, then all
   function parameter/result types and local explicit annotations in global
   DefId order, preserving current per-declaration traversal. Existing
   duplicate-field/type errors keep current codes and origins. Collect at most
   one failure per record/function and 100 overall; stop if any error.
7. Check signature exposure in global DefId, parameter order then result. Emit
   the first offending nominal occurrence per function, maximum 100. Stop if
   any exposure error was emitted.
8. Resolve function bodies in global DefId order using existing within-body
   traversal; type-check only after complete resolution succeeds. Existing
   owned field validation integrates the privacy step described below. Preserve
   current per-function failure and global 100 cap. Then lower, raw verify,
   consumer/entry/native admission, tool invocation/output or reference run,
   in their existing order. There is no early root-main error that bypasses
   an invalid declared child.

At a path segment: missing/wrong namespace before attempting access; if the
segment exists but is inaccessible, report that first inaccessible segment and
do not inspect the rest. Endpoint function/type mismatch preserves the ordinary
unqualified E0200/E0202; missing intermediate modules use E0205. Existing
unknown-field E0305 precedes field privacy for a nonexistent projected field.
For a projection read, check base type and known field, then privacy, before
authorizing the read. Field assignment preserves the predecessor's RHS-first
type-check schedule: after complete name resolution, type the RHS first, then
resolve base/field, report an unknown field if applicable, check privacy, check
target mutability and finally assignment type compatibility. Privacy is checked
before authorizing the resolved write, not before type-checking its RHS. Thus a
bad RHS type expression wins over an inaccessible target field; a valid RHS with
the wrong assignment type loses to field privacy. For a struct
literal, type/path resolution and type accessibility first; then check access
to every declared field in declaration order before initializer completeness,
duplicate/unknown initializer fields or value typing. Primary privacy origin is
the first lexical matching initializer field name if present (including when
duplicates exist), otherwise constructor
type-path span. Ordinary one-file behavior is unchanged because these fields
are accessible there. Privacy diagnostics stop before ownership/lowering.

### 6.3 New errors and origins

| Code/stage | Meaning | Primary; secondary |
| --- | --- | --- |
| E0005/source | Module path policy: case collision/mismatch, symlink/nonregular component, escape, repeated canonical pathname, unsupported host proof | Declaring mod name; earlier mod/entry origin for repeated identity or case collision when available |
| E0002/source | Requested module missing/unreadable or metadata I/O error | Declaring mod name; no invented child span |
| E0003/source, E0004/source | Child invalid UTF-8 or OXBC | Declaring mod name; entry forms retain old null primary |
| E0205/resolve | Missing/wrong-kind intermediate module, import lacking original function/struct endpoint, or import-through-alias/module target | Offending segment, or endpoint; alias declaration secondary when it explains rejection |
| E0206/resolve | Inaccessible module/function/type, including import | First inaccessible segment; original declaration name |
| E0206/resolve | Construction requires access to an inaccessible declared field | First lexical matching initializer field name if present, otherwise constructor path; original field name |
| E0206/type | Inaccessible known field projection/read/write | Field use; original field name |
| E0207/resolve | Signature exposes a less-visible nominal type | Nominal type occurrence in parameter/result; type declaration and, if different, restrictive ancestor module declaration (up to two labels) |
| E0400/source-project | New module/depth/path/probe cap, checked arithmetic failure or handled reserve failure during loading | Current mod name; real root EOF for root-only errors after source retention; null before any source is retained |
| E0400/parse | Qualified path exceeds 34 segments, including crate and endpoint | First over-limit segment; no secondary |
| E0400/resolve-project | New index payload/scratch/work admission or handled allocation failure | Current item/path use; root EOF for whole-index admission |

E0002 filesystem message text may include the host OS cause, as today; code,
stage and specified origins are the portable contract. New policy diagnostics
use bounded messages, ≤1,024 bytes plus two ≤256-byte labels and two ≤256-byte
notes, like existing owned builders. Keep schema_version 1, summary shapes,
ordinary exit 1 and internal E0500 exit 2. Check summary function count is total
original functions across all loaded modules. Never count imports as functions.

At public activation, predecessor diagnostic results intentionally change
for `mod state;`, `use crate::state::Batch;`, `pub fn main` and `crate::value()`
because they gain grammar and module/name/privacy outcomes. The historical Unit1 corpus preserves
the old rejection of all four forms; current qualification records them as
explicit migrations and retains the other 31 original cases unchanged. It must stay identical for accepted
functions/locals named crate/as, the struct+function crate pairing and the
unsupported cast. Publish these deliberate diagnostic migrations explicitly.

## 7. Resource contract and qualification gates

### 7.1 Preserved aggregate limits

Use one project ledger for source bytes (1,048,576), non-EOF tokens including
trivia (100,000) and counted syntax nodes (100,000). These are not per-file
allowances. EOF is one per file but excluded from token count; include its
actual storage in source/AST inventory. Each token stays ≤65,536 bytes; expression
and block nesting remain 64, params/args 256, diagnostics 100. Parser node charges
for existing syntax are unchanged. Charge one additional node for each module
declaration, one per import, and one per segment of a qualified path including
crate. Unqualified names retain their old charge, and a pub flag adds no node.
Each new node/segment is charged before its AST entry is appended. Qualified
paths have an independent 34-segment cap. At each segment, first checked-add
the path count (overflow E0400/parse), then reject a count above 34 at that
segment's span, then charge the aggregate syntax node using the existing parser
resource diagnostic, then append. This freezes the winner when the segment and
node caps would both fail. The check includes the initial contextual crate
segment and happens while parsing, before module loading/index allocation.

All existing scalar/owned declarations, source raw output, verifier metadata,
analysis work, reference activation/fuel/storage and native admission ceilings
remain aggregate over the complete linked project. In particular retain 4,096
records, 1,024 fields per record/65,536 total, 8 MiB declaration payload/1 MiB
layout sum, 100,000 combined slots and statements/events, 300,000 blocks,
100,000,000 ownership work, 32 MiB ownership metadata/flow scratch and 64 MiB
source-produced raw payload. Do not reset a ledger at file boundaries.

### 7.2 Loader and shared index caps

The shared loader implements the inclusive caps below. Qualified-path admission
and shared-index I/J/W accounting are implemented by the parser and index.
These are admission ceilings, not performance claims. Changes require a reviewed
revision before activation, with boundary fixtures and the one-file redundancy
proof updated. The [Unit1 ledger](../docs/architecture/typed-project-unit1-validation.md)
records measured source/loader layouts and distinguishes reachable boundaries
from lowered-limit controls.

| Counter | Inclusive starting cap | Exact counted quantity |
| --- | ---: | --- |
| Modules M | 256 including root | One admitted logical module/file, even empty |
| Module depth D | 32 edges below root | Root=0; check child depth before probe/read |
| Qualified path segments Q | 34 including crate and endpoint | Root + ≤32 module components + endpoint; applies only to new paths |
| One module component | 255 bytes | ASCII name bytes; expected final filename adds 3 bytes, subject to host filename limit |
| One logical relative file path | 4,096 bytes | ASCII components + slash separators + `.ox` |
| Retained loader path payload P | 4 MiB | Exact byte/code-unit storage of owned display, canonical and probe-key paths, including root copies, each retained copy once |
| Directory probes V | 8,448 | Each component verification; 256×33 is conservative |
| Directory entries scanned E | 100,000 total | Every yielded entry, including unrelated metadata, across all probes |
| Directory entry name units U | 16 MiB total | Actual native OsStr representation units inspected; bytes on Unix, UTF-16 code units×2 on Windows |
| Shared index requested payload I | 32 MiB | Exact lengths×size_of of every new table and nested retained index allocation, plus copied/interned bytes, excluding sources/AST/HIR |
| Index peak scratch J | 16 MiB | Maximum simultaneously live new sort/traversal/import staging table lengths×size_of; disjoint lifetimes use max |
| Namespace work W | 256,000,000 units | Definition below; no per-file reset |

Module component cap is conservative source admission, not a guarantee the
host can create that filename. A 253-byte component plus `.ox` may fail with
ordinary E0002 on a 255-byte filename host; qualify actual host limits separately.
Single-file sources receive no new component/path/canonicalization requirement.
For a multi-file entry, apply retained-path cap before copying root paths; OS
canonicalization's own transient allocation is explicitly outside requested
frontend payload accounting.

Unit1 measures SourceMap headers, source strings, line-start vectors (one 0
plus one entry per LF), tokens including M EOFs, AST headers/nested vectors,
module headers, canonical/display paths and traversal scratch on the qualified
64-bit host. Unit2 must extend the inventory to concrete import paths, original
binding indexes, aliases, per-kind ID maps, domains and sorting/import scratch.
AST/source memory remains separate from I and is derived from its existing
count/byte bounds. No unproven catch-all cap may reject accepted original
one-file programs.

Before accepting the proposed I/J/W ceilings, prove that all OriginalSingleFile
inputs admitted under predecessor source/token/node limits fit the new shared
index admission. Use a compact representation with at most one original binding
slot per existing declaration node, per-kind mapping entries, and borrowed/span
keys. Derive actual size formulas, not a survey of a few small programs. If the
formula does not fit, change the representation or revise the proposed cap;
do not quietly weaken the compatibility promise or bypass the shared index.
The starting 32/16 MiB leaves room for compact ≤100,000-entry tables; this is a
design expectation until actual type sizes and reachable counts are measured.

### 7.3 Work definition, preflight and failure order

W is a checked, reproducible frontend work ledger: charge 1 for each original
declaration/import/path segment visited during index collection, each sorted
binding comparison, each byte compared within a spelling comparison (including
the first unequal byte), each binary-search probe, each module-parent/domain
edge tested, and each target/field permission lookup. Charge length mismatch
comparison as one comparison plus inspected bytes, without charging nonexistent
bytes. Count each visit each time, including failed lookups and count passes;
do not charge unrelated lexer/typechecker/IR work under this new name.
Use stable iterative mergesort/binary search or another independently bounded
implementation; a randomized hash table alone is not a worst-case work proof.
Maximum path walk is Q and ancestry depth D. A preflight upper bound for sort
scratch/work must use checked actual binding counts and total key lengths;
lookup operation charges enforce the remaining work at each access. Work is
neither instructions nor wall time. Directory scanning has its separate E/U/V
ledger because external directory size is not bounded by source bytes.

No large allocation is permitted before the counts admitting that allocation
are known. This is a **per-layer preflight**, not a claim that the entire compiler
allocates nothing until every input is known:

- During loading, preflight a child module/header, depth and joined path length
  before joining/copying or probing. Use fixed bounded read scratch; admit each
  retained source chunk against aggregate source bytes before growing its
  buffer. The one over-limit sentinel byte remains scratch. Reject excess bytes
  before UTF-8/lex, exactly as the entry route does. Preflight line-start storage
  from the loaded bytes before constructing the SourceFile. Token/node appends
  use remaining aggregate counters and checked arithmetic
- Once ASTs are immutable, a count-only traversal determines exact new index
  table lengths, nested bytes and peak scratch before constructing/sorting
  index output. Fixed bounded traversal stacks are allowed; the count pass must
  not allocate the very maps/vectors it claims to preflight. Expose counted
  usage internally for independent lowered-limit tests
- Use checked add/mul/conversion for all offsets/counts/byte products and
  fallible reserve on every new variable allocation. Ensure emitted/appended
  counts match the admitted counts in release builds, without hidden growth.
  Sorting and paired imports must include their scratch/temporary pairs.
  No cloned whole module scopes or dense requester×declaration matrices

Where simultaneous failures are possible, first checked arithmetic (overflow
is E0400 in the relevant new project stage), then loader dimensional caps in
order (M,D,component,relative path,P,V,E,U), then metadata/file errors. Q is a
parser admission with its earlier node ordering specified above, not a loader
check. E/U exhaustion shares the single directory-scan diagnostic; index
admission uses existing owned declaration counts first, then I,J,W, then
allocation. A source read checks byte cap before encoding; lexer checks token
length then aggregate count; parser charges nodes before allocating the new
node. At an actual namespace operation W exhaustion precedes that operation's
semantic error, with its path/use origin. Allocation failure is reported only
when a fallible new allocation reports it; invariant/count mismatches remain
E0500 rather than being misreported as user resource use.

The measured payload ledger does not bound Vec capacity, allocator buckets,
fragmentation, Rust/OS internal allocations, diagnostics rendering, total RSS,
LLVM memory, or allocation success. Do not promise total OOM recovery. Existing
ordinary frontend allocations remain under their existing narrower guarantees;
new controlled allocations must satisfy the fallible/preflight contract. No
part of this resource plan is a filesystem sandbox or an ownership soundness
proof.

## 8. Semantic cases

### 8.1 Positive opaque owned API

`main.ox`:

```text
mod state;
use crate::state::Counter;
use crate::state::make;
use crate::state::bump as increment;
use crate::state::finish;
fn main() -> i32 {
    let mut value: Counter = make(4);
    increment(&mut value);
    return finish(value);
}
```

`state.ox`:

```text
pub struct Counter { value: i32 }
pub fn make(value: i32) -> Counter { return Counter { value: value }; }
pub fn bump(value: &mut Counter) -> () { value.value = value.value + 1; return; }
pub fn finish(value: Counter) -> i32 { return value.value; }
```

After public activation, expected check success and reference/native result 5. Root's private state module
is accessible throughout the crate because it is declared in root. Counter and
API functions therefore have effective Subtree(root), while value remains
Subtree(state). The function parameters named value do not shadow module
functions; field spelling does not enter value namespace.

### 8.2 Exposure counterexamples

Root `struct Secret {} pub fn reveal() -> Secret { return Secret {}; }` must
fail E0207 even with no other modules/callers, due to abstract External.

Root `mod hidden;` plus hidden `struct Secret {} pub fn reveal() -> Secret ...`
also fails: reveal is accessible to the whole crate, while Secret is limited to
hidden. Changing Secret to pub permits the signature; hidden's enclosing
privacy then restricts both to Subtree(root).

Root `mod outer;`, outer `mod hidden;`, hidden `pub struct Secret {}`: a pub
function in outer returning hidden::Secret through an absolute path fails if
the function's domain is Subtree(root) but the type's is Subtree(outer).
A private function in outer returning it passes: both domains are
Subtree(outer). Signature inclusion is not merely a same-module/public-bit test.

### 8.3 Finite acceptance/rejection matrix

| Case | Expected result |
| --- | --- |
| Two modules each declare `pub struct Item { x: i32 }` | Distinct RecordIds; passing a::Item to b::Item is existing type mismatch |
| Struct and function X in same module; `use crate::m::X as Y` | Y resolves to original type in annotations/construction, original function in calls |
| One of those X declarations is private to m, requester outside m | Entire paired import fails E0206; no partial Y binding |
| Struct-only import alias meets existing same-name function | Accepted; separate namespaces |
| Module and struct with same spelling in one module | E0201, later name primary, earlier declaration secondary |
| Import alias before later same-namespace original declaration | E0201 at import alias, original declaration secondary; originals preinstalled |
| Identical target imported twice, even under different aliases | E0201 at second alias/name; first import secondary; failed paired import commits no bindings/target-seen entries |
| Import endpoint names another module's import alias | E0205; no alias chase |
| Child calls parent's private function by `crate::...` | Accepted if ancestor-module permissions allow it |
| Child uses parent's imported alias unqualified | E0200/E0202; imports not inherited |
| Sibling accesses private function/type declared in other sibling | E0206 |
| Root projects private field of a public child struct | E0206/type, cross-file field declaration label |
| Root constructs that struct, including by omitting the private field | E0206/resolve before unknown/duplicate/missing initializer or value errors |
| Child of defining module accesses that private field | Accepted with accessible type/path |
| Public empty struct outside its module | Construction accepted; no hidden field exists |
| Root imports child main as main, no original root main | Check may pass; run/compile fail their current entry requirement |
| Valid root plus syntax/type/move error in unused module | Whole project fails before execution/output |
| Scalar root plus unused owned declaration in child | Entire project uses owned route |
| Cross-file call borrows owner twice incompatibly | Existing call alias/ownership diagnostic with correct file-tagged origins |
| Return owned value through imported helper, then reuse moved local | Existing use-after-move rejection |
| Cross-file mutual recursion | Check succeeds if otherwise valid; reference bounded; native rejects entire call graph |
| Two modules directly import each other's original public functions | Accepted; only alias chasing is excluded; native recursion rules concern actual calls |
| Additional child main with arbitrary parameters | Ordinary checked function; never changes entry selection |
| `mod a;` with only `a/mod.ox` | E0002 for expected a.ox; no alternate search |
| `mod A; mod a;` | E0005 case collision at second encountered module declaration |
| Child symlink, symlinked component or repeated canonical file | E0005; distinct hardlinks explicitly remain distinct modules |
| Same start/end offsets straddling UTF-8 differently in two files | Correct owning SourceFileId chosen; no cross-file slicing/panic |

## 9. Batch pilots and equivalence controls

The existing [single-file Batch](../fixtures/owned_source/batch.ox) and its
independent arithmetic result 816 remain historical input. Neither project
pilot below is implemented or qualified by Unit1. Later work must produce two
separately identified project controls.

1. **Mechanical split:** move Batch and existing functions into root/state/jobs
   files, changing only visibility, imports and qualifying name occurrences.
   Make fields public wherever needed to preserve every existing function body
   and call graph. Do not add helpers. Bind an explicit relocation map from
   original declarations/spans to split declarations/spans. Expected result,
   effect order, reference fuel and native budget outcomes equal the original
   source pilot after mapping declaration identities/origins. Numeric DefIds
   may differ with module-major ordering and are compared through that map.
   File paths and native bytes need not be identical. Module/import syntax adds
   zero executable operations and zero fuel
2. **Opaque API:** state owns public Batch with private completed/retries/
   checksum/active fields; exports make, retry, commit, done, next_job, is_active,
   stop, relay and finish as needed. jobs imports Batch/commit and exposes
   dispatch with the existing mutable-parameter reborrow. Root uses these APIs
   for the same six-job/two-attempt retry loop and moves Batch through relay to
   finish. Expected arithmetic remains 816. New constructor/query/update calls
   change lowering and fuel; independently calculate and record their actual
   operation schedule and total. Do not copy the mechanical split's fuel

Before project support is published, both pilots must pass the production
check/run/compile command paths and real standalone native ELF execution after
source files are unavailable, in debug/release compiler profiles and the
qualified LLVM 19.1.7/O0 Linux x86_64 target. Record exact compiler/source-set/
toolchain hashes and exit/stdout/stderr. Include budget-at-success and one-below
controls plus selected intermediate failures with mapped origins. The privacy
helper pilot's extra fuel is separately justified, never charged to modules.

These proposed pilots are intended to demonstrate reusable ownership APIs and
implementation hiding. Their scope excludes a scheduler, filesystem I/O,
production background jobs, general failures/results and a production CLI runtime.

## 10. Independent qualification required for activation

Production self-tests alone are insufficient. The following independent models
and held-out cases must not derive expected answers by calling the production
resolver, reading emitted expected-value files or translating compiler outputs
back into expected results. This table states required observations, not completed
Unit1 results.

| Oracle | Independent representation and finite scope | Required actual observation |
| --- | --- | --- |
| Namespace/visibility | Small rooted trees (root + ≤3 modules), explicit original declaration lists, two namespace maps, requester set containing External and symbolic domains; enumerate pub/private combinations, paired endpoints and collisions | Materialize real .ox projects; public check JSON targets where observable, otherwise calls returning distinct constants, nominal mismatch failures, exact code/stage/file/primary/secondary tuples |
| Source map/identity | Byte arrays independently decoded for LF/CRLF and UTF-8 scalar columns, explicit file ordinal and relocation maps; equal-offset different bytes/Unicode-boundary cases | Public diagnostic JSON and human text; reference runtime overflow/loan origins and source-free native failure messages; reject stale file-only offset extraction |
| Path/loading | Stable temporary directory fixtures with independently declared logical→expected path manifest; platform-capability-labeled symlink/hardlink/case fixtures | Public check/run/compile, exact loaded set via filesystem observation or independent fixture sentinels, no unexpected file/output reads/writes; module error source locations; test invocation entry symlink directory rule |
| Resource | Closed-form count/payload formulas plus independent generated projects and manually inventoried reduced-cap cases; count+one-over, arithmetic overflow and allocation-failure injection | Public CLI at reachable source limits; internal lowered-limit seams for impractical/non-source-reachable maxima, with actual reserve traces proving rejected layer allocates no large output; separately identify seam-only evidence |
| Ownership/consumer composition | Existing independent source operational model extended to explicit files/modules; new namespaces represented independently, original ownership semantics reused | Whole-project check/reference/native differential evidence, cross-file move/borrow/return/unused-function failures; no per-file fallback |
| Split equivalence | Independent source relocation and target mapping, retained predecessor source pilot arithmetic/fuel model | Real public run and native binaries for original/mechanical/opaque cases; observe results, effect order, budgets and mapped failure origins |

The namespace enumeration must distinguish permission from unqualified scope,
root-private from public, inaccessible ancestor vs accessible endpoint, private
type exposure with no loaded external callers, private fields of public structs,
and one inaccessible half of a paired import. Add long same-prefix names and
many-empty-module resources; neither is represented by the Batch workload.

Adversarial error-precedence projects combine two independent failures: bad root
parse/missing child; child lex/next sibling missing; original collision/bad
import; import privacy/alias collision; invalid signature/exposure; unknown
field/private known field; erroneous RHS expression/inaccessible field-write
target; native recursion/output collision. The RHS/privacy pair must separately
cover an invalid RHS expression (RHS error wins) and a well-typed RHS of the
wrong assignment type (privacy wins). Each expected
winner follows §6, not whatever current output happens to be. Qualify one-file
diagnostic precedence separately against frozen predecessor bytes.

Historical ownership and compatibility counts are not project qualification.
New manifests must identify each actual CLI/native invocation, profile, input bytes,
entry path, module identity/path graph, command argv, expected/actual result,
diagnostics, compiler hash, platform/toolchain and source-free artifact step.
Skipped or seam-only conditions remain explicitly labeled. No new native host
is qualified by frontend tests on that host.

## 11. Implementation stages and activation gates

1. **Unit1: source set and identity.** Own a frozen SourceMap and per-file ASTs;
   expose checked file/function/expression/block handles; use the facade in the
   existing public one-file driver. Privately load the declared module tree with
   aggregate source/token/node accounting and measured source/path storage.
   Preserve existing public rejection and diagnostic precedence. Unit1 does not
   allocate a shared declaration index or claim its I/J/W redundancy proof.
2. **Unit2: declarations and lookup.** Implement original declarations, global
   module-major IDs, direct atomic paired imports, contextual paths, effective
   domains, privacy and signature exposure through one shared index. Adapt both
   scalar and owned resolution without replacing scalar OIR. Measure actual
   index and scratch layouts, count/work formulas and fallible allocations;
   prove full original-one-file redundancy before accepting I/J/W admission.
3. **Unit3: linked execution and pilots.** Produce one raw program and retain
   every existing verifier/consumer admission. Establish cross-file diagnostics,
   move/borrow composition, whole-program native rejection and no-clobber behavior.
   Run the separate mechanical and opaque Batch controls with independently
   justified costs and actual source-free native execution.
4. **Unit4: qualification and public activation.** Freeze the candidate and
   independently exercise namespaces, origins, filesystem policy, resources and
   both consumers. Rerun applicable predecessor Rust, source, raw-verifier,
   reference/native, privacy and repository gates. Activate only when both routes,
   real CLI/native evidence, specifications, RFC, inventory and explicit diagnostic
   migrations agree. Applicable hosted CI must pass on the exact publication head;
   a missing or pending check does not establish success.

All stages preserve the verified-witness boundary. File identity and namespace
lookup provide facts to the existing ownership analysis; neither is an ownership
soundness proof. Finite namespace models and a successful pilot do not replace
verifier qualification.

## 12. Compatibility, versions and support

This RFC does not change the default legacy interpreter/module loader or
`--edition legacy-0.9`. No change is made to the toolchain release, OXBC 1.0,
serialized AST version 1, lockfile version or public/native ABI. The
capability remains within the explicit experimental `typed-preview` selector;
it does not establish a stable language edition or Rust compatibility.

| Surface | Current support and qualification boundary |
| --- | --- |
| Public original one-file frontend/reference path | Preserves existing hosts and behavior; this work adds no host claim |
| Private Unit1 multi-file source discovery | Linux x86_64 stable-filesystem qualification only |
| Private discovery on non-Linux hosts | E0005/source policy rejection when module discovery is needed; a module-free root keeps the ordinary one-file path |
| Private discovery on other Linux architectures | Not qualified by current execution evidence; the implementation gates the OS, not the architecture |
| Public typed multi-file frontend/reference path | Unit4 production dispatch; exact current host evidence is recorded in its ledger |
| Existing native target | Linux x86_64, LLVM 19.1.7, O0, within existing admission limits |
| New native targets, separate compilation or stable ABI | Out of scope |

The loader does not promise an adversarial-filesystem sandbox, atomic project
snapshot, content authenticity or an OS-blocking bound. The requested-storage
ledger does not bound total RSS, allocator overhead, LLVM memory or allocation
success. A handled reserve error is not a claim of total OOM recovery.

Historical ownership manifests, source/native receipts and their hashes remain
bound to their original inputs and binaries. The current Unit1 evidence is a
separate body of work; it must not retarget historical receipt identities or
present their counts as new project observations.

## 13. Alternatives and remaining work

- Source concatenation or flattened AST vectors would require complete offset
  and local-ID remapping. Frozen per-file owners and explicit handles retain
  diagnostic identity with a smaller migration surface
- Reusing the dynamic legacy loader would import execution and initialization
  semantics that this declaration-only typed proposal excludes
- Import alias chasing, reexports and module aliases would require a broader
  resolution and cycle policy. Original-only endpoints allow mutual direct
  imports without an alias fixed point
- Caller-enumeration privacy would miss exposure to future callers. Symbolic
  Public/Subtree domains, including External, define exposure independently of
  currently loaded uses
- A universal scalar-to-owned conversion would change established costs and
  admission. One shared index serves the existing two producers instead
- Filesystem-wide discovery or inode identity would broaden the file/identity
  contract. Explicit declarations, canonical pathnames and distinct hardlink
  identities keep it bounded and deterministic on a stable filesystem

The semantic choices in this RFC are concrete. Qualification remains tied to
exact source, compiler and target identities. Broader filesystem hosts, separate
compilation, general package imports and stable ABI remain future work. A material semantic or cap change
requires a corresponding reviewed RFC revision and updated boundary evidence.
