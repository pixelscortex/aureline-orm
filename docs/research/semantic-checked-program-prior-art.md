# Post-analysis program representation — compiler prior art

> **Provenance — 2026-08-18.** Primary sources only: official compiler guides and
> source at rustc `8fa1c96`, TypeScript `b465fdb`, Roslyn `164ff4b`, and Prisma
> Engines `561d7b4`. This is a research finding, not a specification.

## Question and verdict

For the table slice in [map #27](https://github.com/pixelscortex/aureline-orm/issues/27), should
`CheckedProgram` be (a) a proof wrapper over the arena AST and analysis side tables, (b) a copied
semantic graph optimized for downstream queries, or (c) a hybrid?

**Use the hybrid, but keep it much closer to (a) than (b).** The canonical declaration store should
remain the arena AST. Analysis should add only information syntax does not contain: duplicate-aware
name buckets, resolved table identities, normalized `SemanticType`s, and Findings. A successful
conversion should produce a distinct `CheckedProgram` type whose public queries cannot return
missing, ambiguous, unknown, or invalid outcomes. Internally, it should reuse/move the arena and
valid semantic tables rather than clone every table and field into a second graph.

Migration and TypeScript emission should not dictate the storage of `CheckedProgram`. If either
consumer needs a substantially different shape, lower the checked model into a small
consumer-specific artifact at that boundary. This follows [ADR-0002](../adr/0002-four-rules-against-compiler-sprawl.md):
syntax, semantic model, and generation artifact have distinct roles, while infrastructure is added
only when a feature needs it.

## What the compilers actually do

| Compiler | Source/declaration identity | Semantic representation | What generation consumes | Relevant pressure |
|---|---|---|---|---|
| rustc | AST is lowered to HIR; `HirId` identifies HIR nodes relative to an owner | query results plus owner-local `TypeckResults` side tables | successively lowered THIR/MIR, then codegen IR | large batch compiler plus incremental compilation |
| TypeScript | syntax `Node`s, binder-created `Symbol`s, numeric node/symbol IDs | lazy `TypeChecker` caches in `NodeLinks`/`SymbolLinks` | transformed syntax queried through a narrow `EmitResolver` | editor latency and incremental reuse; emit-with-errors is supported |
| Roslyn | immutable full-fidelity syntax trees and symbols | immutable `Compilation` plus per-tree `SemanticModel`; internal typed bound trees | lowered bound trees for IL emission | IDE snapshots, speculative queries, incremental edits |
| Prisma PSL/query compiler | indexed AST IDs inside `ParserDatabase` | AST plus names/types/relations side tables, exposed through ID-backed walkers | query structure wraps the validated schema; `QuerySchema` adds query-specific trees and maps | schema validation and query planning, closest to Aureline |

### rustc: side tables for facts, separate IR when operations change

rustc does not try to make one representation serve parsing, type checking, flow analysis, and
codegen. It lowers AST to HIR, gives HIR nodes `HirId`s, and uses more stable `DefId`s for item-like
definitions; the compiler guide explicitly notes that there is not a `DefId` for every expression
and that `DefId`s are consequently more stable across compilations
([HIR guide](https://rustc-dev-guide.rust-lang.org/hir.html)). Type checking records results such as
node types and type-dependent definitions in owner-local maps inside
[`TypeckResults`](https://github.com/rust-lang/rust/blob/8fa1c96cfd489e4c27654c144ae871ce2c4db6c6/compiler/rustc_middle/src/ty/typeck_results.rs),
rather than cloning a fully typed HIR graph.

When a consumer needs a different operational form, rustc *does* lower: THIR is a more desugared,
typed representation used to construct MIR, and MIR is deliberately simplified for borrow
checking, optimization, and code generation
([compiler overview](https://rustc-dev-guide.rust-lang.org/overview.html),
[MIR guide](https://rustc-dev-guide.rust-lang.org/mir/index.html)). Codegen consumes optimized MIR
and lowers it again to a backend IR
([MIR-to-codegen lowering](https://rustc-dev-guide.rust-lang.org/backend/lowering-mir.html)). The
lesson is not “always build another IR”; it is “build one when the next operations need a different
language or data shape.”

rustc's demand-driven query system is also an incremental-compilation mechanism, not evidence that
a one-file schema compiler needs lazy memoisation. The overview describes codegen recursively
requesting `optimized_mir` and the preceding analysis queries
([compiler overview](https://rustc-dev-guide.rust-lang.org/overview.html)). Aureline's present table
slice needs every declared field for both migration and TypeScript output, so eager deterministic
resolution has a simpler cost model.

### TypeScript: a query facade over syntax and caches, not a checked final IR

TypeScript's binder walks syntax, creates `Symbol`s, and stores them in the containing scope's symbol
table; multiple declarations may contribute to one symbol
([binder notes](https://github.com/microsoft/TypeScript/wiki/Codebase-Compiler-Binder)). Its
`TypeChecker` answers semantic questions lazily rather than materializing a second typed syntax tree
([architectural overview](https://github.com/microsoft/TypeScript/wiki/Architectural-Overview/1afea54fbb7a4af15d613708ac0d1951f73aca14)).
In current source, semantic caches such as `resolvedType`, `resolvedSymbol`, and
`resolvedSignature` live in [`NodeLinks` and `SymbolLinks`](https://github.com/microsoft/TypeScript/blob/b465fdbfe175304d9b977da137b2c178ae1091d3/src/compiler/types.ts#L6058-L6300),
looked up by numeric IDs in the
[`TypeChecker`](https://github.com/microsoft/TypeScript/blob/b465fdbfe175304d9b977da137b2c178ae1091d3/src/compiler/checker.ts#L2932-L2942).

Emission does not receive a separately copied “typed program.” `Program.emit()` obtains a narrow
`EmitResolver` from the checker, ensures the requested file has the required semantic information,
and emits transformed syntax
([program source](https://github.com/microsoft/TypeScript/blob/b465fdbfe175304d9b977da137b2c178ae1091d3/src/compiler/program.ts#L2685-L2740),
[checker source](https://github.com/microsoft/TypeScript/blob/b465fdbfe175304d9b977da137b2c178ae1091d3/src/compiler/checker.ts#L2506-L2512)).
This is strong evidence for a cohesive query interface over canonical storage.

It is weak evidence for Aureline's generation gate: TypeScript intentionally emits despite semantic
errors unless `noEmitOnError` is enabled
([`handleNoEmitOptions`](https://github.com/microsoft/TypeScript/blob/b465fdbfe175304d9b977da137b2c178ae1091d3/src/compiler/program.ts#L5059-L5094)).
Aureline instead promises that generators never see recovery states, so it needs a stronger type
boundary than TypeScript's `Program`/`TypeChecker` pair.

### Roslyn: public semantic snapshots, internal typed lowering

Roslyn exposes separate object models for syntax trees, hierarchical symbols, binding through a
`SemanticModel`, and emission. A `Compilation` is an immutable snapshot, and a `SemanticModel`
answers symbol, type, flow, and diagnostic questions for one syntax tree
([official Roslyn overview](https://github.com/dotnet/roslyn/blob/164ff4b62b600c599ab2669f9dd4e8412651af16/docs/wiki/Roslyn-Overview.md)).
That public architecture resembles a semantic query facade over immutable source and declaration
state, rather than a duplicated public “typed AST.”

Internally, C# binding produces typed `BoundNode`s: the base node carries its source syntax and an
error flag, while bound expressions carry types
([`BoundNode`](https://github.com/dotnet/roslyn/blob/164ff4b62b600c599ab2669f9dd4e8412651af16/src/Compilers/CSharp/Portable/BoundTree/BoundNode.cs)).
Lowering and codegen operate on that bound representation—for example, the local rewriter and
stack optimizer are bound-tree rewriters, and the emitter dispatches over lowered bound statements
([lowering](https://github.com/dotnet/roslyn/tree/164ff4b62b600c599ab2669f9dd4e8412651af16/src/Compilers/CSharp/Portable/Lowering),
[codegen](https://github.com/dotnet/roslyn/tree/164ff4b62b600c599ab2669f9dd4e8412651af16/src/Compilers/CSharp/Portable/CodeGen)).
Roslyn therefore combines both patterns: reusable semantic queries for tools, and a distinct typed
IR where executable lowering warrants one.

Roslyn and TypeScript are optimized for a continuously changing editor workspace. Their immutable
snapshots, per-file models, speculative queries, and lazy checking are responses to IDE workloads
([Roslyn overview](https://github.com/dotnet/roslyn/blob/164ff4b62b600c599ab2669f9dd4e8412651af16/docs/wiki/Roslyn-Overview.md),
[TypeScript language-service goals](https://github.com/microsoft/TypeScript/wiki/Using-the-Language-Service-API)).
Aureline's current one-file batch compiler should copy the separation of roles, not their caching
machinery.

### Prisma: the closest analogue, including the useful compromise

Prisma's current schema frontend is unusually direct prior art. Its own module documentation says
the crate chain is schema AST → parser database → validation, with a separate structure optionally
lifted from a validated schema
([PSL README](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/psl/README.md)).
The AST uses opaque `u32` IDs backed by vectors: `ModelId` indexes a top-level AST vector and
`FieldId` indexes a model's field vector
([schema AST](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/psl/schema-ast/src/ast.rs),
[model AST](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/psl/schema-ast/src/ast/model.rs)).

`ParserDatabase` owns those ASTs alongside separate interned names, resolved types, and relations.
Its documentation is explicit that validation enriches information “without changing the AST,” may
produce diagnostics, and may leave resolved information incomplete
([parser database](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/psl/parser-database/src/lib.rs#L63-L143)).
Consumers do not coordinate its internal maps directly; typed, ID-backed `Walker` values provide
the query interface
([walkers](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/psl/parser-database/src/walkers.rs)).

Most importantly, Prisma recently avoids copying that schema into its query structure.
`InternalDataModel` is an `Arc<ValidatedSchema>` with convenient model, relation, and lookup methods
([source](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/query-compiler/query-structure/src/internal_data_model.rs));
`convert()` simply wraps the `Arc`
([source](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/query-compiler/query-structure/src/convert.rs)).
Only the genuinely query-specific `QuerySchema` adds operation trees and lookup maps
([source](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/query-compiler/schema/src/query_schema.rs#L18-L91)).
That is nearly the recommended Aureline split: reuse validated schema storage, then build a
consumer-specific structure only for consumer-specific operations.

Two Prisma details should *not* be copied. First, its parser database performs several AST walks
(names, types, attributes, then relations), contrary to Aureline's binding one-walk rule
([constructor](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/psl/parser-database/src/lib.rs#L101-L143)).
Second, `ValidatedSchema` can still contain error diagnostics, while only higher-level parsing APIs
return it on success
([public PSL API](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/psl/psl/src/lib.rs#L62-L133)).
Aureline can make the success proof structural and harder to bypass.

## Recommended Aureline shape

The useful separation is three stages, not one universal “final IR”:

```text
SemanticAnalysis (may be rejected)
  arena AST + duplicate-preserving resolution index
  + TypeResolution outcomes + Findings
                     │ no generation-blocking errors
                     ▼
CheckedProgram (valid-only semantic program)
  same/moved arena AST + resolved identity/type tables
  + cohesive table/field/reference query views
                     │ only where a consumer needs a different shape
             ┌───────┴────────┐
             ▼                ▼
MigrationModel        TypeScriptArtifact(s)
flat database state   row shapes/names/imports
```

A concrete Rust design could be:

```rust
pub enum AnalysisOutcome {
    Checked(CheckedProgram),
    Rejected(RejectedProgram),
}

pub struct CheckedProgram {
    ast: Ast,
    names: ResolutionIndex,
    field_types: IdVec<FieldId, SemanticType>,
}
```

`RejectedProgram` retains the arena AST, duplicate buckets, failed `TypeResolution` outcomes, and
Findings for reporting or future editor use. `CheckedProgram` contains only valid types and resolved
record identities. Converting success may move/compact semantic slots once; it must not re-walk
source syntax or clone declarations. Whether `Ast` is owned directly or behind `Arc` is an ownership
choice, not a semantic distinction; start with direct ownership unless two live consumers actually
need shared ownership.

Expose views rather than raw stores:

```rust
checked.tables()
checked.table(table_id)
checked.fields_of(table_id)
checked.field(field_id)
checked.resolve_table(name)       // unique by construction
checked.type_of_field(field_id)   // &SemanticType, never a recovery state
checked.references_to(table_id)   // may scan semantic facts initially
```

This interface preserves freedom to add a reverse-reference side index later without changing
callers. Do not build every imaginable adjacency map now. In the one-file table slice, scanning the
field/type table is likely cheaper in complexity than maintaining another invariant; if migration,
generation, or future query checking repeatedly needs incoming references, build
`TableId -> Vec<FieldId>` during the existing resolution pass and keep source-order vectors for
determinism.

Finally, do not make `CheckedProgram` “optimized for codegen” in the abstract. Migration comparison
wants normalized database entities and stable snapshot identity; TypeScript rendering wants row
shape, presence/nullability, target names, and imports. Those are different roles. Initially each
consumer can read `CheckedProgram` through its query API. Introduce `MigrationModel` or
`GeneratedQueryArtifact` only when that consumer needs normalization or repeated derived data, and
build it from semantic facts—not by walking syntax again.

## Decision consequence for issue #34

The arena remains worthwhile because it is Aureline's canonical `TableId`/`FieldId` store and gives
semantic side tables stable keys. The semantic index is not a second AST: it contains the mappings
the arena cannot provide (`name -> zero/one/many IDs`, resolved record targets, normalized field
types, and eventually any justified reverse edges). `CheckedProgram` should be a distinct,
generation-safe API and type, but not a duplicated schema graph. Specialized IRs belong after that
gate, at the migration or generation boundary where a concrete operation justifies them.
