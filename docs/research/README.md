# Research

Background reading that decisions on the tracker refer to. These are **findings**, not specifications — they record what was true when they were written, and nothing here is binding. Decisions live in `docs/adr/`; live questions live in the Wayfinder map.

- [`synthesis.md`](synthesis.md) — consolidated rules carried forward from both previous attempts, plus the recommended work order
- [`aureline-arc1.md`](aureline-arc1.md) — architectural lessons from attempt one (`pixelscortex/aureline`)
- [`aureline-arc2.md`](aureline-arc2.md) — architectural lessons from attempt two (`pixelscortex/aureline-orm-arc2`)
- [`arc1-salvaged-goals.md`](arc1-salvaged-goals.md) — durable product intent salvaged from attempt one's 95 tracker issues, including seven hard-won SurrealDB constraints
- [`surrealql-static-contract.md`](surrealql-static-contract.md) — SurrealQL facts that constrain what Aureline can type statically
- [`migration-snapshot-prior-art.md`](migration-snapshot-prior-art.md) — how Drizzle v1 and Prisma model migration snapshots and diffing
- [`spacetimedb-codegen-prior-art.md`](spacetimedb-codegen-prior-art.md) — how SpacetimeDB structures multi-language client generation, its schema IR, and its subscription/reactivity model
- [`typesafe-queries-typescript.md`](typesafe-queries-typescript.md) — how best-in-class TS tooling types query results; codegen-concrete-types vs type-level machinery, with Prisma's 95× instantiation benchmark
- [`typesafe-queries-rust.md`](typesafe-queries-rust.md) — sqlx/Diesel/SeaORM/clorinde survey, the surrealdb 3.2 crate's type surface, and the recommended struct-per-query + `Query` trait shape
- [`typesafe-queries-python.md`](typesafe-queries-python.md) — carrier comparison (dataclass/TypedDict/Pydantic/msgspec), checker landscape 2026, and the frozen-slotted-dataclass recommendation
- [`typesafe-queries-go.md`](typesafe-queries-go.md) — Go 1.26 generics state, the sqlc model, phantom-typed record IDs, and the honest "Rust minus exhaustiveness" verdict
- [`arc2-attribute-grammar.md`](arc2-attribute-grammar.md) — exhaustive reconstruction of attempt two's attribute and field grammar, with the full catalog, placement matrix, and every error case
- [`arc2-surql-surface.md`](arc2-surql-surface.md) — exhaustive reconstruction of attempt two's embedded-SurrealQL surface: expressions, operators, record IDs, casts, statements, and every `Opaque` fallback site
- [`convex-components-prior-art.md`](convex-components-prior-art.md) — how Convex packages schema, functions, state, exports, installed instances, table namespaces, and subtransactions into reusable backend modules

The two `arc2-*` files are the raw evidence behind the [Grammar](https://github.com/pixelscortex/aureline-orm/issues/39) roadmap page. That page is the readable summary; these are the receipts.

Clones of both previous attempts sit under `.repo/`, and a point-in-time audit of this repository's state sits at `.context/research/current-reimplementation.md`. Both directories are gitignored and local-only, so anything durable from them belongs here instead.
