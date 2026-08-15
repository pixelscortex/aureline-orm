# SurrealQL facts that constrain Aureline's static contract

Captured 2026-08-02 from SurrealDB's official documentation. These are language-surface constraints, not proposed Aureline architecture.

## Durable findings

- A SurrealDB table can be `TYPE ANY`, `TYPE NORMAL`, or `TYPE RELATION`; relation tables can constrain their `FROM` and `TO` record kinds. Aureline's schema model therefore cannot reduce every table to a flat record collection. Source: [DEFINE TABLE](https://surrealdb.com/docs/surrealql/statements/define/table).
- `SCHEMAFULL` rejects undefined fields in current SurrealDB, while `FLEXIBLE` selectively permits additional keys for object-bearing field types. Static object-shape checking needs an explicit open/closed-shape dimension instead of treating every object as uniformly exact or inexact. Sources: [DEFINE TABLE](https://surrealdb.com/docs/surrealql/statements/define/table), [DEFINE FIELD](https://surrealdb.com/docs/surrealql/statements/define/field).
- Field declarations encode more than a value type: optionality, defaults, assertions, computed/value behavior, read-only behavior, record references, and nested field declarations affect what may be written and what can be read. Aureline should distinguish stored shape, writable input shape, and selected result shape. Source: [DEFINE FIELD](https://surrealdb.com/docs/surrealql/statements/define/field).
- Custom functions have typed parameters, trailing optional arguments, and optional declared return types. A function symbol is therefore a typed callable contract, not merely an emitted text macro. Source: [DEFINE FUNCTION](https://surrealdb.com/docs/surrealql/statements/define/function).
- A `SELECT` target may be a table, record, edge, subquery, parameter, array, object, or other value. Selection plus `FETCH` can reshape record links into nested record data. Result inference must be expression- and projection-driven; it cannot be modeled as “query table gives table row.” Source: [SELECT](https://surrealdb.com/docs/surrealql/statements/select).

## Architectural pressure implied by those facts

These facts favor distinct semantic representations for declared schema, expression/query typing, and host-language output contracts. They also favor explicit transformations between shapes rather than mutating one universal type object as analysis proceeds. This is an inference for future decision tickets, not a resolved design.
