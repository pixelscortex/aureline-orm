# SpacetimeDB client codegen — prior art for Aureline

**Date:** 2026-08-11
**Researcher:** background research agent
**Question:** How does SpacetimeDB structure multi-language client codegen, and which of (A) spec-and-generators, (B) compiler-hosts-generators, (C) monolithic does it actually use?

**Primary sources** (read directly; repo @ `master`, generated snapshots pin `cliVersion: "2.8.1"`):

- [`crates/codegen/src/lib.rs`](https://github.com/clockworklabs/SpacetimeDB/blob/master/crates/codegen/src/lib.rs) (`Lang` trait, `generate()`), [`crates/cli/src/subcommands/generate.rs`](https://github.com/clockworklabs/SpacetimeDB/blob/master/crates/cli/src/subcommands/generate.rs), [`describe.rs`](https://github.com/clockworklabs/SpacetimeDB/blob/master/crates/cli/src/subcommands/describe.rs), [`crates/codegen/src/util.rs`](https://github.com/clockworklabs/SpacetimeDB/blob/master/crates/codegen/src/util.rs).
- [`crates/codegen/tests/snapshots/`](https://github.com/clockworklabs/SpacetimeDB/tree/master/crates/codegen/tests/snapshots) — verbatim generated TS / Rust / C# from one shared `module-test` fixture.
- [`crates/lib/src/db/raw_def/v10.rs`](https://github.com/clockworklabs/SpacetimeDB/blob/master/crates/lib/src/db/raw_def/v10.rs) (+ `v9`, `v8`); [`client-api-messages/src/websocket/v2.rs`](https://github.com/clockworklabs/SpacetimeDB/blob/master/crates/client-api-messages/src/websocket/v2.rs); [`sdks/rust/src/client_cache.rs`](https://github.com/clockworklabs/SpacetimeDB/blob/master/sdks/rust/src/client_cache.rs).
- In-repo docs (source for spacetimedb.com/docs): [subscriptions](https://github.com/clockworklabs/SpacetimeDB/blob/master/docs/docs/00200-core-concepts/00400-subscriptions.md), [SQL reference](https://github.com/clockworklabs/SpacetimeDB/blob/master/docs/docs/00300-resources/00200-reference/00400-sql-reference.md), [HTTP API](https://github.com/clockworklabs/SpacetimeDB/blob/master/docs/docs/00300-resources/00200-reference/00200-http-api/00300-database.md), [`spacetime.json`](https://github.com/clockworklabs/SpacetimeDB/blob/master/docs/docs/00300-resources/00200-reference/00100-cli-reference/00300-spacetime-json.md), [TypeScript reference](https://github.com/clockworklabs/SpacetimeDB/blob/master/docs/docs/00200-core-concepts/00600-clients/00700-typescript-reference.md).
- Third party: [`l3dotdev/godot-spacetimedb-sdk`](https://github.com/l3dotdev/godot-spacetimedb-sdk) — unofficial GDScript SDK with its own generator.

**Could not confirm:** no RFC or ADR in the repo explains the codegen architecture as a recorded decision; no documented extension point for third-party generators; no changelog entry framing the artifact as a contract.

---

## 1. Which architecture is it?

**It is (C) monolithic, wearing (B)'s config file, with an accidental (A) escape hatch that is undocumented in the CLI but documented in the HTTP API.**

### 1.1 Invocation

```
spacetime generate [DATABASE] --lang <LANG> [--module-path <DIR> | --bin-path <PATH> | --js-path <PATH>]
                   [--out-dir <DIR> | --uproject-dir <DIR>] [--unreal-module-name <NAME>] [OPTIONS]
```

`--lang` is a **closed Rust enum**, not a string that resolves to a plugin: `pub enum Language { Csharp, TypeScript, Rust, UnrealCpp }` ([`generate.rs`](https://github.com/clockworklabs/SpacetimeDB/blob/master/crates/cli/src/subcommands/generate.rs)). Other flags: `--out-dir` defaults per language (`src/module_bindings` for Rust/TS, `module_bindings` for C#), language is auto-detected from `package.json` / `Cargo.toml` / `*.csproj` when omitted, and `--include-private` opts private tables in.

The default path builds the module from source (WASM), then shells out to `spacetimedb-standalone extract-schema <wasm>` to *run* the module and ask it for its own schema. That is the key structural difference from Aureline: SpacetimeDB's schema is executed out of a compiled artifact, not parsed out of source.

### 1.2 The (B)-shaped part: `spacetime.json`

```json
{
  "database": "my-game",
  "module-path": "./server",
  "generate": [
    { "language": "typescript", "out-dir": "./web/src/bindings" },
    { "language": "csharp", "out-dir": "./unity/Assets/Bindings", "namespace": "MyGame.Bindings" }
  ]
}
```

This is Prisma's `generator client { provider = ... }` ergonomics — declarative, multi-target, one command regenerates everything. But `language` is validated against the closed enum. **There is no `provider` string, no plugin resolution, no `node_modules`/`$PATH` lookup.** The config buys UX, not extensibility.

### 1.3 The IR: `RawModuleDef`, and it *is* versioned

There is a real serialisable IR, explicitly ABI-versioned, and the V10 doc comments show they take wire compatibility seriously:

```rust
pub enum RawModuleDef { V8BackCompat(RawModuleDefV8), V9(RawModuleDefV9), V10(RawModuleDefV10) }

/// A section of a V10 module definition.
/// New variants MUST be added to the END of this enum, to maintain ABI compatibility.
#[non_exhaustive]
pub enum RawModuleDefV10Section { Typespace(..), Types(..), Tables(..), Reducers(..), /* ... */ }
```

`RawModuleDef` is the *raw, possibly-invalid* form. It is validated into `spacetimedb_schema::def::ModuleDef`, and **codegen consumes the validated `ModuleDef`**, never the raw one. That two-stage split (raw wire form → validated in-memory form) is worth stealing outright.

### 1.4 Is the IR a public contract? Yes — via HTTP, not via the CLI

Documented, versioned, publicly reachable:

> `GET /v1/database/:name_or_identity/schema`
> Query parameter `version` — "The version of `RawModuleDef` to return, e.g. 9."
> "Returns a `RawModuleDef` in JSON form."
> "No authorization is required to fetch a database's schema."

— [HTTP API reference](https://github.com/clockworklabs/SpacetimeDB/blob/master/docs/docs/00300-resources/00200-reference/00200-http-api/00300-database.md)

The docs even inline a full example JSON payload (typespace / tables / reducers). `spacetime describe <db> --json` is the CLI mirror of the same endpoint — but note it prints `eprintln!("{UNSTABLE_WARNING}")` first.

Meanwhile the *input* side of the same artifact is deliberately hidden:

```rust
Arg::new("json_module")
    .hide(true)                       // <-- not in the CLI reference docs
    .long("module-def")
    .help("Generate from a ModuleDef encoded as json"),
```

It reads from a file path or from **stdin**, which is exactly Aureline's proposed `--emit-artifact | my-generator` shape — but SpacetimeDB ships it as an internal testing affordance and does not document it. I confirmed it is absent from the generated [CLI reference](https://github.com/clockworklabs/SpacetimeDB/blob/master/docs/docs/00300-resources/00200-reference/00100-cli-reference/00100-cli-reference.md).

### 1.5 Can a third party write a generator? Yes — and someone did

Two paths exist, both partly accidental:

**Rust path.** `spacetimedb_codegen::Lang` is `pub`, and `generate(module: &ModuleDef, lang: &dyn Lang, options: &CodegenOptions) -> Vec<OutputFile>` takes a trait object. An external crate *can* implement `Lang`. But the crate is effectively unpublished: `spacetimedb-codegen` on crates.io is at **1.3.0 (2025-08-01)** while `spacetimedb-lib` and `spacetimedb-schema` are at **2.8.1**. The codegen crate has been left behind by five minor versions. That is the clearest signal it is not an intended extension point.

**HTTP path — the one actually used.** The unofficial [Godot SDK](https://github.com/l3dotdev/godot-spacetimedb-sdk) ships its own GDScript generator (`addons/SpacetimeDB/GodotHelpers/code_gen.gd`, 732 lines) that fetches the schema itself:

```gdscript
uri += "/v1/database/" + module_name + "/schema?version=9"
...
generated_files.append_array(codegen._on_request_completed(json, parse_module_name))
```
— [`spacetime.gd`](https://github.com/l3dotdev/godot-spacetimedb-sdk/blob/main/godot%20client/addons/SpacetimeDB/spacetime.gd)

It pinned `version=9` and never touched SpacetimeDB's source. **The versioned JSON artifact is what made a third-party generator possible.** The cost: it requires a *live published database*, so the Godot workflow is "upload your module first, then generate" — you cannot generate offline from source.

Counter-example on the same axis: the official [Python SDK](https://github.com/clockworklabs/spacetimedb-python-sdk) has not been pushed since **2024-05-02** and no `python` variant exists in `Language`. A first-party language backend that fell out of the monolith simply died.

### 1.6 Repo layout and how much is shared

All backends live in one crate, one file each. Bytes in `crates/codegen/src/`: `unrealcpp.rs` 281,132 · `rust.rs` 80,201 · `csharp.rs` 69,662 · `typescript.rs` 65,232 · `cpp.rs` 25,816 — versus **shared** (`lib.rs` + `util.rs` + `code_indenter.rs`) **19,373**.

Shared code is **~3.6%** of the codegen crate. The `Lang` trait is the only real seam, and it is coarse — file-per-entity string emission:

```rust
pub struct OutputFile { pub filename: String, pub code: String }

pub trait Lang {
    fn generate_table_file_from_schema(&self, module: &ModuleDef, tbl: &TableDef, schema: TableSchema) -> OutputFile;
    fn generate_type_files(&self, module: &ModuleDef, typ: &TypeDef) -> Vec<OutputFile>;
    fn generate_reducer_file(&self, module: &ModuleDef, reducer: &ReducerDef) -> OutputFile;
    fn generate_procedure_file(&self, module: &ModuleDef, procedure: &ProcedureDef) -> OutputFile;
    fn generate_global_files(&self, module: &ModuleDef, options: &CodegenOptions) -> Vec<OutputFile>;
}
```

What `util.rs` *does* share is the part that matters most for correctness: deterministic ordering. Every iterator is `.sorted_by_key(...)` with the comment *"Sorting is necessary to have deterministic reproducible codegen."* Visibility filtering (`CodegenVisibility::OnlyPublic`) is also shared, so no backend can accidentally leak a private table.

---

## 2. The generated output

All excerpts below are verbatim from `crates/codegen/tests/snapshots/`, generated from the same `module-test` fixture — so the three languages are directly comparable.

### 2.1 Row types

**TypeScript** (`person_table.ts`) — not a type, a *schema builder value*; the type is inferred (`export type Person = __Infer<typeof Person>` in `types.ts`):

```typescript
export default __t.row({ id: __t.u32().primaryKey(), name: __t.string(), age: __t.u8() });
```

**Rust** (`person_type.rs`) — a plain struct plus marker impls:

```rust
#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub struct Person { pub id: u32, pub name: String, pub age: u8 }
impl __sdk::InModule for Person { type Module = super::RemoteModule; }
```

**C#** (`Types/Person.g.cs`) — attribute-driven. Note `partial`: codegen deliberately leaves room for a *second* generator (the C# source generator in `bindings-csharp`) to add serialisation.

```csharp
[SpacetimeDB.Type]
[DataContract]
public sealed partial class Person
{
    [DataMember(Name = "id")] public uint Id;
    [DataMember(Name = "name")] public string Name;
    [DataMember(Name = "age")] public byte Age;
}
```

### 2.2 How a table is referenced

Three genuinely different answers:

**Rust — extension traits + phantom-typed handles.** No registry; the string `"person"` appears exactly once, inside generated code. Users write `ctx.db.person()`.

```rust
pub struct PersonTableHandle<'ctx> {
    imp: __sdk::TableHandle<Person>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}
pub trait PersonTableAccess { fn person(&self) -> PersonTableHandle<'_>; }
impl PersonTableAccess for super::RemoteTables {
    fn person(&self) -> PersonTableHandle<'_> {
        PersonTableHandle { imp: self.imp.get_table::<Person>("person"), ctx: PhantomData }
    }
}
```

**C# — nested handle classes registered in a constructor.** A registry, but a typed one:

```csharp
public sealed partial class RemoteTables : RemoteTablesBase {
    public RemoteTables(DbConnection conn) { AddTable(Person = new(conn)); /* ... */ }
}
public sealed class PersonHandle : RemoteTableHandle<EventContext, Person> {
    public override string RemoteTableName => "person";
    protected override object GetPrimaryKey(Person row) => row.Id;
}
```

**TypeScript — an explicit module-level registry object**, the most data-driven of the three. `tables.person` is both a reference *and* a query builder; types are derived from the value via `typeof tablesSchema.schemaType`.

```typescript
const tablesSchema = __schema({
  person: __table({
    name: 'person',
    indexes: [ { accessor: 'age', name: 'person_age_idx_btree', algorithm: 'btree', columns: ['age'] }, /* ... */ ],
    constraints: [ { name: 'person_id_key', constraint: 'unique', columns: ['id'] } ],
  }, PersonRow),
});
export const tables: Tables = __withTableAccessorAliases(tablesBase, true) as Tables;
```

The size difference is stark: the TS snapshot is **895 lines** where Rust is **4453** and C# **2698** — for identical input. TS pushes work into generic runtime types; Rust/C# emit per-table code.

### 2.3 Type mapping and types the host language lacks

TypeScript maps `i64/u64/i128/u128/i256/u256` → `bigint`, everything smaller → `number`, `Array<u8>` → `__t.byteArray()`. Domain scalars get first-class builders rather than being smuggled through strings: `__t.identity()`, `__t.connectionId()`, `__t.timestamp()`, `__t.scheduleAt()`.

Sum types are the interesting case — Rust has them natively; C# and TS do not, and both get a tagged-union encoding. Note the getters, which exist to break definition cycles:

```typescript
// The tagged union or sum type for the algebraic type `Foobar`.
export const Foobar = __t.enum("Foobar", { get Baz() { return Baz; }, Bar: __t.unit(), Har: __t.u32() });
```

`Option<T>` is *not* mapped to `T | null` — it stays a sum (`__t.option(...)`), so `Option<Option<T>>` survives the round trip.

### 2.4 File layout, marking, and committing

Many small files, one per entity, plus a global barrel (`index.ts` / `mod.rs` / `SpacetimeDBClient.g.cs`). Naming is mechanical: `<snake_table>_table.ts`, `<snake_type>_type.rs`, `Tables/<Pascal>.g.cs`, `Reducers/<Pascal>.g.cs`.

Every file opens with `// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE / WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.`, and the barrel adds `// This was generated using spacetimedb cli version {version} (commit {git_hash}).`

That marker is **load-bearing, not decorative**. Regeneration walks the output tree, `read_exact`s the first `AUTO_GENERATED_PREFIX.len()` bytes of every file, and offers to delete any generated-looking file not emitted this run:

```rust
println!("The following files were not generated by this command and will be deleted:");
if y_or_n(force, "Are you sure you want to delete these files?")? { /* remove_file */ }
```

Writes are content-compared before touching disk (`if !path.exists() || fs::read_to_string(&path)? != code`), so no spurious mtime churn. Generated files are **committed** — docs tell you to `mkdir -p client/src/module_bindings` and import from it. Formatting runs afterwards (`rustfmt`, `dotnet format`; TypeScript formatting is a `// TODO`).

### 2.5 Runtime vs generated

Heavily runtime-weighted. Hand-written: TS `lib/type_builders.ts` alone is 127 KB, plus `sdk/db_connection_impl.ts` (44 KB), `query.ts` (33 KB), `table_cache.ts` (18.6 KB), plus framework adapters for react/svelte/solid/vue/angular/tanstack; Rust `db_connection.rs` (68 KB), `client_cache.rs` (26 KB), `websocket.rs` (25 KB), `subscription.rs` (22 KB). Generated code is thin glue: type definitions, name→type dispatch tables, and per-table impls forwarding to `self.imp`.

---

## 3. Subscriptions and reactivity

### 3.1 Subscribing

Generated code contributes only the *typed surface*; the builder itself is runtime.

```typescript
conn.subscriptionBuilder()
  .onApplied(() => { /* initial rows now in cache */ })
  .subscribe([tables.user, tables.shopItems.where(r => r.requiredLevel.lte(5))]);
```
```rust
ctx.subscription_builder().on_applied(|ctx| { /* ... */ })
   .add_query(|q| q.from.shop_items().r#where(|r| r.required_level.lte(5)))
   .subscribe();
```
```csharp
conn.SubscriptionBuilder().AddQuery(q => q.From.ShopItems().Where(r => r.RequiredLevel.Lte(5))).Subscribe();
```

Raw SQL strings are still accepted (`subscribe(queries: string | string[])`); the typed builder is the recommended default. Subscribing returns a `SubscriptionHandle` with `isActive` / `isEnded` / `unsubscribe`. `subscribeToAllTables()` exists as a convenience and is documented as not cancelable.

### 3.2 What a change notification carries

**Full rows, always. The wire format has no concept of "update."**

```rust
pub struct PersistentTableRows {
    pub inserts: BsatnRowList,
    pub deletes: BsatnRowList,
}
```
— [`websocket/v2.rs`](https://github.com/clockworklabs/SpacetimeDB/blob/master/crates/client-api-messages/src/websocket/v2.rs)

with a TODO admitting the gap:

> *"In the future, we may add additional variants to this enum. In particular, we may add a variant for in-place updates of rows for tables with primary keys. Note that clients will need to opt in to using this new variant, to preserve compatibility of clients which predate the new variant."*

Updates are **synthesised client-side**, and this is exactly where the generated code earns its keep. The generated `DbUpdate::apply_to_client_cache` supplies the primary-key extractor per table:

```rust
diff.person = cache.apply_diff_to_table::<Person>("person", &self.person).with_updates_by_pk(|row| &row.id);
diff.test_d = cache.apply_diff_to_table::<TestD>("test_d", &self.test_d);   // no PK -> no updates
```

The runtime then pairs a delete with an insert sharing a PK — [`client_cache.rs`](https://github.com/clockworklabs/SpacetimeDB/blob/master/sdks/rust/src/client_cache.rs): *"Returns the applied diff restructured with row updates where deletes and inserts are found according to `derive_pk`."* Consequences, all visible in the generated code:

- `on_insert(&EventContext, &Row)` and `on_delete(&EventContext, &Row)` exist for **every** table.
- `on_update(&EventContext, &Row /*old*/, &Row /*new*/)` is emitted **only** for tables with a primary key — it lives behind a separate `impl __sdk::TableWithPrimaryKey`. A PK-less table simply has no `on_update` in its API. Static enforcement by omission, not by runtime error.
- Unsubscribe is modelled as deletes: `parse_unsubscribe_rows` calls `parse_row_list_as_deletes`, so leaving a subscription fires `on_delete` for every row that leaves the cache.

### 3.3 Pagination over live data — there is none

The subscription grammar is a strict subset of the query grammar:

```ebnf
SELECT ( '*' | table '.' '*' ) FROM relation [ WHERE predicate ]
```

Confirmed restrictions from the [SQL reference](https://github.com/clockworklabs/SpacetimeDB/blob/master/docs/docs/00300-resources/00200-reference/00400-sql-reference.md):

- Whole rows only — "Individual column projections are not allowed."
- One output table; at most a two-table `JOIN`, and "subscriptions require an index to be defined on both join columns."
- No arithmetic in `WHERE`. No `INSERT`/`DELETE`.
- **No `LIMIT`.** `LIMIT` exists only in the one-off query language: *"The query language is a strict superset of the subscription language."*

The migration guide says the quiet part out loud — Convex's `Pagination` maps to *"Limit/range query, cursor table, or application-level pagination… Model pagination around stable ordering columns, usually timestamps or monotonic IDs."* **SpacetimeDB does not solve consistent pagination over live data; it hands it back to you.** For Aureline, plan on the same answer, but plan on it deliberately rather than by omission.

---

## 4. Cross-language symmetry

Symmetric at the *concept* level, deliberately idiomatic at the *syntax* level. The docs are built around a language-tab component so the same narrative renders per language, and the concept vocabulary — `DbConnection`, `subscriptionBuilder`, `onApplied`, `db.<table>`, `onInsert/onUpdate/onDelete`, `SubscriptionHandle` — is identical everywhere. Even the naming of state predicates is documented as a deliberate per-language spelling: *"`isActive` / `IsActive` / `is_active`."*

Where they diverge:

- **Casing:** TS/C# camel/Pascal-case identifiers; Rust snake_case. The TS backend has a dedicated test asserting `loggedOutPlayer: __table({` and *not* `logged_out_player: __table({`, plus generated `@deprecated` aliases for the old snake_case names — a real, versioned API migration living inside codegen.
- **Subscription shape:** TS takes an array (`.subscribe([a, b])`); Rust/C#/Unreal chain `.add_query(...)` / `.AddQuery(...)` then `.subscribe()`. Same semantics, different ergonomics.
- **Table access:** TS exports a module-level `tables` registry *in addition to* `conn.db.<table>`; Rust and C# have only the connection-scoped handle. TS needs the free-standing registry because its query builders are values you compose before you have a connection.
- **Depth of the type-level trick:** TS derives types from runtime values (`__Infer<typeof X>`, `satisfies __RemoteModule<...>`); Rust uses associated types and phantom lifetimes; C# uses nominal subclassing. Same guarantees, three different type-system idioms.

I could not find any document stating these divergences as an explicit, recorded decision — they are visible in tests and docs, but there is no ADR/RFC.

---

## 5. Honest verdict

### Copy

1. **A validated IR distinct from the raw serialised IR.** `RawModuleDefV{8,9,10}` (wire, possibly invalid, `#[non_exhaustive]`, "new variants MUST be added to the END") → validated `ModuleDef` → codegen only ever sees the validated form. The single best idea here, and it costs nothing to adopt.
2. **Version the artifact in its name and let consumers pin it.** `?version=9` is why a stranger could ship a Godot SDK.
3. **The auto-generated banner as a delete-safety protocol.** Marking files so regeneration can *safely* remove stale output — with confirmation, and content-compare before writing — kills a whole class of "I renamed a table and there's a ghost file" bugs.
4. **Deterministic ordering as a shared invariant, not a per-backend habit.** Byte-identical output on identical input makes generated code diffable and CI-checkable.
5. **Snapshot-test every backend against one shared fixture,** filtering out the version stamp so it does not churn. This is how you notice cross-language drift.
6. **Emit only the typed surface; put behaviour in a hand-written runtime.** 127 KB of hand-written TS type builders vs 895 lines of generated output.
7. **Encode capability by omission** (no PK ⇒ no `on_update` method exists), and **make generated types `partial`/extensible** so a second stage can bolt on without editing generated files.

### Avoid

1. **A closed `--lang` enum as the only entry point.** It made the Python SDK's death silent and forced the Godot author to reverse-engineer the JSON. If you want third-party generators, the seam must be the thing you *document and test*, not the thing you `.hide(true)`.
2. **Publishing the codegen crate and then not maintaining it.** `spacetimedb-codegen` at 1.3.0 against a 2.8.1 workspace is worse than not publishing at all — it advertises an extension point that does not work.
3. **A `Lang` trait whose only currency is `(filename, String)`.** With 3.6% shared code, "add a language" means "write 60–280 KB of string-formatting from scratch." The Unreal backend at 281 KB is the warning sign. If Aureline expects 3+ languages, invest in a shared, language-agnostic mid-level model (rendered types, rendered declarations) *above* string emission.
4. **Making the only public artifact require a live server.** `GET /v1/database/:name/schema` needs a published database. Aureline's whole premise is offline static checking; the artifact must be obtainable from source with no server.
5. **Leaving pagination undefined.** It is genuinely hard, but SpacetimeDB's silence pushes every user into ad-hoc cursor tables.

### Where SpacetimeDB's constraints differ from Aureline's — read this before copying anything

SpacetimeDB **is** the database. It owns the storage engine, the query planner, the WebSocket protocol, and the client cache. That ownership is load-bearing for most of what looks elegant above:

- **The client cache only works because the server owns the wire protocol.** `BsatnRowList`, zero-copy subscription dedup, `TransactionUpdate` — all of it presupposes a server that will push you exactly the rows in your query set. SurrealDB's LIVE SELECT has its own semantics and its own delivery guarantees, and Aureline cannot change them. **Do not design an Aureline client cache by analogy; design it against what SurrealDB actually pushes.**
- **`on_update(old, new)` is a client-side fiction paid for by the server sending full rows for both sides of every change.** If SurrealDB's live notifications carry a diff, a patch, or only the new row, this API is not reproducible — you would need to keep the prior row in a cache to synthesise `old`, which changes memory characteristics and correctness under reconnect.
- **Query builders that compile to a restricted subscription dialect only make sense if you control the dialect.** SpacetimeDB could forbid `LIMIT` and 3-table joins because it wrote the planner. Aureline's builders must compile to SurrealQL that SurrealDB will actually accept, and the restriction set is not Aureline's to choose — it must be *discovered and encoded* as a static check, which is harder and more valuable.
- **`extract-schema` runs the module.** SpacetimeDB's IR is produced by executing a WASM binary. Aureline's IR comes from parsing `.aurl` and type-checking it, with no execution and no database. That is strictly *better* for the artifact story: Aureline can emit its artifact from a git checkout in CI, offline, deterministically — something SpacetimeDB cannot do without a build toolchain and a standalone host binary.

Net: copy the **artifact discipline** (versioned raw IR → validated IR → codegen; determinism; snapshots; delete-safe markers). Do not copy the **reactivity architecture** without first establishing what SurrealDB's live query protocol actually delivers.

---

## Recommendation for Aureline

**Take (A) — spec-and-generators — with a first-party monolith shipped in the same binary.** Concretely: `aureline generate --lang typescript` and `aureline generate --emit-artifact` are both first-class, and the built-in TypeScript backend consumes the artifact by exactly the same path a third party would.

The reasoning turns on the constraint that Aureline does not own the database.

1. **Not owning the database means the compiler's real product is the artifact, not the code.** SpacetimeDB can get away with (C) because its generated client is inseparable from its wire protocol — the codegen and the server ship together, version together, and are meaningless apart. Aureline's compiler produces a *description of a schema that lives in someone else's database*. That description has value to consumers Aureline will never write: an ERD renderer, a seed-data generator, a Zod/Pydantic emitter, a docs site, a lint rule, an LLM tool definition. Bottling that behind a closed `--lang` enum throws away the most reusable thing the compiler makes.

2. **The Godot SDK is the proof, and its constraint is the lesson.** A stranger shipped a working generator for an unsupported language *purely* because `?version=9` existed. But they had to publish a live database first. Aureline has no such requirement — it parses source. So Aureline can offer strictly more than SpacetimeDB does: a versioned artifact obtainable offline, from a git checkout, in CI. That is a real advantage and it should be the headline.

3. **(B) is the wrong trade at Aureline's size.** Prisma's plugin model means resolving and executing arbitrary binaries, defining a plugin ABI, handling version negotiation and error propagation across a process boundary — before there are three languages, let alone third parties. Note that SpacetimeDB *adopted (B)'s config file and rejected (B)'s plugin resolution*, which is a good instinct. Aureline should do the same: keep `generate` entries in config for multi-target UX; keep the language list closed *in the first-party binary*; make the pipe the extension point.

4. **The stated cost of (A) — artifact-as-contract, generator skew — is manageable, and SpacetimeDB shows how.** Version the artifact explicitly (`{"aurelineArtifactVersion": 1, ...}`), make new fields additive-only, ship a JSON Schema for it, and snapshot-test the artifact itself alongside the generated output. Skew between compiler and generator is exactly what a version field plus a compiler-version stamp in the generated banner is for. And crucially: because the first-party TypeScript generator reads the same artifact, the contract is exercised on every build. SpacetimeDB's `--module-def` rotted precisely because nothing normal depended on it.

**Concrete shape:**

- `aureline generate --emit-artifact` → versioned JSON on **stdout**, diagnostics on **stderr**, non-zero exit on type errors. `aureline generate --lang typescript --out-dir src/db` → the same pipeline internally.
- Two-stage IR: a raw serialisable `AurelineArtifactV1` (additive-only, `#[non_exhaustive]`-equivalent) validated into an internal `Schema` that generators consume. Backends never touch the raw form.
- Ship the artifact's JSON Schema in the repo and version it alongside the compiler.
- Sort everything. Snapshot-test the artifact and every backend's output against one shared `.aurl` fixture.
- Banner every generated file with a stable prefix + compiler version; use it for delete-safe regeneration and content-compare writes.
- Publish the shared IR as a library (Rust crate + npm types) **and keep it published** — or do not publish it at all.
- Defer the reactivity API until SurrealDB's LIVE SELECT payload shape is nailed down empirically. Whether Aureline can offer `onUpdate(old, new)` is a fact about SurrealDB, not a design choice.
