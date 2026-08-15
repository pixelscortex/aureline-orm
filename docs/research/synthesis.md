# Aureline architecture synthesis

## Product invariant

Aureline is a static compiler for a schema-and-query language built around SurrealQL. It receives source, proves a query contract without a live database, and generates SurrealQL plus typed host-language functions. It is not a runtime checker, query executor, or database client.

## Evidence baseline

- The current reimplementation has only a minimal lexer and arena-backed table parser. Checker, typed IR, lowering, codegen, migration, and CLI behavior are absent.
- ARC1 proves the value of separate raw and resolved representations, stable semantic identities, duplicate-preserving catalogs, diagnostic accumulation, and effect-based lowering. It does not solve typed embedded queries.
- ARC2 proves the value of contextual SurrealQL parsing, source remapping, Aureline-owned normalized syntax, lossless facts, static function catalogs, structured findings, and explicit `Any`/`Unknown`/`Error` recovery types. It lacks a persistent typed IR, duplicates traversal, and silently loses type information in several paths.
- Current SurrealQL requires the checker to distinguish stored schema, write inputs, and query results. Table kinds, open versus closed object shapes, computed/default/read-only fields, record links, projections, `FETCH`, nullability, and result cardinality affect those contracts independently.

## Durable rules to carry forward

1. Parsing success is not semantic success, and semantic success is not generation readiness.
2. Preserve source spelling and provenance in syntax representations; never mutate syntax nodes into semantic nodes.
3. Collect Source Facts losslessly before judging them. Duplicate or invalid declarations must remain representable so diagnostics are deterministic.
4. Establish stable identities and ownership before name resolution or type checking.
5. Keep the SurrealDB parser behind one adapter seam. Lower temporary foreign nodes into Aureline-owned, source-spanned syntax.
6. Embedded SurrealQL is context-specific: query bodies, expressions, permissions, and other slots have different grammars and Static Environments.
7. Use one canonical type algebra, but keep `Any`, `Unknown`, and `Error Type` distinct.
8. Model Query Result as at least value/Row Shape, Cardinality, and Nullability. Do not encode all three accidentally in container types.
9. Store resolved references, inferred types, scopes, and result shapes in a persistent checked representation. Later consumers must not repeat inference.
10. Centralize syntax traversal. Individual checks must not each grow their own exhaustive recursive walker.
11. Preserve Opaque Syntax for recovery and editors, but block generation whenever opaque, unresolved, unknown, or error-bearing syntax is reachable from a Generated Query Artifact.
12. Findings are semantic data; Diagnostics are presentation. Stable diagnostic identities and precise mapped spans are part of the checker contract.
13. Target Renderers consume only Generated Query Artifacts and perform no semantic checking.
14. Generated code receives a Database Context and binds parameters; Aureline does not own runtime execution.
15. Port behavioral fixtures from the old attempts as language contracts, not as implementation structure.

## Representation hypothesis to test, not yet an ADR

```text
source
  -> lossless/spanned Aureline syntax
  -> context-aware SurrealQL adapter
  -> Aureline-owned normalized query syntax
  -> lossless facts + stable identities
  -> resolution and static checking
  -> resolved schema + typed query IR
  -> Generated Query Artifact
  -> TypeScript / Rust / Python Target Renderers
```

This is the strongest current hypothesis, but the exact number and interface of representations belongs to the IR Wayfinder map.

## Recommended work order

1. Define the first end-to-end language slice and strict unsupported-feature policy.
2. Settle semantic identities, type/result algebra, and Static Environments.
3. Settle representation and phase interfaces through one vertical query prototype.
4. Define the Generated Query Artifact and prove two target renderers against it.
5. Only then expand SurrealQL coverage and port broader fixtures.

## Avoided premature decisions

- The final grammar and full SurrealQL coverage.
- Whether the typed representation is named HIR, TIR, or another conventional compiler acronym.
- A generic pass registry or plugin framework before phase dependencies repeat.
- A runtime ORM/query-builder interface.
- Target-specific semantic types.
