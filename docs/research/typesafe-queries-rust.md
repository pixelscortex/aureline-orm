# Type-safe query results in Rust — prior art for Aureline codegen

**Date:** 2026-08-13
**Researcher:** background research agent
**Question:** How does state-of-the-art Rust database tooling achieve fully type-safe query results, and what should Aureline's generated Rust look like given that its compiler precomputes each query's result type (the host language only carries types, never computes them)?

**Primary sources** (read directly):

- [sqlx `query!` macro docs](https://docs.rs/sqlx/latest/sqlx/macro.query.html), [sqlx README](https://github.com/launchbadge/sqlx) — compile-time checked queries, offline mode.
- [Diesel getting-started guide](https://diesel.rs/guides/getting-started) — `table!`/`schema.rs`, `Queryable`/`Selectable`, `check_for_backend`.
- [SeaORM entity structure docs](https://www.sea-ql.org/SeaORM/docs/generate-entity/entity-structure/), [`sea_orm::entity`](https://docs.rs/sea-orm/latest/sea_orm/entity/index.html) — `DeriveEntityModel`, Entity/Model/ActiveModel split.
- [cornucopia README](https://github.com/cornucopia-rs/cornucopia) and the clorinde book: [supported types](https://github.com/halcyonnouveau/clorinde/blob/main/docs/src/introduction/types.md), [using queries](https://github.com/halcyonnouveau/clorinde/blob/main/docs/src/using_queries/using_queries.md), [ergonomic parameters](https://github.com/halcyonnouveau/clorinde/blob/main/docs/src/using_queries/ergonomic_parameters.md). Cornucopia 1.0 merged the clorinde fork's rewritten codegen back into the project.
- surrealdb crate v3.2.4 (2026-08-03): [crate root](https://docs.rs/surrealdb/latest/surrealdb/), [`surrealdb::types` module](https://docs.rs/surrealdb/latest/surrealdb/types/index.html), [Rust SDK live-query docs](https://surrealdb.com/docs/sdk/rust/concepts/live), [Rust SDK tips blog](https://surrealdb.com/blog/tips-and-tricks-on-using-the-rust-sdk), [`query` method docs](https://surrealdb.com/docs/languages/rust/methods/query).
- Rust language status: [async closures RFC 3668](https://rust-lang.github.io/rfcs/3668-async-closures.html), [`AsyncFn`](https://doc.rust-lang.org/beta/core/ops/trait.AsyncFn.html) (stable since 1.85, not dyn-compatible), [async-trait crate docs](https://docs.rs/async-trait) (AFIT stable since 1.75; dyn dispatch still excluded).

**Could not confirm:** whether surrealdb 3.x `IndexedResults::take` still accepts plain serde `Deserialize` types or now requires `SurrealValue` exclusively (the 3.x examples derive `SurrealValue`; older 2.x examples derive `serde::Deserialize`). Treat conversion-trait choice as a version-sensitive decision.

---

## 1. Four architectures for type safety

**sqlx — prove it at compile time against a live DB.**
`query!()` connects to a development database during macro expansion (or reads a committed `.sqlx` offline cache, gated by `SQLX_OFFLINE`), asks the DB itself to prepare the statement, and emits an *anonymous* record type — one Rust field per column, so misspelled columns and type mismatches fail the build ([macro docs](https://docs.rs/sqlx/latest/sqlx/macro.query.html)). The anonymous type cannot be named or returned from functions, which sqlx concedes is "the biggest downside"; `query_as!()` exists to target a user-named struct instead. Nullability inference is imperfect, so sqlx ships per-column override syntax (`column as "column!"` to force non-null, `"column: T"` to force a type). Lessons for Aureline:

- The DB (here: Aureline's checker) is the type oracle; the host-language artifact is just a projection of it.
- Un-nameable result types are a dead end — every row type must be a nameable struct.
- The `.sqlx` offline cache is precedent for committing precomputed query metadata to the repo.

**Diesel — encode SQL in the trait system.**
The `diesel` CLI generates `schema.rs` from the live schema; the [`table!` macro](https://docs.diesel.rs/2.3.x/diesel/macro.table.html) "creates a bunch of code based on the database schema to represent all of the tables and columns" ([guide](https://diesel.rs/guides/getting-started)). Queries are then built from typed column tokens and checked by the trait system. Two documented footguns: `#[derive(Queryable)]` maps rows *by field order* ("assumes that the order of fields on the `Post` struct matches the columns"), mitigated by `#[derive(Selectable)]` + `.as_select()`; and error messages are bad enough that an opt-in `#[diesel(check_for_backend(...))]` attribute exists purely to improve them. Safety is real but lives in deeply nested generics — compile time and diagnostics are the tax, and the type computation happens *inside* Rust, which is exactly what Aureline does not need.

**SeaORM — dynamic ORM, safety at the entity edge.**
`DeriveEntityModel` "does all the heavy lifting" of expanding one annotated `Model` struct into `Entity`, `Column`, `PrimaryKey`, and `ActiveModel` (every field wrapped in `ActiveValue` for partial updates) ([entity docs](https://www.sea-ql.org/SeaORM/docs/generate-entity/entity-structure/)). Column tokens prevent referencing non-existent columns, but the shape of a custom `SELECT` is only checked at runtime via `into_model`/`FromQueryResult` — SeaORM is deliberately "dynamic". It is the closest analogue to the surrealdb SDK's current posture, and the gap Aureline exists to close.

**cornucopia/clorinde and the sqlc model — generate plain code from SQL.**
Queries live in `.sql` files with `--! name` annotations and `:named` params; the tool prepares them against a real Postgres, runs a validation suite, then generates a *separate crate* of plain structs and functions — "ergonomic and free of heavy macros or complex generics", explicitly so users "can easily build upon the generated items" ([cornucopia README](https://github.com/cornucopia-rs/cornucopia)). Per query it emits ([using queries](https://github.com/halcyonnouveau/clorinde/blob/main/docs/src/using_queries/using_queries.md)):

- a query function (`authors()`) returning a typed query object;
- a params struct (`AuthorsParams { country: Option<&str> }`) as an alternative to positional `.bind(&client, args...)`;
- a row struct per query, with borrowed variants for zero-copy reads;
- umbrella param traits so `String`/`&str`/`Cow<'_, str>`/`Box<str>` all satisfy a `StringSql` bound ([ergonomic parameters](https://github.com/halcyonnouveau/clorinde/blob/main/docs/src/using_queries/ergonomic_parameters.md));
- `.map()` to transform rows without intermediate allocation (result is another query object);
- cardinality selectors `opt` / `one` / `iter` / `all` — expected row count is part of the API, and statement-only queries skip the query object entirely and just return affected-row counts.

## 2. Which model fits a "types are precomputed elsewhere" pipeline

The clorinde/sqlc model is the direct fit. Aureline's compiler already plays the role clorinde delegates to Postgres `PREPARE`: it knows each query's exact result type ahead of time. Generating plain structs + plain functions means: no proc-macro in the user's build graph, no trait-solving in error paths, rustdoc-able output, `cargo expand`-free debugging, and generated code that users can extend (clorinde's stated design goal). Trait-heavy builders (Diesel) re-derive types *inside* Rust's trait system — pure duplication of work Aureline has already done, paid for in compile time and error quality. The only traits worth generating are thin, hand-shaped ones (one `Query` trait, one value-conversion derive), not a combinator algebra.

## 3. The surrealdb crate's own typing story (v3.x)

- `db.query("...").await?` yields `IndexedResults` (named `Response` in 2.x); `.take(index)` extracts the *n*-th statement's result into any convertible type: `let people: Vec<Person> = db.query("SELECT * FROM person").await?.take(0)?;`. Statement index and target type are both unchecked at compile time — exactly the gap Aureline closes.
- Conversion goes through the `SurrealValue` trait ("type-safe conversion between Rust types and SurrealDB values") with a `#[derive(SurrealValue)]`; 2.x code used serde `Deserialize` and hit well-known friction deserializing `Thing`/`Datetime` wrappers ([issue #5123](https://github.com/surrealdb/surrealdb/issues/5123), [#2421](https://github.com/surrealdb/surrealdb/issues/2421)).
- `surrealdb::types` provides: `RecordId` (struct of table + `RecordIdKey`) — **untyped**: a `person` id and a `product` id are the same Rust type; `Datetime` (crate depends on chrono ^0.4); `Duration`; `Uuid`; `Bytes`; `File`; `Range`; `Geometry`; and `Number { Int, Float, Decimal }` where `Decimal` is a 128-bit fixed-precision value (m/10^e, e ≤ 28 — rust_decimal's representation). There is also a `Kind` enum reifying SurrealQL's type system (e.g. `Kind::Array(Box<Kind>, Option<usize>)`) — useful as a cross-check target for Aureline's inferred kinds.
- Live queries: `.select(...).live()` returns a `Stream` of `Notification<T> { query_id, action, data }` with `Action::{Create, Update, Delete}`. Per the SDK docs, a DELETE notification's `data` carries the record state at deletion time, not just an id — so if Aureline's contract says DELETE delivers only the id, the generated notification enum must give Delete its own payload type rather than reusing the row struct.

## 4. Representing SurrealDB types in generated Rust

- **Typed record ids:** newtype over the SDK id with a phantom table marker:

  ```rust
  pub struct RecordId<T: Table>(surrealdb::types::RecordId, PhantomData<T>);
  pub enum Product {}                       // zero-sized table marker
  impl Table for Product { const NAME: &'static str = "product"; }
  ```

  with a `#[serde(transparent)]`-style delegating (de)serialization/`SurrealValue` impl. `PhantomData` is free at runtime; `product.id: RecordId<Product>` makes cross-table id confusion a compile error, which the raw SDK's untyped `RecordId` cannot. Deserialization should still verify the wire-side table name against `T::NAME` (a `person:1` arriving where `RecordId<Product>` is expected must be a runtime error, not a silent cast).
- **Literal unions** (`"draft" | "published"`): generated fieldless enum with `#[serde(rename = "...")]` per variant (or the `SurrealValue` equivalent). Closed sets should be enums, not `String` — invalid states unrepresentable. For heterogeneous literal unions (`"none" | 0 | true`), generate an untagged enum with one variant per literal kind.
- **Live notifications:** generate a per-query event enum mapped from the SDK's `Notification`/`Action`:

  ```rust
  pub enum ProductEvent {
      Create(ProductRow),
      Update(ProductRow),
      Delete(RecordId<Product>),   // if Aureline's contract says DELETE ships only the id
  }
  ```

  i.e. an internally-tagged shape with `action` as the tag, where the Delete variant has its own payload type. Do not reuse the row struct for a payload that doesn't carry a full row.
- **Optionality:** distinguish *nullable* (value may be NONE/NULL → `Option<T>`, always serialized) from *absent-able* (field may be missing → `#[serde(default)]`, or `#[serde(skip_serializing_if = "Option::is_none")]` on write paths). SurrealQL's NONE-vs-missing distinction means the codegen must track both bits per field, as sqlx's nullability-override syntax concedes even SQL needs.
- **Scalars:** follow both surrealdb (chrono, rust_decimal-compatible `Decimal`, uuid, `Bytes`) and clorinde's table (chrono `DateTime`/`NaiveDate`/`NaiveTime`, `rust_decimal::Decimal`, `uuid::Uuid`, `serde_json::Value`, `Vec<u8>`). Recommended: `datetime → chrono::DateTime<Utc>`, `decimal → rust_decimal::Decimal`, `duration → std::time::Duration` (SurrealDB durations are unsigned), `uuid → uuid::Uuid`, `bytes → surrealdb::types::Bytes` or `Vec<u8>`. Re-export chosen crates from the generated crate so user code never adds mismatched versions.

## 5. Ergonomics traps and modern-Rust calculus

- **Paginator borrows:** an API shaped like

  ```rust
  let mut pages = paginator.pages();          // borrows paginator (and maybe the client)
  while let Some(page) = pages.next().await { // borrow held across every await
  ```

  holds a mutable borrow of the paginator for the whole loop; if `pages()` also borrows the client, the client is unusable inside the loop body, and any design where `next()` yields items *borrowing the paginator's buffer* is a lending stream — inexpressible on stable Rust (no generic associated lifetime in `Stream::Item`). Fixes proven in the ecosystem: make streams **own** their state (clorinde's `iter()` consumes the query object; SeaORM's `Paginator::fetch_and_next` keeps state inside a struct that only borrows the connection immutably), and make the client handle cheaply cloneable — `Surreal` is `Clone` and explicitly usable as a `LazyLock` static ([SDK setup docs](https://surrealdb.com/docs/sdk/rust/setup)), i.e. Arc-style sharing internally, so owned streams can capture a clone.
- **Interior mutability:** if generated state must be shared, prefer the cheap-clone-handle pattern (Arc inside, `&self` API) over exposing `Arc<Mutex<...>>`; a mutex guard held across `.await` is a deadlock-shaped API. Best answer for Aureline: the generated layer stays entirely stateless — free functions and owned futures/streams — so the question never arises.
- **Async traits in 2026:** `async fn` in traits (AFIT) and return-position `impl Trait` in traits (RPITIT) are stable since Rust 1.75, so a generated `Query`/executor trait can have async methods with zero dependencies — but such traits are **not dyn-compatible**; `dyn Query` needs `async-trait`-style boxing ([async-trait docs](https://docs.rs/async-trait)). Async closures and the `AsyncFn{,Mut,Once}` traits are stable since 1.85 / edition 2024 ([RFC 3668](https://rust-lang.github.io/rfcs/3668-async-closures.html)) — good for async `map`-style row transforms — but [`AsyncFn` is likewise not dyn-compatible](https://doc.rust-lang.org/beta/core/ops/trait.AsyncFn.html). Conclusion: design the generated API around static dispatch and concrete named types; never require `dyn`.

## 6. Recommended shape for Aureline's generated Rust

```rust
// generated: one module per query
pub struct FindProductsArgs { pub min_price: rust_decimal::Decimal }   // struct-per-query-args
pub struct FindProductsRow {                                            // struct-per-row
    pub id: RecordId<Product>,
    pub status: ProductStatus,          // enum for "draft" | "published"
    pub tags: Option<Vec<String>>,      // nullable per inferred kind
}
pub struct FindProducts;
impl Query for FindProducts {
    const TEXT: &'static str = "SELECT * FROM product WHERE price > $min_price";
    type Args = FindProductsArgs;
    type Row  = FindProductsRow;        // cardinality decides Vec<Row> / Option<Row> / Row
}
```

Concretely:

- **Struct-per-row and struct-per-query-args**, plain and nameable, with by-name (not positional) field mapping and doc comments carrying the source query — the clorinde/sqlc model, since Aureline's checker already did the type computation clorinde outsources to Postgres.
- **One small `Query` trait carrying `TEXT` + `Args` + `Row`**, plus a cardinality-aware `Output` associated type mirroring clorinde's `opt`/`one`/`all` (checker-inferred `Vec<Row>` vs `Option<Row>` vs `Row`), and a statement index if a multi-statement script maps onto `IndexedResults::take(n)`.
- **One generic runtime shim**, `async fn run<Q: Query>(db: &Surreal<C>, args: Q::Args) -> Result<Q::Output>`, written once by hand using AFIT/RPITIT — the only place that touches `.query().bind().take()`.
- **Plain derives** (`SurrealValue` and/or serde) on generated structs; re-export chrono/rust_decimal/uuid from the generated crate; owned streams for live queries and pagination.

**Avoid:**

- proc-macros or `build.rs` DB access in the user's build — Aureline's compiler already ran; emit source text.
- anonymous/un-nameable result types (sqlx's admitted weakness) and field-order-coupled row mapping (Diesel's `Queryable` footgun).
- trait-encoded query algebras and deep generics — types are precomputed; re-deriving them in the trait system buys nothing and costs compile time and diagnostics.
- `dyn` anywhere on the generated surface (AFIT/`AsyncFn` are not dyn-compatible).
- streams, builders, or paginators that borrow the client or require a long-lived `&mut` — own state, capture a cloned handle.
- `String`-typed record ids and stringly-typed literal unions; conflating NONE/NULL with absent-field.
