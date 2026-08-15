# Aureline ARC2: durable architectural lessons

## Provenance and scope

- Primary source: local clone at `.repo/aureline-orm-arc2` (gitignored, local-only — every claim below is also backed by an immutable GitHub link)
- Repository: `pixelscortex/aureline-orm-arc2`
- Inspected commit: [`c6c752bc66bde7f5bb4e4b80300d0ae907d76c35`](https://github.com/pixelscortex/aureline-orm-arc2/tree/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35) (2026-07-01, `Merge pull request #2 from pixelscortex/feat/semnatics-understanding`)
- Verification: `cargo test --workspace --locked -q` passed on 2026-08-02.
- Scope: durable DSL, compiler, semantic, IR, diagnostics, generation, and testing lessons only. Transient parser/debugging details are intentionally excluded.

## Executive conclusion

ARC2 found several seams worth retaining: an Aureline-owned, source-spanned syntax model; contextual lowering of embedded SurrealQL into that model; duplicate-preserving source facts separated from validation; one semantic type vocabulary with distinct recovery sentinels; deterministic semantic passes; and structured, stable diagnostics. Its central unfinished problem is equally clear: the normalized AST is being asked to serve simultaneously as syntax tree, query IR, and traversal substrate, while semantic results are mostly transient. There is no persistent typed program/query IR and no code-generation layer yet.

For a mature reimplementation, keep the phase boundaries and semantic principles, but introduce an explicit resolved/typed IR with stable node identities and reusable traversal infrastructure. Code generators should consume only that validated IR, never parser ASTs or SurrealDB's arena AST directly.

## Established language surface

ARC2's top-level language consists of five declaration families: tables, relation tables, analyzers, functions, and events. Tables and relations choose `schemafull`/`schemaless`; fields have dotted paths, SurrealDB-shaped types, and attributes; relations add directional `relate` endpoint clauses; functions have typed parameters, optional return annotations, attributes, and a required `run` block; events have an owning table, `when` expression, attributes, and a required `run` block. See [the declaration AST](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-ast/src/schema.rs#L12-L160) and representative [table/relation](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/tests/cases/table.rs#L14-L143), [function](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/tests/cases/func.rs#L1-L64), and [event](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/tests/cases/event.rs#L1-L118) contracts.

The type syntax already captures much of SurrealDB's vocabulary: builtins, custom names, arrays and sets, record/table/file constraints, structural objects, geometries, option, union/either, literal types, tuples, `none`, and `null`. This is broader than the semantic type model presently preserves. See [`TypeExpr`](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-ast/src/types.rs#L5-L73).

Attributes are the language's schema-to-SurQL bridge. The implemented catalog recognizes field transforms/contracts (`@assert`, `@value`, `@default`), indexes (`@index`, `@unique`, full-text, HNSW), counts, and permissions with location rules. Attribute payloads can be positional, named, or embedded SurQL. See [attribute syntax](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-ast/src/schema.rs#L128-L154) and [catalog/location rules](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/attributes/catalog.rs#L3-L134).

Embedded SurQL is deliberately contextual rather than a single undifferentiated string: ordinary attribute and event conditions use an expression slot, permissions use SurrealDB's permission grammar, and `run` bodies use a query/statement-list slot. This is a durable domain rule because identical text can have different legal grammar and meaning depending on its host. See [slot routing and document lowering](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/src/parser/surql/mod.rs#L15-L105).

## Compiler shape recovered from ARC2

```text
.aurl source
  -> Aureline lexer/parser
  -> source-spanned Aureline AST containing raw embedded SurQL
  -> context-selected SurrealDB parse
  -> lowering from temporary SurrealDB arena AST into Aureline-owned AST
  -> duplicate-preserving SchemaFacts + FunctionCatalog
  -> deterministic semantic passes producing Findings
  -> diagnostic rendering + immutable SemanticAnalysis

Standalone embedded expression
  -> same SurrealDB-to-Aureline lowering
  -> ExprChecker + FunctionCatalog + lexical type scope
  -> inferred Ty + findings
```

The parser itself documents the first two phases and exposes syntax-only versus production entry points. This is useful both for tooling and for tests that need to isolate responsibility. See [`Parser::document_syntax` and `Parser::document`](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/src/parser/mod.rs#L1-L99).

SurrealDB's AST is treated as a temporary foreign parse representation. `LowerCx` owns arena access, source remapping, and raw fallback slices; the final Aureline AST does not retain arena references. `SurqlLowerer` centralizes recursive conversions. These are good dependency-containment boundaries. See [`LowerCx`](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/src/parser/surql/cx.rs#L5-L31) and [`SurqlLowerer`](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/src/parser/surql/lower/lowerer.rs#L8-L73).

The normalized query AST models expressions, `SELECT`, `LET`/`FOR`, and mutation statements, while preserving clauses that are not yet semantically interpreted. Unsupported parsed expressions can become source-preserving `Opaque` nodes rather than corrupting meaning. See [`ExprKind`](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-ast/src/expr.rs#L14-L108), [`Select`](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-ast/src/query.rs#L11-L51), and [mutation/statement shapes](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-ast/src/statement.rs#L12-L134).

## Durable semantic-engine rules

### 1. Facts record source; passes judge it

`SchemaFacts` flattens declarations into source-order vectors with analysis-local typed IDs. It intentionally preserves duplicates and invalid references. Derived lookup maps return missing/one/duplicate states instead of silently overwriting declarations. Validation is a separate pass. This supports precise diagnostics, deterministic behavior, and later LSP/codegen consumers without forcing every consumer to walk syntax trees. See [the fact model and invariants](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/index/mod.rs#L1-L69), [fact collection](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/index/collect.rs#L15-L127), and [duplicate-aware lookup outcomes](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/index/lookup.rs#L176-L222).

Carry this forward as a strict rule: collection/index construction must be lossless and diagnostic-free; resolution and validation own policy.

### 2. Navigation is not resolution

Borrowed views provide ergonomic `schema -> table -> field -> attr` navigation without changing storage, resolving names, or hiding duplicates. This is a useful API seam for codegen, LSP, migrations, and checks. See [view invariants and API](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/index/view.rs#L1-L135).

### 3. One semantic type vocabulary, with explicit uncertainty and recovery

`Ty` distinguishes SurrealDB's real dynamic `Any` from incomplete inference (`Unknown`) and already-diagnosed recovery (`Error`). This is essential for an aggressive checker: conflating the three either hides errors or creates cascades. `Ty` is also shared between fields, function catalogs, expression inference, assignability, and future tooling. See [the semantic type model](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/types/mod.rs#L1-L61) and [recovery-aware assignability](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/typecheck/assign.rs#L5-L33).

### 4. Function behavior is static catalog data, not runtime reflection

Builtin and schema functions lower to the same duplicate-preserving catalog of path, provenance, type parameters, parameter arity/type, and return type. Calls resolve against all same-path candidates and choose a useful arity/type diagnostic when none match. This is the right direction for offline type safety and future plugins. See [catalog construction and lookup](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/functions/catalog.rs#L23-L158), [signature/provenance model](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/functions/signature.rs#L12-L149), and [call checking](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/typecheck/call.rs#L7-L98).

### 5. Semantic checks emit domain findings; presentation is separate

Passes produce typed `Finding` values. A renderer assigns stable codes, messages, labels, and help. The public report exposes an immutable analysis plus ordered diagnostics, while the internal context owns mutation. This makes CLI rendering replaceable and diagnostics testable as data. See [finding abstraction](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/finding.rs#L1-L15), [diagnostic data](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/diagnostic.rs#L8-L64), [renderer](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/diagnostic/render.rs#L8-L80), and [fixed engine/report](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/engine.rs#L5-L108).

### 6. Context injects static types, never values

Field expression checking constructs an explicit scope in which `$value`, `$input`, `$after`, `$before`, `$this`, and `$self` have contextual types. Permissions similarly add `$auth`. This is a strong model for other contexts such as events and queries: define a static environment per host construct, then reuse one expression checker. See [field scopes](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/lowering/field.rs#L5-L57) and [attribute/permission checking](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/attributes/validate/types.rs#L69-L190).

## What demonstrably worked

- The two-parser strategy keeps Aureline syntax small while delegating SurrealQL acceptance to a pinned SurrealDB parser, then regains ownership through lowering. Source offsets are remapped back into the outer `.aurl` file, enabling precise diagnostics across embedded snippets. See [parser dependency pin](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/Cargo.toml#L7-L11) and [span remapping](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/src/parser/surql/cx.rs#L19-L31).
- Surgical parser APIs and structural S-expression outlines make tests read as language contracts, independent of Rust debug formatting. The same type fixtures can check equivalence between Aureline's type parser and SurrealDB lowering. See [test guidance](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/tests/cases/AGENTS.md#L1-L18), [shared runner](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/tests/support/runner.rs#L1-L108), and [structural outline model](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-test/src/outline/mod.rs#L1-L54).
- The semantic vertical slices prove useful static contracts: function arity/types, scoped `LET`, query/mutation output cardinality, field default/value/assert compatibility, analyzer references, attribute placement/shape, and non-cascading recovery. See [query cardinality](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/typecheck/query.rs#L7-L36), [statement output inference](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/typecheck/statement.rs#L7-L51), and [field contract tests](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/tests/field_attr_typecheck.rs#L7-L73).
- The CLI is intentionally only a checker: parse, analyze, render diagnostics, and fail on errors. That matches Aureline's static-tooling identity. See [CLI check path](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-cli/src/main.rs#L22-L91).

## Gaps and maintenance hazards

### No typed program IR yet

`SemanticAnalysis` retains the source document, schema facts, and function catalog, but no per-expression inferred types, resolved references, scopes, effects, or result shapes. `ExprChecker` returns one root `Ty` and transient findings. Consequently, later codegen/LSP phases would need to repeat semantic work or depend on checker internals. See [`SemanticAnalysis` storage](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/analysis.rs#L5-L74), [`TypeCheckReport`](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/typecheck/mod.rs#L29-L61), and [`ExprChecker`](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/typecheck/infer.rs#L9-L64).

Durable response: introduce stable syntax IDs, resolution tables, and a typed query/function IR (or typed HIR) whose nodes retain source origins. Store type/reference/cardinality facts once. Generation targets consume that IR.

### The source/query AST is too close to becoming the universal IR

ARC2's `ExprKind` combines literals, paths, access, graph traversal, calls, control flow, statements, subqueries, and opaque syntax. This is workable for normalization, but it couples every semantic visitor to the full SurrealQL surface. The 363-line call walker manually descends every query and mutation clause, while inference repeats another exhaustive dispatch. This is a direct expansion hazard as SurrealQL grows. See [expression breadth](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-ast/src/expr.rs#L16-L108), [manual expression traversal](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/attributes/validate/references/walk.rs#L3-L78), and [manual query/statement traversal](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/attributes/validate/references/walk.rs#L147-L230).

Durable response: make traversal a single AST-owned visitor/fold API or generated child-edge table. Semantic rules should request occurrences through reusable queries/walkers, not each reproduce tree recursion.

### Semantic type lowering is currently lossy

The source type AST models object properties, geometry variants, tuple elements, and collection lengths, but `lower_type` collapses objects to generic `object`, geometry/tuple to custom strings, and arrays/sets to element-only types. Function parameters are worse: their AST field is `Expr`, not `TypeExpr`, and unsupported forms silently become `Ty::Any`; missing returns also become `Any`. These permissive fallbacks undermine aggressive type safety. See [source type richness](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-ast/src/types.rs#L5-L56), [lossy semantic lowering](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/lowering/type_expr.rs#L5-L31), and [function fallback](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/lowering/function.rs#L8-L56).

Durable response: use `TypeExpr` consistently for every annotation; resolve it into a richer canonical type algebra; reserve `Any` for an explicitly declared dynamic type; make unsupported type syntax a diagnostic plus `Error`/`Unknown`, not silent weakening.

### Type checking covers only a narrow slice and is permissive by design

Many central expressions—field access, binary operators, casts, closures, graph traversal, conditions, record IDs—currently infer `Unknown`. `FOR` and several mutation outputs do too. Generic parameters accept everything and are not bound. `SELECT` lists always infer generic object rows rather than schema-derived shapes. See [unknown inference branches](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/typecheck/infer.rs#L59-L95), [permissive generics](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/typecheck/assign.rs#L7-L23), and [projection inference](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/typecheck/query.rs#L18-L35).

Durable response: organize inference by explicit rule families with a shared constraint/substitution context. Treat query result shape and cardinality as first-class, separate dimensions rather than encoding only cardinality through `Array<T>`.

### `Opaque` is valuable recovery but cannot mean “checked”

The lowerer intentionally preserves unsupported parsed syntax as `Opaque`, sometimes using the whole raw query for unsupported top-level forms. This is excellent for forward-compatible parsing and diagnostics, but semantic inference maps it to `Unknown`, so a successful parse is not a proof of static safety. See [opaque source preservation](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/src/parser/surql/cx.rs#L193-L208), [top-level fallback](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/src/parser/surql/mod.rs#L176-L200), and [unknown inference](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-semantic/src/typecheck/infer.rs#L79-L95).

Durable response: retain opaque nodes for IDE/recovery modes, but define a strict compilation gate: any opaque or unresolved node reachable from generated code must produce an unsupported-feature diagnostic and block generation.

### The SurrealDB parser boundary is pinned but structurally coupled

ARC2 pins a specific git revision and correctly hides arena details behind `LowerCx`, yet dozens of lowerers still pattern-match SurrealDB AST variants. Upstream changes will create broad churn. The dependency should remain quarantined in one adapter crate with fixture/compatibility tests, and the rest of Aureline should depend only on owned syntax/IR. See [pinned dependencies](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/Cargo.toml#L7-L11), [adapter facade](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/src/parser/surql/lower/lowerer.rs#L8-L73), and [conversion traits](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-parser/src/parser/surql/lower/from_surql.rs#L3-L28).

### Generation targets are design intent, not existing implementation

The workspace has AST, parser, semantic, CLI, and test crates only; the CLI exposes only `check`. There is no TypeScript, Rust, Python, SurrealQL, migration, or runtime code generator in ARC2. See [workspace members](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/Cargo.toml#L1-L18) and [CLI commands](https://github.com/pixelscortex/aureline-orm-arc2/blob/c6c752bc66bde7f5bb4e4b80300d0ae907d76c35/aureline-cli/src/main.rs#L15-L30).

Durable response: model generation as backends over one language-neutral validated artifact. Separate at least (1) exact SurrealQL/source preservation, (2) parameter binding metadata, (3) result shape/cardinality, and (4) host-language type/rendering policy. Do not bake TypeScript/Rust/Python conventions into semantic types.

## Reimplementation baseline

The mature implementation should preserve these boundaries:

1. Lossless, source-spanned syntax trees and raw snippets for recovery.
2. A quarantined SurrealDB parser adapter that produces Aureline-owned nodes.
3. Lossless source facts with local typed IDs and duplicate-aware lookups.
4. Separate name resolution, type checking, and validation passes with deterministic scheduling.
5. A canonical semantic type algebra where `Any`, `Unknown`, and `Error` remain distinct.
6. A persistent resolved/typed IR containing expression types, references, scopes, query row shapes, cardinality, and source origins.
7. Findings as domain data and diagnostics as rendered presentation.
8. Strict generation gating: no unresolved/opaque/error nodes in generated paths.
9. Backend-neutral generation inputs and thin target-specific renderers.
10. Surgical language-contract tests plus end-to-end fixtures that assert parse -> resolve -> type -> generated query/API.

The key reframing is that `SchemaFacts` is a successful semantic index, but it is not the whole semantic IR; and the normalized SurQL AST is a successful owned parse representation, but it should not become the typed IR. Preserving those distinctions is the main defense against the complexity explosion ARC2 was beginning to show.
