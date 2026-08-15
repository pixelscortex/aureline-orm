# Aureline ARC1: durable architecture and domain lessons

## Scope and provenance

This report treats the cloned `pixelscortex/aureline` repository as a primary source and deliberately ignores transient grammar defects. The inspected revision is [`f8eac12b6d8a8c63e543682d418a1d4181f84413`](https://github.com/pixelscortex/aureline/tree/f8eac12b6d8a8c63e543682d418a1d4181f84413) (`Refactor semantic unknown attribute diagnostics (#120)`, 2026-05-26). Paths below are both local paths under `.repo/aureline/` — gitignored and local-only — and immutable GitHub links at that commit, which are the durable reference.

ARC1 is more mature as a schema compiler and migration frontend than as the proposed typed-query compiler. Its strongest reusable work is the separation of loss-aware syntax, semantic analysis, effect-based lowering, and a resolved schema consumed by SurrealQL emission. Query result inference and host-language runtime generation are not implemented.

## Headline findings

1. **Preserve two representations, not one mutable “AST.”** ARC1 keeps user-written attributes and source ranges in a raw AST, then produces a separate `ResolvedSchema` with structured indexes, permissions, normalized table names, and field flags. Tests explicitly require resolution not to mutate the raw AST. This is the best architectural seam to retain.
2. **Use a flat catalog with stable IDs between syntax and checking.** `SemanticCatalog` flattens nested syntax into table/field/analyzer/function/attribute entries, records ownership edges, and deliberately retains duplicate candidates so diagnostics are facts rather than accidental map overwrites.
3. **Keep analysis and lowering different kinds of work.** Read-only passes accumulate diagnostics; lowering emits typed `SemanticEffect`s; a builder applies those effects only after collection. This scales better than feature code mutating shared nodes while validating them.
4. **The embedded SurrealQL boundary is the central unsolved checker problem.** ARC1 delegates syntax to SurrealDB, but public SurrealDB AST nodes are unavailable, so variable-scope checks fall back to a custom lexical scanner. There is no expression typing, query result inference, or declared-return/result compatibility checking.
5. **Code generation must consume checked IR only.** SurrealQL schema emission already consumes `ResolvedSchema`, but host-language codegen is a placeholder. The old code itself says clients should wait until the schema IR has been exercised; this is a sound sequencing constraint for the reimplementation.

## Durable domain model

The language currently models four top-level syntax items: documentation, tables, analyzers, and typed functions ([`aureline-core/src/ast.rs:10`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/ast.rs#L10)). A function owns a typed parameter list and return type, while its implementation remains an explicit raw `#surql` escape hatch ([`ast.rs:28`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/ast.rs#L28)). Tables own fields and raw `@@` attributes; fields own a type, optionality, and raw `@` attributes ([`ast.rs:79`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/ast.rs#L79), [`ast.rs:106`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/ast.rs#L106)).

The type syntax is recursive: primitives, `option`, bounded/unbounded `array` and `set`, constrained/unconstrained `record`, and geometry ([`ast.rs:264`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/ast.rs#L264)). A useful normalization rule already exists: top-level `option<T>` and `T?` become identical field representations, while nested option types remain explicit ([`convert.rs:650`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/convert.rs#L650)). This is a good example of a syntax-level equivalence that should be canonical before semantic checking.

Attributes form a small extension language rather than compiler-specific conditionals. `AttributeSpec` owns its name, allowed scope, argument contract, and typed parsing result ([`semantic/attributes/spec.rs:8`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/semantic/attributes/spec.rs#L8)). Its value model covers enums, booleans, bounded integers, field lists, numeric tuples, and SurrealQL blocks ([`spec.rs:267`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/semantic/attributes/spec.rs#L267)). This is a reusable pattern for declarative feature contracts, provided feature-specific semantic constraints remain in focused lowering/checking modules.

A realistic fixture demonstrates the resulting vocabulary: schemafull tables, `record<table>` relations, optional fields, field and compound indexes, full-text analyzers, and flexible objects ([`aureline-tree-sitter/examples/realistic/product-app.aureline`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-tree-sitter/examples/realistic/product-app.aureline)).

## Compiler/checker map

```text
source
  -> Pest parse tree
  -> conversion AST (source spans captured)
  -> raw AST (syntax preserved, top-level type sugar normalized)
  -> SemanticCatalog (flat symbols, ownership, duplicate candidates)
  -> read-only analysis passes (symbols, types, analyzers, functions, SurQL scope)
  -> lowering reports (diagnostics + SemanticEffects)
  -> ResolvedSchemaBuilder
  -> ResolvedSchema
  -> SurrealQL schema emitter / migrations / future target codegen
```

The public entry points make the stages explicit: `parse_to_ast`, `parse_validated`, and `parse_resolved` ([`aureline-core/src/lib.rs:42`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/lib.rs#L42), [`lib.rs:71`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/lib.rs#L71)). Conversion captures zero-based source ranges before producing the public AST ([`convert.rs:12`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/convert.rs#L12)). This supports tooling and diagnostics without forcing semantic meaning into parsing.

`SemanticCatalog` is the preparation layer. It stores entity vectors plus lookup maps and ownership, and its name indexes map to vectors rather than single entries ([`semantic/catalog.rs:142`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/semantic/catalog.rs#L142)). The implementation states why: duplicate symbols must be retained for semantic diagnosis ([`catalog.rs:157`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/semantic/catalog.rs#L157)). Stable `TableId`, `FieldId`, `AnalyzerId`, and `FunctionId` values then let later phases refer to entities without stringly typed cross-links.

Analysis uses an intentionally explicit pass order: symbols, types, analyzers, functions, then embedded SurrealQL ([`semantic/analysis/mod.rs:64`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/semantic/analysis/mod.rs#L64)). The comment correctly resists introducing a pass registry before dependencies repeat. This is a useful maturity rule: make dependencies visible first; generalize only after the domain supplies stable repetition.

Resolution does not mutate syntax. It builds the catalog, accumulates all analysis and lowering errors, collects effects, then applies the effects to a new resolved graph ([`semantic/mod.rs:38`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/semantic/mod.rs#L38)). `SemanticEffect` currently covers table indexes, field flexibility, and function permissions ([`semantic/lowering/effects.rs:5`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/semantic/lowering/effects.rs#L5)); `SemanticReport` transports diagnostics and effects together ([`lowering/report.rs:4`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/semantic/lowering/report.rs#L4)). The resolved graph is small and consumer-oriented: analyzers, functions, and tables, with resolved table names/indexes and resolved field flags ([`semantic/resolved.rs:18`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/semantic/resolved.rs#L18)).

## Static checking: what works and what does not

Successful checks are modular and diagnostic-accumulating. The checker detects duplicate symbols, recursively validates `record<table>` references (including nested containers and function signatures), checks analyzer references, validates typed attribute arguments, checks function parameter contracts, validates SurrealQL syntax, and constrains available variables by escape-hatch context. Recursive record checking is compact because it walks the type tree rather than enumerating surface syntaxes ([`semantic/analysis/types.rs:32`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/semantic/analysis/types.rs#L32)). Multiple independent errors are accumulated; a function test asserts simultaneous missing/unknown-parameter and invalid-permission diagnostics ([`tests/semantic/functions.rs:200`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/tests/semantic/functions.rs#L200)).

The important limitation is the SurrealQL seam. Syntax is delegated to `surrealdb_core::syn::parse`, which is correct ownership ([`aureline-core/src/surql.rs:1`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/surql.rs#L1)). But useful expression nodes are crate-private, so context checking uses a handwritten variable scanner ([`semantic/analysis/surql.rs:70`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/semantic/analysis/surql.rs#L70)). Function checking similarly compares declared parameters with a set lexically collected from the raw body ([`semantic/analysis/functions.rs:38`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/semantic/analysis/functions.rs#L38)). This is useful validation, but it is not a typed query checker.

For a mature implementation, embedded query bodies therefore need their own query IR with source provenance, scopes, resolved names, expression types, cardinality/nullability, and result row shape. A lexical scanner should not grow into that IR implicitly. If SurrealDB cannot expose a suitable AST, Aureline must own a supported SurrealQL subset/parser or define a structured query language whose lowering target is SurrealQL.

## Lowering and code generation lessons

SurrealQL schema emission consumes resolved data, sorts entities deterministically, and emits functions/tables from checked fields ([`emit/surql.rs:34`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/emit/surql.rs#L34)). Typed functions lower to `DEFINE FUNCTION` with typed parameters, return type, raw body, and checked permission ([`emit/surql.rs:76`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/emit/surql.rs#L76)). Deterministic emission and consumer-oriented resolved data are patterns to preserve.

The compatibility `emit_schema(&Schema)` path is a warning sign: it conditionally resolves and panics if semantic validity was not established ([`emit/surql.rs:8`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/emit/surql.rs#L8)). New emitters should accept only a checked IR type, making invalid phase ordering unrepresentable.

Host codegen is intentionally only a placeholder returning the target names Rust, TypeScript, and Python ([`aureline-codegen/src/lib.rs`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-codegen/src/lib.rs)). The documentation likewise labels client generation unimplemented and restricts usable behavior to schema checks and migrations ([`docs/wip/codegen-and-client.mdx:8`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-docs/content/docs/wip/codegen-and-client.mdx#L8)). The durable rule is: define a language-neutral checked query ABI first (parameter names/types, result type/cardinality, query text or structured plan, bind map); make TypeScript/Rust/Python thin renderers of that ABI.

## Complexity and failure signals to avoid

- **One representation serving incompatible consumers.** The raw AST intentionally retains source spelling for LSP use, while the resolved graph serves migrations/emission. Do not collapse them or gradually add every semantic fact to parser nodes.
- **Feature conditionals spread across phases.** Attribute specs centralize syntax contracts, while feature modules own semantic lowering. Preserve that ownership; avoid a giant visitor that switches on every feature at every node.
- **Premature pass-framework abstraction.** ARC1 explicitly postpones registries/builders until views, events, permissions, and indexes reveal repetition ([`lowering/mod.rs:36`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/semantic/lowering/mod.rs#L36)). Prefer a short explicit pipeline with declared inputs/outputs.
- **Lexical query analysis expanding into a compiler.** The scanner already tracks closures, strings, comments, and nesting. Treat it as a bounded bridge, not a foundation for result typing.
- **Imprecise embedded-language spans.** Unknown-variable diagnostics point at the whole `#surql` block because the AST does not retain the body offset ([`tests/semantic/surql.rs:117`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/tests/semantic/surql.rs#L117)). Query IR nodes should carry mapped source ranges from day one.
- **Backend assumptions inside validation.** Field permission validation hardcodes `TYPE string` instead of using the Aureline field type ([`surql.rs:34`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/src/surql.rs#L34)). Backend validation contexts must be derived from checked domain types.

## Reusable tests and fixtures

The most valuable regression suite is organized by semantic capability under [`aureline-core/tests/semantic/`](https://github.com/pixelscortex/aureline/tree/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/tests/semantic): pipeline boundaries, symbols, types, analyzers, functions, attributes, SurrealQL scope, ordinary/compound indexes, full-text indexes, and vector indexes. These should be ported as behavioral specifications rather than mechanically copied.

Especially reusable contracts are:

- raw attributes remain uninterpreted until semantics, and resolution does not mutate syntax ([`pipeline.rs:10`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/tests/semantic/pipeline.rs#L10), [`pipeline.rs:45`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/tests/semantic/pipeline.rs#L45));
- parser success is not semantic success ([`pipeline.rs:126`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/tests/semantic/pipeline.rs#L126));
- declarations may be referenced before their textual declaration, so catalog construction precedes resolution ([`tests/semantic/analyzers.rs`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-core/tests/semantic/analyzers.rs));
- invalid programs return multiple useful diagnostics;
- diagnostic ranges survive into embedded-language checks;
- output is deterministic and semantic sugar lowers to structured indexes/flags.

[`aureline-test-support/src/lib.rs`](https://github.com/pixelscortex/aureline/blob/f8eac12b6d8a8c63e543682d418a1d4181f84413/aureline-test-support/src/lib.rs) provides compact parse/resolution/error helpers worth recreating. The realistic product schema is a good cross-feature acceptance fixture, while each focused semantic test should remain small enough to name one invariant.

## Implications for the reimplementation

Treat ARC1's `ResolvedSchema` as proof of the **resolved-schema** concept, not as the final universal IR. The new architecture should have at least these durable boundaries:

1. source/CST and a loss-aware syntax AST for tooling;
2. a definition catalog with stable IDs and ownership;
3. a resolved schema IR containing canonical domain facts;
4. a separate typed query IR containing scopes, resolved paths, expression types, cardinality/nullability, and row shapes;
5. diagnostics as accumulated data with precise provenance;
6. lowering from checked query IR to SurrealQL plus a language-neutral generated-function model;
7. thin host renderers for TypeScript, Rust, and Python.

The deepest transferable lesson is not a particular Rust module layout. It is that every phase should reduce ambiguity and expose a smaller contract to the next consumer. Syntax preserves what the user wrote; catalogs establish identity; analysis proves invariants; lowering records explicit meaning; resolved IR contains only checked facts; emitters render rather than re-check.
