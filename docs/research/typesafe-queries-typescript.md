# Type-Safe Query Results in TypeScript — Prior Art for Aureline Codegen

**Provenance.** Researched 2026-08-13 from primary web sources: vendor docs and blogs (Prisma,
Gel/EdgeDB, sqlc, Convex, TanStack, oRPC, SurrealDB), GitHub repos/issues, and TypeScript release
notes. Links cited inline. Purpose: inform Aureline's TS codegen design. Aureline's Rust compiler
infers each query's result type ahead of time and *generates* concrete TS types — the host language
only carries types, never computes them. This file surveys how existing tools sit on that spectrum
and what the generated output should look like.

## 1. The spectrum: type-level inference vs. generated concrete types

| Tool | Where types come from | Mechanism |
| --- | --- | --- |
| [Kysely](https://kysely.dev/) | Type-level | Hand-written (or [kysely-codegen](https://github.com/kysely-org/kysely)-introspected) `Database` interface; the builder infers result rows from selections, joins, subqueries via conditional/mapped types |
| [Drizzle](https://github.com/drizzle-team/drizzle-orm) | Type-level | Schema defined in TS; compiler re-derives query result types on every keystroke |
| [Prisma](https://www.prisma.io/blog/why-prisma-orm-checks-types-faster-than-drizzle) | Codegen | `prisma generate` writes precomputed `.d.ts`; flat interfaces, no deep recursion |
| [Prisma TypedSQL](https://www.prisma.io/docs/orm/prisma-client/using-raw-sql/typedsql) | Codegen, per query | `prisma generate --sql` emits one typed function per `.sql` file, importable from `@prisma/client/sql` |
| [sqlc-gen-typescript](https://github.com/sqlc-dev/sqlc-gen-typescript) | Codegen, per query | Per annotated query (`-- name: GetAuthor :one`): an `Args` interface, a `Row` interface, and a standalone async function — plain interfaces, zero type-level machinery ([announcement](https://sqlc.dev/posts/2023/12/04/preview-typescript-support-with-sqlc-gen-typescript/)) |
| [Gel/EdgeDB](https://docs.geldata.com/reference/using/js/querybuilder) | Hybrid | `@gel/generate edgeql-js` codegens a schema reflection, then a type-level query builder infers on top (`$infer`); a separate [`queries` generator](https://github.com/geldata/gel-js/blob/master/docs/queries.rst) compiles `.edgeql` files to fully concrete per-query functions |
| [Convex](https://docs.convex.dev/generated-api/) | Codegen | `convex dev` emits `_generated/dataModel.d.ts` with `Doc<"table">`, `Id<"table">`, and a typed `api` object |

**Observed trade-offs.** Type-level inference gives zero build step and "types follow the code
instantly", but the cost is real: Drizzle has long-standing reports of multi-second IDE stalls and
blocked completions on wide schemas ([#870](https://github.com/drizzle-team/drizzle-orm/issues/870),
[#4823](https://github.com/drizzle-team/drizzle-orm/issues/4823)). Prisma's
[benchmark](https://www.prisma.io/blog/why-prisma-orm-checks-types-faster-than-drizzle) against
Drizzle 0.44.4 measured **428 vs. 41,150 type instantiations** (~95×) for schema checking and ~2–3×
faster `tsc` check times for queries, attributing it to avoiding non-homomorphic mapped types,
deeply nested conditional types, and intersection-heavy unions — generation does the type-heavy
work once, and named interfaces hit the compiler's internal caches. Error readability follows the
same line: inferred types expand into page-long structural dumps in errors and hovers, while named
generated interfaces display as their names. Gel's hybrid is notable: even its "inference" layer
stands on generated reflection code, and its `queries` generator — EdgeQL file in, concrete typed
function out — is the closest existing analogue to Aureline's model, as is Prisma TypedSQL.

## 2. Branded record IDs

The established pattern is a string-literal-parameterised brand. Convex's
[`Id<"users">`](https://docs.convex.dev/generated-api/data-model) is
`string & { __tableName: "users" }` — a plain string at runtime, nominally distinct at the type
level ([Convex types cookbook](https://stack.convex.dev/types-cookbook),
[branded validators](https://stack.convex.dev/using-branded-types-in-validators)). SurrealDB's own
JS SDK instead uses a runtime **class** `RecordId<Tb extends string>` (with `StringRecordId`,
`RecordIdRange`, `Table` companions) — see the
[SDK type reference](https://surrealdb.com/docs/sdk/javascript/api/types) — because SurrealDB IDs
are structured values (`table:⟨id⟩` where id may be a number, UUID, array, or object), not opaque
strings.

Known pitfalls of property-branding ([overview](https://nanamanu.com/posts/branded-types-typescript/),
[practical guide](https://oneuptime.com/blog/post/2026-01-30-how-to-implement-branded-types-in-typescript/view)):
branded strings can't index `Record<PlayerId, T>` cleanly; two types sharing the literal brand key
`__brand: "id"` silently collide (the brand payload must carry the table name, as Convex's does);
construction requires an `as` cast, so it must be funnelled through one generated smart constructor;
and `unique symbol` brands break across duplicated package instances in `node_modules` (two copies of
the lib declare two distinct symbols), which is why cross-package codegen favours string-keyed
property brands over symbols. For Aureline the cleanest path is to reuse the SDK's
`RecordId<"user">` class in generated signatures — nominal-enough via the literal parameter, already
serialisation-aware — rather than inventing a parallel string brand.

## 3. Literal unions, event payloads, and DIFF typing

Enum-like columns codegen naturally to named literal-union aliases
(`type PostStatus = "draft" | "published"`), which give exhaustive `switch` narrowing for free.
Live-query notifications are the canonical discriminated-union case: SurrealDB's SDK types them as
`{ action: "CREATE" | "UPDATE" | "DELETE"; result: T }`
([streaming docs](https://surrealdb.com/docs/sdk/javascript/core/streaming)) — but note a codegen
opportunity the SDK misses: `DELETE` carries the *old* record and `CREATE`/`UPDATE` the new one, so
a generated *tagged* union (`{ action: "CREATE"; record: Post } | ... | { action: "DELETE"; record: Post }`)
per table is strictly better than a shared generic.

For `LIVE SELECT DIFF`, payloads are RFC 6902 JSON Patch arrays. SurrealDB's experimental
[surqlize](https://github.com/surrealdb/surqlize) ORM types `.diff()` results as `JsonPatchOp[]`;
the ecosystem norm ([`@json-patch/types`](https://jsr.io/@json-patch/types)) is a six-way tagged
union on `op` (`add`/`remove`/`replace`/`move`/`copy`/`test`). Typing `path` *per result shape* via
template-literal types is possible but is exactly the deep-recursive machinery to avoid; since
Aureline knows the shape statically, it can either emit a concrete union of valid `path` literals
for shallow shapes or fall back to `string` — a decision made in Rust, costing tsserver nothing.

## 4. Reified queries: the `queryOptions` pattern

TanStack Query v5's [`queryOptions`](https://tanstack.com/query/v5/docs/framework/react/guides/query-options)
helper established "query as a value carrying its types": an object bundling `queryKey` + `queryFn`
whose result type flows into `useQuery`, `prefetchQuery`, `getQueryData`, paginators, etc. — one
definition, inference everywhere. [oRPC](https://www.npmjs.com/package/@orpc/tanstack-query) builds
on it: every contract procedure exposes `.queryOptions(...)`, so key, input type, and output type
all derive from one definition. The mechanism is a phantom type parameter on the returned object;
inference can be fragile when consumers spread the object and add `select`
([TanStack/query#5436](https://github.com/TanStack/query/issues/5436)).

This is the pattern Aureline should target: each compiled query becomes an exported **value** —
`{ surql: string, /* phantom */ }` typed as e.g. `AurelineQuery<Args, Result>` — that framework
adapters (TanStack hooks, paginators, live-subscription helpers) consume generically. One small
generic in the adapter, zero generics in generated per-query code.

## 5. TypeScript 5.x changes that matter

- **`satisfies`** (4.9) — lets generated config/values keep literal types while being checked
  against a contract; pairs with **`const` type parameters** (5.0) so adapter helpers preserve
  literal keys without `as const` ([overview](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-5.html)).
- **`NoInfer<T>`** (5.4) — lets adapter APIs pin inference to the query value and block widening
  from other argument positions ([release notes](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-4.html),
  [Total TypeScript](https://www.totaltypescript.com/noinfer)).
- **`--isolatedDeclarations`** (5.5) — rewards fully annotated exports with parallel, checker-free
  declaration emit ([release notes](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-5.html)).
  Generated code can trivially be 100% annotated; type-level libraries can't. Same story for
  Node's type stripping / `--erasableSyntaxOnly`: generated code should avoid `enum` and
  `namespace` so it survives strip-only toolchains.

Net effect: TS 5.x consistently rewards *explicit, annotated, literal-preserving* code — i.e. the
codegen side of the spectrum — while adding nothing that makes deep inference cheaper.

## 6. Implications for Aureline's generated TypeScript

**Emit:**
- **Named `interface`s** for every row/args shape, and named `type` aliases for literal unions —
  names show up in hovers and errors, and interfaces are identity-cached by the compiler
  (per [Prisma's analysis](https://www.prisma.io/blog/why-prisma-orm-checks-types-faster-than-drizzle)).
  Nest via named references, not anonymous inline object types.
- **Per-query modules** (or at least per-table), sqlc-style: `Args` interface + `Result` interface +
  one exported query value/function. Small files keep tsserver incremental checks local and let
  bundlers tree-shake unused queries' runtime (types cost nothing, runtime helpers do).
- **Reified query values** with phantom `Args`/`Result` parameters (`queryOptions` pattern) so
  hooks, paginators, and live helpers consume any query generically.
- **`RecordId<"user">`** (SurrealDB SDK class) in signatures; one generated constructor per table
  so casts never appear in user code.
- **Tagged unions per table** for live events, discriminated on `action`, with `DIFF` variants
  carrying JSON Patch unions.
- Fully annotated exports (`isolatedDeclarations`-clean), no `enum`/`namespace`, side-effect-free
  module top levels (`"sideEffects": false`).

**Avoid:**
- Deep conditional/mapped/recursive types and intersection-heavy unions in anything user-facing —
  the exact patterns behind Drizzle's IDE stalls; Aureline's Rust checker exists so tsserver never
  runs them.
- Template-literal-type computation (e.g. deriving patch paths in TS) — precompute in Rust or
  don't type it.
- `unique symbol` brands or bespoke string brands where the SDK's nominal class already serves.
- One giant barrel re-exporting every query — it couples every consumer's check scope to the whole
  schema and defeats tree-shaking of per-query runtime.
